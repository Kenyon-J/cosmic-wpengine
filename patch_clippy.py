import re

# 1. Fix unused into_raw method warning
with open("src/modules/event.rs", "r") as f:
    content = f.read()

content = content.replace("    pub fn into_raw(mut self) -> Box<[T]> {\n        self.buf.take().unwrap()\n    }", "")

with open("src/modules/event.rs", "w") as f:
    f.write(content)

# 2. Fix repeat.take to repeat_n
with open("src/modules/audio.rs", "r") as f:
    content = f.read()

content = content.replace("std::iter::repeat(0.0).take(FFT_SIZE / 2)", "std::iter::repeat_n(0.0, FFT_SIZE / 2)")
content = content.replace("std::iter::repeat(0.0).take(FFT_SIZE)", "std::iter::repeat_n(0.0, FFT_SIZE)")

with open("src/modules/audio.rs", "w") as f:
    f.write(content)

# 3. Fix redundant closure
with open("src/modules/config.rs", "r") as f:
    content = f.read()

content = content.replace("tokio::task::spawn_blocking(|| Self::load_or_default()).await", "tokio::task::spawn_blocking(Self::load_or_default).await")

with open("src/modules/config.rs", "w") as f:
    f.write(content)

print("Patch applied successfully.")
