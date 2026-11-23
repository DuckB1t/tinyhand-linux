"""Preferences, style catalogue and runtime state for TinyHand Linux.

The ids, titles, colours and defaults mirror TinyHand 1.1.0's
``PointerStyle.swift``, ``Preferences.swift`` and ``TapSound.swift``.
"""

from __future__ import annotations

import json
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

from .paths import config_file, runtime_dir, state_file


@dataclass(frozen=True)
class Style:
    id: str
    title: str
    ink: str
    asset: str | None = None

    @property
    def is_laser(self) -> bool:
        return self.asset is None


STYLES: tuple[Style, ...] = (
    Style("lavender", "Lavender toy", "#8754F2", "hand_lavender"),
    Style("glove", "Cursor glove", "#8754F2", "hand_glove"),
    Style("coral", "Coral vinyl", "#FF2920", "hand_coral"),
    Style("plush", "Plush", "#2F86F0", "hand_plush"),
    Style("robot", "Robot", "#16A87A", "hand_robot"),
    Style("pink", "Bubblegum", "#EE4F98", "hand_pink"),
    Style("blue", "Sky", "#2F86F0", "hand_blue"),
    Style("mint", "Mint", "#16A87A", "hand_mint"),
    Style("lemon", "Lemon", "#F2B705", "hand_lemon"),
    Style("black", "Midnight", "#1D1D21", "hand_black"),
    Style("chrome", "Chrome", "#6B7380", "hand_chrome"),
    Style("holo", "Holo", "#8754F2", "hand_holo"),
    Style("fuzzy", "Fuzzy", "#8754F2", "hand_fuzzy"),
    Style("wood", "Wood", "#9A5E2C", "hand_wood"),
    Style("marble", "Marble", "#6B7380", "hand_marble"),
    Style("terrazzo", "Terrazzo", "#F0561D", "hand_terrazzo"),
    Style("glass", "Glass", "#1597E5", "hand_glass"),
    Style("jelly", "Jelly", "#A33FE6", "hand_jelly"),
    Style("pastel", "Pastel", "#EE4F98", "hand_pastel"),
    Style("pixel", "Pixel", "#1D1D21", "hand_pixel"),
    Style("lowpoly", "Low poly", "#8754F2", "hand_lowpoly"),
    Style("knit", "Knit", "#8754F2", "hand_knit"),
    Style("skeleton", "Skeleton", "#9A5E2C", "hand_skeleton"),
    Style("astro", "Astronaut", "#2F86F0", "hand_astro"),
    Style("space", "Space", "#FF2920", "hand_space"),
    Style("monster", "Monster", "#3E9E2E", "hand_monster"),
    Style("water", "Water", "#1597E5", "hand_water"),
    Style("lava", "Lava", "#F0561D", "hand_lava"),
    Style("grass", "Grass", "#3E9E2E", "hand_grass"),
    Style("cactus", "Cactus", "#3E9E2E", "hand_cactus"),
    Style("choco", "Chocolate", "#9A5E2C", "hand_choco"),
    Style("cloud", "Cloud", "#2F86F0", "hand_cloud"),
    Style("mech", "Mech", "#F0561D", "hand_mech"),
    Style("neon", "Neon", "#E0309A", "hand_neon"),
    Style("leaf", "Leaf", "#3E9E2E", "hand_leaf"),
    Style("cookie", "Gingerbread", "#9A5E2C", "hand_cookie"),
    Style("laser", "Laser dot", "#FF2920", None),
)

STYLE_BY_ID = {style.id: style for style in STYLES}
DEFAULT_PINNED = ("lavender", "glove", "coral", "plush", "robot", "laser")

INK_CHOICES: tuple[tuple[str, str, str | None], ...] = (
    ("match", "Match pointer", None),
    ("red", "Red", "#FF2920"),
    ("orange", "Orange", "#FF8A00"),
    ("yellow", "Yellow", "#FFCC00"),
    ("green", "Green", "#30C25A"),
    ("blue", "Blue", "#0A84FF"),
    ("purple", "Purple", "#8754F2"),
    ("white", "White", "#FFFFFF"),
    ("black", "Black", "#1D1D21"),
)
INK_BY_ID = {item[0]: item for item in INK_CHOICES}

SOUNDS: tuple[tuple[str, str], ...] = (
    ("thock", "Thock"),
    ("click", "Click"),
    ("tock", "Tock"),
    ("clack", "Clack"),
    ("typewriter", "Typewriter"),
    ("snap", "Snap"),
    ("wood", "Wood block"),
    ("pop", "Pop"),
    ("boop", "Boop"),
    ("blip", "Blip"),
    ("coin", "Coin"),
    ("pew", "Pew"),
    ("squeak", "Squeak"),
    ("quack", "Quack"),
    ("chirp", "Chirp"),
)

