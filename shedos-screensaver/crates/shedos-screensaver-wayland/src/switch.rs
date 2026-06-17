//! Free-VT selection for lock-screen fast user switching. The locker
//! computes a candidate VT and hands it to the polkit-gated helper,
//! which re-validates it before starting a parallel login — the
//! unprivileged side only proposes.

use std::fs;
use std::process::Command;

/// VTs 1..=6 are reserved for the primary seat's text consoles; the
/// graphical seat and any parallel logins live at 7 and above.
const VT_FLOOR: u32 = 7;
/// Upper bound on VTs we hand out — well under the kernel's 63-console
/// limit. 7..=12 leaves room for several parallel logins.
const VT_CEILING: u32 = 12;

/// Lowest VT in `VT_FLOOR..=max_vt` that is neither `active` nor in
/// `busy`. `None` means exhaustion; the caller must fail closed.
fn first_free_vt_from(active: u32, busy: &[u32], max_vt: u32) -> Option<u32> {
    (VT_FLOOR..=max_vt).find(|vt| *vt != active && !busy.contains(vt))
}

/// Probe the system for a free VT: active console from
/// `/sys/class/tty/tty0/active`, occupied VTs from logind. Defensive — an
/// unreadable source only narrows what we treat as busy, and the helper
/// re-validates the result regardless.
pub fn first_free_vt() -> Option<u32> {
    first_free_vt_from(active_vt().unwrap_or(0), &busy_vts(), VT_CEILING)
}

fn active_vt() -> Option<u32> {
    let s = fs::read_to_string("/sys/class/tty/tty0/active").ok()?;
    s.trim().strip_prefix("tty")?.parse().ok()
}

/// VTs that already carry a logind session. Read from `loginctl`, not the
/// `/run/systemd/sessions` files — those omit the VT number on current
/// systemd, so parsing them would see every VT as free.
fn busy_vts() -> Vec<u32> {
    let Ok(out) = Command::new("loginctl")
        .args(["list-sessions", "--no-legend"])
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        // Columns: SESSION UID USER SEAT LEADER CLASS TTY IDLE SINCE.
        .filter_map(|line| line.split_whitespace().nth(6)?.strip_prefix("tty"))
        .filter_map(|n| n.parse::<u32>().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_lowest_free_above_floor() {
        assert_eq!(first_free_vt_from(1, &[], 12), Some(7));
    }

    #[test]
    fn skips_busy_and_active() {
        assert_eq!(first_free_vt_from(7, &[8, 9], 12), Some(10));
    }

    #[test]
    fn none_on_exhaustion() {
        assert_eq!(first_free_vt_from(7, &[8, 9, 10, 11, 12], 12), None);
    }
}
