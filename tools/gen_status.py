#!/usr/bin/env python3
"""Generate docs/STATUS.json from result records. Never hand-edit STATUS.json.

Inputs
  docs/CURRENT_MILESTONE            one line, e.g. "M0a"
  docs/reports/results/*.json       list of platform-level records
  docs/reports/m0/results.json      human-recorded M0b manual test results
  docs/reports/m0/spikes/*.md       M0a spike reports with a validated time-budget header

M0b evidence-bearing results (pass/fail/partial/inconclusive) must identify the
exact app build and binary, environment, tester, date, and a non-empty evidence
file present inside this repository, outside evidence-private/.

Usage
  python3 tools/gen_status.py          # regenerate docs/STATUS.json
  python3 tools/gen_status.py --check  # exit 1 on drift, 2 on invalid records
"""
import datetime as dt
import glob
import hashlib
import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
PLATFORMS = ["macos", "windows", "linux_x11", "linux_wayland"]
LEVELS = ["build", "smoke", "functional", "release"]
RESULTS = {"pass", "fail", "not_run"}
M0B_RESULTS = {"pass", "fail", "partial", "not_run", "inconclusive"}
M0B_EVIDENCE_RESULTS = {"pass", "fail", "partial", "inconclusive"}
ENVIRONMENTS = {"bare_metal", "vm", "ci_runner"}
SHA = re.compile(r"^[0-9a-f]{7,40}$")
FULL_SHA = re.compile(r"^[0-9a-f]{40}$")
SHA256 = re.compile(r"^[0-9a-f]{64}$")
RUN_URL = re.compile(r"^https://github\.com/[^/\s]+/[^/\s]+/actions/runs/\d+(?:/attempts/\d+)?$")
# Platform-level evidence must live in one of these explicit artifact areas.
# M0b has its separate docs/reports/m0/evidence/ path policy.
LEVEL_ARTIFACT_DIRS = ("docs/reports/ci", "docs/reports/platform")
# spike id -> (per-report cap in minutes, aggregate cap in minutes); mirrors docs/PLAN.md section M0a
SPIKE_CAPS = {
    "engine_floor": (45, 180),
    "S1": (90, 720),
    "S2a": (60, 60),
    "S3": (60, 60),
    "S4": (90, 360),
    "S5": (60, 240),
    "S7": (60, 60),
    "S8": (45, 240),
    "CI": (60, 120),
}
SPIKE_OUTCOMES = {"pass", "fail", "inconclusive", "blocked", "not_run"}
STOP_REASONS = {"completed", "deadline", "blocked"}
SPIKE_START_REQUIRED = (
    "spike", "platform", "substrate", "budget_minutes", "started_at", "deadline_at",
)
SPIKE_COMPLETION_REQUIRED = ("stopped_at", "stopped_reason", "outcome")
SPIKE_REQUIRED = SPIKE_START_REQUIRED + SPIKE_COMPLETION_REQUIRED
DEADLINE_GRACE = dt.timedelta(minutes=2)
GIT_SLACK = dt.timedelta(minutes=5)
M0B_REQUIRED_FIELDS = (
    "app_commit",
    "binary_sha256",
    "host_architecture",
    "binary_architecture",
    "os_version",
    "engine_version",
    "tester",
    "date",
    "evidence",
)
PLACEHOLDER_VALUES = {
    "",
    "unknown",
    "n/a",
    "na",
    "none",
    "null",
    "not recorded",
    "pending",
    "todo",
    "tbd",
    "fill in",
    "fill-in",
    "<version>",
    "<sha>",
    "<handle>",
    "reviewer-handle",
}
PLACEHOLDER_TOKENS = (
    "replace_with",
    "replace with",
    "exact version",
    "exact build",
    "your handle",
    "<",
    ">",
)


class Invalid(Exception):
    pass


def load(path):
    try:
        return json.loads(pathlib.Path(path).read_text(encoding="utf-8"))
    except Exception as e:  # noqa: BLE001
        raise Invalid(f"{path}: cannot parse JSON: {e}") from e