INTERACT_KEYS = ("option", "command", "control", "off")
FADE_CHOICES = (3, 6, 10, 0)


def hex_rgb(value: str) -> tuple[float, float, float]:
    value = value.lstrip("#")
    return tuple(int(value[i : i + 2], 16) / 255 for i in (0, 2, 4))  # type: ignore[return-value]


def resolved_ink(choice: str, style_id: str) -> tuple[float, float, float]:
    colour = INK_BY_ID.get(choice, INK_BY_ID["match"])[2]
    if colour is None:
        colour = STYLE_BY_ID.get(style_id, STYLES[0]).ink
    return hex_rgb(colour)


@dataclass
class Settings:
    toggle_shortcut: str = "Ctrl+Alt+H"
    clear_shortcut: str = "Ctrl+Alt+X"
    pointer_size: float = 0.13
    show_shadow: bool = True
    fade_seconds: int = 3
    sound_hands: bool = True
    tap_sound: str = "thock"
    sound_laser: bool = False
    interact_keeps_hand: bool = False
    double_shift_click_through: bool = True
    interact_key: str = "option"
    ink_choice: str = "match"
    show_toolbar: bool = True
    show_ink_button: bool = True
    pinned_styles: list[str] = field(default_factory=lambda: list(DEFAULT_PINNED))
    pointer_style: str = "lavender"
    welcome_shown: bool = False
    first_run_hint_shown: bool = False
    _path: Path | None = field(default=None, repr=False, compare=False)

    @classmethod
    def load(cls, path: Path | None = None) -> "Settings":
        path = path or config_file()
        data: dict[str, Any] = {}
        try:
            if path.exists():
                data = json.loads(path.read_text())
        except (OSError, ValueError):
            data = {}
        known = {f for f in cls.__dataclass_fields__ if not f.startswith("_")}
        clean = {k: v for k, v in data.items() if k in known}
        if "pointer_style" in clean:
            clean["pointer_style"] = migrate_style(str(clean["pointer_style"]))
        clean["pinned_styles"] = [
            migrate_style(s)
            for s in clean.get("pinned_styles", list(DEFAULT_PINNED))
            if migrate_style(s) in STYLE_BY_ID
        ][:8] or list(DEFAULT_PINNED)
        obj = cls(**clean)
        obj._path = path
        return obj

    def save(self, path: Path | None = None) -> None:
        path = path or self._path or config_file()
        path.parent.mkdir(parents=True, exist_ok=True)
        data = asdict(self)
        data.pop("_path", None)
        path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")

    @property
    def style(self) -> Style:
        return STYLE_BY_ID.get(self.pointer_style, STYLES[0])

    def set_style(self, style_id: str) -> bool:
        if style_id not in STYLE_BY_ID:
            return False
        self.pointer_style = style_id
        return True

    def cycle_style(self, step: int = 1) -> str:
        ids = list(STYLE_BY_ID)
        current = ids.index(self.pointer_style) if self.pointer_style in ids else 0
        self.pointer_style = ids[(current + step) % len(ids)]
        return self.pointer_style

    def pin(self, style_id: str) -> bool:
        if style_id not in STYLE_BY_ID or style_id in self.pinned_styles:
            return False
        if len(self.pinned_styles) >= 8:
            return False
        self.pinned_styles.append(style_id)
        return True

    def unpin(self, style_id: str) -> bool:
        if style_id not in self.pinned_styles or len(self.pinned_styles) <= 1:
            return False
        self.pinned_styles.remove(style_id)
        return True

    def reset(self) -> None:
        defaults = Settings()
        for key, value in asdict(defaults).items():
            if key != "_path" and key not in ("welcome_shown", "first_run_hint_shown"):
                setattr(self, key, value)


def migrate_style(style_id: str) -> str:
    return {"dot": "laser", "sunny": "coral"}.get(style_id, style_id)


@dataclass
class RuntimeState:
    pid: int
    active: bool
    style: str
    sound_on: bool
    styles: list[dict[str, str]] = field(default_factory=list)
    strokes: int = 0
    version: str = "1.1.0"

    @classmethod
    def build(cls, pid: int, active: bool, style: str, sound_on: bool, strokes: int) -> "RuntimeState":
        return cls(
            pid=pid,
            active=active,
            style=style,
            sound_on=sound_on,
            styles=[
                {"id": s.id, "name": s.title, "type": "laser" if s.is_laser else "hand"}
                for s in STYLES
            ],
            strokes=strokes,
        )

    def write(self, path=None) -> None:
        path = path or state_file()
        try:
            path.parent.mkdir(parents=True, exist_ok=True)
            tmp = path.with_suffix(".tmp")
            tmp.write_text(json.dumps(asdict(self), indent=2, sort_keys=True) + "\n")
            tmp.replace(path)
        except OSError:
            pass


def remove_state() -> None:
    path = state_file()
    try:
        path.unlink()
    except OSError:
        pass
