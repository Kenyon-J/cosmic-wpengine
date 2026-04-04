import re

with open("src/modules/renderer/core.rs", "r") as f:
    content = f.read()

# Add import for WaylandOutput
content = content.replace("use crate::modules::wayland::WaylandManager;", "use crate::modules::wayland::{WaylandManager, WaylandOutput};")

with open("src/modules/renderer/core.rs", "w") as f:
    f.write(content)

print("Patch applied successfully.")
