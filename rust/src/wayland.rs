use std::ffi::CString;
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cairo::{Context, ImageSurface};
use memmap2::MmapMut;
use nix::poll::{poll, PollFlags};
use nix::unistd::ftruncate;
use wayland_client::protocol::wl_buffer::{Event as BufferEvent, WlBuffer};
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_region::WlRegion;
use wayland_client::protocol::wl_shm::{Format, WlShm};
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, delegate_noop, globals::registry_queue_init};
use wayland_client::globals::GlobalListContents;
use wayland_client::protocol::wl_registry;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::{Layer, ZwlrLayerShellV1};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{Anchor, Event as LayerEvent, KeyboardInteractivity, ZwlrLayerSurfaceV1};

delegate_noop!(WaylandState: ignore WlCompositor);
delegate_noop!(WaylandState: ignore WlShm);
delegate_noop!(WaylandState: ignore WlShmPool);
delegate_noop!(WaylandState: ignore WlRegion);
delegate_noop!(WaylandState: ignore WlSurface);
delegate_noop!(WaylandState: ignore ZwlrLayerShellV1);

struct Buffer {
    buffer: WlBuffer,
    surface: ImageSurface,
    busy: Arc<AtomicBool>,
}

pub struct WaylandState {
    width: i32,
    height: i32,
    dirty: bool,
    resized: Option<(i32, i32)>,
    buffers: Vec<Buffer>,
    layer: Option<ZwlrLayerSurfaceV1>,
    surface: Option<WlSurface>,
    shm: Option<WlShm>,
    compositor: Option<WlCompositor>,
    qh: Option<QueueHandle<WaylandState>>,
}

impl WaylandState {
    fn new() -> Self {
        Self {
            width: 0, height: 0, dirty: true, resized: None, buffers: Vec::new(),
            layer: None, surface: None, shm: None, compositor: None, qh: None,
        }
    }

    fn create_buffers(&mut self) {
        self.destroy_buffers();
        if self.width <= 0 || self.height <= 0 {
            return;
        }
        let Some(shm) = self.shm.clone() else { return; };
        let Some(qh) = self.qh.clone() else { return; };
        let stride = self.width * 4;
        let size = (stride * self.height) as i64;
        for _ in 0..2 {
            let name = CString::new("tinyhand").unwrap();
            let fd = unsafe { libc::memfd_create(name.as_ptr(), 0) };
            if fd < 0 { continue; }
            let owned = unsafe { OwnedFd::from_raw_fd(fd) };
            if ftruncate(&owned, size as libc::off_t).is_err() { continue; }
            let memory = match unsafe { MmapMut::map_mut(&owned) } { Ok(memory) => memory, Err(_) => continue };
            let pool = shm.create_pool(owned.as_fd(), size as i32, &qh, ());
            let busy = Arc::new(AtomicBool::new(false));
            let buffer = pool.create_buffer(0, self.width, self.height, stride, Format::Argb8888, &qh, busy.clone());
            let surface = match ImageSurface::create_for_data(memory, cairo::Format::ARgb32, self.width, self.height, stride) {
                Ok(surface) => surface,
                Err(_) => continue,
            };
            self.buffers.push(Buffer { buffer, surface, busy });
        }
    }

