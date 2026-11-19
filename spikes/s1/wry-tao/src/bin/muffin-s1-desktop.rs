//! Test-only native desktop and trust-boundary fixture.
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("This follow-up fixture requires AppKit");
    std::process::exit(2);
}

#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    macos::run()
}

#[cfg(target_os = "macos")]
mod macos {
    use block2::RcBlock;
    use objc2::{
        class, msg_send,
        runtime::{AnyObject, Bool},
    };
    use std::{
        borrow::Cow,
        ffi::CStr,
        path::PathBuf,
        ptr,
        time::{Duration, Instant},
    };
    use tao::{
        dpi::{LogicalPosition, LogicalSize},
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
        platform::macos::WindowExtMacOS,
        window::{Window, WindowBuilder},
    };
    use wry::{
        http, NewWindowResponse, PageLoadEvent, Rect, WebView, WebViewBuilder,
        WebViewBuilderExtDarwin, WebViewExtMacOS,
    };

    const CHROME_URL: &str = "muffin-chrome://fixture/index.html";
    const CHROME: &str = r#"<!doctype html><title>S1 trusted chrome</title>
<style>body{background:#153568;color:white;font:18px system-ui;margin:10px}</style>
<label>Chrome input <input autofocus aria-label="Chrome input" autocomplete="off"></label>
<button id="mark">Run chrome canary</button>"#;
    const OVERLAY: &str = r#"<!doctype html><title>S1 overlay</title>
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'">
<style>body{background:#b30036;color:white;font:20px system-ui;text-align:center}</style>
<p>S1 native overlay</p>"#;
    const DOM_PROBE: &str = "JSON.stringify({focus:document.hasFocus(),active:document.activeElement?.tagName, width:innerWidth,height:innerHeight, testInput:document.querySelector('input')?.value?.match(/^[abc]{0,12}$/)?.[0] ?? null})";
    const COMMAND: &str = "muffin-followup:mark";

    #[derive(Debug, Clone, Copy)]
    enum Pane {
        Chrome,
        Content,
    }

    enum Signal {
        Tick,
        Shortcut(Pane),
        FocusSample(Pane, String),
        Dom(&'static str, String),
        ChromeLoaded,
        ContentLoaded,
        Mark,
        Terminated,
    }

    fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect {
            position: LogicalPosition::new(x, y).into(),
            size: LogicalSize::new(w, h).into(),
        }
    }

    fn uri_policy(url: &str, allow: impl FnOnce(&str) -> bool, view: &str) -> bool {
        let accepted = allow(url);
        if !accepted {
            // Emit only the scheme. Paths and query strings can contain private information.
            println!(
                "navigation_denied view={view} scheme={}",
                url.split(':').next().unwrap_or("")
            );
        }
        accepted
    }

    fn observe(window: &Window, chrome: &WebView, content: &WebView) -> String {
        let native = window.ns_window().cast::<AnyObject>();
        let chrome_view = chrome.webview();
        let content_view = content.webview();
        // TAO and WRY own these live objects. This function runs on the AppKit main thread.
        unsafe {
            let key: Bool = msg_send![native, isKeyWindow];
            let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
            let active: Bool = msg_send![app, isActive];
            let responder: *mut AnyObject = msg_send![native, firstResponder];
            let is_view: Bool = msg_send![responder, isKindOfClass: class!(NSView)];
            let pane = if is_view.as_bool() {
                let chrome_ptr = ptr::from_ref(&*chrome_view).cast::<AnyObject>();
                let content_ptr = ptr::from_ref(&*content_view).cast::<AnyObject>();
                let in_chrome: Bool = msg_send![responder, isDescendantOf: chrome_ptr];
                let in_content: Bool = msg_send![responder, isDescendantOf: content_ptr];
                if in_chrome.as_bool() {
                    "chrome"
                } else if in_content.as_bool() {
                    "content"
                } else {
                    "other_view"
                }
            } else {
                "other"
            };
            let responder_class = responder
                .as_ref()
                .map(|r| r.class().name().to_string_lossy())
                .unwrap_or_default();
            format!(
                "active={} key={} responder={responder_class} pane={pane}",
                active.as_bool(),
                key.as_bool()
            )
        }
    }

    fn focus_native(window: &Window, view: &WebView, pane: Pane) {
        let native = window.ns_window().cast::<AnyObject>();
        let target = view.webview();
        let target_ptr = ptr::from_ref(&*target).cast::<AnyObject>();
        // AppKit keeps the actual responder. This return value only reports the request.
        let accepted: Bool = unsafe { msg_send![native, makeFirstResponder: target_ptr] };
        println!(
            "make_first_responder target={pane:?} returned={}",
            accepted.as_bool()
        );
    }

    fn install_key_monitor(proxy: EventLoopProxy<Signal>) -> *mut AnyObject {
        let monitor = RcBlock::new(move |event: *mut AnyObject| -> *mut AnyObject {
            // The local NSEvent monitor is called by AppKit on its main thread.
            unsafe {
                let modifiers: usize = msg_send![event, modifierFlags];
                let chars: *mut AnyObject = msg_send![event, charactersIgnoringModifiers];
                let bytes: *const i8 = msg_send![chars, UTF8String];
                if bytes.is_null() {
                    return event;
                }
                let key = CStr::from_ptr(bytes).to_bytes();
                const COMMAND_FLAG: usize = 1 << 20;
                const MODIFIER_MASK: usize = (1 << 17) | (1 << 18) | (1 << 19) | COMMAND_FLAG;
                let shortcut = if modifiers & MODIFIER_MASK == COMMAND_FLAG {
                    match key {
                        b"1" => Some(Pane::Chrome),
                        b"2" => Some(Pane::Content),
                        _ => None,
                    }
                } else {
                    None
                };
                if let Some(pane) = shortcut {
                    println!("appkit_shortcut={pane:?}");
                    let _ = proxy.send_event(Signal::Shortcut(pane));
                    return ptr::null_mut();
                }
                if modifiers & MODIFIER_MASK == 0 && matches!(key, b"a" | b"b" | b"c") {
                    println!("appkit_test_key={}", char::from(key[0]));
                }
                event
            }
        });
        // The mask is NSEventMaskKeyDown (1 << NSEventTypeKeyDown).
        unsafe {
            msg_send![class!(NSEvent), addLocalMonitorForEventsMatchingMask: 1usize << 10, handler: &*monitor]
        }
    }

    fn dom(view: &WebView, label: &'static str, proxy: EventLoopProxy<Signal>) {
        if let Err(error) = view.evaluate_script_with_callback(DOM_PROBE, move |value| {
            let _ = proxy.send_event(Signal::Dom(label, value));
        }) {
            println!("dom_query_error pane={label} error={error}");
        }
    }

    fn session_path() -> std::io::Result<PathBuf> {
        Ok(std::env::current_exe()?
            .parent()
            .expect("app executable directory")
            .join("../Resources/session.json"))
    }

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let session: serde_json::Value = serde_json::from_slice(&std::fs::read(session_path()?)?)?;
        let content_url = session["content_url"]
            .as_str()
            .ok_or("missing fixture URL")?
            .to_owned();
        let uri: http::Uri = content_url.parse()?;
        if uri.scheme_str() != Some("http")
            || uri.host() != Some("127.0.0.1")
            || uri.port().is_none()
        {
            return Err("fixture content must use an explicit loopback HTTP origin".into());
        }
        let content_origin = format!(
            "http://{}/",
            uri.authority().ok_or("missing fixture authority")?
        );
        println!(
            "source_fixture=interactive-follow-up webkit={} pid={}",
            wry::webview_version()?,
            std::process::id()
        );
        let events = EventLoopBuilder::<Signal>::with_user_event().build();
        let proxy = events.create_proxy();
        let window = WindowBuilder::new()
            .with_title("Muffin S1 Desktop Fixture")
            .with_inner_size(LogicalSize::new(880, 585))
            .with_min_inner_size(LogicalSize::new(700, 470))
            .build(&events)?;
        let logger = proxy.clone();
        let loaded = proxy.clone();
        let chrome = WebViewBuilder::new()
            .with_url(CHROME_URL)
            .with_bounds(rect(0, 0, 880, 72))
            .with_incognito(true)
            .with_navigation_handler(|url| uri_policy(&url, |u| u == CHROME_URL, "chrome"))
            .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
            .with_custom_protocol("muffin-chrome".into(), |_, request| {
                let allowed = request.method() == http::Method::GET
                    && request.uri().to_string() == CHROME_URL;
                println!("chrome_protocol_response allowed={allowed}");
                http::Response::builder()
                    .status(if allowed { 200 } else { 404 })
                    .header("content-type", "text/html; charset=utf-8")
                    .header(
                        "content-security-policy",
                        "default-src 'none'; style-src 'unsafe-inline'; frame-ancestors 'none'",
                    )
                    .body(Cow::Borrowed(if allowed { CHROME.as_bytes() } else { b"" }))
                    .expect("fixture response")
            })
            .with_ipc_handler(move |request| {
                let allowed = request.uri().to_string() == CHROME_URL && request.body() == COMMAND;
                println!("chrome_ipc_received allowed={allowed}");
                if allowed {
                    let _ = logger.send_event(Signal::Mark);
                }
            })
            .with_on_page_load_handler(move |event, url| {
                if matches!(event, PageLoadEvent::Finished) && url == CHROME_URL {
                    let _ = loaded.send_event(Signal::ChromeLoaded);
                }
            })
            .build_as_child(&window)?;
        let loaded = proxy.clone();
        let terminated = proxy.clone();
        let content = WebViewBuilder::new()
            .with_url(&content_url)
            .with_bounds(rect(0, 72, 880, 513))
            .with_incognito(true)
            .with_navigation_handler(move |url| {
                uri_policy(&url, |u| u.starts_with(&content_origin), "content")
            })
            .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
            .with_on_page_load_handler(move |event, _| {
                if matches!(event, PageLoadEvent::Finished) {
                    let _ = loaded.send_event(Signal::ContentLoaded);
                }
            })
            .with_on_web_content_process_terminate_handler(move || {
                let _ = terminated.send_event(Signal::Terminated);
            })
            .build_as_child(&window)?;
        let _overlay = WebViewBuilder::new()
            .with_html(OVERLAY)
            .with_focused(false)
            .with_bounds(rect(210, 44, 260, 126))
            .with_incognito(true)
            .build_as_child(&window)?;
        let monitor = install_key_monitor(proxy.clone());
        println!("appkit_key_monitor={}", !monitor.is_null());
        let timer = proxy.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(650));
            if timer.send_event(Signal::Tick).is_err() {
                break;
            }
        });
        let mut last_native = String::new();
        let mut last_dom = std::collections::BTreeMap::new();
        let began = Instant::now();
        let mut commands = 0;
        let mut ready = true;
        events.run(move |event, _, control| {
            // Keeping the child alive preserves its native compositor surface.
            let _ = &_overlay;
            *control = ControlFlow::Wait;
            match event {
                Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                    println!("fixture_end commands={commands}");
                    *control = ControlFlow::Exit;
                }
                Event::WindowEvent { event: WindowEvent::Resized(size), .. } => {
                    let logical = size.to_logical::<i32>(window.scale_factor());
                    let c = chrome.set_bounds(rect(0,0,logical.width,72));
                    let p = content.set_bounds(rect(0,72,logical.width,(logical.height-72).max(1)));
                    println!("parent_resize={}x{} applied={}", logical.width, logical.height, c.is_ok() && p.is_ok());
                }
                Event::UserEvent(Signal::ChromeLoaded) => {
                    let script = "document.getElementById('mark').addEventListener('click', () => window.ipc.postMessage('muffin-followup:mark'))";
                    println!("chrome_loaded controllable={}", chrome.evaluate_script(script).is_ok());
                }
                Event::UserEvent(Signal::ContentLoaded) => println!("content_loaded=true"),
                Event::UserEvent(Signal::Mark) => {
                    commands += 1;
                    println!("native_command_count={commands}");
                }
                Event::UserEvent(Signal::Shortcut(pane)) => {
                    let view = match pane { Pane::Chrome => &chrome, Pane::Content => &content };
                    focus_native(&window, view, pane);
                    println!("appkit_after_focus {}", observe(&window, &chrome, &content));
                    let notify = proxy.clone();
                    if let Err(error) = view.evaluate_script_with_callback(DOM_PROBE, move |value| {
                        let _ = notify.send_event(Signal::FocusSample(pane, value));
                    }) {
                        println!("focus_sample_error target={pane:?} error={error}");
                    }
                }
                Event::UserEvent(Signal::FocusSample(pane, value)) => {
                    let view = match pane { Pane::Chrome => &chrome, Pane::Content => &content };
                    println!("dom_before_explicit_focus target={pane:?} {value}");
                    let applied = view.evaluate_script("document.querySelector('input')?.focus()").is_ok();
                    println!("fixture_dom_focus_requested target={pane:?} dispatched={applied}");
                }
                Event::UserEvent(Signal::Terminated) => {
                    println!("webkit_content_process_terminated=true");
                    println!("recovery_dispatch={}", content.load_url(&content_url).is_ok());
                }
                Event::UserEvent(Signal::Dom(label, value)) => {
                    if last_dom.get(label) != Some(&value) {
                        println!("dom pane={label} {value}");
                        last_dom.insert(label, value);
                    }
                }
                Event::UserEvent(Signal::Tick) => {
                    if began.elapsed() > Duration::from_secs(600) {
                        println!("fixture_watchdog commands={commands}");
                        *control = ControlFlow::Exit;
                        return;
                    }
                    let current = observe(&window,&chrome,&content);
                    if current != last_native {
                        println!("appkit {current}");
                        last_native = current;
                    }
                    if ready {
                        let native = window.ns_window().cast::<AnyObject>();
                        let number: isize = unsafe { msg_send![native, windowNumber] };
                        println!("window_number={number}");
                        ready = false;
                    }
                    dom(&chrome, "chrome", proxy.clone());
                    dom(&content, "content", proxy.clone());
                }
                _ => {}
            }
        })
    }
}
