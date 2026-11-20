//! The macOS download path holds WebKit's original request until the user
//! chooses a destination in Muffin's trusted Downloads surface.

use crate::downloads::{Downloads, MAX_FILE_BYTES};
use block2::{DynBlock, RcBlock};
use objc2::{
    define_class, msg_send, rc::Retained, runtime::ProtocolObject, DeclaredClass, MainThreadOnly,
};
use objc2_foundation::{
    MainThreadMarker, NSData, NSError, NSHTTPURLResponse, NSObject, NSObjectProtocol, NSString,
    NSURLRequest, NSURLResponse, NSURL,
};
use objc2_web_kit::{
    WKDownload, WKDownloadDelegate, WKDownloadRedirectPolicy, WKNavigation, WKNavigationAction,
    WKNavigationActionPolicy, WKNavigationDelegate, WKNavigationResponse,
    WKNavigationResponsePolicy, WKWebView,
};
use std::{
    cell::RefCell,
    collections::HashMap,
    path::{Path, PathBuf},
    ptr,
    rc::Rc,
    time::{Duration, Instant},
};

const MAX_TRANSFERS_PER_TAB: usize = 2;
const APPROVAL_TIMEOUT: Duration = Duration::from_secs(60);
type Destination = RcBlock<dyn Fn(*mut NSURL)>;

struct Transfer {
    download: Retained<WKDownload>,
    phase: Phase,
    expires: Instant,
}

enum Phase {
    WaitingForResponse,
    AwaitingApproval { id: u32, destination: Destination },
    Saving { id: u32, staged_path: PathBuf },
}

struct TransferState {
    store: Rc<RefCell<Downloads>>,
    entries: RefCell<HashMap<usize, Transfer>>,
    changed: Rc<dyn Fn()>,
}

impl TransferState {
    fn adopt(&self, download: &WKDownload, delegate: &TransferDelegate) {
        let key = identity(download);
        let entries = self.entries.borrow();
        if entries.contains_key(&key) {
            return;
        }
        let at_capacity = entries.len() >= MAX_TRANSFERS_PER_TAB;
        drop(entries);
        if at_capacity {
            unsafe { download.cancel(None) };
            return;
        }
        let Some(download) = (unsafe { Retained::retain(ptr::from_ref(download).cast_mut()) })
        else {
            return;
        };
        unsafe { download.setDelegate(Some(ProtocolObject::from_ref(delegate))) };
        self.entries.borrow_mut().insert(
            key,
            Transfer {
                download,
                phase: Phase::WaitingForResponse,
                expires: Instant::now() + APPROVAL_TIMEOUT,
            },
        );
    }

    fn offer(
        &self,
        download: &WKDownload,
        response: &NSURLResponse,
        suggested: &NSString,
        destination: &DynBlock<dyn Fn(*mut NSURL)>,
    ) {
        let key = identity(download);
        let mut entries = self.entries.borrow_mut();
        let eligible = entries.get(&key).is_some_and(|entry| {
            matches!(entry.phase, Phase::WaitingForResponse) && entry.expires >= Instant::now()
        });
        let final_url = response.URL();
        let response_url = http_url(final_url.as_deref());
        let expected = response.expectedContentLength();
        let id = response_url
            .filter(|_| eligible && expected <= MAX_FILE_BYTES as i64)
            .and_then(|url| {
                self.store
                    .borrow_mut()
                    .offer(&url, Path::new(&suggested.to_string()))
            });
        if let (Some(transfer), Some(id)) = (entries.get_mut(&key), id) {
            transfer.phase = Phase::AwaitingApproval {
                id,
                destination: destination.copy(),
            };
            transfer.expires = Instant::now() + APPROVAL_TIMEOUT;
            drop(entries);
            (self.changed)();
            return;
        }
        drop(entries);
        destination.call((ptr::null_mut(),));
        self.complete(download, false);
    }

    fn decide(&self, id: u32, approve: bool) -> bool {
        // Move the completion block out before calling WebKit. It can call the
        // delegate again while resolving a cancelled destination.
        let mut entries = self.entries.borrow_mut();
        let Some(key) = entries
            .iter()
            .find_map(|(key, transfer)| match &transfer.phase {
                Phase::AwaitingApproval { id: current, .. } if *current == id => Some(*key),
                _ => None,
            })
        else {
            return false;
        };
        let mut transfer = entries.remove(&key).expect("download is still present");
        let Phase::AwaitingApproval { destination, .. } = &transfer.phase else {
            unreachable!("selected download must be awaiting approval");
        };
        let destination = destination.clone();
        let approved = approve && transfer.expires >= Instant::now();
        let target = self
            .destination_for(id, approved)
            .map(|(target, staged_path)| {
                transfer.phase = Phase::Saving { id, staged_path };
                entries.insert(key, transfer);
                target
            });
        drop(entries);
        destination.call((target
            .as_ref()
            .map_or(ptr::null_mut(), |url| Retained::as_ptr(url).cast_mut()),));
        (self.changed)();
        true
    }

