"""TinyHand Linux application engine."""

from __future__ import annotations

import fcntl
import json
import math
import os
import selectors
import signal
import socket
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import cairo

from .draw import HandRenderer, draw_stroke, measure_text, panel, rounded_rect, text
from .input import HyprlandCursor, InputManager
from .paths import (
    control_socket,
    data_dir,
    lock_file,
    log_file,
    state_file,
)
from .prefs import (
    DEFAULT_PINNED,
    FADE_CHOICES,
    INTERACT_KEYS,
    SOUNDS,
    STYLE_BY_ID,
    RuntimeState,
    Settings,
    resolved_ink,
)
from .ui import UI
from .wayland import LayerCanvas


@dataclass
class Stroke:
    points: list[tuple[float, float, float]] = field(default_factory=list)
    color: tuple[float, float, float] = (1, 0.83, 0.18)
    born: float = field(default_factory=time.time)
    arrow: bool = False

    @property
    def done(self) -> bool:
        return len(self.points) > 1


@dataclass
class Ripple:
    x: float
    y: float
    born: float


AUTOSTART_PATH = Path.home() / ".config" / "autostart" / "tinyhand-linux.desktop"


class TinyHandApp:
    def __init__(self) -> None:
        self.prefs = Settings.load()
        self.renderer = HandRenderer()
        self.ui = UI(self.renderer, self.prefs, self.ui_action)
        self.ui.autostart = AUTOSTART_PATH.exists()
        self.ui.fields["hand_path"] = ""

        self.running = True
        self.active = True
        self.toolbar_visible = False
        self.sound_on = self.prefs.sound_hands
        self.cursor: tuple[float, float] = (0.0, 0.0)
        self.grip_x: float | None = None
        self.press = 0.0
        self.drawing = False
        self.strokes: list[Stroke] = []
        self.ripples: list[Ripple] = []
        self.modifiers: set[str] = set()
        self.interacting = False
        self.guard_until = 0.0
        self.click_through = False
        self.shift_taps: list[float] = []
        self.reveal_until = 0.0
        self.hint_until = 0.0
        self.notice = ""
        self.notice_until = 0.0
        self.settings_open = False
        self.welcome_open = not self.prefs.welcome_shown
        self.last_input_rects: tuple[tuple[int, int, int, int], ...] = ()
        self.lock_fd: int | None = None
        self.control_server: socket.socket | None = None
        self.canvas = LayerCanvas(on_resize=self.on_resize)
        self.selector = selectors.DefaultSelector()
        self.hypr = HyprlandCursor(self.set_cursor)
        self.input = InputManager(self.on_button, self.ui.scroll, self.on_key)

    # ------------------------------------------------------------------ setup

    def acquire_lock(self) -> bool:
        path = lock_file()
        path.parent.mkdir(parents=True, exist_ok=True)
        self.lock_fd = os.open(path, os.O_CREAT | os.O_RDWR, 0o600)
        try:
            fcntl.flock(self.lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            return True
        except BlockingIOError:
            os.close(self.lock_fd)
            self.lock_fd = None
            return False

    def setup_control(self) -> None:
        path = control_socket()
        path.parent.mkdir(parents=True, exist_ok=True)
        if path.exists():
            path.unlink()
        server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        server.bind(str(path))
        server.listen(8)
        self.control_server = server
        self.selector.register(server.fileno(), selectors.EVENT_READ, ("control", server))

    def run(self) -> int:
        if not self.acquire_lock():
            print("tinyhand: another instance is already running", file=sys.stderr)
            return 0
        self.canvas.connect()
        self.selector.register(self.canvas.fileno(), selectors.EVENT_READ, ("wayland", None))
        self.setup_control()
        self.input.register(self.selector)
        hypr_fd = self.hypr.fileno()
        if hypr_fd is not None:
            self.selector.register(hypr_fd, selectors.EVENT_READ, ("hypr", None))

        if self.active:
            self.activate(first=True)

        def stop(_signum, _frame) -> None:
            self.running = False

        signal.signal(signal.SIGINT, stop)
        signal.signal(signal.SIGTERM, stop)
        self.write_state()
        self.log("ready on Hyprland/Wayland")

        frame = 1 / 60
        try:
            while self.running:
                self.hypr.poll()
                self.tick()
                self.render()
                try:
                    events = self.selector.select(frame)
                except (OSError, ValueError):
                    break
                for key, _mask in events:
                    kind, obj = key.data
                    if kind == "wayland":
                        try:
                            self.canvas.display.read()  # type: ignore[union-attr]
                            self.canvas.display.dispatch()  # type: ignore[union-attr]
                        except Exception:
                            pass
                    elif kind == "control":
                        self.handle_control(obj)
                    elif kind == "hypr":
                        self.hypr.handle()
                    elif kind == "input":
                        self.input.handle(obj)
        except Exception:
            self.cleanup()
            raise
        self.cleanup()
        return 0

    def log(self, message: str) -> None:
        try:
            path = log_file()
            with path.open("a") as handle:
                handle.write(f"{time.strftime('%Y-%m-%d %H:%M:%S')} [{os.getpid()}] {message}\n")
        except OSError:
            pass
        print("[tinyhand]", message, flush=True)

    def cleanup(self) -> None:
        self.prefs.save()
        for dev in self.input.devices:
            try:
                self.selector.unregister(dev.fd)
            except Exception:
                pass
        self.input.close()
        self.hypr.close()
        if self.control_server:
            self.control_server.close()
        for path in (control_socket(), lock_file(), state_file()):
            try:
                path.unlink()
            except OSError:
                pass
        if self.lock_fd is not None:
            try:
                fcntl.flock(self.lock_fd, fcntl.LOCK_UN)
            finally:
                os.close(self.lock_fd)
            self.lock_fd = None
        self.canvas.close()

    # ------------------------------------------------------------------ state

    def set_cursor(self, x: float, y: float) -> None:
        if (x, y) == self.cursor:
            return
        self.cursor = (x, y)
        if self.drawing:
            self.add_stroke_point()
        self.canvas.dirty = True

    def on_resize(self, width: int, height: int) -> None:
        self.strokes.clear()
        self.grip_x = None
        self.log(f"surface configured: {width}x{height}")

    def activate(self, first: bool = False) -> None:
        self.active = True
        self.reveal_until = time.time() + 3
        if first and not self.prefs.first_run_hint_shown:
            self.prefs.first_run_hint_shown = True
            self.prefs.save()
            self.reveal_until = time.time() + 10
            self.hint_until = time.time() + 10
            self.ui.hint_parts = [
                ("Click", "to tap"),
                ("Drag", "to draw"),
                ("Hold ⌥", "to click for real"),
                ("Esc", "to exit"),
            ]
        self.write_state()

    def deactivate(self) -> None:
        self.active = False
        self.drawing = False
        self.strokes.clear()
        self.ripples.clear()
        self.ui.hint_parts = []
        self.ui.ink_open = False
        self.settings_open = False
        self.welcome_open = False
        self.ui.settings_open = False
        self.ui.welcome_open = False
        self.press = 0.0
        self.write_state()

    def toggle(self) -> None:
        if self.active:
            self.deactivate()
        else:
            self.activate()

    def write_state(self) -> None:
        RuntimeState.build(
            os.getpid(), self.active, self.prefs.pointer_style, self.sound_on, len(self.strokes)
        ).write()

    # ------------------------------------------------------------ input events

    def on_button(self, button: str, pressed: bool) -> None:
        x, y = self.cursor
        widget = self.ui.hit(x, y) if (self.ui.widgets and (self.settings_open or self.welcome_open or self.ui.toolbar_visible)) else None
        if widget and pressed:
            # UI clicks are captured by the input region; handle them once.
            self.ui_action(widget.id, widget.value)
            return
        if not self.active or not pressed and button != "left":
            return

        if button == "shift":
            return
        if button in ("right", "middle", "side", "extra"):
            if button == "right" and pressed:
                self.cycle_style()
            return
        if button != "left":
            return

        if self.click_through:
            return
        if pressed:
            if not self.drawing:
                self.drawing = True
                self.strokes.append(
                    Stroke(
                        color=resolved_ink(self.prefs.ink_choice, self.prefs.pointer_style),
                        arrow="ctrl" in self.modifiers,
                    )
                )
                self.add_stroke_point()
                self.ripples.append(Ripple(x, y, time.time()))
                self.press = 1.0
                self.play_tap(self.prefs.pointer_style)
                self.ui.hint_parts = []
        elif self.drawing:
            self.drawing = False
            self.press = 0.0

    def on_key(self, key: str, modifiers: set[str], value: int) -> None:
        self.modifiers = set(modifiers)

        if key in ("shift", "ctrl", "alt", "super"):
            self._interaction_modifiers()
            return
        if value != 1:
            return

        if self.ui.recording:
            self._record_shortcut(key, modifiers)
            return
        if self.ui.active_field and self.ui.active_field in self.ui.fields:
            self._type_into_field(self.ui.active_field, key, modifiers)
            return

        if key == "Escape":
            if self.ui.picker_open:
                self.ui.picker_open = False
            elif self.settings_open:
                self.ui_action("settings_close", None)
            elif self.welcome_open:
                self.welcome_open = False
            elif self.active:
                self.deactivate()
            return
        if key == "h" and modifiers == {"ctrl", "alt"}:
            self.toggle()
        elif key == "x" and modifiers == {"ctrl", "alt"}:
            self.clear()
        elif key == "e" and modifiers == {"ctrl", "alt"}:
            self.cycle_style()
        elif key == "m" and modifiers == {"ctrl", "alt"}:
            self.sound_on = not self.sound_on
            self.prefs.sound_hands = self.sound_on
            self.notify(f"sound: {'on' if self.sound_on else 'off'}")

    def _interaction_modifiers(self) -> None:
        key = self.prefs.interact_key
        held = (key == "option" and "alt" in self.modifiers) or (key == "control" and "ctrl" in self.modifiers) or (key == "command" and "super" in self.modifiers)
        if held == self.interacting:
            return
        self.interacting = held
        if held:
            self.guard_until = time.time() + 2
        if self.prefs.double_shift_click_through:
            now = time.time()
            if "shift" in self.modifiers:
                self.shift_taps.append(now)
                self.shift_taps = [t for t in self.shift_taps if now - t < 0.35]
                if len(self.shift_taps) >= 2 and self._clean_shift_taps():
                    self.click_through = not self.click_through
                    self.shift_taps.clear()
                    self.notify("clicks go through" if self.click_through else "taps restored")
        self.canvas.dirty = True

    def _clean_shift_taps(self) -> bool:
        taps = self.shift_taps[-2:]
        return len(taps) == 2 and 0 < taps[1] - taps[0] < 0.35

    def _record_shortcut(self, key: str, modifiers: set[str]) -> None:
        if key == "Escape":
            self.ui.recording = None
            return
        parts = []
        if "ctrl" in modifiers:
            parts.append("Ctrl")
        if "alt" in modifiers:
            parts.append("Alt")
        if "shift" in modifiers:
            parts.append("Shift")
        if "super" in modifiers:
            parts.append("Super")
        if key not in modifiers:
            parts.append(key)
        combo = "+".join(parts)
        which = self.ui.recording
        if which == "toggle_shortcut":
            self.prefs.toggle_shortcut = combo
        elif which == "clear_shortcut":
            self.prefs.clear_shortcut = combo
        self.ui.recording = None
        self.prefs.save()
        self.notify(f"shortcut: {combo}")

    def _type_into_field(self, field_id: str, key: str, modifiers: set[str]) -> None:
        current = self.ui.fields.get(field_id, "")
        if key == "BackSpace":
            self.ui.fields[field_id] = current[:-1]
        elif key == "Return":
            self.ui.active_field = None
        elif len(key) == 1:
            self.ui.fields[field_id] = current + key
        elif key == "Escape":
            self.ui.active_field = None

    def add_stroke_point(self) -> None:
        if not self.drawing or not self.strokes:
            return
        stroke = self.strokes[-1]
        x, y = self.cursor
        if not stroke.points or math.dist(stroke.points[-1][:2], (x, y)) > 1.5:
            stroke.points.append((x, y, time.time()))

    # ---------------------------------------------------------------- commands

    def command(self, text: str) -> dict[str, Any]:
        parts = text.replace("/", " ").split()
        cmd = parts[0].lower() if parts else "status"
        arg = parts[1].lower() if len(parts) > 1 else ""
        if cmd in ("on", "off", "toggle"):
            if cmd == "on":
                self.active = True
                self.activate()
            elif cmd == "off":
                self.deactivate()
            else:
                self.toggle()
        elif cmd == "clear":
            self.clear()
        elif cmd == "demo":
            self.demo()
        elif cmd == "quit":
            self.running = False
        elif cmd == "style":
            if not arg or arg == "next":
                arg = self.prefs.cycle_style()
            elif arg == "prev":
                arg = self.prefs.cycle_style(-1)
            if arg in STYLE_BY_ID:
                self.prefs.pointer_style = arg
            self.notify(f"style: {self.prefs.style.title}")
        elif cmd == "sound":
            if arg == "on":
                self.sound_on = True
            elif arg == "off":
                self.sound_on = False
            else:
                self.sound_on = not self.sound_on
            self.prefs.sound_hands = self.sound_on
            self.notify(f"sound: {'on' if self.sound_on else 'off'}")
        elif cmd == "settings":
            self.open_settings(arg if arg else None)
        elif cmd == "toolbar":
            self.reveal_until = time.time() + 10
        elif cmd == "section":
            self.open_settings(arg)
        elif cmd == "welcome":
            self.welcome_open = True
            self.ui.welcome_open = True
        elif cmd == "status":
            pass
        else:
            return {"error": f"unknown command: {cmd}"}
        self.prefs.save()
        self.write_state()
        self.canvas.dirty = True
        return self.status()

    def handle_control(self, server: socket.socket) -> None:
        try:
            conn, _ = server.accept()
            conn.settimeout(1.5)
            data = conn.recv(4096).decode(errors="replace").strip()
            result = self.command(data)
            conn.sendall((json.dumps(result) + "\n").encode())
            conn.close()
        except (OSError, ValueError):
            pass

    def status(self) -> dict[str, Any]:
        return {
            "active": self.active,
            "style": self.prefs.pointer_style,
            "sound": self.sound_on,
            "strokes": len(self.strokes),
            "cursor": [round(self.cursor[0]), round(self.cursor[1])],
            "settings": self.settings_open,
            "toolbar": self.toolbar_visible,
            "reveal": round(max(0.0, self.reveal_until - time.time()), 1),
            "show_toolbar": self.prefs.show_toolbar,
            "widgets": len(self.ui.widgets),
            "version": "1.1.0",
        }

    def notify(self, message: str) -> None:
        self.notice = message
        self.notice_until = time.time() + 1.6
        self.reveal_until = max(self.reveal_until, time.time() + 1.6)
        self.canvas.dirty = True

    def clear(self) -> None:
        self.strokes.clear()
        self.ripples.clear()
        self.notify("drawing cleared")

    def demo(self) -> None:
        self.active = True
        self.strokes.clear()
        self.ripples.clear()
        cx, cy = self.cursor
        stroke = Stroke(color=(0.15, 0.6, 1.0))
        for index in range(60):
            a = index / 59 * math.pi * 2
            stroke.points.append((cx + 150 * math.cos(a), cy + 95 * math.sin(a), time.time()))
        self.strokes.append(stroke)
        self.ripples.append(Ripple(cx, cy, time.time()))
        self.notify("demo stroke")

    def cycle_style(self, step: int = 1) -> None:
        style_id = self.prefs.cycle_style(step)
        self.prefs.save()
        self.notify(f"style: {STYLE_BY_ID[style_id].title}")

    def open_settings(self, section: str | None = None) -> None:
        self.settings_open = True
        self.welcome_open = False
        self.ui.settings_open = True
        self.ui.welcome_open = False
        if section:
            self.ui.settings_section = section
        self.ui.scroll_y = 0
        self.canvas.dirty = True

    def play_tap(self, style_id: str) -> None:
        if style_id == "laser" and not self.prefs.sound_laser:
            return
        if not self.sound_on:
            return
        sample = self.prefs.tap_sound if self.prefs.tap_sound in dict(SOUNDS) else "thock"
        path = Path(__file__).resolve().parent / "assets" / "sounds" / f"tap_{sample}.wav"
        if not path.exists():
            return
        try:
            subprocess.Popen(
                ["paplay", str(path)],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
        except OSError:
            pass

    # ------------------------------------------------------------ UI actions

    def ui_action(self, widget_id: str, value: Any) -> None:
        if widget_id == "redraw":
            self.canvas.dirty = True
            return
        if widget_id.startswith("style:"):
            self.prefs.pointer_style = widget_id.split(":", 1)[1]
            self.ui.ink_open = False
        elif widget_id == "ink":
            self.ui.ink_open = not self.ui.ink_open
        elif widget_id.startswith("ink_choice:"):
            self.prefs.ink_choice = widget_id.split(":", 1)[1]
            self.ui.ink_open = False
        elif widget_id == "sound":
            self.sound_on = not self.sound_on
            self.prefs.sound_hands = self.sound_on
            self.play_tap(self.prefs.pointer_style)
        elif widget_id == "exit":
            self.deactivate()
        elif widget_id.startswith("section:"):
            self.ui.settings_section = widget_id.split(":", 1)[1]
            self.ui.scroll_y = 0
            self.ui.picker_open = False
        elif widget_id == "settings_close":
            self.settings_open = False
            self.ui.settings_open = False
            self.ui.picker_open = False
        elif widget_id.startswith("field:"):
            field_id = widget_id.split(":", 1)[1]
            if field_id in ("toggle_shortcut", "clear_shortcut"):
                self.ui.active_field = None
                self.ui.recording = field_id
            else:
                self.ui.active_field = field_id
        elif widget_id in ("double_shift", "show_shadow", "show_toolbar", "show_ink_button", "sound_laser", "autostart", "welcome_shown"):
            attr = widget_id
            current = self.ui.autostart if attr == "autostart" else getattr(self.prefs, attr)
            setattr(self.prefs if attr != "autostart" else self.ui, attr, not current)
            if attr == "autostart":
                self._set_autostart(self.ui.autostart)
        elif widget_id == "interact_key":
            index = INTERACT_KEYS.index(self.prefs.interact_key) if self.prefs.interact_key in INTERACT_KEYS else 0
            self.prefs.interact_key = INTERACT_KEYS[(index + 1) % len(INTERACT_KEYS)]
        elif widget_id == "interact_keeps_hand":
            self.prefs.interact_keeps_hand = not self.prefs.interact_keeps_hand
        elif widget_id == "fade":
            index = FADE_CHOICES.index(self.prefs.fade_seconds) if self.prefs.fade_seconds in FADE_CHOICES else 0
            self.prefs.fade_seconds = FADE_CHOICES[(index + 1) % len(FADE_CHOICES)]
        elif widget_id == "reset_toolbar":
            self.prefs.pinned_styles = list(DEFAULT_PINNED)
        elif widget_id == "add_pointer":
            self.ui.picker_open = not self.ui.picker_open
            self.ui.picker_scroll = 0
        elif widget_id.startswith("picker_style:"):
            style_id = widget_id.split(":", 1)[1]
            self.prefs.set_style(style_id)
            self.prefs.pin(style_id)
            self.ui.picker_open = False
        elif widget_id.startswith("pinned:"):
            style_id = widget_id.split(":", 1)[1]
            self.prefs.pointer_style = style_id
        elif widget_id == "make_hand":
            self.ui.settings_section = "make_hand"
            self.ui.scroll_y = 0
        elif widget_id == "back_pointers":
            self.ui.settings_section = "pointers"
            self.ui.scroll_y = 0
        elif widget_id == "copy_prompt":
            self._copy_prompt()
        elif widget_id == "add_hand":
            self.notify("custom-hand import is available in the docs")
        elif widget_id.startswith("settings_ink:"):
            self.prefs.ink_choice = widget_id.split(":", 1)[1]
        elif widget_id.startswith("sound:"):
            sound_id = widget_id.split(":", 1)[1]
            if sound_id == "off":
                self.sound_on = False
                self.prefs.sound_hands = False
            else:
                self.sound_on = True
                self.prefs.sound_hands = True
                self.prefs.tap_sound = sound_id
                self.play_tap(sound_id)
        elif widget_id.startswith("link:"):
            index = int(widget_id.split(":", 1)[1])
            urls = [
                "https://tinyhand.app",
                "https://github.com/nilbuild/tinyhand",
                "https://x.com/nilbuild",
                "https://github.com/nilbuild/tinyhand/blob/main/LICENSE",
            ]
            if 0 <= index < len(urls):
                try:
                    subprocess.Popen(["xdg-open", urls[index]], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                except OSError:
                    pass
        elif widget_id == "reset_all":
            self.prefs.reset()
            self.sound_on = self.prefs.sound_hands
            self.notify("settings reset")
        elif widget_id == "welcome_button":
            if not self.prefs.welcome_shown:
                self.prefs.welcome_shown = True
                self.prefs.save()
                self.open_settings("general")
            else:
                self.welcome_open = False
                self.ui.welcome_open = False
                self.activate()
        elif widget_id == "welcome_close":
            self.prefs.welcome_shown = True
            self.welcome_open = False
            self.ui.welcome_open = False

        self.prefs.save()
        self.write_state()
        self.canvas.dirty = True

    def _set_autostart(self, enabled: bool) -> None:
        try:
            if enabled:
                import shutil

                launcher = data_dir() / "venv" / "bin" / "tinyhand"
                executable = str(launcher if launcher.exists() else (shutil.which("tinyhand") or "tinyhand"))
                AUTOSTART_PATH.parent.mkdir(parents=True, exist_ok=True)
                AUTOSTART_PATH.write_text(
                    "[Desktop Entry]\nType=Application\nName=TinyHand Linux\n"
                    f"Exec={executable} --hidden\nNoDisplay=true\nX-GNOME-Autostart-enabled=true\n"
                )
            else:
                AUTOSTART_PATH.unlink(missing_ok=True)
        except OSError:
            pass

    def _copy_prompt(self) -> None:
        idea = self.ui.fields.get("idea", "") or "a soft vinyl toy in a bright colour"
        prompt = (
            "Make a pointer image for TinyHand. Draw one cartoon right hand in this style: "
            + idea
            + ". We see the back of the hand; index finger points straight up at the far left; "
            "transparent background; upright; portrait; no shadow, text or stick."
        )
        try:
            subprocess.run(["xclip", "-selection", "clipboard"], input=prompt, text=True, timeout=1)
        except (OSError, subprocess.TimeoutExpired):
            pass
        self.notify("prompt copied")

    # ------------------------------------------------------------------- tick

    def tick(self) -> None:
        now = time.time()
        self.press *= 0.82 if not self.drawing else 0.94
        if self.grip_x is None:
            self.grip_x = min(max(self.cursor[0] + 0.30 * self.canvas.width, 0.15 * self.canvas.width), 1.05 * self.canvas.width)
        else:
            target = min(max(self.cursor[0] + 0.30 * self.canvas.width, 0.15 * self.canvas.width), 1.05 * self.canvas.width)
            self.grip_x += (target - self.grip_x) * 0.08

        near_toolbar = self.cursor[1] > self.canvas.height - 170
        toolbar = self.active and self.prefs.show_toolbar and (near_toolbar or now < self.reveal_until or self.ui.ink_open or bool(self.ui.hint_parts))
        self.toolbar_visible = toolbar
        self.ui.toolbar_visible = toolbar
        if self.ui.hint_parts and now > self.hint_until:
            self.ui.hint_parts = []
        if now > self.notice_until:
            self.notice = ""
        self.ui.settings_open = self.settings_open
        self.ui.welcome_open = self.welcome_open
        self.ui.ink_open = self.ui.ink_open and toolbar
        self.ui.layout(self, self.canvas.width, self.canvas.height)

        rects = tuple(self.ui.input_rects) if (self.settings_open or self.welcome_open or toolbar) else ()
        if rects != self.last_input_rects:
            self.last_input_rects = rects
            self.canvas.set_interactive(bool(rects), list(rects))

    # ------------------------------------------------------------------ paint

    def render(self) -> None:
        animating = bool(self.ripples) or self.drawing or self.prefs.pointer_style == "laser" or self.press > 0.01
        if not (self.canvas.dirty or animating):
            return
        self.canvas.draw(self.draw_scene)

    def draw_scene(self, ctx: cairo.Context, width: int, height: int) -> None:
        ctx.set_operator(cairo.OPERATOR_CLEAR)
        ctx.paint()
        ctx.set_operator(cairo.OPERATOR_OVER)
        if self.active:
            self._draw_ink(ctx)
        self.ui.paint(ctx, self, width, height)
        if self.notice and not self.settings_open and not self.welcome_open:
            text(ctx, width / 2, 54, self.notice, 14, (1, 1, 1, 0.95), True, "center")

    def _draw_ink(self, ctx: cairo.Context) -> None:
        now = time.time()
        line_width = max(3, self.canvas.height * 0.0055)
        remaining = []
        for stroke in self.strokes:
            alpha = 1.0
            fade = max(0.5, self.prefs.fade_seconds)
            if self.prefs.fade_seconds > 0 and stroke.points:
                age = now - stroke.points[-1][2]
                delay = max(0.5, self.prefs.fade_seconds)
                if age > delay:
                    alpha = max(0, 1 - (age - delay) / 0.6)
            if alpha <= 0:
                continue
            remaining.append(stroke)
            draw_stroke(ctx, stroke.points, stroke.color, line_width, alpha, stroke.arrow)
        self.strokes = remaining

        self.ripples = [r for r in self.ripples if now - r.born < 0.45]
        for ripple in self.ripples:
            t = (now - ripple.born) / 0.45
            ctx.set_source_rgba(1, 0.85, 0.2, 0.7 * (1 - t))
            ctx.set_line_width(3 * (1 - t) + 1)
            ctx.arc(ripple.x, ripple.y, 8 + 34 * t, 0, 2 * math.pi)
            ctx.stroke()

        hand_len = self.prefs.pointer_size * self.canvas.height * (1 - 0.06 * self.press)
        grip = (self.grip_x or self.cursor[0], -0.12 * self.canvas.height)
        hidden = (self.interacting and not self.prefs.interact_keeps_hand) or self.click_through or now < self.guard_until and self.interacting
        if not hidden:
            self.renderer.draw(
                ctx,
                self.prefs.pointer_style,
                self.cursor,
                grip,
                hand_len,
                self.press,
                self.prefs.show_shadow,
            )
