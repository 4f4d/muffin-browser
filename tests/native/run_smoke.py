"""Exercise the Apple Silicon shell against a disposable loopback fixture."""

import argparse
from hashlib import file_digest, sha256
import json
from pathlib import Path
import platform
import plistlib
import shutil
import subprocess
from tempfile import TemporaryDirectory
import time
import tomllib

from fixture import (
    DOWNLOAD_PAYLOADS, RETRY_DOWNLOAD, RETRY_PAYLOAD, SLOW_DOWNLOAD, SLOW_DOWNLOAD_CHUNK,
    UNREQUESTED_DOWNLOAD, browser_fixture,
)

THEME_BASE_COLORS = {
    "light": {"selected_tab_fill": "rgb(232, 235, 238)", "surface_fill": "rgb(245, 246, 247)"},
    "dark": {"selected_tab_fill": "rgb(53, 58, 64)", "surface_fill": "rgb(32, 35, 39)"},
}


def until(process, description, read, timeout=18):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"browser exited ({process.returncode}) waiting for {description}")
        value = read()
        if value:
            return value
        time.sleep(0.05)
    raise TimeoutError(f"waiting for {description}")


def send_command(control, action, **fields):
    with (control / "commands.jsonl").open("a", encoding="utf-8") as stream:
        stream.write(json.dumps({"version": 1, "action": action, **fields}) + "\n")


def observation(control):
    state_path = control / "state.json"
    if state_path.is_file():
        return json.loads(state_path.read_text(encoding="utf-8"))
    return {}


def exercise(process, control, first, second, steps, capture):
    def command(action, **fields):
        send_command(control, action, **fields)

    def expect(**fields):
        def read():
            state = observation(control)
            if all(state.get(key) == value for key, value in fields.items()):
                return {key: state[key] for key in fields}

        state = until(process, repr(fields), read)
        steps.append(state)

    expect(version=1, status="home", chrome_ready=True)
    first_url = first.origin + "/first"
    second_url = second.origin + "/second"
    command("chrome_navigate", value=first_url)
    expect(url=first_url, status="ready", title="Muffin fixture | First page", secure=False)
    steps.append({"navigation_input": "trusted chrome address form"})
    manifest = Path(__file__).resolve().parents[2] / "crates/muffin/Cargo.toml"
    with manifest.open("rb") as stream:
        product_token = "Muffin/" + tomllib.load(stream)["package"]["version"]
    user_agent = first.user_agents.get("/first", "")
    if "AppleWebKit/" not in user_agent or product_token not in user_agent.split():
        raise AssertionError("WebKit request did not include the normal engine and Muffin product tokens")
    steps.append({"webkit_user_agent_token": True, "browser_product_token": product_token})
    command("navigate", value=first.origin + "/redirect")
    expect(url=second_url, status="ready", title="Muffin fixture | Second page", can_back=True)
    until(
        process,
        "page-owned IPC lookalike fixture finished",
        lambda: ("POST", "/remote-page-ready?chrome_state=false") in second.requests,
    )
    expect(url=second_url, status="ready")
    if ("POST", "/unexpected-native-read") in second.requests:
        raise AssertionError("the native shell consumed the page-owned action queue")
    if ("GET", "/unsolicited-popup") in second.requests:
        raise AssertionError("the non-interactive page opened an unsolicited pop-up")
    steps.append({"remote_page_executed": True, "remote_chrome_state_received": False})
    if capture is not None:
        capture(process, control, command)

    command("back")
    expect(url=first_url, status="ready", can_forward=True)
    command("forward")
    expect(url=second_url, status="ready")

    command("navigate", value="file:///muffin-fixture-should-stay-inaccessible")
    expect(status="blocked", url=second_url)
    if ("GET", "/redirect") not in first.requests:
        raise AssertionError("the first origin did not serve the redirect")
    steps.append({"distinct_loopback_origins": True, "redirect_requested": True})

    command("navigate", value=first.origin + "/slow")
    until(process, "slow fixture response", lambda: ("GET", "/slow") in first.requests)
    expect(loading=True)
    command("stop")
    expect(loading=False, native_loading=False)
    first.release_slow_page.set()
    if ("GET", "/unsolicited-popup") in second.requests:
        raise AssertionError("the non-interactive page opened an unsolicited pop-up")
    steps.append({"unsolicited_popup_requested": False})