def non_placeholder(value, field, src, test_id):
    if not isinstance(value, str):
        raise Invalid(f"{src}: {test_id}: '{field}' must contain a real, non-placeholder value")
    normalized = value.strip().casefold()
    if normalized in PLACEHOLDER_VALUES or any(token in normalized for token in PLACEHOLDER_TOKENS):
        raise Invalid(f"{src}: {test_id}: '{field}' must contain a real, non-placeholder value")
    return value.strip()


def validate_evidence_path(value, src, test_id):
    if not isinstance(value, str) or not value.strip():
        raise Invalid(f"{src}: {test_id}: evidence must be a repository-relative file path")

    # Evidence paths are stored with POSIX separators so records remain portable.
    raw = value.strip()
    rel = pathlib.PurePosixPath(raw)
    if rel.is_absolute() or "\\" in raw or any(part in ("", ".", "..") for part in rel.parts):
        raise Invalid(f"{src}: {test_id}: evidence path must be a clean repository-relative path: {raw!r}")
    if "evidence-private" in {part.casefold() for part in rel.parts}:
        raise Invalid(f"{src}: {test_id}: evidence must not be inside evidence-private/")

    repo = ROOT.resolve()
    candidate = ROOT.joinpath(*rel.parts)
    try:
        resolved = candidate.resolve(strict=True)
    except (FileNotFoundError, OSError, RuntimeError) as e:
        raise Invalid(f"{src}: {test_id}: evidence file does not exist: {raw}") from e

    try:
        resolved.relative_to(repo)
    except ValueError as e:
        raise Invalid(f"{src}: {test_id}: evidence resolves outside the repository: {raw}") from e

    # Check again after resolving symlinks; a normal-looking in-repo path must
    # not be allowed to point into evidence-private/ or outside the checkout.
    resolved_rel = resolved.relative_to(repo)
    if "evidence-private" in {part.casefold() for part in resolved_rel.parts}:
        raise Invalid(f"{src}: {test_id}: evidence resolves inside evidence-private/")
    if not resolved.is_file() or resolved.stat().st_size <= 0:
        raise Invalid(f"{src}: {test_id}: evidence must be an existing, non-empty file: {raw}")
    return resolved


def validate_level_artifact_path(value, src, record_id):
    """Validate a platform-level artifact and restrict it to designated report areas."""
    resolved = validate_evidence_path(value, src, record_id)
    raw_rel = pathlib.PurePosixPath(value.strip()).as_posix()
    allowed = [directory for directory in LEVEL_ARTIFACT_DIRS if raw_rel.startswith(directory + "/")]
    if not allowed:
        allowed_text = ", ".join(directory + "/" for directory in LEVEL_ARTIFACT_DIRS)
        raise Invalid(
            f"{src}: {record_id}: platform artifact must be inside a designated reports artifact directory: {allowed_text}"
        )

    resolved_rel = resolved.relative_to(ROOT.resolve()).as_posix()
    # A symlink must remain inside the same designated area, not merely somewhere
    # else in the repository or in a different approved area.
    if not any(resolved_rel.startswith(directory + "/") for directory in allowed):
        raise Invalid(f"{src}: {record_id}: artifact resolves outside its designated reports artifact directory")
    return resolved


