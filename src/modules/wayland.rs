// =============================================================================
// modules/wayland.rs
// =============================================================================
// Sets up the Wayland layer surface — the special surface type that sits
// behind all windows and acts as the wallpaper canvas.
//
// Key concepts for beginners:
//   - Wayland is the display protocol used by COSMIC (and most modern Linux)
//   - A "surface" is a drawable area managed by the compositor (cosmic-comp)
//   - "layer shell" is a protocol extension that lets apps declare a surface
//     as belonging to the background, bottom, top, or overlay layer
//   - We use the BACKGROUND layer, which sits behind everything else
// =============================================================================

use anyhow::{Context, Result};
use tracing::info;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle};
use std::ptr::NonNull;

use wayland_client::{
    backend::ObjectId,
    protocol::{wl_output, wl_registry, wl_surface::WlSurface},
    Connection, EventQueue, Proxy, QueueHandle,
};

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::wlr_layer::{
        Anchor, Layer, LayerShellHandler, LayerSurface, LayerSurfaceConfigure, LayerState,
    },
};

pub struct AppData {
    registry_state: RegistryState,
    output_state: OutputState,
    compositor_state: CompositorState,
    layer_state: LayerState,

    pub windows: Vec<WaylandWindowInfo>,
}

pub struct WaylandWindowInfo {
    pub output: wl_output::WlOutput,
    pub surface: WlSurface,
    pub layer: LayerSurface,
    pub width: u32,
    pub height: u32,
    pub scale_factor: i32,
    pub first_configure: bool,
}

delegate_registry!(AppData);
delegate_output!(AppData);
delegate_compositor!(AppData);
delegate_layer!(AppData);

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

        let layer = LayerSurface::builder()
            .namespace("cosmic-wallpaper")
            .size((0, 0))
            .anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT)
            .layer(Layer::Background)
            .exclusive_zone(-1)
            .output(output.clone())
            .build(qh, &self.layer_state, surface.clone())
            .expect("Failed to create layer surface");

        surface.commit();

        self.windows.push(WaylandWindowInfo {
            output,
            surface,
            layer,
            width: 1920,
            height: 1080,
            scale_factor: 1,
            first_configure: false,
        });
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
        info!("Monitor disconnected, removing layer surface...");
        self.windows.retain(|w| w.output != output);
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
    ) {}
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
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {}
    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
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

            win.first_configure = true;
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

/// Manages the Wayland connection and all background surfaces.
pub struct WaylandManager {
    display_ptr: *mut std::ffi::c_void,

    // Keep Wayland handles alive so the surface is not destroyed
    _conn: Connection,
    _event_queue: EventQueue<AppData>,

    pub app_data: AppData,
}

impl WaylandManager {
    /// Connect to the Wayland compositor and create a background layer surface.
    pub fn new() -> Result<Self> {
        info!("Connecting to Wayland compositor...");

        let conn = Connection::connect_to_env().context("Failed to connect to Wayland")?;
        let (globals, mut event_queue) =
            smithay_client_toolkit::registry::registry_queue_init::<AppData>(&conn)
                .context("Failed to initialize registry")?;
        let qh = event_queue.handle();

        let mut app_data = AppData {
            registry_state: RegistryState::new(&globals),
            output_state: OutputState::new(&globals, &qh),
            compositor_state: CompositorState::bind(&globals, &qh)
                .context("wl_compositor not available")?,
            layer_state: LayerState::bind(&globals, &qh)
                .context("wlr_layer_shell_v1 not available")?,
            windows: Vec::new(),
        };

        // Discover all outputs
        event_queue.roundtrip(&mut app_data)?;

        // Wait for the first configure event to get our assigned size
        while app_data.windows.iter().any(|w| !w.first_configure) {
            event_queue.blocking_dispatch(&mut app_data)?;
        }
        
        // Once configured, commit again to apply the configuration and show the surface
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

    /// Dispatch any pending Wayland events without blocking.
    pub fn dispatch_events(&mut self) -> Result<()> {
        self._event_queue.dispatch_pending(&mut self.app_data).context("Wayland event dispatch failed")?;
        Ok(())
    }
}
