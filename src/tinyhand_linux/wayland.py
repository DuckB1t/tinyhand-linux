"""Minimal wlroots layer-shell surface used by TinyHand Linux."""

from __future__ import annotations

import mmap
import os
import time
from dataclasses import dataclass
from typing import Callable

import cairo
from pywayland.client import Display
from pywayland.protocol.wayland import WlCompositor, WlRegistry, WlShm, WlSurface

from .protocol.wlr_layer_shell_unstable_v1 import ZwlrLayerShellV1, ZwlrLayerSurfaceV1


@dataclass
class Buffer:
    buffer: object
    surface: cairo.ImageSurface
    mmap: mmap.mmap
    busy: bool = False


class LayerCanvas:
    """A fullscreen, input-transparent wlr-layer-shell surface."""

    def __init__(self, namespace: str = "tinyhand-linux", on_resize=None) -> None:
        self.namespace = namespace
        self.on_resize = on_resize
        self.width = 0
        self.height = 0
        self.scale = 1
        self.dirty = True
        self.buffers: list[Buffer] = []
        self.display: Display | None = None
        self.compositor = None
        self.shm = None
        self.layer_shell = None
        self.surface: WlSurface | None = None
        self.layer_surface = None
        self._input_rects: list[tuple[int, int, int, int]] = []
        self._interactive = False

    def connect(self) -> None:
        self.display = Display()
        self.display.connect()
        registry = self.display.get_registry()

        def on_global(_proxy, name: int, interface: str, version: int) -> None:
            if interface == WlCompositor.name:
                self.compositor = registry.bind(name, WlCompositor, min(version, 6))
            elif interface == WlShm.name:
                self.shm = registry.bind(name, WlShm, min(version, 3))
            elif interface == ZwlrLayerShellV1.name:
                self.layer_shell = registry.bind(name, ZwlrLayerShellV1, min(version, 4))

        registry.dispatcher["global"] = on_global
        self.display.roundtrip()
        if not (self.compositor and self.shm and self.layer_shell):
            raise RuntimeError(
                "Missing wl_compositor, wl_shm or zwlr_layer_shell_v1. "
                "TinyHand needs a wlroots compositor such as Hyprland or sway."
            )

        self.surface = self.compositor.create_surface()
        self.set_interactive(False)
        self.layer_surface = self.layer_shell.get_layer_surface(
            self.surface,
            None,
            ZwlrLayerShellV1.layer.overlay,
            self.namespace,
        )
        self.layer_surface.set_anchor(
            ZwlrLayerSurfaceV1.anchor.top
            | ZwlrLayerSurfaceV1.anchor.bottom
            | ZwlrLayerSurfaceV1.anchor.left
            | ZwlrLayerSurfaceV1.anchor.right
        )
        self.layer_surface.set_exclusive_zone(-1)
        self.layer_surface.set_keyboard_interactivity(
            ZwlrLayerSurfaceV1.keyboard_interactivity.none
        )
        self.layer_surface.set_size(0, 0)
        self.layer_surface.dispatcher["configure"] = self._configure
        self.surface.commit()
        self.display.flush()

        deadline = time.time() + 5
        while not self.width and time.time() < deadline:
            self.dispatch(0.2)
        if not self.width:
            raise RuntimeError("Layer surface was never configured by the compositor")
        # The configure callback normally creates the buffers. Only do it here
        # when a compositor configures with zero size (or sends no configure).
        if not self.buffers:
            self._create_buffers()

    def _configure(self, _proxy, serial: int, width: int, height: int) -> None:
        new_w, new_h = int(width), int(height)
        if new_w and new_h and (new_w, new_h) != (self.width, self.height):
            self.width, self.height = new_w, new_h
            self._destroy_buffers()
            self._create_buffers()
            if self.on_resize:
                self.on_resize(new_w, new_h)
        self.layer_surface.ack_configure(serial)
        self.dirty = True

    def _destroy_buffers(self) -> None:
        for item in self.buffers:
            try:
                item.buffer.destroy()
            except Exception:
                pass
        self.buffers.clear()

    def _create_buffers(self) -> None:
        if not self.width or not self.height or not self.shm:
            return
        stride = self.width * 4
        size = stride * self.height
        memfd = os.memfd_create("tinyhand", 0)
        os.ftruncate(memfd, size * 2)
        pool = self.shm.create_pool(memfd, size * 2)
        for index in range(2):
            offset = index * size
            memory = mmap.mmap(memfd, size, offset=offset)
            surface = cairo.ImageSurface.create_for_data(
                memory, cairo.FORMAT_ARGB32, self.width, self.height, stride
            )
            buffer = pool.create_buffer(
                offset,
                self.width,
                self.height,
                stride,
                WlShm.format.argb8888,
            )
            item = Buffer(buffer, surface, memory)
            buffer.dispatcher["release"] = lambda *_args, b=item: setattr(b, "busy", False)
            self.buffers.append(item)
        os.close(memfd)

    def set_interactive(self, enabled: bool, rects: list[tuple[int, int, int, int]] | None = None) -> None:
        """Restrict input to UI rectangles; an empty region passes clicks through."""
        self._interactive = enabled
        self._input_rects = rects or []
        if not self.surface or not self.compositor:
            return
        region = self.compositor.create_region()
        if enabled:
            for x, y, w, h in self._input_rects:
                region.add(int(x), int(y), int(w), int(h))
        self.surface.set_input_region(region)
        region.destroy()
        if self.layer_surface:
            mode = (
                ZwlrLayerSurfaceV1.keyboard_interactivity.on_demand
                if enabled
                else ZwlrLayerSurfaceV1.keyboard_interactivity.none
            )
            self.layer_surface.set_keyboard_interactivity(mode)
        self.surface.commit()
        if self.display:
            self.display.flush()
        self.dirty = True

    def draw(self, painter) -> None:
        if not self.width or not self.height:
            return
        item = next((b for b in self.buffers if not b.busy), None)
        if item is None:
            return
        ctx = cairo.Context(item.surface)
        painter(ctx, self.width, self.height)
        item.surface.flush()
        item.busy = True
        assert self.surface is not None
        self.surface.attach(item.buffer, 0, 0)
        self.surface.damage(0, 0, self.width, self.height)
        self.surface.commit()
        if self.display:
            self.display.flush()
        self.dirty = False

    def dispatch(self, timeout: float = 0) -> None:
        if not self.display:
            return
        import select

        ready, _, _ = select.select([self.display.get_fd()], [], [], timeout)
        if ready:
            self.display.read()
            self.display.dispatch()

    def fileno(self) -> int:
        assert self.display is not None
        return self.display.get_fd()

    def close(self) -> None:
        self._destroy_buffers()
        if self.display:
            try:
                self.display.flush()
                self.display.disconnect()
            except Exception:
                pass
