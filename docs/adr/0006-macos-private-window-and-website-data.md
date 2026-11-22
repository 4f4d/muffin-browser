# ADR 0006 — macOS private windows and website data

**Status** Implemented for M5 development verification  
**Date** 2026-11-23

## Decision

Each private window owns a nonpersistent WebKit website data store. Tabs and in-window popups use that window’s store. Another private window receives a fresh store. Regular windows share the regular WebKit store. The debug verifier uses a disposable regular store to protect the owner’s data. [Apple data store configuration](https://developer.apple.com/documentation/webkit/wkwebviewconfiguration/websitedatastore?changes=_9), [native window and store](../../crates/muffin/src/main.rs)

Private windows do not read history, bookmarks, shortcuts or suggestions from the regular profile. They read the user’s appearance and search preferences. Private visits and session tabs do not enter the regular profile or session file. External URLs submitted through the OS or second-instance channel open in a regular window. Page-created tabs stay with their opener window. [Profile, session and routing](../../crates/muffin/src/main.rs)

Private Downloads uses the folder preference in effect when the window opens and keeps its transfer rows in memory. A native Save decision can still write a file. A finished file and any system quarantine provenance can outlive the private window. Muffin leaves operating-system diagnostics, routing and file metadata to their respective systems. [Download store](../../crates/muffin/src/downloads.rs), [download consent](0004-macos-download-consent.md)

Regular Settings offers an explicit confirmation before clearing cookies, caches and other website data from the regular WebKit store. This action can sign users out. An open page can create website data again. Regular browser history and bookmarks have their own controls. The completion notice reports that WebKit finished the operation. Private website stores remain independent and close with their windows. [Apple removal API](https://developer.apple.com/documentation/webkit/wkwebsitedatastore/removedata%28oftypes%3Amodifiedsince%3Acompletionhandler%3A%29?language=swift), [WebKit adapter](../../crates/muffin/src/site_data.rs)

## Verification and revisit triggers

The loopback verifier compares cookies, localStorage and Cache Storage between regular and private views. It checks private history and shortcut absence against SQLite, regular session restoration, separate download rows and continued file presence after explicit Save. It also checks a site-data clear while a private view stays open. Revisit the design when adding a sync mechanism, a shared private session, new profile databases or another platform runtime. [Native fixture](../../tests/native/run_smoke.py)
