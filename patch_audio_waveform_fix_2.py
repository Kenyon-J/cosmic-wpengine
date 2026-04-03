import re

with open("src/modules/audio.rs", "r") as f:
    content = f.read()

# Make sure we actually removed `recycle_waveform_rx.try_recv()` from the timeout branch
content = re.sub(r"let mut wave_buffer = recycle_waveform_rx\.try_recv\(\)\.map\(\|b\| b\.into_vec\(\)\)\.unwrap_or_else\(\|_\| Vec::with_capacity\(FFT_SIZE\)\);",
                 "let mut wave_buffer = Vec::with_capacity(FFT_SIZE);", content)

with open("src/modules/audio.rs", "w") as f:
    f.write(content)

print("Patch applied successfully.")
