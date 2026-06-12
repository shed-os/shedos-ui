shedos-switcher(1)

# NAME

shedos-switcher - Alt-Tab window switcher overlay

# SYNOPSIS

*shedos-switcher* [*--prev*]

# DESCRIPTION

The classic Alt-Tab: a centered strip of every open window in
most-recently-used order, each card showing the real application
icon, class, and workspace. Opens preselecting the previous window,
so a quick tap toggles between the two most recent. While Alt is
held, *Tab*/*Shift+Tab* (or arrows) cycle; *releasing Alt switches*
to the selection. *Esc* cancels; *Enter* and clicking work too.
Each press of the bind re-runs the binary: the first invocation
opens the strip, every further one tells it to advance — *--prev*
steps backwards instead (Alt+Shift+Tab). Bound to *Alt+Tab* on the
default desktop. For fuzzy text search
over windows instead, type *$* in walker.

# SEE ALSO

*shedman-keybindings*(1)
