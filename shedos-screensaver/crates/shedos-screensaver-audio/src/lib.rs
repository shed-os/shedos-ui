//! cpal-backed audio source for shedos-screensaver.
//!
//! `Source::Mic` opens the system default input device. `Source::Desktop`
//! picks the first input device whose name contains `monitor`
//! (pulse/pipewire loopback convention), falling back to the default
//! input if none is found.
//!
//! Samples are mixed to mono f32, ring-buffered, and analyzed by a
//! windowed FFT (32 log-spaced bands + peak + beat). On stream-open
//! failure, a warning is logged and [`AudioCapture::latest`] returns
//! [`AudioFrame::silent`].

pub mod fft;

pub use shedos_screensaver_core::{AudioFrame, NUM_BANDS};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

const RING_CAPACITY_FRAMES: usize = 8192;
const FFT_WINDOW: usize = 2048;

/// Where to capture from. The exact backing device is chosen by cpal
/// on Linux (ALSA / pipewire-via-pulse / pipewire-via-jack).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Look for a `*.monitor` input device (system audio loopback).
    Desktop,
    /// Default input device (microphone, line-in, etc.).
    Mic,
}

/// Live audio source. Spawns a background capture stream on `start`
/// and tears it down on Drop.
pub struct AudioCapture {
    latest: Arc<Mutex<AudioFrame>>,
    available: Arc<AtomicBool>,
    /// Keep the cpal Stream alive (it's !Send + !Sync but Drop here
    /// is fine; the stream owns the audio thread).
    _stream: Option<Stream>,
}

// Stream is !Send (cpal host internals) but never moves across
// threads; it stays in the struct. The Mutex<AudioFrame> is the
// cross-thread channel.
unsafe impl Send for AudioCapture {}
unsafe impl Sync for AudioCapture {}

impl AudioCapture {
    pub fn start(source: Source) -> Self {
        let latest = Arc::new(Mutex::new(AudioFrame::silent()));
        let available = Arc::new(AtomicBool::new(false));

        let stream = match Self::open(source, &latest, &available) {
            Ok(s) => Some(s),
            Err(e) => {
                eprintln!(
                    "shedos-screensaver-audio: could not open input stream ({e}); \
                     audio reactivity disabled."
                );
                None
            }
        };

        // Give cpal a moment to establish the stream so available()
        // reflects reality on the first call.
        std::thread::sleep(Duration::from_millis(50));

        Self { latest, available, _stream: stream }
    }

    pub fn available(&self) -> bool {
        self.available.load(Ordering::Acquire)
    }

    pub fn latest(&self) -> AudioFrame {
        if !self.available() {
            return AudioFrame::silent();
        }
        self.latest
            .lock()
            .map(|g| g.clone())
            .unwrap_or_else(|_| AudioFrame::silent())
    }

    fn open(
        source: Source,
        latest: &Arc<Mutex<AudioFrame>>,
        available: &Arc<AtomicBool>,
    ) -> Result<Stream, String> {
        let host = cpal::default_host();
        let device = match source {
            Source::Mic => host
                .default_input_device()
                .ok_or_else(|| "no default input device".to_string())?,
            Source::Desktop => Self::find_monitor_device(&host)
                .or_else(|| host.default_input_device())
                .ok_or_else(|| {
                    "no monitor input device found and no default input device".to_string()
                })?,
        };
        let supported = device
            .default_input_config()
            .map_err(|e| format!("default_input_config: {e}"))?;
        let sample_rate = supported.sample_rate().0;
        let channels = supported.channels();
        let config: StreamConfig = supported.clone().into();

        let ring = Arc::new(Mutex::new(RingF32::new(RING_CAPACITY_FRAMES)));
        let analyzer = Arc::new(Mutex::new(fft::Analyzer::new(FFT_WINDOW, sample_rate)));

        let ring_for_cb = Arc::clone(&ring);
        let analyzer_for_cb = Arc::clone(&analyzer);
        let latest_for_cb = Arc::clone(latest);
        let available_for_cb = Arc::clone(available);

        let err_cb = |e| eprintln!("shedos-screensaver-audio: stream error: {e}");

        let stream = match supported.sample_format() {
            SampleFormat::F32 => device.build_input_stream::<f32, _, _>(
                &config,
                move |data, _| {
                    handle_chunk(
                        data,
                        channels,
                        &ring_for_cb,
                        &analyzer_for_cb,
                        &latest_for_cb,
                        &available_for_cb,
                    );
                },
                err_cb,
                None,
            ),
            SampleFormat::I16 => device.build_input_stream::<i16, _, _>(
                &config,
                move |data, _| {
                    let f: Vec<f32> = data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
                    handle_chunk(
                        &f,
                        channels,
                        &ring_for_cb,
                        &analyzer_for_cb,
                        &latest_for_cb,
                        &available_for_cb,
                    );
                },
                err_cb,
                None,
            ),
            SampleFormat::U16 => device.build_input_stream::<u16, _, _>(
                &config,
                move |data, _| {
                    let f: Vec<f32> = data
                        .iter()
                        .map(|&s| (s as f32 - 32_768.0) / 32_768.0)
                        .collect();
                    handle_chunk(
                        &f,
                        channels,
                        &ring_for_cb,
                        &analyzer_for_cb,
                        &latest_for_cb,
                        &available_for_cb,
                    );
                },
                err_cb,
                None,
            ),
            other => return Err(format!("unsupported sample format: {other:?}")),
        }
        .map_err(|e| format!("build_input_stream: {e}"))?;

        stream.play().map_err(|e| format!("stream.play: {e}"))?;
        Ok(stream)
    }

