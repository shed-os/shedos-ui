use signal_hook::consts::signal::{SIGINT, SIGTERM, SIGUSR1};
use signal_hook::flag;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Listens for SIGINT, SIGTERM, SIGUSR1 and sets the returned flag
/// when any fires. The frame loop polls it per iteration and exits.
///
/// SIGUSR1 is hypridle's `on-resume` channel for tearing down the
/// `--idle-daemon` instance on user input.
pub struct SignalListener {
    flag: Arc<AtomicBool>,
}

impl SignalListener {
    pub fn install() -> std::io::Result<Self> {
        let flag = Arc::new(AtomicBool::new(false));
        flag::register(SIGINT, Arc::clone(&flag))?;
        flag::register(SIGTERM, Arc::clone(&flag))?;
        flag::register(SIGUSR1, Arc::clone(&flag))?;
        Ok(Self { flag })
    }

    /// SIGUSR1 (the dismiss convention) defaults to Term, so register it
    /// to a dead flag — a real no-op that can't kill the lock client.
    pub fn install_for_lock() -> std::io::Result<Self> {
        let flag = Arc::new(AtomicBool::new(false));
        flag::register(SIGINT, Arc::clone(&flag))?;
        flag::register(SIGTERM, Arc::clone(&flag))?;
        flag::register(SIGUSR1, Arc::new(AtomicBool::new(false)))?;
        Ok(Self { flag })
    }

    pub fn flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.flag)
    }

    pub fn fired(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn listener_installs_and_starts_unset() {
        let l = SignalListener::install().unwrap();
        assert!(!l.fired());
    }

    #[test]
    fn flag_handle_observes_changes() {
        let l = SignalListener::install().unwrap();
        let f = l.flag();
        assert!(!f.load(Ordering::Relaxed));
        f.store(true, Ordering::Relaxed);
        assert!(l.fired());
    }
}
