# shedos-screensaver effects

Twenty-six forming effects ship in the registry. Each takes a
SHEDOS art *target* and animates the canvas toward it. The
animation IS how the art appears.

Run `shedos-screensaver --list-effects` for the live registry, or
`--help-effect <name>` for one effect's details.

| Effect | Duration | Reactive? | What it does |
|---|---|---|---|
| **rain** | 4.5 s | yes | Characters fall from above into target positions. Beat triggers a one-frame burst. |
| **decrypt** | 5.5 s | no | Movie-style decryption: ciphertext flickers in every cell, resolves cell by cell. |
| **print** | 5 s | no | Typewriter reveal with a blinking cursor at the next-to-be-printed cell. |
| **scattered** | 4 s | no | Cells start scattered across the canvas and fly to position with eased motion. |
| **wipe** | 3.5 s | no | Diagonal sweep front; cells reveal in its wake with a bright leading edge. |
| **slide** | 3.5 s | no | Even rows slide in from the left, odd rows from the right. |
| **expand** | 3.5 s | no | Cells emerge from the canvas center and fly outward to target positions. |
| **crumble** | 4.5 s | no | Gravity-driven fall with sinusoidal horizontal drift. |
| **spotlights** | 5.5 s | no | Three orbiting spotlights illuminate cells progressively. |
| **burn** | 5 s | no | Flame front rises from the bottom; cells appear glowing through ember to the final color. |
| **colorshift** | 5 s | yes | Target shown immediately; cells cycle through the Catppuccin palette before settling. Peak amplitude speeds the cycle. |
| **glitch** | 5 s | yes | Datamosh corruption: rows scroll/garble, then heal. Beats spike destabilization. |
| **quantum** | 6 s | no | Wavefunction collapse: superposed glyphs flicker, then each cell collapses to its definite target. |
| **synthgrid** | 5.5 s | no | Synthwave perspective grid forms from the horizon, then dissolves into the target. |
| **matrix-rain** | 6.5 s | yes | Green katakana rain falls; trails freeze into target cells. |
| **hologram** | 5 s | no | CRT scanlines sweep cyan; cells solidify in their wake. |
| **neon-trace** | 5.5 s | no | Cells light up neon-pink/cyan one by one along a diagonal sweep, then fade to the target color. CRT vector-display feel. |
| **blackhole** | 6 s | no | Cells spiral into the canvas center under gravity, vanish at the singularity, then explode outward to their target positions. |
| **shockwave** | 4 s | no | A radial pulse expands from the canvas center; cells reveal as the wavefront passes them, with a bright leading edge. |
| **liquid-fill** | 5 s | no | Cells fill from the bottom up like rising water; surface oscillates with foam at the meniscus. |
| **constellation** | 5.5 s | no | Cells appear as dots, brighten into stars, then morph into letterforms — the SHEDOS art drawn as a star map. |
| **interlace** | 4.5 s | no | CRT interlace reveal: even rows sweep left-to-right first, then odd rows fill in. |
| **thermal** | 5 s | no | Cells heat up through a thermal colormap (blue → cyan → yellow → red → white) and settle to the final color. No motion — pure color. |
| **data-stream** | 5.5 s | no | Streams of 1s and 0s flow across each row; cells lock to the target glyph as the stream passes them. |
| **tetris** | 6 s | no | Each lit cell is a falling block; blocks per column land bottom-up and lock at their target rows. |
| **boot-sequence** | 7 s | no | Fake terminal boot log scrolls up; the final line is `Initializing SHEDOS…` and the art emerges from below. |

## Reactive effects

Effects flagged "reactive" consult the audio frame each step when
`--audio-source=desktop` or `--audio-source=mic` is set. The
non-reactive ones still work fine without audio; they just don't
respond to it.

## Adding new effects

A new effect is a single file under
`crates/shedos-screensaver-effects/src/effects/<name>.rs`
implementing the `Effect` trait, plus one row in `registry.rs`. The
contract:

```rust
pub trait Effect: Send {
    fn name(&self) -> &'static str;
    fn title(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn duration(&self) -> Duration;
    fn reactive(&self) -> bool { false }
    fn setup(&mut self, target: &Frame, ctx: &mut EffectCtx<'_>);
    fn step(&mut self, frame: &mut Frame, dt: Duration, audio: Option<&AudioFrame>) -> bool;
    fn reset(&mut self);
}
```

`setup()` captures the target Frame; `step()` advances the canvas
toward it and returns `true` when the canvas equals the target.
The integration tests in
`crates/shedos-screensaver-effects/tests/integration.rs`
automatically exercise every registered effect.
