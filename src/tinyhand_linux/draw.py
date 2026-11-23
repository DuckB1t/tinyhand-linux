"""Cairo drawing primitives and the pixel-accurate pointer renderer."""

from __future__ import annotations

import json
import math
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

import cairo
import numpy as np
from PIL import Image

from .paths import asset_dir
from .prefs import hex_rgb

MANIFEST: dict[str, dict] = json.loads((asset_dir() / "manifest.json").read_text())


def _cairo_bytes(image: Image.Image) -> bytearray:
    """Convert PIL RGBA pixels to cairo ARGB32 (native little-endian BGRA).

    Cairo expects premultiplied alpha; straight RGBA bytes render with swapped
    red/blue channels and dark fringes on translucent edges.
    """
    rgba = np.asarray(image.convert("RGBA"), dtype=np.uint8)
    alpha = rgba[:, :, 3:4].astype(np.uint16)
    rgb = rgba[:, :, :3].astype(np.uint16)
    premultiplied = ((rgb * alpha + 127) // 255).astype(np.uint8)
    bgra = np.concatenate(
        (
            premultiplied[:, :, 2:3],
            premultiplied[:, :, 1:2],
            premultiplied[:, :, 0:1],
            rgba[:, :, 3:4],
        ),
        axis=2,
    )
    return bytearray(bgra.tobytes())


# ---------------------------------------------------------------------- paths


def ellipse(
    ctx: cairo.Context, cx: float, cy: float, rx: float, ry: float
) -> None:
    ctx.save()
    ctx.translate(cx, cy)
    ctx.scale(rx, ry)
    ctx.arc(0, 0, 1, 0, 2 * math.pi)
    ctx.restore()


def rounded_rect(
    ctx: cairo.Context, x: float, y: float, w: float, h: float, r: float
) -> None:
    r = max(0.0, min(r, w / 2, h / 2))
    ctx.new_path()
    ctx.arc(x + r, y + r, r, math.pi, 1.5 * math.pi)
    ctx.arc(x + w - r, y + r, r, 1.5 * math.pi, 2 * math.pi)
    ctx.arc(x + w - r, y + h - r, r, 0, 0.5 * math.pi)
    ctx.arc(x + r, y + h - r, r, 0.5 * math.pi, math.pi)
    ctx.close_path()


def panel(
    ctx: cairo.Context,
    x: float,
    y: float,
    w: float,
    h: float,
    radius: float = 16,
    alpha: float = 0.72,
) -> None:
    rounded_rect(ctx, x, y, w, h, radius)
    ctx.set_source_rgba(0.055, 0.06, 0.075, alpha)
    ctx.fill_preserve()
    ctx.set_source_rgba(1, 1, 1, 0.12)
    ctx.set_line_width(1)
    ctx.stroke()


def measure_text(ctx: cairo.Context, text: str, size: float, bold: bool = False) -> tuple[float, float]:
    ctx.select_font_face("Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_BOLD if bold else cairo.FONT_WEIGHT_NORMAL)
    ctx.set_font_size(size)
    ext = ctx.text_extents(text)
    return ext.width, ext.height


def text(
    ctx: cairo.Context,
    x: float,
    y: float,
    value: str,
    size: float = 12,
    color: tuple[float, float, float, float] = (1, 1, 1, 1),
    bold: bool = False,
    align: str = "left",
) -> float:
    ctx.select_font_face("Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_BOLD if bold else cairo.FONT_WEIGHT_NORMAL)
    ctx.set_font_size(size)
    ext = ctx.text_extents(value)
    if align == "center":
        x -= ext.width / 2
    elif align == "right":
        x -= ext.width
    ctx.set_source_rgba(*color)
    ctx.move_to(x, y)
    ctx.show_text(value)
    return ext.width


# ---------------------------------------------------------------------- icons


def icon(ctx: cairo.Context, name: str, cx: float, cy: float, size: float, color=(1, 1, 1, 0.85)) -> None:
    ctx.save()
    ctx.translate(cx, cy)
    ctx.set_source_rgba(*color)
    ctx.set_line_width(max(1.2, size * 0.09))
    ctx.set_line_cap(cairo.LINE_CAP_ROUND)
    ctx.set_line_join(cairo.LINE_JOIN_ROUND)
    s = size / 2

    if name == "speaker":
        ctx.move_to(-s * 0.7, -s * 0.3)
        ctx.line_to(-s * 0.25, -s * 0.3)
        ctx.line_to(s * 0.1, -s * 0.65)
        ctx.line_to(s * 0.1, s * 0.65)
        ctx.line_to(-s * 0.25, s * 0.3)
        ctx.line_to(-s * 0.7, s * 0.3)
        ctx.close_path()
        ctx.fill()
        for r in (0.45, 0.72):
            ctx.new_path()
            ctx.arc(s * 0.1, 0, s * r, -0.85, 0.85)
            ctx.stroke()
    elif name == "speaker_slash":
        ctx.move_to(-s * 0.7, -s * 0.3)
        ctx.line_to(-s * 0.25, -s * 0.3)
        ctx.line_to(s * 0.1, -s * 0.65)
        ctx.line_to(s * 0.1, s * 0.65)
        ctx.line_to(-s * 0.25, s * 0.3)
        ctx.line_to(-s * 0.7, s * 0.3)
        ctx.close_path()
        ctx.fill()
        ctx.move_to(-s * 0.8, s * 0.8)
        ctx.line_to(s * 0.85, -s * 0.85)
        ctx.stroke()
    elif name == "x":
        ctx.move_to(-s * 0.5, -s * 0.5)
        ctx.line_to(s * 0.5, s * 0.5)
        ctx.move_to(s * 0.5, -s * 0.5)
        ctx.line_to(-s * 0.5, s * 0.5)
        ctx.stroke()
    elif name == "plus":
        ctx.move_to(0, -s * 0.6)
        ctx.line_to(0, s * 0.6)
        ctx.move_to(-s * 0.6, 0)
        ctx.line_to(s * 0.6, 0)
        ctx.stroke()
    elif name == "gear":
        ctx.arc(0, 0, s * 0.3, 0, 2 * math.pi)
        ctx.stroke()
        for i in range(8):
            a = i * math.pi / 4
            ctx.move_to(math.cos(a) * s * 0.48, math.sin(a) * s * 0.48)
            ctx.line_to(math.cos(a) * s * 0.78, math.sin(a) * s * 0.78)
            ctx.stroke()
    elif name == "chevron_left":
        ctx.move_to(s * 0.25, -s * 0.5)
        ctx.line_to(-s * 0.35, 0)
        ctx.line_to(s * 0.25, s * 0.5)
        ctx.stroke()
    elif name == "check":
        ctx.move_to(-s * 0.55, 0)
        ctx.line_to(-s * 0.1, s * 0.45)
        ctx.line_to(s * 0.6, -s * 0.5)
        ctx.stroke()
    elif name == "arrow":
        ctx.move_to(-s * 0.5, s * 0.5)
        ctx.line_to(s * 0.55, -s * 0.55)
        ctx.move_to(s * 0.05, -s * 0.55)
        ctx.line_to(s * 0.55, -s * 0.55)
        ctx.line_to(s * 0.55, -s * 0.05)
        ctx.stroke()
    elif name == "waveform":
        for index, height in enumerate((0.25, 0.55, 0.9, 0.5, 0.75, 0.35)):
            x = -s * 0.7 + index * s * 0.28
            ctx.move_to(x, -s * height)
            ctx.line_to(x, s * height)
            ctx.stroke()
    elif name == "sparkles":
        for ox, oy, r in ((-0.25, -0.15, 0.35), (0.35, 0.2, 0.25)):
            ctx.move_to(ox * s, (oy - r) * s)
            ctx.line_to((ox + r * 0.3) * s, (oy - r * 0.3) * s)
            ctx.line_to((ox + r) * s, oy * s)
            ctx.line_to((ox + r * 0.3) * s, (oy + r * 0.3) * s)
            ctx.line_to(ox * s, (oy + r) * s)
            ctx.line_to((ox - r * 0.3) * s, (oy + r * 0.3) * s)
            ctx.line_to((ox - r) * s, oy * s)
            ctx.line_to((ox - r * 0.3) * s, (oy - r * 0.3) * s)
            ctx.close_path()
            ctx.stroke()
    elif name == "pencil":
        ctx.move_to(-s * 0.55, s * 0.35)
        ctx.line_to(s * 0.15, -s * 0.55)
        ctx.line_to(s * 0.65, -s * 0.05)
        ctx.line_to(-s * 0.05, s * 0.65)
        ctx.close_path()
        ctx.stroke()
    elif name == "hand":
        ctx.move_to(-s * 0.2, s * 0.65)
        ctx.line_to(-s * 0.2, -s * 0.4)
        ctx.move_to(0, s * 0.65)
        ctx.line_to(0, -s * 0.7)
        ctx.move_to(s * 0.25, s * 0.65)
        ctx.line_to(s * 0.25, -s * 0.45)
        ctx.move_to(s * 0.5, s * 0.6)
        ctx.line_to(s * 0.5, -s * 0.15)
        ctx.move_to(-s * 0.55, s * 0.2)
        ctx.line_to(-s * 0.35, s * 0.65)
        ctx.line_to(s * 0.55, s * 0.65)
        ctx.close_path()
        ctx.stroke()
    elif name == "info":
        ctx.arc(0, 0, s * 0.75, 0, 2 * math.pi)
        ctx.stroke()
        ctx.move_to(0, -s * 0.35)
        ctx.line_to(0, s * 0.1)
        ctx.stroke()
        ctx.arc(0, s * 0.45, 0.8, 0, 2 * math.pi)
        ctx.fill()
    elif name == "warning":
        ctx.move_to(0, -s * 0.7)
        ctx.line_to(s * 0.8, s * 0.65)
        ctx.line_to(-s * 0.8, s * 0.65)
        ctx.close_path()
        ctx.stroke()
        ctx.move_to(0, -s * 0.15)
        ctx.line_to(0, s * 0.25)
        ctx.stroke()
        ctx.arc(0, s * 0.48, 0.9, 0, 2 * math.pi)
        ctx.fill()
    else:
        ctx.arc(0, 0, s * 0.6, 0, 2 * math.pi)
        ctx.stroke()
    ctx.restore()


def ink_dot(
    ctx: cairo.Context, cx: float, cy: float, colour: str, radius: float = 13
) -> None:
    ctx.save()
    ctx.set_source_rgba(0, 0, 0, 0.35)
    ellipse(ctx, cx, cy + 1.5, radius + 1.5, radius + 1.5)
    ctx.fill()
    r, g, b = hex_rgb(colour)
    ctx.set_source_rgb(r, g, b)
    ctx.arc(cx, cy, radius, 0, 2 * math.pi)
    ctx.fill()
    ctx.set_source_rgba(1, 1, 1, 0.35)
    ellipse(ctx, cx - radius * 0.2, cy - radius * 0.4, radius * 0.35, radius * 0.25)
    ctx.fill()
    ctx.restore()


def rainbow_dot(ctx: cairo.Context, cx: float, cy: float, radius: float = 13) -> None:
    ctx.save()
    ctx.set_source_rgba(0, 0, 0, 0.35)
    ctx.arc(cx, cy + 1.5, radius + 1.5, 0, 2 * math.pi)
    ctx.fill()
    colours = ("#FF3B30", "#FF9500", "#FFCC00", "#34C759", "#0A84FF", "#AF52DE")
    for index, colour in enumerate(colours):
        a0 = math.pi / 2 + index * math.pi / 3
        a1 = a0 + math.pi / 3
        ctx.set_source_rgb(*hex_rgb(colour))
        ctx.move_to(cx, cy)
        ctx.arc(cx, cy, radius, a0, a1)
        ctx.close_path()
        ctx.fill()
    ctx.restore()


@dataclass
class Geometry:
    size: tuple[int, int]
    hand_top: int
    tip_x: float
    wrist: int
    rod_end: int
    rod_x: float
    aspect: float


class HandRenderer:
    """Loads and draws the original 1024×1536 hand PNGs."""

    def __init__(self, assets: Path | None = None) -> None:
        self.assets = assets or asset_dir()
        self.geometry: dict[str, Geometry] = {
            key: Geometry(
                tuple(value["size"]),  # type: ignore[arg-type]
                value["hand_top"],
                value["tip_x"],
                value["wrist"],
                value["rod_end"],
                value["rod_x"],
                value["aspect"],
            )
            for key, value in MANIFEST.items()
        }
        self._cache: dict[str, cairo.ImageSurface] = {}
        self._buffers: dict[str, bytearray] = {}
        self._order: list[str] = []

    def thumbnail_for(self, style_id: str, size: int = 40) -> cairo.ImageSurface | None:
        """Crop the hand region of an asset into a square toolbar thumbnail."""
        key = f"thumb:{style_id}:{size}"
        if key in self._cache:
            return self._cache[key]
        if style_id == "laser":
            return None
        geom = self.geometry.get(style_id)
        path = self.assets / "hands" / f"hand_{style_id}.png"
        if geom is None or not path.exists():
            return None
        image = Image.open(path).convert("RGBA")
        # Recompute the hand crop from alpha (the manifest stores the pose metrics).
        alpha = np.array(image.getchannel("A"))
        hand = alpha[geom.hand_top : geom.wrist + 1]
        hx = np.where((hand > 128).any(axis=0))[0]
        min_x, max_x = int(hx.min()), int(hx.max())
        side = max(max_x - min_x + 1, geom.wrist - geom.hand_top + 1) * 1.06
        cx = (min_x + max_x) / 2
        cy = (geom.hand_top + geom.wrist) / 2
        left = max(0, int(cx - side / 2))
        top = max(0, int(cy - side / 2))
        box = (left, top, min(image.width, int(left + side)), min(image.height, int(top + side)))
        crop = image.crop(box).resize((size, size), Image.Resampling.LANCZOS)
        data = _cairo_bytes(crop)
        surface = cairo.ImageSurface.create_for_data(
            data, cairo.FORMAT_ARGB32, size, size, size * 4
        )
        self._buffers[key] = data
        self._cache[key] = surface
        return surface

    def surface_for(self, style_id: str) -> cairo.ImageSurface | None:
        if style_id in self._cache:
            return self._cache[style_id]
        geom = self.geometry.get(style_id)
        if geom is None:
            return None
        path = self.assets / "hands" / f"hand_{style_id}.png"
        if not path.exists():
            return None
        image = Image.open(path).convert("RGBA")
        data = _cairo_bytes(image)
        surface = cairo.ImageSurface.create_for_data(
            data, cairo.FORMAT_ARGB32, image.width, image.height, image.width * 4
        )
        self._buffers[style_id] = data
        self._cache[style_id] = surface
        self._order.append(style_id)
        while len(self._order) > 3:
            old = self._order.pop(0)
            if old != style_id:
                self._cache.pop(old, None)
                self._buffers.pop(old, None)
        return surface

    @staticmethod
    def _paint_crop(
        ctx: cairo.Context,
        surface: cairo.ImageSurface,
        geom: Geometry,
        cursor: tuple[float, float],
        angle: float,
        scale: float,
        alpha: float,
    ) -> None:
        ctx.save()
        ctx.translate(*cursor)
        ctx.rotate(angle)
        ctx.scale(scale, scale)
        ctx.translate(-geom.tip_x, -geom.hand_top)
        ctx.rectangle(0, geom.hand_top, geom.size[0], geom.wrist - geom.hand_top)
        ctx.clip()
        if alpha >= 0.999:
            ctx.set_source_surface(surface, 0, 0)
            ctx.paint()
        else:
            ctx.push_group()
            ctx.set_source_surface(surface, 0, 0)
            ctx.paint()
            group = ctx.pop_group()
            ctx.set_source(group)
            ctx.paint_with_alpha(alpha)
        ctx.restore()

    def draw(
        self,
        ctx: cairo.Context,
        style_id: str,
        cursor: tuple[float, float],
        grip: tuple[float, float],
        hand_len: float,
        press: float,
        show_shadow: bool = True,
    ) -> None:
        if style_id == "laser":
            self.draw_laser(ctx, cursor, hand_len, press)
            return
        surface = self.surface_for(style_id)
        geom = self.geometry.get(style_id)
        if surface is None or geom is None:
            self.draw_laser(ctx, cursor, hand_len, press)
            return

        dx, dy = cursor[0] - grip[0], cursor[1] - grip[1]
        length = max(1.0, math.hypot(dx, dy))
        ux, uy = dx / length, dy / length
        base = (cursor[0] - ux * hand_len, cursor[1] - uy * hand_len)

        scale = hand_len / max(1.0, geom.wrist - geom.hand_top)
        src_vx, src_vy = geom.rod_x - geom.tip_x, geom.wrist - geom.hand_top
        target_vx, target_vy = base[0] - cursor[0], base[1] - cursor[1]
        angle = math.atan2(target_vy, target_vx) - math.atan2(src_vy, src_vx)

        lift = 0.06 * max(1.0, hand_len / 0.13) * (1 - 0.8 * press)
        shadow = (0.6 * lift * 0.25, -lift * 0.25) if show_shadow else (0, 0)

        # Stick: dark outline, graphite body, soft highlight.
        width = max(8.0, 54 * scale)
        if show_shadow:
            ctx.set_source_rgba(0, 0, 0, 0.22)
            ctx.set_line_width(width + 5)
            ctx.move_to(grip[0] + shadow[0], grip[1] + shadow[1])
            ctx.line_to(base[0] + shadow[0], base[1] + shadow[1])
            ctx.stroke()
        ctx.set_source_rgba(0.04, 0.04, 0.05, 1)
        ctx.set_line_width(width + 3)
        ctx.move_to(*grip)
        ctx.line_to(*base)
        ctx.stroke()
        ctx.set_source_rgba(0.25, 0.25, 0.28, 1)
        ctx.set_line_width(width)
        ctx.move_to(*grip)
        ctx.line_to(*base)
        ctx.stroke()
        ctx.set_source_rgba(0.55, 0.55, 0.58, 0.75)
        ctx.set_line_width(max(1.0, width * 0.16))
        ctx.move_to(grip[0] - width * 0.22, grip[1])
        ctx.line_to(base[0] - width * 0.22, base[1])
        ctx.stroke()

        if show_shadow:
            self._paint_crop(
                ctx, surface, geom, (cursor[0] + shadow[0], cursor[1] + shadow[1]), angle, scale, 0.26
            )
        self._paint_crop(ctx, surface, geom, cursor, angle, scale, 1.0)

    @staticmethod
    def draw_laser(
        ctx: cairo.Context,
        cursor: tuple[float, float],
        hand_len: float,
        press: float,
    ) -> None:
        radius = max(4.0, hand_len * 0.027) * (1 + 0.2 * press)
        r, g, b = hex_rgb("#FF2920")
        ctx.set_source_rgba(r, g, b, 0.22)
        ctx.arc(cursor[0], cursor[1], radius * 2.8, 0, 2 * math.pi)
        ctx.fill()
        ctx.set_source_rgba(r, g, b, 0.55)
        ctx.arc(cursor[0], cursor[1], radius * 1.7, 0, 2 * math.pi)
        ctx.fill()
        ctx.set_source_rgba(r, g, b, 1)
        ctx.arc(cursor[0], cursor[1], radius, 0, 2 * math.pi)
        ctx.fill()
        ctx.set_source_rgba(1, 0.86, 0.82, 0.95)
        ctx.arc(cursor[0], cursor[1], radius * 0.42, 0, 2 * math.pi)
        ctx.fill()


def draw_stroke(
    ctx: cairo.Context,
    points: Iterable[tuple[float, float, float]],
    colour: tuple[float, float, float],
    width: float,
    alpha: float = 1.0,
    arrow: bool = False,
) -> None:
    pts = list(points)
    if len(pts) < 2:
        return
    ctx.save()
    ctx.set_line_cap(cairo.LINE_CAP_ROUND)
    ctx.set_line_join(cairo.LINE_JOIN_ROUND)
    coords = [(p[0], p[1]) for p in pts]

    def path() -> None:
        ctx.move_to(*coords[0])
        if len(coords) == 2:
            ctx.line_to(*coords[1])
            return
        previous = coords[0]
        for i in range(1, len(coords) - 1):
            control = coords[i]
            mx = (coords[i][0] + coords[i + 1][0]) / 2
            my = (coords[i][1] + coords[i + 1][1]) / 2
            # Cairo has no quadratic primitive; convert Q(previous, control, mid)
            # to its exact cubic equivalent.
            cp1 = (
                previous[0] + (control[0] - previous[0]) * 2 / 3,
                previous[1] + (control[1] - previous[1]) * 2 / 3,
            )
            cp2 = (
                mx + (control[0] - mx) * 2 / 3,
                my + (control[1] - my) * 2 / 3,
            )
            ctx.curve_to(cp1[0], cp1[1], cp2[0], cp2[1], mx, my)
            previous = (mx, my)
        ctx.line_to(*coords[-1])

    ctx.set_source_rgba(0, 0, 0, 0.18 * alpha)
    ctx.set_line_width(width + 3)
    path()
    ctx.stroke()
    ctx.set_source_rgba(*colour, 0.98 * alpha)
    ctx.set_line_width(width)
    path()
    ctx.stroke()

    if arrow:
        end = coords[-1]
        back = coords[-2]
        for candidate in reversed(coords[:-1]):
            if math.dist(candidate, end) >= max(14, width * 3.2) * 1.2:
                back = candidate
                break
        angle = math.atan2(end[1] - back[1], end[0] - back[0])
        size = max(14, width * 3.2)
        length = min(size, math.dist(back, end) * 0.8)
        for side in (-1, 1):
            a = angle + math.pi + side * math.pi / 7
            ctx.move_to(end[0], end[1])
            ctx.line_to(end[0] + math.cos(a) * length, end[1] + math.sin(a) * length)
            ctx.stroke()
    ctx.restore()
