#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("This Muffin development build targets macOS Apple Silicon.");
    std::process::exit(1);
}

#[cfg(target_os = "macos")]
mod download_delegate;
#[cfg(target_os = "macos")]
mod downloads;
#[cfg(target_os = "macos")]
mod instance;
#[cfg(target_os = "macos")]
mod pdf;
#[cfg(target_os = "macos")]
mod profile;
#[cfg(target_os = "macos")]
#[path = "session.rs"]
mod session;
#[cfg(target_os = "macos")]
#[path = "tabs.rs"]
mod tabs;

#[cfg(target_os = "macos")]
mod browser {
    use super::{
        download_delegate::TabDownloads,
        downloads::{DownloadItem, DownloadStatus, Downloads},
        profile::{Bookmark, Profile, Settings, Shortcut, Suggestion, Visit},
        session::SessionStore,
        tabs::{SavedWindow, SessionDocument, TabSet},
    };
    use objc2::MainThreadMarker;
    use objc2_foundation::NSString;
    use objc2_web_kit::{
        WKFindConfiguration, WKFindResult, WKWebViewConfiguration, WKWebsiteDataStore,
    };
    use serde::{Deserialize, Serialize};
    #[cfg(debug_assertions)]
    use std::path::Path;
    use std::{
        cell::RefCell,
        collections::HashMap,
        path::PathBuf,
        ptr::NonNull,
        rc::Rc,
        time::{Duration, Instant},
    };
    #[cfg(debug_assertions)]
    use std::{
        fs,
        io::{BufRead, BufReader},
    };
    use tao::{
        dpi::{LogicalPosition, LogicalSize},
        event::{Event, WindowEvent},
        event_loop::{
            ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget,
        },
        window::{Fullscreen, Window, WindowBuilder, WindowId},
    };
    use wry::{
        PageLoadEvent, PermissionResponse, WebView, WebViewBuilder, WebViewBuilderExtDarwin,
        WebViewBuilderExtMacos, WebViewExtMacOS,
    };

    const CHROME: &str = include_str!("../../../ui/dist/index.html");
    const CHROME_HEIGHT: u32 = 96;
    const MAX_MESSAGE_BYTES: usize = 64 * 1024;
    type SharedDownloads = Rc<RefCell<Downloads>>;

