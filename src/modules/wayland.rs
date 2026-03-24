use anyhow::{Context, Result};
use tracing::info;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle};
use std::ptr::NonNull;

use wayland_client::{
    backend::WaylandError,
    protocol::{wl_callback::WlCallback, wl_output, wl_region::WlRegion, wl_surface::WlSurface},
    globals::registry_queue_init,
    Connection, Dispatch, EventQueue, Proxy, QueueHandle,
};

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::wlr_layer::{
        Anchor, Layer, LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure,
    },
};

pub struct AppData {
    registry_state: RegistryState,
    output_state: OutputState,
    compositor_state: CompositorState,
    layer_shell: LayerShell,

    pub is_transparent: bool,
    pub windows: Vec<WaylandWindowInfo>,
    pub dead_windows: Vec<WaylandWindowInfo>,
    pub configuration_serial: u32,
}

pub struct WaylandWindowInfo {
    pub output: wl_output::WlOutput,
    pub surface: WlSurface,
    pub layer: LayerSurface,
    pub width: u32,
    pub height: u32,
    pub scale_factor: i32,
    pub first_configure: bool,
    pub frame_pending: bool,
    pub frame_callback: Option<WlCallback>,
    pub last_frame_request: std::time::Instant,
}

delegate_registry!(AppData);
delegate_output!(AppData);
delegate_compositor!(AppData);
delegate_layer!(AppData);

impl Dispatch<WlRegion, ()> for AppData {
    fn event(
        _state: &mut Self,
        _proxy: &WlRegion,
        _event: <WlRegion as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // WlRegion has no events to handle
    }
}

impl ProvidesRegistryState for AppData {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState];
}

impl OutputHandler for AppData {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        info!("New monitor detected, creating layer surface...");
        let surface = self.compositor_state.create_surface(qh);

        let layer = self.layer_shell.create_layer_surface(
            qh,
            surface.clone(),
            Layer::Background,
            Some("cosmic-wallpaper".to_string()),
            Some(&output),
        );
        layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        layer.set_exclusive_zone(-1);
        layer.set_size(0, 0);

        surface.commit();

        self.windows.push(WaylandWindowInfo {
            output,
            surface,
            layer,
            width: 1920,
            height: 1080,
            scale_factor: 1,
            first_configure: false,
            frame_pending: false,
            frame_callback: None,
            last_frame_request: std::time::Instant::now(),
        });
        self.configuration_serial += 1;
    }
    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {}
    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        info!("Monitor disconnected, scheduling surface removal...");
        let mut i = 0;
        while i < self.windows.len() {
            if self.windows[i].output == output {
                self.dead_windows.push(self.windows.remove(i));
                self.configuration_serial += 1;
            } else {
                i += 1;
            }
        }
    }
}

impl CompositorHandler for AppData {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        surface: &WlSurface,
        new_factor: i32,
    ) {
        if let Some(win) = self.windows.iter_mut().find(|w| &w.surface == surface) {
            win.scale_factor = new_factor;
            surface.set_buffer_scale(new_factor);
        }
    }
    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &WlSurface,
        _new_transform: wl_output::Transform,
    ) {}
    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &WlSurface,
        _time: u32,
    ) {
        if let Some(win) = self.windows.iter_mut().find(|w| &w.surface == _surface) {
            win.frame_pending = false;
            win.frame_callback = None;
        }
    }
    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &WlSurface,
        _output: &wl_output::WlOutput,
    ) {}
    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &WlSurface,
        _output: &wl_output::WlOutput,
    ) {}
}

impl LayerShellHandler for AppData {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, layer: &LayerSurface) {
        info!("Layer surface closed by compositor.");
        let mut i = 0;
        while i < self.windows.len() {
            if &self.windows[i].layer == layer {
                self.dead_windows.push(self.windows.remove(i));
                self.configuration_serial += 1;
            } else {
                i += 1;
            }
        }
    }
    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if let Some(win) = self.windows.iter_mut().find(|w| &w.layer == layer) {
            win.width = configure.new_size.0;
            win.height = configure.new_size.1;

            if win.width == 0 {
                win.width = 1920;
            }
            if win.height == 0 {
                win.height = 1080;
            }

            if !self.is_transparent {
                let region = self.compositor_state.wl_compositor().create_region(_qh, ());
                region.add(0, 0, win.width as i32, win.height as i32);
                win.surface.set_opaque_region(Some(&region));
                region.destroy();
            } else {
                win.surface.set_opaque_region(None);
            }

            win.first_configure = true;
            win.frame_pending = false;
            win.frame_callback = None;
        }
    }
}

pub struct WaylandOutput {
    pub width: u32,
    pub height: u32,
    display_ptr: *mut std::ffi::c_void,
    surface_ptr: *mut std::ffi::c_void,
}

