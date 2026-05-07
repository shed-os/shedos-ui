# shedos-screensaver logo variants

Nine SHEDOS art variants ship in the binary's catalog. Each is a
different "font" rendition of the SHEDOS mark. Each cycle of the
screensaver picks one (random by default; `--logo=NAME` locks to
one).

Run `shedos-screensaver --list-logos` for the live catalog.

Each variant ships with a curated palette of Catppuccin Mocha
colors. The cycle engine picks one at random per session, so the
same logo appears in different palette members across cycles.
Pass `--color <name>` to lock to a specific color (any Catppuccin
Mocha key, plus `#rrggbb`, `r,g,b`, and named ANSI all work).

| Variant | Rows × Cols | Palette (first = canonical) | Notes |
|---|---|---|---|
| **block** | 5 × 47 | blue, mauve, green, peach, sapphire | Solid block letters. The canonical mark — also fastfetch's logo. |
| **ansi-shadow** | 6 × 49 | mauve, lavender, sky, sapphire, maroon | Block letters with depth shading via Unicode box-drawing. |
| **big** | 7 × 58 | green, yellow, peach, teal, red | Bold filled block letters at a larger scale. |
| **outline** | 5 × 35 | sky, lavender, pink, teal, rosewater | Hollow letters in box-drawing characters. |
| **3d-iso** | 5 × 48 | blue, sapphire, mauve, lavender, sky | Block letters with a single-cell ▒ depth shadow on each letter's right edge. |
| **gradient** | 5 × 47 | peach, yellow, mauve, blue, teal | Block letters with a vertical density gradient — █ at the top fading through ▓▒░ to the bottom. |
| **emboss** | 6 × 47 | red, peach, yellow, green, blue | Block letters with a single-row ░ drop shadow directly underneath. |
| **shadow-cast** | 7 × 49 | sapphire, mauve, red, green, peach | Block letters with a two-row offset drop shadow that fades from ▒ to ░. |
| **wide** | 5 × 52 | lavender, sky, teal, mauve, sapphire | Block letters with extra inter-letter spacing for a more open layout. |

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
