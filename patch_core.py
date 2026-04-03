import re

with open("src/modules/renderer/core.rs", "r") as f:
    content = f.read()

# 1. Add fields to Renderer struct
content = re.sub(
    r"text_buffer_cache: std::collections::HashMap<String, Buffer>,",
    r"text_buffer_cache: std::collections::HashMap<String, Buffer>,\n    text_buffers: Vec<PositionedBuffer>,\n    current_outputs_cache: Vec<WaylandOutput>,",
    content
)

# 2. Add initialization in new()
content = re.sub(
    r"text_buffer_cache: std::collections::HashMap::new\(\),",
    r"text_buffer_cache: std::collections::HashMap::new(),\n            text_buffers: Vec::new(),\n            current_outputs_cache: Vec::new(),",
    content
)

# 3. Update run() to use current_outputs_cache
content = re.sub(
    r"let current_outputs: Vec<_> = wayland_manager\.outputs\(\)\.collect\(\);",
    r"self.current_outputs_cache.clear();\n            self.current_outputs_cache.extend(wayland_manager.outputs());\n            let current_outputs = &self.current_outputs_cache;",
    content
)

# 4. Update draw_frame() to use self.text_buffers
content = re.sub(
    r"let mut text_buffers = Vec::new\(\);",
    r"self.text_buffers.clear();",
    content
)

# Now we need to replace all usages of text_buffers.push(...) to self.text_buffers.push(...) inside draw_frame
# First replace the macro usages
content = content.replace("text_buffers.push(", "self.text_buffers.push(")
content = content.replace("text_buffers.as_mut()", "self.text_buffers.as_mut()")

# Also replace the for loop iteration
content = content.replace("for p_buf in text_buffers {", "for p_buf in self.text_buffers.drain(..) {")

# Write back
with open("src/modules/renderer/core.rs", "w") as f:
    f.write(content)

print("Patch applied successfully.")
