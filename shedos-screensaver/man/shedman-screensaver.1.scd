shedman-screensaver(1)

# NAME

shedman-screensaver - animated SHEDOS screensaver (TTY + Wayland)

# SYNOPSIS

*shedman screensaver* [_options_]++
*shedos-screensaver* [_options_]

# DESCRIPTION

Animates SHEDOS ASCII art into existence using one of 16 forming
effects (rain, decrypt, scattered, glitch, hologram, …). Each cycle
picks a logo variant + an effect, animates the variant in, holds
the resolved art for *--hold* seconds, then picks a new pair and
loops. The Omarchy / TerminalTextEffects mental model: *the
animation is how the art appears*.

The same binary backs both surfaces — pass *--mode=tty* or
*--mode=wayland* to force one. Default *--mode=auto* picks Wayland
when *$WAYLAND_DISPLAY* is set and stdin is not a TTY.

Eight SHEDOS art variants ship in the catalog (block, ansi-shadow,
slant, big, small, doom, outline, mini). Sixteen effects ship in
the registry. With both axes random, that's 128 visually distinct
sessions.

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
	Default: 3.

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

The 16 ship effects. Run *shedos-screensaver --list* for the live
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

# LOGO VARIANTS

Eight SHEDOS art variants. Run *shedos-screensaver --list-logos*
for descriptions.

*block*, *ansi-shadow*, *slant*, *big*, *small*, *doom*, *outline*, *mini*

# CONFIGURATION

/etc/shedos/screensaver.toml
	System-wide defaults. Backed up by pacman; user edits survive
	package upgrades.

# HYPRIDLE INTEGRATION

On fresh installs, /etc/skel/.config/hypr/hypridle.conf includes
a listener that fires *shedos-screensaver --mode=wayland
--idle-daemon* 60 s before lock, with an *on-resume* hook that
sends SIGUSR1.

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