    #[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    enum Surface {
        Browse,
        #[default]
        Start,
        Library,
        Settings,
        Downloads,
    }

    #[derive(Clone, Copy, Debug, Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum ZoomStep {
        In,
        Out,
        Reset,
    }

    #[derive(Serialize)]
    struct PageToolsSnapshot {
        zoom_percent: u16,
        find_found: Option<bool>,
        fullscreen: bool,
    }

    #[derive(Clone, Default, Serialize)]
    struct ProfileSnapshot {
        surface: Surface,
        settings: Settings,
        suggestions: Vec<Suggestion>,
        history: Vec<Visit>,
        bookmarks: Vec<Bookmark>,
        shortcuts: Vec<Shortcut>,
        bookmarked: bool,
        notice: String,
    }

    #[derive(Clone, Debug, Deserialize)]
    #[serde(tag = "action", deny_unknown_fields)]
    enum Action {
        #[serde(rename = "show_surface")]
        ShowSurface { version: u8, value: Surface },
        #[serde(rename = "find_page")]
        FindPage {
            version: u8,
            value: String,
            backwards: bool,
        },
        #[serde(rename = "zoom_page")]
        ZoomPage { version: u8, value: ZoomStep },
        #[serde(rename = "toggle_fullscreen")]
        ToggleFullscreen { version: u8 },
        #[serde(rename = "print_page")]
        PrintPage { version: u8 },
        #[serde(rename = "save_pdf")]
        SavePdf { version: u8 },
        #[serde(rename = "reveal_download")]
        RevealDownload { version: u8, id: u32 },
        #[serde(rename = "decide_download")]
        DecideDownload { version: u8, id: u32, approve: bool },
        #[serde(rename = "cancel_download")]
        CancelDownload { version: u8, id: u32 },
        #[serde(rename = "clear_downloads")]
        ClearDownloads { version: u8 },
        #[serde(skip)]
        FocusFind { version: u8 },
        #[serde(rename = "query_omnibox")]
        QueryOmnibox { version: u8, value: String },
        #[serde(rename = "query_library")]
        QueryLibrary { version: u8, value: String },
        #[serde(rename = "set_theme")]
        SetTheme { version: u8, value: String },
        #[serde(rename = "set_search_engine")]
        SetSearchEngine { version: u8, value: String },
        #[serde(rename = "clear_history")]
        ClearHistory { version: u8, value: String },
        #[serde(rename = "set_restore_session")]
        SetRestoreSession { version: u8, enabled: bool },
        #[serde(rename = "set_clear_history_on_exit")]
        SetClearHistoryOnExit { version: u8, enabled: bool },
        #[serde(rename = "bookmark_current")]
        BookmarkCurrent { version: u8 },
        #[serde(rename = "import_bookmarks")]
        ImportBookmarks { version: u8 },
        #[serde(rename = "export_bookmarks")]
        ExportBookmarks { version: u8 },
        #[serde(rename = "remove_visit")]
        RemoveVisit { version: u8, id: i64 },
        #[serde(rename = "delete_bookmark")]
        DeleteBookmark { version: u8, id: i64 },
        #[serde(rename = "move_bookmark")]
        MoveBookmark {
            version: u8,
            id: i64,
            parent_id: Option<i64>,
        },
        #[serde(rename = "remove_shortcut")]
        RemoveShortcut { version: u8, id: i64 },
        #[serde(rename = "create_folder")]
        CreateFolder {
            version: u8,
            title: String,
            parent_id: Option<i64>,
        },
        #[serde(rename = "add_shortcut")]
        AddShortcut {
            version: u8,
            title: String,
            url: String,
        },
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
        #[serde(rename = "new_tab")]
        NewTab { version: u8 },
        #[serde(rename = "new_window")]
        NewWindow { version: u8 },
        #[serde(rename = "reopen_tab")]
        ReopenTab { version: u8 },
        #[serde(rename = "tab_search")]
        TabSearch { version: u8 },
        #[serde(rename = "select_tab")]
        SelectTab { version: u8, id: u32 },
        #[serde(rename = "close_tab")]
        CloseTab { version: u8, id: u32 },
        #[serde(rename = "duplicate_tab")]
        DuplicateTab { version: u8, id: u32 },
        #[serde(rename = "discard_tab")]
        DiscardTab { version: u8, id: u32 },
        #[serde(rename = "pin_tab")]
        PinTab { version: u8, id: u32, pinned: bool },
        #[serde(rename = "move_tab")]
        MoveTab { version: u8, id: u32, index: u32 },
        #[serde(rename = "close_window")]
        CloseWindow { version: u8 },
        #[serde(skip)]
        CloseSelectedTab { version: u8 },
    }

    #[cfg(debug_assertions)]
    #[derive(Deserialize)]
    #[serde(tag = "action", deny_unknown_fields)]
    enum ChromeTestAction {
        #[serde(rename = "chrome_navigate")]
        Navigate { version: u8, value: String },
        #[serde(rename = "capture_chrome")]
        Capture { version: u8 },
        #[serde(rename = "inspect_chrome")]
        Inspect { version: u8 },
        #[serde(rename = "compact_fixture_window")]
        CompactWindow { version: u8 },
        #[serde(rename = "open_fixture_browser_actions")]
        OpenBrowserActions { version: u8 },
        #[serde(rename = "capture_pdf")]
        CapturePdf { version: u8 },
        #[serde(rename = "click_fixture_link")]
        ClickFixtureLink { version: u8 },
        #[serde(rename = "capture_fixture_content")]
        CaptureFixtureContent { version: u8 },
        #[serde(rename = "print_fixture_pdf")]
        PrintFixturePdf { version: u8 },
    }

    #[cfg(debug_assertions)]
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum TestCommand {
        Chrome(ChromeTestAction),
        Native(Action),
        Quit { version: u8, action: String },
    }

    #[cfg(debug_assertions)]
    fn decode_test_command(raw: &str) -> Option<TestCommand> {
        let command: TestCommand = serde_json::from_str(raw).ok()?;
        match &command {
            TestCommand::Chrome(
                ChromeTestAction::Navigate { version: 1, .. }
                | ChromeTestAction::Capture { version: 1 }
                | ChromeTestAction::Inspect { version: 1 }
                | ChromeTestAction::CompactWindow { version: 1 }
                | ChromeTestAction::OpenBrowserActions { version: 1 }
                | ChromeTestAction::CapturePdf { version: 1 }
                | ChromeTestAction::ClickFixtureLink { version: 1 }
                | ChromeTestAction::CaptureFixtureContent { version: 1 }
                | ChromeTestAction::PrintFixturePdf { version: 1 },
            ) => Some(command),
            TestCommand::Native(action) if action.version() == 1 => Some(command),
            TestCommand::Quit { version: 1, action } if action == "quit_test" => Some(command),
            _ => None,
        }
    }

    impl Action {
        fn version(&self) -> u8 {
            match self {
                Self::ShowSurface { version, .. }
                | Self::FindPage { version, .. }
                | Self::ZoomPage { version, .. }
                | Self::ToggleFullscreen { version }
                | Self::PrintPage { version }
                | Self::SavePdf { version }
                | Self::RevealDownload { version, .. }
                | Self::DecideDownload { version, .. }
                | Self::CancelDownload { version, .. }
                | Self::ClearDownloads { version }
                | Self::FocusFind { version }
                | Self::QueryOmnibox { version, .. }
                | Self::QueryLibrary { version, .. }
                | Self::SetTheme { version, .. }
                | Self::SetSearchEngine { version, .. }
                | Self::ClearHistory { version, .. }
                | Self::SetRestoreSession { version, .. }
                | Self::SetClearHistoryOnExit { version, .. }
                | Self::BookmarkCurrent { version, .. }
                | Self::ImportBookmarks { version, .. }
                | Self::ExportBookmarks { version, .. }
                | Self::RemoveVisit { version, .. }
                | Self::DeleteBookmark { version, .. }
                | Self::MoveBookmark { version, .. }
                | Self::RemoveShortcut { version, .. }
                | Self::CreateFolder { version, .. }
                | Self::AddShortcut { version, .. }
                | Self::Navigate { version, .. }
                | Self::Back { version }
                | Self::Forward { version }
                | Self::Reload { version }
                | Self::Stop { version }
                | Self::FocusPage { version }
                | Self::FocusAddress { version }
                | Self::OpenHome { version } => *version,
                Self::CloseWindow { version } | Self::CloseSelectedTab { version } => *version,
                Self::NewTab { version }
                | Self::NewWindow { version }
                | Self::ReopenTab { version }
                | Self::TabSearch { version } => *version,
                Self::SelectTab { version, .. }
                | Self::CloseTab { version, .. }
                | Self::DuplicateTab { version, .. }
                | Self::DiscardTab { version, .. }
                | Self::PinTab { version, .. }
                | Self::MoveTab { version, .. } => *version,
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
    struct TabSnapshot {
        id: u32,
        title: String,
        pinned: bool,
        loading: bool,
        discarded: bool,
    }

    #[derive(Serialize)]
    struct Snapshot {
        version: u8,
        url: String,
        title: String,
        loading: bool,
        can_back: bool,
        can_forward: bool,
        status: Status,
        secure: bool,
        tabs: Vec<TabSnapshot>,
        selected_tab: u32,
        can_restore: bool,
        downloads: Vec<DownloadItem>,
        download_approval_count: usize,
        page_tools: PageToolsSnapshot,
        #[serde(flatten)]
        profile: ProfileSnapshot,
    }

    enum Message {
        ExternalUrls(Vec<url::Url>),
        UserPopup(WindowId, u32, u64, url::Url),
        ChromeResult(WindowId, String),
        NativeAction(isize, Action),
        Loaded(WindowId, u32, u64, PageLoadEvent, String),
        Title(WindowId, u32, u64, String),
        ProcessTerminated(WindowId, u32, u64),
        DownloadChanged,
        FindResult(WindowId, u32, u64, u64, bool),
        PdfFinished(WindowId, bool),
        DiscardSafety(WindowId, u32, u64, u64, bool),
    }

    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    struct TestControl {
        directory: PathBuf,
        cursor: u64,
        session_path: Option<PathBuf>,
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
            state: &Snapshot,
            chrome_ready: bool,
            window_number: isize,
            window_count: usize,
            native_loading: bool,
            native_fullscreen: bool,
        ) {
            #[derive(Serialize)]
            struct Observation<'a> {
                #[serde(flatten)]
                snapshot: &'a Snapshot,
                chrome_ready: bool,
                window_number: isize,
                window_count: usize,
                native_loading: bool,
                native_fullscreen: bool,
            }
            let Ok(json) = serde_json::to_vec(&Observation {
                snapshot: state,
                chrome_ready,
                window_number,
                window_count,
                native_loading,
                native_fullscreen,
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

    struct TabView {
        downloads: Option<TabDownloads>,
        webview: WebView,
        recovery_attempted: bool,
        load_started: Option<Instant>,
        finished_navigation: bool,
        discard_ticket: Option<u64>,
        find_ticket: u64,
        find_found: Option<bool>,
    }

    struct BrowserWindow {
        window: Window,
        chrome: WebView,
        tabs: TabSet,
        views: HashMap<u32, TabView>,
        proxy: EventLoopProxy<Message>,
        status: Status,
        chrome_ready: bool,
        chrome_poll_started: Option<Instant>,
        last_poll: Instant,
        test_mode: bool,
        discard_serial: u64,
        profile_state: ProfileSnapshot,
        omnibox_query: String,
        library_query: String,
        downloads: SharedDownloads,
    }

    impl BrowserWindow {
        fn new(
            target: &EventLoopWindowTarget<Message>,
            proxy: EventLoopProxy<Message>,
            saved: Option<SavedWindow>,
            test_mode: bool,
            downloads: SharedDownloads,
        ) -> Result<Self, String> {
            let window = WindowBuilder::new()
                .with_title("Muffin")
                .with_inner_size(LogicalSize::new(1100.0, 760.0))
                .with_min_inner_size(LogicalSize::new(640.0, 420.0))
                .build(target)
                .map_err(|error| error.to_string())?;
            let chrome = WebViewBuilder::new()
                .with_devtools(false)
                .with_incognito(true)
                .with_navigation_handler(|url| url == "about:blank")
                .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
                .with_download_started_handler(|_, _| false)
                .with_bounds(bounds(&window, 0, CHROME_HEIGHT))
                .build_as_child(&window)
                .map_err(|error| error.to_string())?;
            chrome
                .load_html(CHROME)
                .map_err(|error| error.to_string())?;
            let tabs = saved.map(TabSet::from_saved).unwrap_or_else(TabSet::new);
            let mut browser = Self {
                window,
                chrome,
                tabs,
                views: HashMap::new(),
                proxy,
                status: Status::Home,
                chrome_ready: false,
                chrome_poll_started: None,
                last_poll: Instant::now(),
                test_mode,
                discard_serial: 0,
                profile_state: ProfileSnapshot::default(),
                omnibox_query: String::new(),
                library_query: String::new(),
                downloads,
            };
            let selected = browser.tabs.selected();
            browser.ensure_view(selected)?;
            browser.set_selected_surface();
            browser.layout();
            Ok(browser)
        }

        fn make_web_configuration(
            test_mode: bool,
        ) -> Result<objc2::rc::Retained<WKWebViewConfiguration>, String> {
            let main_thread = MainThreadMarker::new().ok_or("WebKit must start on main thread")?;
            let configuration = unsafe { WKWebViewConfiguration::new(main_thread) };
            unsafe {
                let store = if test_mode {
                    WKWebsiteDataStore::nonPersistentDataStore(main_thread)
                } else {
                    WKWebsiteDataStore::defaultDataStore(main_thread)
                };
                configuration.setWebsiteDataStore(&store);
                configuration
                    .preferences()
                    .setJavaScriptCanOpenWindowsAutomatically(false);
                configuration.setApplicationNameForUserAgent(Some(&NSString::from_str(&format!(
                    "Muffin/{}",
                    env!("CARGO_PKG_VERSION")
                ))));
            }
            Ok(configuration)
        }

        fn create_view(&mut self, tab_id: u32) -> Result<(), String> {
            let window_id = self.window.id();
            let tab = self.tabs.get_mut(tab_id).ok_or("tab no longer exists")?;
            tab.generation = tab.generation.saturating_add(1);
            let generation = tab.generation;
            let start_url = tab.url.clone();
            tab.discarded = false;
            let loaded_proxy = self.proxy.clone();
            let title_proxy = self.proxy.clone();
            let terminate_proxy = self.proxy.clone();
            let popup_proxy = self.proxy.clone();
            let configuration = Self::make_web_configuration(self.test_mode)?;
            let builder = WebViewBuilder::new()
                .with_webview_configuration(configuration)
                .with_devtools(false)
                .with_incognito(self.test_mode)
                .with_focused(false)
                .with_navigation_handler(allowed_navigation)
                .with_new_window_req_handler(move |url, _| {
                    if let Some(url) = external_web_url(&url) {
                        let _ = popup_proxy
                            .send_event(Message::UserPopup(window_id, tab_id, generation, url));
                    }
                    wry::NewWindowResponse::Deny
                })
                .with_download_started_handler(|_, _| false)
                .with_permission_handler(|_| PermissionResponse::Deny)
                .with_on_page_load_handler(move |event, url| {
                    let _ = loaded_proxy
                        .send_event(Message::Loaded(window_id, tab_id, generation, event, url));
                })
                .with_document_title_changed_handler(move |title| {
                    let _ = title_proxy
                        .send_event(Message::Title(window_id, tab_id, generation, title));
                })
                .with_on_web_content_process_terminate_handler(move || {
                    let _ = terminate_proxy
                        .send_event(Message::ProcessTerminated(window_id, tab_id, generation));
                })
                .with_bounds(bounds(
                    &self.window,
                    CHROME_HEIGHT,
                    self.window
                        .inner_size()
                        .to_logical::<u32>(self.window.scale_factor())
                        .height
                        .saturating_sub(CHROME_HEIGHT),
                ));
            let webview = builder
                .build_as_child(&self.window)
                .map_err(|error| error.to_string())?;
            let download_proxy = self.proxy.clone();
            let downloads =
                TabDownloads::install(&webview.webview(), Rc::clone(&self.downloads), move || {
                    let _ = download_proxy.send_event(Message::DownloadChanged);
                });
            let _ = webview.set_visible(tab_id == self.tabs.selected());
            self.views.insert(
                tab_id,
                TabView {
                    downloads,
                    webview,
                    recovery_attempted: false,
                    load_started: None,
                    finished_navigation: false,
                    discard_ticket: None,
                    find_ticket: 0,
                    find_found: None,
                },
            );
            if start_url != "about:blank" {
                self.navigate(tab_id, &start_url);
            }
            Ok(())
        }

        fn ensure_view(&mut self, tab_id: u32) -> Result<(), String> {
            if !self.views.contains_key(&tab_id) {
                self.create_view(tab_id)?;
            }
            self.layout();
            Ok(())
        }

        fn layout(&self) {
            let logical = self
                .window
                .inner_size()
                .to_logical::<u32>(self.window.scale_factor());
            let showing_content = self.profile_state.surface == Surface::Browse;
            let chrome_height = if showing_content {
                CHROME_HEIGHT
            } else {
                logical.height
            };
            let _ = self
                .chrome
                .set_bounds(bounds(&self.window, 0, chrome_height));
            for (id, view) in &self.views {
                let _ = view
                    .webview
                    .set_visible(showing_content && *id == self.tabs.selected());
                let _ = view.webview.set_bounds(bounds(
                    &self.window,
                    CHROME_HEIGHT,
                    logical.height.saturating_sub(CHROME_HEIGHT),
                ));
            }
        }

        fn navigate(&mut self, tab_id: u32, url: &str) {
            self.navigate_internal(tab_id, url, false);
        }

        fn navigate_internal(&mut self, tab_id: u32, url: &str, recovering: bool) {
            if self.ensure_view(tab_id).is_err() {
                self.status = Status::Failed;
                return;
            }
            if tab_id == self.tabs.selected() && !recovering {
                self.profile_state.surface = if url == "about:blank" {
                    Surface::Start
                } else {
                    Surface::Browse
                };
                self.omnibox_query.clear();
                self.profile_state.suggestions.clear();
                self.layout();
            }
            self.begin_tab_load(tab_id, !recovering);
            if self.views[&tab_id].webview.load_url(url).is_err() {
                self.fail_tab_load(tab_id);
            }
        }

        fn begin_tab_load(&mut self, tab_id: u32, explicit: bool) {
            if let Some(view) = self.views.get_mut(&tab_id) {
                view.load_started = Some(Instant::now());
                view.finished_navigation = false;
                view.discard_ticket = None;
                view.find_ticket = view.find_ticket.wrapping_add(1);
                view.find_found = None;
                if explicit {
                    view.recovery_attempted = false;
                }
            }
            if let Some(tab) = self.tabs.get_mut(tab_id) {
                tab.loading = true;
            }
            if self.tabs.selected() == tab_id {
                self.status = Status::Loading;
            }
        }

        fn fail_tab_load(&mut self, tab_id: u32) {
            if let Some(view) = self.views.get_mut(&tab_id) {
                view.load_started = None;
            }
            if let Some(tab) = self.tabs.get_mut(tab_id) {
                tab.loading = false;
            }
            if self.tabs.selected() == tab_id {
                self.status = Status::Failed;
            }
        }

        fn selected_view(&self) -> Option<&WebView> {
            self.views
                .get(&self.tabs.selected())
                .map(|view| &view.webview)
        }

        fn find_page(&mut self, value: &str, backwards: bool) {
            let id = self.tabs.selected();
            let Some(tab) = self.tabs.get(id) else { return };
            if self.profile_state.surface != Surface::Browse || tab.loading {
                return;
            }
            let Some(view) = self.views.get_mut(&id) else {
                return;
            };
            view.find_ticket = view.find_ticket.wrapping_add(1);
            view.find_found = None;
            if value.trim().is_empty() || value.len() > 1024 || value.chars().any(char::is_control)
            {
                return;
            }
            let Some(main_thread) = MainThreadMarker::new() else {
                return;
            };
            let window = self.window.id();
            let generation = tab.generation;
            let ticket = view.find_ticket;
            let proxy = self.proxy.clone();
            let callback = block2::RcBlock::new(move |result: NonNull<WKFindResult>| {
                let found = unsafe { result.as_ref().matchFound() };
                let _ =
                    proxy.send_event(Message::FindResult(window, id, generation, ticket, found));
            });
            unsafe {
                let configuration = WKFindConfiguration::new(main_thread);
                configuration.setBackwards(backwards);
                configuration.setWraps(true);
                view.webview
                    .webview()
                    .findString_withConfiguration_completionHandler(
                        &NSString::from_str(value),
                        Some(&configuration),
                        &callback,
                    );
            }
        }

        fn zoom_page(&self, step: ZoomStep) {
            if self.profile_state.surface != Surface::Browse {
                return;
            }
            if let Some(view) = self.selected_view() {
                let current = unsafe { view.webview().pageZoom() };
                let percent = (current * 100.0).round();
                let requested = match step {
                    ZoomStep::In => percent + 10.0,
                    ZoomStep::Out => percent - 10.0,
                    ZoomStep::Reset => 100.0,
                };
                let _ = view.zoom(requested.clamp(50.0, 200.0) / 100.0);
            }
        }

        fn apply(&mut self, action: Action) -> WindowEffect {
            if action.version() != 1 {
                return WindowEffect::None;
            }
            let previously_selected = self.tabs.selected();
            match action {
                Action::Navigate { value, .. } => {
                    match address_destination(&value, &self.profile_state.settings.search_engine) {
                        Some(url) => self.navigate(self.tabs.selected(), &url),
                        None => self.status = Status::Blocked,
                    }
                }
                Action::Back { .. } => {
                    if self
                        .selected_view()
                        .is_some_and(|view| view.can_go_back().unwrap_or(false))
                    {
                        self.begin_tab_load(previously_selected, true);
                        let _ = self.views[&previously_selected].webview.go_back();
                    }
                }
                Action::Forward { .. } => {
                    if self
                        .selected_view()
                        .is_some_and(|view| view.can_go_forward().unwrap_or(false))
                    {
                        self.begin_tab_load(previously_selected, true);
                        let _ = self.views[&previously_selected].webview.go_forward();
                    }
                }
                Action::Reload { .. } => {
                    if self.views.contains_key(&previously_selected) {
                        self.begin_tab_load(previously_selected, true);
                        let _ = self.views[&previously_selected].webview.reload();
                    }
                }
                Action::Stop { .. } => {
                    if let Some(view) = self.views.get_mut(&previously_selected) {
                        unsafe { view.webview.webview().stopLoading() };
                        view.load_started = None;
                    }
                    if let Some(tab) = self.tabs.get_mut(self.tabs.selected()) {
                        tab.loading = false;
                    }
                    self.status = if self
                        .tabs
                        .get(previously_selected)
                        .is_some_and(|tab| tab.url == "about:blank")
                    {
                        Status::Home
                    } else {
                        Status::Ready
                    };
                }
                Action::FocusPage { .. } => {
                    if let Some(view) = self.selected_view() {
                        let _ = view.focus();
                    }
                }
                Action::FocusAddress { .. } => {
                    let _ = self.chrome.focus();
                    let _ = self
                        .chrome
                        .evaluate_script("window.muffinChrome?.focusAddress()");
                }
                Action::FindPage {
                    value, backwards, ..
                } => self.find_page(&value, backwards),
                Action::ZoomPage { value, .. } => self.zoom_page(value),
                Action::FocusFind { .. } => {
                    if self.profile_state.surface == Surface::Browse {
                        let _ = self.chrome.focus();
                        let _ = self.chrome.evaluate_script(
                            "window.dispatchEvent(new Event('muffin:focus-find'))",
                        );
                    }
                }
                Action::ToggleFullscreen { .. } => {
                    let fullscreen = self
                        .window
                        .fullscreen()
                        .is_none()
                        .then_some(Fullscreen::Borderless(None));
                    self.window.set_fullscreen(fullscreen);
                }
                Action::PrintPage { .. } => {
                    if self.profile_state.surface == Surface::Browse {
                        if let Some(view) = self.selected_view() {
                            if view.print().is_err() {
                                self.profile_state.notice =
                                    "Muffin couldn’t open the system print dialog.".into();
                            }
                        }
                    }
                }
                Action::SavePdf { .. } => {
                    if self.profile_state.surface == Surface::Browse && !self.test_mode {
                        if let Some(view) = self.selected_view() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("PDF document", &["pdf"])
                                .set_file_name("Web Page.pdf")
                                .save_file()
                            {
                                let id = self.window.id();
                                let proxy = self.proxy.clone();
                                super::pdf::save(view, path, move |result| {
                                    let _ =
                                        proxy.send_event(Message::PdfFinished(id, result.is_ok()));
                                });
                            }
                        }
                    }
                }
                Action::RevealDownload { id, .. } => {
                    if self.profile_state.surface == Surface::Downloads && !self.test_mode {
                        let path = self
                            .downloads
                            .borrow()
                            .completed_path(id)
                            .map(|path| path.to_owned());
                        if let Some(path) = path {
                            let _ = std::process::Command::new("/usr/bin/open")
                                .arg("-R")
                                .arg(path)
                                .spawn();
                        }
                    }
                }
                Action::OpenHome { .. } => self.navigate(self.tabs.selected(), "about:blank"),
                Action::NewTab { .. } => {
                    if self.tabs.can_create() {
                        self.tabs.create(None);
                    }
                }
                Action::NewWindow { .. } => return WindowEffect::NewWindow,
                Action::CloseWindow { .. } => return WindowEffect::CloseWindow,
                Action::CloseSelectedTab { .. } => {
                    let id = self.tabs.selected();
                    self.views.remove(&id);
                    self.tabs.close(id);
                }
                Action::ReopenTab { .. } => {
                    self.tabs.reopen();
                }
                Action::TabSearch { .. } => {
                    let _ = self
                        .chrome
                        .evaluate_script("window.muffinChrome?.focusTabSearch()");
                }
                Action::SelectTab { id, .. } => {
                    self.tabs.select(id);
                }
                Action::CloseTab { id, .. } => {
                    self.views.remove(&id);
                    self.tabs.close(id);
                }
                Action::DuplicateTab { id, .. } => {
                    self.tabs.duplicate(id);
                }
                Action::PinTab { id, pinned, .. } => {
                    self.tabs.set_pinned(id, pinned);
                }
                Action::MoveTab { id, index, .. } => {
                    self.tabs.move_to(id, index as usize);
                }
                Action::DiscardTab { id, .. } => return WindowEffect::Discard(id),
                _ => return WindowEffect::None,
            }
            if self.tabs.selected() != previously_selected {
                self.restore_selected();
            }
            self.layout();
            self.render();
            WindowEffect::None
        }

        fn set_selected_surface(&mut self) {
            self.profile_state.surface = if self
                .tabs
                .get(self.tabs.selected())
                .is_some_and(|tab| tab.url == "about:blank")
            {
                Surface::Start
            } else {
                Surface::Browse
            };
            self.omnibox_query.clear();
        }

        fn restore_selected(&mut self) {
            let selected = self.tabs.selected();
            self.set_selected_surface();
            if self.ensure_view(selected).is_err() {
                self.status = Status::Failed;
                return;
            }
            if let Some(view) = self.views.get_mut(&selected) {
                view.discard_ticket = None;
            }
            if let Some(tab) = self.tabs.get(selected) {
                self.status = if tab.loading {
                    Status::Loading
                } else if tab.url == "about:blank" {
                    Status::Home
                } else {
                    Status::Ready
                };
                self.window.set_title(&format!("{} — Muffin", tab.title));
            }
        }

        fn refresh_profile(&mut self, database: Option<&Profile>) {
            let Some(database) = database else {
                self.profile_state.notice = "The local profile is unavailable.".into();
                return;
            };
            let result = (|| -> super::profile::Result<()> {
                self.profile_state.settings = database.settings()?;
                self.profile_state.bookmarked = self
                    .tabs
                    .get(self.tabs.selected())
                    .filter(|tab| tab.url != "about:blank")
                    .map(|tab| database.bookmark_for_url(&tab.url))
                    .transpose()?
                    .flatten()
                    .is_some();
                self.profile_state.suggestions = database.suggest(&self.omnibox_query, 8)?;
                self.profile_state
                    .suggestions
                    .retain(|item| external_web_url(&item.url).is_some());
                for item in &mut self.profile_state.suggestions {
                    item.title = bounded_title(&item.title);
                }
                self.profile_state.history.clear();
                self.profile_state.bookmarks.clear();
                self.profile_state.shortcuts.clear();
                match self.profile_state.surface {
                    Surface::Library => {
                        self.profile_state.history = if self.library_query.trim().is_empty() {
                            database.history(48)?
                        } else {
                            database.search_history(&self.library_query, 48)?
                        };
                        for item in &mut self.profile_state.history {
                            item.title = bounded_title(&item.title);
                        }
                        self.profile_state.bookmarks = database
                            .bookmarks()?
                            .into_iter()
                            .filter(|item| {
                                item.url
                                    .as_ref()
                                    .is_none_or(|url| external_web_url(url).is_some())
                            })
                            .take(500)
                            .collect();
                        for item in &mut self.profile_state.bookmarks {
                            item.title = bounded_title(&item.title);
                        }
                    }
                    Surface::Start => {
                        self.profile_state.shortcuts = database
                            .shortcuts()?
                            .into_iter()
                            .filter(|item| external_web_url(&item.url).is_some())
                            .take(32)
                            .collect();
                        for item in &mut self.profile_state.shortcuts {
                            item.title = bounded_title(&item.title);
                        }
                    }
                    Surface::Browse | Surface::Settings | Surface::Downloads => {}
                }
                Ok(())
            })();
            if result.is_err() {
                self.profile_state.notice = "Muffin couldn’t read the local profile.".into();
            }
        }

        fn snapshot(&self) -> Snapshot {
            let downloads = self.downloads.borrow().items();
            let download_approval_count = downloads
                .iter()
                .filter(|item| item.status == DownloadStatus::AwaitingApproval)
                .count();
            let id = self.tabs.selected();
            let tab = self.tabs.get(id);
            let view = self.selected_view();
            Snapshot {
                version: 1,
                url: tab.map(|tab| tab.url.clone()).unwrap_or_default(),
                title: tab.map(|tab| tab.title.clone()).unwrap_or_default(),
                loading: tab.is_some_and(|tab| tab.loading),
                can_back: view.is_some_and(|view| view.can_go_back().unwrap_or(false)),
                can_forward: view.is_some_and(|view| view.can_go_forward().unwrap_or(false)),
                status: self.status,
                secure: tab.is_some_and(|tab| {
                    tab.url.starts_with("https://") && matches!(self.status, Status::Ready)
                }) && self.views.get(&id).is_some_and(|view| {
                    view.finished_navigation
                        && unsafe { view.webview.webview().hasOnlySecureContent() }
                }),
                tabs: self
                    .tabs
                    .tabs()
                    .iter()
                    .map(|tab| TabSnapshot {
                        id: tab.id,
                        title: tab.title.clone(),
                        pinned: tab.pinned,
                        loading: tab.loading,
                        discarded: tab.discarded,
                    })
                    .collect(),
                selected_tab: id,
                can_restore: self.tabs.can_reopen(),
                downloads: if self.profile_state.surface == Surface::Downloads {
                    downloads
                } else {
                    Vec::new()
                },
                download_approval_count,
                page_tools: PageToolsSnapshot {
                    zoom_percent: view.map_or(100, |view| {
                        (unsafe { view.webview().pageZoom() } * 100.0).round() as u16
                    }),
                    find_found: self.views.get(&id).and_then(|view| view.find_found),
                    fullscreen: self.window.fullscreen().is_some(),
                },
                profile: self.profile_state.clone(),
            }
        }

        fn render(&self) {
            let snapshot = self.snapshot();
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
            for downloads in self
                .views
                .values()
                .filter_map(|view| view.downloads.as_ref())
            {
                downloads.poll();
            }
            let uncompleted: Vec<_> = self
                .views
                .iter()
                .filter_map(|(&id, view)| {
                    (view
                        .load_started
                        .is_some_and(|since| since.elapsed() > Duration::from_millis(750))
                        && !unsafe { view.webview.webview().isLoading() })
                    .then_some(id)
                })
                .collect();
            if !uncompleted.is_empty() {
                for id in uncompleted {
                    self.fail_tab_load(id);
                }
                self.render();
            }
            if self
                .chrome_poll_started
                .is_some_and(|started| started.elapsed() > Duration::from_secs(2))
            {
                self.chrome_poll_started = None;
            }
            if self.chrome_poll_started.is_none() {
                let proxy = self.proxy.clone();
                let window_id = self.window.id();
                self.chrome_poll_started = Some(Instant::now());
                if self.chrome.evaluate_script_with_callback("window.muffinChrome && document.getElementById('address') ? ({version:1,action:window.muffinChrome.take()}) : null", move |result| { let _ = proxy.send_event(Message::ChromeResult(window_id, result)); }).is_err() { self.chrome_poll_started = None; }
            }
        }

        fn begin_discard(&mut self, tab_id: u32) {
            if tab_id == self.tabs.selected() || self.tabs.get(tab_id).is_none_or(|tab| tab.loading)
            {
                return;
            }
            let Some(view) = self.views.get_mut(&tab_id) else {
                return;
            };
            let generation = self.tabs.get(tab_id).unwrap().generation;
            let window_id = self.window.id();
            let proxy = self.proxy.clone();
            let wk = view.webview.webview();
            if !capture_is_idle(&view.webview) {
                return;
            }
            self.discard_serial = self.discard_serial.wrapping_add(1);
            let ticket = self.discard_serial;
            view.discard_ticket = Some(ticket);
            let callback =
                block2::RcBlock::new(move |state: objc2_web_kit::WKMediaPlaybackState| {
                    let safe = state == objc2_web_kit::WKMediaPlaybackState::None;
                    let _ = proxy.send_event(Message::DiscardSafety(
                        window_id, tab_id, generation, ticket, safe,
                    ));
                });
            unsafe {
                wk.requestMediaPlaybackStateWithCompletionHandler(&callback);
            }
        }
    }

    enum WindowEffect {
        None,
        NewWindow,
        CloseWindow,
        Discard(u32),
    }

    struct App {
        windows: Vec<BrowserWindow>,
        focused: WindowId,
        proxy: EventLoopProxy<Message>,
        session: Option<SessionStore>,
        profile: Option<Profile>,
        downloads: SharedDownloads,
        test_control: Option<TestControl>,
        session_dirty: bool,
        last_session_save: Instant,
    }

    impl App {
        fn new(
            event_loop: &EventLoop<Message>,
            test_control: Option<TestControl>,
            test_session: Option<PathBuf>,
        ) -> Result<Self, String> {
            let opened_profile = if let Some(path) = test_session.as_ref() {
                Profile::open(path.with_file_name("profile.sqlite3"))
            } else if test_control.is_some() {
                Profile::in_memory()
            } else if let Some(path) = SessionStore::default_path() {
                Profile::open(path.with_file_name("profile.sqlite3"))
            } else {
                Profile::in_memory()
            };
            let profile = match opened_profile {
                Ok(profile) => Some(profile),
                Err(error) => {
                    eprintln!("profile_open_failed: {error}");
                    None
                }
            };
            let restore = profile
                .as_ref()
                .and_then(|profile| profile.settings().ok())
                .is_none_or(|settings| settings.restore_session);
            let session = if let Some(path) = test_session {
                Some(SessionStore::new(path))
            } else if test_control.is_some() {
                None
            } else {
                SessionStore::default_path().map(SessionStore::new)
            };
            let (saved, session) = match session {
                Some(store) if !restore => (None, Some(store)),
                Some(store) => match store.load_recovering() {
                    Ok(saved) => (saved, Some(store)),
                    Err(error) => {
                        eprintln!("session_load_failed: {error}");
                        (None, None)
                    }
                },
                None => (None, None),
            };
            let proxy = event_loop.create_proxy();
            let test_mode = test_control.is_some();
            let downloads_root = if let Some(control) = &test_control {
                let path = control.directory.join("downloads");
                std::fs::create_dir_all(&path)
                    .map_err(|error| format!("create fixture downloads directory: {error}"))?;
                path
            } else {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .filter(|home| home.is_absolute())
                    .map(|home| home.join("Downloads"))
                    .unwrap_or_default()
            };
            let downloads = Rc::new(RefCell::new(Downloads::new(downloads_root)));
            let saved_windows = saved
                .map(|document| document.windows)
                .filter(|windows| !windows.is_empty());
            let mut windows = Vec::new();
            if let Some(saved_windows) = saved_windows {
                for saved in saved_windows.into_iter().take(8) {
                    windows.push(BrowserWindow::new(
                        event_loop,
                        proxy.clone(),
                        Some(saved),
                        test_mode,
                        Rc::clone(&downloads),
                    )?);
                }
            } else {
                windows.push(BrowserWindow::new(
                    event_loop,
                    proxy.clone(),
                    None,
                    test_mode,
                    Rc::clone(&downloads),
                )?);
            }
            let focused = windows[0].window.id();
            let mut app = Self {
                windows,
                focused,
                proxy,
                session,
                profile,
                downloads,
                test_control,
                session_dirty: false,
                last_session_save: Instant::now(),
            };
            app.refresh_profile_windows();
            if restore {
                app.save_session();
            }
            app.render_active();
            Ok(app)
        }

        fn active_id(&self) -> WindowId {
            self.windows
                .iter()
                .find(|window| window.window.id() == self.focused)
                .map(|window| window.window.id())
                .unwrap_or_else(|| self.windows[0].window.id())
        }
        fn window_mut(&mut self, id: WindowId) -> Option<&mut BrowserWindow> {
            self.windows
                .iter_mut()
                .find(|window| window.window.id() == id)
        }
        fn refresh_profile_windows(&mut self) {
            for window in &mut self.windows {
                window.refresh_profile(self.profile.as_ref());
                window.render();
            }
        }

        fn refresh_profile_window(&mut self, id: WindowId) {
            if let Some(window) = self
                .windows
                .iter_mut()
                .find(|window| window.window.id() == id)
            {
                window.refresh_profile(self.profile.as_ref());
                window.render();
            }
        }

        fn quit(&self) {
            self.save_session();
            if let Some(profile) = &self.profile {
                if profile.clear_history_on_exit().is_err() {
                    eprintln!("clear_history_on_exit_failed");
                }
            }
        }

        fn render_active(&self) {
            if let Some(window) = self
                .windows
                .iter()
                .find(|window| window.window.id() == self.active_id())
            {
                window.render();
            }
        }
        fn save_session(&self) {
            let Some(store) = &self.session else {
                return;
            };
            let document = SessionDocument {
                version: 1,
                windows: self
                    .windows
                    .iter()
                    .map(|window| window.tabs.saved())
                    .collect(),
            };
            if let Err(error) = store.save(&document) {
                eprintln!("session_save_failed: {error}");
            }
        }
        fn write_test_state(&self) {
            #[cfg(debug_assertions)]
            if let Some(control) = &self.test_control {
                if let Some(window) = self
                    .windows
                    .iter()
                    .find(|window| window.window.id() == self.active_id())
                {
                    use tao::platform::macos::WindowExtMacOS;
                    let native = window
                        .window
                        .ns_window()
                        .cast::<objc2::runtime::AnyObject>();
                    let number: isize = unsafe { objc2::msg_send![native, windowNumber] };
                    let style: objc2_app_kit::NSWindowStyleMask =
                        unsafe { objc2::msg_send![native, styleMask] };
                    control.write_state(
                        &window.snapshot(),
                        window.chrome_ready,
                        number,
                        self.windows.len(),
                        window
                            .selected_view()
                            .is_some_and(|view| unsafe { view.webview().isLoading() }),
                        style.contains(objc2_app_kit::NSWindowStyleMask::FullScreen),
                    );
                }
            }
        }
        fn create_window(&mut self, target: &EventLoopWindowTarget<Message>) {
            if self.windows.len() >= 8 {
                return;
            }
            if let Ok(window) = BrowserWindow::new(
                target,
                self.proxy.clone(),
                None,
                self.test_control.is_some(),
                Rc::clone(&self.downloads),
            ) {
                self.focused = window.window.id();
                self.windows.push(window);
                self.refresh_profile_window(self.focused);
                self.session_dirty = true;
            }
        }
        fn forward_urls(&mut self, urls: Vec<url::Url>) {
            let active = self.active_id();
            if let Some(window) = self.window_mut(active) {
                window.window.set_minimized(false);
                window.window.set_focus();
            }
            for url in urls {
                let value = url.to_string();
                if external_web_url(&value).is_none() {
                    continue;
                }
                if let Some(window) = self
                    .windows
                    .iter_mut()
                    .find(|window| window.window.id() == active)
                {
                    if !window.tabs.can_create() {
                        break;
                    }
                    let id = window.tabs.create(None);
                    if window.ensure_view(id).is_ok() {
                        window.navigate(id, &value);
                        window.render();
                    }
                }
            }
            self.session_dirty = true;
            self.write_test_state();
        }
        fn close_window(&mut self, id: WindowId) -> bool {
            if self.windows.len() <= 1 {
                return false;
            }
            let Some(index) = self
                .windows
                .iter()
                .position(|window| window.window.id() == id)
            else {
                return false;
            };
            self.windows.remove(index);
            if self.focused == id {
                self.focused = self.windows[0].window.id();
            }
            self.session_dirty = true;
            true
        }
        fn apply(&mut self, target: &EventLoopWindowTarget<Message>, id: WindowId, action: Action) {
            if matches!(
                action,
                Action::DecideDownload { .. }
                    | Action::CancelDownload { .. }
                    | Action::ClearDownloads { .. }
            ) {
                let visible = self.windows.iter().any(|window| {
                    window.window.id() == id && window.profile_state.surface == Surface::Downloads
                });
                if !visible {
                    return;
                }
                if matches!(action, Action::ClearDownloads { .. }) {
                    if self.downloads.borrow_mut().clear_terminal() {
                        self.handle(target, Message::DownloadChanged);
                    }
                } else {
                    self.windows
                        .iter()
                        .flat_map(|window| window.views.values())
                        .filter_map(|view| view.downloads.as_ref())
                        .any(|downloads| match action {
                            Action::DecideDownload { id, approve, .. } => {
                                downloads.decide(id, approve)
                            }
                            Action::CancelDownload { id, .. } => downloads.cancel_transfer(id),
                            _ => unreachable!("only download transfers are dispatched here"),
                        });
                }
                self.write_test_state();
                return;
            }
            if self.apply_profile_action(id, &action) {
                self.write_test_state();
                return;
            }
            let effect = if let Some(window) = self.window_mut(id) {
                window.apply(action)
            } else {
                return;
            };
            match effect {
                WindowEffect::None => {}
                WindowEffect::NewWindow => self.create_window(target),
                WindowEffect::CloseWindow => {
                    self.close_window(id);
                }
                WindowEffect::Discard(tab_id) => {
                    if let Some(window) = self.window_mut(id) {
                        window.begin_discard(tab_id);
                    }
                }
            }
            self.session_dirty = true;
            self.refresh_profile_window(id);
            self.write_test_state();
        }
        fn apply_profile_action(&mut self, id: WindowId, action: &Action) -> bool {
            let Some(window) = self.window_mut(id) else {
                return true;
            };
            match action {
                Action::ShowSurface { value, .. } => {
                    window.profile_state.notice.clear();
                    window.library_query.clear();
                    window.omnibox_query.clear();
                    if *value == Surface::Start {
                        window.navigate(window.tabs.selected(), "about:blank");
                    } else if *value == Surface::Browse {
                        window.set_selected_surface();
                    } else {
                        window.profile_state.surface = *value;
                    }
                    window.layout();
                    let _ = window.chrome.focus();
                    self.refresh_profile_window(id);
                    return true;
                }
                Action::QueryOmnibox { value, .. } => {
                    window.omnibox_query = value.chars().take(240).collect();
                    self.refresh_profile_window(id);
                    return true;
                }
                Action::QueryLibrary { value, .. } => {
                    window.library_query = value.chars().take(240).collect();
                    self.refresh_profile_window(id);
                    return true;
                }
                Action::BookmarkCurrent { .. }
                | Action::CreateFolder { .. }
                | Action::DeleteBookmark { .. }
                | Action::MoveBookmark { .. }
                | Action::ImportBookmarks { .. }
                | Action::ExportBookmarks { .. }
                | Action::RemoveVisit { .. }
                | Action::ClearHistory { .. }
                | Action::AddShortcut { .. }
                | Action::RemoveShortcut { .. }
                | Action::SetTheme { .. }
                | Action::SetSearchEngine { .. }
                | Action::SetRestoreSession { .. }
                | Action::SetClearHistoryOnExit { .. } => {}
                _ => return false,
            }

            let current = window
                .tabs
                .get(window.tabs.selected())
                .map(|tab| (tab.url.clone(), tab.title.clone()))
                .unwrap_or_default();
            let file = match action {
                Action::ImportBookmarks { .. } => {
                    let Some(path) = rfd::FileDialog::new()
                        .add_filter("Bookmarks", &["html", "htm"])
                        .pick_file()
                    else {
                        return true;
                    };
                    Some(path)
                }
                Action::ExportBookmarks { .. } => {
                    let Some(path) = rfd::FileDialog::new()
                        .set_file_name("Muffin Bookmarks.html")
                        .add_filter("Bookmarks", &["html"])
                        .save_file()
                    else {
                        return true;
                    };
                    Some(path)
                }
                _ => None,
            };
            let result = match self.profile.as_mut() {
                Some(profile) => update_profile(profile, action, &current, file.as_deref()),
                None => Err("The local profile is unavailable.".into()),
            };
            if let Some(window) = self.window_mut(id) {
                window.profile_state.notice = match result {
                    Ok(message) => message,
                    Err(error) => {
                        eprintln!("profile_action_failed: {error}");
                        "Muffin couldn’t complete that library change.".into()
                    }
                };
            }
            self.refresh_profile_windows();
            true
        }

        fn handle(&mut self, target: &EventLoopWindowTarget<Message>, message: Message) {
            match message {
                Message::ExternalUrls(urls) => self.forward_urls(urls),
                Message::DownloadChanged => {
                    for window in &self.windows {
                        window.render();
                    }
                    self.write_test_state();
                }
                Message::PdfFinished(id, saved) => {
                    if let Some(window) = self.window_mut(id) {
                        window.profile_state.notice = if saved {
                            "PDF saved.".into()
                        } else {
                            "Muffin couldn’t save the PDF.".into()
                        };
                        window.render();
                    }
                    self.write_test_state();
                }
                Message::FindResult(id, tab_id, generation, ticket, found) => {
                    if let Some(window) = self.window_mut(id) {
                        if window
                            .tabs
                            .get(tab_id)
                            .is_some_and(|tab| tab.generation == generation)
                        {
                            if let Some(view) = window
                                .views
                                .get_mut(&tab_id)
                                .filter(|view| view.find_ticket == ticket)
                            {
                                view.find_found = Some(found);
                                if tab_id == window.tabs.selected() {
                                    window.render();
                                }
                            }
                        }
                    }
                    self.write_test_state();
                }
                Message::UserPopup(id, opener, generation, url) => {
                    let current = self.active_id() == id
                        && self.window_mut(id).is_some_and(|window| {
                            window.tabs.selected() == opener
                                && window
                                    .tabs
                                    .get(opener)
                                    .is_some_and(|tab| tab.generation == generation)
                        });
                    if current {
                        self.forward_urls(vec![url]);
                    }
                }
                Message::ChromeResult(id, raw) => {
                    let action = if let Some(window) = self.window_mut(id) {
                        window.chrome_poll_started = None;
                        let reply = decode_reply(&raw);
                        if reply.is_some() && !window.chrome_ready {
                            window.chrome_ready = true;
                            window.render();
                        }
                        reply.and_then(|reply| reply.action)
                    } else {
                        None
                    };
                    if let Some(action) = action {
                        self.apply(target, id, action);
                    }
                }
                Message::NativeAction(number, action) => {
                    use tao::platform::macos::WindowExtMacOS;
                    let found = self
                        .windows
                        .iter()
                        .find(|window| {
                            let observed: isize = unsafe {
                                objc2::msg_send![
                                    window
                                        .window
                                        .ns_window()
                                        .cast::<objc2::runtime::AnyObject>(),
                                    windowNumber
                                ]
                            };
                            observed == number
                        })
                        .map(|window| window.window.id());
                    if let Some(id) = found {
                        self.apply(target, id, action);
                    }
                }
                Message::Loaded(id, tab_id, generation, event, url) => {
                    let mut visit = None;
                    if let Some(window) = self.window_mut(id) {
                        if window
                            .tabs
                            .get(tab_id)
                            .is_none_or(|tab| tab.generation != generation)
                        {
                            return;
                        }
                        match event {
                            PageLoadEvent::Started => {
                                window.begin_tab_load(tab_id, false);
                                if allowed_navigation(url.clone()) {
                                    if let Some(tab) = window.tabs.get_mut(tab_id) {
                                        tab.url = url;
                                    }
                                }
                            }
                            PageLoadEvent::Finished if allowed_navigation(url.clone()) => {
                                if let Some(view) = window.views.get_mut(&tab_id) {
                                    view.load_started = None;
                                    view.finished_navigation = true;
                                    view.discard_ticket = None;
                                }
                                let title = window
                                    .views
                                    .get(&tab_id)
                                    .and_then(|view| unsafe { view.webview.webview().title() })
                                    .map(|value| bounded_title(&value.to_string()));
                                if let Some(tab) = window.tabs.get_mut(tab_id) {
                                    tab.url = url;
                                    tab.loading = false;
                                    tab.discarded = false;
                                    if let Some(title) = title {
                                        tab.title = title;
                                    }
                                    if external_web_url(&tab.url).is_some() {
                                        visit = Some((tab.url.clone(), tab.title.clone()));
                                    }
                                }
                                if tab_id == window.tabs.selected() {
                                    window.status = if window
                                        .tabs
                                        .get(tab_id)
                                        .is_some_and(|tab| tab.url == "about:blank")
                                    {
                                        Status::Home
                                    } else {
                                        Status::Ready
                                    };
                                }
                            }
                            _ => {}
                        }
                        window.render();
                    }
                    if let (Some(profile), Some((url, title))) = (&self.profile, visit) {
                        if profile.add_visit(&url, &title, None).is_err() {
                            eprintln!("history_visit_save_failed");
                        }
                    }
                    self.refresh_profile_window(id);
                    self.session_dirty = true;
                    self.write_test_state();
                }
                Message::Title(id, tab_id, generation, title) => {
                    if let Some(window) = self.window_mut(id) {
                        let selected = tab_id == window.tabs.selected();
                        let display_title = if let Some(tab) = window
                            .tabs
                            .get_mut(tab_id)
                            .filter(|tab| tab.generation == generation)
                        {
                            tab.title = bounded_title(&title);
                            Some(tab.title.clone())
                        } else {
                            None
                        };
                        if selected {
                            if let Some(title) = display_title {
                                window.window.set_title(&format!("{} — Muffin", title));
                            }
                        }
                        window.render();
                    }
                    self.session_dirty = true;
                    self.write_test_state();
                }
                Message::ProcessTerminated(id, tab_id, generation) => {
                    if let Some(window) = self.window_mut(id) {
                        let url = window
                            .tabs
                            .get(tab_id)
                            .filter(|tab| tab.generation == generation)
                            .map(|tab| tab.url.clone());
                        if let Some(url) = url {
                            if let Some(view) = window.views.get_mut(&tab_id) {
                                view.load_started = None;
                                view.finished_navigation = false;
                            }
                            if let Some(tab) = window.tabs.get_mut(tab_id) {
                                tab.loading = false;
                            }
                            if tab_id != window.tabs.selected() {
                                window.tabs.discard(tab_id);
                                window.views.remove(&tab_id);
                                window.render();
                                return;
                            }
                            let recover = window.views.get_mut(&tab_id).is_some_and(|view| {
                                should_recover(&mut view.recovery_attempted, &url)
                            });
                            window.status = Status::Terminated;
                            if recover {
                                window.navigate_internal(tab_id, &url, true);
                            }
                            window.render();
                        }
                    }
                }
                Message::DiscardSafety(id, tab_id, generation, ticket, safe) => {
                    if let Some(window) = self.window_mut(id) {
                        let current_tab = window.tabs.get(tab_id).is_some_and(|tab| {
                            tab.generation == generation
                                && !tab.loading
                                && tab_id != window.tabs.selected()
                        });
                        let matching_view = window.views.get_mut(&tab_id).is_some_and(|view| {
                            if view.discard_ticket != Some(ticket) {
                                return false;
                            }
                            view.discard_ticket = None;
                            safe && !view
                                .downloads
                                .as_ref()
                                .is_some_and(TabDownloads::has_transfers)
                                && capture_is_idle(&view.webview)
                                && !unsafe { view.webview.webview().isLoading() }
                        });
                        if current_tab && matching_view && window.tabs.discard(tab_id) {
                            window.views.remove(&tab_id);
                            window.render();
                        }
                    }
                    self.write_test_state();
                }
            }
        }
        fn poll(&mut self, target: &EventLoopWindowTarget<Message>) -> bool {
            for window in &mut self.windows {
                window.poll_chrome();
            }
            if self.session_dirty && self.last_session_save.elapsed() >= Duration::from_millis(500)
            {
                self.save_session();
                self.session_dirty = false;
                self.last_session_save = Instant::now();
            }
            #[cfg(debug_assertions)]
            if self.poll_test_control(target) {
                return true;
            }
            #[cfg(not(debug_assertions))]
            let _ = target;
            self.write_test_state();
            false
        }

        #[cfg(debug_assertions)]
        fn poll_test_control(&mut self, target: &EventLoopWindowTarget<Message>) -> bool {
            let active = self.active_id();
            if !self
                .windows
                .iter()
                .any(|window| window.window.id() == active && window.chrome_ready)
            {
                return false;
            }
            let Some(command) = self.test_control.as_mut().and_then(TestControl::poll) else {
                return false;
            };
            match command {
                TestCommand::Quit { .. } => return true,
                TestCommand::Native(action) => self.apply(target, active, action),
                TestCommand::Chrome(ChromeTestAction::Navigate { value, .. }) => {
                    if let (Some(window), Ok(value)) =
                        (self.window_mut(active), serde_json::to_string(&value))
                    {
                        let _ = window.chrome.evaluate_script(&format!("(()=>{{const field=document.querySelector('#address');if(field){{field.value={value};field.dispatchEvent(new Event('input',{{bubbles:true}}));document.querySelector('form[role=search]')?.requestSubmit();}}}})()"));
                    }
                }
                TestCommand::Chrome(ChromeTestAction::Capture { .. }) => {
                    let directory = self
                        .test_control
                        .as_ref()
                        .map(|control| control.directory.clone());
                    if let (Some(window), Some(directory)) = (self.window_mut(active), directory) {
                        capture_webview(&window.chrome, &directory.join("chrome.tiff"));
                    }
                }
                TestCommand::Chrome(ChromeTestAction::Inspect { .. }) => {
                    let directory = self
                        .test_control
                        .as_ref()
                        .map(|control| control.directory.clone());
                    if let (Some(window), Some(directory)) = (self.window_mut(active), directory) {
                        inspect_chrome_view(&window.chrome, &directory);
                    }
                }
                TestCommand::Chrome(ChromeTestAction::CompactWindow { .. }) => {
                    if let Some(window) = self.window_mut(active) {
                        use tao::platform::macos::WindowExtMacOS;
                        let native = window
                            .window
                            .ns_window()
                            .cast::<objc2::runtime::AnyObject>();
                        let _: () = unsafe {
                            objc2::msg_send![native, setContentSize: objc2_foundation::NSSize::new(640.0, 520.0)]
                        };
                        window.layout();
                    }
                }
                TestCommand::Chrome(ChromeTestAction::OpenBrowserActions { .. }) => {
                    if let Some(window) = self.window_mut(active) {
                        let _ = window.chrome.evaluate_script(
                            "document.querySelector('button.more-button')?.click()",
                        );
                    }
                }
                TestCommand::Chrome(ChromeTestAction::CapturePdf { .. }) => {
                    let path = self
                        .test_control
                        .as_ref()
                        .map(|control| control.directory.join("fixture-export.pdf"));
                    if let (Some(window), Some(path)) = (self.window_mut(active), path) {
                        if window.profile_state.surface == Surface::Browse {
                            if let Some(view) = window.selected_view() {
                                super::pdf::save(view, path, |result| {
                                    if let Err(error) = result {
                                        eprintln!("fixture_pdf_failed: {error}");
                                    }
                                });
                            }
                        }
                    }
                }
                TestCommand::Chrome(ChromeTestAction::ClickFixtureLink { .. }) => {
                    if let Some(window) = self.window_mut(active) {
                        if let Some(view) = fixture_content(window) {
                            let _ = view.evaluate_script(
                                "document.querySelector('#fixture-download')?.click()",
                            );
                        }
                    }
                }
                TestCommand::Chrome(ChromeTestAction::CaptureFixtureContent { .. }) => {
                    let path = self
                        .test_control
                        .as_ref()
                        .map(|control| control.directory.join("content.tiff"));
                    if let (Some(window), Some(path)) = (self.window_mut(active), path) {
                        if let Some(view) = fixture_content(window) {
                            capture_webview(view, &path);
                        }
                    }
                }
                TestCommand::Chrome(ChromeTestAction::PrintFixturePdf { .. }) => {
                    let path = self
                        .test_control
                        .as_ref()
                        .map(|control| control.directory.join("fixture-print.pdf"));
                    if let (Some(window), Some(path)) = (self.window_mut(active), path) {
                        if let Some(view) = fixture_content(window) {
                            if !super::pdf::print_fixture(view, &path) {
                                eprintln!("fixture_print_failed");
                            }
                        }
                    }
                }
            }
            false
        }
    }

    fn update_profile(
        profile: &mut Profile,
        action: &Action,
        current: &(String, String),
        file: Option<&std::path::Path>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        use std::io::{Read, Write};

        let message = match action {
            Action::BookmarkCurrent { .. } => {
                let (url, title) = current;
                if external_web_url(url).is_none() {
                    return Ok("Open a web page to bookmark it.".into());
                }
                if let Some(id) = profile.bookmark_for_url(url)? {
                    profile.delete_bookmark(id)?;
                    "Bookmark removed."
                } else {
                    profile.add_bookmark(None, Some(url), title)?;
                    "Bookmark added."
                }
            }
            Action::CreateFolder {
                title, parent_id, ..
            } => {
                let title = title.trim();
                if title.is_empty() || title.len() > 512 {
                    return Ok("Enter a folder name.".into());
                }
                profile.add_bookmark(*parent_id, None, title)?;
                "Folder created."
            }
            Action::DeleteBookmark { id, .. } => {
                profile.delete_bookmark(*id)?;
                "Bookmark removed."
            }
            Action::MoveBookmark { id, parent_id, .. } => {
                profile.move_bookmark(*id, *parent_id)?;
                "Bookmark moved."
            }
            Action::ImportBookmarks { .. } => {
                let Some(path) = file else {
                    return Ok(String::new());
                };
                let mut html = String::new();
                std::fs::File::open(path)?
                    .take(4 * 1024 * 1024 + 1)
                    .read_to_string(&mut html)?;
                let imported = profile.import_bookmarks_html(&html)?;
                return Ok(format!("Imported {imported} bookmark entries."));
            }
            Action::ExportBookmarks { .. } => {
                let Some(path) = file else {
                    return Ok(String::new());
                };
                let html = profile.export_bookmarks_html()?;
                let parent = path.parent().ok_or("Export destination is unavailable.")?;
                let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
                temporary.write_all(html.as_bytes())?;
                temporary.as_file().sync_all()?;
                temporary.persist(path)?;
                "Bookmarks exported."
            }
            Action::RemoveVisit { id, .. } => {
                profile.delete_visit(*id)?;
                "History entry removed."
            }
            Action::ClearHistory { value, .. } => {
                let elapsed = match value.as_str() {
                    "hour" => Some(3_600_000),
                    "day" => Some(86_400_000),
                    "week" => Some(604_800_000),
                    "all" => None,
                    _ => return Ok("Choose a valid history range.".into()),
                };
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_millis() as i64;
                profile
                    .clear_history_range(elapsed.map(|delta| now.saturating_sub(delta)), None)?;
                "History cleared."
            }
            Action::AddShortcut { title, url, .. } => {
                let Some(url) = external_web_url(url) else {
                    return Ok("Enter an HTTP or HTTPS address for the shortcut.".into());
                };
                profile.set_shortcut(None, url.as_str(), &bounded_title(title.trim()))?;
                "Shortcut added."
            }
            Action::RemoveShortcut { id, .. } => {
                profile.remove_shortcut(*id)?;
                "Shortcut removed."
            }
            action => {
                let mut settings = profile.settings()?;
                match action {
                    Action::SetTheme { value, .. } => settings.theme = value.clone(),
                    Action::SetSearchEngine { value, .. } => settings.search_engine = value.clone(),
                    Action::SetRestoreSession { enabled, .. } => {
                        settings.restore_session = *enabled
                    }
                    Action::SetClearHistoryOnExit { enabled, .. } => {
                        settings.clear_history_on_exit = *enabled
                    }
                    _ => Err("Unsupported profile action.")?,
                }
                profile.save_settings(&settings)?;
                "Settings saved."
            }
        };
        Ok(message.into())
    }

    #[cfg(debug_assertions)]
    fn inspect_chrome_view(chrome: &WebView, directory: &Path) {
        let result_path = directory.join("chrome-layout.json");
        let temporary_path = directory.join("chrome-layout.tmp");
        let _ = chrome.evaluate_script_with_callback(
            r#"(() => {
                const panel = document.querySelector('.surface-page');
                const heading = panel?.querySelector('h1');
                if (!heading) return null;
                const bounds = panel.getBoundingClientRect();
                const label = heading.getBoundingClientRect();
                const start = panel.scrollTop;
                panel.scrollTop = 64;
                const scrollProbe = panel.scrollTop;
                panel.scrollTop = start;
                const selected = document.querySelector('.tab-item.selected');
                const selectedTabFill = selected ? getComputedStyle(selected).backgroundColor : '';
                const strip = document.querySelector('.tab-list-scroll');
                const stripBounds = strip?.getBoundingClientRect();
                const selectedBounds = selected?.getBoundingClientRect();
                const address = document.querySelector('.address-shell')?.getBoundingClientRect();
                const newTab = document.querySelector('button[aria-label="Open new tab"]')?.getBoundingClientRect();
                const actions = document.querySelector('#more-actions');
                const actionsBounds = actions?.getBoundingClientRect();
                return {
                    width: Math.round(bounds.width), height: Math.round(bounds.height),
                    heading: heading.textContent.trim(),
                    heading_visible: label.top >= bounds.top && label.bottom <= bounds.bottom &&
                        label.bottom <= window.innerHeight && label.height > 0,
                    client_height: panel.clientHeight, scroll_height: panel.scrollHeight,
                    scroll_probe: scrollProbe, theme: document.documentElement.dataset.theme,
                    selected_tab_fill: selectedTabFill,
                    address_width: Math.round(address?.width ?? 0),
                    tab_strip_width: strip?.clientWidth ?? 0,
                    tab_strip_scroll_width: strip?.scrollWidth ?? 0,
                    selected_tab_visible: !!stripBounds && !!selectedBounds &&
                        selectedBounds.left >= stripBounds.left - 1 &&
                        selectedBounds.right <= stripBounds.right + 1,
                    new_tab_visible: !!newTab && newTab.width > 0 &&
                        newTab.left >= 0 && newTab.right <= window.innerWidth,
                    more_actions_visible: !!actionsBounds && actionsBounds.width > 0 &&
                        actionsBounds.left >= 0 && actionsBounds.right <= window.innerWidth &&
                        actionsBounds.bottom <= document.querySelector('.tabbar').getBoundingClientRect().bottom,
                    more_actions_fit: !!actions && actions.scrollWidth <= actions.clientWidth,
                    more_actions_width: actions?.clientWidth ?? 0,
                    more_actions_scroll_width: actions?.scrollWidth ?? 0,
                    more_action_count: actions?.querySelectorAll('button').length ?? 0,
                };
            })()"#,
            move |json| {
                if !json.is_empty() && fs::write(&temporary_path, json).is_ok() {
                    let _ = fs::rename(&temporary_path, &result_path);
                }
            },
        );
    }

    #[cfg(debug_assertions)]
    fn fixture_content(window: &BrowserWindow) -> Option<&WebView> {
        let address = external_web_url(&window.tabs.get(window.tabs.selected())?.url)?;
        (address.scheme() == "http"
            && address.host_str() == Some("127.0.0.1")
            && window.profile_state.surface == Surface::Browse
            && matches!(window.status, Status::Ready))
        .then_some(window.selected_view())
        .flatten()
    }

    #[cfg(debug_assertions)]
    fn capture_webview(view: &WebView, image_path: &Path) {
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
            view.webview()
                .takeSnapshotWithConfiguration_completionHandler(None, &callback);
        }
    }

    fn capture_is_idle(view: &WebView) -> bool {
        let native = view.webview();
        unsafe {
            native.cameraCaptureState() == objc2_web_kit::WKMediaCaptureState::None
                && native.microphoneCaptureState() == objc2_web_kit::WKMediaCaptureState::None
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
        url == "about:blank" || external_web_url(&url).is_some()
    }

    fn external_web_url(url: &str) -> Option<url::Url> {
        if url.len() > 4096 || url.chars().any(char::is_control) {
            return None;
        }
        let parsed = url::Url::parse(url).ok()?;
        (matches!(parsed.scheme(), "http" | "https")
            && parsed.has_host()
            && parsed.username().is_empty()
            && parsed.password().is_none())
        .then_some(parsed)
    }

    fn bounded_title(value: &str) -> String {
        let mut end = value.len().min(512);
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        value[..end].to_owned()
    }

    fn address_destination(input: &str, engine: &str) -> Option<String> {
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
            let endpoint = match engine {
                "google" => "https://www.google.com/search",
                "bing" => "https://www.bing.com/search",
                _ => "https://duckduckgo.com/",
            };
            let mut search = url::Url::parse(endpoint).ok()?;
            search.query_pairs_mut().append_pair("q", input);
            search
        };
        (matches!(parsed.scheme(), "http" | "https")
            && parsed.has_host()
            && parsed.username().is_empty()
            && parsed.password().is_none())
        .then(|| parsed.to_string())
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
                    [letter] if letter.eq_ignore_ascii_case(&b'f') => {
                        Action::FocusFind { version: 1 }
                    }
                    [letter] if letter.eq_ignore_ascii_case(&b'p') => {
                        Action::PrintPage { version: 1 }
                    }
                    b"=" | b"+" => Action::ZoomPage {
                        version: 1,
                        value: ZoomStep::In,
                    },
                    b"-" => Action::ZoomPage {
                        version: 1,
                        value: ZoomStep::Out,
                    },
                    b"0" => Action::ZoomPage {
                        version: 1,
                        value: ZoomStep::Reset,
                    },
                    [letter] if letter.eq_ignore_ascii_case(&b'd') => {
                        Action::BookmarkCurrent { version: 1 }
                    }
                    [letter] if letter.eq_ignore_ascii_case(&b'y') => Action::ShowSurface {
                        version: 1,
                        value: Surface::Library,
                    },
                    b"," => Action::ShowSurface {
                        version: 1,
                        value: Surface::Settings,
                    },

                    [letter] if letter.eq_ignore_ascii_case(&b't') => Action::NewTab { version: 1 },
                    [letter] if letter.eq_ignore_ascii_case(&b'n') => {
                        Action::NewWindow { version: 1 }
                    }
                    [letter] if letter.eq_ignore_ascii_case(&b'w') => {
                        Action::CloseSelectedTab { version: 1 }
                    }
                    b"[" => Action::Back { version: 1 },
                    b"]" => Action::Forward { version: 1 },
                    b"." => Action::Stop { version: 1 },
                    _ => return event,
                };
                let number: isize = msg_send![event, windowNumber];
                let _ = proxy.send_event(Message::NativeAction(number, action));
                ptr::null_mut()
            }
        });
        let monitor: *mut AnyObject = unsafe {
            msg_send![class!(NSEvent), addLocalMonitorForEventsMatchingMask: 1usize << 10, handler: &*monitor]
        };
        unsafe { Retained::retain(monitor) }.expect("AppKit local keyboard monitor")
    }

    pub fn run() -> Result<(), String> {
        let (control, startup_urls) = launch_options()?;
        let mut builder = EventLoopBuilder::<Message>::with_user_event();
        let event_loop = builder.build();
        let session_path = control
            .as_ref()
            .and_then(|control| control.session_path.clone());
        let profile_file = if control.is_some() {
            session_path.clone()
        } else {
            Some(SessionStore::default_path().ok_or("Muffin profile directory is unavailable")?)
        };
        let instance_guard = if let Some(profile_file) = profile_file {
            use std::os::unix::fs::DirBuilderExt;
            let directory = profile_file
                .parent()
                .ok_or("Muffin profile directory is unavailable")?;
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(directory)
                .map_err(|error| format!("create Muffin profile directory: {error}"))?;
            let proxy = event_loop.create_proxy();
            match super::instance::start(directory, startup_urls.clone(), move |urls| {
                proxy.send_event(Message::ExternalUrls(urls)).is_ok()
            })? {
                super::instance::Instance::Forwarded => return Ok(()),
                super::instance::Instance::Primary(guard) => Some(guard),
            }
        } else {
            None
        };
        let mut state = App::new(&event_loop, control, session_path)?;
        if !startup_urls.is_empty() {
            state.forward_urls(startup_urls);
        }
        let mut app = Some(state);
        let _key_monitor = install_keys(event_loop.create_proxy());
        let mut next_tick = Instant::now();
        event_loop.run(move |event, target, control_flow| {
            let _keep_primary_instance = &instance_guard;
            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    window_id,
                    ..
                } => {
                    if let Some(state) = app.as_mut() {
                        if !state.close_window(window_id) {
                            state.quit();
                            app = None;
                            *control_flow = ControlFlow::Exit;
                            return;
                        }
                    }
                }
                Event::WindowEvent {
                    event: WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. },
                    window_id,
                    ..
                } => {
                    if let Some(app) = app.as_mut() {
                        if let Some(window) = app.window_mut(window_id) {
                            window.layout();
                            window.render();
                        }
                    }
                }
                Event::WindowEvent {
                    event: WindowEvent::Focused(true),
                    window_id,
                    ..
                } => {
                    if let Some(app) = app.as_mut() {
                        app.focused = window_id;
                        app.write_test_state();
                    }
                }
                Event::UserEvent(message) => {
                    if let Some(app) = app.as_mut() {
                        app.handle(target, message);
                    }
                }
                Event::Opened { urls } => {
                    if let Some(app) = app.as_mut() {
                        app.forward_urls(urls);
                    }
                }
                Event::LoopDestroyed => {
                    if let Some(state) = app.as_ref() {
                        state.quit();
                    }
                    return;
                }
                _ => {}
            }
            if Instant::now() >= next_tick {
                next_tick = Instant::now() + Duration::from_millis(100);
                let should_quit = app.as_mut().is_some_and(|state| state.poll(target));
                if should_quit {
                    if let Some(state) = app.as_ref() {
                        state.quit();
                    }
                    app = None;
                    *control_flow = ControlFlow::Exit;
                    return;
                }
            }
            *control_flow = ControlFlow::WaitUntil(next_tick);
        });
    }

    fn launch_options() -> Result<(Option<TestControl>, Vec<url::Url>), String> {
        let args = std::env::args_os().skip(1).peekable();
        #[cfg(debug_assertions)]
        let mut args = args;
        let malformed = || "unknown or malformed argument".to_owned();
        #[cfg(debug_assertions)]
        let control = if args.peek().is_some_and(|arg| arg == "--test-control") {
            args.next();
            let directory = PathBuf::from(args.next().ok_or_else(malformed)?);
            if !directory.is_absolute() || !directory.is_dir() {
                return Err(malformed());
            }
            let session_path = if args.peek().is_some_and(|arg| arg == "--test-session") {
                args.next();
                let path = PathBuf::from(args.next().ok_or_else(malformed)?);
                if path != directory.join("session.json") {
                    return Err(malformed());
                }
                Some(path)
            } else {
                None
            };
            Some(TestControl {
                directory,
                cursor: 0,
                session_path,
            })
        } else {
            None
        };
        #[cfg(not(debug_assertions))]
        let control = None;

        let urls = args
            .take(17)
            .map(|argument| {
                let raw = argument.to_str().ok_or_else(malformed)?;
                if raw.len() > 4096 || raw.chars().any(char::is_control) || raw.contains('\\') {
                    return Err(malformed());
                }
                let url = url::Url::parse(raw).map_err(|_| malformed())?;
                if !matches!(url.scheme(), "http" | "https") || !allowed_navigation(raw.to_owned())
                {
                    return Err(malformed());
                }
                Ok(url)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if urls.len() > 16 {
            return Err(malformed());
        }
        Ok((control, urls))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn address_policy_keeps_web_schemes_and_local_fixture_http() {
            assert_eq!(
                address_destination("example.com", "duckduckgo"),
                Some("https://example.com/".into())
            );
            assert_eq!(
                address_destination("127.0.0.1:8765/page", "duckduckgo"),
                Some("http://127.0.0.1:8765/page".into())
            );
            assert_eq!(
                address_destination("about:blank", "duckduckgo"),
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
            assert_eq!(
                address_destination("file:///etc/passwd", "duckduckgo"),
                None
            );
            assert_eq!(
                address_destination("https://user:secret@example.com", "duckduckgo"),
                None
            );
            assert_eq!(
                address_destination("javascript:alert(1)", "duckduckgo"),
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