def exercise_tabs(process, control, first, second, steps, capture):
    def command(action, **fields):
        send_command(control, action, **fields)

    def expect(description, matches):
        def read():
            state = observation(control)
            if state.get("chrome_ready") and matches(state):
                return state
        state = until(process, description, read)
        steps.append({
            "case": description,
            "selected_tab": state["selected_tab"],
            "tabs": [tab["id"] for tab in state["tabs"]],
            "url": state["url"],
            "window_count": state["window_count"],
        })
        return state

    def show_url(url, **expected):
        return expect("tab committed its own URL", lambda state: (
            state["url"] == url and state["status"] == "ready"
            and all(state[k] == value for k, value in expected.items())
        ))

    def row(state, tab_id):
        return next((tab for tab in state["tabs"] if tab["id"] == tab_id), {})

    first_url = first.origin + "/first"
    second_url = second.origin + "/second"
    command("navigate", value=first_url)
    first_tab = show_url(first_url)["selected_tab"]
    command("new_tab")
    created = expect("new tab is selected in the same native window", lambda state: (
        state["window_count"] == 1 and len(state["tabs"]) == 2
        and state["selected_tab"] != first_tab and state["status"] == "home"
    ))
    second_tab = created["selected_tab"]
    command("chrome_navigate", value=second_url)
    show_url(second_url, selected_tab=second_tab)
    command("duplicate_tab", id=second_tab)
    duplicate = expect("duplicate retains committed URL with a distinct ID", lambda state: (
        len(state["tabs"]) == 3 and state["selected_tab"] not in {first_tab, second_tab}
        and state["url"] == second_url and state["status"] == "ready"
    ))["selected_tab"]
    command("pin_tab", id=first_tab, pinned=True)
    expect("pin moves tab into the pinned segment", lambda state: (
        bool(state["tabs"]) and state["tabs"][0]["id"] == first_tab
        and state["tabs"][0]["pinned"]
    ))
    command("move_tab", id=duplicate, index=1)
    expect("unpinned tab reorders within the unpinned segment", lambda state: (
        [tab["id"] for tab in state["tabs"]] == [first_tab, duplicate, second_tab]
    ))
    command("select_tab", id=first_tab)
    show_url(first_url, selected_tab=first_tab)
    command("discard_tab", id=second_tab)
    expect("inactive tab retains metadata when its native view is discarded", lambda state: (
        state["selected_tab"] == first_tab and row(state, second_tab).get("discarded")
    ))
    command("select_tab", id=second_tab)
    show_url(second_url, selected_tab=second_tab)
    expect("selection restores the discarded WebView", lambda state: (
        not row(state, second_tab).get("discarded", True)
    ))
    command("close_tab", id=second_tab)
    expect("closed tab exposes an undo entry", lambda state: (
        len(state["tabs"]) == 2 and not row(state, second_tab) and state["can_restore"]
    ))
    command("reopen_tab")
    expect("reopened tab reloads its committed URL", lambda state: (
        len(state["tabs"]) == 3 and state["url"] == second_url and state["status"] == "ready"
    ))
    if capture is not None:
        capture(process, control, command)
    command("new_window")
    expect("second native window has an independent selected tab", lambda state: (
        state["window_count"] == 2 and len(state["tabs"]) == 1 and state["status"] == "home"
    ))
    command("navigate", value=first_url)
    show_url(first_url, window_count=2)
    if ("POST", "/unexpected-native-read") in second.requests:
        raise AssertionError("the native shell polled a remote page during tab or window changes")


def verify_session_restore(process, control, first, second, steps):
    urls = {first.origin + "/first", second.origin + "/second"}

    def restored():
        state = observation(control)
        if (
            state.get("chrome_ready")
            and state.get("window_count") == 2
            and state.get("status") == "ready"
            and state.get("url") in urls
        ):
            return state

    state = until(process, "restored windows and committed tab after restart", restored)
    steps.append({
        "case": "graceful restart restored two windows and a committed fixture tab",
        "window_count": state["window_count"],
        "selected_tab": state["selected_tab"],
        "url": state["url"],
    })


def verify_instance_forwarding(process, control, launch, first, second, steps):
    window_count = observation(control)["window_count"]
    subprocess.run(
        [*launch, first.origin + "/redirect"],
        check=True, capture_output=True, timeout=8,
    )

    def forwarded():
        state = observation(control)
        if (
            state.get("window_count") == window_count
            and state.get("url") == second.origin + "/second"
            and state.get("status") == "ready"
        ):
            return state

    state = until(process, "a second process forwarded its HTTP URL to the current instance", forwarded)
    steps.append({
        "case": "second process forwarded a URL and exited",
        "window_count": state["window_count"],
        "url": state["url"],
    })


def exercise_profile(process, control, first, second, steps, capture):
    first_url = first.origin + "/first"
    second_url = second.origin + "/second"

    def command(action, **fields):
        send_command(control, action, **fields)

    def expect(case, predicate):
        def read():
            state = observation(control)
            return state if state.get("chrome_ready") and predicate(state) else None

        state = until(process, case, read)
        steps.append({"case": case, "surface": state["surface"]})
        return state

    def expect_layout(heading, *, scroll=False, theme=None):
        path = control / "chrome-layout.json"
        path.unlink(missing_ok=True)
        command("inspect_chrome")
        until(process, f"{heading} native layout observation", path.is_file)
        layout = json.loads(path.read_text(encoding="utf-8"))
        if (not layout or layout["heading"] != heading or not layout["heading_visible"]
                or layout["width"] < 600 or layout["height"] < 350):
            raise AssertionError(f"{heading} does not fill the native chrome WebView: {layout}")
        if scroll and (layout["scroll_height"] <= layout["client_height"] or layout["scroll_probe"] <= 0):
            raise AssertionError(f"{heading} does not scroll with an overflowing list: {layout}")
        if theme is not None and layout["theme"] != theme:
            raise AssertionError(f"{heading} did not apply {theme} theme: {layout}")
        if theme in THEME_BASE_COLORS and layout["selected_tab_fill"] != THEME_BASE_COLORS[theme]["selected_tab_fill"]:
            raise AssertionError(f"{heading} shows a tab fill outside the {theme} palette: {layout}")
        steps.append({"native_chrome_layout": layout})

    for index in range(16):
        url = f"{first_url}?visit={index}"
        command("navigate", value=url)
        until(process, f"scroll fixture visit {index}", lambda: (
            (state := observation(control)).get("url") == url and state.get("status") == "ready"
        ))

    command("navigate", value=second_url)
    expect("profile fixture committed a browsing page", lambda s: (
        s.get("url") == second_url and s.get("status") == "ready" and s.get("surface") == "browse"
    ))
    command("show_surface", value="library")
    expect("Library shows persisted WebKit visits after restart", lambda s: (
        s.get("surface") == "library" and {first_url, second_url}.issubset(
            {item["url"] for item in s.get("history", [])}
        )
    ))
    command("query_library", value="first")
    expect("Library history search matches the requested local term", lambda s: (
        bool(s.get("history")) and all(item["url"].split("?", 1)[0] == first_url for item in s["history"])
    ))
    command("query_library", value="")
    command("bookmark_current")
    bookmarked = expect("active committed URL was saved as a bookmark", lambda s: s.get("bookmarked"))["bookmarks"]
    bookmark_id = next(item["id"] for item in bookmarked if item["url"] == second_url)
    command("create_folder", title="Reading", parent_id=None)
    folders = expect("Library contains a new bookmark folder", lambda s: any(
        item["url"] is None and item["title"] == "Reading" for item in s.get("bookmarks", [])
    ))["bookmarks"]
    folder_id = next(item["id"] for item in folders if item["url"] is None and item["title"] == "Reading")
    command("move_bookmark", id=bookmark_id, parent_id=folder_id)
    expect("bookmark moved under its folder", lambda s: any(
        item["id"] == bookmark_id and item["parent_id"] == folder_id for item in s.get("bookmarks", [])
    ))
    expect_layout("Library", scroll=True)
    if capture is not None:
        capture(process, control, command, "native-library")

    command("show_surface", value="start")
    expect("Start opens inside the trusted chrome", lambda s: s.get("surface") == "start")
    command("add_shortcut", title="Read the fixture", url=second_url)
    expect("Start shows a native-owned shortcut", lambda s: any(
        item["url"] == second_url and item["title"] == "Read the fixture" for item in s.get("shortcuts", [])
    ))
    command("query_omnibox", value="second")
    expect("Omnibox returns ranked local suggestions including a saved bookmark", lambda s: any(
        item["url"] == second_url and item["bookmarked"] for item in s.get("suggestions", [])
    ))
    expect_layout("Muffin")
    if capture is not None:
        capture(process, control, command, "native-start")

    command("show_surface", value="settings")
    command("set_theme", value="light")
    command("set_search_engine", value="bing")
    command("set_restore_session", enabled=False)
    command("set_clear_history_on_exit", enabled=True)
    expect("Preferences reflect the applied theme, search and startup settings", lambda s: (
        s.get("surface") == "settings" and s.get("settings") == {
            "theme": "light", "search_engine": "bing", "restore_session": False, "clear_history_on_exit": True,
        }
    ))
    expect_layout("Settings", theme="light")
    if capture is not None:
        capture(process, control, command, "native-settings")

    command("show_surface", value="library")
    expect("Library still contains browsing history before clearing it", lambda s: (
        s.get("surface") == "library" and bool(s.get("history"))
    ))
    command("clear_history", value="hour")
    expect("Clearing the last hour removed the recent fixture visits", lambda s: (
        s.get("surface") == "library" and s.get("history") == []
        and s.get("notice") == "History cleared."
    ))
    command("navigate", value=first_url)
    expect("A new visit is committed before clean exit", lambda s: s.get("url") == first_url and s.get("status") == "ready")


