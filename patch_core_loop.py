import re

with open("src/modules/renderer/core.rs", "r") as f:
    content = f.read()

# Replace `let current_outputs = &self.current_outputs_cache;` with `let current_outputs = &self.current_outputs_cache;` and fix loop
content = content.replace("let current_outputs = &self.current_outputs_cache;", "let current_outputs = &self.current_outputs_cache;")
content = content.replace("for info in &current_outputs {", "for info in current_outputs {")

with open("src/modules/renderer/core.rs", "w") as f:
    f.write(content)

print("Patch applied successfully.")
