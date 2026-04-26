shedman-screensaver(1)

# NAME

shedman-screensaver - animated ShedOS screensaver (TTY + Wayland)

# SYNOPSIS

*shedman screensaver* [_options_]++
*shedos-screensaver* [_options_]

# DESCRIPTION

Runs an animated screensaver as either a terminal-based animation
(default when invoked from a TTY) or a Wayland-native fullscreen
overlay (used by hypridle on idle).

The same binary backs both surfaces — pass *--mode=tty* or
*--mode=wayland* to force one. The default *--mode=auto* picks
Wayland when *$WAYLAND_DISPLAY* is set and stdin is not a TTY.

Eight visual styles are bundled (see *STYLES* below). Each style
honors a *--color* override and a per-style typed *--style-opt*
override; the per-style schema is printed by *--help-style*.

# READ-ONLY OPTIONS

*--list*
	List all available styles with their default colors and exit.

*--help-style* _NAME_
	Print the option schema for one style and exit.

*--help*, *-h*
	Print full help and exit.

*--help-summary*
	Print a one-line summary (used by *shedman help*).

*--version*, *-V*
	Print version and exit.

*--complete-bash*, *--complete-zsh*, *--complete-fish*
	Emit the corresponding shell-completion script and exit.

# RENDER OPTIONS

*--mode* _tty_|_wayland_|_auto_
	Render backend. Default: *auto*.

*--style* _NAME_
	One of: *logo-bounce*, *matrix*, *plasma*, *starfield*,
	*conway*, *tunnel*, *waves*, *mandala*. Required unless
	*--random* is set or [defaults].style is set in
	/etc/shedos/screensaver.toml.

*--color* _SPEC_
	Override the style's default color. Accepts:
	- Hex: *#89b4fa*, *89b4fa*
	- RGB: *137,180,250*
	- Named ANSI: *red green blue cyan magenta yellow white black*
	- Catppuccin Mocha: *blue mauve peach text rosewater flamingo
	  pink red maroon yellow green teal sky sapphire lavender*
	  (and the surface/overlay/base/mantle/crust variants)

*--style-opt* _KEY=VAL_
	Per-style typed override. Repeatable. See *--help-style* for
	the per-style schema.

*--fps* _N_
	Frames per second; default 30 (TTY) / 60 (Wayland).

*--duration* _SECS_
	Auto-exit after this many seconds. Used by tests.

*--random*
	Pick a random style at start.

*--shuffle* _SECS_
	Rotate styles every N seconds.

*--idle-daemon*
	Long-running mode: ignores keyboard, exits only on SIGUSR1.
	Used by the hypridle integration so on-resume can deterministically
	end the screensaver.

*--audio-source* _desktop_|_mic_|_none_
	Audio reactivity source. Default: *none*.

*--wallpaper* _PATH_|_none_|_auto_
	Wayland-only background layer. *auto* uses
	~/.config/hypr/wallpaper.png if present.

*--wallpaper-dim* _F_
	Backdrop dimming multiplier (0.0..=1.0). Default: 0.3.

*--locale* _BCP47_
	Override system locale. Default: resolved from
	$LC_ALL → $LC_MESSAGES → $LANG → en-US.

# STYLES

*logo-bounce*
	DVD-screensaver bounce of the SHEDOS logo. Color cycles on
	wall hits.

*matrix*
	Falling glyph trails in the green Matrix tradition. Density,
	trail length, and glyph set tunable.

*plasma*
	Sin/cos blended plasma field. Optionally audio-reactive via
	bass/mid/treble bands.

*starfield*
	3D-perspective stars streaming outward from center. SHEDOS
	logo pulses at the vanishing point.

*conway*
	Conway's Game of Life seeded from the SHEDOS silhouette.
	Reseeds every 30 s by default.

*tunnel*
	Concentric rings of glyphs zoom toward the viewer; SHEDOS
	logo at the far end.

*waves*
	Two interfering sine waves sweep glyphs across the canvas.
	Phase shifts over time.

*mandala*
	8-fold rotational symmetric kaleidoscope using SHEDOS glyphs
	as the kernel.

# CONFIGURATION

/etc/shedos/screensaver.toml
	System-wide defaults. Backed up by pacman; user edits survive
	package upgrades. The [defaults] section sets *style*, *color*,
	*fps*, *mode*, *audio_source*, *wallpaper*, *wallpaper_dim*, and
	*locale*. Per-style sections [style.<name>] override the
	style's option defaults.

# HYPRIDLE INTEGRATION

On fresh installs, /etc/skel/.config/hypr/hypridle.conf includes
a listener that fires *shedos-screensaver --mode=wayland
--idle-daemon --shuffle 60* 60 s before the lock timeout, with an
*on-resume* hook that sends SIGUSR1 to tear it down.

For users on existing accounts (where /etc/skel doesn't apply),
copy the snippet from
/usr/share/doc/shedos-screensaver/hypridle-example.conf into your
~/.config/hypr/hypridle.conf.

# EXIT STATUS

*0*
	Success.

*2*
	Invalid arguments (unknown style, malformed color, malformed
	style-opt, etc.).

# FILES

/etc/shedos/screensaver.toml
	Defaults; backed up by pacman.

/etc/shedos-ascii.txt
	The canonical SHEDOS ASCII art consumed by the *logo-bounce*,
	*conway*, *starfield*, *tunnel*, and *mandala* styles.

/usr/share/doc/shedos-screensaver/
	Bundled docs: hypridle-example.conf, styles.md, audio-setup.md.

# SEE ALSO

*shedman*(1), *hypridle*(1), *hyprlock*(1)
