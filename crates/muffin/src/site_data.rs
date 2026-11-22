//! Site data operations for the selected WebKit data store.

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2_foundation::NSDate;
use objc2_web_kit::WKWebsiteDataStore;
use std::{cell::RefCell, rc::Rc};

/// Ask WebKit to remove every website data type from this store.
///
/// The completion callback runs when WebKit's removal operation completes.
/// The helper does not report which website records or data types existed.
pub fn clear_all(
    store: &WKWebsiteDataStore,
    main_thread: MainThreadMarker,
    completed: impl FnOnce() + 'static,
) {
    let data_types = unsafe { WKWebsiteDataStore::allWebsiteDataTypes(main_thread) };
    let earliest_date = NSDate::distantPast();
    let completed = Rc::new(RefCell::new(Some(completed)));
    let completion = RcBlock::new(move || {
        if let Some(completed) = completed.borrow_mut().take() {
            completed();
        }
    });

    unsafe {
        store.removeDataOfTypes_modifiedSince_completionHandler(
            &data_types,
            &earliest_date,
            &completion,
        );
    }
}
