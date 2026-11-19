use super::{shortcut, ShortcutKey};
use crate::{Role, Signal};
use block2::RcBlock;
use objc2::{
    class, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, Bool, NSObject, ProtocolObject},
    DefinedClass, MainThreadOnly,
};
use objc2_foundation::{MainThreadMarker, NSObjectProtocol};
use objc2_web_kit::{WKUIDelegate, WKWebView};
use std::{ffi::CStr, ptr};
use tao::{event_loop::EventLoopProxy, platform::macos::WindowExtMacOS, window::Window};
use wry::{WebView, WebViewBuilder, WebViewBuilderExtDarwin, WebViewExtMacOS};

pub fn defaults(builder: WebViewBuilder<'_>) -> WebViewBuilder<'_> {
    builder.with_allow_link_preview(false)
}

pub fn instrument_builder(
    builder: WebViewBuilder<'_>,
    role: Role,
    proxy: EventLoopProxy<Signal>,
) -> WebViewBuilder<'_> {
    builder.with_on_web_content_process_terminate_handler(move || {
        let _ = proxy.send_event(Signal::Terminated(role));
    })
}

pub fn is_private(view: &WebView) -> bool {
    // All WebKit calls here use the live view on AppKit's main thread.
    unsafe {
        !view
            .webview()
            .configuration()
            .websiteDataStore()
            .isPersistent()
    }
}

pub fn focus(window: &Window, view: &WebView) {
    let native = window.ns_window().cast::<AnyObject>();
    let target = view.webview();
    let view_ptr = ptr::from_ref(&*target).cast::<AnyObject>();
    // The window and view are retained by TAO and WRY for this main-thread call.
    unsafe {
        let _: Bool = msg_send![native, makeFirstResponder: view_ptr];
    }
}

pub fn stop(view: &WebView) {
    unsafe { view.webview().stopLoading() };
}

pub fn audio(_view: &WebView) -> Option<(bool, bool)> {
    None
}

pub fn toggle_mute(_view: &WebView) {}

pub fn install_keys(proxy: EventLoopProxy<Signal>) -> Retained<AnyObject> {
    let monitor = RcBlock::new(move |event: *mut AnyObject| -> *mut AnyObject {
        // AppKit passes a retained-for-dispatch NSEvent on its main thread.
        unsafe {
            let flags: usize = msg_send![event, modifierFlags];
            const SHIFT: usize = 1 << 17;
            const CONTROL: usize = 1 << 18;
            const OPTION: usize = 1 << 19;
            const COMMAND: usize = 1 << 20;
            if flags & (CONTROL | OPTION | COMMAND) != COMMAND {
                return event;
            }
            let chars: *mut AnyObject = msg_send![event, charactersIgnoringModifiers];
            let bytes: *const i8 = msg_send![chars, UTF8String];
            if bytes.is_null() {
                return event;
            }
            let key = match CStr::from_ptr(bytes).to_bytes() {
                [key] if key.is_ascii_alphabetic() => {
                    ShortcutKey::Letter(key.to_ascii_lowercase().into())
                }
                b"[" => ShortcutKey::LeftBracket,
                b"]" => ShortcutKey::RightBracket,
                b"." => ShortcutKey::Period,
                b"\t" => ShortcutKey::Tab,
                _ => return event,
            };
            if let Some(command) = shortcut(key, flags & SHIFT != 0) {
                let window: *mut AnyObject = msg_send![event, window];
                let number: isize = msg_send![window, windowNumber];
                let _ = proxy.send_event(Signal::AppKitShortcut(number, command));
                return ptr::null_mut();
            }
            event
        }
    });
    let monitor: *mut AnyObject = unsafe {
        msg_send![class!(NSEvent), addLocalMonitorForEventsMatchingMask: 1usize << 10, handler: &*monitor]
    };
    unsafe { Retained::retain(monitor) }.expect("AppKit local key monitor")
}

pub fn window_number(window: &Window) -> isize {
    unsafe { msg_send![window.ns_window().cast::<AnyObject>(), windowNumber] }
}

pub struct PopupDelegateState {
    proxy: EventLoopProxy<Signal>,
    id: u32,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = PopupDelegateState]
    pub struct PopupDelegate;

    unsafe impl NSObjectProtocol for PopupDelegate {}

    unsafe impl WKUIDelegate for PopupDelegate {
        #[unsafe(method(webViewDidClose:))]
        fn web_view_did_close(&self, _: &WKWebView) {
            let state = self.ivars();
            let _ = state.proxy.send_event(Signal::PopupClose(state.id));
        }
    }
);

pub struct NativeHooks {
    _popup_delegate: Option<Retained<PopupDelegate>>,
}

pub fn hooks(
    _window: &Window,
    view: &WebView,
    role: Role,
    proxy: EventLoopProxy<Signal>,
    _downloads: &crate::downloads::SharedDownloads,
    _policy: &crate::policy::Policy,
) -> Result<NativeHooks, &'static str> {
    let popup = if let Role::Popup(id) = role {
        let marker = MainThreadMarker::new().ok_or("not on AppKit main thread")?;
        // This popup has no nested windows or privileged callbacks. Retain the
        // delegate alongside the view because WKWebView only holds it weakly.
        let allocated = marker
            .alloc::<PopupDelegate>()
            .set_ivars(PopupDelegateState { proxy, id });
        let delegate: Retained<PopupDelegate> = unsafe { msg_send![super(allocated), init] };
        unsafe {
            view.webview()
                .setUIDelegate(Some(ProtocolObject::from_ref(&*delegate)))
        };
        Some(delegate)
    } else {
        None
    };
    Ok(NativeHooks {
        _popup_delegate: popup,
    })
}
