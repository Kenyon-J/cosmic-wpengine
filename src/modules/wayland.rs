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

use anyhow::Result;
use tracing::info;

/// Represents our Wayland background surface.
/// The renderer uses this to present rendered frames to the compositor.
pub struct WaylandSurface {
    /// The raw Wayland surface handle — passed to wgpu for rendering
    pub width: u32,
    pub height: u32,

    /// Which output (monitor) this surface covers.
    /// Future: we'll have one WaylandSurface per monitor.
    pub output_name: String,
}

impl WaylandSurface {
    /// Connect to the Wayland compositor and create a background layer surface.
    pub fn new() -> Result<Self> {
        // NOTE: Full Wayland setup requires using smithay-client-toolkit to:
        //   1. Connect to the Wayland display (wayland-client)
        //   2. Bind the wl_compositor global (to create surfaces)
        //   3. Bind the zwlr_layer_shell_v1 global (to create layer surfaces)
        //   4. Create a wl_surface
        //   5. Wrap it in a zwlr_layer_surface_v1 configured for BACKGROUND layer
        //   6. Anchor to all edges and set exclusive zone to -1 (full screen)
        //   7. Commit the surface and wait for configure event
        //
        // This is fairly boilerplate-heavy. The smithay-client-toolkit crate
        // has an example called "layer_shell" that shows the full setup.
        //
        // For now, we return a placeholder so the rest of the code compiles.

        info!("Connecting to Wayland compositor...");

        // In the real implementation, we'd query the output dimensions
        // from the compositor's xdg-output protocol
        let width = 1920;
        let height = 1080;

        info!("Layer surface created: {}x{}", width, height);

        Ok(Self {
            width,
            height,
            output_name: "primary".to_string(),
        })
    }

    /// Returns the raw surface handle for wgpu to render into.
    /// In the real implementation this would return a wgpu-compatible
    /// Wayland surface handle.
    pub fn raw_handle(&self) -> WaylandHandle {
        WaylandHandle {
            width: self.width,
            height: self.height,
        }
    }
}

/// A placeholder handle passed to wgpu.
/// In the real implementation this would wrap the actual wl_surface pointer
/// that wgpu's Vulkan/EGL backend uses to create a swap chain.
pub struct WaylandHandle {
    pub width: u32,
    pub height: u32,
}
