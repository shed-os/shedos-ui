//! Pure-data definition of an audio analysis frame, lifted into core
//! so styles can consume it without pulling in pipewire (the actual
//! capture lives in `shedos-screensaver-audio`).

pub const NUM_BANDS: usize = 32;

#[derive(Debug, Clone)]
pub struct AudioFrame {
    /// 32 band magnitudes, normalized 0..1 (log-spaced from 60 Hz to Nyquist).
    pub bands: [f32; NUM_BANDS],
    /// Peak amplitude across all bands, 0..1.
    pub peak: f32,
    /// True if the bass band crossed the rolling-window energy threshold this tick.
    pub beat: bool,
    /// Sample rate of the captured stream.
    pub sample_rate: u32,
}

impl AudioFrame {
    pub fn silent() -> Self {
        Self {
            bands: [0.0; NUM_BANDS],
            peak: 0.0,
            beat: false,
            sample_rate: 48_000,
        }
    }

    /// Convenience: average magnitude across the bottom `n` bands
    /// (treat as "bass energy").
    pub fn bass(&self, n: usize) -> f32 {
        let n = n.clamp(1, NUM_BANDS);
        self.bands[..n].iter().sum::<f32>() / n as f32
    }

    /// Convenience: average magnitude across bands `[lo..hi)`.
    pub fn band_range(&self, lo: usize, hi: usize) -> f32 {
        let lo = lo.min(NUM_BANDS);
        let hi = hi.min(NUM_BANDS).max(lo + 1);
        self.bands[lo..hi].iter().sum::<f32>() / (hi - lo) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silent_is_all_zero() {
        let f = AudioFrame::silent();
        assert_eq!(f.peak, 0.0);
        assert!(!f.beat);
        assert!(f.bands.iter().all(|&b| b == 0.0));
    }

    #[test]
    fn bass_averages_low_bands() {
        let mut f = AudioFrame::silent();
        f.bands[0] = 0.4;
        f.bands[1] = 0.6;
        f.bands[2] = 1.0;
        assert!((f.bass(2) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn band_range_clamps_args() {
        let mut f = AudioFrame::silent();
        f.bands[5] = 0.5;
        f.bands[6] = 0.5;
        // 5..7 → avg 0.5
        assert!((f.band_range(5, 7) - 0.5).abs() < 1e-6);
        // out-of-range hi clamps to NUM_BANDS
        assert!((f.band_range(0, 1000) - (1.0 / NUM_BANDS as f32)).abs() < 1e-6);
    }
}