def validate_m0b_result(r, test, src):
    test_id = test["id"]
    if not isinstance(r, dict):
        raise Invalid(f"{src}: {test_id}: each result must be a JSON object")
    for key in ("platform", "environment", "result"):
        if key not in r:
            raise Invalid(f"{src}: {test_id}: result is missing '{key}'")

    if not isinstance(r["platform"], str) or r["platform"] not in PLATFORMS:
        raise Invalid(f"{src}: {test_id}: invalid platform {r['platform']!r}")
    if r["platform"] not in test["platforms"]:
        raise Invalid(f"{src}: {test_id}: platform {r['platform']!r} not applicable")
    if not isinstance(r["result"], str) or r["result"] not in M0B_RESULTS:
        raise Invalid(f"{src}: {test_id}: bad result {r['result']!r}")
    if not isinstance(r["environment"], str) or r["environment"] not in ENVIRONMENTS:
        raise Invalid(f"{src}: {test_id}: bad environment {r['environment']!r}")

    # Tests marked vm_valid=false are bare-metal-only. Reject VM and CI records
    # for every outcome, not only 'pass', so invalid observations cannot enter
    # the human decision summary under another result label.
    if test["vm_valid"] is False and r["environment"] != "bare_metal":
        raise Invalid(
            f"{src}: {test_id}: bare-metal-only test rejects environment "
            f"{r['environment']!r}; use bare_metal"
        )

    if r["result"] == "not_run":
        return

    for field in M0B_REQUIRED_FIELDS:
        if field not in r:
            raise Invalid(f"{src}: {test_id}: {r['result']} result is missing required '{field}'")

    app_commit = non_placeholder(r["app_commit"], "app_commit", src, test_id)
    if not FULL_SHA.fullmatch(app_commit):
        raise Invalid(f"{src}: {test_id}: app_commit must be a full 40-character lowercase Git SHA")
    if not commit_exists(app_commit):
        raise Invalid(
            f"{src}: {test_id}: app_commit {app_commit} is not present in this repository's Git object database"
        )

    binary_hash = non_placeholder(r["binary_sha256"], "binary_sha256", src, test_id)
    if not SHA256.fullmatch(binary_hash):
        raise Invalid(f"{src}: {test_id}: binary_sha256 must be 64 lowercase hexadecimal characters")

    for field in (
        "host_architecture",
        "binary_architecture",
        "os_version",
        "engine_version",
        "tester",
    ):
        non_placeholder(r[field], field, src, test_id)

    # Require numeric version components so generic labels such as "macOS"
    # or "WebView2" cannot masquerade as exact version metadata.
    for field in ("os_version", "engine_version"):
        if not re.search(r"\d+(?:\.\d+)+", r[field]):
            raise Invalid(f"{src}: {test_id}: '{field}' must include an exact numeric version")

    date_value = non_placeholder(r["date"], "date", src, test_id)
    try:
        parsed_date = dt.date.fromisoformat(date_value)
    except ValueError as e:
        raise Invalid(f"{src}: {test_id}: date must be a real ISO date in YYYY-MM-DD format") from e
    if parsed_date.isoformat() != date_value:
        raise Invalid(f"{src}: {test_id}: date must use YYYY-MM-DD format")

    validate_evidence_path(r["evidence"], src, test_id)


def commit_exists(sha):
    """True only if `sha` is a commit object in this repository's history."""
    try:
        r = subprocess.run(
            ["git", "-C", str(ROOT), "cat-file", "-e", f"{sha}^{{commit}}"],
            capture_output=True, timeout=30,
        )
    except (OSError, subprocess.SubprocessError):
        return False
    return r.returncode == 0


def file_sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def iso_date(value, field, src, label):
    value = non_placeholder(value, field, src, label)
    try:
        parsed = dt.date.fromisoformat(value)
    except ValueError as e:
        raise Invalid(f"{src}: {label}: {field} must be a real ISO date in YYYY-MM-DD format") from e
    if parsed.isoformat() != value:
        raise Invalid(f"{src}: {label}: {field} must use YYYY-MM-DD format")
    return value