    fn destroy_buffers(&mut self) {
        for item in self.buffers.drain(..) {
            item.buffer.destroy();
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for WaylandState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_registry::WlRegistry,
        _event: wl_registry::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlBuffer, Arc<AtomicBool>> for WaylandState {
    fn event(
        _state: &mut Self,
        _proxy: &WlBuffer,
        event: BufferEvent,
        busy: &Arc<AtomicBool>,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if matches!(event, BufferEvent::Release) {
            busy.store(false, Ordering::SeqCst);
        }
    }
}

impl Dispatch<ZwlrLayerSurfaceV1, ()> for WaylandState {
    fn event(
        state: &mut Self,
        proxy: &ZwlrLayerSurfaceV1,
        event: LayerEvent,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let LayerEvent::Configure { serial, width, height } = event {
            let w = width as i32;
            let h = height as i32;
            if w > 0 && h > 0 && (w, h) != (state.width, state.height) {
                state.width = w;
                state.height = h;
                state.create_buffers();
                state.resized = Some((w, h));
            }
            proxy.ack_configure(serial);
            state.dirty = true;
        }
    }
}

pub struct Canvas {
    queue: EventQueue<WaylandState>,
    state: WaylandState,
}

impl Canvas {
    pub fn new() -> Result<Self, String> {
        let conn = Connection::connect_to_env().map_err(|e| format!("Wayland connection failed: {e}"))?;
        let (globals, mut queue) = registry_queue_init::<WaylandState>(&conn)
            .map_err(|e| format!("Wayland registry failed: {e}"))?;
        let qh = queue.handle();
        let compositor: WlCompositor = globals.bind(&qh, 1..=6, ())
            .map_err(|_| "Missing wl_compositor".to_string())?;
        let shm: WlShm = globals.bind(&qh, 1..=3, ())
            .map_err(|_| "Missing wl_shm".to_string())?;
        let shell: ZwlrLayerShellV1 = globals.bind(&qh, 1..=4, ())
            .map_err(|_| "Missing zwlr_layer_shell_v1".to_string())?;
        let surface = compositor.create_surface(&qh, ());
        let layer = shell.get_layer_surface(
            &surface,
            None,
            Layer::Overlay,
            "tinyhand-linux".to_string(),
            &qh,
            (),
        );
        let input_region = compositor.create_region(&qh, ());
        surface.set_input_region(Some(&input_region));
        input_region.destroy();
        layer.set_anchor(Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right);
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.set_size(0, 0);
        surface.commit();
        conn.flush().map_err(|e| format!("Wayland flush failed: {e}"))?;

        let mut state = WaylandState::new();
        state.surface = Some(surface);
        state.layer = Some(layer);
        state.shm = Some(shm);
        state.compositor = Some(compositor);
        state.qh = Some(qh);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while state.width == 0 {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() { break; }
            queue.flush().map_err(|e| format!("{e}"))?;
            queue.dispatch_pending(&mut state).map_err(|e| format!("{e}"))?;
            let mut fd = [nix::poll::PollFd::new(queue.as_fd(), PollFlags::POLLIN)];
            if poll(&mut fd, remaining.as_millis().min(u16::MAX as u128) as u16).map_err(|e| format!("{e}"))? == 0 {
                continue;
            }
            if let Some(guard) = queue.prepare_read() {
                guard.read().map_err(|e| format!("{e}"))?;
            }
            queue.dispatch_pending(&mut state).map_err(|e| format!("{e}"))?;
        }
        if state.width == 0 {
            return Err("Layer surface was never configured by the compositor".into());
        }
        if state.buffers.is_empty() { state.create_buffers(); }
        Ok(Self { queue, state })
    }

    pub fn fd(&self) -> RawFd {
        self.queue.as_fd().as_raw_fd()
    }

    pub fn size(&self) -> (i32, i32) {
        (self.state.width, self.state.height)
    }

    pub fn dirty(&self) -> bool {
        self.state.dirty
    }

    pub fn set_dirty(&mut self, value: bool) {
        self.state.dirty = value;
    }

    pub fn take_resized(&mut self) -> Option<(i32, i32)> {
        self.state.resized.take()
    }

    pub fn dispatch(&mut self) -> Result<(), String> {
        self.queue.blocking_dispatch(&mut self.state).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn flush(&self) {
        let _ = self.queue.flush();
    }

    pub fn set_interactive(&mut self, enabled: bool, rects: &[(i32, i32, i32, i32)]) {
        let Some(compositor) = self.state.compositor.clone() else { return; };
        let Some(surface) = self.state.surface.clone() else { return; };
        let Some(qh) = self.state.qh.clone() else { return; };
        let region = compositor.create_region(&qh, ());
        if enabled {
            for &(x, y, w, h) in rects {
                region.add(x, y, w, h);
            }
        }
        surface.set_input_region(Some(&region));
        region.destroy();
        if let Some(layer) = &self.state.layer {
            let mode = if enabled { KeyboardInteractivity::OnDemand } else { KeyboardInteractivity::None };
            layer.set_keyboard_interactivity(mode);
        }
        surface.commit();
        self.flush();
        self.state.dirty = true;
    }

    pub fn draw<F: FnOnce(&Context, i32, i32)>(&mut self, painter: F) {
        let index = match self.state.buffers.iter().position(|b| !b.busy.load(Ordering::SeqCst)) {
            Some(index) => index,
            None => return,
        };
        let width = self.state.width;
        let height = self.state.height;
        {
            let item = &mut self.state.buffers[index];
            let ctx = Context::new(&item.surface).unwrap();
            painter(&ctx, width, height);
            let _ = item.surface.flush();
            item.busy.store(true, Ordering::SeqCst);
        }
        let Some(surface) = self.state.surface.clone() else { return; };
        let buffer = self.state.buffers[index].buffer.clone();
        surface.attach(Some(&buffer), 0, 0);
        surface.damage(0, 0, width, height);
        surface.commit();
        self.flush();
        self.state.dirty = false;
    }
}
