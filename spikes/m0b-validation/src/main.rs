mod downloads;
mod identity;
mod platform;
mod policy;
mod smoke;

use downloads::{Downloads, SharedDownloads};
use policy::{Command, Policy, HOME};
use serde_json::{json, Value};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};
use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget},
    window::{Window, WindowBuilder},
};
use tempfile::TempDir;
use wry::{
    NewWindowFeatures, NewWindowResponse, PageLoadEvent, WebContext, WebView, WebViewBuilder,
};

const MAX_TABS: usize = 24;
const MAX_POPUPS: usize = 4;
const CHROME: &str = include_str!("chrome.html");
const TICK: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Chrome,
    Tab(u32),
    Popup(u32),
}

enum Signal {
    ChromeInput(u32, String),
    Loaded(Role, bool, String),
    #[cfg(not(target_os = "macos"))]
    LoadError(Role),
    Title(Role, String),
    #[cfg(not(target_os = "macos"))]
    Shortcut(Option<u32>, Command),
    #[cfg(target_os = "macos")]
    AppKitShortcut(isize, Command),
    PopupClose(u32),
    NewPopup,
    Terminated(Role),
    #[cfg(not(target_os = "macos"))]
    ViewFocused(Role),
    #[cfg(not(target_os = "macos"))]
    Fullscreen(Role, bool),
    Probe(u32, String),
}

struct View {
    view: WebView,
    role: Role,
    _native: platform::NativeHooks,
    _temporary_data: Option<TempDir>,
    #[cfg(target_os = "linux")]
    container: gtk::Box,
}

struct Tab {
    id: u32,
    private: bool,
    title: String,
    url: String,
    loading: bool,
    view: Option<View>,
}

impl Tab {
    fn new(id: u32, private: bool) -> Self {
        Self {
            id,
            private,
            title: "New tab".into(),
            url: HOME.into(),
            loading: false,
            view: None,
        }
    }
}

struct Popup {
    view: View,
    window: Window,
    owner: u32,
}

struct PopupState {
    spare: Vec<Window>,
    visible: Vec<Popup>,
    next: u32,
}

#[derive(Clone)]
struct Views {
    policy: Policy,
    proxy: EventLoopProxy<Signal>,
    downloads: SharedDownloads,
    popups: Rc<RefCell<PopupState>>,
    diagnostics: bool,
}

impl Views {
    fn wrap<'a>(&self, builder: WebViewBuilder<'a>, role: Role) -> WebViewBuilder<'a> {
        let (nav, title, load) = (self.policy.clone(), self.proxy.clone(), self.proxy.clone());
        let diagnostics = self.diagnostics;
        let builder = platform::defaults(builder)
            .with_devtools(false)
            .with_general_autofill_enabled(false)
            .with_navigation_handler(move |url| {
                let allowed = if role == Role::Chrome {
                    url == HOME
                } else {
                    nav.accepts(&url)
                };
                if diagnostics && !allowed {
                    println!(
                        "navigation_denied role={role:?} class={}",
                        blocked_class(&url)
                    );
                }
                allowed
            })
            .with_document_title_changed_handler(move |text| {
                let _ = title.send_event(Signal::Title(role, text));
            })
            .with_on_page_load_handler(move |kind, url| {
                let finished = matches!(kind, PageLoadEvent::Finished);
                let _ = load.send_event(Signal::Loaded(role, finished, url));
            });
        let builder = platform::instrument_builder(builder, role, self.proxy.clone());
        #[cfg(target_os = "macos")]
        let builder = {
            let (started, completed) = (self.downloads.clone(), self.downloads.clone());
            let policy = self.policy.clone();
            builder
                .with_download_started_handler(move |url, suggested| {
                    if role == Role::Chrome {
                        return false;
                    }
                    let Some((_, target)) = started.borrow_mut().begin(&policy, &url, suggested)
                    else {
                        return false;
                    };
                    *suggested = target;
                    true
                })
                .with_download_completed_handler(move |url, path, success| {
                    let mut transfers = completed.borrow_mut();
                    let id = path
                        .as_deref()
                        .and_then(|path| transfers.by_path(path))
                        .or_else(|| transfers.by_url(&url));
                    if let Some(id) = id {
                        transfers.complete(id, success);
                    }
                })
        };
        builder
    }

    fn close_popup(&self, id: u32, reason: &'static str) {
        let closed = {
            let mut state = self.popups.borrow_mut();
            state
                .visible
                .iter()
                .position(|popup| popup.view.role == Role::Popup(id))
                .map(|index| state.visible.remove(index))
        };
        if closed.is_some() && self.diagnostics {
            println!("popup_closed id={id} reason={reason}");
        }
        drop(closed);
    }

    fn popup(
        &self,
        url: &str,
        features: NewWindowFeatures,
        owner: u32,
        private: bool,
    ) -> NewWindowResponse {
        if !self.policy.accepts(url) {
            if self.diagnostics {
                println!("popup_denied reason=policy");
            }
            return NewWindowResponse::Deny;
        }
        let (window, id) = {
            let mut state = self.popups.borrow_mut();
            let Some(window) = state.spare.pop() else {
                if self.diagnostics {
                    println!("popup_denied reason=capacity");
                }
                return NewWindowResponse::Deny;
            };
            state.next += 1;
            (window, state.next)
        };
        let _ = self.proxy.send_event(Signal::NewPopup);
        let role = Role::Popup(id);
        let name = format!("popup-{id}");
        let builder = self
            .wrap(platform::related(features), role)
            .with_id(&name)
            .with_incognito(private)
            .with_new_window_req_handler(|_, _| NewWindowResponse::Deny);
        #[cfg(target_os = "linux")]
        let parent = platform::top_box(&window);
        #[cfg(not(target_os = "linux"))]
        let parent = &window;
        #[cfg(not(target_os = "linux"))]
        let builder = builder.with_bounds(platform::bounds(&window, 0, None));
        let result = platform::build(builder, parent).ok().and_then(|view| {
            let native = platform::hooks(
                &window,
                &view,
                role,
                self.proxy.clone(),
                &self.downloads,
                &self.policy,
            )
            .ok()?;
            // The engine supplies opener state. An inherited store must match
            // its owning browsing mode before the engine loads the target URL.
            if platform::is_private(&view) != private {
                return None;
            }
            Some(View {
                view,
                role,
                _native: native,
                _temporary_data: None,
                #[cfg(target_os = "linux")]
                container: parent.clone(),
            })
        });
        let Some(view) = result else {
            if self.diagnostics {
                println!("popup_denied reason=native_setup");
            }
            return NewWindowResponse::Deny;
        };
        let response = platform::new_window_response(&view.view);
        window.set_visible(true);
        self.popups.borrow_mut().visible.push(Popup {
            view,
            window,
            owner,
        });
        if self.diagnostics {
            println!("popup_created id={id} private={private}");
        }
        response
    }
}