def verify_profile_restart(process, control, steps):
    def restored():
        state = observation(control)
        settings = state.get("settings") or {}
        if (
            state.get("chrome_ready") and state.get("surface") == "start"
            and state.get("window_count") == 1 and settings.get("theme") == "light"
            and settings.get("search_engine") == "bing" and settings.get("restore_session") is False
            and settings.get("clear_history_on_exit") is True and state.get("shortcuts")
        ):
            return state

    until(process, "profile preferences and shortcut survived restart", restored)
    send_command(control, "show_surface", value="library")
    until(process, "clear-on-exit removed the final visit and kept bookmarks", lambda: (
        (state := observation(control)).get("surface") == "library"
        and state.get("history") == []
        and any(item["title"] == "Reading" for item in state.get("bookmarks", []))
    ))
    steps.append({"case": "profile restart retained settings, shortcuts and bookmarks and cleared history"})


def exercise_page_tools(process, control, first, second, steps, capture, pdf_output):
    def command(action, **fields):
        send_command(control, action, **fields)

    def expect(case, predicate):
        def read():
            state = observation(control)
            return state if predicate(state) else None

        return until(process, case, read)

    second_url = second.origin + "/second?tools=1"
    command("navigate", value=second_url)
    expect("page-tools fixture committed the new document", lambda s: (
        s.get("url") == second_url and s.get("status") == "ready" and s.get("surface") == "browse"
    ))
    command("find_page", value="amber muffin fixture", backwards=False)
    expect("WebKit find reported the visible fixture phrase", lambda s: (
        s.get("page_tools", {}).get("find_found") is True
    ))
    command("find_page", value="q3x-absent-find-token-p9", backwards=True)
    expect("WebKit find rejected the absent phrase", lambda s: (
        s.get("page_tools", {}).get("find_found") is False
    ))
    steps.append({"native_webkit_find": {"visible_phrase": True, "missing_phrase": False}})
    command("zoom_page", value="in")
    expect("WebKit native pageZoom setter read back at 110 percent", lambda s: (
        s.get("page_tools", {}).get("zoom_percent") == 110
    ))
    command("zoom_page", value="reset")
    expect("WebKit native pageZoom read back at 100 percent after reset", lambda s: (
        s.get("page_tools", {}).get("zoom_percent") == 100
    ))
    steps.append({"native_webkit_zoom_percent": [110, 100]})

    command("capture_pdf")
    pdf_file = control / "fixture-export.pdf"
    until(process, "WKWebView wrote its PDF output", pdf_file.is_file)
    pdf_data = pdf_file.read_bytes()
    if len(pdf_data) < 256 or not pdf_data.startswith(b"%PDF-") or not pdf_data.rstrip().endswith(b"%%EOF"):
        raise AssertionError("WKWebView did not export a complete PDF")
    steps.append({"native_pdf_export": {"bytes": len(pdf_data), "sha256": sha256(pdf_data).hexdigest()}})
    if pdf_output is not None:
        shutil.copyfile(pdf_file, pdf_output)

    command("print_fixture_pdf")
    print_path = control / "fixture-print.pdf"

    def completed_print():
        if print_path.is_file():
            data = print_path.read_bytes()
            if len(data) >= 256 and data.startswith(b"%PDF-") and data.rstrip().endswith(b"%%EOF"):
                return data
        return None

    print_data = until(process, "AppKit print operation wrote a complete local PDF", completed_print)
    steps.append({"native_appkit_print_to_file": {
        "bytes": len(print_data), "sha256": sha256(print_data).hexdigest(),
    }})
    if pdf_output is not None:
        pdf_output.with_name("native-print.pdf").write_bytes(print_data)

    command("navigate", value=first.origin + "/attempt-download")
    until(process, "remote page attempted an automatic download", lambda: (
        ("GET", UNREQUESTED_DOWNLOAD) in first.requests
    ))
    command("show_surface", value="downloads")
    pending = expect("automatic remote attachment awaits trusted approval", lambda s: (
        s.get("surface") == "downloads"
        and any(item.get("status") == "awaiting_approval" for item in s.get("downloads", []))
    ))
    remote_row = next(item for item in pending["downloads"] if item.get("status") == "awaiting_approval")
    directory = control / "downloads"

    def files_below_download_root():
        return [path for path in directory.rglob("*") if path.is_file()] if directory.exists() else []

    def file_set():
        return {path.resolve() for path in files_below_download_root()}

    if any(directory.iterdir()):
        raise AssertionError("a download created a filesystem artifact before trusted approval")
    command("decide_download", id=remote_row["id"], approve=False)
    cancelled = expect("dismissed remote attachment is cancelled", lambda s: any(
        item.get("id") == remote_row["id"] and item.get("status") == "cancelled"
        for item in s.get("downloads", [])
    ))
    if any(directory.iterdir()):
        raise AssertionError("a denied remote attachment left a filesystem artifact")
    steps.append({"remote_attachment": "awaiting_approval_then_cancelled", "published": False})

    def approve_download(label, start, server, requested_path):
        before = file_set()
        start()
        until(process, f"fixture received {label}", lambda: ("GET", requested_path) in server.requests)
        command("show_surface", value="downloads")
        state = expect(f"{label} awaits trusted approval", lambda s: (
            s.get("surface") == "downloads"
            and any(item.get("status") == "awaiting_approval" for item in s.get("downloads", []))
        ))
        row = next(item for item in state["downloads"] if item.get("status") == "awaiting_approval")
        if not isinstance(row.get("host"), str) or not row["host"]:
            raise AssertionError(f"{label} pending row omitted its source host")
        if file_set() != before:
            raise AssertionError(f"{label} created a filesystem artifact before approval")
        command("decide_download", id=row["id"], approve=True)
        return row["id"]

    def begin_download(label, start, server, requested_path):
        download_id = approve_download(label, start, server, requested_path)
        terminal = expect(f"{label} approval decision reached a terminal state", lambda s: any(
            item.get("id") == download_id
            and item.get("status") in {"finished", "failed", "cancelled"}
            for item in s.get("downloads", [])
        ))
        result = next(item for item in terminal["downloads"] if item.get("id") == download_id)
        if result.get("status") != "finished":
            raise AssertionError(f"{label} did not finish after approval: {result.get('status')}")
        if result.get("openable") is not False:
            raise AssertionError("unsupported fixture .bin appeared as openable")
        return result

    approved_rows = []
    command("navigate", value=second_url)
    expect("content link fixture is ready after declining the automatic attachment", lambda s: (
        s.get("surface") == "browse" and s.get("status") == "ready" and s.get("url") == second_url
    ))
    alpha = begin_download(
        "content link attachment",
        lambda: command("click_fixture_link"),
        second,
        "/download/alpha",
    )
    approved_rows.append(alpha)
    beta = begin_download(
        "trusted address-bar attachment",
        lambda: command("chrome_navigate", value=first.origin + "/download/beta"),
        first,
        "/download/beta",
    )
    approved_rows.append(beta)
    redirected = begin_download(
        "cross-origin redirected attachment",
        lambda: command("chrome_navigate", value=first.origin + "/redirect-download"),
        second,
        "/download/redirect",
    )
    if redirected.get("host") != second.origin.removeprefix("http://"):
        raise AssertionError("the redirected download displayed a host other than the response host")

    payloads = []
    for file in files_below_download_root():
        if not file.is_symlink():
            with file.open("rb") as stream:
                payloads.append((file.stat().st_size, file_digest(stream, "sha256").hexdigest()))
    payloads.sort()
    expected_payloads = [DOWNLOAD_PAYLOADS[path] for path in (
        "/download/alpha", "/download/beta", "/download/redirect",
    )]
    expected = sorted((len(data), sha256(data).hexdigest()) for data in expected_payloads)
    if payloads != expected:
        raise AssertionError(f"approved native downloads did not match fixture payloads: {payloads}")
    names = [item.get("name") for item in approved_rows]
    if names != ["shared.bin", "shared (1).bin"]:
        raise AssertionError(f"download rows did not report collision-safe publish names: {names}")
    steps.append({
        "native_download_consent": {
            "denied_remote_attachment": next(
                item["status"] for item in cancelled["downloads"] if item["id"] == remote_row["id"]
            ),
            "approved_downloads": [
                {"name": item.get("name"), "host": item.get("host"), "status": item.get("status")}
                for item in approved_rows
            ],
            "approved_names": names,
            "approved_payloads": [{"bytes": size, "sha256": digest} for size, digest in payloads],
            "cross_origin_redirect_status": redirected.get("status"),
            "cross_origin_redirect_name": redirected.get("name"),
            "unapproved_filesystem_writes": False,
        },
    })
    if capture is not None:
        capture(process, control, command, "native-downloads")

    published_paths = file_set()
    active_id = approve_download(
        "slow attachment",
        lambda: command("chrome_navigate", value=first.origin + SLOW_DOWNLOAD),
        first,
        SLOW_DOWNLOAD,
    )
    progressing = expect("slow download publishes live bytes before EOF", lambda s: any(
        item["id"] == active_id and item["status"] == "downloading"
        and 0 < item["bytes"] <= len(SLOW_DOWNLOAD_CHUNK)
        for item in s.get("downloads", [])
    ))
    transferred = next(item["bytes"] for item in progressing["downloads"] if item["id"] == active_id)
    if capture is not None:
        capture(process, control, command, "native-download-progress")
    command("clear_downloads")
    expect("clearing the list preserves the active transfer", lambda s: [
        (item["id"], item["status"]) for item in s.get("downloads", [])
    ] == [(active_id, "downloading")])
    command("cancel_download", id=active_id)
    expect("active transfer reaches cancelled state", lambda s: [
        (item["id"], item["status"]) for item in s.get("downloads", [])
    ] == [(active_id, "cancelled")])
    if file_set() != published_paths or any(directory.glob(".muffin-download-*")):
        raise AssertionError("cancel or clear changed published files or left a staging directory")
    command("clear_downloads")
    expect("cancelled transfer can be cleared", lambda s: s.get("downloads") == [])
    steps.append({"native_download_management": {
        "live_transferred_bytes": transferred,
        "file_remainder_withheld": not first.release_slow_download.is_set(),
        "in_flight_cancelled": True,
        "staging_cleaned": True,
        "clear_preserves_active_transfer": True,
        "retained_published_file_count": len(published_paths),
    }})

    failed_id = approve_download(
        "interrupted attachment",
        lambda: command("chrome_navigate", value=first.origin + RETRY_DOWNLOAD),
        first,
        RETRY_DOWNLOAD,
    )
    expect("initial retry fixture transfers bytes before the network breaks", lambda s: any(
        item["id"] == failed_id and item["status"] == "downloading" and item["bytes"] > 0
        for item in s.get("downloads", [])
    ))
    first.break_retry_download.set()
    expect("broken response exposes retry for the original GET", lambda s: any(
        item["id"] == failed_id and item["status"] == "failed" and item.get("retryable") is True
        for item in s.get("downloads", [])
    ))
    if file_set() != published_paths or any(directory.glob(".muffin-download-*")):
        raise AssertionError("a failed transfer retained its partial file")
    if first.requests.count(("GET", RETRY_DOWNLOAD)) != 1:
        raise AssertionError("the failed native transfer retried without a chrome action")
    command("retry_download", id=failed_id)
    restarted = expect("Retry creates a fresh approval row", lambda s: any(
        item["id"] != failed_id and item["status"] == "awaiting_approval"
        for item in s.get("downloads", [])
    ))
    new_id = next(item["id"] for item in restarted["downloads"] if item["status"] == "awaiting_approval")
    if first.requests.count(("GET", RETRY_DOWNLOAD)) != 2 or file_set() != published_paths:
        raise AssertionError("Retry did not issue exactly one fresh request without writing files")
    command("decide_download", id=new_id, approve=True)
    expect("fresh approval finishes the restarted native download", lambda s: any(
        item["id"] == new_id and item["status"] == "finished" and item.get("openable") is True
        for item in s.get("downloads", [])
    ))
    digest = sha256((directory / "retry.txt").read_bytes()).hexdigest()
    if digest != sha256(RETRY_PAYLOAD).hexdigest():
        raise AssertionError("retry file did not match the original fixture")
    steps.append({"native_download_retry": {
        "failed_download_id": failed_id, "approval_download_id": new_id,
        "get_requests": 2, "unapproved_filesystem_writes": False, "sha256": digest,
    }})
    if capture is not None:
        capture(process, control, command, "native-download-retry")


