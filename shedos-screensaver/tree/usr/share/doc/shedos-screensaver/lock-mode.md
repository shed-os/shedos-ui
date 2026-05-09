# Lock mode

`shedos-screensaver --mode=lock` is the screen-lock client on
ShedOS. It claims the `ext-session-lock-v1` Wayland protocol, so
no other surface composites until it exits. `shedman lock` is the
user-facing entry point and is what hypridle fires after 5 minutes
of idle.

## The four-phase cycle

Once locked, the binary runs a deliberate cycle so the screen never
just goes black on you. The numbers below come from
`/etc/shedos/screensaver.toml`'s `[lock]` table; the flags
`--prompt-after-secs`, `--prompt-idle-hide-secs`, and
`--prompt-cycles` override them.

* **Screensaver phase** — the running screensaver (one of 46
  effects, picked uniformly per cycle) renders fullscreen. No
  prompt visible. Default duration before the prompt appears: 300
  seconds.
* **Prompt phase** — password input appears with the user's
  greeting, the fingerprint icon (if a finger is enrolled), and a
  hint line. Default visible duration before reverting: 120
  seconds.
* **Repeat** — Screensaver + Prompt alternate for
  `prompt_cycles` round trips (default 3).
* **DPMS off** — after the configured cycles, monitors power off
  via `wlr-output-power-management-unstable-v1`. Any keypress or
  pointer activity wakes them and returns to the prompt phase.

A successful authentication during any phase exits the binary and
releases the lock.

## Configuration

System-wide defaults live in `/etc/shedos/screensaver.toml`:

```toml
[lock]
prompt_after_secs = 300
prompt_idle_hide_secs = 120
prompt_cycles = 3
pam_service = "shedos-screensaver"
```

That file is in pacman's `backup=` list, so personal edits survive
upgrades.

For one-off overrides without editing the config, pass the
matching `--prompt-*` flag at invocation time. Useful for testing
the cycle on a short fuse:

```
shedos-screensaver --mode=lock \
    --prompt-after-secs=5 \
    --prompt-idle-hide-secs=10 \
    --prompt-cycles=2 \
    --duration=120
```

## Authentication

Two PAM stacks are wired in parallel.

The password path uses `/etc/pam.d/shedos-screensaver` which
includes `system-auth`. Whatever password unlocks your account
(via login, sudo, or polkit) unlocks the screen.

The fingerprint path uses `/etc/pam.d/shedos-screensaver-fp` which
runs `pam_fprintd.so` directly. It only spins up if the user has
at least one enrolled finger (probed at lock startup via
`fprintd-list`). The auth thread runs concurrently with the
keyboard listener; whichever returns success first releases the
lock.

The on-screen fingerprint icon reflects scan state in real time —
red on a `verify-no-match` and green on a `verify-match` before
the unlock dispatches. Idle no-touch periods stay visually quiet.

## Setting up fingerprint

```
shedman fingerprint detect    # confirms the sensor and driver
shedman fingerprint enroll    # walks 5 touches for right-index-finger
shedman fingerprint list      # sanity-check the enrollment landed
```

To use a different finger:

```
shedman fingerprint enroll left-thumb
```

To remove a single finger without losing others:

```
shedman fingerprint delete right-thumb
```

To remove every enrolled finger:

```
shedman fingerprint clear
```

## Matcher reliability

Linux fingerprint quality varies sharply across libfprint variants.
Drivers (especially small-area sensors like Goodix 53XD) ship with
permissive match thresholds — the upstream libfprint default is 40
on the BOZORTH3 score, but most drivers in the libfprint AUR forks
override it down to 24 to compensate for the limited minutiae a
small sensor captures. The trade-off is non-zero False Accept
Rate: occasionally, a wrong finger will match.

Characterise your sensor before relying on fingerprint unlock:

```
shedman fingerprint test
```

Defaults to 20 enrolled-finger trials and 20 wrong-finger trials.
A clean run reports 0 false accepts and a low (<10%) false reject
rate. Any non-zero false accept means the matcher is unsafe at
this driver's threshold for security-grade authentication. The
test exits with status `2` in that case.

## Recovery if you're locked out

If the lock client crashes or hangs, the compositor stays locked —
that's the contract of `ext-session-lock-v1`. Recovery from a
text console:

1. Switch to a tty: `Ctrl+Alt+F2`
2. Log in with username + password.
3. Unlock the locked session:
   ```
   loginctl unlock-session
   ```
4. Optionally kill the stale lock client:
   ```
   systemctl --user kill --signal=SIGTERM shedos-screensaver-lock
   ```
5. `Ctrl+Alt+F1` returns to the graphical session.

If `loginctl unlock-session` doesn't list the right session, run
`loginctl list-sessions` to find your session ID and pass it
explicitly.

## Troubleshooting

**Lock screen shows nothing / is black on a Hyprland session.** The
binary works around a Hyprland quirk where `configure(0, 0)` is
sent for fullscreen overlay layer surfaces; the fallback uses
`OutputState.info().logical_size`. If the screen is still black,
check `journalctl --user -u shedos-screensaver-lock -b` for a
binding error — the most common cause is the compositor not
advertising `ext_session_lock_manager_v1`.

**`fprintd-list` says "no devices."** Either fprintd isn't running
(`sudo systemctl restart fprintd`) or the kernel hasn't picked up
the sensor's USB driver. `lsusb | grep -i goodix` (or your
sensor's vendor) should show the device. If fprintd is up but the
sensor isn't detected, the wrong libfprint variant is loaded —
`shedman fingerprint detect` prints which package is installed,
and `pacman -Qq | grep libfprint` shows what's available.

**Fingerprint icon never reacts to touches.** pam_fprintd's verify
session may be silently failing the device claim. Run
`journalctl SYSLOG_IDENTIFIER=fprintd -f` in another tty and
trigger the lock — if `VerifyStart` never fires, fprintd's polkit
policy is denying the call. The default ShedOS policy permits
active-session users; check that your session is `Active=yes` via
`loginctl show-session $XDG_SESSION_ID`.

**Password unlock works but fingerprint doesn't, even after a clean
re-enrollment.** Some libfprint matcher implementations time out
the verify session if the user types into the password field while
the fingerprint thread is mid-scan. Touch the sensor first; or use
the keyboard alone if you're going to use the keyboard.