def validate_level_record(r, src):
    """Platform-level records (build/smoke/functional/release) drive 'verified' claims,
    so pass AND fail records need the same evidence rigour as M0b results."""
    if not isinstance(r, dict):
        raise Invalid(f"{src}: each record must be a JSON object")
    for k in ("id", "platform", "level", "result", "environment", "date"):
        if k not in r:
            raise Invalid(f"{src}: record missing '{k}': {r}")
    label = str(r["id"])
    if not isinstance(r["id"], str) or not r["id"].strip():
        raise Invalid(f"{src}: record id must be a non-empty string")
    if r["platform"] not in PLATFORMS:
        raise Invalid(f"{src}: bad platform {r['platform']!r}")
    if r["level"] not in LEVELS:
        raise Invalid(f"{src}: bad level {r['level']!r}")
    if r["result"] not in RESULTS:
        raise Invalid(f"{src}: bad result {r['result']!r}")
    if r["environment"] not in ENVIRONMENTS:
        raise Invalid(f"{src}: bad environment {r['environment']!r}")
    iso_date(r["date"], "date", src, label)
    if r["result"] == "not_run":
        return

    commit = non_placeholder(r.get("commit"), "commit", src, label)
    if not FULL_SHA.fullmatch(commit):
        raise Invalid(f"{src}: {label}: commit must be a full 40-character lowercase Git SHA")
    if not commit_exists(commit):
        raise Invalid(
            f"{src}: {label}: commit {commit} is not in this repository's history "
            "(CI checkouts must use fetch-depth: 0)"
        )

    resolved = validate_level_artifact_path(r.get("artifact"), src, label)
    digest = non_placeholder(r.get("artifact_sha256"), "artifact_sha256", src, label)
    if not SHA256.fullmatch(digest):
        raise Invalid(f"{src}: {label}: artifact_sha256 must be 64 lowercase hexadecimal characters")
    if file_sha256(resolved) != digest:
        raise Invalid(f"{src}: {label}: artifact_sha256 does not match the artifact file contents")

    if r["environment"] == "ci_runner":
        url = r.get("run_url")
        if not isinstance(url, str) or not RUN_URL.fullmatch(url.strip()):
            raise Invalid(
                f"{src}: {label}: ci_runner records need run_url "
                "https://github.com/<owner>/<repo>/actions/runs/<id>"
            )
    else:
        non_placeholder(r.get("tester"), "tester", src, label)
    if r["environment"] == "vm" and r["level"] == "release":
        raise Invalid(f"{src}: a VM cannot satisfy level 'release' ({label})")


def level_status():
    plat = {p: {l: {"result": "not_run"} for l in LEVELS} for p in PLATFORMS}
    latest = {}
    seen_ids = set()
    for f in sorted(glob.glob(str(ROOT / "docs/reports/results/*.json"))):
        recs = load(f)
        if not isinstance(recs, list):
            raise Invalid(f"{f}: expected a list of records")
        for r in recs:
            validate_level_record(r, f)
            if r["id"] in seen_ids:
                raise Invalid(f"{f}: duplicate record id {r['id']!r}")
            seen_ids.add(r["id"])
            key = (r["platform"], r["level"])
            order = (r["date"], r["id"])
            if key not in latest or order >= latest[key][0]:
                latest[key] = (order, r)
    for (p, l), (_, r) in latest.items():
        entry = {"result": r["result"], "environment": r["environment"], "date": r["date"]}
        if r.get("commit"):
            entry["commit"] = r["commit"]
        if r.get("artifact"):
            entry["artifact"] = r["artifact"]
        if r.get("artifact_sha256"):
            entry["artifact_sha256"] = r["artifact_sha256"]
        if r.get("run_url"):
            entry["run_url"] = r["run_url"]
        plat[p][l] = entry
    return plat


def m0b_status():
    path = ROOT / "docs/reports/m0/results.json"
    if not path.exists():
        return {}
    data = load(path)
    if not isinstance(data, dict) or not isinstance(data.get("tests"), list):
        raise Invalid(f"{path}: expected an object containing a tests list")

    summary = {}
    seen_test_ids = set()
    for t in data["tests"]:
        if not isinstance(t, dict):
            raise Invalid(f"{path}: each test entry must be an object")
        for k in ("id", "vm_valid", "platforms"):
            if k not in t:
                raise Invalid(f"{path}: test missing '{k}': {t.get('id')}")
        if not isinstance(t["id"], str) or not t["id"].strip():
            raise Invalid(f"{path}: test id must be a non-empty string")
        if t["id"] in seen_test_ids:
            raise Invalid(f"{path}: duplicate test id {t['id']!r}")
        seen_test_ids.add(t["id"])
        if not isinstance(t["vm_valid"], bool):
            raise Invalid(f"{path}: {t['id']}: vm_valid must be boolean")
        if not isinstance(t["platforms"], list) or not t["platforms"]:
            raise Invalid(f"{path}: {t['id']}: platforms must be a non-empty list")
        if any(not isinstance(p, str) or p not in PLATFORMS for p in t["platforms"]):
            raise Invalid(f"{path}: {t['id']}: invalid platform in platforms list")
        if len(set(t["platforms"])) != len(t["platforms"]):
            raise Invalid(f"{path}: {t['id']}: duplicate platform in platforms list")
        if not isinstance(t.get("results", []), list):
            raise Invalid(f"{path}: {t['id']}: results must be a list")

        counts = {k: 0 for k in sorted(M0B_RESULTS)}
        recorded = {}
        for r in t.get("results", []):
            validate_m0b_result(r, t, path)
            platform = r["platform"]
            if platform in recorded:
                raise Invalid(f"{path}: {t['id']}: duplicate result for platform {platform!r}")
            recorded[platform] = r["result"]
        for res in recorded.values():
            counts[res] += 1
        summary[t["id"]] = {
            "applicable_platforms": len(t["platforms"]),
            "recorded": len(recorded),
            "counts": {k: v for k, v in counts.items() if v},
        }
    return summary