def exercise_media(process, control, server, steps):
    state_path = control / "fixture-playback.txt"

    def expect_native_state(expected):
        sent_at = 0
        state_path.unlink(missing_ok=True)

        def read():
            nonlocal sent_at
            if state_path.exists() and state_path.read_text() == expected:
                return True
            if time.monotonic() - sent_at > 0.3:
                send_command(control, "inspect_fixture_playback")
                sent_at = time.monotonic()
            return None

        until(process, f"WebKit playback state {expected}", read)

    first_url = server.origin + "/media?tab=one"
    send_command(control, "navigate", value=first_url)
    first = until(process, "first media fixture view", lambda: (
        state if (state := observation(control)).get("url") == first_url
        and state.get("status") == "ready" else None
    ))["selected_tab"]
    until(process, "first fixture tab plays", lambda: server.media_states.get("one", (0, 0))[0] > 0.2)
    expect_native_state("playing")
    send_command(control, "set_media_suspended", enabled=True)
    expect_native_state("suspended")
    previous = server.media_states["one"]

    def stable_clock():
        nonlocal previous
        sample = server.media_states["one"]
        if sample[1] <= previous[1]:
            return None
        stable = abs(sample[0] - previous[0]) <= 0.03
        previous = sample
        return sample if stable else None

    paused_at, _ = until(process, "stable page clock after native suspension", stable_clock)
    send_command(control, "new_tab")
    until(process, "second tab selected", lambda: observation(control)["selected_tab"] != first)
    second_url = server.origin + "/media?tab=two"
    send_command(control, "navigate", value=second_url)
    until(process, "second fixture tab plays independently", lambda: (
        server.media_states.get("two", (0, 0))[0] > 0.2
    ))
    if abs(server.media_states["one"][0] - paused_at) > 0.1:
        raise AssertionError(f"first tab clock changed while suspended: {paused_at} -> {server.media_states['one'][0]}")
    send_command(control, "select_tab", id=first)
    until(process, "suspension is retained by the first tab", lambda: (
        (state := observation(control))["selected_tab"] == first
        and state["page_tools"]["media_suspended"] is True
    ))
    send_command(control, "set_media_suspended", enabled=False)
    until(process, "first fixture tab resumes", lambda: server.media_states["one"][0] > paused_at + 0.2)
    expect_native_state("playing")
    steps.append({"native_media": {
        "first_tab_played": True, "first_tab_suspended_at_seconds": paused_at,
        "native_playback_states": ["playing", "suspended", "playing"],
        "other_tab_played_while_first_suspended": True, "first_tab_resumed": True,
    }})


