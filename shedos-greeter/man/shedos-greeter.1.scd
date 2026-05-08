shedos-greeter(1)

# NAME

shedos-greeter - greetd greeter for ShedOS

# SYNOPSIS

*shedos-greeter*

# DESCRIPTION

*shedos-greeter* is the ShedOS greetd greeter. It renders a fullscreen
xdg-shell surface with a wallpaper background, the current clock and
date, a password input box, and a "Hi, _username_" greeting. On
Enter, it submits the typed password to greetd via the JSON-over-Unix-
socket IPC protocol (greetd validates server-side via PAM as root,
before any user environment exists). On success, greetd execs the
chosen session command for the authenticated user. On failure, the
greeter renders a red-bordered input + the PAM error string for two
seconds, then accepts a fresh password attempt.

All pixel composition (wallpaper blit, clock, prompt input, branding)
is delegated to the *shedos-prompt-ui* library so the greeter stays
visually identical to the lock screen. On multi-monitor setups, the
full UI is mirrored on every output (no dimming).

The greeter takes no GTK widgets and renders every pixel itself.
Wallpaper, fonts, and palette come from the theme reconciler at
_/etc/shedos/themes/current/_; the greeter listens for atomic theme
swaps via inotify and picks up new colors / wallpaper on the next
frame, without restart.

# ENVIRONMENT

*GREETD_SOCK*
	Path to greetd's Unix socket. Set by greetd when it execs the
	greeter command. The login flow is gated on this variable;
	leaving it unset (e.g. when running the binary outside
	greetd for visual development) lets the UI render but
	*Enter* will fail to authenticate.

*WAYLAND_DISPLAY*
	The Wayland display name. The greeter expects to run inside a
	Wayland compositor (typically *cage*(1) launched as
	greetd's *default_session.command*).

*RUST_LOG*
	Log filter (passed to *env_logger*). Defaults to *info*.

# FILES

_/etc/shedos/login-user_
	Username to authenticate. If present and non-empty, the
	greeter authenticates this user. If absent or empty, the
	greeter auto-detects the single regular user (uid in
	1000..65534 with a real shell) from _/etc/passwd_. ShedOS
	is single-user by design.

_/etc/shedos/themes/current/greeter.toml_
	Active theme palette + font selections + wallpaper paths,
	written atomically by the theme reconciler. Missing or
	corrupt → the greeter falls back per-field to bundled
	Catppuccin Mocha defaults.

_/etc/shedos/themes/current/wallpaper-blurred.png_
	Blurred wallpaper composed into the greeter background. The
	desktop daemon (*awww*(1)) consumes the sharp companion at
	_wallpaper.png_.

_/usr/share/shedos/wallpapers/_
	Bundled wallpapers (sharp + blurred) shipped by
	*shedos-branding*. Used as the fallback source when no
	theme is applied.

# EXIT STATUS

*0*
	Authentication and *StartSession* both succeeded; greetd
	will now exec the user's session.

*non-zero*
	A panic, missing Wayland connection, or other startup
	failure. greetd will normally relaunch the greeter on a
	non-zero exit.

# SEE ALSO

*greetd*(1), *cage*(1), *shedman-theme*(1)
