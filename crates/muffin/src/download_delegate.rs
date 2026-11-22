//! The macOS download path holds WebKit's original request until the user
//! chooses a destination in Muffin's trusted Downloads surface.

use crate::downloads::{Downloads, MAX_FILE_BYTES};
use block2::{DynBlock, RcBlock};
use objc2::{
    define_class, msg_send, rc::Retained, runtime::ProtocolObject, DeclaredClass, MainThreadOnly,
};
use objc2_foundation::{
    MainThreadMarker, NSData, NSError, NSHTTPURLResponse, NSMutableURLRequest, NSObject,
    NSObjectProtocol, NSString, NSURLRequest, NSURLResponse, NSURL,
};
use objc2_web_kit::{
    WKDownload, WKDownloadDelegate, WKDownloadRedirectPolicy, WKNavigation, WKNavigationAction,
    WKNavigationActionPolicy, WKNavigationDelegate, WKNavigationResponse,
    WKNavigationResponsePolicy, WKWebView,
};
use std::ptr::NonNull;
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
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
    retry_request: Option<Retained<NSURLRequest>>,
}

enum Phase {
    WaitingForResponse,
    AwaitingApproval { id: u32, destination: Destination },
    Saving { id: u32, staged_path: PathBuf },
}

struct TransferState {
    store: Rc<RefCell<Downloads>>,
    entries: RefCell<HashMap<usize, Transfer>>,
    retry_requests: RefCell<HashMap<u32, Retained<NSURLRequest>>>,
    closed: Cell<bool>,
    changed: Rc<dyn Fn()>,
}

