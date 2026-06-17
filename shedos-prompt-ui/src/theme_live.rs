//! Shared live-theming helper. Owns the active `Theme`, a dirty flag,
//! and the inotify watcher, so every UI wires `shedman theme set`
//! reload in a few lines instead of re-implementing the
//! flag + watch + wake + reload dance (and re-introducing the
//! flag-set-but-loop-never-woken bug per crate).
//!
//! The crate stays event-loop agnostic: the loop wake is a
//! caller-supplied closure (`move || ping.ping()` for a calloop UI),
//! so no `calloop` dependency is pulled in here.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use crate::theme::Theme;
use crate::watch;

/// Split `Theme::CURRENT_DIR` into the parent the reconciler renames
/// into and the final component to match. The watch lives on the
/// parent, not on `current/` itself, so it survives the atomic rename.
fn current_parent_and_name() -> (&'static Path, &'static str) {
    let dir = Path::new(Theme::CURRENT_DIR);
    let name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .expect("Theme::CURRENT_DIR must have a final component");
    let parent = dir.parent().expect("Theme::CURRENT_DIR must have a parent");
    (parent, name)
}

/// Live-theming state for one UI surface: the owned mutable `Theme`,
/// the dirty flag the watcher sets, and the watcher handle (held for
/// the process lifetime).
pub struct LiveTheme {
    theme: Theme,
    theme_dirty: Arc<AtomicBool>,
    _watcher: Option<JoinHandle<()>>,
}

impl LiveTheme {
    /// Loads the active theme now, then arms the watcher whose callback
    /// sets the dirty flag with `Release` AND calls `wake` — a clone of
    /// the UI's calloop `Ping` wrapped as `move || ping.ping()`. The two
    /// halves are inseparable so a UI cannot end up with a flag that
    /// never wakes the loop. If the watcher fails to start it logs and
    /// continues with no live reload; the surface still paints.
    pub fn new<W>(wake: W) -> Self
    where
        W: FnMut() + Send + 'static,
    {
        let theme = Theme::load_or_default();
        let theme_dirty = Arc::new(AtomicBool::new(false));
        let watcher = spawn_watcher(theme_dirty.clone(), wake);
        Self { theme, theme_dirty, _watcher: watcher }
    }

    /// Call at the top of draw(), before composing. Claims the dirty
    /// flag with `swap(false, AcqRel)`; on a set flag reloads the theme
    /// and returns `true` so the caller can refresh its own derived
    /// caches (`cache.refresh_wallpaper(live.theme())`).
    pub fn reload_if_dirty(&mut self) -> bool {
        if self.theme_dirty.swap(false, Ordering::AcqRel) {
            log::info!("theme reload signaled; reloading from {}", Theme::CURRENT_DIR);
            self.theme = Theme::load_or_default();
            true
        } else {
            false
        }
    }

    /// Borrow the active theme for painting / cache refresh / render.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// `true` while the watcher is running; `false` if it failed to
    /// start (theming not live this process). Diagnostics only.
    pub fn is_live(&self) -> bool {
        self._watcher.is_some()
    }
}

/// Arm the watcher on the parent of `current/`. A start failure is
/// logged and swallowed (returns `None`) so a UI without inotify, or
/// with the theme dir absent, still paints.
fn spawn_watcher<W>(theme_dirty: Arc<AtomicBool>, mut wake: W) -> Option<JoinHandle<()>>
where
    W: FnMut() + Send + 'static,
{
    let (parent, name) = current_parent_and_name();
    match watch::watch(parent, name, move || {
        theme_dirty.store(true, Ordering::Release);
        wake();
    }) {
        Ok(handle) => Some(handle),
        Err(e) => {
            log::warn!("theme watcher disabled: {e:#} — live reload unavailable");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn current_dir_splits_into_parent_and_name() {
        let (parent, name) = current_parent_and_name();
        assert_eq!(parent, Path::new("/etc/shedos/themes"));
        assert_eq!(name, "current");
    }

    #[test]
    fn reload_is_false_when_not_dirty() {
        let mut live = LiveTheme::new(|| {});
        assert!(!live.reload_if_dirty(), "clean flag must not reload");
    }

    #[test]
    fn dirty_flag_drives_one_reload_then_clears() {
        let mut live = LiveTheme::new(|| {});
        live.theme_dirty.store(true, Ordering::Release);
        assert!(live.reload_if_dirty(), "set flag reloads once");
        assert!(
            !live.reload_if_dirty(),
            "flag is consumed by swap; second call is a no-op"
        );
    }

    #[test]
    fn wake_wiring_sets_flag_and_wakes() {
        // Exercise the set-flag-AND-wake pairing directly, without
        // depending on a live inotify event.
        let flag = Arc::new(AtomicBool::new(false));
        let count = Arc::new(AtomicUsize::new(0));
        let f = flag.clone();
        let c = count.clone();
        let callback = move || {
            f.store(true, Ordering::Release);
            c.fetch_add(1, Ordering::Release);
        };
        callback();
        assert!(flag.load(Ordering::Acquire));
        assert_eq!(count.load(Ordering::Acquire), 1);
    }
}
