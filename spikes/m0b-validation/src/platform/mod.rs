use crate::policy::Command;
#[cfg(not(target_os = "macos"))]
use crate::Signal;
#[cfg(not(target_os = "linux"))]
use tao::dpi::{LogicalPosition, LogicalSize};
#[cfg(not(target_os = "macos"))]
use tao::event_loop::EventLoopProxy;
#[cfg(not(target_os = "linux"))]
use tao::window::Window;
use wry::{NewWindowFeatures, NewWindowResponse, WebView, WebViewBuilder};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod mac;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::*;
#[cfg(target_os = "macos")]
pub use mac::*;
#[cfg(target_os = "windows")]
pub use windows::*;

#[cfg(not(target_os = "linux"))]
use wry::Rect;

#[cfg(not(target_os = "linux"))]
pub const CHROME_HEIGHT: u32 = 90;
#[cfg(not(target_os = "linux"))]
pub const TOOLS_HEIGHT: u32 = 132;

#[cfg(not(target_os = "linux"))]
pub fn bounds(window: &Window, y: u32, height: Option<u32>) -> Rect {
    let size = window.inner_size().to_logical::<u32>(window.scale_factor());
    Rect {
        position: LogicalPosition::new(0, y).into(),
        size: LogicalSize::new(
            size.width.max(1),
            height.unwrap_or(size.height.saturating_sub(y)).max(1),
        )
        .into(),
    }
}

#[cfg(not(target_os = "linux"))]
pub fn build(builder: WebViewBuilder<'_>, window: &Window) -> wry::Result<WebView> {
    builder.build_as_child(window)
}

#[cfg(target_os = "linux")]
pub fn build(builder: WebViewBuilder<'_>, parent: &gtk::Box) -> wry::Result<WebView> {
    use wry::WebViewBuilderExtUnix;
    builder.build_gtk(parent)
}

// These shortcuts are selected from native keyboard events. Neither a remote
// JavaScript keyboard event nor the trusted chrome's DOM runs this mapping.
pub fn shortcut(key: ShortcutKey, shift: bool) -> Option<Command> {
    use ShortcutKey::*;
    match (key, shift) {
        (Letter('l'), false) => Some(Command::FocusAddress),
        (Letter('t'), false) => Some(Command::NewTab),
        (Letter('n'), true) => Some(Command::NewPrivate),
        (Letter('w'), false) => Some(Command::CloseTab),
        (Letter('r'), false) => Some(Command::Reload),
        (Letter('p'), false) => Some(Command::Print),
        (Letter('b'), true) => Some(Command::FocusPage),
        (LeftBracket, false) => Some(Command::Back),
        (RightBracket, false) => Some(Command::Forward),
        (Tab, false) => Some(Command::NextTab),
        (Period, false) => Some(Command::Stop),
        _ => None,
    }
}

pub enum ShortcutKey {
    Letter(char),
    LeftBracket,
    RightBracket,
    Tab,
    Period,
}

#[cfg(not(target_os = "macos"))]
pub fn send_shortcut(proxy: &EventLoopProxy<Signal>, popup: Option<u32>, command: Command) {
    let _ = proxy.send_event(Signal::Shortcut(popup, command));
}

pub fn related(features: NewWindowFeatures) -> WebViewBuilder<'static> {
    let builder = WebViewBuilder::new();
    #[cfg(target_os = "macos")]
    {
        use wry::WebViewBuilderExtMacos;
        builder.with_webview_configuration(features.opener.target_configuration)
    }
    #[cfg(target_os = "windows")]
    {
        use wry::WebViewBuilderExtWindows;
        builder.with_environment(features.opener.environment)
    }
    #[cfg(target_os = "linux")]
    {
        use wry::WebViewBuilderExtUnix;
        builder.with_related_view(features.opener.webview)
    }
}

pub fn new_window_response(view: &WebView) -> NewWindowResponse {
    #[cfg(target_os = "macos")]
    {
        use wry::WebViewExtMacOS;
        NewWindowResponse::Create {
            webview: view.webview().into_super(),
        }
    }
    #[cfg(target_os = "windows")]
    {
        use wry::WebViewExtWindows;
        NewWindowResponse::Create {
            webview: view.webview(),
        }
    }
    #[cfg(target_os = "linux")]
    {
        use wry::WebViewExtUnix;
        NewWindowResponse::Create {
            webview: view.webview(),
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn chrome_uri() -> &'static str {
    crate::policy::HOME
}

#[cfg(not(target_os = "windows"))]
pub fn load_chrome(view: &WebView) -> wry::Result<()> {
    view.load_html(crate::CHROME)
}
