## ShedOS animated screensavers — English (US) catalog.
## Keys are referenced by t!() / I18n::t() in the Rust source;
## new keys here MUST also be added to en-US (this file) before
## any other locale, since this is the embedded fallback.

app-name = ShedOS animated screensaver
help-summary = Run an animated ShedOS screensaver in this terminal or as a Wayland overlay

# --list output
list-header = Available styles:
list-style-line = { $key } — { $title } (default color: { $color })

# --help-style output
help-style-header = Options for style "{ $name }":
help-style-line = { $key } ({ $ty }, default { $default }) — { $desc }
help-style-no-options = (no options)

# Style titles (mirrored in shedos-screensaver-styles when registry is wired)
style-logo-bounce-title = Bouncing SHEDOS
style-matrix-title = Matrix Rain
style-plasma-title = Plasma Field
style-starfield-title = Warp Stars
style-conway-title = Conway's SHEDOS
style-tunnel-title = Tunnel
style-waves-title = Wave Lattice
style-mandala-title = SHEDOS Mandala

# Errors
error-unknown-style = unknown style "{ $name }". Run `shedos-screensaver --list` to see all styles.
error-invalid-color = invalid color "{ $spec }"; expected #rrggbb, r,g,b, named ANSI, or Catppuccin shorthand.
error-invalid-style-opt = invalid --style-opt "{ $arg }"; expected KEY=VAL.
error-style-opt-out-of-range = option "{ $key }" for style "{ $style }" must be { $range }; got { $given }.
error-no-default-style = no style selected; pass --style or set [defaults].style in /etc/shedos/screensaver.toml.
error-wayland-unavailable = Wayland mode requested but $WAYLAND_DISPLAY is not set; falling back to TTY.
error-audio-unavailable = audio source "{ $source }" requested but pipewire is not reachable; running without audio reactivity.
