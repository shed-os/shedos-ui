//! Windowed real-FFT analyzer + log-spaced band aggregation +
//! beat-detection. Pure-CPU, no external state outside the
//! `Analyzer` struct, so it's straightforward to unit-test against
//! synthetic sine inputs.

use realfft::num_complex::Complex;
use realfft::{RealFftPlanner, RealToComplex};
use shedos_screensaver_core::{AudioFrame, NUM_BANDS};
use std::sync::Arc;

const MIN_HZ: f32 = 60.0;
/// Bass band is the lowest log-spaced band (~60-120 Hz) — used for
/// beat detection.
const BASS_BAND_INDEX: usize = 0;
/// Rolling-window length (in analysis ticks) for the energy threshold.
const ROLLING_WINDOW: usize = 16;
/// Bass energy must exceed `BEAT_FACTOR * rolling_avg` to register a beat.
const BEAT_FACTOR: f32 = 1.5;

pub struct Analyzer {
    fft: Arc<dyn RealToComplex<f32>>,
    sample_rate: u32,
    window_size: usize,
    /// Hann window coefficients, baked once.
    hann: Vec<f32>,
    /// Reusable windowed-sample scratch (length = window_size).
    input: Vec<f32>,
    /// Reusable FFT output scratch (length = window_size/2 + 1).
    output: Vec<Complex<f32>>,
    /// Log-spaced band edges, computed once.
    band_edges: [f32; NUM_BANDS + 1],
    /// Hann coherent gain (≈ window_size / 2), used to normalize magnitudes.
    coherent_gain: f32,
    /// FFT bin width in Hz, used to map band edges → bin indices.
    bin_hz: f32,
    /// Rolling buffer of recent bass-band energies for beat detection.
    bass_history: Vec<f32>,
    bass_idx: usize,
    /// How many history slots have been populated (saturates at len()).
    history_count: usize,
}

