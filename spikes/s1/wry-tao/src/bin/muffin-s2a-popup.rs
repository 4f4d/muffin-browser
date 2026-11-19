//! Loopback-only WRY popup feasibility fixture.
use std::{
    cell::{Cell, RefCell},
    path::Path,
    rc::Rc,
    time::{Duration, Instant},
};
#[cfg(target_os = "linux")]
use tao::platform::unix::WindowExtUnix;
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::{Window, WindowBuilder},
};
use wry::{
    http::Uri, NewWindowFeatures, NewWindowResponse, PageLoadEvent, WebView, WebViewBuilder,
};
#[cfg(target_os = "macos")]
use wry::{WebViewBuilderExtMacos, WebViewExtMacOS};
#[cfg(target_os = "linux")]
use wry::{WebViewBuilderExtUnix, WebViewExtUnix};
#[cfg(target_os = "windows")]
use wry::{WebViewBuilderExtWindows, WebViewExtWindows};

fn authority(url: &str) -> Option<String> {
    let uri = url.parse::<Uri>().ok()?;
    if uri.scheme_str() == Some("http") && uri.host() == Some("127.0.0.1") && uri.port().is_some() {
        return uri.authority().map(ToString::to_string);
    }
    None
}
fn allowed(url: &str, origins: &[String; 2]) -> bool {
    authority(url).is_some_and(|origin| origins.contains(&origin))
}
fn build(builder: WebViewBuilder<'_>, window: &Window) -> wry::Result<WebView> {
    #[cfg(target_os = "linux")]
    return builder.build_gtk(window.default_vbox().expect("TAO GTK container"));
    #[cfg(not(target_os = "linux"))]
    builder.build(window)
}
fn related(features: NewWindowFeatures) -> WebViewBuilder<'static> {
    let builder = WebViewBuilder::new();
    #[cfg(target_os = "linux")]
    return builder.with_related_view(features.opener.webview);
    #[cfg(target_os = "macos")]
    return builder.with_webview_configuration(features.opener.target_configuration);
    #[cfg(target_os = "windows")]
    builder.with_environment(features.opener.environment)
}
fn create(view: &WebView) -> NewWindowResponse {
    #[cfg(target_os = "macos")]
    let webview = view.webview().into_super();
    #[cfg(not(target_os = "macos"))]
    let webview = view.webview();
    NewWindowResponse::Create { webview }
}
fn guarded<'a>(
    builder: WebViewBuilder<'a>,
    origins: Rc<[String; 2]>,
    pane: &'static str,
) -> WebViewBuilder<'a> {
    builder
        .with_navigation_handler(move |url| {
            let ok = url == "about:blank" || allowed(&url, &origins);
            println!("navigation pane={pane} allowed={ok} url={url}");
            ok
        })
        .with_on_page_load_handler(move |event, url| {
            if matches!(event, PageLoadEvent::Finished) {
                println!("page_finished pane={pane} url={url}");
            }
        })
}
struct Popup {
    _view: WebView,
    window: Window,
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let start = args.next().ok_or("missing fixture URL")?;
    let second = args.next().ok_or("missing secondary fixture URL")?;
    let scratch = Path::new(&args.next().ok_or("missing scratch directory")?).canonicalize()?;
    let auto = args.next().as_deref() == Some("--auto");
    let origins = Rc::new([
        authority(&start).ok_or("non-loopback URL")?,
        authority(&second).ok_or("non-loopback URL")?,
    ]);
    println!("engine_version={}", wry::webview_version()?);
    let events = EventLoopBuilder::<()>::with_user_event().build();
    let parent = WindowBuilder::new()
        .with_title("Muffin S2a popup fixture")
        .with_inner_size(tao::dpi::LogicalSize::new(900, 440))
        .build(&events)?;
    let parent_id = parent.id();
    let mut slots = Vec::new();
    for id in (1..=10).rev() {
        slots.push(
            WindowBuilder::new()
                .with_title(format!("Muffin S2a popup {id}"))
                .with_inner_size(tao::dpi::LogicalSize::new(620, 280))
                .with_visible(false)
                .build(&events)?,
        );
    }
    let slots = RefCell::new(slots);
    let popups = Rc::new(RefCell::new(Vec::<Popup>::new()));
    let retain = Rc::clone(&popups);
    let request_origins = Rc::clone(&origins);
    let downloads = Rc::new(Cell::new(0));
    let main = build(
        guarded(WebViewBuilder::new().with_url(&start), origins, "parent")
            .with_new_window_req_handler(move |url, features| {
                println!(
                    "popup_request url={url} size={:?} position={:?}",
                    features.size, features.position
                );
                if url != "about:blank" && !allowed(&url, &request_origins) {
                    println!("popup_decision=deny reason=origin");
                    return NewWindowResponse::Deny;
                }
                let Some(window) = slots.borrow_mut().pop() else {
                    println!("popup_decision=deny reason=capacity");
                    return NewWindowResponse::Deny;
                };
                let target_dir = scratch.clone();
                let download_origins = Rc::clone(&request_origins);
                let sequence = Rc::clone(&downloads);
                let builder = guarded(related(features), Rc::clone(&request_origins), "popup")
                    .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
                    .with_download_started_handler(move |url, target| {
                        if !allowed(&url, &download_origins)
                            || !url
                                .parse::<Uri>()
                                .is_ok_and(|uri| uri.path() == "/download")
                        {
                            return false;
                        }
                        let id = sequence.get() + 1;
                        sequence.set(id);
                        *target = target_dir.join(format!("popup-download-{id}.txt"));
                        println!("popup_download_started sequence={id}");
                        true
                    })
                    .with_download_completed_handler(|_, _, success| {
                        println!("popup_download_completed success={success}")
                    });
                match build(builder, &window) {
                    Ok(view) => {
                        let response = create(&view);
                        let mut open = retain.borrow_mut();
                        window.set_visible(true);
                        open.push(Popup {
                            _view: view,
                            window,
                        });
                        println!("popup_decision=create native_window_count={}", open.len());
                        response
                    }
                    Err(error) => {
                        println!("popup_decision=deny reason=build error={error}");
                        NewWindowResponse::Deny
                    }
                }
            }),
        &parent,
    )?;
    println!("fixture_ready auto={auto}");
    let stop = Instant::now() + Duration::from_secs(24);
    events.run(move |event, _, control| {
        let (_main, _parent) = (&main, &parent);
        *control = if auto {
            ControlFlow::WaitUntil(stop)
        } else {
            ControlFlow::Wait
        };
        if auto && Instant::now() >= stop {
            println!(
                "fixture_closed reason=bounded_auto_window popups={}",
                popups.borrow().len()
            );
            *control = ControlFlow::Exit;
            return;
        }
        if let Event::WindowEvent {
            event, window_id, ..
        } = event
        {
            match event {
                WindowEvent::CloseRequested if window_id == parent_id => {
                    println!(
                        "fixture_closed reason=native_parent_close popups={}",
                        popups.borrow().len()
                    );
                    *control = ControlFlow::Exit;
                }
                WindowEvent::CloseRequested => {
                    let mut open = popups.borrow_mut();
                    let prior = open.len();
                    open.retain(|popup| popup.window.id() != window_id);
                    if prior != open.len() {
                        println!("popup_native_close remaining={}", open.len());
                    }
                }
                WindowEvent::Focused(true) => println!(
                    "window_focus pane={}",
                    if window_id == parent_id {
                        "parent"
                    } else {
                        "popup_or_slot"
                    }
                ),
                _ => {}
            }
        }
    });
}
fn main() {
    if let Err(error) = run() {
        eprintln!("fixture_error={error}");
        std::process::exit(1);
    }
}
