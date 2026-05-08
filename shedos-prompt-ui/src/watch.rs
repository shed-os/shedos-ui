//! Inotify-backed theme watcher. Spawns a thread that watches the
//! parent of `/etc/shedos/themes/current/` and fires a callback once
//! per atomic swap performed by the reconciler.
//!
//! Why the parent directory? `current/` is replaced via
//! `os.rename(current.tmp, current)` — the OLD inode is gone after
//! the swap, so any inotify watch held on the old `current/` itself
//! is torn down. Watching the parent for `IN_MOVED_TO` events whose
//! name is `current` produces one stable event per swap regardless
//! of inode churn. The reconciler's `.applied-at` sentinel lives
//! inside the new directory and is the consumer's confirmation
//! signal that the swap is complete.

use std::path::{Path, PathBuf};
use std::thread::{self, JoinHandle};

use anyhow::{Context, Result};
use inotify::{Inotify, WatchMask};

/// Spawn a watcher thread on `parent_dir`. Every time `current` (or
/// any sibling directory matching `target_name`) is moved into place
/// by an atomic rename, `callback` fires.
///
/// The returned `JoinHandle` is detachable — drop it to let the
/// thread run for the rest of the process lifetime. Errors during
/// initialization (cannot open inotify, cannot add watch) bubble up
/// synchronously; errors after that are logged and the thread
/// continues so a transient hiccup doesn't permanently disable
/// theme reloads.
pub fn watch<F>(
    parent_dir: &Path,
    target_name: &str,
    mut callback: F,
) -> Result<JoinHandle<()>>
where
    F: FnMut() + Send + 'static,
{
    let inotify = Inotify::init()
        .context("inotify init failed (CONFIG_INOTIFY_USER missing?)")?;
    inotify
        .watches()
        .add(parent_dir, WatchMask::MOVED_TO | WatchMask::CREATE)
        .with_context(|| format!("inotify watch on {}", parent_dir.display()))?;

    let parent: PathBuf = parent_dir.to_path_buf();
    let target: String = target_name.to_string();
    let handle = thread::Builder::new()
        .name("shedos-prompt-ui-theme-watcher".to_string())
        .spawn(move || run(parent, target, inotify, &mut callback))
        .context("spawn theme watcher thread")?;
    Ok(handle)
}

fn run<F>(parent: PathBuf, target: String, mut inotify: Inotify, callback: &mut F)
where
    F: FnMut() + Send + 'static,
{
    log::info!(
        "theme watcher armed on {} for name={:?}",
        parent.display(),
        target
    );
    let mut buffer = [0u8; 4096];
    loop {
        match inotify.read_events_blocking(&mut buffer) {
            Ok(events) => {
                let mut fired = false;
                for ev in events {
                    if let Some(name) = ev.name {
                        if name.to_string_lossy() == target {
                            fired = true;
                        }
                    }
                }
                if fired {
                    callback();
                }
            }
            Err(e) => {
                log::warn!(
                    "theme watcher read error on {}: {} — sleeping 1s and retrying",
                    parent.display(),
                    e
                );
                thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watcher_spawns_without_error_on_existing_dir() {
        // /tmp definitely exists on any sane test runner.
        let h = watch(Path::new("/tmp"), "no-such-target-xxxxxxxxx", || {})
            .expect("spawn watcher");
        // The thread is running in the background; we can let it leak
        // for the rest of the test binary's lifetime — that's the
        // typical use pattern for a long-running watcher.
        std::mem::forget(h);
    }

    #[test]
    fn watcher_errors_on_missing_dir() {
        let r = watch(
            Path::new("/nonexistent/shedos-watcher-test"),
            "current",
            || {},
        );
        assert!(r.is_err(), "watch on missing dir should return Err");
    }
}
