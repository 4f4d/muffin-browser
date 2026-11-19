//! Deterministic probes against the explicitly configured synthetic origins.
use super::{Browser, Role, Signal};
use crate::policy::{Command, HOME};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    path::Path,
    time::{Duration, Instant},
};
use tempfile::NamedTempFile;

pub enum Mode {
    First,
    Relaunch,
}

pub struct Canary {
    pub file_url: String,
    _keep: NamedTempFile,
}

pub fn private_canary(root: &Path) -> Result<Canary, &'static str> {
    use std::io::Write;
    let mut file = NamedTempFile::new_in(root)
        .map_err(|_| "could not create the synthetic file-boundary canary")?;
    file.write_all(b"muffin-m0b-synthetic-private-resource")
        .map_err(|_| "could not seed synthetic file canary")?;
    let file_url = url::Url::from_file_path(file.path())
        .map_err(|_| "could not identify synthetic file canary")?;
    Ok(Canary {
        file_url: file_url.into(),
        _keep: file,
    })
}

enum Target {
    Chrome,
    Active,
    Popup,
}

enum Action {
    Command(Command),
    Script(Target, String),
    PopupClick,
    None,
}

enum Check {
    ChromeReady,
    TabCount(usize),
    IdleUrl(String),
    TabHasNoView,
    Js(Target, &'static str, Value),
    Download(u32),
    DownloadAtLeast(u32),
    Boundary(usize, String),
    Pdf(String, u32),
    Loading,
    Stopped,
    NativeDownloadProgress,
    InterruptedDownload,
    PopupClosed,
}

struct Step {
    name: &'static str,
    action: Action,
    check: Check,
    required: bool,
}

impl Step {
    fn new(name: &'static str, action: Action, check: Check) -> Self {
        Self {
            name,
            action,
            check,
            required: true,
        }
    }

