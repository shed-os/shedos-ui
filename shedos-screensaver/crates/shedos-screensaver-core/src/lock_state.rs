//! Pure state machine for the four-phase lock-screen cycle.
//!
//! No Wayland, PAM, or render state, just timestamps and a phase
//! discriminant. Callers `tick(now)` per event-loop wake and
//! `on_input(now)` per keystroke, then act on `phase()`.

use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockPhase {
    Screensaver,
    Prompt,
    Dpms,
}

#[derive(Debug, Clone)]
pub struct LockStateConfig {
    /// Screensaver dwell before the prompt appears automatically.
    pub prompt_after: Duration,
    /// Prompt-idle dwell before the prompt hides and the screensaver resumes.
    pub prompt_idle_hide: Duration,
    /// Prompt→Screensaver round-trips before the lock screen powers monitors off.
    pub cycles_before_dpms: u32,
}

#[derive(Debug)]
pub struct LockState {
    config: LockStateConfig,
    phase: LockPhase,
    phase_entered_at: Instant,
    last_input_at: Instant,
    screensaver_visits: u32,
}

impl LockState {
    pub fn new(config: LockStateConfig, now: Instant) -> Self {
        Self {
            config,
            phase: LockPhase::Screensaver,
            phase_entered_at: now,
            last_input_at: now,
            screensaver_visits: 0,
        }
    }

    pub fn phase(&self) -> LockPhase {
        self.phase
    }

    pub fn screensaver_visits(&self) -> u32 {
        self.screensaver_visits
    }

    /// Apply every time-driven transition due as of `now`. May chain
    /// transitions (e.g. T3 elapsed → Screensaver, then visits ≥ N
    /// → Dpms) so a single call settles the state.
    pub fn tick(&mut self, now: Instant) {
        loop {
            let prev = self.phase;
            match self.phase {
                LockPhase::Screensaver => {
                    if self.screensaver_visits >= self.config.cycles_before_dpms {
                        self.enter(LockPhase::Dpms, now);
                    } else if now.saturating_duration_since(self.phase_entered_at)
                        >= self.config.prompt_after
                    {
                        self.enter(LockPhase::Prompt, now);
                        self.last_input_at = now;
                    }
                }
                LockPhase::Prompt => {
                    if now.saturating_duration_since(self.last_input_at)
                        >= self.config.prompt_idle_hide
                    {
                        self.screensaver_visits = self.screensaver_visits.saturating_add(1);
                        self.enter(LockPhase::Screensaver, now);
                    }
                }
                LockPhase::Dpms => {}
            }
            if self.phase == prev {
                return;
            }
        }
    }

    /// Apply a keypress event. Always updates `last_input_at` so the
    /// `Prompt` T3 timer is reset on every keystroke; phase transitions
    /// happen for `Screensaver` and `Dpms` (both go to `Prompt`).
    pub fn on_input(&mut self, now: Instant) {
        self.last_input_at = now;
        match self.phase {
            LockPhase::Screensaver => self.enter(LockPhase::Prompt, now),
            LockPhase::Prompt => {}
            LockPhase::Dpms => {
                self.screensaver_visits = 0;
                self.enter(LockPhase::Prompt, now);
            }
        }
    }

    /// Wall-clock duration from `now` until the next time-driven
    /// transition. `None` in `Dpms` (only input leaves it). Lets the
    /// event loop arm a calloop timer rather than polling.
    pub fn time_until_next_transition(&self, now: Instant) -> Option<Duration> {
        match self.phase {
            LockPhase::Screensaver => {
                if self.screensaver_visits >= self.config.cycles_before_dpms {
                    Some(Duration::ZERO)
                } else {
                    let deadline = self.phase_entered_at + self.config.prompt_after;
                    Some(deadline.saturating_duration_since(now))
                }
            }
            LockPhase::Prompt => {
                let deadline = self.last_input_at + self.config.prompt_idle_hide;
                Some(deadline.saturating_duration_since(now))
            }
            LockPhase::Dpms => None,
        }
    }