def parse_front_matter_text(text, src):
    if not text.startswith("---\n"):
        raise Invalid(f"{src}: spike report must start with a '---' front-matter header")
    end = text.find("\n---", 4)
    if end == -1:
        raise Invalid(f"{src}: spike report front-matter header is not closed with '---'")
    data = {}
    for line in text[4:end].splitlines():
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        if ":" not in line:
            raise Invalid(f"{src}: bad front-matter line {line!r}")
        key, _, value = line.partition(":")
        data[key.strip()] = value.strip()
    return data


def parse_front_matter(path):
    return parse_front_matter_text(pathlib.Path(path).read_text(encoding="utf-8"), path)


def committed_front_matter(commit_sha, relative_path, src):
    """Read a report's front matter from a specific committed tree snapshot."""
    result = subprocess.run(
        ["git", "-C", str(ROOT), "show", f"{commit_sha}:{relative_path}"],
        capture_output=True, text=True, timeout=30,
    )
    if result.returncode != 0:
        raise Invalid(f"{src}: cannot read the initial header commit's report snapshot")
    return parse_front_matter_text(result.stdout, f"{src} at commit {commit_sha[:12]}")


def parse_timestamp(value, field, src):
    try:
        parsed = dt.datetime.fromisoformat(str(value).strip().replace("Z", "+00:00"))
    except ValueError as e:
        raise Invalid(f"{src}: {field} must be an ISO-8601 timestamp with timezone") from e
    if parsed.tzinfo is None:
        raise Invalid(f"{src}: {field} must include a timezone (use Z or an offset)")
    return parsed


def git_commit_history(path):
    """Return distinct (commit SHA, committer time) entries for a tracked, clean report.

    Git timestamps are corroborating metadata only; they are not a trusted clock.
    """
    try:
        rel = pathlib.Path(path).resolve().relative_to(ROOT.resolve()).as_posix()
    except ValueError as e:
        raise Invalid(f"{path}: spike report is outside the repository") from e

    tracked = subprocess.run(
        ["git", "-C", str(ROOT), "ls-files", "--error-unmatch", "--", rel],
        capture_output=True, text=True, timeout=30,
    )
    if tracked.returncode != 0 or not tracked.stdout.strip():
        raise Invalid(f"{path}: spike report must be tracked in Git; commit its initial header before starting")

    status = subprocess.run(
        ["git", "-C", str(ROOT), "status", "--porcelain", "--untracked-files=all", "--", rel],
        capture_output=True, text=True, timeout=30,
    )
    if status.returncode != 0:
        raise Invalid(f"{path}: cannot inspect Git status for spike report")
    if status.stdout.strip():
        raise Invalid(f"{path}: spike report has uncommitted changes; commit the final report after stopping")

    result = subprocess.run(
        ["git", "-C", str(ROOT), "log", "--format=%H%x09%cI", "--", rel],
        capture_output=True, text=True, timeout=30,
    )
    if result.returncode != 0:
        raise Invalid(f"{path}: cannot read spike report Git history")

    entries = []
    seen = set()
    for line in result.stdout.splitlines():
        if not line.strip():
            continue
        parts = line.split("\t", 1)
        if len(parts) != 2:
            raise Invalid(f"{path}: malformed Git history entry for spike report")
        sha, timestamp = parts
        if sha in seen:
            continue
        seen.add(sha)
        entries.append((sha, parse_timestamp(timestamp, "git commit time", path)))
    entries.sort(key=lambda item: item[1])
    if not entries:
        raise Invalid(f"{path}: spike report has no committed Git history")
    return entries


