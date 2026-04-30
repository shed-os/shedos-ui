shedos-greeter(1)

# NAME

shedos-greeter - greetd greeter that mirrors the hyprlock visual

# SYNOPSIS

*shedos-greeter* [_wallpaper-path_]

# DESCRIPTION

*shedos-greeter* is the ShedOS greetd greeter. It renders a fullscreen
wlr-layer-shell surface with a wallpaper background, the current clock
and date, a password input box, and a "Hi, _username_" greeting. On
Enter, it submits the typed password to greetd via the JSON-over-Unix-
socket IPC protocol (greetd validates server-side via PAM as root,
before any user environment exists). On success, greetd execs the
chosen session command for the authenticated user. On failure, the
greeter renders a red-bordered input + "Authentication Failed" line
for two seconds, then accepts a fresh password attempt.

The greeter takes no GTK widgets and renders every pixel itself, so
backgrounds, fonts, and chrome can be matched bit-for-bit to the
ShedOS hyprlock screen.

# OPTIONS

_wallpaper-path_
	Optional positional argument. Path to the wallpaper image
	(PNG or JPEG). Defaults to
	_/usr/share/shedos/wallpapers/dusk-blurred.png_.

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

_/usr/share/shedos/wallpapers/_
	Default wallpaper search path. Provided by *shedos-branding*.

# EXIT STATUS

*0*
	Authentication and *StartSession* both succeeded; greetd
	will now exec the user's session.

*non-zero*
	A panic, missing Wayland connection, missing wallpaper, or
	other startup failure. greetd will normally relaunch the
	greeter on a non-zero exit.

# SEE ALSO

*greetd*(1), *cage*(1), *hyprlock*(1)
