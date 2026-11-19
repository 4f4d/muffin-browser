use super::{send_shortcut, shortcut, ShortcutKey};
use crate::{downloads::SharedDownloads, policy::Policy, Role, Signal};
use gtk::{gdk, glib, prelude::*};
use std::path::Path;
use tao::{event_loop::EventLoopProxy, platform::unix::WindowExtUnix, window::Window};
use webkit2gtk::{DownloadExt, SettingsExt, URIRequestExt, WebContextExt, WebViewExt};
use wry::{WebView, WebViewBuilder, WebViewExtUnix};

pub const CHROME_HEIGHT: u32 = 90;
pub const TOOLS_HEIGHT: u32 = 132;

pub fn new_box() -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Vertical, 0)
}

pub fn top_box(window: &Window) -> &gtk::Box {
    window.default_vbox().expect("TAO GTK container")
}

pub fn defaults(builder: WebViewBuilder<'_>) -> WebViewBuilder<'_> {
    builder
}

pub fn instrument_builder(
    builder: WebViewBuilder<'_>,
    _role: Role,
    _proxy: EventLoopProxy<Signal>,
) -> WebViewBuilder<'_> {
    builder
}

pub fn is_private(view: &WebView) -> bool {
    view.webview().is_ephemeral()
}

pub fn focus(_window: &Window, view: &WebView) {
    view.webview().grab_focus();
}

pub fn stop(view: &WebView) {
    view.webview().stop_loading();
}

pub fn audio(view: &WebView) -> Option<(bool, bool)> {
    let native = view.webview();
    Some((native.is_playing_audio(), native.is_muted()))
}

pub fn toggle_mute(view: &WebView) {
    let native = view.webview();
    native.set_is_muted(!native.is_muted());
}

pub fn install_keys(
    window: &Window,
    proxy: EventLoopProxy<Signal>,
    popup: Option<u32>,
) -> gtk::EventControllerKey {
    let keys = gtk::EventControllerKey::new(window.gtk_window());
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(move |_, key, _, flags| {
        use gdk::ModifierType as Mod;
        if !flags.contains(Mod::CONTROL_MASK) || flags.intersects(Mod::MOD1_MASK | Mod::SUPER_MASK)
        {
            return false;
        }
        let key = match char::from_u32(key) {
            Some(key) if key.is_ascii_alphabetic() => ShortcutKey::Letter(key.to_ascii_lowercase()),
            Some('[') => ShortcutKey::LeftBracket,
            Some(']') => ShortcutKey::RightBracket,
            Some('.') => ShortcutKey::Period,
            _ if key == *gdk::keys::constants::Tab => ShortcutKey::Tab,
            _ => return false,
        };
        if let Some(command) = shortcut(key, flags.contains(Mod::SHIFT_MASK)) {
            send_shortcut(&proxy, popup, command);
            true
        } else {
            false
        }
    });
    keys
}

pub struct NativeHooks {
    _keys: Option<gtk::EventControllerKey>,
}

pub fn hooks(
    window: &Window,
    view: &WebView,
    role: Role,
    proxy: EventLoopProxy<Signal>,
    downloads: &SharedDownloads,
    policy: &Policy,
) -> Result<NativeHooks, &'static str> {
    let native = view.webview();
    if let Some(settings) = WebViewExt::settings(&native) {
        settings.set_allow_file_access_from_file_urls(false);
        settings.set_allow_universal_access_from_file_urls(false);
        settings.set_enable_developer_extras(false);
    }
    let terminated = proxy.clone();
    native.connect_web_process_terminated(move |_, _| {
        let _ = terminated.send_event(Signal::Terminated(role));
    });
    let focus = proxy.clone();
    native.connect_focus_in_event(move |_, _| {
        let _ = focus.send_event(Signal::ViewFocused(role));
        glib::Propagation::Proceed
    });
    native.connect_load_failed({
        let proxy = proxy.clone();
        move |_, _, _, _| {
            let _ = proxy.send_event(Signal::LoadError(role));
            false
        }
    });
    let enter = proxy.clone();
    native.connect_enter_fullscreen(move |_| {
        let _ = enter.send_event(Signal::Fullscreen(role, true));
        false
    });
    let leave = proxy.clone();
    native.connect_leave_fullscreen(move |_| {
        let _ = leave.send_event(Signal::Fullscreen(role, false));
        false
    });
    let keys = match role {
        Role::Chrome => Some(install_keys(window, proxy.clone(), None)),
        Role::Popup(id) => {
            let close = proxy.clone();
            native.connect_close(move |_| {
                let _ = close.send_event(Signal::PopupClose(id));
            });
            Some(install_keys(window, proxy.clone(), Some(id)))
        }
        Role::Tab(_) => None,
    };
    let context = native.context().ok_or("missing WebKitGTK context")?;
    // Download events belong to the context, including downloads from related
    // views. Install one policy per context. The GTK signal owns its lifetime.
    let key = gtk::glib::Quark::from_str("muffin-validation-downloads");
    if unsafe { context.qdata::<()>(key) }.is_none() {
        unsafe { context.set_qdata(key, ()) };
        let downloads = downloads.clone();
        let policy = policy.clone();
        context.connect_download_started(move |_, operation| {
            if role == Role::Chrome {
                operation.cancel();
                return;
            }
            let id = std::rc::Rc::new(std::cell::Cell::new(None));
            operation.connect_decide_destination({
                let downloads = downloads.clone();
                let policy = policy.clone();
                let id = id.clone();
                move |operation, filename| {
                    let started = operation
                        .request()
                        .and_then(|request| request.uri())
                        .and_then(|uri| {
                            downloads
                                .borrow_mut()
                                .begin(&policy, &uri, Path::new(filename))
                        });
                    if let Some((number, path)) = started {
                        if let Ok(uri) = url::Url::from_file_path(&path) {
                            id.set(Some(number));
                            operation.set_allow_overwrite(false);
                            operation.set_destination(uri.as_str());
                            let weak = operation.downgrade();
                            downloads.borrow_mut().track(
                                number,
                                Box::new(move || {
                                    if let Some(operation) = weak.upgrade() {
                                        operation.cancel();
                                    }
                                }),
                            );
                        } else {
                            downloads.borrow_mut().complete(number, false);
                            operation.cancel();
                        }
                    } else {
                        operation.cancel();
                    }
                    true
                }
            });
            operation.connect_received_data({
                let downloads = downloads.clone();
                let id = id.clone();
                move |operation, _| {
                    if let Some(id) = id.get() {
                        downloads
                            .borrow_mut()
                            .progress(id, operation.received_data_length());
                    }
                }
            });
            operation.connect_failed({
                let downloads = downloads.clone();
                let id = id.clone();
                move |_, _| {
                    if let Some(id) = id.take() {
                        downloads.borrow_mut().complete(id, false);
                    }
                }
            });
            operation.connect_finished({
                let downloads = downloads.clone();
                move |_| {
                    if let Some(id) = id.take() {
                        downloads.borrow_mut().complete(id, true);
                    }
                }
            });
        });
    }
    Ok(NativeHooks { _keys: keys })
}