def exercise_chrome_layout(process, control, steps, capture):
    def command(action, **fields):
        send_command(control, action, **fields)

    def inspect(label, width, min_address_width, min_tab_strip, *, tab_overflow=False):
        path = control / "chrome-layout.json"
        last_layout = None

        def read():
            nonlocal last_layout
            if not path.exists():
                command("inspect_chrome")
                return None
            layout = json.loads(path.read_text(encoding="utf-8"))
            path.unlink()
            last_layout = layout
            if (layout and abs(layout["width"] - width) <= 2
                    and layout["address_width"] >= min_address_width
                    and layout["tab_strip_width"] >= min_tab_strip
                    and layout["new_tab_visible"] and layout["selected_tab_visible"]
                    and layout["new_tab_gap"] is not None and 0 <= layout["new_tab_gap"] <= 12
                    and (not tab_overflow or layout["tab_strip_scroll_width"] > layout["tab_strip_width"])):
                return layout
            return None

        # Let a resize or selection settle before measuring the rendered WebKit view.
        path.unlink(missing_ok=True)
        command("inspect_chrome")
        until(process, f"{label} native chrome", lambda: path.is_file())
        try:
            layout = until(process, f"{label} navigation and tab geometry", read, timeout=5)
        except TimeoutError:
            steps.append({label: last_layout})
            if capture is not None:
                capture(process, control, command, label)
            raise AssertionError(f"{label} did not fit the native chrome: {last_layout}") from None
        steps.append({label: layout})
        if capture is not None:
            capture(process, control, command, label)

    command("show_surface", value="start")
    inspect("native-chrome-regular", 1100, 550, 180)
    command("compact_fixture_window")
    path = control / "chrome-layout.json"
    path.unlink(missing_ok=True)

    def compact_rendered():
        if not path.exists():
            command("inspect_chrome")
            return False
        layout = json.loads(path.read_text(encoding="utf-8"))
        path.unlink()
        return layout and abs(layout["width"] - 640) <= 2

    until(process, "native WebKit compact resize before creating tabs", compact_rendered, timeout=6)
    for _ in range(8):
        command("new_tab")
    until(process, "nine tabs in compact native chrome", lambda: len(observation(control).get("tabs", [])) >= 9)
    inspect("native-chrome-compact", 640, 280, 420, tab_overflow=True)
    command("open_fixture_browser_actions")
    path.unlink(missing_ok=True)
    command("inspect_chrome")
    until(process, "compact browser actions layout", path.is_file)
    layout = json.loads(path.read_text(encoding="utf-8"))
    steps.append({"native-chrome-compact-actions": layout})
    if capture is not None:
        capture(process, control, command, "native-chrome-compact-actions")
    if (not layout or not layout["more_actions_visible"] or not layout["more_actions_fit"]
            or layout["more_action_count"] != 7):
        raise AssertionError(f"browser actions do not fit the compact toolbar: {layout}")


