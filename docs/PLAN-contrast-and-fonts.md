# Plan: text contrast enforcement + per-theme fonts

Two independent features. Implement in order; each is a self-contained commit.
Everything you need is listed here - do NOT explore the repo beyond the
file:line anchors given (they were verified against master @ da9b2d0).

## Context primer (read only these regions)

| File | What it is |
|---|---|
| `src/modules/colour/mod.rs` (whole file, ~90 lines) | Color helpers: `extract_palette`, `lerp_colour`, `time_to_sky_colour`. Home for new contrast helpers. Tests in `colour/tests.rs`. |
| `src/modules/renderer/core/updates.rs:324-368` | `update_text_colors()` - the ONLY place text colors are chosen. Picks dark/light tinted text via a naive luminance threshold against `palette[0]`. |
| `src/modules/config/types.rs:97-110` | `ThemeLayout` struct (per-style layout loaded from `~/.config/cosmic-wallpaper/shaders/<style>.toml`, serde with per-field defaults). |
| `src/modules/config/types.rs:340-351` | `impl Default for ThemeLayout`. |
| `src/modules/config/types.rs:353-~420` | `ThemeLayout::load()` - parses the TOML or falls back to built-in per-style defaults (`monstercat`, `symmetric`, `waveform` blocks). |
| `src/modules/renderer/draw.rs:380-383` | Where the font is resolved each frame: `config.appearance.font_family` -> `Family::Name`, else `Family::SansSerif`. |
| `src/modules/config/types.rs:76` | Existing global `appearance.font_family: Option<String>` (already user-editable in the GUI - do not touch GUI code). |

Notes that save you from dead ends:
- Palette colors are 0.0-1.0 sRGB-ish floats (`[[f32; 3]]`), palette[0] = dominant.
- Text buffers are cached, but `Event::ConfigUpdated` already clears
  `text_buffer_cache`, and theme (style) changes always flow through
  ConfigUpdated - so font changes need NO cache-invalidation work.
- cosmic-text falls back to a default font automatically when a named family
  isn't installed - no error handling needed for bad font names.
- `update_text_colors()` is called on every track/palette change; keep it
  allocation-free.

---

## Feature 1: contrast-checked text colors

Problem: the current dark/light flip at luminance 0.55 with a 0.3 accent tint
can produce low-contrast text on mid-tone album palettes.

### 1a. Add helpers to `src/modules/colour/mod.rs`

```rust
/// WCAG relative luminance of an sRGB color (components 0.0-1.0).
pub fn relative_luminance(c: [f32; 3]) -> f32 {
    fn lin(u: f32) -> f32 {
        if u <= 0.04045 { u / 12.92 } else { ((u + 0.055) / 1.055).powf(2.4) }
    }
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

/// WCAG contrast ratio, 1.0..=21.0.
pub fn contrast_ratio(a: [f32; 3], b: [f32; 3]) -> f32 {
    let (l1, l2) = {
        let la = relative_luminance(a);
        let lb = relative_luminance(b);
        (la.max(lb), la.min(lb))
    };
    (l1 + 0.05) / (l2 + 0.05)
}

/// Returns `text` adjusted (blended toward black or white, whichever helps)
/// until it reaches `min_ratio` against `bg`. Preserves hue as long as
/// possible; ends at pure black/white if the ratio is unreachable otherwise.
pub fn ensure_contrast(text: [f32; 3], bg: [f32; 3], min_ratio: f32) -> [f32; 3] {
    if contrast_ratio(text, bg) >= min_ratio {
        return text;
    }
    // Blend toward whichever pole is further from the bg's luminance.
    let target = if relative_luminance(bg) > 0.179 { [0.0; 3] } else { [1.0; 3] };
    // 16 fixed steps is plenty; binary search is overkill for a wallpaper.
    for i in 1..=16 {
        let t = i as f32 / 16.0;
        let candidate = lerp_colour(text, target, t);
        if contrast_ratio(candidate, bg) >= min_ratio {
            return candidate;
        }
    }
    target
}
```