impl WaylandOutput {
    pub fn raw_display_handle(&self) -> RawDisplayHandle {
        let display = NonNull::new(self.display_ptr).unwrap_or(NonNull::dangling());
        let handle = WaylandDisplayHandle::new(display);
        RawDisplayHandle::Wayland(handle)
    }

    pub fn raw_window_handle(&self) -> RawWindowHandle {
        let surface = NonNull::new(self.surface_ptr).unwrap_or(NonNull::dangling());
        let handle = WaylandWindowHandle::new(surface);
        RawWindowHandle::Wayland(handle)
    }
}

pub struct WaylandManager {
    display_ptr: *mut std::ffi::c_void,

    _conn: Connection,
    _event_queue: EventQueue<AppData>,

    pub app_data: AppData,
}

impl WaylandManager {
    pub fn new() -> Result<Self> {
        info!("Connecting to Wayland compositor...");

        let conn = Connection::connect_to_env().context("Failed to connect to Wayland")?;
        let (globals, mut event_queue) =
            registry_queue_init::<AppData>(&conn).context("Failed to initialize registry")?;
        let qh: QueueHandle<AppData> = event_queue.handle();

        let mut app_data = AppData {
            registry_state: RegistryState::new(&globals),
            output_state: OutputState::new(&globals, &qh),
            compositor_state: CompositorState::bind(&globals, &qh)
                .context("wl_compositor not available")?,
            layer_shell: LayerShell::bind(&globals, &qh)
                .context("layer shell not available")?,
            is_transparent: false,
            windows: Vec::new(),
            dead_windows: Vec::new(),
            configuration_serial: 0,
        };

        event_queue.roundtrip(&mut app_data)?;

        while app_data.windows.iter().any(|w| !w.first_configure) {
            event_queue.blocking_dispatch(&mut app_data)?;
        }
        
        for win in &app_data.windows {
            win.surface.commit();
        }

        info!("Layer surfaces created for {} output(s)", app_data.windows.len());

        let display_ptr = conn.backend().display_ptr() as *mut std::ffi::c_void;

        Ok(Self {
            display_ptr,
            _conn: conn,
            _event_queue: event_queue,
            app_data,
        })
    }

    pub fn outputs(&self) -> Vec<WaylandOutput> {
        self.app_data.windows.iter().map(|w| WaylandOutput {
            width: w.width * (w.scale_factor as u32),
            height: w.height * (w.scale_factor as u32),
            display_ptr: self.display_ptr,
            surface_ptr: w.surface.id().as_ptr() as *mut _,
        }).collect()
    }

    pub fn dispatch_events(&mut self) -> Result<()> {
        let _ = self._conn.flush();
        if let Some(guard) = self._conn.prepare_read() {
            if let Err(e) = guard.read() {
                match e {
                    WaylandError::Io(err) if err.kind() == std::io::ErrorKind::WouldBlock => {}
                    _ => tracing::warn!("Error reading wayland events: {}", e),
                }
            }
        }
        self._event_queue.dispatch_pending(&mut self.app_data).context("Wayland event dispatch failed")?;
        Ok(())
    }

    pub fn update_opaque_regions(&mut self, is_transparent: bool) {
        self.app_data.is_transparent = is_transparent;
        let compositor = self.app_data.compositor_state.wl_compositor().clone();
        let qh = self._event_queue.handle();
        
        for win in &self.app_data.windows {
            if !is_transparent && win.width > 0 && win.height > 0 {
                let region = compositor.create_region(&qh, ());
                region.add(0, 0, win.width as i32, win.height as i32);
                win.surface.set_opaque_region(Some(&region));
                region.destroy();
            } else {
                win.surface.set_opaque_region(None);
            }
            win.surface.commit();
        }
    }

    pub fn cleanup_dead_windows(&mut self) {
        self.app_data.dead_windows.clear();
    }

    pub fn mark_frame_rendered(&mut self, index: usize) {
        let qh = self._event_queue.handle();
        if let Some(win) = self.app_data.windows.get_mut(index) {
            win.frame_pending = true;
            win.last_frame_request = std::time::Instant::now();
            win.frame_callback = Some(win.surface.frame(&qh, win.surface.clone()));
        }
    }

    pub fn is_frame_pending(&self, index: usize) -> bool {
        self.app_data.windows.get(index).is_some_and(|w| w.frame_pending || !w.first_configure)
    }

    pub fn any_monitor_ready(&self) -> bool {
        self.app_data.windows.is_empty() || self.app_data.windows.iter().any(|w| !w.frame_pending && w.first_configure)
    }

    pub fn is_occluded(&self) -> bool {
        if self.app_data.windows.is_empty() {
            return false;
        }
        self.app_data.windows.iter().all(|w| {
            w.frame_pending && w.last_frame_request.elapsed() > std::time::Duration::from_millis(100)
        })
    }
}