def contrast_ratio(foreground, background):
    def luminance(color):
        if color.startswith("#") and len(color) == 7:
            channels = [int(color[index:index + 2], 16) for index in (1, 3, 5)]
        elif color.startswith("rgb(") and color.endswith(")"):
            channels = [int(channel) for channel in color[4:-1].split(",")]
        else:
            raise AssertionError(f"expected an opaque sRGB color, got {color!r}")
        normalized = [channel / 255 for channel in channels]
        linear = [value / 12.92 if value <= 0.04045 else ((value + 0.055) / 1.055) ** 2.4 for value in normalized]
        return sum(value * weight for value, weight in zip(linear, (0.2126, 0.7152, 0.0722), strict=True))

    dark, light = sorted((luminance(foreground), luminance(background)))
    return (light + 0.05) / (dark + 0.05)


def exercise_native_shortcuts(process, control, first, steps):
    def expect(label, predicate):
        return until(process, label, lambda: (state if predicate(state := observation(control)) else None), timeout=8)

    focus_receipt = control / "shortcut-first-responder.txt"

    def press(shortcut, label, predicate):
        focus_receipt.unlink(missing_ok=True)
        send_command(control, "press_fixture_shortcut", shortcut=shortcut)
        state = expect(label, predicate)
        steps.append({"appkit_shortcut": shortcut, "case": label, "first_responder_before": focus_receipt.read_text(),
                      "selected_tab": state["selected_tab"], "surface": state["surface"]})
        return state

    def expect_chrome_field(field):
        layout = control / "chrome-layout.json"
        layout.unlink(missing_ok=True)

        def read():
            if layout.exists():
                current = json.loads(layout.read_text())
                layout.unlink()
                if current and current["focused_control"] == field:
                    return current
            send_command(control, "inspect_chrome")
            return None

        until(process, f"WebKit focuses {field}", read, timeout=6)
        steps.append({"focused_chrome_field": field})

    address = first.origin + "/first?keyboard"
    send_command(control, "navigate", value=address)
    initial = expect("keyboard page is ready", lambda s: s.get("url") == address and s.get("status") == "ready")
    initial_count = len(initial["tabs"])
    first_id = initial["tabs"][0]["id"]
    send_command(control, "focus_page")
    opened = press("new_tab", "Command+T opens Start", lambda s: (
        len(s.get("tabs", [])) == initial_count + 1 and s.get("surface") == "start"
    ))
    if steps[-1]["first_responder_before"] != "content":
        raise AssertionError("the AppKit New Tab shortcut was not sent from the content responder")
    expect_chrome_field("address")
    new_id = opened["selected_tab"]
    press("first_tab", "Command+1 selects the first tab", lambda s: s.get("selected_tab") == first_id)
    press("last_tab", "Command+9 selects the last tab", lambda s: s.get("selected_tab") == new_id)
    press("close_tab", "Command+W closes the selected tab", lambda s: len(s.get("tabs", [])) == initial_count)
    press("reopen_tab", "Shift+Command+T reopens the closed tab", lambda s: (
        len(s.get("tabs", [])) == initial_count + 1 and s.get("surface") == "start" and s.get("selected_tab") != new_id
    ))
    press("search_tabs", "Shift+Command+A opens tab search", lambda _s: focus_receipt.exists())
    expect_chrome_field("tab-search")
    press("downloads", "Shift+Command+J opens Downloads", lambda s: s.get("surface") == "downloads")