(0.179 is the WCAG luminance where black and white text give equal contrast.)

### 1b. Rewire `update_text_colors()` (updates.rs:324)

Keep the existing accent-tint aesthetic as the *candidate* color, then enforce
contrast against `text_bg_color` (palette[0] - the dominant color of the art
the text sits over):

- Compute the tinted rgb exactly as today (both branches can collapse: pick
  the branch by `relative_luminance(text_bg_color) > 0.179` instead of the
  0.299/0.587/0.114 threshold, keeping tint math unchanged).
- `let rgb = crate::modules::colour::ensure_contrast(tint, text_bg_color, 4.5);`
- `primary_text_color = [rgb, 1.0]`, `secondary_text_color = [rgb, 0.7 or 0.45]`
  (alphas exactly as today per branch), `text_color_diff` recomputed as today.

Do not change anything else in the file.

### 1c. Tests (append to `src/modules/colour/tests.rs`)

- `relative_luminance([0.;3]) == 0.0`, `([1.;3])` ~= 1.0.
- `contrast_ratio(black, white)` ~= 21.0 (assert > 20.9).
- `ensure_contrast` on a mid-gray text/mid-gray bg returns a color with
  ratio >= 4.5.
- A color already >= 4.5 is returned unchanged.

---

## Feature 2: per-theme font

Goal: each style's `.toml` (and built-in defaults) can name a font; the
user's global `appearance.font_family` still wins when set.
Resolution order: `appearance.font_family` > `theme.font_family` > SansSerif.

### 2a. `ThemeLayout` field (types.rs:97-110)

```rust
#[serde(default)]
pub font_family: Option<String>,
```

- Add `font_family: None,` to `impl Default for ThemeLayout` (types.rs:340).
- In `ThemeLayout::load()`'s built-in blocks, set a fitting default per style
  (these are taste defaults; cosmic-text silently falls back if not installed):
  - `monstercat`: `Some("Inter".to_string())`
  - `symmetric`: `Some("Inter".to_string())`
  - `waveform`: `Some("Fira Sans".to_string())`
- The file also embeds example-TOML template strings (search for the
  `[lyrics]` headers around types.rs:433/486/530/898): add a commented
  top-level line `# font_family = "Inter"` above the first section of each
  template so users discover the key.

### 2b. Resolution in draw.rs (lines 380-383)

```rust
let font_family_owned = renderer
    .state
    .config
    .appearance
    .font_family
    .clone()
    .or_else(|| renderer.theme.font_family.clone());
```
(The `.as_deref().map_or(Family::SansSerif, Family::Name)` line below stays.)

### 2c. Test (append to config tests in types.rs or its test module)

- A theme TOML string containing `font_family = "Foo"` parses with
  `Some("Foo")`; one without it parses with the serde default `None`.
- `ThemeLayout::load("monstercat")` (no file on disk) returns
  `font_family == Some("Inter")`.
  (Config tests already use an env mutex for HOME/XDG - reuse the existing
  pattern if the test touches `Config::config_dir()`.)

---

## Verification (both features)

```
cargo fmt && cargo clippy --all-targets && cargo test
```
All pre-existing tests must stay green (44 currently). For a visual check,
run `./target/debug/cosmic-wallpaper` with music playing; text should remain
readable on bright album art, and setting `font_family` in
`~/.config/cosmic-wallpaper/shaders/monstercat.toml` should change the font
live (config watcher picks it up).

## Out of scope - do not build

- Per-element fonts (separate lyrics/track/weather families), font weight or
  style attributes.
- Sampling the actual framebuffer/blur region for contrast (palette[0] is the
  agreed approximation).
- GUI changes of any kind (global font field already exists there).
- Anything in `mpris/`, `lrclib/`, `video/`, or the GUI binary.

Expected total diff: roughly +150 lines across 4 files.