fn blocked_class(url: &str) -> &'static str {
    let scheme = url.split_once(':').map(|(scheme, _)| scheme);
    match scheme {
        Some("file") => "file",
        Some("http" | "https") => "web_policy",
        Some("about") => "local",
        _ => "unsupported",
    }
}

struct Browser {
    chrome: Option<View>,
    tabs: Vec<Tab>,
    window: Window,
    views: Views,
    normal_context: WebContext,
    _chrome_context: WebContext,
    _chrome_data: TempDir,
    selected: usize,
    next_tab: u32,
    next_view: u32,
    tools: bool,
    status: &'static str,
    chrome_ready: bool,
    chrome_poll: Option<(u32, Instant)>,
    chrome_serial: u32,
    last_render: Option<Value>,
    fullscreen: Option<Role>,
    #[cfg(target_os = "linux")]
    stack: gtk::Stack,
    #[cfg(target_os = "linux")]
    empty: gtk::Box,
    #[cfg(target_os = "macos")]
    _key_monitor: objc2::rc::Retained<objc2::runtime::AnyObject>,
}

impl Browser {
    fn new(
        target: &EventLoopWindowTarget<Signal>,
        proxy: EventLoopProxy<Signal>,
        policy: Policy,
        root: &Path,
        diagnostics: bool,
        size: (u32, u32),
    ) -> Result<Self, &'static str> {
        let downloads_dir = root.join("downloads");
        std::fs::create_dir_all(&downloads_dir).map_err(|_| "could not create downloads folder")?;
        let window = WindowBuilder::new()
            .with_title("Muffin · M0b validation")
            .with_visible(false)
            .with_inner_size(LogicalSize::new(size.0, size.1))
            .with_min_inner_size(LogicalSize::new(730, 470))
            .build(target)
            .map_err(|_| "could not create native window")?;
        let views = Views {
            policy,
            proxy: proxy.clone(),
            downloads: Rc::new(RefCell::new(Downloads::new(downloads_dir, diagnostics))),
            popups: Rc::new(RefCell::new(PopupState {
                spare: Vec::new(),
                visible: Vec::new(),
                next: 0,
            })),
            diagnostics,
        };
        let chrome_data = tempfile::Builder::new()
            .prefix("muffin-validation-chrome-")
            .tempdir()
            .map_err(|_| "could not allocate temporary chrome data")?;
        let mut chrome_context = WebContext::new(Some(chrome_data.path().to_path_buf()));
        #[cfg(target_os = "linux")]
        let (chrome_parent, stack, empty) = {
            use gtk::prelude::*;
            let top = platform::top_box(&window);
            let holder = platform::new_box();
            holder.set_size_request(-1, platform::CHROME_HEIGHT as i32);
            top.pack_start(&holder, false, false, 0);
            let stack = gtk::Stack::new();
            let empty = platform::new_box();
            stack.add_named(&empty, "empty");
            top.pack_start(&stack, true, true, 0);
            holder.show();
            empty.show();
            stack.show();
            (holder, stack, empty)
        };
        let builder = views
            .wrap(
                WebViewBuilder::new_with_web_context(&mut chrome_context),
                Role::Chrome,
            )
            .with_id("muffin-chrome")
            .with_incognito(true)
            .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
            .with_drag_drop_handler(|_| true)
            .with_back_forward_navigation_gestures(false)
            .with_hotkeys_zoom(false);
        #[cfg(not(target_os = "linux"))]
        let builder =
            builder.with_bounds(platform::bounds(&window, 0, Some(platform::CHROME_HEIGHT)));
        #[cfg(target_os = "linux")]
        let chrome_view = platform::build(builder, &chrome_parent);
        #[cfg(not(target_os = "linux"))]
        let chrome_view = platform::build(builder, &window);
        let chrome_view = chrome_view.map_err(|_| "could not construct isolated chrome")?;
        let native = platform::hooks(
            &window,
            &chrome_view,
            Role::Chrome,
            proxy.clone(),
            &views.downloads,
            &views.policy,
        )?;
        if !platform::is_private(&chrome_view) {
            return Err("engine could not confirm ephemeral chrome store");
        }
        chrome_view
            .load_html(CHROME)
            .map_err(|_| "chrome document load failed")?;
        let chrome = View {
            view: chrome_view,
            role: Role::Chrome,
            _native: native,
            _temporary_data: None,
            #[cfg(target_os = "linux")]
            container: chrome_parent,
        };
        #[cfg(target_os = "macos")]
        let key_monitor = platform::install_keys(proxy);
        let mut browser = Self {
            chrome: Some(chrome),
            tabs: vec![Tab::new(1, false)],
            window,
            views,
            normal_context: WebContext::new(Some(root.join("normal-web-data"))),
            _chrome_context: chrome_context,
            _chrome_data: chrome_data,
            next_tab: 1,
            next_view: 0,
            selected: 0,
            tools: false,
            status: "",
            chrome_poll: None,
            chrome_serial: 0,
            chrome_ready: false,
            last_render: None,
            fullscreen: None,
            #[cfg(target_os = "linux")]
            stack,
            #[cfg(target_os = "linux")]
            empty,
            #[cfg(target_os = "macos")]
            _key_monitor: key_monitor,
        };
        browser.reserve_popup(target);
        browser.window.set_visible(true);
        browser.layout();
        browser.log("chrome_created isolated=true");
        Ok(browser)
    }

    fn log(&self, entry: &str) {
        if self.views.diagnostics {
            println!("{entry}");
        }
    }

    fn reserve_popup(&mut self, target: &EventLoopWindowTarget<Signal>) {
        let mut state = self.views.popups.borrow_mut();
        if !state.spare.is_empty() || state.visible.len() >= MAX_POPUPS {
            return;
        }
        let builder = WindowBuilder::new()
            .with_title("Muffin · Browsing popup")
            .with_inner_size(LogicalSize::new(660u32, 500u32))
            .with_visible(false);
        if let Ok(window) = builder.build(target) {
            state.spare.push(window);
        }
    }

    fn active(&self) -> &Tab {
        &self.tabs[self.selected]
    }

    fn load(&mut self, destination: String) -> Result<(), &'static str> {
        if !self.views.policy.accepts(&destination) {
            return Err("Address blocked by navigation policy");
        }
        if self.active().view.is_none() {
            self.create_view()?;
        }
        let tab = &mut self.tabs[self.selected];
        let view = &tab.view.as_ref().ok_or("Content view unavailable")?.view;
        view.load_url(&destination)
            .map_err(|_| "Engine could not start navigation")?;
        tab.loading = true;
        self.layout();
        self.focus_page();
        Ok(())
    }

    fn create_view(&mut self) -> Result<(), &'static str> {
        self.next_view += 1;
        let role = Role::Tab(self.next_view);
        let name = format!("tab-{}", self.next_view);
        #[cfg(not(target_os = "linux"))]
        let height = self.height();
        let owner = self.active().id;
        let private = self.active().private;
        let popup = self.views.clone();
        let temporary_data: Option<TempDir> = if cfg!(target_os = "windows") && private {
            Some(
                tempfile::Builder::new()
                    .prefix("muffin-validation-private-")
                    .tempdir()
                    .map_err(|_| "could not create private runtime directory")?,
            )
        } else {
            None
        };
        let mut private_context = temporary_data
            .as_ref()
            .map(|dir| WebContext::new(Some(dir.path().to_path_buf())));
        let builder = match private_context.as_mut() {
            Some(context) => WebViewBuilder::new_with_web_context(context),
            None if private => WebViewBuilder::new(),
            None => WebViewBuilder::new_with_web_context(&mut self.normal_context),
        };
        let builder = self
            .views
            .wrap(builder, role)
            .with_id(&name)
            .with_incognito(private)
            .with_focused(false)
            .with_new_window_req_handler(move |url, features| {
                popup.popup(&url, features, owner, private)
            });
        #[cfg(target_os = "linux")]
        let parent = platform::new_box();
        #[cfg(target_os = "linux")]
        let result = platform::build(builder, &parent);
        #[cfg(not(target_os = "linux"))]
        let result = platform::build(
            builder.with_bounds(platform::bounds(&self.window, height, None)),
            &self.window,
        );
        let view = result.map_err(|_| "Could not create content WebView")?;
        let native = platform::hooks(
            &self.window,
            &view,
            role,
            self.views.proxy.clone(),
            &self.views.downloads,
            &self.views.policy,
        )?;
        if platform::is_private(&view) != private {
            return Err("Engine did not establish the requested storage boundary");
        }
        #[cfg(target_os = "linux")]
        {
            use gtk::prelude::*;
            self.stack.add_named(&parent, &name);
            parent.show();
        }
        self.tabs[self.selected].view = Some(View {
            view,
            role,
            _native: native,
            _temporary_data: temporary_data,
            #[cfg(target_os = "linux")]
            container: parent,
        });
        self.log(&format!("view_created role={role:?} private={private}"));
        Ok(())
    }

    fn height(&self) -> u32 {
        if self.fullscreen.is_some() {
            0
        } else if self.tools {
            platform::TOOLS_HEIGHT
        } else {
            platform::CHROME_HEIGHT
        }
    }

    fn layout(&mut self) {
        let height = self.height();
        #[cfg(target_os = "linux")]
        {
            use gtk::prelude::*;
            if let Some(chrome) = self.chrome.as_ref() {
                chrome.container.set_visible(height > 0);
                chrome.container.set_size_request(-1, height as i32);
            }
            let container = self
                .active()
                .view
                .as_ref()
                .map_or(&self.empty, |view| &view.container);
            self.stack.set_visible_child(container);
        }
        #[cfg(not(target_os = "linux"))]
        {
            if let Some(chrome) = self.chrome.as_ref() {
                let _ = chrome.view.set_visible(height > 0);
                if height > 0 {
                    let _ = chrome
                        .view
                        .set_bounds(platform::bounds(&self.window, 0, Some(height)));
                }
            }
            for (index, tab) in self.tabs.iter().enumerate() {
                if let Some(view) = tab.view.as_ref() {
                    if index == self.selected {
                        let _ = view
                            .view
                            .set_bounds(platform::bounds(&self.window, height, None));
                    }
                    let _ = view.view.set_visible(index == self.selected);
                }
            }
        }
    }

    fn release(&mut self, index: usize) {
        if let Some(view) = self.tabs[index].view.take() {
            let id = self.tabs[index].id;
            let private = self.tabs[index].private;
            let retired: Vec<_> = {
                let mut popups = self.views.popups.borrow_mut();
                let (retired, keep) = popups
                    .visible
                    .drain(..)
                    .partition(|popup| popup.owner == id);
                popups.visible = keep;
                retired
            };
            drop(retired);
            #[cfg(target_os = "linux")]
            {
                use gtk::prelude::*;
                self.stack.remove(&view.container);
            }
            self.log(&format!(
                "view_released role={:?} private={private}",
                view.role
            ));
            drop(view);
            self.tabs[index].loading = false;
            if private {
                self.tabs[index].url = HOME.into();
                self.tabs[index].title = "New tab".into();
            }
        }
    }

    fn select(&mut self, index: usize) {
        self.leave_fullscreen();
        self.selected = index;
        self.layout();
        if self.active().view.is_some() {
            self.focus_page();
        } else {
            self.focus_address();
        }
    }

    fn leave_fullscreen(&mut self) {
        if self.fullscreen.take().is_some() {
            self.window.set_fullscreen(None);
        }
    }

    fn focus_page(&self) {
        if let Some(view) = &self.active().view {
            platform::focus(&self.window, &view.view);
            self.log(&format!("focus_requested role={:?}", view.role));
        }
    }

    fn focus_address(&self) {
        if let Some(chrome) = self.chrome.as_ref() {
            if self.fullscreen.is_none() {
                platform::focus(&self.window, &chrome.view);
                let _ = chrome
                    .view
                    .evaluate_script("window.muffinControls?.focus()");
                self.log("focus_requested role=Chrome");
            }
        }
    }

    fn command(&mut self, command: Command) {
        use Command::*;
        self.status = "";
        match command {
            Navigate(input) => {
                let Some(destination) = self.views.policy.destination(&input) else {
                    self.log("address_rejected");
                    self.status = "Address rejected · HTTP and HTTPS only";
                    return;
                };
                if let Err(message) = self.load(destination) {
                    self.status = message;
                }
            }
            NewTab | NewPrivate => {
                if self.tabs.len() >= MAX_TABS {
                    self.status = "Close a tab before opening another";
                    return;
                }
                self.next_tab += 1;
                self.tabs
                    .push(Tab::new(self.next_tab, command == NewPrivate));
                self.select(self.tabs.len() - 1);
            }
            CloseTab => {
                self.leave_fullscreen();
                let index = self.selected;
                self.release(index);
                self.tabs.remove(index);
                if self.tabs.is_empty() {
                    self.next_tab += 1;
                    self.tabs.push(Tab::new(self.next_tab, false));
                }
                self.select(index.min(self.tabs.len() - 1));
            }
            NextTab => self.select((self.selected + 1) % self.tabs.len()),
            SelectTab(id) => {
                if let Some(index) = self.tabs.iter().position(|tab| tab.id == id) {
                    self.select(index);
                }
            }
            ReleaseInactive => {
                for index in 0..self.tabs.len() {
                    if index != self.selected {
                        self.release(index);
                    }
                }
                self.status = "Inactive views released · Go reinstates a saved normal address";
            }
            ToggleTools => {
                self.tools = !self.tools;
                self.layout();
            }
            FocusAddress => {
                self.leave_fullscreen();
                self.layout();
                self.focus_address();
            }
            FocusPage => self.focus_page(),
            ChooseDownloads => {
                let directory = rfd::FileDialog::new()
                    .set_title("Choose folder for Muffin validation downloads")
                    .set_parent(&self.window)
                    .pick_folder();
                if let Some(directory) = directory.and_then(|path| path.canonicalize().ok()) {
                    self.views.downloads.borrow_mut().set_folder(directory);
                    self.status = "Download folder selected";
                }
            }
            CancelDownloads => {
                let callbacks = self.views.downloads.borrow_mut().take_cancellations();
                for call in callbacks {
                    call();
                }
            }
            Back | Forward | Reload | Stop | ToggleMute | Print => {
                let Some(view) = self.active().view.as_ref().map(|view| &view.view) else {
                    return;
                };
                match command {
                    Back => {
                        let _ = view.go_back();
                    }
                    Forward => {
                        let _ = view.go_forward();
                    }
                    Reload => {
                        let _ = view.reload();
                    }
                    Stop => {
                        platform::stop(view);
                        self.tabs[self.selected].loading = false;
                    }
                    ToggleMute => platform::toggle_mute(view),
                    Print => {
                        let _ = view.print();
                    }
                    _ => unreachable!(),
                }
            }
        }
    }

    fn shortcut(&mut self, popup: Option<u32>, command: Command) {
        self.log(&format!(
            "native_shortcut scope={}",
            if popup.is_some() { "popup" } else { "main" }
        ));
        if let Some(id) = popup {
            if command == Command::CloseTab {
                self.views.close_popup(id, "shortcut");
                return;
            }
            if matches!(
                command,
                Command::Back
                    | Command::Forward
                    | Command::Reload
                    | Command::Stop
                    | Command::Print
                    | Command::FocusPage
            ) {
                if let Some(popup) = self
                    .views
                    .popups
                    .borrow()
                    .visible
                    .iter()
                    .find(|popup| popup.view.role == Role::Popup(id))
                {
                    let view = &popup.view.view;
                    match command {
                        Command::Back => {
                            let _ = view.go_back();
                        }
                        Command::Forward => {
                            let _ = view.go_forward();
                        }
                        Command::Reload => {
                            let _ = view.reload();
                        }
                        Command::Stop => platform::stop(view),
                        Command::Print => {
                            let _ = view.print();
                        }
                        Command::FocusPage => platform::focus(&popup.window, view),
                        _ => unreachable!(),
                    }
                }
                return;
            }
            self.window.set_focus();
        }
        self.command(command);
    }

    fn signal(&mut self, signal: Signal) {
        match signal {
            Signal::ChromeInput(serial, raw) => {
                if self
                    .chrome_poll
                    .is_some_and(|(pending, _)| pending == serial)
                {
                    self.chrome_poll = None;
                    if let Some(command) = policy::chrome_result(&raw) {
                        self.log("chrome_command_accepted");
                        self.command(command);
                    }
                }
            }
            Signal::Loaded(Role::Chrome, true, url) if url == HOME => {
                self.chrome_ready = true;
                self.last_render = None;
                self.focus_address();
            }
            Signal::Loaded(role, finished, url) => {
                if let Some(tab) = self.tab_for(role) {
                    tab.loading = !finished;
                    if let Ok(parsed) = url::Url::parse(&url) {
                        if parsed.username().is_empty() && parsed.password().is_none() {
                            tab.url = url.chars().take(policy::MAX_INPUT).collect();
                        }
                    }
                }
                if role != Role::Chrome {
                    self.log(&format!(
                        "navigation role={role:?} phase={}",
                        if finished { "finished" } else { "started" }
                    ));
                }
            }
            Signal::Title(role, title) => {
                if let Some(tab) = self.tab_for(role) {
                    tab.title = title
                        .chars()
                        .filter(|ch| !ch.is_control())
                        .take(80)
                        .collect();
                    if tab.title.trim().is_empty() {
                        tab.title = "Page".into();
                    }
                }
            }
            #[cfg(not(target_os = "macos"))]
            Signal::LoadError(role) => {
                if let Some(tab) = self.tab_for(role) {
                    tab.loading = false;
                }
                if self
                    .active()
                    .view
                    .as_ref()
                    .is_some_and(|view| view.role == role)
                {
                    self.status = "Engine reported a navigation error";
                }
                self.log(&format!("navigation_error role={role:?}"));
            }
            #[cfg(not(target_os = "macos"))]
            Signal::Shortcut(popup, command) => self.shortcut(popup, command),
            #[cfg(target_os = "macos")]
            Signal::AppKitShortcut(number, command) => {
                if number == platform::window_number(&self.window) {
                    self.shortcut(None, command);
                } else {
                    let popup = self.views.popups.borrow().visible.iter().find_map(|popup| {
                        match popup.view.role {
                            Role::Popup(id) if number == platform::window_number(&popup.window) => {
                                Some(id)
                            }
                            _ => None,
                        }
                    });
                    if let Some(id) = popup {
                        self.shortcut(Some(id), command);
                    }
                }
            }
            Signal::PopupClose(id) => self.views.close_popup(id, "webview"),
            Signal::Terminated(role) => {
                self.log(&format!("renderer_terminated role={role:?}"));
                if role == Role::Chrome {
                    self.chrome_ready = false;
                    self.chrome_poll = None;
                    if let Some(chrome) = self.chrome.as_ref() {
                        let _ = chrome.view.load_html(CHROME);
                    }
                } else if let Some(tab) = self.tab_for(role) {
                    tab.loading = false;
                    if self
                        .active()
                        .view
                        .as_ref()
                        .is_some_and(|view| view.role == role)
                    {
                        self.status = "Renderer stopped · use Reload to recover";
                    }
                }
            }
            #[cfg(not(target_os = "macos"))]
            Signal::ViewFocused(role) => self.log(&format!("native_view_focused role={role:?}")),
            #[cfg(not(target_os = "macos"))]
            Signal::Fullscreen(role, fullscreen) => {
                if self
                    .active()
                    .view
                    .as_ref()
                    .is_some_and(|view| view.role == role)
                {
                    self.fullscreen = fullscreen.then_some(role);
                    self.window.set_fullscreen(fullscreen.then(|| {
                        tao::window::Fullscreen::Borderless(self.window.current_monitor())
                    }));
                    self.layout();
                } else if let Role::Popup(id) = role {
                    if let Some(popup) = self
                        .views
                        .popups
                        .borrow()
                        .visible
                        .iter()
                        .find(|popup| popup.view.role == Role::Popup(id))
                    {
                        popup.window.set_fullscreen(fullscreen.then(|| {
                            tao::window::Fullscreen::Borderless(popup.window.current_monitor())
                        }));
                    }
                }
            }
            Signal::NewPopup | Signal::Probe(_, _) => {}
        }
    }

    fn tab_for(&mut self, role: Role) -> Option<&mut Tab> {
        self.tabs
            .iter_mut()
            .find(|tab| tab.view.as_ref().is_some_and(|view| view.role == role))
    }

    fn tick(&mut self) {
        let Some(chrome) = self.chrome.as_ref() else {
            return;
        };
        if !self.chrome_ready || chrome.view.url().ok().as_deref() != Some(HOME) {
            return;
        }
        if self
            .chrome_poll
            .is_none_or(|(_, started)| started.elapsed() > Duration::from_secs(2))
        {
            self.chrome_serial = self.chrome_serial.wrapping_add(1);
            let proxy = self.views.proxy.clone();
            let serial = self.chrome_serial;
            self.chrome_poll = Some((serial, Instant::now()));
            let _ = chrome.view.evaluate_script_with_callback(
                "window.muffinControls?.take() ?? null",
                move |raw| {
                    let _ = proxy.send_event(Signal::ChromeInput(serial, raw));
                },
            );
        }
        let tab = self.active();
        let webview = tab.view.as_ref().map(|view| &view.view);
        let audio = webview.and_then(platform::audio);
        let tabs: Vec<_> = self
            .tabs
            .iter()
            .map(|tab| {
                let audio = tab
                    .view
                    .as_ref()
                    .and_then(|view| platform::audio(&view.view))
                    .is_some_and(|(playing, muted)| playing && !muted);
                json!({"id": tab.id, "private": tab.private, "title": tab.title, "audio": audio})
            })
            .collect();
        let downloads = self.views.downloads.borrow();
        let download_status = downloads.status();
        let state = json!({
            "selected": tab.id,
            "url": if tab.url == HOME { "" } else { &tab.url },
            "transport": policy::transport(&tab.url),
            "back": webview.is_some_and(|view| view.can_go_back().unwrap_or(false)),
            "forward": webview.is_some_and(|view| view.can_go_forward().unwrap_or(false)),
            "loading": tab.loading,
            "tools": self.tools,
            "muted": audio.map(|(_, muted)| muted),
            "cancel": downloads.cancelable(),
            "tabs": tabs,
            "status": if !self.status.is_empty() { self.status } else if tab.loading { "Loading…" } else { &download_status },
        });
        drop(downloads);
        if self.last_render.as_ref() != Some(&state) {
            let _ = chrome
                .view
                .evaluate_script(&format!("window.muffinControls?.render({state})"));
            self.last_render = Some(state);
        }
    }

    fn shutdown(&mut self) {
        self.leave_fullscreen();
        for index in 0..self.tabs.len() {
            self.release(index);
        }
        let (visible, spare) = {
            let mut popups = self.views.popups.borrow_mut();
            (
                std::mem::take(&mut popups.visible),
                std::mem::take(&mut popups.spare),
            )
        };
        drop((visible, spare));
        self.chrome.take();
        self.log("shutdown storage_views=closed");
    }
}

