shedman-screensaver(1)

# NAME

shedman-screensaver - animated SHEDOS screensaver (TTY + Wayland)

# SYNOPSIS

*shedman screensaver* [_options_]++
*shedos-screensaver* [_options_]

# DESCRIPTION

Animates SHEDOS ASCII art into existence using one of 26 forming
effects (rain, decrypt, scattered, glitch, hologram, …). Each cycle
picks a logo variant + an effect, animates the variant in, holds
the resolved art for *--hold* seconds, then picks a new pair and
loops. *The animation is how the art appears* — every effect takes
a target Frame (the SHEDOS variant rendered at the canvas size) and
drives the canvas from blank to that target.

The same binary backs both surfaces — pass *--mode=tty* or
*--mode=wayland* to force one. Default *--mode=auto* picks Wayland
when *$WAYLAND_DISPLAY* is set and stdin is not a TTY.

Four SHEDOS art variants ship in the catalog (block, ansi-shadow,
big, outline). Twenty-six effects ship in the registry.
With both axes random, that's 104 visually distinct sessions.

# READ-ONLY OPTIONS

*--list*, *--list-effects*
	List all available effects with title + duration + description.

*--list-logos*
	List all available SHEDOS logo variants.

*--help-effect* _NAME_
	Print the description of one effect (title, description,
	duration, audio-reactivity).

*--help*, *-h*
	Print full help and exit.

*--help-summary*
	Print a one-line summary (used by *shedman help*).

*--version*, *-V*
	Print version and exit.

*--complete-bash*, *--complete-zsh*, *--complete-fish*
	Emit the corresponding shell-completion script and exit.

# RUNTIME OPTIONS

*--mode* _tty_|_wayland_|_auto_
	Render backend. Default: *auto*.

*--effect* _NAME_
	Lock to one effect (otherwise: random each cycle). One of
	the names from *--list-effects*.

*--logo* _VARIANT_
	Lock to one logo variant (otherwise: random each cycle).
	One of the names from *--list-logos*.

*--cycle* _NAME_
	Repeatable: build a custom rotation. Example:
	*--cycle rain --cycle decrypt --cycle matrix-rain* picks
	uniformly across only those three effects each cycle.

*--color* _SPEC_
	Override the target color (one for the entire session).
	Accepts:
	- Hex: *#89b4fa*, *89b4fa*
	- RGB: *137,180,250*
	- Named ANSI: *red green blue cyan magenta yellow white black*
	- Catppuccin Mocha: *blue mauve peach text rosewater flamingo
	  pink red maroon yellow green teal sky sapphire lavender* …

*--fps* _N_
	Frames per second; default 30 (TTY) / 60 (Wayland).

*--duration* _SECS_
	Auto-exit after this many seconds. Used by tests.

*--hold* _SECS_
	Hold the resolved art for this many seconds between effects.
	Default: 3. Pass *--hold 0* for one-shot mode: animate the art
	in once and sit on the resolved frame until *--duration* /
	SIGINT / SIGUSR1 / a keypress.

*--idle-daemon*
	Long-running mode for hypridle: ignores keyboard, exits only
	on SIGUSR1.

*--audio-source* _desktop_|_mic_|_none_
	Audio reactivity source. Default: *none*. See
	/usr/share/doc/shedos-screensaver/audio-setup.md.

*--wallpaper* _PATH_|_none_|_auto_
	Wayland-only background layer.

*--wallpaper-dim* _F_
	Backdrop dimming multiplier (0..=1). Default: 0.3.

*--font-path* _PATH_
	Wayland-only font (defaults to system DejaVu Sans Mono).

*--cell-height-px* _PX_
	Wayland-only cell pixel height. Default: 18.

*--locale* _BCP47_
	Override system locale.

# EFFECTS

The 41 ship effects. Run *shedos-screensaver --list* for the live
catalog with durations.

*rain*
	Characters fall from above into target positions, staggered.

*decrypt*
	Movie-style decryption: ciphertext flickers in every cell,
	resolves cell by cell.

*print*
	Typewriter reveal with blinking cursor.

*scattered*
	Cells start scattered across the canvas and fly to position
	with eased motion.

*wipe*
	Diagonal sweep front; cells reveal in its wake.

*slide*
	Even rows slide in from the left, odd rows from the right.

*expand*
	Cells emerge from the canvas center and fly outward.

*crumble*
	Gravity-driven fall with sinusoidal horizontal drift.

*spotlights*
	Three orbiting spotlights illuminate cells progressively.

*burn*
	Flame front rises from the bottom; cells appear glowing
	through ember to the final color.

*colorshift*
	Target shown immediately; cells cycle through Catppuccin
	palette before settling.

*glitch*
	Datamosh corruption: rows scroll/garble, then heal.
	Audio-reactive: beats spike destabilization.

*quantum*
	Wavefunction collapse: superposed glyphs flicker, then each
	cell collapses to its definite target.

*synthgrid*
	Synthwave perspective grid forms, then dissolves into the
	target.

