"""Synthetic acceptance contract for a future native chrome adapter."""
from dataclasses import dataclass
from urllib.parse import urlsplit

CHROME = "muffin-chrome://fixture/index.html"
CHROME_RESOURCES = {CHROME, "muffin-chrome://fixture/shell.css"}


@dataclass(frozen=True)
class BrowserFacts:
    """Inputs supplied by a native adapter, never taken from a web message."""

    view: str
    epoch: int
    committed_document: str
    main_frame: bool


def permits_command(active: BrowserFacts, sender: BrowserFacts, body: object) -> bool:
    return (
        active == sender
        and sender.main_frame
        and sender.committed_document == CHROME
        and isinstance(body, dict)
        and body == {"version": 1, "command": "FixtureStatus"}
        and type(body.get("version")) is int
    )


def permits_resource(method: str, url: str) -> bool:
    return method == "GET" and url in CHROME_RESOURCES


def permits_content_navigation(url: str, fixture_authorities: frozenset[str]) -> bool:
    try:
        parts = urlsplit(url)
        return (
            parts.scheme == "http"
            and parts.hostname == "127.0.0.1"
            and parts.username is None and parts.password is None
            and parts.netloc in fixture_authorities
        )
    except ValueError:
        return False
