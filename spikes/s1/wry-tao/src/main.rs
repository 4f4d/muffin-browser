use std::{
    collections::BTreeSet,
    env,
    time::{Duration, Instant},
};
use tao::{
    dpi::{LogicalPosition, LogicalSize},
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
    window::{Window, WindowBuilder},
};
use wry::{PageLoadEvent, Rect, WebView, WebViewBuilder};

#[cfg(target_os = "linux")]
use gtk::prelude::*;
#[cfg(target_os = "linux")]
use tao::platform::unix::WindowExtUnix;
#[cfg(target_os = "linux")]
use wry::WebViewBuilderExtUnix;

const CHROME_HEIGHT: i32 = 72;
const OVERLAY: &str = r#"<!doctype html><meta charset=utf-8><title>M0 overlay</title>
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'">
<style>body{background:#b30036;color:white;font:20px system-ui;text-align:center}</style>
<p>Muffin overlay</p>"#;
const CHROME: &str = r#"<!doctype html><meta charset=utf-8><title>M0 chrome</title>
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'">
<style>body{background:#153568;color:white;font:20px system-ui}input{margin:12px}</style>
<label>Muffin chrome <input value="Local fixture" aria-label="Address"></label>"#;
const INSPECT: &str = "({title:document.title,width:innerWidth,height:innerHeight,focused:document.hasFocus(),ipc:typeof window.ipc,tauri:typeof window.__TAURI_INTERNALS__,url:location.href})";
const COUNTS: &[usize] = &[1, 5, 10, 20];

enum Signal {
    Loaded(String, String),
    Inspect(&'static str, String),
    Tick,
}

struct Parent {
    window: Window,
    #[cfg(target_os = "linux")]
    container: gtk::Fixed,
}

impl Parent {
    fn new(window: Window) -> Self {
        #[cfg(target_os = "linux")]
        let container = {
            let fixed = gtk::Fixed::new();
            window
                .default_vbox()
                .expect("TAO GTK container")
                .pack_start(&fixed, true, true, 0);
            fixed.show();
            let backend = gtk::gdk::Display::default()
                .expect("GTK display")
                .type_()
                .name()
                .to_string();
            println!("gdk_backend={backend}");
            fixed
        };
        Self {
            window,
            #[cfg(target_os = "linux")]
            container,
        }
    }

    fn child(&self, builder: WebViewBuilder<'_>) -> wry::Result<WebView> {
        #[cfg(target_os = "linux")]
        {
            builder.build_gtk(&self.container)
        }
        #[cfg(not(target_os = "linux"))]
        {
            builder.build_as_child(&self.window)
        }
    }

    fn dimensions(&self) -> (i32, i32) {
        let size = self
            .window
            .inner_size()
            .to_logical::<i32>(self.window.scale_factor());
        (size.width, size.height)
    }
}

fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect {
    Rect {
        position: LogicalPosition::new(x, y).into(),
        size: LogicalSize::new(width, height).into(),
    }
}

fn bounds(parent: &Parent) -> (Rect, Rect) {
    let (width, height) = parent.dimensions();
    (
        rect(0, 0, width, CHROME_HEIGHT),
        rect(0, CHROME_HEIGHT, width, (height - CHROME_HEIGHT).max(1)),
    )
}

fn view_builder(
    id: String,
    proxy: EventLoopProxy<Signal>,
    bounds: Rect,
) -> WebViewBuilder<'static> {
    WebViewBuilder::new()
        .with_bounds(bounds)
        .with_incognito(true)
        .with_devtools(false)
        .with_on_page_load_handler(move |event, url| {
            if matches!(event, PageLoadEvent::Finished) {
                let _ = proxy.send_event(Signal::Loaded(id.clone(), url));
            }
        })
}

fn inspect(view: &WebView, label: &'static str, proxy: EventLoopProxy<Signal>) -> wry::Result<()> {
    view.evaluate_script_with_callback(INSPECT, move |value| {
        let _ = proxy.send_event(Signal::Inspect(label, value));
    })
}

fn main() -> wry::Result<()> {
    println!("native_app_pid={}", std::process::id());
    let port: u16 = env::var("MUFFIN_FIXTURE_PORT")
        .expect("local fixture port")
        .parse()
        .expect("port");
    let origin = format!("http://127.0.0.1:{port}");
    let original = format!("{origin}/start");
    let next = format!("{origin}/next");
    let hold_seconds = env::var("MUFFIN_S1_HOLD")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let event_loop = EventLoopBuilder::<Signal>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let parent = Parent::new(
        WindowBuilder::new()
            .with_title("Muffin S1 WRY TAO")
            .with_inner_size(LogicalSize::new(900, 620))
            .with_min_inner_size(LogicalSize::new(720, 520))
            .build(&event_loop)
            .expect("native test window"),
    );
    let (chrome_bounds, content_bounds) = bounds(&parent);
    let chrome = parent
        .child(view_builder("chrome".into(), proxy.clone(), chrome_bounds).with_html(CHROME))?;
    let content = parent
        .child(view_builder("content".into(), proxy.clone(), content_bounds).with_url(&original))?;
    let overlay = parent.child(
        view_builder("overlay".into(), proxy.clone(), rect(190, 42, 270, 144))
            .with_focused(false)
            .with_html(OVERLAY),
    )?;
    parent.window.set_focus();
    println!("engine={}", wry::webview_version()?);
    println!("scale={}", parent.window.scale_factor());

    let timer = proxy.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(250));
        if timer.send_event(Signal::Tick).is_err() {
            break;
        }
    });
    let began = Instant::now();
    let mut stage = 0;
    let mut group = 0;
    let mut tabs = Vec::new();
    let mut loaded = BTreeSet::new();
    let mut ready_exit = None;
    let mut inspections_pending = 0;
    let mut failed = false;
    let mut previous_size = parent.dimensions();

    event_loop.run(move |event, _, control| {
        *control = ControlFlow::Wait;
        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                *control = ControlFlow::ExitWithCode(1);
            }
            Event::WindowEvent { event: WindowEvent::Resized(_), .. } => {
                let (chrome_rect, content_rect) = bounds(&parent);
                if let Err(error) = chrome.set_bounds(chrome_rect).and_then(|_| content.set_bounds(content_rect)) {
                    eprintln!("resizing child views: {error}");
                    *control = ControlFlow::ExitWithCode(1);
                    return;
                }
                println!("resize_parent={:?}", parent.dimensions());
            }
            Event::WindowEvent { event: WindowEvent::KeyboardInput { event, .. }, .. } => {
                println!("native_key={:?}", event.logical_key);
            }
            Event::UserEvent(Signal::Loaded(id, url)) => {
                println!("loaded={id} {url}");
                loaded.insert(id.clone());
                if stage == 2 && id == "content" && url == next {
                    stage = 3;
                    if let Err(error) = inspect(&content, "navigated", proxy.clone()) {
                        eprintln!("inspect after navigation: {error}");
                        *control = ControlFlow::ExitWithCode(1);
                    }
                }
            }
            Event::UserEvent(Signal::Inspect(label, data)) => {
                println!("inspection={label} {data}");
                match label {
                    "initial-chrome" | "initial-content" | "resized-chrome" | "resized-content" => {
                        let is_chrome = label.ends_with("chrome");
                        let (w, h) = parent.dimensions();
                        let expected_height = if is_chrome { CHROME_HEIGHT } else { h - CHROME_HEIGHT };
                        let value: serde_json::Value = serde_json::from_str(&data).unwrap_or_default();
                        let title = if is_chrome { "M0 chrome" } else { "M0 content" };
                        let expected_width = value["width"].as_i64().is_some_and(|n| (n - i64::from(w)).abs() <= 2);
                        let expected_height = value["height"].as_i64().is_some_and(|n| (n - i64::from(expected_height)).abs() <= 2);
                        let bridge_free = is_chrome || (value["ipc"] == "undefined" && value["tauri"] == "undefined");
                        if value["title"] != title || !expected_width || !expected_height || !bridge_free {
                            eprintln!("fixture_mismatch={label} parent={w}x{h} expected_title={title} actual={value}");
                            failed = true;
                        }
                        inspections_pending -= 1;
                    }
                    "overlay" => {
                        let value: serde_json::Value = serde_json::from_str(&data).unwrap_or_default();
                        if value["title"] != "M0 overlay" || value["width"] != 270 || value["height"] != 144 {
                            eprintln!("overlay_viewport_mismatch={value}");
                            failed = true;
                        }
                        inspections_pending -= 1;
                    }
                    "navigated" => {
                        let value: serde_json::Value = serde_json::from_str(&data).unwrap_or_default();
                        if value["title"] != "M0 next" || value["url"] != next {
                            eprintln!("navigation_mismatch={value}");
                            failed = true;
                        }
                        stage = 4;
                    }
                    _ => {}
                }
            }
            Event::UserEvent(Signal::Tick) => {
                if let Some(finish) = ready_exit {
                    if Instant::now() >= finish {
                        println!("fixture_result={}", if failed { "fail" } else { "pass" });
                        *control = ControlFlow::ExitWithCode(i32::from(failed));
                    }
                    return;
                }
                if began.elapsed() >= Duration::from_secs(150) {
                    eprintln!("fixture_timeout stage={stage} group={group} tabs={} loaded={loaded:?}", tabs.len());
                    *control = ControlFlow::ExitWithCode(2);
                    return;
                }
                let result = match stage {
                    0 if ["chrome", "content", "overlay"].iter().all(|id| loaded.contains(*id)) => {
                        stage = 1;
                        inspections_pending = 3;
                        inspect(&chrome, "initial-chrome", proxy.clone())
                            .and_then(|_| inspect(&content, "initial-content", proxy.clone()))
                            .and_then(|_| inspect(&overlay, "overlay", proxy.clone()))
                    }
                    1 if inspections_pending == 0 => {
                        stage = 5;
                        previous_size = parent.dimensions();
                        parent.window.set_inner_size(LogicalSize::new(1000, 700));
                        Ok(())
                    }
                    5 if parent.dimensions() != previous_size => {
                        stage = 6;
                        inspections_pending = 2;
                        inspect(&chrome, "resized-chrome", proxy.clone())
                            .and_then(|_| inspect(&content, "resized-content", proxy.clone()))
                    }
                    6 if inspections_pending == 0 => {
                        stage = 2;
                        content.load_url(&next)
                    }
                    4 if group < COUNTS.len() && tabs.is_empty() => {
                        let (_, content_rect) = bounds(&parent);
                        (0..COUNTS[group]).try_for_each(|number| {
                            let id = format!("tab-{group}-{number}");
                            let url = format!("{origin}/start?tab={group}-{number}");
                            let new = parent.child(view_builder(id.clone(), proxy.clone(), content_rect)
                                .with_focused(false).with_visible(false).with_url(url))?;
                            tabs.push((id, new));
                            Ok(())
                        })
                    }
                    4 if group < COUNTS.len() && tabs.iter().all(|(id, _)| loaded.contains(id)) => {
                        let count = tabs.len();
                        tabs.clear();
                        println!("cycle_complete={count}");
                        group += 1;
                        Ok(())
                    }
                    4 if group == COUNTS.len() => {
                        stage = 7;
                        content.focus().and_then(|_| inspect(&content, "focused-content", proxy.clone()))
                    }
                    7 => {
                        stage = 8;
                        chrome.focus().and_then(|_| inspect(&chrome, "focused-chrome", proxy.clone()))
                    }
                    8 => {
                        println!("screenshot_ready=true");
                        ready_exit = Some(Instant::now() + Duration::from_secs(hold_seconds.max(1)));
                        Ok(())
                    }
                    _ => Ok(()),
                };
                if let Err(error) = result {
                    eprintln!("fixture_error stage={stage}: {error}");
                    *control = ControlFlow::ExitWithCode(1);
                }
            }
            _ => {}
        }
    })
}