*matrix-rain*
	Green katakana rain falls; trails freeze into target cells.

*hologram*
	CRT scanlines sweep cyan; cells solidify in the wake.

*neon-trace*
	Cells light up neon-pink/cyan one by one along a diagonal sweep,
	then fade to the target color. CRT vector-display feel.

*blackhole*
	Cells spiral into the canvas center under gravity, vanish, then
	explode outward to their target positions. Dramatic rebirth.

*shockwave*
	A radial pulse expands from the canvas center; cells reveal as
	the wavefront passes them, with a bright leading edge.

*liquid-fill*
	Cells fill from the bottom up like rising water; surface oscillates
	with foam at the meniscus.

*constellation*
	Cells appear as dots, brighten into stars, then morph into
	letterforms — the SHEDOS art drawn as a star map.

*interlace*
	CRT interlace reveal: even rows sweep left-to-right first, then
	odd rows fill in. Cyan leading edge.

*thermal*
	Cells heat up through a thermal colormap (blue → cyan → yellow
	→ red → white) and settle to the final color. No motion — pure color.

*data-stream*
	Streams of 1s and 0s flow across each row; cells lock to the
	target glyph as the stream passes them.

*tetris*
	Each lit cell is a falling block; blocks per column land
	bottom-up and lock at their target rows.

*boot-sequence*
	Fake terminal boot log scrolls up; the final line is `Initializing
	SHEDOS…` and the art emerges from below.

*fog-clear*
	Full canvas covered by a fog texture; the fog dissipates radially
	outward from center, revealing SHEDOS as it thins.

*lightning-strike*
	Branched lightning bolts strike at random angles; each bolt
	illuminates a cluster of cells in a brief white flash that fades
	to the target color.

*echo-fade*
	Multiple ghost copies of SHEDOS appear at offsets and converge
	toward canonical position; each fades in then out, the final
	copy solidifies.

*fault-line*
	A horizontal seismic crack tears the canvas open at the middle
	row; the halves separate, revealing SHEDOS in the widening gap.

*vortex*
	A clockwise spiral arm sweeps inward from canvas perimeter to
	center; cells reveal at canonical positions as the arm passes
	their (angle, radius).

*snowstorm*
	Snowflakes drift down with gentle horizontal sway; each flake
	sticks to its assigned SHEDOS cell on arrival.

*fountain*
	Particles spray upward from a center point at the canvas
	bottom, arc outward under gravity, and land at SHEDOS cell
	positions.

*ember-rise*
	Glowing embers float upward from below with flickering
	brightness; settle into the SHEDOS shape from bottom up.

*dust-settle*
	Dust motes swirl chaotically with decaying amplitude;
	crystallize into the SHEDOS shape as the swirl damps to zero.

*firework*
	Three burst points emit colored sparks that drift along
	parabolic arcs to SHEDOS cells.

*brick-build*
	Cells slide in from the nearest edge like bricks stacking up;
	bottom rows arrive first so the wall builds upward.

*hexagon-tile*
	Cells reveal in a diagonal hex-flow sweep; each shows a brief
	hex glyph during its flip, then settles to the target.

*pixelate-in*
	A coarse 8× pixelated SHEDOS appears immediately and refines
	through 4× and 2× until the art is sharp.

*circuit-trace*
	Bright PCB-style traces draw between consecutive SHEDOS cells
	in a greedy nearest-neighbor traversal; trace cells fade after
	the connecting cell materializes.

*kaleidoscope*
	Top-left quadrant fills first; mirrored copies appear in the
	other three quadrants then fade as the canonical SHEDOS
	resolves.

# LOGO VARIANTS

Four SHEDOS art variants. Run *shedos-screensaver --list-logos*
for descriptions.

*block*, *ansi-shadow*, *big*, *outline*

# CONFIGURATION

/etc/shedos/screensaver.toml
	System-wide defaults. Backed up by pacman; user edits survive
	package upgrades.

# HYPRIDLE INTEGRATION

On fresh installs, /etc/skel/.config/hypr/hypridle.conf includes
a listener that fires
*shedos-screensaver --mode=wayland --idle-daemon* 60 s before
lock, with an *on-resume* hook that sends SIGUSR1.

For users on existing accounts (where /etc/skel doesn't apply),
copy the snippet from
/usr/share/doc/shedos-screensaver/hypridle-example.conf into your
~/.config/hypr/hypridle.conf.

# EXIT STATUS

*0* — success.++
*2* — invalid arguments (unknown effect/logo/color, malformed flag).

# FILES

/etc/shedos/screensaver.toml
	Defaults; backed up by pacman.

/etc/shedos-ascii.txt
	Per-system override of the canonical SHEDOS art (the *block*
	variant). Optional — the binary embeds all 8 variants at
	compile time, so this file is purely an override channel.

/usr/share/doc/shedos-screensaver/
	Bundled docs: hypridle-example.conf, effects.md, logos.md,
	audio-setup.md.

# SEE ALSO

*shedman*(1), *hypridle*(1), *hyprlock*(1)