struct Options {
    root: Option<PathBuf>,
    start: Option<String>,
    diagnostic: bool,
    identity_only: bool,
    fixture: Option<[String; 2]>,
    smoke: Option<smoke::Mode>,
    smoke_x11_gesture: bool,
    smoke_core_only: bool,
}

fn options() -> Result<Options, &'static str> {
    let mut args = std::env::args().skip(1);
    let mut result = Options {
        root: None,
        start: None,
        diagnostic: false,
        identity_only: false,
        fixture: None,
        smoke: None,
        smoke_x11_gesture: false,
        smoke_core_only: false,
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--data-dir" => result.root = Some(args.next().ok_or("missing data directory")?.into()),
            "--start" => result.start = Some(args.next().ok_or("missing address")?),
            "--diagnostics" => result.diagnostic = true,
            "--identity" => result.identity_only = true,
            "--fixture" => {
                result.fixture = Some([
                    args.next().ok_or("missing fixture origin")?,
                    args.next().ok_or("missing fixture peer")?,
                ])
            }
            "--smoke-x11-gesture" => result.smoke_x11_gesture = true,
            "--smoke-core-only" => result.smoke_core_only = true,
            "--smoke" => {
                result.smoke = Some(match args.next().as_deref() {
                    Some("first") => smoke::Mode::First,
                    Some("relaunch") => smoke::Mode::Relaunch,
                    _ => return Err("expected first or relaunch smoke mode"),
                });
                result.diagnostic = true;
            }
            "--help" | "-h" => {
                println!("Muffin M0b engineering build\n  --data-dir DIR --start ADDRESS --diagnostics\n  --fixture LOOPBACK_ORIGIN LOOPBACK_PEER --smoke first|relaunch\n  --identity");
                std::process::exit(0);
            }
            _ => return Err("unrecognized validation option"),
        }
    }
    if result.smoke.is_some()
        && (result.fixture.is_none() || result.root.is_none() || result.start.is_some())
    {
        return Err("smoke requires a fixture and isolated data directory");
    }
    if (result.smoke_x11_gesture || result.smoke_core_only) && result.smoke.is_none() {
        return Err("smoke inputs require smoke mode");
    }
    if result.smoke_x11_gesture && (!cfg!(target_os = "linux") || result.smoke_core_only) {
        return Err("X11 input requires Linux desktop smoke mode");
    }
    Ok(result)
}

