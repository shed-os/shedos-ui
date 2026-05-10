//! Inotify-backed theme watcher. Spawns a thread that watches the
//! parent of `/etc/shedos/themes/current/` and fires a callback once
//! per atomic swap performed by the reconciler.
//!
//! Watching the parent rather than `current/` itself: the reconciler
//! replaces `current/` via `os.rename`, so an inotify watch on the
//! old inode is torn down with it. `IN_MOVED_TO` on the parent
//! survives that.

use std::path::{Path, PathBuf};
use std::thread::{self, JoinHandle};

use anyhow::{Context, Result};
use inotify::{Inotify, WatchMask};

/// Spawn a watcher thread on `parent_dir`. Every time a sibling
/// directory matching `target_name` is moved into place by an
/// atomic rename, `callback` fires.
///
/// The returned `JoinHandle` is detachable; drop it to let the thread
/// run for the rest of the process lifetime. Init errors (open
/// inotify, add watch) bubble up synchronously; later errors are
/// logged and the thread keeps running.
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
        let h = watch(Path::new("/tmp"), "no-such-target-xxxxxxxxx", || {})
            .expect("spawn watcher");
        std::mem::forget(h); // intentionally leaked for the test's lifetime
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
