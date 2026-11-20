#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Muffin M1 currently targets macOS Apple Silicon.");
    std::process::exit(1);
}

#[cfg(target_os = "macos")]
mod browser {
    use objc2::MainThreadMarker;
    use objc2_foundation::NSString;
    use objc2_web_kit::{WKWebViewConfiguration, WKWebsiteDataStore};
    use serde::{Deserialize, Serialize};
    #[cfg(debug_assertions)]
    use std::path::Path;
    #[cfg(debug_assertions)]
    use std::{
        fs,
        io::{BufRead, BufReader},
    };
    use std::{
        path::PathBuf,
        time::{Duration, Instant},
    };
    use tao::{
        dpi::{LogicalPosition, LogicalSize},
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy},
        window::{Window, WindowBuilder},
    };
    use tempfile::TempDir;
    use wry::{
        PageLoadEvent, PermissionResponse, WebContext, WebView, WebViewBuilder,
        WebViewBuilderExtDarwin, WebViewBuilderExtMacos, WebViewExtMacOS,
    };

    const CHROME: &str = include_str!("../../../ui/dist/index.html");
    const CHROME_HEIGHT: u32 = 56;
    const MAX_MESSAGE_BYTES: usize = 64 * 1024;

    #[derive(Clone, Debug, Deserialize)]
    #[serde(tag = "action", deny_unknown_fields)]
    enum Action {
        #[serde(rename = "navigate")]
        Navigate { version: u8, value: String },
        #[serde(rename = "back")]
        Back { version: u8 },
        #[serde(rename = "forward")]
        Forward { version: u8 },
        #[serde(rename = "reload")]
        Reload { version: u8 },
        #[serde(rename = "stop")]
        Stop { version: u8 },
        #[serde(rename = "focus_page")]
        FocusPage { version: u8 },
        #[serde(rename = "focus_address")]
        FocusAddress { version: u8 },
        #[serde(rename = "open_home")]
        OpenHome { version: u8 },
    }

    #[cfg(debug_assertions)]
    #[derive(Deserialize)]
    #[serde(tag = "action", deny_unknown_fields)]
    enum ChromeTestAction {
        #[serde(rename = "chrome_navigate")]
        Navigate { version: u8, value: String },
        #[serde(rename = "capture_chrome")]
        Capture { version: u8 },
    }

    #[cfg(debug_assertions)]
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum TestCommand {
        Chrome(ChromeTestAction),
        Native(Action),
    }

    #[cfg(debug_assertions)]
    fn decode_test_command(raw: &str) -> Option<TestCommand> {
        let command: TestCommand = serde_json::from_str(raw).ok()?;
        match &command {
            TestCommand::Chrome(
                ChromeTestAction::Navigate { version: 1, .. }
                | ChromeTestAction::Capture { version: 1 },
            ) => Some(command),
            TestCommand::Native(action) if action.version() == 1 => Some(command),
            _ => None,
        }
    }

    impl Action {
        fn version(&self) -> u8 {
            match self {
                Self::Navigate { version, .. }
                | Self::Back { version }
                | Self::Forward { version }
                | Self::Reload { version }
                | Self::Stop { version }
                | Self::FocusPage { version }
                | Self::FocusAddress { version }
                | Self::OpenHome { version } => *version,
            }
        }
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ChromeReply {
        version: u8,
        action: Option<Action>,
    }

    fn decode_reply(raw: &str) -> Option<ChromeReply> {
        if raw.len() > MAX_MESSAGE_BYTES {
            return None;
        }
        serde_json::from_str(raw)
            .ok()
            .filter(|reply: &ChromeReply| {
                reply.version == 1
                    && reply
                        .action
                        .as_ref()
                        .is_none_or(|action| action.version() == 1)
            })
    }

    #[derive(Clone, Copy, Serialize)]
    #[serde(rename_all = "snake_case")]
    enum Status {
        Home,
        Loading,
        Ready,
        Blocked,
        Failed,
        Terminated,
    }

    #[derive(Serialize)]
    struct Snapshot<'a> {
        version: u8,
        url: &'a str,
        title: &'a str,
        loading: bool,
        can_back: bool,
        can_forward: bool,
        status: Status,
        secure: bool,
    }

    enum Message {
        ChromeResult(String),
        NativeAction(Action),
        Loaded(PageLoadEvent, String),
        Title(String),
        Resize,
        ProcessTerminated,
    }

    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    struct TestControl {
        directory: PathBuf,
        cursor: u64,
    }

    impl TestControl {
        #[cfg(debug_assertions)]
        fn poll(&mut self) -> Option<TestCommand> {
            let path = self.directory.join("commands.jsonl");
            let file = fs::File::open(path).ok()?;
            use std::io::Seek;
            let mut reader = BufReader::new(file);
            if reader.seek(std::io::SeekFrom::Start(self.cursor)).is_err() {
                self.cursor = 0;
                reader =
                    BufReader::new(fs::File::open(self.directory.join("commands.jsonl")).ok()?);
            }
            use std::io::Read;
            let mut line = String::new();
            let mut limited = reader.take((MAX_MESSAGE_BYTES + 1) as u64);
            if limited.read_line(&mut line).ok()? == 0 {
                return None;
            }
            reader = limited.into_inner();
            if line.len() > MAX_MESSAGE_BYTES {
                self.cursor = reader.seek(std::io::SeekFrom::End(0)).ok()?;
                return None;
            }
            if !line.ends_with('\n') {
                return None;
            }
            self.cursor = reader.stream_position().ok()?;
            decode_test_command(&line)
        }

        #[cfg(debug_assertions)]
        fn write_state(
            &self,
            state: &Snapshot<'_>,
            chrome_ready: bool,
            window_number: isize,
            native_loading: bool,
        ) {
            #[derive(Serialize)]
            struct Observation<'a> {
                #[serde(flatten)]
                snapshot: &'a Snapshot<'a>,
                chrome_ready: bool,
                window_number: isize,
                native_loading: bool,
            }
            let Ok(json) = serde_json::to_vec(&Observation {
                snapshot: state,
                chrome_ready,
                window_number,
                native_loading,
            }) else {
                return;
            };
            if json.len() > MAX_MESSAGE_BYTES {
                return;
            }
            let temp = self.directory.join("state.json.tmp");
            let target = self.directory.join("state.json");
            if fs::write(&temp, json).is_ok() {
                let _ = fs::rename(temp, target);
            }
        }
    }

    struct App {
        window: Window,
        chrome: WebView,
        content: WebView,
        _context: WebContext,
        _data: TempDir,
        proxy: EventLoopProxy<Message>,
        #[cfg_attr(not(debug_assertions), allow(dead_code))]
        test_control: Option<TestControl>,
        address: String,
        title: String,
        load_started: Option<Instant>,
        status: Status,
        last_poll: Instant,
        chrome_ready: bool,
        chrome_poll_started: Option<Instant>,
        recovery_attempted: bool,
        native_loading: bool,
        finished_navigation: bool,
    }

    impl App {
        fn new(
            event_loop: &EventLoop<Message>,
            test_control: Option<TestControl>,
        ) -> Result<Self, String> {
            let data = tempfile::Builder::new()
                .prefix("muffin-m1-")
                .tempdir()
                .map_err(|e| e.to_string())?;
            let mut context = WebContext::new(Some(data.path().join("web-data")));
            let window = WindowBuilder::new()
                .with_title("Muffin")
                .with_inner_size(LogicalSize::new(1100.0, 760.0))
                .with_min_inner_size(LogicalSize::new(640.0, 420.0))
                .build(event_loop)
                .map_err(|e| e.to_string())?;
            let proxy = event_loop.create_proxy();
            let chrome = WebViewBuilder::new()
                .with_devtools(false)
                .with_incognito(true)
                .with_navigation_handler(|url| url == "about:blank")
                .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
                .with_download_started_handler(|_, _| false)
                .with_bounds(bounds(&window, 0, CHROME_HEIGHT))
                .build_as_child(&window)
                .map_err(|e| e.to_string())?;
            chrome.load_html(CHROME).map_err(|e| e.to_string())?;
            let loaded_proxy = proxy.clone();
            let title_proxy = proxy.clone();
            let terminated_proxy = proxy.clone();
            let main_thread =
                MainThreadMarker::new().ok_or("WebKit must start on the main thread")?;
            let web_configuration = unsafe { WKWebViewConfiguration::new(main_thread) };
            unsafe {
                web_configuration
                    .setWebsiteDataStore(&WKWebsiteDataStore::nonPersistentDataStore(main_thread));
                web_configuration.setApplicationNameForUserAgent(Some(&NSString::from_str(
                    &format!("Muffin/{}", env!("CARGO_PKG_VERSION")),
                )));
            }
            let content = WebViewBuilder::new_with_web_context(&mut context)
                .with_webview_configuration(web_configuration)
                .with_devtools(false)
                .with_incognito(true)
                .with_focused(false)
                .with_navigation_handler(allowed_navigation)
                .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
                .with_download_started_handler(|_, _| false)
                .with_permission_handler(|_| PermissionResponse::Deny)
                .with_on_page_load_handler(move |event, url| {
                    let _ = loaded_proxy.send_event(Message::Loaded(event, url));
                })
                .with_document_title_changed_handler(move |title| {
                    let _ = title_proxy.send_event(Message::Title(title));
                })
                .with_on_web_content_process_terminate_handler(move || {
                    let _ = terminated_proxy.send_event(Message::ProcessTerminated);
                })
                .with_bounds(bounds(
                    &window,
                    CHROME_HEIGHT,
                    window
                        .inner_size()
                        .to_logical::<u32>(window.scale_factor())
                        .height
                        .saturating_sub(CHROME_HEIGHT),
                ))
                .build_as_child(&window)
                .map_err(|e| e.to_string())?;
            let mut app = Self {
                window,
                chrome,
                content,
                _context: context,
                _data: data,
                proxy,
                test_control,
                address: "about:blank".into(),
                title: "New Tab".into(),
                load_started: None,
                status: Status::Home,
                last_poll: Instant::now(),
                chrome_ready: false,
                chrome_poll_started: None,
                recovery_attempted: false,
                native_loading: false,
                finished_navigation: false,
            };
            app.render();
            Ok(app)
        }

        fn layout(&self) {
            let size = self.window.inner_size();
            let logical = size.to_logical::<u32>(self.window.scale_factor());
            let _ = self
                .chrome
                .set_bounds(bounds(&self.window, 0, CHROME_HEIGHT));
            let _ = self.content.set_bounds(bounds(
                &self.window,
                CHROME_HEIGHT,
                logical.height.saturating_sub(CHROME_HEIGHT),
            ));
        }

        fn apply(&mut self, action: Action) {
            if action.version() != 1 {
                return;
            }
            match action {
                Action::Navigate { value, .. } => match address_destination(&value) {
                    Some(url) => {
                        self.recovery_attempted = false;
                        self.navigate(&url);
                    }
                    None => {
                        self.load_started = None;
                        self.status = Status::Blocked;
                    }
                },
                Action::Back { .. } if self.content.can_go_back().unwrap_or(false) => {
                    self.recovery_attempted = false;
                    self.start_load();
                    let _ = self.content.go_back();
                }
                Action::Forward { .. } if self.content.can_go_forward().unwrap_or(false) => {
                    self.recovery_attempted = false;
                    self.start_load();
                    let _ = self.content.go_forward();
                }
                Action::Reload { .. } => {
                    self.recovery_attempted = false;
                    self.start_load();
                    let _ = self.content.reload();
                }
                Action::Stop { .. } => {
                    unsafe { self.content.webview().stopLoading() };
                    self.load_started = None;
                    self.status = if self.address == "about:blank" {
                        Status::Home
                    } else {
                        Status::Ready
                    };
                }
                Action::FocusPage { .. } => {
                    let _ = self.content.focus();
                }
                Action::FocusAddress { .. } => {
                    let _ = self.chrome.focus();
                    let _ = self
                        .chrome
                        .evaluate_script("window.muffinChrome?.focusAddress()");
                }
                Action::OpenHome { .. } => {
                    self.recovery_attempted = false;
                    self.navigate("about:blank");
                }
                _ => {}
            }
            self.render();
        }

        fn start_load(&mut self) {
            self.status = Status::Loading;
            self.load_started = Some(Instant::now());
            self.finished_navigation = false;
        }

        fn fail_load(&mut self) {
            self.status = Status::Failed;
            self.load_started = None;
        }

        fn navigate(&mut self, url: &str) {
            self.start_load();
            if self.content.load_url(url).is_err() {
                self.fail_load();
            }
        }

        fn render(&mut self) {
            self.native_loading = unsafe { self.content.webview().isLoading() };
            let snapshot = Snapshot {
                version: 1,
                url: &self.address,
                title: &self.title,
                loading: self.load_started.is_some(),
                can_back: self.content.can_go_back().unwrap_or(false),
                can_forward: self.content.can_go_forward().unwrap_or(false),
                status: self.status,
                secure: self.status_matches_verified_https(),
            };
            #[cfg(debug_assertions)]
            if let Some(control) = &self.test_control {
                use tao::platform::macos::WindowExtMacOS;
                let window = self.window.ns_window().cast::<objc2::runtime::AnyObject>();
                let window_number: isize = unsafe { objc2::msg_send![window, windowNumber] };
                control.write_state(
                    &snapshot,
                    self.chrome_ready,
                    window_number,
                    self.native_loading,
                );
            }
            if let Ok(json) = serde_json::to_string(&snapshot) {
                let _ = self
                    .chrome
                    .evaluate_script(&format!("window.muffinChrome?.render({json})"));
            }
        }

        fn poll_chrome(&mut self) {
            if self.last_poll.elapsed() < Duration::from_millis(100) {
                return;
            }
            self.last_poll = Instant::now();
            let native_loading = unsafe { self.content.webview().isLoading() };
            if self
                .load_started
                .is_some_and(|since| since.elapsed() > Duration::from_millis(750))
                && !native_loading
            {
                self.fail_load();
                self.render();
            } else if native_loading != self.native_loading {
                self.render();
            }
            if self.chrome_poll_started.is_none() {
                let proxy = self.proxy.clone();
                self.chrome_poll_started = Some(Instant::now());
                if self.chrome.evaluate_script_with_callback(
                    "window.muffinChrome && document.getElementById('address') ? ({version: 1, action: window.muffinChrome.take()}) : null",
                    move |result| {
                        let _ = proxy.send_event(Message::ChromeResult(result));
                    },
                ).is_err() {
                    self.chrome_poll_started = None;
                }
            } else if self
                .chrome_poll_started
                .is_some_and(|started| started.elapsed() > Duration::from_secs(2))
            {
                self.chrome_poll_started = None;
            }
            #[cfg(debug_assertions)]
            if let Some(command) = self
                .test_control
                .as_mut()
                .filter(|_| self.chrome_ready)
                .and_then(TestControl::poll)
            {
                match command {
                    TestCommand::Native(action) => self.apply(action),
                    TestCommand::Chrome(ChromeTestAction::Navigate { value, .. }) => {
                        if let Ok(value) = serde_json::to_string(&value) {
                            let _ = self.chrome.evaluate_script(&format!(
                                "const field = document.querySelector('#address'); if (field) {{ field.value = {value}; field.dispatchEvent(new Event('input', {{ bubbles: true }})); document.querySelector('form[role=search]')?.requestSubmit(); }}"
                            ));
                        }
                    }
                    TestCommand::Chrome(ChromeTestAction::Capture { .. }) => {
                        if let Some(control) = &self.test_control {
                            capture_chrome_view(&self.chrome, &control.directory);
                        }
                    }
                }
            }
        }

        fn status_matches_verified_https(&self) -> bool {
            if !self.finished_navigation
                || !matches!(self.status, Status::Ready)
                || !self.address.starts_with("https://")
            {
                return false;
            }
            unsafe { self.content.webview().hasOnlySecureContent() }
        }

        fn handle(&mut self, message: Message) {
            match message {
                Message::ChromeResult(raw) => {
                    self.chrome_poll_started = None;
                    if let Some(reply) = decode_reply(&raw) {
                        if !self.chrome_ready {
                            self.chrome_ready = true;
                            self.render();
                        }
                        if let Some(action) = reply.action {
                            self.apply(action);
                        }
                    }
                }
                Message::Loaded(PageLoadEvent::Started, url) => {
                    self.start_load();
                    if allowed_navigation(url.clone()) {
                        self.address = url;
                    }
                    self.render();
                }
                Message::Loaded(PageLoadEvent::Finished, url) => {
                    self.load_started = None;
                    self.finished_navigation = true;
                    self.address = url;
                    self.status = if self.address == "about:blank" {
                        Status::Home
                    } else {
                        Status::Ready
                    };
                    self.render();
                }
                Message::Title(title) => {
                    self.title = title.chars().take(512).collect();
                    self.window.set_title(&format!("{} — Muffin", self.title));
                    self.render();
                }
                Message::NativeAction(action) => self.apply(action),
                Message::ProcessTerminated => {
                    self.status = Status::Terminated;
                    self.load_started = None;
                    if should_recover(&mut self.recovery_attempted, &self.address) {
                        self.navigate(&self.address.clone());
                    }
                    self.render();
                }
                Message::Resize => self.layout(),
            }
        }
    }

    #[cfg(debug_assertions)]
    fn capture_chrome_view(chrome: &WebView, directory: &Path) {
        let image_path = directory.join("chrome.tiff");
        let Some(image_path) = image_path.to_str() else {
            return;
        };
        let path = NSString::from_str(image_path);
        let callback = block2::RcBlock::new(
            move |image: *mut objc2_app_kit::NSImage, _: *mut objc2_foundation::NSError| {
                if let Some(image) = unsafe { image.as_ref() } {
                    if let Some(tiff) = image.TIFFRepresentation() {
                        tiff.writeToFile_atomically(&path, true);
                    }
                }
            },
        );
        unsafe {
            chrome
                .webview()
                .takeSnapshotWithConfiguration_completionHandler(None, &callback);
        }
    }

    fn should_recover(attempted: &mut bool, address: &str) -> bool {
        if *attempted || address == "about:blank" || !allowed_navigation(address.to_owned()) {
            return false;
        }
        *attempted = true;
        true
    }

    fn bounds(window: &Window, y: u32, height: u32) -> wry::Rect {
        let logical = window.inner_size().to_logical::<u32>(window.scale_factor());
        wry::Rect {
            position: LogicalPosition::new(0, y).into(),
            size: LogicalSize::new(logical.width.max(1), height.max(1)).into(),
        }
    }

    fn allowed_navigation(url: String) -> bool {
        if url == "about:blank" {
            return true;
        }
        let Ok(parsed) = url::Url::parse(&url) else {
            return false;
        };
        matches!(parsed.scheme(), "http" | "https")
            && parsed.has_host()
            && parsed.username().is_empty()
            && parsed.password().is_none()
    }

    fn address_destination(input: &str) -> Option<String> {
        let input = input.trim();
        if input.is_empty()
            || input.len() > 4096
            || input.chars().any(char::is_control)
            || input.contains('\\')
        {
            return None;
        }
        if input == "about:blank" {
            return Some(input.to_owned());
        }
        let host = input.split('/').next()?;
        let is_local = host == "localhost"
            || host.starts_with("localhost:")
            || host == "127.0.0.1"
            || host.starts_with("127.0.0.1:");
        let has_port = host.split_once(':').is_some_and(|(_, port)| {
            !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit())
        });
        let parsed = if input.contains("://") {
            url::Url::parse(input).ok()?
        } else if has_port {
            url::Url::parse(&format!(
                "{}://{input}",
                if is_local { "http" } else { "https" }
            ))
            .ok()?
        } else if input.split('/').next()?.contains('.') || input.starts_with("localhost") {
            url::Url::parse(&format!("https://{input}")).ok()?
        } else {
            let mut search = url::Url::parse("https://duckduckgo.com/").ok()?;
            search.query_pairs_mut().append_pair("q", input);
            search
        };
        (matches!(parsed.scheme(), "http" | "https")
            && parsed.has_host()
            && parsed.username().is_empty()
            && parsed.password().is_none())
        .then(|| parsed.to_string())
    }

    #[cfg(debug_assertions)]
    fn test_control() -> Option<TestControl> {
        let mut args = std::env::args_os().skip(1);
        while let Some(arg) = args.next() {
            if arg == "--test-control" {
                let directory = PathBuf::from(args.next()?);
                if !directory.is_dir() {
                    return None;
                }
                return Some(TestControl {
                    directory,
                    cursor: 0,
                });
            }
        }
        None
    }

    #[cfg(not(debug_assertions))]
    fn test_control() -> Option<TestControl> {
        None
    }

    fn install_keys(
        proxy: EventLoopProxy<Message>,
    ) -> objc2::rc::Retained<objc2::runtime::AnyObject> {
        use block2::RcBlock;
        use objc2::{class, msg_send, rc::Retained, runtime::AnyObject};
        use std::{ffi::CStr, ptr};
        let monitor = RcBlock::new(move |event: *mut AnyObject| -> *mut AnyObject {
            unsafe {
                let flags: usize = msg_send![event, modifierFlags];
                const CONTROL: usize = 1 << 18;
                const OPTION: usize = 1 << 19;
                const COMMAND: usize = 1 << 20;
                if flags & (CONTROL | OPTION | COMMAND) != COMMAND || flags & (1 << 17) != 0 {
                    return event;
                }
                let chars: *mut AnyObject = msg_send![event, charactersIgnoringModifiers];
                let bytes: *const i8 = msg_send![chars, UTF8String];
                if bytes.is_null() {
                    return event;
                }
                let action = match CStr::from_ptr(bytes).to_bytes() {
                    [letter] if letter.eq_ignore_ascii_case(&b'l') => {
                        Action::FocusAddress { version: 1 }
                    }
                    [letter] if letter.eq_ignore_ascii_case(&b'r') => Action::Reload { version: 1 },
                    b"[" => Action::Back { version: 1 },
                    b"]" => Action::Forward { version: 1 },
                    b"." => Action::Stop { version: 1 },
                    _ => return event,
                };
                let _ = proxy.send_event(Message::NativeAction(action));
                ptr::null_mut()
            }
        });
        let monitor: *mut AnyObject = unsafe {
            msg_send![class!(NSEvent), addLocalMonitorForEventsMatchingMask: 1usize << 10, handler: &*monitor]
        };
        unsafe { Retained::retain(monitor) }.expect("AppKit local keyboard monitor")
    }

    pub fn run() -> Result<(), String> {
        validate_arguments()?;
        let mut builder = EventLoopBuilder::<Message>::with_user_event();
        let event_loop = builder.build();
        let mut app = Some(App::new(&event_loop, test_control())?);
        let _key_monitor = install_keys(event_loop.create_proxy());
        let mut next_tick = Instant::now();
        event_loop.run(move |event, _, control_flow| {
            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    app = None;
                    *control_flow = ControlFlow::Exit;
                    return;
                }
                Event::WindowEvent {
                    event: WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. },
                    ..
                } => {
                    if let Some(app) = app.as_mut() {
                        app.handle(Message::Resize);
                    }
                }
                Event::UserEvent(message) => {
                    if let Some(app) = app.as_mut() {
                        app.handle(message);
                    }
                }
                Event::LoopDestroyed => return,
                _ => {}
            }
            if Instant::now() >= next_tick {
                next_tick = Instant::now() + Duration::from_millis(100);
                if let Some(app) = app.as_mut() {
                    app.poll_chrome();
                }
            }
            *control_flow = ControlFlow::WaitUntil(next_tick);
        });
    }

    fn validate_arguments() -> Result<(), String> {
        let args: Vec<_> = std::env::args_os().skip(1).collect();
        if args.is_empty() {
            return Ok(());
        }
        #[cfg(debug_assertions)]
        if args.len() == 2 && args[0] == "--test-control" && Path::new(&args[1]).is_dir() {
            return Ok(());
        }
        Err("unknown or malformed argument".into())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn address_policy_keeps_web_schemes_and_local_fixture_http() {
            assert_eq!(
                address_destination("example.com"),
                Some("https://example.com/".into())
            );
            assert_eq!(
                address_destination("127.0.0.1:8765/page"),
                Some("http://127.0.0.1:8765/page".into())
            );
            assert_eq!(
                address_destination("about:blank"),
                Some("about:blank".into())
            );
        }

        #[test]
        fn recovery_budget_stops_repeated_termination_until_an_explicit_navigation() {
            let mut attempted = false;
            assert!(!should_recover(&mut attempted, "about:blank"));
            assert!(should_recover(&mut attempted, "https://example.test/"));
            assert!(!should_recover(&mut attempted, "https://example.test/"));
            attempted = false;
            assert!(should_recover(&mut attempted, "https://example.test/"));
        }

        #[test]
        fn address_policy_rejects_file_and_credentials() {
            assert_eq!(address_destination("file:///etc/passwd"), None);
            assert_eq!(address_destination("https://user:secret@example.com"), None);
            assert_eq!(
                address_destination("javascript:alert(1)"),
                Some("https://duckduckgo.com/?q=javascript%3Aalert%281%29".into())
            );
        }

        #[test]
        fn ipc_rejects_unknown_fields_and_versions() {
            assert!(
                decode_reply(r#"{"version":1,"action":{"action":"reload","version":2}}"#).is_none()
            );
            assert!(decode_reply(
                r#"{"version":1,"action":{"action":"reload","version":1,"extra":true}}"#
            )
            .is_none());
            assert!(decode_reply(
                r#"{"version":1,"action":{"action":"native_command","version":1}}"#
            )
            .is_none());
        }

        #[test]
        fn debug_test_commands_keep_native_and_chrome_routes_distinct() {
            assert!(matches!(
                decode_test_command(
                    r#"{"version":1,"action":"navigate","value":"127.0.0.1:8080"}"#
                ),
                Some(TestCommand::Native(Action::Navigate { .. }))
            ));
            assert!(matches!(
                decode_test_command(
                    r#"{"version":1,"action":"chrome_navigate","value":"127.0.0.1:8080"}"#
                ),
                Some(TestCommand::Chrome(ChromeTestAction::Navigate { .. }))
            ));
            assert!(
                decode_test_command(r#"{"version":2,"action":"chrome_navigate","value":"x"}"#)
                    .is_none()
            );
        }
    }
}

#[cfg(target_os = "macos")]
fn main() {
    if let Err(error) = browser::run() {
        eprintln!("Muffin could not start: {error}");
        std::process::exit(1);
    }
}
