//! Bounded WRY + TAO shell and fixed-tab workload shape for S5.
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("fixture_error=this benchmark-shape source revision is macOS-only");
    std::process::exit(2);
}

#[cfg(target_os = "macos")]
fn main() {
    if let Err(error) = native::run() {
        eprintln!("fixture_error={error}");
        std::process::exit(1);
    }
}

#[cfg(target_os = "macos")]
mod native {
    use std::{
        collections::BTreeSet,
        time::{Duration, Instant},
    };
    use tao::{
        dpi::LogicalSize,
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
        window::{Window, WindowBuilder},
    };
    use wry::{http, PageLoadEvent, Rect, WebView, WebViewBuilder, WebViewBuilderExtDarwin};

    enum Signal {
        PageFinished(String),
        Stop,
    }

    fn validated_origin(input: &str) -> Result<String, Box<dyn std::error::Error>> {
        let uri: http::Uri = input.parse()?;
        if uri.scheme_str() != Some("http")
            || uri.host() != Some("127.0.0.1")
            || uri.port().is_none()
            || !matches!(uri.path(), "" | "/")
            || uri.query().is_some()
            || uri
                .authority()
                .is_some_and(|authority| authority.as_str().contains('@'))
        {
            return Err(
                "base URL must be http://127.0.0.1:<port> with no credentials, path, or query"
                    .into(),
            );
        }
        Ok(format!(
            "http://{}",
            uri.authority().ok_or("missing port authority")?
        ))
    }

    fn make_view(
        window: &Window,
        origin: &str,
        slot: usize,
        visible: bool,
        proxy: EventLoopProxy<Signal>,
    ) -> Result<WebView, Box<dyn std::error::Error>> {
        let url = format!("{origin}/tab?slot={slot}");
        let allowed_origin = origin.to_owned();
        Ok(WebViewBuilder::new()
            .with_url(&url)
            .with_bounds(Rect {
                position: tao::dpi::LogicalPosition::new(0, 0).into(),
                size: LogicalSize::new(1280, 820).into(),
            })
            .with_visible(visible)
            .with_data_store_identifier(*b"MuffinS5Profile1")
            .with_navigation_handler(move |candidate| {
                candidate.parse::<http::Uri>().is_ok_and(|uri| {
                    uri.scheme_str() == Some("http")
                        && uri.host() == Some("127.0.0.1")
                        && uri.port().is_some()
                        && uri.authority().is_some_and(|authority| {
                            format!("http://{authority}") == allowed_origin
                        })
                        && uri.path().starts_with("/tab")
                })
            })
            .with_on_page_load_handler(move |event, url| {
                if matches!(event, PageLoadEvent::Finished) {
                    let _ = proxy.send_event(Signal::PageFinished(url));
                }
            })
            .build_as_child(window)?)
    }

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let mut args = std::env::args().skip(1);
        let origin = validated_origin(&args.next().ok_or("expected loopback base URL")?)?;
        let count: usize = args
            .next()
            .ok_or("expected tab count 0, 1, 5, or 10")?
            .parse()?;
        if !matches!(count, 0 | 1 | 5 | 10) {
            return Err("tab count must be 0, 1, 5, or 10".into());
        }
        let seconds: u64 = args
            .next()
            .ok_or("expected run duration seconds")?
            .parse()?;
        if !(1..=600).contains(&seconds) {
            return Err("duration must be between 1 and 600 seconds".into());
        }
        let started = Instant::now();
        let events = EventLoopBuilder::<Signal>::with_user_event().build();
        let proxy = events.create_proxy();
        let window = WindowBuilder::new()
            .with_title("Muffin S5 bounded benchmark-shape fixture")
            .with_inner_size(LogicalSize::new(1280, 820))
            .build(&events)?;
        println!("process_id={}", std::process::id());
        println!(
            "host=macOS architecture={} webview={}",
            std::env::consts::ARCH,
            wry::webview_version()?
        );
        println!("profile=WKWebsiteDataStore_custom_persistent id=MuffinS5Profile1");
        println!(
            "tab_count={count} mode={}",
            if count == 0 {
                "tao_shell_only"
            } else {
                "fixed_webviews"
            }
        );
        println!(
            "shell_created_elapsed_ms={}",
            started.elapsed().as_secs_f64() * 1000.0
        );
        let mut views = Vec::with_capacity(count);
        for slot in 0..count {
            let view = make_view(&window, &origin, slot, slot == 0, proxy.clone())?;
            println!("webview_created slot={slot} visible={}", slot == 0);
            views.push(view);
        }
        if count > 0 {
            println!(
                "webviews_created_elapsed_ms={}",
                started.elapsed().as_secs_f64() * 1000.0
            );
        }
        let timer = proxy.clone();
        let remaining = Duration::from_secs(seconds).saturating_sub(started.elapsed());
        std::thread::spawn(move || {
            std::thread::sleep(remaining);
            let _ = timer.send_event(Signal::Stop);
        });
        let mut ready = BTreeSet::new();
        events.run(move |event, _, control| {
            *control = ControlFlow::Wait;
            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    println!(
                        "benchmark_end reason=window_closed page_load_callbacks={}",
                        ready.len()
                    );
                    *control = ControlFlow::Exit;
                }
                Event::UserEvent(Signal::PageFinished(url)) => {
                    if let Some(slot) = url
                        .split("slot=")
                        .nth(1)
                        .and_then(|s| s.parse::<usize>().ok())
                    {
                        if slot < count && ready.insert(slot) {
                            println!(
                                "native_page_finished slot={slot} elapsed_ms={}",
                                started.elapsed().as_secs_f64() * 1000.0
                            );
                        }
                    }
                }
                Event::UserEvent(Signal::Stop) => {
                    println!(
                        "benchmark_end reason=duration page_load_callbacks={}",
                        ready.len()
                    );
                    *control = ControlFlow::Exit;
                }
                _ => {}
            }
            let _ = (&window, &views);
        });
    }
}