impl Analyzer {
    pub fn new(window_size: usize, sample_rate: u32) -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(window_size);
        let hann: Vec<f32> = (0..window_size)
            .map(|i| {
                let x = (i as f32) / (window_size as f32 - 1.0);
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * x).cos())
            })
            .collect();
        let input = vec![0.0; window_size];
        let output = fft.make_output_vec();
        let nyquist = sample_rate as f32 / 2.0;
        let bin_hz = nyquist / (window_size as f32 / 2.0);
        let log_min = MIN_HZ.ln();
        let log_max = nyquist.ln();
        let mut band_edges = [0.0f32; NUM_BANDS + 1];
        for (i, edge) in band_edges.iter_mut().enumerate() {
            *edge = (log_min + (log_max - log_min) * (i as f32 / NUM_BANDS as f32)).exp();
        }
        Self {
            fft,
            sample_rate,
            window_size,
            hann,
            input,
            output,
            band_edges,
            coherent_gain: window_size as f32 * 0.5,
            bin_hz,
            bass_history: vec![0.0; ROLLING_WINDOW],
            bass_idx: 0,
            history_count: 0,
        }
    }

    pub fn analyze(&mut self, samples: &[f32]) -> AudioFrame {
        debug_assert_eq!(samples.len(), self.window_size, "analyze: window size mismatch");
        for i in 0..self.window_size {
            self.input[i] = samples[i] * self.hann[i];
        }
        // Process is infallible for matching sizes.
        let _ = self.fft.process(&mut self.input, &mut self.output);

        // Build NUM_BANDS log-spaced bins from cached edges.
        let mut bands = [0.0f32; NUM_BANDS];
        // Sum-of-magnitudes per band (not average) so localized tones
        // — which only light one bin — aren't washed out by being
        // divided across the full band's bin count. Normalize by the
        // window size's Hann coherent gain (≈ N/2) so a unit-amplitude
        // sine peaks at ≈ 0.5.
        for (band_i, band_pair) in self.band_edges.windows(2).enumerate() {
            let lo_hz = band_pair[0];
            let hi_hz = band_pair[1];
            let lo_bin = ((lo_hz / self.bin_hz).floor() as usize).max(1);
            let hi_bin = ((hi_hz / self.bin_hz).ceil() as usize).min(self.output.len() - 1);
            if hi_bin <= lo_bin {
                continue;
            }
            let mut sum = 0.0;
            for bin in lo_bin..hi_bin {
                let mag = (self.output[bin].re * self.output[bin].re
                    + self.output[bin].im * self.output[bin].im)
                    .sqrt();
                sum += mag;
            }
            bands[band_i] = (sum / self.coherent_gain).clamp(0.0, 1.0);
        }

        let peak = bands.iter().copied().fold(0.0_f32, f32::max);

        // Beat detection on the bass band. Only flag a beat once enough
        // history has accumulated to compute a meaningful average; the
        // cold-start period would otherwise spuriously flag every tick
        // because (history_avg ≈ 0) makes any non-zero `bass_now` cross
        // the BEAT_FACTOR multiplier.
        let bass_now = bands[BASS_BAND_INDEX];
        let beat = if self.history_count < self.bass_history.len() / 2 {
            // Warmup. Don't fire beats yet; just accumulate.
            false
        } else {
            // Average across populated slots only.
            let populated = self.history_count.min(self.bass_history.len());
            let sum: f32 = self.bass_history[..populated].iter().sum();
            let avg = sum / populated as f32;
            bass_now > avg * BEAT_FACTOR && bass_now > 0.05
        };
        self.bass_history[self.bass_idx] = bass_now;
        self.bass_idx = (self.bass_idx + 1) % self.bass_history.len();
        if self.history_count < self.bass_history.len() {
            self.history_count += 1;
        }

        AudioFrame {
            bands,
            peak,
            beat,
            sample_rate: self.sample_rate,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synth_sine(hz: f32, sr: u32, n: usize) -> Vec<f32> {
        let two_pi = 2.0 * std::f32::consts::PI;
        (0..n).map(|i| (two_pi * hz * (i as f32) / (sr as f32)).sin()).collect()
    }

    #[test]
    fn pure_silence_yields_zero_bands() {
        let mut a = Analyzer::new(2048, 48_000);
        let silence = vec![0.0f32; 2048];
        let f = a.analyze(&silence);
        assert!(f.peak < 0.001);
        assert!(!f.beat);
    }

    #[test]
    fn bass_sine_lights_lowest_band() {
        let mut a = Analyzer::new(2048, 48_000);
        let sig = synth_sine(80.0, 48_000, 2048);
        let f = a.analyze(&sig);
        // 80 Hz lands in the lowest log-spaced band.
        assert!(f.bands[0] > 0.05, "expected bass energy in band 0; got {:?}", f.bands[0]);
    }

    #[test]
    fn high_sine_lights_upper_band_not_bass() {
        let mut a = Analyzer::new(2048, 48_000);
        // 16 kHz lands well into the top-quintile of the 32 log-spaced bands
        // (band ≈ 30 at 48 kHz / 32 bands from 60 Hz to 24 kHz).
        let sig = synth_sine(16000.0, 48_000, 2048);
        let f = a.analyze(&sig);
        assert!(f.bands[0] < 0.05, "bass should be cold; got {}", f.bands[0]);
        assert!(
            f.bands[NUM_BANDS - 5..].iter().any(|&v| v > 0.05),
            "expected energy in upper bands; got {:?}", &f.bands[NUM_BANDS - 5..]
        );
    }

    #[test]
    fn beat_does_not_fire_during_warmup_or_on_steady_signal() {
        let mut a = Analyzer::new(2048, 48_000);
        let sig = synth_sine(80.0, 48_000, 2048);
        let mut beats = 0;
        for _ in 0..32 {
            let f = a.analyze(&sig);
            if f.beat {
                beats += 1;
            }
        }
        // Warmup suppresses early beats; once warm, a steady signal
        // never crosses 1.5×avg → no beats at all.
        assert_eq!(beats, 0, "steady signal generated {beats} beats; expected 0");
    }

    #[test]
    fn beat_fires_on_transient_after_warmup() {
        let mut a = Analyzer::new(2048, 48_000);
        let quiet = vec![0.0001f32; 2048];
        // Build up history with near-silence (passes warmup).
        for _ in 0..10 {
            a.analyze(&quiet);
        }
        // A loud bass hit should now register a beat.
        let loud = synth_sine(80.0, 48_000, 2048);
        let f = a.analyze(&loud);
        assert!(f.beat, "loud bass after quiet warmup should trigger beat");
    }
}
