//! Free-VT selection for lock-screen fast user switching. The locker
//! computes a candidate VT and hands it to the polkit-gated helper,
//! which re-validates it before starting a parallel login — the
//! unprivileged side only proposes.

use std::fs;

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
/// `/sys/class/tty/tty0/active`, occupied VTs from logind's per-session
/// `VTNr`. Defensive — an unreadable source only narrows what we treat
/// as busy, and the helper re-validates the result regardless.
pub fn first_free_vt() -> Option<u32> {
    first_free_vt_from(active_vt().unwrap_or(0), &busy_vts(), VT_CEILING)
}

fn active_vt() -> Option<u32> {
    let s = fs::read_to_string("/sys/class/tty/tty0/active").ok()?;
    s.trim().strip_prefix("tty")?.parse().ok()
}

fn busy_vts() -> Vec<u32> {
    let Ok(entries) = fs::read_dir("/run/systemd/sessions") else {
        return Vec::new();
    };
    let mut vts = Vec::new();
    for entry in entries.flatten() {
        // Session state lives in regular files; the `<id>.ref` entries
        // are FIFOs that would block a read. Skip everything else.
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let Ok(text) = fs::read_to_string(entry.path()) else {
            continue;
        };
        for line in text.lines() {
            if let Some(n) = line.strip_prefix("VTNr=") {
                if let Ok(vt) = n.trim().parse::<u32>() {
                    vts.push(vt);
                }
            }
        }
    }
    vts
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
