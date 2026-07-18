# Layout themes

A theme decides where everything sits on your desktop: the album art, track
info, lyrics, visualiser and weather. Themes are single TOML files in
`~/.config/cosmic-wallpaper/shaders/`, named `<theme>.toml`; the active one
is chosen in Settings → Layout Themes (or by setting `audio.style` in
`config.toml`).

Two ways to build one:

- **The editor** — Settings → Layout Themes. Pick an element, drag the
  sliders. Every change writes the file and the engine reloads it live, so
  your desktop is the preview.
- **By hand** — edit the file in anything; the engine picks up saves
  immediately while it runs. *Create Theme* in Settings writes a
  fully-commented starting file.

Every key is optional: missing keys take the defaults listed below, so a
theme file can be as short as the one thing it changes.

## Coordinates

Positions are `[x, y]` fractions of the screen. `[0.0, 0.0]` is the
top-left corner, `[1.0, 1.0]` the bottom-right, `[0.5, 0.5]` dead centre.
Each element is anchored at its own centre.

## `[album_art]`

| Key | Default | Meaning |
| --- | --- | --- |
| `position` | `[0.5, 0.5]` | Centre of the cover |
| `size` | `0.25` | Height as a fraction of screen height |
| `shape` | `"square"` | `"square"` or `"circular"` |

## `[track_info]`, `[lyrics]`, `[weather]`

All three text elements share the same keys:

| Key | Default | Meaning |
| --- | --- | --- |
| `position` | element-specific* | Anchor point of the text |
| `align` | element-specific* | `"left"`, `"center"` or `"right"` relative to the anchor |
| `size` | `1.0` | Font scale multiplier (0.5 = half size, 2.0 = double) |

\* defaults: track_info `[0.5, 0.10]` center · lyrics `[0.5, 0.85]` center ·
weather `[0.98, 0.05]` right.

The active lyric line renders 1.5× the base size and the whole lyric stack
scales together, so one `size` value keeps the proportions.

## `[visualiser]`

| Key | Default | Meaning |
| --- | --- | --- |
| `shape` | `"linear"` | `"linear"` (bars), `"circular"` (ring) or `"square"` |
| `position` | `[0.5, 0.5]` | Centre of the bars / ring |
| `size` | `0.25` | Bar span (linear) or ring radius (circular), as a fraction of screen height |
| `rotation` | `0.0` | Degrees, clockwise |
| `amplitude` | `1.0` | Bar height multiplier |
| `align` | `"center"` | Band ordering: low frequencies at the `left`, mirrored from `center`, or at the `right` |
| `color_top` | *(album palette)* | `[r, g, b]` 0–1 override for the bar tips |
| `color_bottom` | *(album palette)* | `[r, g, b]` 0–1 override for the bar bases |
| `shader` | *(none)* | Custom WGSL visualiser shader file name |

Leave the colours unset and the bars follow each track's album palette.

## `[effects]`

| Key | Default | Meaning |
| --- | --- | --- |
| `lyric_bounce` | `1.0` | How far the active lyric hops on the beat (0 = off) |
| `lyric_spring_stiffness` | `150.0` | Snappiness of the lyric scroll |
| `lyric_spring_damping` | `12.0` | Wobble control - lower is bouncier |
| `beat_pulse` | `1.0` | Visualiser pulse on detected beats (0 = off) |

## Top level

| Key | Default | Meaning |
| --- | --- | --- |
| `font_family` | *(system)* | Font for this theme's text; the user's global font setting wins when set |

## Examples

Ready-made themes live in [`themes/`](../themes/) in the repository — copy
one into your shaders folder, or drop it onto the Layout Themes page in
Settings. Contributions welcome: a theme is one file and a screenshot.

```toml
# Everything out of the way: art bottom-left, lyrics low, ring in the corner.
[album_art]
position = [0.12, 0.82]
size = 0.18

[track_info]
position = [0.12, 0.94]
align = "center"
size = 0.9

[lyrics]
position = [0.5, 0.92]
size = 1.1

[visualiser]
shape = "circular"
position = [0.88, 0.82]
size = 0.12
amplitude = 1.4
```
