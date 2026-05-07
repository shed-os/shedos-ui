# shedos-screensaver logo variants

Four SHEDOS art variants ship in the binary's catalog. Each is a
different "font" rendition of the SHEDOS mark. Each cycle of the
screensaver picks one (random by default; `--logo=NAME` locks to
one).

Run `shedos-screensaver --list-logos` for the live catalog.

| Variant | Rows × Cols | Default color | Notes |
|---|---|---|---|
| **block** | 5 × 47 | Catppuccin blue | Solid block letters. The canonical mark — also fastfetch's logo. |
| **ansi-shadow** | 6 × 49 | Catppuccin mauve | Block letters with depth shading via Unicode box-drawing. |
| **big** | 7 × 58 | Catppuccin green | Bold filled block letters at a larger scale. |
| **outline** | 5 × 35 | Catppuccin sky | Hollow letters in box-drawing characters. |

## Auto-fitting

When the screensaver runs in random-logo mode, it filters the
catalog to variants whose dimensions fit the current canvas (with
a 1-cell margin). On a tiny terminal that can't fit any of the
larger variants, it falls back to whichever variant has the
fewest columns in the current catalog. So the animation always
has SHEDOS to render — never a blank canvas.

## Customizing

The catalog is compiled into the binary, so adding a new variant
is a code change (drop a `.txt` under
`crates/shedos-screensaver-logos/art/`, add a row to
`crates/shedos-screensaver-logos/src/lib.rs::LIBRARY`). For a
per-system override of the canonical *block* art, write to
`/etc/shedos-ascii.txt` — the binary prefers that file over the
embedded *block* copy when present (so fastfetch and the
screensaver stay in sync if you customize one).
