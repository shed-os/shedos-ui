## ShedOS animated screensavers — English (US) catalog.
## Keys are referenced by t!() / I18n::t() in the Rust source;
## new keys here MUST also be added to en-US (this file) before
## any other locale, since this is the embedded fallback.

app-name = ShedOS animated screensaver
help-summary = Form animated SHEDOS art with random effects: terminal-based or Wayland fullscreen overlay

# --list / --list-effects output
list-effects-header = Available effects:
list-effect-line = { $key } — { $title } ({ $duration_ms } ms): { $description }

# --list-logos output
list-logos-header = Available logo variants:
list-logo-line = { $key } — { $title }: { $description }
list-logo-colors = Colors: { $palette }

# --help-effect output
help-effect-header = Effect "{ $name }":

# Errors
error-unknown-effect = unknown effect "{ $name }". Run `shedos-screensaver --list-effects` to see all effects.
error-unknown-logo = unknown logo variant "{ $name }". Run `shedos-screensaver --list-logos` to see all variants.
error-invalid-color = invalid color "{ $spec }"; expected #rrggbb, r,g,b, named ANSI, or Catppuccin shorthand.
error-no-default-effect = no effect selected; pass --effect or accept the default (random each cycle).
error-wayland-unavailable = Wayland mode requested but $WAYLAND_DISPLAY is not set; falling back to TTY.
error-audio-unavailable = audio source "{ $source }" requested but pipewire is not reachable; running without audio reactivity.
