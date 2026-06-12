shedos-power(1)

# NAME

shedos-power - power menu overlay for ShedOS

# SYNOPSIS

*shedos-power*

# DESCRIPTION

*shedos-power* opens a fullscreen centred overlay listing the
power actions: *Lock screen*, *Sleep*, *Hibernate* (only when the
machine is configured for it), *Reboot*, and *Shut down*.
Lock is one click; every other action opens a
confirmation card before firing. The overlay claims a
*wlr-layer-shell-unstable-v1* surface
on the active output with exclusive keyboard interactivity, dims the
desktop behind a Catppuccin Mocha backdrop, and exits when the user
either commits an action or dismisses.

Invoked by the Waybar *custom/power* module via *shedman power*.
Action subcommands (*shedman power reboot* and friends) skip the
overlay and execute directly; the GUI is only used when *shedman
power* is called with no argument.

# INTERACTION

*Mouse*
	Click a row to select it. Lock executes immediately;
	Reboot and Shut down swap to a confirm card. On the confirm card, *Cancel*
	returns to the menu and the action button executes. Clicking
	outside the card dismisses.

*Keyboard*
	*j* / *Down* / *Tab* and *k* / *Up* / *Shift+Tab* move focus.
	On the confirm card *h* selects Cancel and *l* selects the
	action button. *Enter* activates the focused item.
	*Esc* or *q* cancels (returns to menu from the confirm card,
	dismisses from the menu).

# ACTIONS

*Lock screen*
	Runs *loginctl lock-session*. One click — no confirmation.

*Sleep*
	Runs *systemctl suspend*. One click — no confirmation; hypridle
	locks the session before the machine sleeps.

*Hibernate*
	Suspend to disk via *systemctl hibernate*, after confirmation.
	Shown only when the machine is configured for hibernation
	(kernel disk support and a resume target).

*Reboot*
	Runs *systemctl reboot* after confirmation.

*Shut down*
	Runs *systemctl poweroff* after confirmation.

# ENVIRONMENT

*WAYLAND_DISPLAY*
	The Wayland display socket. The binary refuses to start without
	a compositor that advertises *wlr-layer-shell-unstable-v1*.

*RUST_LOG*
	Log filter forwarded to *env_logger*. Defaults to *info*.

# EXIT STATUS

*0*
	Normal exit (user committed an action or dismissed).

*non-zero*
	Wayland connection failed, the layer-shell protocol is
	unavailable, or a font could not be loaded.

# SEE ALSO

*shedman*(1), *systemctl*(1), *loginctl*(1), *waybar*(5)