fn data_root() -> Result<PathBuf, &'static str> {
    #[cfg(target_os = "macos")]
    let (var, suffix) = ("HOME", "Library/Application Support/Muffin M0b Validation");
    #[cfg(target_os = "windows")]
    let (var, suffix) = ("LOCALAPPDATA", "Muffin M0b Validation");
    #[cfg(target_os = "linux")]
    let (var, suffix) = ("HOME", ".local/share/muffin-m0b-validation");
    std::env::var_os(var)
        .map(|base| PathBuf::from(base).join(suffix))
        .ok_or("set --data-dir for this environment")
}

fn fail(message: &str) -> ! {
    eprintln!("validation stopped: {message}");
    std::process::exit(1);
}

fn main() {
    let options = options().unwrap_or_else(|message| fail(message));
    let engine = wry::webview_version()
        .unwrap_or_else(|_| fail("required platform WebView runtime was unavailable"));
    identity::report(&engine).unwrap_or_else(|_| fail("could not record build identity"));
    if options.identity_only {
        return;
    }
    let policy = match options.fixture.as_ref() {
        Some([first, second]) => Policy::fixture(first, second)
            .unwrap_or_else(|| fail("expected two distinct loopback origins")),
        None => Policy::browsing(),
    };
    let root = options
        .root
        .unwrap_or_else(|| data_root().unwrap_or_else(|message| fail(message)));
    std::fs::create_dir_all(&root).unwrap_or_else(|_| fail("could not create app data directory"));
    let root = root
        .canonicalize()
        .unwrap_or_else(|_| fail("could not resolve app data directory"));
    let canary = if options.smoke.is_some() {
        Some(smoke::private_canary(&root).unwrap_or_else(|message| fail(message)))
    } else {
        None
    };
    let events = EventLoopBuilder::<Signal>::with_user_event().build();
    let mut browser = Browser::new(
        &events,
        events.create_proxy(),
        policy,
        &root,
        options.diagnostic,
        (1080, 740),
    )
    .unwrap_or_else(|message| fail(message));
    if let Some(start) = options.start.or_else(|| {
        options
            .fixture
            .as_ref()
            .filter(|_| options.smoke.is_none())
            .map(|origins| origins[0].clone())
    }) {
        browser.command(Command::Navigate(start));
    }
    let mut smoke = options.smoke.zip(options.fixture).map(|(mode, origins)| {
        smoke::Smoke::new(
            mode,
            origins,
            canary.expect("smoke canary initialized"),
            options.smoke_x11_gesture,
            options.smoke_core_only,
        )
    });
    let mut next_tick = Instant::now();
    events.run(move |event, target, control| {
        *control = ControlFlow::WaitUntil(next_tick);
        match event {
            Event::UserEvent(Signal::Probe(id, raw)) => {
                if let Some(smoke) = smoke.as_mut() {
                    smoke.probe_result(id, &raw);
                }
            }
            Event::UserEvent(Signal::NewPopup) => browser.reserve_popup(target),
            Event::UserEvent(signal) => browser.signal(signal),
            Event::WindowEvent {
                window_id,
                event: WindowEvent::CloseRequested,
                ..
            } if window_id == browser.window.id() => {
                browser.shutdown();
                *control = ControlFlow::Exit;
                return;
            }
            Event::WindowEvent {
                window_id,
                event: WindowEvent::CloseRequested | WindowEvent::Destroyed,
                ..
            } => {
                let id = browser
                    .views
                    .popups
                    .borrow()
                    .visible
                    .iter()
                    .find_map(|popup| match popup.view.role {
                        Role::Popup(id) if popup.window.id() == window_id => Some(id),
                        _ => None,
                    });
                if let Some(id) = id {
                    browser.views.close_popup(id, "native_window");
                    browser.reserve_popup(target);
                }
            }
            Event::WindowEvent {
                window_id,
                event: WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. },
                ..
            } => {
                if window_id == browser.window.id() {
                    browser.layout();
                }
                #[cfg(not(target_os = "linux"))]
                if let Some(popup) = browser
                    .views
                    .popups
                    .borrow()
                    .visible
                    .iter()
                    .find(|popup| popup.window.id() == window_id)
                {
                    let _ = popup
                        .view
                        .view
                        .set_bounds(platform::bounds(&popup.window, 0, None));
                }
            }
            _ => {}
        }
        if Instant::now() >= next_tick {
            next_tick = Instant::now() + TICK;
            browser.tick();
            if let Some(suite) = smoke.as_mut() {
                if let Some(passed) = suite.tick(&mut browser) {
                    browser.shutdown();
                    *control = ControlFlow::ExitWithCode(if passed { 0 } else { 1 });
                    return;
                }
            }
        }
        *control = ControlFlow::WaitUntil(next_tick);
    });
}
