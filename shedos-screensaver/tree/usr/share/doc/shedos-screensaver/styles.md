# shedos-screensaver styles

Eight animation styles ship in the initial release. Each renders
identically through the TTY (crossterm) and Wayland (wlr-layer-shell
+ wl_shm) backends; the only differences between modes are the
backdrop (Wayland mode optionally composites the user's hyprlock
wallpaper) and the input-dismissal behavior (Wayland grabs keyboard
exclusively unless `--idle-daemon` is set).

Run `shedos-screensaver --list` for the runtime-canonical list and
`shedos-screensaver --help-style <name>` for per-style options.

---

## logo-bounce — Bouncing SHEDOS

The SHEDOS ASCII art reads from `/etc/shedos-ascii.txt` and bounces
DVD-screensaver-style around the canvas. On each wall hit the color
shifts through a Catppuccin Mocha palette (blue → mauve → peach →
green → teal → red → yellow → sky, then loop).

Default color: `blue` (Catppuccin #89b4fa).
Audio reactivity: none.

| Option | Type | Default | Description |
|---|---|---|---|
| `speed` | f32 (0.1–10.0) | 1.0 | Multiplier on bounce velocity. |
| `color_cycle` | bool | `true` | Disable to lock the color to `--color`. |

---

## matrix — Matrix Rain

Falling glyph trails with a bright (white) head and a fading tail.
The glyph set is selectable: katakana (default — the original
Matrix vibe), ascii (Latin alphanumerics), hex (0–9 and a–f), or
brand (cycling SHEDOS letters).

Default color: green (custom `#88c970`, slightly brighter than
Catppuccin's green so the head stands out).
Audio reactivity: a beat triggers a one-frame burst of new
spawns (4× density), making downbeats look like rain "splashing".

| Option | Type | Default | Description |
|---|---|---|---|
| `density` | f32 (0.0–1.0) | 0.5 | Probability per column per frame of starting a new trail. |
| `trail_length` | u32 (1–100) | 20 | Trail length in cells. |
| `glyphs` | enum | katakana | One of: katakana, ascii, hex, brand. |

---

## plasma — Plasma Field

Sin/cos blended truecolor plasma. Continuously varying.

Default color: mauve (Catppuccin #cba6f7).
Audio reactivity: bass deforms `freq_x`; treble deforms `freq_y`.

| Option | Type | Default | Description |
|---|---|---|---|
| `freq_x` | f32 (0.1–10.0) | 1.0 | X-axis spatial frequency. |
| `freq_y` | f32 (0.1–10.0) | 1.5 | Y-axis spatial frequency. |

---

## starfield — Warp Stars

3D-perspective stars stream outward from center. The SHEDOS logo
pulses at the vanishing point.

Default color: text (Catppuccin #cdd6f4 — neutral white).
Audio reactivity: a beat doubles the warp factor for that frame
(snapshot effect — feels like an FTL "kick").

| Option | Type | Default | Description |
|---|---|---|---|
| `count` | u32 (1–10000) | 200 | Number of stars. |
| `warp_factor` | f32 (1.0–100.0) | 5.0 | Speed of perspective motion. |

---

## conway — Conway's SHEDOS

Game of Life seeded from the SHEDOS silhouette. The SHEDOS pattern
is restamped onto the field every `reseed_interval` seconds, so
the colony never dies out completely.

Default color: peach (Catppuccin #fab387).
Audio reactivity: none (Life is deterministic; reactive jitter
would feel wrong).

| Option | Type | Default | Description |
|---|---|---|---|
| `rule` | str | `B3/S23` | B/S notation (e.g. `B36/S23` for HighLife). |
| `reseed_interval` | u32 (1–600) | 30 | Reseed from logo every N seconds. |

---

## tunnel — Tunnel

Concentric rings of glyphs zoom inward; SHEDOS logo glows at the
vanishing point.

Default color: blue (Catppuccin #89b4fa).
Audio reactivity: peak amplitude brightens the rings.

| Option | Type | Default | Description |
|---|---|---|---|
| `rings` | u32 (5–50) | 20 | Number of concentric rings. |
| `speed` | f32 (0.1–10.0) | 1.0 | Inward zoom speed multiplier. |

---

## waves — Wave Lattice

Two interfering sine waves sweep glyphs across the canvas. Phase
shifts continuously over time.

Default color: mauve (Catppuccin #cba6f7).
Audio reactivity: bass shifts wavelength inward (denser waves);
peak amplitude brightens the glyphs.

| Option | Type | Default | Description |
|---|---|---|---|
| `wavelength_x` | f32 (0.1–10.0) | 1.0 | X-axis wavelength. |
| `wavelength_y` | f32 (0.1–10.0) | 1.5 | Y-axis wavelength. |
| `speed` | f32 (0.1–10.0) | 1.0 | Phase advance per second. |

---

## mandala — SHEDOS Mandala

N-fold rotational symmetric kaleidoscope. A small kernel pattern
(SHEDOS-themed glyphs) is rotated and replicated; rotation +
growth animate over time.

Default color: peach (Catppuccin #fab387).
Audio reactivity: none.

| Option | Type | Default | Description |
|---|---|---|---|
| `symmetry` | u32 (2–16) | 8 | N-fold rotational symmetry. |
| `growth` | f32 (0.1–10.0) | 1.0 | Growth speed of kernel. |