def spike_status():
    files = sorted(
        p for p in glob.glob(str(ROOT / "docs/reports/m0/spikes/*.md"))
        if not pathlib.Path(p).name.startswith(("_", "README"))
    )
    reports = {}
    totals = {}
    for f in files:
        h = parse_front_matter(f)
        for k in SPIKE_START_REQUIRED:
            if not h.get(k):
                raise Invalid(f"{f}: spike header is missing required '{k}'")

        completion_present = [bool(h.get(k)) for k in SPIKE_COMPLETION_REQUIRED]
        if any(completion_present) and not all(completion_present):
            missing = SPIKE_COMPLETION_REQUIRED[completion_present.index(False)]
            raise Invalid(f"{f}: spike header is missing required '{missing}'")
        in_progress = not any(completion_present)

        spike = h["spike"]
        if spike not in SPIKE_CAPS:
            raise Invalid(f"{f}: unknown spike {spike!r}; expected one of {sorted(SPIKE_CAPS)}")
        if h["platform"] not in PLATFORMS:
            raise Invalid(f"{f}: bad platform {h['platform']!r}")
        if not in_progress and h["outcome"] not in SPIKE_OUTCOMES:
            raise Invalid(f"{f}: bad outcome {h['outcome']!r}")
        if not in_progress and h["stopped_reason"] not in STOP_REASONS:
            raise Invalid(f"{f}: bad stopped_reason {h['stopped_reason']!r}")
        try:
            budget = int(h["budget_minutes"])
        except ValueError as e:
            raise Invalid(f"{f}: budget_minutes must be an integer") from e
        per_cap, agg_cap = SPIKE_CAPS[spike]
        if not 1 <= budget <= per_cap:
            raise Invalid(f"{f}: budget_minutes {budget} exceeds the {per_cap}-minute cap for spike {spike}")

        extension = 0
        if h.get("extension_minutes"):
            try:
                extension = int(h["extension_minutes"])
            except ValueError as e:
                raise Invalid(f"{f}: extension_minutes must be an integer") from e
            if extension <= 0:
                raise Invalid(f"{f}: extension_minutes must be positive")
            non_placeholder(h.get("extension_approved_by"), "extension_approved_by", f, spike)
            non_placeholder(h.get("extension_reason"), "extension_reason", f, spike)

        started = parse_timestamp(h["started_at"], "started_at", f)
        deadline = parse_timestamp(h["deadline_at"], "deadline_at", f)
        expected = started + dt.timedelta(minutes=budget + extension)
        if abs((deadline - expected).total_seconds()) > 60:
            raise Invalid(f"{f}: deadline_at must equal started_at + budget_minutes (+ approved extension)")

        stopped = None
        if not in_progress:
            stopped = parse_timestamp(h["stopped_at"], "stopped_at", f)
            if stopped < started:
                raise Invalid(f"{f}: stopped_at is before started_at")
            if stopped > deadline + DEADLINE_GRACE:
                raise Invalid(f"{f}: stopped_at is after deadline_at; stop at the deadline and record partial findings")
            if h["outcome"] == "fail" and h["stopped_reason"] != "completed":
                raise Invalid(f"{f}: outcome 'fail' requires stopped_reason 'completed'; blocked or timed-out work is not a completed failure")
            if h["stopped_reason"] == "deadline":
                if stopped < deadline - DEADLINE_GRACE:
                    raise Invalid(f"{f}: stopped_reason 'deadline' but stopped_at is well before deadline_at")
                if h["outcome"] != "inconclusive":
                    raise Invalid(f"{f}: a spike stopped by its deadline must have outcome 'inconclusive', not {h['outcome']!r}")
            if h["stopped_reason"] == "blocked" and h["outcome"] not in ("blocked", "inconclusive"):
                raise Invalid(f"{f}: stopped_reason 'blocked' requires outcome 'blocked' or 'inconclusive', not {h['outcome']!r}")
            if h["outcome"] == "blocked" and h["stopped_reason"] != "blocked":
                raise Invalid(f"{f}: outcome 'blocked' requires stopped_reason 'blocked'")

        history = git_commit_history(f)
        first_commit_sha, first_commit_at = history[0]
        relative_report_path = pathlib.Path(f).resolve().relative_to(ROOT.resolve()).as_posix()
        initial_header = committed_front_matter(first_commit_sha, relative_report_path, f)
        for field in ("spike", "platform", "substrate", "budget_minutes", "started_at"):
            initial_value = initial_header.get(field)
            if not initial_value:
                raise Invalid(f"{f}: initial Git commit is missing header field '{field}'")
            if initial_value != h[field]:
                raise Invalid(f"{f}: initial Git header field '{field}' does not match the final report")
        initial_deadline_value = initial_header.get("deadline_at")
        if not initial_deadline_value:
            raise Invalid(f"{f}: initial Git commit is missing header field 'deadline_at'")
        initial_deadline = parse_timestamp(initial_deadline_value, "initial deadline_at", f)
        original_deadline = started + dt.timedelta(minutes=budget)
        if abs((initial_deadline - original_deadline).total_seconds()) > 60:
            raise Invalid(f"{f}: initial Git header deadline_at must equal started_at + original budget_minutes")
        if first_commit_at >= started:
            raise Invalid(f"{f}: initial header commit must be before started_at")
        if started - first_commit_at > GIT_SLACK:
            raise Invalid(f"{f}: initial header commit time must correspond to started_at within {int(GIT_SLACK.total_seconds() // 60)} minutes")

        if not in_progress:
            if len(history) < 2:
                raise Invalid(
                    f"{f}: completed spike report needs at least two distinct Git commits: an initial header commit before the spike and a final report commit after it"
                )
            final_commit_at = history[-1][1]
            if final_commit_at <= stopped:
                raise Invalid(f"{f}: final report commit must be after stopped_at")
            if final_commit_at - stopped > GIT_SLACK:
                raise Invalid(f"{f}: final report commit time must correspond to stopped_at within {int(GIT_SLACK.total_seconds() // 60)} minutes")

        key = f"{spike}:{h['platform']}:{h.get('substrate', 'n/a')}"
        if key in reports:
            raise Invalid(f"{f}: duplicate spike report for {key}; a restart needs a human-approved extension in the original report")
        totals[spike] = totals.get(spike, 0) + budget + extension
        if totals[spike] > agg_cap:
            raise Invalid(f"{f}: aggregate budget for spike {spike} exceeds {agg_cap} minutes")
        reports[key] = {
            "outcome": "in_progress" if in_progress else h["outcome"],
            "stopped_reason": "in_progress" if in_progress else h["stopped_reason"],
            "budget_minutes": budget + extension,
        }

    kill = sorted(
        k for k, v in reports.items()
        if k.split(":")[0] == "S1" and k.split(":")[1] in ("macos", "windows")
        and v["outcome"] == "fail" and v["stopped_reason"] == "completed"
    )
    needs_human = sorted(k for k, v in reports.items() if v["outcome"] in ("inconclusive", "blocked"))
    return {
        "reports": reports,
        "kill_rule": {"triggered": bool(kill), "triggering_reports": kill},
        "needs_human_decision": needs_human,
    }


def build():
    ms_file = ROOT / "docs/CURRENT_MILESTONE"
    milestone = ms_file.read_text(encoding="utf-8").strip() if ms_file.exists() else "unknown"
    return {
        "schema_version": 2,
        "project": "Muffin Browser",
        "current_milestone": milestone,
        "platforms": level_status(),
        "m0a_spikes": spike_status(),
        "m0b_manual_tests": m0b_status(),
        "generated_by": "tools/gen_status.py (do not hand-edit this file)",
    }


def main(argv):
    try:
        text = json.dumps(build(), indent=2, sort_keys=True) + "\n"
    except Invalid as e:
        print(f"INVALID: {e}", file=sys.stderr)
        return 2
    out = ROOT / "docs/STATUS.json"
    if "--check" in argv:
        current = out.read_text(encoding="utf-8") if out.exists() else ""
        if current != text:
            print(
                "DRIFT: docs/STATUS.json does not match generated output. "
                "Run: python3 tools/gen_status.py",
                file=sys.stderr,
            )
            return 1
        print("OK: docs/STATUS.json matches generated output")
        return 0
    out.write_text(text, encoding="utf-8")
    print("wrote docs/STATUS.json")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