    fn optional(mut self) -> Self {
        self.required = false;
        self
    }
}

pub struct Smoke {
    pending: VecDeque<Step>,
    current: Option<(Step, Instant)>,
    sequence: u32,
    awaited: Option<(u32, Instant)>,
    value: Option<Value>,
    failed: bool,
    x11_gesture: bool,
    core_only: bool,
    pending_desktop: usize,
    _canary: Canary,
}

fn script(target: Target, code: impl Into<String>) -> Action {
    Action::Script(target, code.into())
}
fn navigate(url: &str) -> Action {
    Action::Command(Command::Navigate(url.into()))
}

fn go(name: &'static str, url: &str) -> Step {
    Step::new(name, navigate(url), Check::IdleUrl(url.into()))
}

fn storage(name: &'static str, action: Action, marker: bool) -> Step {
    Step::new(name, action, Check::Js(
        Target::Active,
        "document.querySelector('#privacy-state') ? JSON.parse(document.querySelector('#privacy-state').textContent) : null",
        json!({"localStorage": marker, "cookie": marker}),
    ))
}

fn popup_checks(home: &str, core_only: bool) -> [Step; 4] {
    let mut checks = [
        go("return_to_popup_fixture", home),
        Step::new(
            "popup_opener_and_cross_origin_message",
            Action::PopupClick,
            Check::Js(
                Target::Active,
                "document.getElementById('popup-status')?.textContent",
                json!("cross-origin popup message received"),
            ),
        ),
        Step::new(
            "popup_preserved_opener",
            Action::None,
            Check::Js(
                Target::Popup,
                "document.getElementById('popup-status')?.textContent",
                json!("opener-present"),
            ),
        ),
        Step::new(
            "popup_webview_close",
            script(
                Target::Popup,
                "document.getElementById('popup-close')?.click()",
            ),
            Check::PopupClosed,
        ),
    ];
    if core_only {
        for step in &mut checks[1..] {
            step.required = false;
        }
    }
    checks
}

impl Smoke {
    pub fn new(
        mode: Mode,
        [origin, peer]: [String; 2],
        canary: Canary,
        x11_gesture: bool,
        core_only: bool,
    ) -> Self {
        let home = format!("{}/", origin.trim_end_matches('/'));
        let final_url = format!("{home}nav/final");
        let privacy = format!("{home}privacy");
        let pdf = format!("{home}document.pdf");
        let boundary = format!(
            r#"
            (() => {{
                const file = {};
                const mark = {{action: 'new_tab'}};
                window.fixtureFileBlocked = false;
                window.fixtureChromeResourceBlocked = false;
                fetch(file).then(() => {{}}, () => window.fixtureFileBlocked = true);
                fetch('muffin-chrome://validation/controls').then(() => {{}}, () => window.fixtureChromeResourceBlocked = true);
                window.postMessage(mark, location.origin);
                try {{ globalThis.ipc?.postMessage(JSON.stringify(mark)); }} catch (_) {{}}
                try {{ globalThis.chrome?.webview?.postMessage(JSON.stringify(mark)); }} catch (_) {{}}
                document.dispatchEvent(new KeyboardEvent('keydown', {{key:'t', ctrlKey:true, metaKey:true, bubbles:true}}));
            }})()
        "#,
            json!(canary.file_url)
        );
        let mut pending = vec![Step::new(
            "trusted_chrome_committed",
            Action::None,
            Check::ChromeReady,
        )];
        match mode {
            Mode::First => {
                pending.extend([
                    Step::new(
                        "trusted_chrome_command_created_lazy_tab",
                        script(
                            Target::Chrome,
                            "document.querySelector('[data-action=new_tab]')?.click()",
                        ),
                        Check::TabCount(2),
                    ),
                    Step::new(
                        "new_tab_has_no_content_webview",
                        Action::None,
                        Check::TabHasNoView,
                    ),
                    go("navigation_home", &home),
                    go("navigation_target", &final_url),
                    Step::new(
                        "back",
                        Action::Command(Command::Back),
                        Check::IdleUrl(home.clone()),
                    ),
                    Step::new(
                        "forward",
                        Action::Command(Command::Forward),
                        Check::IdleUrl(final_url.clone()),
                    ),
                    Step::new(
                        "reload",
                        Action::Command(Command::Reload),
                        Check::Js(
                            Target::Active,
                            "document.querySelector('#result')?.textContent",
                            json!("navigation target reached"),
                        ),
                    ),
                    go("cross_origin_redirect", &format!("{home}nav/redirect")),
                ]);
                // The redirect navigates to the second fixture origin.
                pending.last_mut().expect("redirect step").check =
                    Check::IdleUrl(format!("{}/nav/final", peer.trim_end_matches('/')));
                pending.extend([
                    Step::new("start_slow_navigation", navigate(&format!("{home}nav/slow")), Check::Loading),
                    Step::new("stop_navigation", Action::Command(Command::Stop), Check::Stopped),
                    go("security_fixture", &home),
                    Step::new("content_cannot_act_as_chrome_or_read_resources",
                        script(Target::Active, boundary), Check::Boundary(2, home.clone())),
                    Step::new("remote_file_url_blocked",
                        script(Target::Active, format!("location.href = {}", json!(canary.file_url))),
                        Check::Boundary(2, home.clone())),
                    Step::new("remote_external_protocol_blocked",
                        script(Target::Active, "location.href = 'muffin-validation-fixture:inert'"),
                        Check::Boundary(2, home.clone())),
                    Step::new("address_rejects_malformed_url",
                        navigate("https:///invalid.test"), Check::IdleUrl(home.clone())),
                    go("upload_fixture", &format!("{home}upload")),
                    Step::new("synthetic_upload_submission",
                        script(Target::Active, r#"
                            (() => {
                                const picker = document.querySelector('#upload-file');
                                if (!picker) return;
                                const data = new DataTransfer();
                                data.items.add(new File(['muffin-m0b'], 'synthetic.txt', {type: 'text/plain'}));
                                picker.files = data.files;
                                document.getElementById('upload-submit').click();
                            })()
                        "#),
                        Check::Js(Target::Active, "document.querySelector('#upload-result')?.textContent", json!("Upload received. Bytes: 10. Receipt: 1."))),
                    go("authenticated_download_login", &format!("{home}auth/login")),
                    Step::new("synthetic_login_redirect",
                        script(Target::Active, "document.getElementById('auth-login-button')?.click()"),
                        Check::IdleUrl(format!("{home}auth/ready"))),
                    Step::new("cookie_authenticated_download",
                        script(Target::Active, "document.getElementById('auth-download-link')?.click()"),
                        Check::Download(1)),
                    Step::new("duplicate_download_destination",
                        script(Target::Active, "document.getElementById('auth-download-link')?.click()"),
                        Check::Download(2)),
                    Step::new("pdf_native_route", navigate(&pdf), Check::Pdf(pdf, 2)).optional(),
                    go("media_fixture_navigation", &format!("{home}media")),
                    Step::new("pcm_audio_metadata", Action::None, Check::Js(
                        Target::Active,
                        "document.querySelector('#m0b-audio-wav')?.duration === 1",
                        json!(true),
                    )).optional(),
                ]);
                pending.push(go("large_download_fixture", &home));
                if cfg!(target_os = "macos") {
                    pending.push(Step::new(
                        "large_download_completion",
                        script(
                            Target::Active,
                            "document.getElementById('slow-download')?.click()",
                        ),
                        Check::DownloadAtLeast(3),
                    ));
                } else {
                    pending.extend([
                        Step::new(
                            "native_download_progress",
                            script(
                                Target::Active,
                                "document.getElementById('slow-download')?.click()",
                            ),
                            Check::NativeDownloadProgress,
                        ),
                        Step::new(
                            "native_download_cancellation",
                            Action::Command(Command::CancelDownloads),
                            Check::InterruptedDownload,
                        ),
                    ]);
                }
                pending.extend(popup_checks(&home, core_only));
                pending.extend([
                    go("normal_store_reached", &privacy),
                    storage(
                        "normal_store_written",
                        script(
                            Target::Active,
                            "document.getElementById('privacy-write')?.click()",
                        ),
                        true,
                    ),
                    go("normal_store_next_navigation", &home),
                    go("normal_store_return_navigation", &privacy),
                    storage("normal_store_retained", Action::None, true),
                    Step::new(
                        "new_private_view_is_lazy",
                        Action::Command(Command::NewPrivate),
                        Check::TabHasNoView,
                    ),
                    storage(
                        "private_store_isolated_from_normal",
                        navigate(&privacy),
                        false,
                    ),
                    storage(
                        "private_store_written",
                        script(
                            Target::Active,
                            "document.getElementById('privacy-write')?.click()",
                        ),
                        true,
                    ),
                    Step::new(
                        "second_private_view_is_lazy",
                        Action::Command(Command::NewPrivate),
                        Check::TabHasNoView,
                    ),
                    storage("private_sessions_are_separate", navigate(&privacy), false),
                    Step::new(
                        "close_private_tab",
                        Action::Command(Command::CloseTab),
                        Check::TabCount(3),
                    ),
                    storage("first_private_view_still_live", Action::None, true),
                    Step::new(
                        "close_first_private_tab",
                        Action::Command(Command::CloseTab),
                        Check::TabCount(2),
                    ),
                    storage(
                        "normal_store_unchanged_by_private_close",
                        Action::None,
                        true,
                    ),
                    Step::new(
                        "release_inactive_normal_view",
                        Action::Command(Command::NewTab),
                        Check::TabHasNoView,
                    ),
                    Step::new(
                        "inactive_webviews_released",
                        Action::Command(Command::ReleaseInactive),
                        Check::TabHasNoView,
                    ),
                ]);
            }
            Mode::Relaunch => pending.extend([
                storage(
                    "normal_store_persists_after_relaunch",
                    navigate(&privacy),
                    true,
                ),
                Step::new(
                    "relaunch_private_is_lazy",
                    Action::Command(Command::NewPrivate),
                    Check::TabHasNoView,
                ),
                storage(
                    "private_store_empty_after_relaunch",
                    navigate(&privacy),
                    false,
                ),
            ]),
        }
        Self {
            pending: pending.into(),
            current: None,
            sequence: 0,
            awaited: None,
            value: None,
            failed: false,
            x11_gesture,
            core_only,
            pending_desktop: 0,
            _canary: canary,
        }
    }

    pub fn probe_result(&mut self, id: u32, raw: &str) {
        if self.awaited.is_some_and(|(pending, _)| pending == id) {
            self.awaited = None;
            if raw.len() <= 256 {
                self.value = serde_json::from_str(raw).ok();
            }
        }
    }

    pub fn tick(&mut self, browser: &mut Browser) -> Option<bool> {
        let Some((step, started)) = self.current.take() else {
            let Some(step) = self.pending.pop_front() else {
                println!(
                    "smoke_complete required_cases_ok={} desktop_popup={}",
                    !self.failed,
                    if self.core_only && self.pending_desktop != 0 {
                        "inconclusive"
                    } else if self.failed {
                        "see_cases"
                    } else {
                        "no_omissions"
                    }
                );
                return Some(!self.failed);
            };
            self.value = None;
            self.awaited = None;
            match &step.action {
                Action::Command(command) => browser.command(command.clone()),
                Action::Script(target, script) => {
                    self.with_view(browser, target, |view| {
                        let _ = view.evaluate_script(script);
                    });
                }
                Action::PopupClick => {
                    if self.x11_gesture {
                        let scale = browser.window.scale_factor();
                        println!(
                            "fixture_input=x11_popup x={} y={}",
                            (100.0 * scale).round() as u32,
                            ((crate::platform::CHROME_HEIGHT as f64 + 119.0) * scale).round()
                                as u32
                        );
                    } else {
                        println!("fixture_input=programmatic_popup");
                        self.with_view(browser, &Target::Active, |view| {
                            let _ = view.evaluate_script(
                                "document.getElementById('open-cross-origin-popup')?.click()",
                            );
                        });
                    }
                }
                Action::None => {}
            }
            self.current = Some((step, Instant::now()));
            return None;
        };
        let elapsed = started.elapsed();
        let ready = match &step.check {
            Check::ChromeReady => browser.chrome_ready,
            Check::TabCount(expected) => browser.tabs.len() == *expected,
            Check::TabHasNoView => browser.active().view.is_none() &&
                (step.name != "inactive_webviews_released" || browser.tabs.iter().all(|tab| tab.view.is_none())),
            Check::IdleUrl(url) => at(browser, url),
            Check::Js(target, code, expected) => self.check_js(browser, target, code, expected),
            Check::Download(expected) => browser.views.downloads.borrow().finished() == *expected,
            Check::DownloadAtLeast(expected) => browser.views.downloads.borrow().finished() >= *expected,
            Check::Loading => elapsed > Duration::from_millis(450) && browser.active().loading,
            Check::Stopped => elapsed > Duration::from_millis(550) && !browser.active().loading
                && self.check_js(browser, &Target::Active, "!document.querySelector('#slow-complete')", &json!(true)),
            Check::NativeDownloadProgress => {
                let downloads = browser.views.downloads.borrow();
                downloads.cancelable() && downloads.native_bytes() > 0
            }
            Check::InterruptedDownload => browser.views.downloads.borrow().interrupted() > 0
                && !browser.views.downloads.borrow().cancelable(),
            Check::Boundary(count, url) => elapsed >= Duration::from_millis(700)
                && browser.tabs.len() == *count && at(browser, url)
                && browser.views.popups.borrow().visible.is_empty()
                && self.check_js(browser, &Target::Active, "window.fixtureFileBlocked === true && window.fixtureChromeResourceBlocked === true && !window.muffinControls", &json!(true)),
            Check::Pdf(url, downloads) => {
                let inline = at(browser, url);
                let downloaded = browser.views.downloads.borrow().finished() > *downloads;
                if inline || downloaded { println!("pdf_disposition inline_navigation={inline} download={downloaded}"); }
                inline || downloaded
            }
            Check::PopupClosed => elapsed >= Duration::from_millis(500)
                && browser.views.popups.borrow().next > 0 && browser.views.popups.borrow().visible.is_empty(),
        };
        let budget = Duration::from_secs(if step.required { 7 } else { 3 });
        if ready {
            println!("smoke_case={} result=pass", step.name);
        } else if elapsed >= budget {
            if step.required {
                self.failed = true;
            } else if step.name.starts_with("popup_") {
                self.pending_desktop += 1;
            }
            println!(
                "smoke_case={} result={}",
                step.name,
                if step.required {
                    "fail"
                } else {
                    "inconclusive"
                }
            );
        } else {
            self.current = Some((step, started));
        }
        None
    }

    fn with_view<T>(
        &self,
        browser: &Browser,
        target: &Target,
        action: impl FnOnce(&wry::WebView) -> T,
    ) -> Option<T> {
        match target {
            Target::Chrome => browser.chrome.as_ref().map(|view| action(&view.view)),
            Target::Active => browser
                .active()
                .view
                .as_ref()
                .map(|view| action(&view.view)),
            Target::Popup => browser
                .views
                .popups
                .borrow()
                .visible
                .first()
                .map(|popup| action(&popup.view.view)),
        }
    }

    fn check_js(
        &mut self,
        browser: &Browser,
        target: &Target,
        script: &str,
        expected: &Value,
    ) -> bool {
        if self.value.take().as_ref() == Some(expected) {
            return true;
        }
        if self
            .awaited
            .is_none_or(|(_, started)| started.elapsed() > Duration::from_secs(1))
        {
            self.sequence += 1;
            let id = self.sequence;
            let proxy = browser.views.proxy.clone();
            let eval = self.with_view(browser, target, |view| {
                view.evaluate_script_with_callback(script, move |raw| {
                    let _ = proxy.send_event(Signal::Probe(id, raw));
                })
            });
            if eval.is_some_and(|result| result.is_ok()) {
                self.awaited = Some((id, Instant::now()));
            }
        }
        false
    }
}

fn at(browser: &Browser, url: &str) -> bool {
    browser.active().view.as_ref().is_some_and(|view| {
        matches!(view.role, Role::Tab(_)) && view.view.url().ok().as_deref() == Some(url)
    }) && !browser.active().loading
        && browser.active().url != HOME
}