    fn destination_for(&self, id: u32, approve: bool) -> Option<(Retained<NSURL>, PathBuf)> {
        let mut store = self.store.borrow_mut();
        if !approve {
            store.dismiss(id);
            return None;
        }
        let target = store.approve(id)?;
        let Some(path) = target.to_str() else {
            store.finish(id, false);
            return None;
        };
        let destination = NSURL::fileURLWithPath_isDirectory(&NSString::from_str(path), false);
        Some((destination, target))
    }

    fn complete(&self, download: &WKDownload, success: bool) {
        let transfer = self.entries.borrow_mut().remove(&identity(download));
        let Some(transfer) = transfer else { return };
        match transfer.phase {
            Phase::AwaitingApproval { id, destination } => {
                self.store.borrow_mut().dismiss(id);
                destination.call((ptr::null_mut(),));
            }
            Phase::Saving { id, .. } => self.store.borrow_mut().finish(id, success),
            Phase::WaitingForResponse => return,
        }
        (self.changed)();
    }

    fn poll(&self) {
        let now = Instant::now();
        let stopped: Vec<_> = self
            .entries
            .borrow()
            .iter()
            .filter(|(_, transfer)| match &transfer.phase {
                Phase::Saving { staged_path, .. } => staged_path
                    .metadata()
                    .is_ok_and(|meta| meta.len() > MAX_FILE_BYTES),
                _ => transfer.expires < now,
            })
            .map(|(identity, _)| *identity)
            .collect();
        for key in stopped {
            let transfer = self.entries.borrow_mut().remove(&key);
            if let Some(transfer) = transfer {
                self.cancel(transfer);
            }
        }
    }

    fn close(&self) {
        let active = std::mem::take(&mut *self.entries.borrow_mut());
        for transfer in active.into_values() {
            self.cancel(transfer);
        }
    }

    fn cancel(&self, transfer: Transfer) {
        match transfer.phase {
            Phase::AwaitingApproval { id, destination } => {
                self.store.borrow_mut().dismiss(id);
                destination.call((ptr::null_mut(),));
                (self.changed)();
            }
            Phase::Saving { id, .. } => {
                let store = Rc::clone(&self.store);
                let changed = Rc::clone(&self.changed);
                let completion = RcBlock::new(move |_: *mut NSData| {
                    store.borrow_mut().finish(id, false);
                    changed();
                });
                unsafe { transfer.download.cancel(Some(&completion)) };
            }
            Phase::WaitingForResponse => unsafe { transfer.download.cancel(None) },
        }
    }
}

fn identity(download: &WKDownload) -> usize {
    ptr::from_ref(download).addr()
}

fn http_url(url: Option<&NSURL>) -> Option<url::Url> {
    let address = url?.absoluteString()?.to_string();
    let parsed = url::Url::parse(&address).ok()?;
    (matches!(parsed.scheme(), "http" | "https")
        && parsed.has_host()
        && parsed.username().is_empty()
        && parsed.password().is_none())
    .then_some(parsed)
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = Rc<TransferState>]
    struct TransferDelegate;

    unsafe impl NSObjectProtocol for TransferDelegate {}

    unsafe impl WKDownloadDelegate for TransferDelegate {
        #[unsafe(method(download:decideDestinationUsingResponse:suggestedFilename:completionHandler:))]
        fn destination(&self, download: &WKDownload, response: &NSURLResponse, name: &NSString, handler: &DynBlock<dyn Fn(*mut NSURL)>) {
            self.ivars().offer(download, response, name, handler);
        }

        #[unsafe(method(download:willPerformHTTPRedirection:newRequest:decisionHandler:))]
        fn redirect(&self, _: &WKDownload, _: &NSHTTPURLResponse, request: &NSURLRequest, handler: &DynBlock<dyn Fn(WKDownloadRedirectPolicy)>) {
            let target = request.URL();
            handler.call((if http_url(target.as_deref()).is_some() {
                WKDownloadRedirectPolicy::Allow
            } else {
                WKDownloadRedirectPolicy::Cancel
            },));
        }

        #[unsafe(method(downloadDidFinish:))]
        fn finish(&self, download: &WKDownload) {
            self.ivars().complete(download, true);
        }

        #[unsafe(method(download:didFailWithError:resumeData:))]
        fn fail(&self, download: &WKDownload, _: &NSError, _: Option<&NSData>) {
            self.ivars().complete(download, false);
        }
    }
);