    fn find_monitor_device(host: &cpal::Host) -> Option<cpal::Device> {
        let inputs = host.input_devices().ok()?;
        for d in inputs {
            if let Ok(name) = d.name() {
                if name.contains("monitor") || name.contains(".monitor") {
                    return Some(d);
                }
            }
        }
        None
    }
}

fn handle_chunk(
    data: &[f32],
    channels: u16,
    ring: &Arc<Mutex<RingF32>>,
    analyzer: &Arc<Mutex<fft::Analyzer>>,
    latest: &Arc<Mutex<AudioFrame>>,
    available: &Arc<AtomicBool>,
) {
    available.store(true, Ordering::Release);
    let mono = if channels <= 1 {
        data.to_vec()
    } else {
        data.chunks(channels as usize)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect()
    };
    if let Ok(mut r) = ring.lock() {
        r.push(&mono);
        if r.len() >= FFT_WINDOW {
            let window = r.snapshot_tail(FFT_WINDOW);
            drop(r);
            if let Ok(mut a) = analyzer.lock() {
                let frame = a.analyze(&window);
                if let Ok(mut g) = latest.lock() {
                    *g = frame;
                }
            }
        }
    }
}

/// Single-producer / single-consumer ring of f32 samples; lock-based
/// since the analysis tick runs at ~60 Hz, well below contention pain.
pub(crate) struct RingF32 {
    buf: Vec<f32>,
    cap: usize,
}

impl RingF32 {
    pub fn new(cap: usize) -> Self {
        Self { buf: Vec::with_capacity(cap), cap }
    }

    pub fn push(&mut self, samples: &[f32]) {
        if samples.len() >= self.cap {
            self.buf.clear();
            let from = samples.len() - self.cap;
            self.buf.extend_from_slice(&samples[from..]);
            return;
        }
        let needed = self.buf.len() + samples.len();
        if needed > self.cap {
            let drop = needed - self.cap;
            self.buf.drain(..drop);
        }
        self.buf.extend_from_slice(samples);
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn snapshot_tail(&self, n: usize) -> Vec<f32> {
        let n = n.min(self.buf.len());
        let from = self.buf.len() - n;
        self.buf[from..].to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_keeps_tail_when_overflowed() {
        let mut r = RingF32::new(4);
        r.push(&[1.0, 2.0, 3.0]);
        r.push(&[4.0, 5.0]);
        let snap = r.snapshot_tail(4);
        assert_eq!(snap, vec![2.0, 3.0, 4.0, 5.0]);
    }

    #[test]
    fn ring_handles_oversized_single_push() {
        let mut r = RingF32::new(3);
        r.push(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        let snap = r.snapshot_tail(3);
        assert_eq!(snap, vec![3.0, 4.0, 5.0]);
    }

    #[test]
    fn audio_frame_silent_is_zero() {
        let f = AudioFrame::silent();
        assert!(f.bands.iter().all(|&b| b == 0.0));
        assert_eq!(f.peak, 0.0);
        assert!(!f.beat);
    }
}
