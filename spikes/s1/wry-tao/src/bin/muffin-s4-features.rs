//! Bounded local S4 download, storage-isolation, and PDF feasibility fixture.
fn main() {
    if let Err(error) = native::run() {
        eprintln!("fixture_error={error}");
        std::process::exit(1);
    }
}

mod native {
    use std::{cell::Cell, path::PathBuf, rc::Rc, time::Duration};
    use tao::{
        dpi::{LogicalPosition, LogicalSize},
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
        window::{Window, WindowBuilder},
    };
    #[cfg(target_os = "windows")]
    use wry::WebViewBuilderExtWindows;
    use wry::{http, PageLoadEvent, Rect, WebContext, WebView, WebViewBuilder};
    #[cfg(target_os = "linux")]
    use {gtk::prelude::*, tao::platform::unix::WindowExtUnix, wry::WebViewBuilderExtUnix};

    struct Parent {
        window: Window,
        #[cfg(target_os = "linux")]
        fixed: gtk::Fixed,
    }
    impl Parent {
        fn new(window: Window) -> Self {
            #[cfg(target_os = "linux")]
            let fixed = {
                let fixed = gtk::Fixed::new();
                window
                    .default_vbox()
                    .expect("TAO GTK box")
                    .pack_start(&fixed, true, true, 0);
                fixed.show();
                fixed
            };
            Self {
                window,
                #[cfg(target_os = "linux")]
                fixed,
            }
        }
        fn build(&self, builder: WebViewBuilder<'_>) -> wry::Result<WebView> {
            #[cfg(target_os = "linux")]
            return builder.build_gtk(&self.fixed);
            #[cfg(not(target_os = "linux"))]
            builder.build_as_child(&self.window)
        }
    }

    fn storage_json(text: &str) -> serde_json::Value {
        let value: serde_json::Value = serde_json::from_str(text).unwrap_or_default();
        if let Some(nested) = value.as_str() {
            return serde_json::from_str(nested).unwrap_or_default();
        }
        value
    }

    enum Signal {
        NormalLoaded,
        PrivateLoaded,
        Seeded(String),
        PrivateChecked(String),
        DownloadStarted(String),
        DownloadCompleted(String, bool),
        DenyDownload,
        LoadPdf,
        PdfLoaded,
        Timeout,
    }

    fn bounds(x: i32, width: i32) -> Rect {
        Rect {
            position: LogicalPosition::new(x, 0).into(),
            size: LogicalSize::new(width, 700).into(),
        }
    }

    fn exact_origin(url: &str, origin: &http::Uri) -> bool {
        url.parse::<http::Uri>().is_ok_and(|uri| {
            uri.scheme_str() == Some("http")
                && uri.host() == Some("127.0.0.1")
                && uri.authority() == origin.authority()
        })
    }

    fn expected_download(url: &str, origin: &http::Uri) -> bool {
        url.parse::<http::Uri>().is_ok_and(|uri| {
            exact_origin(url, origin)
                && uri.path() == "/download"
                && matches!(uri.query(), Some("copy=1" | "copy=2"))
        })
    }

