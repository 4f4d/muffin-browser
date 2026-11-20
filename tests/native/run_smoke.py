"""Exercise the Apple Silicon shell against a disposable loopback fixture."""

import argparse
from hashlib import file_digest
import json
from pathlib import Path
import platform
import plistlib
import subprocess
from tempfile import TemporaryDirectory
import time
import tomllib

from fixture import browser_fixture


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
        if theme == "light" and layout["selected_tab_fill"] != "rgb(234, 227, 217)":
            raise AssertionError(f"{heading} shows a tab fill outside the light palette: {layout}")
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/debug/muffin"))
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--tabs", action="store_true", help="include M2 tab and native window lifecycle checks")
    parser.add_argument("--restart", action="store_true", help="also exercise disposable session restore")
    parser.add_argument("--profile", action="store_true", help="exercise the disposable local profile")
    visual = parser.add_mutually_exclusive_group()
    visual.add_argument("--capture-window", action="store_true", help="attempt a capture of the fixture window")
    visual.add_argument("--capture-chrome", action="store_true", help="capture the native trusted chrome view")
    args = parser.parse_args()
    if args.restart and not args.tabs:
        parser.error("--restart requires --tabs")
    if args.profile and not args.restart:
        parser.error("--profile requires --restart")
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