def exercise_chrome_themes(process, control, first, steps, capture):
    def command(action, **fields):
        send_command(control, action, **fields)

    path = control / "chrome-layout.json"
    for theme, expected in THEME_BASE_COLORS.items():
        command("set_theme", value=theme)
        address = f"{first.origin}/first?theme={theme}"
        command("navigate", value=address)
        until(process, f"browse toolbar in explicit {theme}", lambda: (
            (state := observation(control)).get("url") == address and state.get("status") == "ready"
            and state.get("surface") == "browse" and state.get("settings", {}).get("theme") == theme
        ))
        if capture is not None:
            capture(process, control, command, f"native-chrome-toolbar-{theme}")
        command("show_surface", value="start")
        path.unlink(missing_ok=True)
        command("inspect_chrome")
        until(process, f"{theme} native theme observation", path.is_file)
        layout = json.loads(path.read_text(encoding="utf-8"))
        if (not layout or layout["theme"] != theme or layout["heading"] != "Muffin"
                or any(layout[key] != value for key, value in expected.items())):
            raise AssertionError(f"wrong palette painted for {theme}: {layout}")

        contrast = {
            "surface_text": contrast_ratio(layout["surface_ink"], layout["surface_fill"]),
            "muted_surface_text": contrast_ratio(layout["muted_ink"], layout["surface_fill"]),
            "active_tab_text": contrast_ratio(layout["selected_tab_ink"], layout["selected_tab_fill"]),
            "focus_on_active_tab": contrast_ratio(layout["focus_ink"], layout["selected_tab_fill"]),
        }
        if (min(contrast[key] for key in ("surface_text", "muted_surface_text", "active_tab_text")) < 4.5
                or contrast["focus_on_active_tab"] < 3):
            raise AssertionError(f"{theme} palette contrast below the fixture minimum: {contrast}")
        label = f"native-chrome-theme-{theme}"
        steps.append({label: {"layout": layout, "base_color_contrast": contrast}})
        if capture is not None:
            capture(process, control, command, label)


