# shedos-screensaver logo variants

Eight SHEDOS art variants ship in the binary's catalog. Each is a
different "font" rendition of the SHEDOS mark. Each cycle of the
screensaver picks one (random by default; `--logo=NAME` locks to
one).

Run `shedos-screensaver --list-logos` for the live catalog.

| Variant | Rows × Cols | Default color | Notes |
|---|---|---|---|
| **block** | 5 × 47 | Catppuccin blue | Solid block letters. The canonical mark — also fastfetch's logo. |
| **ansi-shadow** | 6 × 49 | Catppuccin mauve | Block letters with depth shading via Unicode box-drawing. |
| **slant** | 5 × 36 | Catppuccin peach | Italic-style figlet font. |
| **big** | 6 × 43 | Catppuccin green | Wide rounded letters, figlet "big" font. |
| **small** | 4 × 28 | Catppuccin teal | Tight 4-row variant for narrow terminals. |
| **doom** | 6 × 37 | Catppuccin red | Mailbox-style figlet "doom" font. |
| **outline** | 5 × 36 | Catppuccin sky | Hollow letters in box-drawing characters. |
| **mini** | 2 × 23 | Catppuccin yellow | Compact 2-row variant for tiny canvases. |

## Auto-fitting

When the screensaver runs in random-logo mode, it filters the
catalog to variants whose dimensions fit the current canvas (with
a 1-cell margin). On a tiny terminal that can't fit any of the
larger variants, it falls back to the **mini** variant. So the
animation always has SHEDOS to render — never a blank canvas.

## Customizing

The catalog is compiled into the binary, so adding a new variant
is a code change (drop a `.txt` under
`crates/shedos-screensaver-logos/art/`, add a row to
`crates/shedos-screensaver-logos/src/lib.rs::LIBRARY`). For a
per-system override of the canonical *block* art, write to
`/etc/shedos-ascii.txt` — the binary prefers that file over the
embedded *block* copy when present (so fastfetch and the
screensaver stay in sync if you customize one).