    fn build_view(
        parent: &Parent,
        context: &mut WebContext,
        url: &str,
        origin: http::Uri,
        x: i32,
        width: i32,
        incognito: bool,
        proxy: EventLoopProxy<Signal>,
        downloads: PathBuf,
        next_path: Rc<Cell<u32>>,
    ) -> Result<WebView, Box<dyn std::error::Error>> {
        let origin_for_navigation = origin.clone();
        let origin_for_download = origin;
        let load_proxy = proxy.clone();
        let started_proxy = proxy.clone();
        let completed_proxy = proxy;
        let builder = WebViewBuilder::new_with_web_context(context)
            .with_url(url)
            .with_bounds(bounds(x, width))
            .with_incognito(incognito)
            .with_navigation_handler(move |url| exact_origin(&url, &origin_for_navigation))
            .with_download_started_handler(move |url, path| {
                let uri = url.parse::<http::Uri>().ok();
                if uri.as_ref().is_some_and(|uri| {
                    exact_origin(&url, &origin_for_download) && uri.path() == "/denied"
                }) {
                    println!("download_start allowed=false reason=fixture-denied");
                    return false;
                }
                if !expected_download(&url, &origin_for_download) {
                    println!("download_start allowed=false reason=outside-fixture-allowlist");
                    return false;
                }
                let n = next_path.get() + 1;
                next_path.set(n);
                *path = downloads.join(format!("download-{n:02}.bin"));
                println!(
                    "download_start allowed=true sequence={n} unique_destination=true url={url}"
                );
                let _ = started_proxy.send_event(Signal::DownloadStarted(url));
                true
            })
            .with_download_completed_handler(move |url, _, success| {
                println!("download_complete success={success} url={url}");
                let _ = completed_proxy.send_event(Signal::DownloadCompleted(url, success));
            })
            .with_on_page_load_handler(move |event, url| {
                if matches!(event, PageLoadEvent::Finished) {
                    if url.contains("profile=normal") {
                        let _ = load_proxy.send_event(Signal::NormalLoaded);
                    } else if url.contains("profile=private") {
                        let _ = load_proxy.send_event(Signal::PrivateLoaded);
                    } else if url.ends_with("/fixture.pdf") {
                        let _ = load_proxy.send_event(Signal::PdfLoaded);
                    } else {
                        println!("page_finished url={url}");
                    }
                }
            });
        #[cfg(target_os = "windows")]
        let builder = builder.with_additional_browser_args("");
        Ok(parent.build(builder.with_focused(false))?)
    }

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let mut args = std::env::args().skip(1);
        let base = args.next().ok_or("expected fixture origin URL")?;
        let work = PathBuf::from(args.next().ok_or("expected fixture work directory")?);
        let origin: http::Uri = base.parse()?;
        if origin.scheme_str() != Some("http")
            || origin.host() != Some("127.0.0.1")
            || origin.port().is_none()
            || origin.path() != "/"
        {
            return Err("fixture origin must be http://127.0.0.1:<port>/".into());
        }
        std::fs::create_dir_all(&work)?;
        let downloads = PathBuf::from(std::env::var("MUFFIN_S4_DOWNLOAD_DIR")?);
        std::fs::create_dir_all(&downloads)?;
        let width = 580;
        let events = EventLoopBuilder::<Signal>::with_user_event().build();
        let proxy = events.create_proxy();
        let parent = Parent::new(
            WindowBuilder::new()
                .with_title("Muffin S4 native feature fixture")
                .with_inner_size(LogicalSize::new(width * 2, 700))
                .build(&events)?,
        );
        let normal_dir = work.join("normal-profile");
        let private_dir = work.join("private-profile");
        std::fs::create_dir_all(&normal_dir)?;
        std::fs::create_dir_all(&private_dir)?;
        let mut normal_context = WebContext::new(Some(normal_dir));
        let mut private_context = WebContext::new(Some(private_dir));
        let next_path = Rc::new(Cell::new(0));
        let normal = build_view(
            &parent,
            &mut normal_context,
            &format!("{base}/?profile=normal"),
            origin.clone(),
            0,
            width,
            false,
            proxy.clone(),
            downloads.clone(),
            Rc::clone(&next_path),
        )?;
        let private = build_view(
            &parent,
            &mut private_context,
            &format!("{base}/?profile=private"),
            origin.clone(),
            width,
            width,
            true,
            proxy.clone(),
            downloads,
            Rc::clone(&next_path),
        )?;
        println!(
            "source_fixture=s4-native pid={} webview={}",
            std::process::id(),
            wry::webview_version()?
        );
        println!("normal_profile=default_or_scratch_context private_profile=with_incognito_true");
        println!(
            "progress_api=not_exposed_by_generic_WRY_download_callbacks pause_resume=not_tested"
        );
        let timeout = proxy.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(35));
            let _ = timeout.send_event(Signal::Timeout);
        });
        let mut normal_ready = false;
        let mut private_ready = false;
        let mut seeding = false;
        let mut storage_checked = false;
        let mut downloads_started = 0u8;
        let mut downloads_finished = 0u8;
        let mut private_auth_denied_requested = false;
        let mut denied_route_requested = false;
        let mut pdf_requested = false;
        let mut pdf_timer_fired = false;
        events.run(move |event, _, control| {
            *control = ControlFlow::Wait;
            match event {
                Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                    println!("fixture_finished outcome=window_closed storage_checked={storage_checked} downloads_started={downloads_started} downloads_completed={downloads_finished} private_auth_denied_requested={private_auth_denied_requested} denied_attachment_requested={denied_route_requested} pdf_requested={pdf_requested}");
                    *control = ControlFlow::Exit;
                }
                Event::UserEvent(Signal::NormalLoaded) => normal_ready = true,
                Event::UserEvent(Signal::PrivateLoaded) => private_ready = true,
                Event::UserEvent(Signal::Seeded(value)) => {
                    println!("normal_storage_seeded={value}");
                    let seeded = storage_json(&value);
                    if seeded["localStorage"] != true || seeded["cookie"] != true {
                        println!("fixture_error=normal_storage_positive_control_failed");
                        *control = ControlFlow::Exit;
                        return;
                    }
                    let checked = proxy.clone();
                    let script = "const s={localStorage:localStorage.getItem('muffin-s4-marker')==='S4-SYNTHETIC-MARKER',cookie:document.cookie.includes('muffin_s4=synthetic')};JSON.stringify(s)";
                    if let Err(error) = private.evaluate_script_with_callback(script, move |value| {
                        let _ = checked.send_event(Signal::PrivateChecked(value));
                    }) {
                        println!("private_storage_check_error={error}");
                        *control = ControlFlow::Exit;
                    }
                }
                Event::UserEvent(Signal::PrivateChecked(value)) => {
                    storage_checked = true;
                    let state = storage_json(&value);
                    let isolated = state["localStorage"] == false && state["cookie"] == false;
                    println!("private_storage_observed={value} private_storage_isolated={isolated}");
                    let _ = private.evaluate_script(&format!("fetch('/event',{{method:'POST',headers:{{'content-type':'application/json'}},body:JSON.stringify({{kind:'storage_observation',data:{{profile:'private',localStoragePresent:{},cookiePresent:{}}}}})}})", state["localStorage"], state["cookie"]));
                    let _ = normal.load_url(&format!("{base}/download?copy=1"));
                }
                Event::UserEvent(Signal::DownloadStarted(_url)) => {
                    downloads_started += 1;
                    if downloads_started == 1 {
                        let _ = normal.load_url(&format!("{base}/download?copy=2"));
                    } else if downloads_started == 2 {
                        private_auth_denied_requested = true;
                        let _ = private.load_url(&format!("{base}/download?copy=private"));
                        let next = proxy.clone();
                        std::thread::spawn(move || {
                            std::thread::sleep(Duration::from_secs(1));
                            let _ = next.send_event(Signal::DenyDownload);
                        });
                    }
                }
                Event::UserEvent(Signal::DenyDownload) => {
                    denied_route_requested = true;
                    let _ = normal.load_url(&format!("{base}/denied?copy=blocked"));
                    let next = proxy.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_secs(1));
                        let _ = next.send_event(Signal::LoadPdf);
                    });
                }
                Event::UserEvent(Signal::LoadPdf) => {
                    pdf_timer_fired = true;
                    if downloads_finished >= 2 && !pdf_requested {
                        pdf_requested = true;
                        println!("pdf_navigation_requested=true visual_render=unverified");
                        let _ = normal.load_url(&format!("{base}/fixture.pdf"));
                    }
                }
                Event::UserEvent(Signal::PdfLoaded) => {
                    println!("pdf_page_load_finished=true visual_render=unverified");
                    println!("fixture_finished outcome=completed storage_checked={storage_checked} downloads_started={downloads_started} downloads_completed={downloads_finished} private_auth_denied_requested={private_auth_denied_requested} denied_attachment_requested={denied_route_requested} pdf_requested={pdf_requested}");
                    *control = ControlFlow::Exit;
                }
                Event::UserEvent(Signal::DownloadCompleted(url, success)) => {
                    downloads_finished += 1;
                    println!("download_result index={downloads_finished} success={success} url={url}");
                    if pdf_timer_fired && downloads_finished >= 2 && !pdf_requested {
                        pdf_requested = true;
                        println!("pdf_navigation_requested=true visual_render=unverified");
                        let _ = normal.load_url(&format!("{base}/fixture.pdf"));
                    }
                }
                Event::UserEvent(Signal::Timeout) => {
                    println!("fixture_finished outcome=timeout storage_checked={storage_checked} downloads_started={downloads_started} downloads_completed={downloads_finished} private_auth_denied_requested={private_auth_denied_requested} denied_attachment_requested={denied_route_requested} pdf_requested={pdf_requested}");
                    *control = ControlFlow::Exit;
                }
                Event::MainEventsCleared if normal_ready && private_ready && !seeding => {
                    seeding = true;
                    let ready = proxy.clone();
                    let script = "localStorage.setItem('muffin-s4-marker','S4-SYNTHETIC-MARKER');document.cookie='muffin_s4=synthetic; Path=/; SameSite=Lax';JSON.stringify({localStorage:localStorage.getItem('muffin-s4-marker')==='S4-SYNTHETIC-MARKER',cookie:document.cookie.includes('muffin_s4=synthetic')})";
                    if let Err(error) = normal.evaluate_script_with_callback(script, move |value| {
                        let _ = ready.send_event(Signal::Seeded(value));
                    }) {
                        println!("normal_storage_setup_error={error}");
                        *control = ControlFlow::Exit;
                    }
                }
                _ => {}
            }
            let _lifetimes = (&parent.window, &normal_context, &private_context);
        });
    }
}