def exercise_download_folder(process, control, first, steps, capture):
    def expect(label, predicate):
        return until(process, label, lambda: (state if predicate(state := observation(control)) else None))

    def approve(path):
        send_command(control, "navigate", value=first.origin + path)
        send_command(control, "show_surface", value="downloads")
        state = expect(f"{path} awaiting consent", lambda s: any(
            item["status"] == "awaiting_approval" for item in s.get("downloads", [])
        ))
        row = next(item for item in state["downloads"] if item["status"] == "awaiting_approval")
        send_command(control, "decide_download", id=row["id"], approve=True)
        return row["id"]

    def finish(download_id):
        state = expect(f"download {download_id} published", lambda s: any(
            item["id"] == download_id and item["status"] == "finished" for item in s.get("downloads", [])
        ))
        return next(item["name"] for item in state["downloads"] if item["id"] == download_id)

    original = control / "downloads"
    alternate = control / "alternate-downloads"
    alternate.mkdir()
    (alternate / "shared.bin").write_bytes(b"keep fixture user file")
    active = approve(SLOW_DOWNLOAD)
    expect("the original folder has an active transfer", lambda s: any(
        item["id"] == active and item["status"] == "downloading" and item["bytes"] > 0
        for item in s.get("downloads", [])
    ))
    send_command(control, "show_surface", value="settings")
    send_command(control, "choose_fixture_download_folder")
    expect("the chosen folder is displayed by name", lambda s: s.get("download_folder") == alternate.name)
    layout = control / "chrome-layout.json"
    layout.unlink(missing_ok=True)
    send_command(control, "inspect_chrome")
    until(process, "Settings WebKit layout after choosing a folder", layout.is_file)
    if json.loads(layout.read_text())["visible_download_folder"] != alternate.name:
        raise AssertionError("Settings did not render the chosen folder name")
    if capture is not None:
        capture(process, control, lambda action, **fields: send_command(control, action, **fields), "native-download-folder-setting")
    first.release_slow_download.set()
    send_command(control, "show_surface", value="downloads")
    retained = original / finish(active)
    if retained.read_bytes() != SLOW_DOWNLOAD_CHUNK * 2:
        raise AssertionError("the active transfer did not keep its original destination and data")
    chosen = alternate / finish(approve("/download/alpha"))
    if chosen.name != "shared (1).bin" or chosen.read_bytes() != DOWNLOAD_PAYLOADS["/download/alpha"]:
        raise AssertionError("the next transfer did not publish to the chosen folder without clobbering")
    if (alternate / "shared.bin").read_bytes() != b"keep fixture user file":
        raise AssertionError("the chosen folder lost a pre-existing file")
    steps.append({"download_folder": {
        "chosen_name": alternate.name, "active_transfer_kept_original_folder": True,
        "next_transfer_used_chosen_folder": True, "collision_name": chosen.name, "original_file_preserved": True,
    }})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/debug/muffin"))
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--tabs", action="store_true", help="include M2 tab and native window lifecycle checks")
    parser.add_argument("--restart", action="store_true", help="also exercise disposable session restore")
    parser.add_argument("--profile", action="store_true", help="exercise the disposable local profile")
    parser.add_argument("--page-tools", action="store_true", help="verify native find, zoom and downloads")
    parser.add_argument("--chrome-layout", action="store_true", help="verify default and compact native chrome geometry")
    parser.add_argument("--chrome-themes", action="store_true", help="verify light and dark native chrome palettes")
    parser.add_argument("--native-shortcuts", action="store_true", help="verify AppKit shortcut routing and chrome focus")
    parser.add_argument("--download-folder", action="store_true", help="verify destination changes and persistence")
    visual = parser.add_mutually_exclusive_group()
    visual.add_argument("--capture-window", action="store_true", help="attempt a capture of the fixture window")
    visual.add_argument("--capture-chrome", action="store_true", help="capture the native trusted chrome view")
    args = parser.parse_args()
    if args.restart and not args.tabs:
        parser.error("--restart requires --tabs")
    if args.profile and not args.restart:
        parser.error("--profile requires --restart")
    if args.download_folder and not args.restart:
        parser.error("--download-folder requires --restart")
    binary = args.binary.resolve(strict=True)
    args.out.mkdir(parents=True, exist_ok=True)
    with binary.open("rb") as stream:
        binary_sha = file_digest(stream, "sha256").hexdigest()
    with Path("/System/Library/Frameworks/WebKit.framework/Resources/Info.plist").open("rb") as stream:
        webkit_version = plistlib.load(stream)["CFBundleVersion"]
    result = {
        "source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
        "source_worktree_clean": not subprocess.check_output(
            ["git", "status", "--porcelain"], text=True
        ).strip(),
        "binary_sha256": binary_sha,
        "host_arch": platform.machine(),
        "macos_version": platform.mac_ver()[0],
        "webkit_bundle_version": webkit_version,
        "kind": "development_native_debug_loopback",
        "steps": [],
    }
    capture = None
    if args.capture_window:
        def capture(_process, control, _command, name="muffin-window"):
            observation = json.loads((control / "state.json").read_text(encoding="utf-8"))
            window_number = observation["window_number"]
            image = args.out / f"{name}.png"
            if not isinstance(window_number, int) or window_number <= 0:
                result["window_capture"] = "unavailable_window_number"
                return
            capture_result = subprocess.run(
                ["/usr/sbin/screencapture", "-x", "-o", "-l", str(window_number), str(image)],
                timeout=12, capture_output=True,
            )
            if capture_result.returncode != 0 or not image.is_file() or not image.stat().st_size:
                result["window_capture"] = "capture_failed"
                result["window_capture_diagnostic"] = capture_result.stderr.decode(errors="replace").strip()
                return
            with image.open("rb") as stream:
                result["window_image_sha256"] = file_digest(stream, "sha256").hexdigest()
            result["window_capture"] = "captured"
    if args.capture_chrome:
        def capture(process, control, command, name="native-chrome"):
            snapshot = control / "chrome.tiff"
            snapshot.unlink(missing_ok=True)
            image = args.out / f"{name}.png"
            command("capture_chrome")
            until(process, "native trusted chrome image", lambda: snapshot.is_file() and snapshot.stat().st_size)
            subprocess.run(
                ["/usr/bin/sips", "-s", "format", "png", str(snapshot), "--out", str(image)],
                check=True, capture_output=True, timeout=10,
            )
            with image.open("rb") as stream:
                checksum = file_digest(stream, "sha256").hexdigest()
            if name == "native-chrome":
                result["chrome_image_sha256"] = checksum
            else:
                result.setdefault("additional_capture_sha256", {})[name] = checksum

    try:
        with TemporaryDirectory(prefix="muffin-smoke-", dir="/private/tmp") as temporary, browser_fixture() as servers:
            control = Path(temporary)
            with (args.out / "browser.log").open("wb") as log:
                launch = [str(binary), "--test-control", str(control)]
                if args.restart:
                    launch.extend(("--test-session", str(control / "session.json")))
                process = subprocess.Popen(launch, stdout=log, stderr=log)
                try:
                    exercise(
                        process, control, *servers, result["steps"],
                        None if args.tabs else capture,
                    )
                    if args.tabs:
                        exercise_tabs(process, control, *servers, result["steps"], capture)
                    if args.restart:
                        verify_instance_forwarding(process, control, launch, *servers, result["steps"])
                        send_command(control, "quit_test")
                        if process.wait(timeout=8) != 0:
                            raise RuntimeError("graceful fixture shutdown failed")
                        (control / "commands.jsonl").unlink()
                        (control / "state.json").unlink()
                        process = subprocess.Popen(launch, stdout=log, stderr=log)
                        verify_session_restore(process, control, *servers, result["steps"])
                    if args.profile:
                        exercise_profile(process, control, *servers, result["steps"], capture)
                        send_command(control, "quit_test")
                        if process.wait(timeout=8) != 0:
                            raise RuntimeError("profile fixture shutdown failed")
                        (control / "commands.jsonl").unlink()
                        (control / "state.json").unlink()
                        process = subprocess.Popen(launch, stdout=log, stderr=log)
                        verify_profile_restart(process, control, result["steps"])
                    if args.page_tools:
                        exercise_page_tools(
                            process, control, *servers, result["steps"], capture,
                            args.out / "native-fixture.pdf" if args.capture_chrome else None,
                        )
                        exercise_media(process, control, servers[0], result["steps"])
                    if args.chrome_themes:
                        exercise_chrome_themes(process, control, servers[0], result["steps"], capture)
                    if args.native_shortcuts:
                        exercise_native_shortcuts(process, control, servers[0], result["steps"])
                    if args.download_folder:
                        exercise_download_folder(process, control, servers[0], result["steps"], capture)
                        send_command(control, "quit_test")
                        if process.wait(timeout=8) != 0:
                            raise RuntimeError("download folder fixture shutdown failed")
                        (control / "commands.jsonl").unlink()
                        (control / "state.json").unlink()
                        process = subprocess.Popen(launch, stdout=log, stderr=log)
                        until(process, "download folder survives restart", lambda: (
                            (state := observation(control)).get("chrome_ready")
                            and state.get("download_folder") == "alternate-downloads"
                        ))
                        (control / "alternate-downloads").rename(control / "detached-downloads")
                        send_command(control, "show_surface", value="settings")
                        until(process, "Settings warns when the saved folder is unavailable", lambda: (
                            (state := observation(control)).get("surface") == "settings"
                            and "download folder is unavailable" in state.get("notice", "")
                        ))
                        send_command(control, "reset_download_folder")
                        until(process, "Reset restores default download folder", lambda: (
                            (state := observation(control)).get("surface") == "settings"
                            and "download_folder" in state and state["download_folder"] is None
                        ))
                        result["steps"].append({"download_folder_reopened": True, "missing_folder_warned_in_settings": True, "reset_to_default": True})
                    if args.chrome_layout:
                        exercise_chrome_layout(process, control, result["steps"], capture)
                except Exception:
                    state_path = control / "state.json"
                    if state_path.is_file():
                        result["last_observation"] = json.loads(state_path.read_text(encoding="utf-8"))
                    raise
                finally:
                    if process.poll() is None:
                        process.terminate()
                        try:
                            process.wait(timeout=5)
                        except subprocess.TimeoutExpired:
                            process.kill()
                            process.wait(timeout=5)
    except Exception as exc:
        result.update(outcome="failed", error=str(exc))
        raise
    else:
        result["outcome"] = "passed"
    finally:
        (args.out / "smoke.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(f"Native debug smoke passed ({args.out / 'smoke.json'})")


if __name__ == "__main__":
    main()