    fn enter(&mut self, phase: LockPhase, now: Instant) {
        self.phase = phase;
        self.phase_entered_at = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(t2_secs: u64, t3_secs: u64, n: u32) -> LockStateConfig {
        LockStateConfig {
            prompt_after: Duration::from_secs(t2_secs),
            prompt_idle_hide: Duration::from_secs(t3_secs),
            cycles_before_dpms: n,
        }
    }

    fn instant() -> Instant {
        Instant::now()
    }

    #[test]
    fn new_starts_in_screensaver() {
        let s = LockState::new(cfg(300, 120, 3), instant());
        assert_eq!(s.phase(), LockPhase::Screensaver);
    }

    #[test]
    fn new_has_zero_visits() {
        let s = LockState::new(cfg(300, 120, 3), instant());
        assert_eq!(s.screensaver_visits(), 0);
    }

    #[test]
    fn screensaver_holds_before_t2() {
        let now = instant();
        let mut s = LockState::new(cfg(300, 120, 3), now);
        s.tick(now + Duration::from_secs(299));
        assert_eq!(s.phase(), LockPhase::Screensaver);
    }

    #[test]
    fn screensaver_transitions_to_prompt_at_exact_t2() {
        let now = instant();
        let mut s = LockState::new(cfg(300, 120, 3), now);
        s.tick(now + Duration::from_secs(300));
        assert_eq!(s.phase(), LockPhase::Prompt);
    }

    #[test]
    fn screensaver_transitions_to_prompt_well_past_t2() {
        let now = instant();
        let mut s = LockState::new(cfg(300, 120, 3), now);
        s.tick(now + Duration::from_secs(1000));
        assert_eq!(s.phase(), LockPhase::Prompt);
    }

    #[test]
    fn prompt_entry_resets_input_clock() {
        let now = instant();
        let mut s = LockState::new(cfg(300, 120, 3), now);
        // Tick past T2 so we enter Prompt at now+300. T3 must time
        // from that entry, not from `now`.
        let entered = now + Duration::from_secs(300);
        s.tick(entered);
        // 119s after entry: still Prompt.
        s.tick(entered + Duration::from_secs(119));
        assert_eq!(s.phase(), LockPhase::Prompt);
    }

    #[test]
    fn prompt_holds_before_t3() {
        let now = instant();
        let mut s = LockState::new(cfg(0, 120, 3), now);
        s.tick(now); // → Prompt immediately (T2=0)
        assert_eq!(s.phase(), LockPhase::Prompt);
        s.tick(now + Duration::from_secs(119));
        assert_eq!(s.phase(), LockPhase::Prompt);
    }

    #[test]
    fn prompt_transitions_to_screensaver_at_exact_t3() {
        let now = instant();
        let mut s = LockState::new(cfg(300, 120, 3), now);
        s.tick(now + Duration::from_secs(300)); // → Prompt at +300
        assert_eq!(s.phase(), LockPhase::Prompt);
        s.tick(now + Duration::from_secs(420)); // T3 elapsed at +420
        assert_eq!(s.phase(), LockPhase::Screensaver);
        assert_eq!(s.screensaver_visits(), 1);
    }

    #[test]
    fn prompt_to_screensaver_increments_visits() {
        let now = instant();
        let mut s = LockState::new(cfg(60, 30, 99), now);
        s.tick(now + Duration::from_secs(60)); // → Prompt
        s.tick(now + Duration::from_secs(90)); // → Screensaver, visits=1
        assert_eq!(s.phase(), LockPhase::Screensaver);
        assert_eq!(s.screensaver_visits(), 1);
        s.tick(now + Duration::from_secs(150)); // → Prompt again
        s.tick(now + Duration::from_secs(180)); // → Screensaver, visits=2
        assert_eq!(s.screensaver_visits(), 2);
    }

    #[test]
    fn cycles_reach_n_then_dpms() {
        let now = instant();
        let mut s = LockState::new(cfg(60, 30, 3), now);
        let mut t = now;
        for visit in 1..=3 {
            t += Duration::from_secs(60); // T2 elapses → Prompt
            s.tick(t);
            assert_eq!(s.phase(), LockPhase::Prompt, "visit {visit}");
            t += Duration::from_secs(30); // T3 elapses → Screensaver
            s.tick(t);
            assert_eq!(s.screensaver_visits(), visit);
        }
        // Visit count is now 3 (== N). Next tick from Screensaver → Dpms.
        s.tick(t);
        assert_eq!(s.phase(), LockPhase::Dpms);
    }

    #[test]
    fn dpms_holds_indefinitely_without_input() {
        let now = instant();
        let mut s = LockState::new(cfg(0, 0, 0), now);
        s.tick(now);
        assert_eq!(s.phase(), LockPhase::Dpms);
        s.tick(now + Duration::from_secs(1_000_000));
        assert_eq!(s.phase(), LockPhase::Dpms);
    }

    #[test]
    fn input_in_screensaver_jumps_to_prompt() {
        let now = instant();
        let mut s = LockState::new(cfg(300, 120, 3), now);
        s.on_input(now + Duration::from_secs(5));
        assert_eq!(s.phase(), LockPhase::Prompt);
    }

    #[test]
    fn input_in_prompt_resets_t3_timer() {
        let now = instant();
        let mut s = LockState::new(cfg(300, 120, 3), now);
        s.tick(now + Duration::from_secs(300)); // → Prompt at +300
        s.tick(now + Duration::from_secs(419)); // still Prompt
        s.on_input(now + Duration::from_secs(419)); // resets T3 from +419
        s.tick(now + Duration::from_secs(538)); // 119s after last input
        assert_eq!(s.phase(), LockPhase::Prompt);
        s.tick(now + Duration::from_secs(539)); // 120s after last input
        assert_eq!(s.phase(), LockPhase::Screensaver);
    }

    #[test]
    fn input_in_dpms_jumps_to_prompt_and_resets_visits() {
        let now = instant();
        let mut s = LockState::new(cfg(0, 0, 0), now);
        s.tick(now); // → Dpms (N=0)
        assert_eq!(s.phase(), LockPhase::Dpms);
        s.on_input(now + Duration::from_secs(60));
        assert_eq!(s.phase(), LockPhase::Prompt);
        assert_eq!(s.screensaver_visits(), 0);
    }

    #[test]
    fn repeated_input_in_prompt_keeps_extending() {
        let now = instant();
        let mut s = LockState::new(cfg(0, 120, 3), now);
        s.tick(now); // → Prompt
        for i in 1..=5 {
            s.on_input(now + Duration::from_secs(i * 60));
            s.tick(now + Duration::from_secs(i * 60));
            assert_eq!(s.phase(), LockPhase::Prompt, "iter {i}");
        }
    }

    #[test]
    fn t2_zero_means_immediate_prompt() {
        let now = instant();
        let mut s = LockState::new(cfg(0, 120, 3), now);
        s.tick(now);
        assert_eq!(s.phase(), LockPhase::Prompt);
    }

    #[test]
    fn t3_zero_means_immediate_screensaver_return() {
        let now = instant();
        let mut s = LockState::new(cfg(300, 0, 99), now);
        // First tick at +300 fires Screensaver→Prompt; T3=0 means
        // that tick's chained loop immediately also fires
        // Prompt→Screensaver. Final phase: Screensaver, visits=1.
        s.tick(now + Duration::from_secs(300));
        assert_eq!(s.phase(), LockPhase::Screensaver);
        assert_eq!(s.screensaver_visits(), 1);
    }

    #[test]
    fn n_zero_means_immediate_dpms() {
        let now = instant();
        let mut s = LockState::new(cfg(0, 0, 0), now);
        s.tick(now);
        assert_eq!(s.phase(), LockPhase::Dpms);
    }

    #[test]
    fn clock_backwards_in_tick_does_not_panic() {
        let now = instant();
        let mut s = LockState::new(cfg(300, 120, 3), now + Duration::from_secs(1000));
        // Tick at an earlier instant. saturating_duration_since must yield 0.
        s.tick(now);
        assert_eq!(s.phase(), LockPhase::Screensaver);
    }

    #[test]
    fn ticking_through_n_cycles_lands_in_dpms() {
        let now = instant();
        let mut s = LockState::new(cfg(60, 30, 2), now);
        let mut t = now;
        for _ in 0..2 {
            t += Duration::from_secs(60);
            s.tick(t);
            t += Duration::from_secs(30);
            s.tick(t);
        }
        s.tick(t);
        assert_eq!(s.phase(), LockPhase::Dpms);
        assert_eq!(s.screensaver_visits(), 2);
    }

    #[test]
    fn idempotent_tick_at_same_instant() {
        let now = instant();
        let mut s = LockState::new(cfg(60, 30, 5), now);
        let tick_at = now + Duration::from_secs(100);
        s.tick(tick_at);
        let phase_after_first = s.phase();
        let visits_after_first = s.screensaver_visits();
        s.tick(tick_at);
        s.tick(tick_at);
        assert_eq!(s.phase(), phase_after_first);
        assert_eq!(s.screensaver_visits(), visits_after_first);
    }

    #[test]
    fn full_cycle_screensaver_prompt_screensaver_prompt() {
        let now = instant();
        let mut s = LockState::new(cfg(60, 30, 3), now);
        s.tick(now + Duration::from_secs(60));
        assert_eq!(s.phase(), LockPhase::Prompt);
        s.tick(now + Duration::from_secs(90));
        assert_eq!(s.phase(), LockPhase::Screensaver);
        assert_eq!(s.screensaver_visits(), 1);
        s.tick(now + Duration::from_secs(150));
        assert_eq!(s.phase(), LockPhase::Prompt);
        s.tick(now + Duration::from_secs(180));
        assert_eq!(s.phase(), LockPhase::Screensaver);
        assert_eq!(s.screensaver_visits(), 2);
    }

    #[test]
    fn input_then_idle_completes_a_visit() {
        let now = instant();
        let mut s = LockState::new(cfg(300, 120, 3), now);
        s.on_input(now); // Screensaver → Prompt
        s.tick(now + Duration::from_secs(120));
        assert_eq!(s.phase(), LockPhase::Screensaver);
        assert_eq!(s.screensaver_visits(), 1);
    }

    #[test]
    fn time_until_transition_screensaver_partway_through_t2() {
        let now = instant();
        let s = LockState::new(cfg(300, 120, 3), now);
        let remaining = s
            .time_until_next_transition(now + Duration::from_secs(100))
            .unwrap();
        assert_eq!(remaining, Duration::from_secs(200));
    }

    #[test]
    fn time_until_transition_screensaver_at_t2_is_zero() {
        let now = instant();
        let s = LockState::new(cfg(300, 120, 3), now);
        let remaining = s
            .time_until_next_transition(now + Duration::from_secs(300))
            .unwrap();
        assert_eq!(remaining, Duration::ZERO);
    }

    #[test]
    fn time_until_transition_screensaver_past_t2_is_zero() {
        let now = instant();
        let s = LockState::new(cfg(300, 120, 3), now);
        let remaining = s
            .time_until_next_transition(now + Duration::from_secs(500))
            .unwrap();
        assert_eq!(remaining, Duration::ZERO);
    }

    // (No "screensaver_at_n_is_zero" test: `tick`'s chained loop
    // immediately advances to Dpms in that case. The
    // `Some(Duration::ZERO)` arm of `time_until_next_transition` is
    // defensive code unreachable through the normal API.)

    #[test]
    fn time_until_transition_prompt_partway_through_t3() {
        let now = instant();
        let mut s = LockState::new(cfg(0, 120, 3), now);
        s.tick(now); // → Prompt at `now`
        let remaining = s
            .time_until_next_transition(now + Duration::from_secs(40))
            .unwrap();
        assert_eq!(remaining, Duration::from_secs(80));
    }

    #[test]
    fn time_until_transition_prompt_extended_by_input() {
        let now = instant();
        let mut s = LockState::new(cfg(0, 120, 3), now);
        s.tick(now); // → Prompt at `now`
        s.on_input(now + Duration::from_secs(60));
        // T3 timer reset; new deadline = 60+120 = 180.
        let remaining = s
            .time_until_next_transition(now + Duration::from_secs(60))
            .unwrap();
        assert_eq!(remaining, Duration::from_secs(120));
    }

    #[test]
    fn time_until_transition_dpms_is_none() {
        let now = instant();
        let mut s = LockState::new(cfg(0, 0, 0), now);
        s.tick(now);
        assert!(s.time_until_next_transition(now).is_none());
    }

    #[test]
    fn three_back_to_back_inputs_in_prompt_each_reset_timer() {
        let now = instant();
        let mut s = LockState::new(cfg(300, 30, 3), now);
        s.tick(now + Duration::from_secs(300)); // → Prompt at +300
        s.on_input(now + Duration::from_secs(310));
        s.on_input(now + Duration::from_secs(320));
        s.on_input(now + Duration::from_secs(329));
        // Last input at +329 → T3 deadline at +359.
        s.tick(now + Duration::from_secs(358));
        assert_eq!(s.phase(), LockPhase::Prompt);
        s.tick(now + Duration::from_secs(359));
        assert_eq!(s.phase(), LockPhase::Screensaver);
    }

    #[test]
    fn dpms_exit_resets_visits_to_zero_not_one() {
        let now = instant();
        let mut s = LockState::new(cfg(60, 30, 1), now);
        let mut t = now;
        t += Duration::from_secs(60);
        s.tick(t); // → Prompt
        t += Duration::from_secs(30);
        s.tick(t); // → Screensaver, visits=1; chained → Dpms (visits >= N=1)
        assert_eq!(s.phase(), LockPhase::Dpms);
        s.on_input(t);
        assert_eq!(s.phase(), LockPhase::Prompt);
        assert_eq!(s.screensaver_visits(), 0);
    }

    #[test]
    fn input_in_screensaver_does_not_reset_visits() {
        let now = instant();
        let mut s = LockState::new(cfg(60, 30, 3), now);
        let mut t = now;
        t += Duration::from_secs(60);
        s.tick(t); // → Prompt
        t += Duration::from_secs(30);
        s.tick(t); // → Screensaver, visits=1
        s.on_input(t); // Screensaver → Prompt; visits should remain 1
        assert_eq!(s.phase(), LockPhase::Prompt);
        assert_eq!(s.screensaver_visits(), 1);
    }
}
