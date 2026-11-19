use serde::Deserialize;
use url::Url;

pub const MAX_INPUT: usize = 4096;
pub const HOME: &str = "about:blank";

#[derive(Clone)]
pub struct Policy {
    origins: Option<[String; 2]>,
}

impl Policy {
    pub fn browsing() -> Self {
        Self { origins: None }
    }

    pub fn fixture(first: &str, second: &str) -> Option<Self> {
        let a = fixture_origin(first)?;
        let b = fixture_origin(second)?;
        (a != b).then_some(Self {
            origins: Some([a, b]),
        })
    }

    pub fn accepts(&self, input: &str) -> bool {
        if input == HOME {
            return true;
        }
        let Some(url) = web_url(input) else {
            return false;
        };
        self.origins
            .as_ref()
            .is_none_or(|origins| origins.contains(&url.origin().ascii_serialization()))
    }

    pub fn destination(&self, input: &str) -> Option<String> {
        let input = input.trim();
        if input.is_empty() || input.len() > MAX_INPUT || input.chars().any(char::is_control) {
            return None;
        }
        if input == HOME {
            return Some(HOME.to_string());
        }
        // Host:port is useful for stably addressing synthetic local fixtures.
        let first = input.split('/').next()?;
        let host_port = first.split_once(':').is_some_and(|(host, port)| {
            (host == "localhost" || host.contains('.'))
                && !port.is_empty()
                && port.bytes().all(|byte| byte.is_ascii_digit())
        });
        let scheme = input.split_once(':').is_some_and(|(lead, _)| {
            !lead.is_empty()
                && lead.bytes().enumerate().all(|(i, b)| {
                    b.is_ascii_alphabetic()
                        || (i > 0 && (b.is_ascii_digit() || b"+-.".contains(&b)))
                })
        });
        let url = if scheme && !host_port {
            web_url(input)?
        } else if !input.contains(char::is_whitespace)
            && (host_port || first.contains('.') || first == "localhost")
        {
            let protocol = if first == "localhost"
                || first.starts_with("localhost:")
                || first == "127.0.0.1"
                || first.starts_with("127.0.0.1:")
            {
                "http"
            } else {
                "https"
            };
            web_url(&format!("{protocol}://{input}"))?
        } else {
            if self.origins.is_some() || input.starts_with(['/', '\\']) {
                return None;
            }
            let mut search = Url::parse("https://duckduckgo.com/").expect("static search URL");
            search.query_pairs_mut().append_pair("q", input);
            search
        };
        let url: String = url.into();
        self.accepts(&url).then_some(url)
    }
}

fn web_url(input: &str) -> Option<Url> {
    if input.len() > MAX_INPUT || input.chars().any(char::is_control) || input.contains('\\') {
        return None;
    }
    let (scheme, rest) = input.split_once("://")?;
    if !(scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https"))
        || rest.is_empty()
        || rest.starts_with('/')
    {
        return None;
    }
    let url = Url::parse(input).ok()?;
    (url.has_host() && url.username().is_empty() && url.password().is_none()).then_some(url)
}

fn fixture_origin(input: &str) -> Option<String> {
    let url = web_url(input)?;
    if url.scheme() != "http"
        || url.host_str() != Some("127.0.0.1")
        || url.port().is_none()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    Some(url.origin().ascii_serialization())
}

pub fn transport(input: &str) -> &'static str {
    match web_url(input).map(|url| url.scheme() == "https") {
        Some(true) => "HTTPS",
        Some(false) => "HTTP",
        None => "Local",
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(
    tag = "action",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Command {
    Navigate(String),
    Back,
    Forward,
    Reload,
    Stop,
    NewTab,
    NewPrivate,
    CloseTab,
    NextTab,
    SelectTab(u32),
    ReleaseInactive,
    FocusAddress,
    FocusPage,
    ToggleTools,
    ChooseDownloads,
    CancelDownloads,
    ToggleMute,
    Print,
}

pub fn chrome_result(raw: &str) -> Option<Command> {
    if raw.len() > MAX_INPUT * 3 {
        return None;
    }
    let command = serde_json::from_str(raw).ok()?;
    if let Command::Navigate(ref input) = command {
        if input.len() > MAX_INPUT {
            return None;
        }
    }
    Some(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_search_and_loopback_policy() {
        let policy = Policy::browsing();
        assert_eq!(
            policy.destination("example.org").as_deref(),
            Some("https://example.org/")
        );
        assert_eq!(
            policy.destination("127.0.0.1:8123/a").as_deref(),
            Some("http://127.0.0.1:8123/a")
        );
        assert_eq!(
            policy.destination("synthetic query").as_deref(),
            Some("https://duckduckgo.com/?q=synthetic+query")
        );
        let local = Policy::fixture("http://127.0.0.1:8123", "http://127.0.0.1:8124").unwrap();
        assert!(local.accepts("http://127.0.0.1:8123/path"));
        assert!(local.accepts("http://127.0.0.1:8124/redirect"));
        assert!(!local.accepts("http://127.0.0.1:8125/path"));
        assert!(!local.accepts("https://example.org"));
        assert!(local.destination("synthetic query").is_none());
    }

    #[test]
    fn malformed_file_credential_and_external_top_level_urls_are_denied() {
        let policy = Policy::browsing();
        for input in [
            "file:///app/file",
            "javascript:void(0)",
            "mailto:test@example.org",
            "data:text/plain,x",
            "muffin-chrome://app/",
            "https:///example.org",
            "http://name:word@example.org/",
            "http:\\not-a-valid-http.test",
            "https://example.org/\npath",
            "http://",
        ] {
            assert!(!policy.accepts(input), "{input}");
            assert!(policy.destination(input).is_none(), "{input}");
        }
        assert_eq!(transport("https://example.org"), "HTTPS");
        assert_eq!(transport("http://example.org"), "HTTP");
    }

    #[test]
    fn trusted_command_parser_is_closed_and_bounded() {
        assert_eq!(
            chrome_result(r#"{"action":"select_tab","value":7}"#),
            Some(Command::SelectTab(7))
        );
        assert_eq!(
            chrome_result(r#"{"action":"new_tab"}"#),
            Some(Command::NewTab)
        );
        assert!(chrome_result(r#"{"action":"read_file","value":"synthetic"}"#).is_none());
        assert!(chrome_result(r#"{"action":"reload","extra":true}"#).is_none());
        assert!(chrome_result(r#"{"action":"navigate","value":false}"#).is_none());
        assert!(chrome_result(&"x".repeat(MAX_INPUT * 4)).is_none());
    }
}
