use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Time source, abstracted so tests can drive frame loops with a
/// deterministic clock instead of waiting on real wall time.
pub trait Clock: Send + Sync {
    /// Monotonic elapsed since the clock was created.
    fn elapsed(&self) -> Duration;

    /// Sleep until at least `target` elapsed. May overshoot.
    /// Returns the new elapsed value.
    fn sleep_until(&self, target: Duration) -> Duration;
}

#[derive(Debug)]
pub struct RealClock {
    start: Instant,
}

impl RealClock {
    pub fn new() -> Self {
        Self { start: Instant::now() }
    }
}

impl Default for RealClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for RealClock {
    fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    fn sleep_until(&self, target: Duration) -> Duration {
        let now = self.elapsed();
        if target > now {
            std::thread::sleep(target - now);
        }
        self.elapsed()
    }
}

/// Deterministic clock for tests — `tick()` advances time;
/// `elapsed()` is monotonic and never moves on its own.
#[derive(Debug)]
pub struct MockClock {
    inner: Mutex<Duration>,
}

impl MockClock {
    pub fn new() -> Self {
        Self { inner: Mutex::new(Duration::ZERO) }
    }

    pub fn tick(&self, by: Duration) {
        let mut g = self.inner.lock().expect("MockClock mutex poisoned");
        *g += by;
    }

    pub fn set(&self, to: Duration) {
        let mut g = self.inner.lock().expect("MockClock mutex poisoned");
        *g = to;
    }
}

impl Default for MockClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for MockClock {
    fn elapsed(&self) -> Duration {
        *self.inner.lock().expect("MockClock mutex poisoned")
    }

    fn sleep_until(&self, target: Duration) -> Duration {
        let mut g = self.inner.lock().expect("MockClock mutex poisoned");
        if target > *g {
            *g = target;
        }
        *g
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_clock_starts_at_zero() {
        let c = MockClock::new();
        assert_eq!(c.elapsed(), Duration::ZERO);
    }

    #[test]
    fn mock_clock_tick_advances() {
        let c = MockClock::new();
        c.tick(Duration::from_millis(500));
        assert_eq!(c.elapsed(), Duration::from_millis(500));
        c.tick(Duration::from_millis(250));
        assert_eq!(c.elapsed(), Duration::from_millis(750));
    }

    #[test]
    fn mock_clock_sleep_jumps_to_target() {
        let c = MockClock::new();
        let now = c.sleep_until(Duration::from_secs(5));
        assert_eq!(now, Duration::from_secs(5));
    }

    #[test]
    fn mock_clock_sleep_in_past_is_noop() {
        let c = MockClock::new();
        c.set(Duration::from_secs(10));
        let now = c.sleep_until(Duration::from_secs(5));
        assert_eq!(now, Duration::from_secs(10));
    }

    #[test]
    fn real_clock_elapsed_is_monotonic() {
        let c = RealClock::new();
        let a = c.elapsed();
        std::thread::sleep(Duration::from_millis(2));
        let b = c.elapsed();
        assert!(b >= a);
    }
}
