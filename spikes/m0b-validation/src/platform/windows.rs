use super::{send_shortcut, shortcut, ShortcutKey};
use crate::{downloads::SharedDownloads, policy::Policy, Role, Signal};
use std::path::PathBuf;
use tao::{event_loop::EventLoopProxy, window::Window};
use webview2_com::{
    take_pwstr, AcceleratorKeyPressedEventHandler, BytesReceivedChangedEventHandler,
    ContainsFullScreenElementChangedEventHandler, DownloadStartingEventHandler,
    FocusChangedEventHandler, LaunchingExternalUriSchemeEventHandler,
    Microsoft::Web::WebView2::Win32::*, NavigationCompletedEventHandler, ProcessFailedEventHandler,
    StateChangedEventHandler, WindowCloseRequestedEventHandler,
};
use windows::{
    core::{Interface, HSTRING, PWSTR},
    Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_MENU, VK_SHIFT},
};
use wry::{WebView, WebViewBuilder, WebViewBuilderExtWindows, WebViewExtWindows};

pub fn defaults(builder: WebViewBuilder<'_>) -> WebViewBuilder<'_> {
    builder
        .with_additional_browser_args("")
        .with_browser_accelerator_keys(false)
}

pub fn instrument_builder(
    builder: WebViewBuilder<'_>,
    _role: Role,
    _proxy: EventLoopProxy<Signal>,
) -> WebViewBuilder<'_> {
    builder
}

pub fn is_private(view: &WebView) -> bool {
    unsafe {
        let Ok(core) = view.webview().cast::<ICoreWebView2_13>() else {
            return false;
        };
        let Ok(profile) = core.Profile() else {
            return false;
        };
        let mut private = Default::default();
        profile.IsInPrivateModeEnabled(&mut private).is_ok() && private.as_bool()
    }
}

pub fn focus(_window: &Window, view: &WebView) {
    let _ = view.focus();
}

pub fn stop(view: &WebView) {
    let _ = unsafe { view.webview().Stop() };
}

pub fn audio(view: &WebView) -> Option<(bool, bool)> {
    let core = view.webview().cast::<ICoreWebView2_8>().ok()?;
    let (mut playing, mut muted) = (Default::default(), Default::default());
    unsafe {
        core.IsDocumentPlayingAudio(&mut playing).ok()?;
        core.IsMuted(&mut muted).ok()?;
    }
    Some((playing.as_bool(), muted.as_bool()))
}

pub fn toggle_mute(view: &WebView) {
    if let (Some((_, muted)), Ok(core)) = (audio(view), view.webview().cast::<ICoreWebView2_8>()) {
        let _ = unsafe { core.SetIsMuted(!muted) };
    }
}

pub struct NativeHooks;

pub fn hooks(
    _window: &Window,
    view: &WebView,
    role: Role,
    proxy: EventLoopProxy<Signal>,
    downloads: &SharedDownloads,
    policy: &Policy,
) -> Result<NativeHooks, &'static str> {
    hooks_result(view, role, proxy, downloads, policy)
        .map_err(|_| "WebView2 native adapter initialization")
}