struct NavigationState {
    wry: Retained<ProtocolObject<dyn WKNavigationDelegate>>,
    downloads: Retained<TransferDelegate>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = NavigationState]
    struct NavigationDelegate;

    unsafe impl NSObjectProtocol for NavigationDelegate {}

    unsafe impl WKNavigationDelegate for NavigationDelegate {
        #[unsafe(method(webView:decidePolicyForNavigationAction:decisionHandler:))]
        fn action(
            &self,
            view: &WKWebView,
            action: &WKNavigationAction,
            handler: &DynBlock<dyn Fn(WKNavigationActionPolicy)>,
        ) {
            let download = unsafe {
                action.respondsToSelector(objc2::sel!(shouldPerformDownload))
                    && action.shouldPerformDownload()
            };
            if download {
                let request = unsafe { action.request() };
                let address = request.URL();
                let main_frame = unsafe {
                    action
                        .sourceFrame()
                        .is_some_and(|frame| frame.isMainFrame())
                };
                handler.call((if main_frame && http_url(address.as_deref()).is_some() {
                    WKNavigationActionPolicy::Download
                } else {
                    WKNavigationActionPolicy::Cancel
                },));
                return;
            }
            unsafe {
                self.ivars()
                    .wry
                    .webView_decidePolicyForNavigationAction_decisionHandler(view, action, handler)
            };
        }

        #[unsafe(method(webView:decidePolicyForNavigationResponse:decisionHandler:))]
        fn response(
            &self,
            _: &WKWebView,
            response: &WKNavigationResponse,
            handler: &DynBlock<dyn Fn(WKNavigationResponsePolicy)>,
        ) {
            let native = unsafe { response.response() };
            let attachment = native
                .clone()
                .downcast::<NSHTTPURLResponse>()
                .ok()
                .and_then(|http| {
                    http.valueForHTTPHeaderField(&NSString::from_str("Content-Disposition"))
                })
                .is_some_and(|value| {
                    value
                        .to_string()
                        .split(';')
                        .next()
                        .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("attachment"))
                });
            if attachment || !unsafe { response.canShowMIMEType() } {
                let address = native.URL();
                let allowed =
                    unsafe { response.isForMainFrame() } && http_url(address.as_deref()).is_some();
                handler.call((if allowed {
                    WKNavigationResponsePolicy::Download
                } else {
                    WKNavigationResponsePolicy::Cancel
                },));
                return;
            }
            handler.call((WKNavigationResponsePolicy::Allow,));
        }

        #[unsafe(method(webView:navigationAction:didBecomeDownload:))]
        fn action_download(&self, _: &WKWebView, _: &WKNavigationAction, download: &WKDownload) {
            let delegate = &self.ivars().downloads;
            delegate.ivars().adopt(download, delegate);
        }

        #[unsafe(method(webView:navigationResponse:didBecomeDownload:))]
        fn response_download(
            &self,
            _: &WKWebView,
            _: &WKNavigationResponse,
            download: &WKDownload,
        ) {
            let delegate = &self.ivars().downloads;
            delegate.ivars().adopt(download, delegate);
        }

        #[unsafe(method(webView:didCommitNavigation:))]
        fn committed(&self, view: &WKWebView, navigation: Option<&WKNavigation>) {
            unsafe {
                self.ivars()
                    .wry
                    .webView_didCommitNavigation(view, navigation)
            };
        }

        #[unsafe(method(webView:didFinishNavigation:))]
        fn finished(&self, view: &WKWebView, navigation: Option<&WKNavigation>) {
            unsafe {
                self.ivars()
                    .wry
                    .webView_didFinishNavigation(view, navigation)
            };
        }

        #[unsafe(method(webViewWebContentProcessDidTerminate:))]
        fn terminated(&self, view: &WKWebView) {
            unsafe { self.ivars().wry.webViewWebContentProcessDidTerminate(view) };
        }
    }
);

pub struct TabDownloads {
    _navigation: Retained<NavigationDelegate>,
    state: Rc<TransferState>,
}

impl TabDownloads {
    pub fn install(
        view: &WKWebView,
        store: Rc<RefCell<Downloads>>,
        changed: impl Fn() + 'static,
    ) -> Option<Self> {
        let main_thread = MainThreadMarker::new()?;
        if !view.respondsToSelector(objc2::sel!(startDownloadUsingRequest:completionHandler:)) {
            return None;
        }
        let wry = unsafe { view.navigationDelegate() }?;
        let state = Rc::new(TransferState {
            store,
            entries: RefCell::new(HashMap::new()),
            changed: Rc::new(changed),
        });
        let downloads = main_thread
            .alloc::<TransferDelegate>()
            .set_ivars(Rc::clone(&state));
        let downloads = unsafe { msg_send![super(downloads), init] };
        let navigation = main_thread
            .alloc::<NavigationDelegate>()
            .set_ivars(NavigationState { wry, downloads });
        let navigation: Retained<NavigationDelegate> =
            unsafe { msg_send![super(navigation), init] };
        unsafe { view.setNavigationDelegate(Some(ProtocolObject::from_ref(&*navigation))) };
        Some(Self {
            _navigation: navigation,
            state,
        })
    }

    pub fn decide(&self, id: u32, approve: bool) -> bool {
        self.state.decide(id, approve)
    }

    pub fn poll(&self) {
        self.state.poll();
    }

    pub fn has_transfers(&self) -> bool {
        !self.state.entries.borrow().is_empty()
    }
}

impl Drop for TabDownloads {
    fn drop(&mut self) {
        self.state.close();
    }
}
