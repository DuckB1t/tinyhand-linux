"""XDG paths used by TinyHand Linux."""

from __future__ import annotations

import os
from pathlib import Path


def xdg_dir(name: str, default: str) -> Path:
    return Path(os.environ.get(name, str(Path.home() / default))).expanduser()


def config_dir() -> Path:
    return xdg_dir("XDG_CONFIG_HOME", ".config") / "tinyhand-linux"


def config_file() -> Path:
    return config_dir() / "config.json"


def data_dir() -> Path:
    return xdg_dir("XDG_DATA_HOME", ".local/share") / "tinyhand-linux"


def runtime_dir() -> Path:
    return Path(os.environ.get("XDG_RUNTIME_DIR", "/tmp")).expanduser()


def control_socket() -> Path:
    return runtime_dir() / "tinyhand-linux.sock"


def lock_file() -> Path:
    return runtime_dir() / "tinyhand-linux.lock"


def state_file() -> Path:
    return runtime_dir() / "tinyhand-linux-state.json"


def log_file() -> Path:
    path = xdg_dir("XDG_STATE_HOME", ".local/state") / "tinyhand-linux" / "tinyhand.log"
    path.parent.mkdir(parents=True, exist_ok=True)
    return path


def asset_dir() -> Path:
    return Path(__file__).resolve().parent / "assets"


def protocol_dir() -> Path:
    return Path(__file__).resolve().parent / "protocol"