fn hooks_result(
    view: &WebView,
    role: Role,
    proxy: EventLoopProxy<Signal>,
    downloads: &SharedDownloads,
    policy: &Policy,
) -> windows::core::Result<NativeHooks> {
    let controller = view.controller();
    let webview = view.webview();
    let popup = if let Role::Popup(id) = role {
        Some(id)
    } else {
        None
    };
    // COM interfaces are retained by WRY and only accessed on the STA window thread.
    unsafe {
        webview.Settings()?.SetIsWebMessageEnabled(false)?;
        webview.Settings()?.SetAreHostObjectsAllowed(false)?;
        webview
            .cast::<ICoreWebView2_18>()?
            .add_LaunchingExternalUriScheme(
                &LaunchingExternalUriSchemeEventHandler::create(Box::new(|_, args| {
                    if let Some(args) = args {
                        args.SetCancel(true)?;
                    }
                    Ok(())
                })),
                &mut Default::default(),
            )?;
        controller.add_AcceleratorKeyPressed(
            &AcceleratorKeyPressedEventHandler::create(Box::new({
                let proxy = proxy.clone();
                move |_, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };
                    let mut kind = Default::default();
                    args.KeyEventKind(&mut kind)?;
                    if kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN
                        && kind != COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN
                    {
                        return Ok(());
                    }
                    if GetKeyState(VK_CONTROL.0.into()) >= 0 || GetKeyState(VK_MENU.0.into()) < 0 {
                        return Ok(());
                    }
                    let mut virtual_key = 0;
                    args.VirtualKey(&mut virtual_key)?;
                    let key = match virtual_key {
                        65..=90 => {
                            ShortcutKey::Letter(char::from(virtual_key as u8).to_ascii_lowercase())
                        }
                        9 => ShortcutKey::Tab,
                        0xdb => ShortcutKey::LeftBracket,
                        0xdd => ShortcutKey::RightBracket,
                        0xbe => ShortcutKey::Period,
                        _ => return Ok(()),
                    };
                    if let Some(command) = shortcut(key, GetKeyState(VK_SHIFT.0.into()) < 0) {
                        args.SetHandled(true)?;
                        send_shortcut(&proxy, popup, command);
                    }
                    Ok(())
                }
            })),
            &mut Default::default(),
        )?;
        controller.add_GotFocus(
            &FocusChangedEventHandler::create(Box::new({
                let proxy = proxy.clone();
                move |_, _| {
                    let _ = proxy.send_event(Signal::ViewFocused(role));
                    Ok(())
                }
            })),
            &mut Default::default(),
        )?;
        webview.add_NavigationCompleted(
            &NavigationCompletedEventHandler::create(Box::new({
                let proxy = proxy.clone();
                move |_, args| {
                    if let Some(args) = args {
                        let mut success = Default::default();
                        args.IsSuccess(&mut success)?;
                        if !success.as_bool() {
                            let _ = proxy.send_event(Signal::LoadError(role));
                        }
                    }
                    Ok(())
                }
            })),
            &mut Default::default(),
        )?;
        webview.add_ProcessFailed(
            &ProcessFailedEventHandler::create(Box::new({
                let proxy = proxy.clone();
                move |_, _| {
                    let _ = proxy.send_event(Signal::Terminated(role));
                    Ok(())
                }
            })),
            &mut Default::default(),
        )?;
        webview.add_ContainsFullScreenElementChanged(
            &ContainsFullScreenElementChangedEventHandler::create(Box::new({
                let proxy = proxy.clone();
                move |webview, _| {
                    if let Some(webview) = webview {
                        let mut fullscreen = Default::default();
                        webview.ContainsFullScreenElement(&mut fullscreen)?;
                        let _ = proxy.send_event(Signal::Fullscreen(role, fullscreen.as_bool()));
                    }
                    Ok(())
                }
            })),
            &mut Default::default(),
        )?;
        if let Some(id) = popup {
            webview.add_WindowCloseRequested(
                &WindowCloseRequestedEventHandler::create(Box::new(move |_, _| {
                    let _ = proxy.send_event(Signal::PopupClose(id));
                    Ok(())
                })),
                &mut Default::default(),
            )?;
        }
        let downloads = downloads.clone();
        let policy = policy.clone();
        webview.cast::<ICoreWebView2_4>()?.add_DownloadStarting(
            &DownloadStartingEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                // Start canceled, only clear after a safe destination and hooks are installed.
                args.SetCancel(true)?;
                args.SetHandled(true)?;
                if role == Role::Chrome {
                    return Ok(());
                }
                let operation = args.DownloadOperation()?;
                let mut address = PWSTR::null();
                operation.Uri(&mut address)?;
                let mut suggested = PWSTR::null();
                args.ResultFilePath(&mut suggested)?;
                let suggested = PathBuf::from(take_pwstr(suggested));
                let started =
                    downloads
                        .borrow_mut()
                        .begin(&policy, &take_pwstr(address), &suggested);
                let Some((id, path)) = started else {
                    return Ok(());
                };
                let result = (|| {
                    operation.add_BytesReceivedChanged(
                        &BytesReceivedChangedEventHandler::create(Box::new({
                            let downloads = downloads.clone();
                            move |operation, _| {
                                if let Some(operation) = operation {
                                    let mut bytes = 0;
                                    operation.BytesReceived(&mut bytes)?;
                                    downloads.borrow_mut().progress(id, bytes.max(0) as u64);
                                }
                                Ok(())
                            }
                        })),
                        &mut Default::default(),
                    )?;
                    operation.add_StateChanged(
                        &StateChangedEventHandler::create(Box::new({
                            let downloads = downloads.clone();
                            move |operation, _| {
                                if let Some(operation) = operation {
                                    let mut state = Default::default();
                                    operation.State(&mut state)?;
                                    if state != COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS {
                                        downloads.borrow_mut().complete(
                                            id,
                                            state == COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED,
                                        );
                                    }
                                }
                                Ok(())
                            }
                        })),
                        &mut Default::default(),
                    )?;
                    args.SetResultFilePath(&HSTRING::from(dunce::simplified(&path).as_os_str()))?;
                    args.SetCancel(false)?;
                    downloads.borrow_mut().track(
                        id,
                        Box::new(move || {
                            let _ = operation.Cancel();
                        }),
                    );
                    Ok(())
                })();
                if result.is_err() {
                    downloads.borrow_mut().complete(id, false);
                }
                result
            })),
            &mut Default::default(),
        )?;
    }
    Ok(NativeHooks)
}
