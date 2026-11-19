use std::{
    env,
    error::Error,
    sync::mpsc::{channel, Receiver},
    thread,
    time::Duration,
};
use tauri::{
    LogicalPosition, LogicalSize, Webview, WebviewBuilder, WebviewUrl, Window, WindowBuilder,
};

const CHROME_HEIGHT: f64 = 72.0;
const INSPECT: &str = "({title:document.title,width:innerWidth,height:innerHeight,focused:document.hasFocus(),ipc:typeof window.ipc,tauri:typeof window.__TAURI_INTERNALS__,url:location.href})";
const TIMEOUT: Duration = Duration::from_secs(15);
type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn child(
    window: &Window,
    label: &str,
    url: WebviewUrl,
    bounds: (f64, f64, f64, f64),
) -> Result<(Webview, Receiver<String>)> {
    let (loaded, events) = channel();
    let builder = WebviewBuilder::new(label, url)
        .incognito(true)
        .on_page_load(move |view, event| {
            if event.event() == tauri::webview::PageLoadEvent::Finished {
                println!("loaded={} {}", view.label(), event.url());
                let _ = loaded.send(event.url().to_string());
            }
        });
    let (x, y, width, height) = bounds;
    let view = window.add_child(
        builder,
        LogicalPosition::new(x, y),
        LogicalSize::new(width, height),
    )?;
    Ok((view, events))
}

fn inspect(view: &Webview, label: &str) -> Result<()> {
    let (result, response) = channel();
    view.eval_with_callback(INSPECT, move |value| {
        let _ = result.send(value);
    })?;
    println!("inspection={label} {}", response.recv_timeout(TIMEOUT)?);
    Ok(())
}

fn size(window: &Window) -> Result<LogicalSize<f64>> {
    Ok(window.inner_size()?.to_logical(window.scale_factor()?))
}

fn layout(view: &Webview, y: f64, width: f64, height: f64) -> Result<()> {
    view.set_bounds(tauri::Rect {
        position: LogicalPosition::new(0.0, y).into(),
        size: LogicalSize::new(width, height).into(),
    })?;
    Ok(())
}

fn exercise(window: &Window, port: u16, hold: u64) -> Result<()> {
    let origin = format!("http://127.0.0.1:{port}");
    let initial = size(window)?;
    println!("parent_initial={initial:?}");
    let (chrome, chrome_events) = child(
        window,
        "chrome",
        WebviewUrl::App("index.html".into()),
        (0.0, 0.0, initial.width, CHROME_HEIGHT),
    )?;
    let (content, content_events) = child(
        window,
        "content",
        WebviewUrl::External(format!("{origin}/start").parse()?),
        (
            0.0,
            CHROME_HEIGHT,
            initial.width,
            initial.height - CHROME_HEIGHT,
        ),
    )?;
    let (overlay, overlay_events) = child(
        window,
        "overlay",
        WebviewUrl::App("overlay.html".into()),
        (190.0, 42.0, 270.0, 144.0),
    )?;
    for events in [&chrome_events, &content_events, &overlay_events] {
        events.recv_timeout(TIMEOUT)?;
    }
    inspect(&chrome, "initial-chrome")?;
    inspect(&content, "initial-content")?;
    inspect(&overlay, "overlay")?;
    window.set_size(LogicalSize::new(1000.0, 700.0))?;
    thread::sleep(Duration::from_millis(500));
    let resized = size(window)?;
    println!("parent_resized={resized:?}");
    layout(&chrome, 0.0, resized.width, CHROME_HEIGHT)?;
    layout(
        &content,
        CHROME_HEIGHT,
        resized.width,
        resized.height - CHROME_HEIGHT,
    )?;
    inspect(&chrome, "resized-chrome")?;
    inspect(&content, "resized-content")?;

    let next = format!("{origin}/next");
    content.navigate(next.parse()?)?;
    let loaded = content_events.recv_timeout(TIMEOUT)?;
    if loaded != next {
        return Err(format!("expected {next}, got {loaded}").into());
    }
    inspect(&content, "navigated")?;
    for count in [1, 5, 10, 20] {
        let mut tabs = Vec::with_capacity(count);
        for index in 0..count {
            let label = format!("tab-{count}-{index}");
            let address = format!("{origin}/start?tab={count}-{index}");
            let (tab, events) = child(
                window,
                &label,
                WebviewUrl::External(address.parse()?),
                (
                    0.0,
                    CHROME_HEIGHT,
                    resized.width,
                    resized.height - CHROME_HEIGHT,
                ),
            )?;
            tab.hide()?;
            tabs.push((tab, events, address));
        }
        for (tab, events, address) in tabs {
            if events.recv_timeout(TIMEOUT)? != address {
                return Err("created tab loaded a different URL".into());
            }
            tab.close()?;
        }
        println!("cycle_complete={count}");
    }
    content.set_focus()?;
    inspect(&content, "focused-content")?;
    chrome.set_focus()?;
    inspect(&chrome, "focused-chrome")?;
    println!("screenshot_ready=true");
    thread::sleep(Duration::from_secs(hold.max(1)));
    println!("fixture_result=observations_complete");
    Ok(())
}

fn main() {
    let port = env::var("MUFFIN_FIXTURE_PORT")
        .expect("loopback port")
        .parse()
        .expect("valid port");
    let hold = env::var("MUFFIN_S1_HOLD")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let result = tauri::Builder::default()
        .setup(move |app| {
            let window = WindowBuilder::new(app, "main")
                .title("Muffin S1 Tauri")
                .inner_size(900.0, 620.0)
                .min_inner_size(720.0, 520.0)
                .build()?;
            let handle = app.handle().clone();
            // add_child dispatches to the GUI thread and blocks until creation returns.
            thread::spawn(move || match exercise(&window, port, hold) {
                Ok(()) => handle.exit(0),
                Err(error) => {
                    eprintln!("fixture_error={error}");
                    handle.exit(1);
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("Tauri startup: {error}");
        std::process::exit(1);
    }
}