impl TransferState {
    fn adopt(&self, download: &WKDownload, delegate: &TransferDelegate) {
        if self.closed.get() {
            unsafe { download.cancel(None) };
            return;
        }
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
        let retry_request = unsafe { download.originalRequest() }
            .and_then(|request| sanitized_retry_request(&request));
        self.entries.borrow_mut().insert(
            key,
            Transfer {
                download,
                phase: Phase::WaitingForResponse,
                expires: Instant::now() + APPROVAL_TIMEOUT,
                retry_request,
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
        let retry_request = transfer.retry_request;
        match transfer.phase {
            Phase::AwaitingApproval { id, destination } => {
                self.store.borrow_mut().dismiss(id);
                destination.call((ptr::null_mut(),));
            }
            Phase::Saving { id, .. } => {
                self.store.borrow_mut().finish(id, success);
                if !success {
                    if let Some(request) = retry_request {
                        if self.store.borrow_mut().mark_retryable(id, true) {
                            self.retry_requests.borrow_mut().insert(id, request);
                        }
                    }
                }
            }
            Phase::WaitingForResponse => return,
        }
        (self.changed)();
    }

    fn poll(&self) {
        let retryable = self
            .store
            .borrow()
            .items()
            .into_iter()
            .filter(|item| item.retryable)
            .map(|item| item.id)
            .collect::<HashSet<_>>();
        self.retry_requests
            .borrow_mut()
            .retain(|id, _| retryable.contains(id));
        let now = Instant::now();
        let mut changed = false;
        let stopped: Vec<_> = self
            .entries
            .borrow()
            .iter()
            .filter(|(_, transfer)| match &transfer.phase {
                Phase::Saving { id, staged_path } => {
                    let bytes = staged_path.metadata().map_or(0, |meta| meta.len());
                    if bytes > MAX_FILE_BYTES {
                        return true;
                    }
                    changed |= self.store.borrow_mut().update_progress(*id, bytes);
                    false
                }
                _ => transfer.expires < now,
            })
            .map(|(identity, _)| *identity)
            .collect();
        for key in stopped {
            let transfer = self.entries.borrow_mut().remove(&key);
            if let Some(transfer) = transfer {
                self.stop(transfer, false);
            }
        }
        if changed {
            (self.changed)();
        }
    }

    fn cancel_transfer(&self, id: u32) -> bool {
        let mut entries = self.entries.borrow_mut();
        let Some(key) = entries
            .iter()
            .find_map(|(key, transfer)| match &transfer.phase {
                Phase::Saving { id: current, .. } if *current == id => Some(*key),
                _ => None,
            })
        else {
            return false;
        };
        let transfer = entries.remove(&key).expect("download is still present");
        drop(entries);
        self.stop(transfer, true);
        true
    }

    fn close(&self) {
        self.closed.set(true);
        let retry_ids = self
            .retry_requests
            .borrow()
            .keys()
            .copied()
            .collect::<Vec<_>>();
        let had_retries = !retry_ids.is_empty();
        self.retry_requests.borrow_mut().clear();
        for id in retry_ids {
            self.store.borrow_mut().mark_retryable(id, false);
        }
        if had_retries {
            (self.changed)();
        }
        let active = std::mem::take(&mut *self.entries.borrow_mut());
        for transfer in active.into_values() {
            self.stop(transfer, false);
        }
    }

    fn stop(&self, transfer: Transfer, user_cancelled: bool) {
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
                    if user_cancelled {
                        store.borrow_mut().cancel_transfer(id);
                    } else {
                        store.borrow_mut().finish(id, false);
                    }
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

fn view_identity(view: &WKWebView) -> usize {
    ptr::from_ref(view).addr()
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

fn sanitized_retry_request(request: &NSURLRequest) -> Option<Retained<NSURLRequest>> {
    let url = http_url(request.URL().as_deref())?;
    let method = request.HTTPMethod()?.to_string();
    let has_body = request.HTTPBody().is_some();
    let has_body_stream = request.HTTPBodyStream().is_some();
    let eligible = replayable_request_parts(Some(url), &method, has_body, has_body_stream);
    if !eligible {
        return None;
    }

    // Construct a fresh request without any original headers, body, or cookie
    // handling. This keeps the URL while dropping credentials and custom data.
    let address = request.URL()?;
    let sanitized = NSMutableURLRequest::requestWithURL(&address);
    sanitized.setHTTPMethod(&NSString::from_str("GET"));
    sanitized.setAllHTTPHeaderFields(None);
    sanitized.setHTTPShouldHandleCookies(false);
    Some(sanitized.into_super())
}

fn replayable_request_parts(
    url: Option<url::Url>,
    method: &str,
    has_body: bool,
    has_body_stream: bool,
) -> bool {
    url.is_some_and(|url| {
        matches!(url.scheme(), "http" | "https")
            && url.has_host()
            && url.username().is_empty()
            && url.password().is_none()
    }) && method == "GET"
        && !has_body
        && !has_body_stream
}

fn sanitized_request_is_safe(request: &NSURLRequest) -> bool {
    let Some(url) = http_url(request.URL().as_deref()) else {
        return false;
    };
    request
        .HTTPMethod()
        .is_some_and(|method| method.to_string() == "GET")
        && request.HTTPBody().is_none()
        && request.HTTPBodyStream().is_none()
        && !request.HTTPShouldHandleCookies()
        && request
            .allHTTPHeaderFields()
            .is_none_or(|headers| headers.is_empty())
        && matches!(url.scheme(), "http" | "https")
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
    view_identity: usize,
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
            retry_requests: RefCell::new(HashMap::new()),
            closed: Cell::new(false),
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
            view_identity: view_identity(view),
        })
    }

    pub fn decide(&self, id: u32, approve: bool) -> bool {
        self.state.decide(id, approve)
    }

    pub fn cancel_transfer(&self, id: u32) -> bool {
        self.state.cancel_transfer(id)
    }

    pub fn retry(&self, view: &WKWebView, id: u32) -> bool {
        if self.state.closed.get() || self.view_identity != view_identity(view) {
            return false;
        }
        let Some(request) = self.state.retry_requests.borrow().get(&id).cloned() else {
            return false;
        };
        if !sanitized_request_is_safe(&request) || !self.state.store.borrow_mut().consume_retry(id)
        {
            return false;
        }
        self.state.retry_requests.borrow_mut().remove(&id);
        (self.state.changed)();

        let state = Rc::clone(&self.state);
        let delegate = self._navigation.ivars().downloads.clone();
        let completion = RcBlock::new(move |download: NonNull<WKDownload>| {
            state.adopt(unsafe { download.as_ref() }, &delegate);
        });
        unsafe { view.startDownloadUsingRequest_completionHandler(&request, &completion) };
        true
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

#[cfg(test)]
mod retry_tests {
    use super::{
        replayable_request_parts, sanitized_request_is_safe, sanitized_retry_request, TransferState,
    };
    use objc2_foundation::{NSMutableURLRequest, NSString, NSURLRequest, NSURL};
    use std::{
        cell::{Cell, RefCell},
        collections::HashMap,
        rc::Rc,
    };
    use url::Url;

    fn candidate(address: &str, method: &str, body: bool, stream: bool) -> bool {
        replayable_request_parts(Url::parse(address).ok(), method, body, stream)
    }

    #[test]
    fn retry_requires_body_free_http_get_without_url_credentials() {
        assert!(candidate("https://example.test/file", "GET", false, false));
        for (address, method, body, stream) in [
            ("file:///tmp/file", "GET", false, false),
            ("https://user@example.test/file", "GET", false, false),
            ("https://example.test/file", "POST", false, false),
            ("https://example.test/file", "GET", true, false),
            ("https://example.test/file", "GET", false, true),
        ] {
            assert!(!candidate(address, method, body, stream));
        }
    }

    #[test]
    fn retry_request_is_fresh_and_disables_cookie_and_header_replay() {
        let address = NSURL::URLWithString(&NSString::from_str("https://example.test/file"))
            .expect("valid fixture URL");
        let original = NSMutableURLRequest::requestWithURL(&address);
        original.setHTTPMethod(&NSString::from_str("GET"));
        original.setValue_forHTTPHeaderField(
            Some(&NSString::from_str("test-secret")),
            &NSString::from_str("Authorization"),
        );
        original.setValue_forHTTPHeaderField(
            Some(&NSString::from_str("test-cookie")),
            &NSString::from_str("Cookie"),
        );
        original.setHTTPShouldHandleCookies(true);
        assert!(original.HTTPShouldHandleCookies());
        let retry = sanitized_retry_request(&original).expect("safe GET is replayable");
        assert_eq!(retry.HTTPMethod().unwrap().to_string(), "GET");
        assert!(!retry.HTTPShouldHandleCookies());
        assert!(retry.HTTPBody().is_none());
        assert!(retry.HTTPBodyStream().is_none());
        assert!(retry
            .allHTTPHeaderFields()
            .is_none_or(|headers| headers.is_empty()));
        assert!(sanitized_request_is_safe(&retry));
        let _typed_request: &NSURLRequest = &retry;
    }

    #[test]
    fn closing_owner_invalidates_retry_token_and_row() {
        let directory = tempfile::tempdir().unwrap();
        let mut downloads = crate::downloads::Downloads::new(directory.path().join("downloads"));
        let id = downloads
            .offer(
                &Url::parse("https://example.test/file").unwrap(),
                std::path::Path::new("file"),
            )
            .unwrap();
        let staged = downloads.approve(id).unwrap();
        std::fs::write(staged, b"partial").unwrap();
        downloads.finish(id, false);
        downloads.mark_retryable(id, true);
        let address = NSURL::URLWithString(&NSString::from_str("https://example.test/file"))
            .expect("valid fixture URL");
        let request = NSMutableURLRequest::requestWithURL(&address).into_super();
        let state = TransferState {
            store: Rc::new(RefCell::new(downloads)),
            entries: RefCell::new(HashMap::new()),
            retry_requests: RefCell::new(HashMap::from([(id, request)])),
            closed: Cell::new(false),
            changed: Rc::new(|| {}),
        };
        state.close();
        assert!(state.closed.get());
        assert!(state.retry_requests.borrow().is_empty());
        assert!(!state.store.borrow().items()[0].retryable);
    }
}
