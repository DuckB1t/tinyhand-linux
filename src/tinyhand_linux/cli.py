"""Command line entry points for TinyHand Linux."""

from __future__ import annotations

import json
import os
import socket
import sys
from pathlib import Path

from .paths import control_socket

CONTROL_COMMANDS = {
    "status", "on", "off", "toggle", "clear", "demo", "quit",
    "style", "sound", "settings", "section", "toolbar", "welcome",
}


def send_command(command: str) -> int:
    path = control_socket()
    if not path.exists():
        print(f"tinyhandctl: TinyHand is not running ({path})", file=sys.stderr)
        return 1
    client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    try:
        client.settimeout(2.0)
        client.connect(str(path))
        client.sendall(command.encode())
        data = b""
        while True:
            chunk = client.recv(4096)
            if not chunk:
                break
            data += chunk
    except OSError as exc:
        print(f"tinyhandctl: {exc}", file=sys.stderr)
        return 1
    finally:
        client.close()
    try:
        result = json.loads(data)
    except ValueError:
        print(data.decode(errors="replace").strip())
        return 0
    if isinstance(result, dict) and "error" in result:
        print(result["error"], file=sys.stderr)
        return 1
    if isinstance(result, dict):
        print("  ".join(f"{key}={value}" for key, value in result.items()))
    else:
        print(result)
    return 0


def apply_startup_flags(app, hidden: bool = False, settings: bool = False) -> None:
    """Apply launch-time flags before the event loop starts."""
    if hidden:
        # Autostart should be truly invisible, including the first-run card.
        app.active = False
        app.welcome_open = False
        app.ui.welcome_open = False
    if settings:
        app.settings_open = True
        app.ui.settings_open = True


def main() -> int:
    name = Path(sys.argv[0]).name
    args = sys.argv[1:]
    if args and args[0].lower().startswith("tinyhand://"):
        from urllib.parse import urlparse

        parsed = urlparse(args[0])
        command = parsed.hostname or "status"
        argument = parsed.path.strip("/").split("/", 1)[0]
        return send_command(f"{command} {argument}".strip())
    if name == "tinyhandctl" or (args and args[0] in CONTROL_COMMANDS):
        command = " ".join(args) or "status"
        return send_command(command)

    if not os.environ.get("WAYLAND_DISPLAY"):
        print("tinyhand: WAYLAND_DISPLAY is unset — start it inside a Wayland session", file=sys.stderr)
        return 1

    hidden = "--hidden" in args
    settings = "--settings" in args
    from .app import TinyHandApp

    app = TinyHandApp()
    # A running instance owns the control socket; ask it to open settings.
    if settings and control_socket().exists():
        return send_command("settings")
    apply_startup_flags(app, hidden=hidden, settings=settings)
    try:
        return app.run()
    except KeyboardInterrupt:
        return 0
    except Exception as exc:
        print(f"tinyhand: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
