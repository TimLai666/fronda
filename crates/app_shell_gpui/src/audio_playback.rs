//! Timeline audio playback (decision D2, phase 1).
//!
//! Architecture: a cpal output device behind the [`OutputBackend`] trait, a
//! shared ring buffer between the device callback (consumer) and a background
//! feeder thread (producer) that premixes the timeline via the existing
//! `render_core::audio_plan` path at the DEVICE sample rate, and an audio
//! clock counting samples actually consumed — the playhead's source of truth
//! during playback. No output device → silent degradation (one log line), the
//! visual transport keeps dead-reckoning as before.
//!
//! Everything except the concrete cpal backend is pure and unit-tested with a
//! mock backend; real-device output is a manual verification follow-up.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Interleaved samples the ring holds (~170 ms of 48 kHz stereo).
const RING_CAPACITY: usize = 32 * 1024;
/// Max interleaved samples per feeder push.
const CHUNK_SAMPLES: usize = 4096;
/// Feeder loop cadence (also its command-response latency).
const FEEDER_TICK: Duration = Duration::from_millis(2);

/// Device output format negotiated at open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputSpec {
    pub sample_rate: u32,
    pub channels: u16,
}

/// The whole-timeline premix the feeder serves: called once on the feeder
/// thread with the device rate/channels (heavy — decodes + mixes).
pub type PremixFn = Box<dyn FnOnce(u32, u16) -> Arc<Vec<f32>> + Send + 'static>;

// ---------------------------------------------------------------------------
// Ring buffer

/// Bounded FIFO of interleaved f32 samples shared between the feeder
/// (producer) and the device callback (consumer). An epoch stamp fences out a
/// superseded feeder: a producer holding a stale epoch can no longer push, so
/// re-arming playback never interleaves old and new audio.
pub struct AudioRing {
    inner: Mutex<VecDeque<f32>>,
    capacity: usize,
    epoch: AtomicU64,
}

impl AudioRing {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
            epoch: AtomicU64::new(0),
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.inner.lock().map(|b| b.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn free(&self) -> usize {
        self.capacity.saturating_sub(self.len())
    }

    pub fn current_epoch(&self) -> u64 {
        self.epoch.load(Ordering::Acquire)
    }

    /// Invalidate every producer holding the previous epoch and drop any
    /// buffered samples. Returns the new epoch.
    pub fn bump_epoch(&self) -> u64 {
        let next = self.epoch.fetch_add(1, Ordering::AcqRel) + 1;
        self.clear();
        next
    }

    /// Push up to the free space; rejects a stale `epoch` entirely.
    /// Returns the number of samples accepted.
    pub fn push(&self, epoch: u64, samples: &[f32]) -> usize {
        let Ok(mut buf) = self.inner.lock() else {
            return 0;
        };
        // Re-check the epoch UNDER the lock. `bump_epoch` clears under the same
        // lock, so serializing the check against `clear` here closes a TOCTOU:
        // a stale producer that passed a pre-lock check could otherwise block
        // on the lock behind `clear`, then acquire it and append stale samples
        // that survive the bump — exactly what the fence is meant to prevent.
        if epoch != self.current_epoch() {
            return 0;
        }
        let n = samples.len().min(self.capacity.saturating_sub(buf.len()));
        buf.extend(samples[..n].iter().copied());
        n
    }

    /// Pop up to `out.len()` samples; the unfilled tail is zeroed (underrun).
    /// Returns the number of REAL samples written.
    pub fn pop_into(&self, out: &mut [f32]) -> usize {
        let popped = match self.inner.lock() {
            Ok(mut buf) => {
                let n = out.len().min(buf.len());
                for slot in out[..n].iter_mut() {
                    *slot = buf.pop_front().unwrap_or(0.0);
                }
                n
            }
            Err(_) => 0,
        };
        for slot in out[popped..].iter_mut() {
            *slot = 0.0;
        }
        popped
    }

    pub fn clear(&self) {
        if let Ok(mut buf) = self.inner.lock() {
            buf.clear();
        }
    }
}

// ---------------------------------------------------------------------------
// Audio clock

/// Playback position = seek base + samples actually consumed by the device.
/// Underruns pad zeros without counting, so the clock (and the playhead it
/// drives) stalls instead of drifting ahead of the audible audio.
pub struct AudioClock {
    consumed: AtomicU64,
    base_us: AtomicI64,
    rate: AtomicU32,
    channels: AtomicU32,
}

impl Default for AudioClock {
    fn default() -> Self {
        Self {
            consumed: AtomicU64::new(0),
            base_us: AtomicI64::new(0),
            rate: AtomicU32::new(48_000),
            channels: AtomicU32::new(2),
        }
    }
}

impl AudioClock {
    pub fn configure(&self, spec: OutputSpec) {
        self.rate.store(spec.sample_rate.max(1), Ordering::Release);
        self.channels
            .store(spec.channels.max(1) as u32, Ordering::Release);
    }

    pub fn channels(&self) -> u32 {
        self.channels.load(Ordering::Acquire).max(1)
    }

    /// Rebase the position to `seconds` and zero the consumed count.
    pub fn reset_to(&self, seconds: f64) {
        self.base_us
            .store((seconds.max(0.0) * 1e6) as i64, Ordering::Release);
        self.consumed.store(0, Ordering::Release);
    }

    /// Record `samples` interleaved samples consumed by the device.
    pub fn add_consumed(&self, samples: u64) {
        self.consumed.fetch_add(samples, Ordering::AcqRel);
    }

    pub fn position_seconds(&self) -> f64 {
        let rate = self.rate.load(Ordering::Acquire).max(1) as f64;
        let ch = self.channels() as f64;
        let consumed = self.consumed.load(Ordering::Acquire) as f64;
        self.base_us.load(Ordering::Acquire) as f64 / 1e6 + consumed / (rate * ch)
    }

    pub fn position_frame(&self, fps: i64) -> i64 {
        (self.position_seconds() * fps.max(1) as f64).floor() as i64
    }
}

// ---------------------------------------------------------------------------
// Output level tap (meter feed)

/// Latest per-channel peak of what the device actually output.
#[derive(Default)]
pub struct OutputTap {
    left_bits: AtomicU32,
    right_bits: AtomicU32,
}

impl OutputTap {
    pub fn publish(&self, left: f32, right: f32) {
        self.left_bits.store(left.to_bits(), Ordering::Release);
        self.right_bits.store(right.to_bits(), Ordering::Release);
    }

    pub fn levels(&self) -> (f32, f32) {
        (
            f32::from_bits(self.left_bits.load(Ordering::Acquire)),
            f32::from_bits(self.right_bits.load(Ordering::Acquire)),
        )
    }
}

// ---------------------------------------------------------------------------
// Output sink — the device callback body

/// What a backend's data callback drains: ring + clock + level tap. Cloneable
/// so the callback owns its handles.
#[derive(Clone)]
pub struct OutputSink {
    ring: Arc<AudioRing>,
    clock: Arc<AudioClock>,
    tap: Arc<OutputTap>,
}

impl OutputSink {
    /// Fill a device buffer: pop real samples (zero-fill the underrun tail),
    /// advance the clock by the real samples only, and publish the chunk's
    /// per-channel peaks for the meter.
    pub fn fill_output(&self, out: &mut [f32]) {
        let real = self.ring.pop_into(out);
        self.clock.add_consumed(real as u64);
        let ch = self.clock.channels() as usize;
        let (mut left, mut right) = (0.0f32, 0.0f32);
        for (i, s) in out[..real].iter().enumerate() {
            let mag = s.abs();
            if ch >= 2 && i % ch == 1 {
                right = right.max(mag);
            } else {
                left = left.max(mag);
            }
        }
        if ch < 2 {
            right = left;
        }
        self.tap.publish(left, right);
    }
}

// ---------------------------------------------------------------------------
// Backend trait

/// Platform output-device adapter (cpal in production, a mock in tests).
pub trait OutputBackend: Send {
    /// Open the default output stream draining `sink`. Called once, lazily.
    fn open(&mut self, sink: OutputSink) -> Result<OutputSpec, String>;
    fn set_playing(&mut self, playing: bool);
}

// ---------------------------------------------------------------------------
// Feeder

enum FeederCmd {
    Seek(f64),
    Stop,
}

struct Feeder {
    tx: mpsc::Sender<FeederCmd>,
}

impl Drop for Feeder {
    fn drop(&mut self) {
        let _ = self.tx.send(FeederCmd::Stop);
    }
}

/// The next `[start, start+len)` interleaved slice a feeder should push, given
/// the premixed data length, its cursor, and the ring's free space. Aligned to
/// whole channel frames; `None` when there is nothing to push.
pub fn next_chunk(
    data_len: usize,
    cursor: usize,
    free: usize,
    channels: usize,
) -> Option<(usize, usize)> {
    if cursor >= data_len {
        return None;
    }
    let ch = channels.max(1);
    let n = free.min(CHUNK_SAMPLES).min(data_len - cursor);
    let n = n - (n % ch);
    if n == 0 {
        None
    } else {
        Some((cursor, n))
    }
}

fn cursor_for_seconds(seconds: f64, spec: OutputSpec) -> usize {
    let frame = (seconds.max(0.0) * spec.sample_rate as f64).floor() as usize;
    frame * spec.channels.max(1) as usize
}

fn spawn_feeder(
    premix: PremixFn,
    spec: OutputSpec,
    start_seconds: f64,
    ring: Arc<AudioRing>,
    ended: Arc<AtomicBool>,
) -> Feeder {
    let (tx, rx) = mpsc::channel::<FeederCmd>();
    let epoch = ring.current_epoch();
    std::thread::Builder::new()
        .name("fronda-audio-feeder".into())
        .spawn(move || {
            let data = premix(spec.sample_rate, spec.channels);
            let ch = spec.channels.max(1) as usize;
            let mut cursor = cursor_for_seconds(start_seconds, spec);
            loop {
                match rx.recv_timeout(FEEDER_TICK) {
                    Ok(FeederCmd::Seek(seconds)) => {
                        ring.clear();
                        cursor = cursor_for_seconds(seconds, spec);
                        ended.store(false, Ordering::Release);
                        continue;
                    }
                    Ok(FeederCmd::Stop) | Err(RecvTimeoutError::Disconnected) => return,
                    Err(RecvTimeoutError::Timeout) => {}
                }
                if let Some((start, n)) = next_chunk(data.len(), cursor, ring.free(), ch) {
                    if ring.push(epoch, &data[start..start + n]) == 0 {
                        return; // superseded by a newer arm
                    }
                    cursor = start + n;
                }
                if cursor >= data.len() {
                    ended.store(true, Ordering::Release);
                }
            }
        })
        .expect("spawn audio feeder");
    Feeder { tx }
}

// ---------------------------------------------------------------------------
// Playback engine (state machine)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayState {
    Idle,
    Playing,
    Paused,
}

/// Owns the backend, ring, clock, and feeder. All methods are cheap except the
/// first `play`, which lazily opens the device (a failed open degrades to
/// silent playback — one log line, everything else keeps working).
pub struct PlaybackEngine {
    backend: Box<dyn OutputBackend>,
    spec: Option<OutputSpec>,
    degraded: bool,
    ring: Arc<AudioRing>,
    clock: Arc<AudioClock>,
    tap: Arc<OutputTap>,
    ended: Arc<AtomicBool>,
    state: PlayState,
    feeder: Option<Feeder>,
}

impl PlaybackEngine {
    pub fn new(backend: Box<dyn OutputBackend>) -> Self {
        Self {
            backend,
            spec: None,
            degraded: false,
            ring: Arc::new(AudioRing::new(RING_CAPACITY)),
            clock: Arc::new(AudioClock::default()),
            tap: Arc::new(OutputTap::default()),
            ended: Arc::new(AtomicBool::new(false)),
            state: PlayState::Idle,
            feeder: None,
        }
    }

    fn ensure_open(&mut self) {
        if self.spec.is_some() || self.degraded {
            return;
        }
        let sink = OutputSink {
            ring: self.ring.clone(),
            clock: self.clock.clone(),
            tap: self.tap.clone(),
        };
        match self.backend.open(sink) {
            Ok(spec) => {
                self.clock.configure(spec);
                self.spec = Some(spec);
            }
            Err(reason) => {
                self.degraded = true;
                eprintln!("Audio output unavailable ({reason}); playback continues silently.");
            }
        }
    }

    /// Arm a fresh premix and start playing from `start_seconds`. Any previous
    /// feeder is fenced out via the ring epoch.
    pub fn play(&mut self, start_seconds: f64, premix: PremixFn) {
        self.ensure_open();
        self.feeder = None; // Drop sends Stop; the epoch bump below fences stragglers.
        self.ring.bump_epoch();
        self.ended.store(false, Ordering::Release);
        self.clock.reset_to(start_seconds);
        if let Some(spec) = self.spec {
            self.feeder = Some(spawn_feeder(
                premix,
                spec,
                start_seconds,
                self.ring.clone(),
                self.ended.clone(),
            ));
            self.backend.set_playing(true);
        }
        self.state = PlayState::Playing;
    }

    /// Resume an armed (paused) feeder at `seconds` without re-premixing.
    /// Returns false when no feeder is armed — the caller should `play`.
    pub fn resume_at(&mut self, seconds: f64) -> bool {
        if self.degraded {
            self.clock.reset_to(seconds);
            self.state = PlayState::Playing;
            return true;
        }
        let Some(feeder) = self.feeder.as_ref() else {
            return false;
        };
        if feeder.tx.send(FeederCmd::Seek(seconds)).is_err() {
            self.feeder = None;
            return false;
        }
        self.clock.reset_to(seconds);
        self.backend.set_playing(true);
        self.state = PlayState::Playing;
        true
    }

    pub fn pause(&mut self) {
        if self.state == PlayState::Playing {
            self.backend.set_playing(false);
            self.state = PlayState::Paused;
        }
    }

    /// Jump to `seconds`: the feeder clears the ring and refills from there.
    pub fn seek(&mut self, seconds: f64) {
        if let Some(feeder) = self.feeder.as_ref() {
            if feeder.tx.send(FeederCmd::Seek(seconds)).is_err() {
                self.feeder = None;
            }
        }
        self.clock.reset_to(seconds);
    }

    pub fn stop(&mut self) {
        self.backend.set_playing(false);
        self.feeder = None;
        self.ring.bump_epoch();
        self.ended.store(false, Ordering::Release);
        self.clock.reset_to(0.0);
        self.state = PlayState::Idle;
    }

    pub fn state(&self) -> PlayState {
        self.state
    }

    pub fn degraded(&self) -> bool {
        self.degraded
    }

    /// Real audio is being produced (playing, device open, not degraded).
    pub fn is_live_playing(&self) -> bool {
        self.state == PlayState::Playing && !self.degraded && self.spec.is_some()
    }

    pub fn position_seconds(&self) -> f64 {
        self.clock.position_seconds()
    }

    pub fn position_frame(&self, fps: i64) -> i64 {
        self.clock.position_frame(fps)
    }

    /// The premix is fully fed AND drained — playback ran off the end.
    pub fn ended(&self) -> bool {
        self.ended.load(Ordering::Acquire) && self.ring.is_empty()
    }

    /// Latest output-chunk peaks for the meter; `None` unless live.
    pub fn live_levels(&self) -> Option<(f32, f32)> {
        self.is_live_playing().then(|| self.tap.levels())
    }
}

// ---------------------------------------------------------------------------
// cpal backend + global engine (desktop app only)

#[cfg(feature = "desktop-app")]
mod cpal_backend {
    use super::{OutputBackend, OutputSink, OutputSpec};
    use std::sync::mpsc;

    enum Cmd {
        Play,
        Pause,
        Shutdown,
    }

    /// cpal stream behind a dedicated thread (the stream is !Send) driven by a
    /// command channel, so the engine stays Send.
    #[derive(Default)]
    pub struct CpalBackend {
        tx: Option<mpsc::Sender<Cmd>>,
    }

    impl Drop for CpalBackend {
        fn drop(&mut self) {
            if let Some(tx) = self.tx.take() {
                let _ = tx.send(Cmd::Shutdown);
            }
        }
    }

    impl OutputBackend for CpalBackend {
        fn open(&mut self, sink: OutputSink) -> Result<OutputSpec, String> {
            let (tx, rx) = mpsc::channel::<Cmd>();
            let (spec_tx, spec_rx) = mpsc::channel::<Result<OutputSpec, String>>();
            std::thread::Builder::new()
                .name("fronda-audio-output".into())
                .spawn(move || stream_thread(sink, rx, spec_tx))
                .map_err(|e| format!("spawn audio thread: {e}"))?;
            let spec = spec_rx
                .recv()
                .map_err(|_| "audio thread exited before reporting".to_string())??;
            self.tx = Some(tx);
            Ok(spec)
        }

        fn set_playing(&mut self, playing: bool) {
            if let Some(tx) = self.tx.as_ref() {
                let _ = tx.send(if playing { Cmd::Play } else { Cmd::Pause });
            }
        }
    }

    fn stream_thread(
        sink: OutputSink,
        rx: mpsc::Receiver<Cmd>,
        spec_tx: mpsc::Sender<Result<OutputSpec, String>>,
    ) {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        let build = || -> Result<(cpal::Stream, OutputSpec), String> {
            let device = cpal::default_host()
                .default_output_device()
                .ok_or("no default output device")?;
            let supported = device
                .default_output_config()
                .map_err(|e| format!("default output config: {e}"))?;
            let spec = OutputSpec {
                sample_rate: supported.sample_rate(),
                channels: supported.channels(),
            };
            let config: cpal::StreamConfig = supported.config();
            let err_fn = |e| eprintln!("Audio output stream error: {e}");
            let stream = match supported.sample_format() {
                cpal::SampleFormat::F32 => device
                    .build_output_stream(
                        config,
                        move |out: &mut [f32], _: &cpal::OutputCallbackInfo| {
                            sink.fill_output(out);
                        },
                        err_fn,
                        None,
                    )
                    .map_err(|e| format!("build output stream: {e}"))?,
                cpal::SampleFormat::I16 => build_converting::<i16>(&device, config, sink)?,
                cpal::SampleFormat::U16 => build_converting::<u16>(&device, config, sink)?,
                other => return Err(format!("unsupported output sample format {other:?}")),
            };
            // Some hosts start streams running; hold until the first Play.
            let _ = stream.pause();
            Ok((stream, spec))
        };

        let stream = match build() {
            Ok((stream, spec)) => {
                let _ = spec_tx.send(Ok(spec));
                stream
            }
            Err(reason) => {
                let _ = spec_tx.send(Err(reason));
                return;
            }
        };
        loop {
            match rx.recv() {
                Ok(Cmd::Play) => {
                    if let Err(e) = stream.play() {
                        eprintln!("Audio output play failed: {e}");
                    }
                }
                Ok(Cmd::Pause) => {
                    let _ = stream.pause();
                }
                Ok(Cmd::Shutdown) | Err(_) => return,
            }
        }
    }

    fn build_converting<T>(
        device: &cpal::Device,
        config: cpal::StreamConfig,
        sink: OutputSink,
    ) -> Result<cpal::Stream, String>
    where
        T: cpal::SizedSample + cpal::FromSample<f32>,
    {
        use cpal::traits::DeviceTrait;
        let mut scratch: Vec<f32> = Vec::new();
        device
            .build_output_stream(
                config,
                move |out: &mut [T], _: &cpal::OutputCallbackInfo| {
                    scratch.resize(out.len(), 0.0);
                    sink.fill_output(&mut scratch);
                    for (dst, src) in out.iter_mut().zip(scratch.iter()) {
                        *dst = T::from_sample(*src);
                    }
                },
                |e| eprintln!("Audio output stream error: {e}"),
                None,
            )
            .map_err(|e| format!("build output stream: {e}"))
    }
}

/// The app-wide playback engine over the default cpal output device.
#[cfg(feature = "desktop-app")]
pub fn engine() -> &'static Mutex<PlaybackEngine> {
    static ENGINE: std::sync::OnceLock<Mutex<PlaybackEngine>> = std::sync::OnceLock::new();
    ENGINE.get_or_init(|| {
        Mutex::new(PlaybackEngine::new(Box::new(
            cpal_backend::CpalBackend::default(),
        )))
    })
}

/// Peaks of the audio actually playing right now; `None` when not live (the
/// meter then falls back to the playhead-envelope mode).
#[cfg(feature = "desktop-app")]
pub fn live_output_levels() -> Option<(f32, f32)> {
    engine().lock().ok()?.live_levels()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::time::Instant;

    const SPEC: OutputSpec = OutputSpec {
        sample_rate: 100,
        channels: 2,
    };

    struct MockBackend {
        sink: Arc<Mutex<Option<OutputSink>>>,
        playing: Arc<AtomicBool>,
        fail_open: bool,
    }

    impl MockBackend {
        fn new() -> (Self, Arc<Mutex<Option<OutputSink>>>, Arc<AtomicBool>) {
            let sink = Arc::new(Mutex::new(None));
            let playing = Arc::new(AtomicBool::new(false));
            (
                Self {
                    sink: sink.clone(),
                    playing: playing.clone(),
                    fail_open: false,
                },
                sink,
                playing,
            )
        }
    }

    impl OutputBackend for MockBackend {
        fn open(&mut self, sink: OutputSink) -> Result<OutputSpec, String> {
            if self.fail_open {
                return Err("no device".into());
            }
            *self.sink.lock().unwrap() = Some(sink);
            Ok(SPEC)
        }

        fn set_playing(&mut self, playing: bool) {
            self.playing.store(playing, Ordering::SeqCst);
        }
    }

    /// data[i] = i, so any drained sample identifies its premix position.
    fn ramp(len: usize) -> Arc<Vec<f32>> {
        Arc::new((0..len).map(|i| i as f32).collect())
    }

    fn ramp_premix(len: usize) -> PremixFn {
        Box::new(move |_, _| ramp(len))
    }

    fn wait_until(what: &str, mut cond: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !cond() {
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    // ---- ring -------------------------------------------------------------

    #[test]
    fn ring_push_caps_at_capacity_and_pop_returns_real_count() {
        let ring = AudioRing::new(4);
        let e = ring.current_epoch();
        assert_eq!(ring.push(e, &[1.0, 2.0, 3.0]), 3);
        assert_eq!(ring.push(e, &[4.0, 5.0]), 1, "only one slot free");
        assert_eq!(ring.len(), 4);
        let mut out = [0.0f32; 3];
        assert_eq!(ring.pop_into(&mut out), 3);
        assert_eq!(out, [1.0, 2.0, 3.0]);
        assert_eq!(ring.len(), 1);
    }

    #[test]
    fn ring_pop_underrun_zero_fills_tail() {
        let ring = AudioRing::new(8);
        ring.push(ring.current_epoch(), &[0.5, 0.6]);
        let mut out = [9.0f32; 5];
        assert_eq!(ring.pop_into(&mut out), 2, "only real samples counted");
        assert_eq!(out, [0.5, 0.6, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn ring_stale_epoch_push_is_rejected() {
        let ring = AudioRing::new(8);
        let stale = ring.current_epoch();
        ring.push(stale, &[1.0]);
        ring.bump_epoch();
        assert_eq!(ring.push(stale, &[2.0]), 0, "stale producer fenced out");
        assert_eq!(ring.len(), 0, "bump also cleared buffered samples");
        assert_eq!(ring.push(ring.current_epoch(), &[3.0]), 1);
    }

    // A stale producer racing a bump must never leave its samples behind: the
    // final settled buffer holds only current-epoch (2.0) or nothing, never a
    // stale marker (1.0). Exercises the push/clear lock-ordering many times to
    // hit the race window that a pre-lock epoch check would let through.
    #[test]
    fn ring_stale_producer_never_survives_a_concurrent_bump() {
        use std::sync::Arc;
        // Many stale producers hammer push() against a stream of bumps+clears.
        // After the last bump and a final drain, no stale marker (1.0) may
        // remain — only current-epoch samples (2.0) or silence. Contended so
        // the race window (producer blocked on the lock behind clear) is hit.
        for _ in 0..200 {
            let ring = Arc::new(AudioRing::new(4096));
            let stale = ring.current_epoch();
            let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let producers: Vec<_> = (0..8)
                .map(|_| {
                    let ring = Arc::clone(&ring);
                    let stop = Arc::clone(&stop);
                    std::thread::spawn(move || {
                        while !stop.load(Ordering::Relaxed) {
                            ring.push(stale, &[1.0; 8]);
                        }
                    })
                })
                .collect();
            for _ in 0..50 {
                ring.bump_epoch();
                std::thread::yield_now();
            }
            let last = ring.bump_epoch();
            stop.store(true, Ordering::Relaxed);
            for p in producers {
                p.join().unwrap();
            }
            // Fresh push proves the ring is usable; then drain and check.
            ring.push(last, &[2.0; 8]);
            let mut out = vec![0.0f32; ring.capacity()];
            ring.pop_into(&mut out);
            assert!(
                out.iter().all(|&s| s != 1.0),
                "a stale-epoch sample survived the bump fence"
            );
        }
    }

    // ---- clock ------------------------------------------------------------

    #[test]
    fn clock_position_tracks_consumed_samples() {
        let clock = AudioClock::default();
        clock.configure(SPEC); // 100 Hz stereo → 200 samples/sec
        clock.reset_to(1.0);
        clock.add_consumed(100); // 0.5 s of stereo
        assert!((clock.position_seconds() - 1.5).abs() < 1e-9);
        assert_eq!(clock.position_frame(10), 15);
    }

    #[test]
    fn clock_reset_rebases_and_zeroes_consumed() {
        let clock = AudioClock::default();
        clock.configure(SPEC);
        clock.add_consumed(400);
        clock.reset_to(2.0);
        assert!((clock.position_seconds() - 2.0).abs() < 1e-9);
    }

    // ---- chunking ---------------------------------------------------------

    #[test]
    fn next_chunk_respects_free_space_and_cap() {
        assert_eq!(next_chunk(10_000, 0, 6, 2), Some((0, 6)));
        // Free space beyond CHUNK_SAMPLES clamps to the chunk cap.
        assert_eq!(next_chunk(100_000, 10, usize::MAX, 2), Some((10, 4096)));
        // End of data clamps the length.
        assert_eq!(next_chunk(10, 8, 100, 2), Some((8, 2)));
    }

    #[test]
    fn next_chunk_aligns_to_channel_frames() {
        assert_eq!(next_chunk(100, 0, 5, 2), Some((0, 4)), "odd free drops 1");
        assert_eq!(next_chunk(100, 0, 1, 2), None, "sub-frame room is no room");
    }

    #[test]
    fn next_chunk_none_at_end() {
        assert_eq!(next_chunk(10, 10, 100, 2), None);
        assert_eq!(next_chunk(0, 0, 100, 2), None);
    }

    // ---- sink -------------------------------------------------------------

    #[test]
    fn fill_output_advances_clock_only_by_real_samples() {
        let sink = OutputSink {
            ring: Arc::new(AudioRing::new(16)),
            clock: Arc::new(AudioClock::default()),
            tap: Arc::new(OutputTap::default()),
        };
        sink.clock.configure(SPEC);
        sink.ring.push(sink.ring.current_epoch(), &[0.1, 0.2, 0.3]);
        let mut out = [7.0f32; 6];
        sink.fill_output(&mut out);
        assert_eq!(out, [0.1, 0.2, 0.3, 0.0, 0.0, 0.0]);
        // 3 real samples at 100 Hz stereo = 0.015 s.
        assert!((sink.clock.position_seconds() - 0.015).abs() < 1e-9);
    }

    #[test]
    fn fill_output_publishes_per_channel_peaks() {
        let sink = OutputSink {
            ring: Arc::new(AudioRing::new(16)),
            clock: Arc::new(AudioClock::default()),
            tap: Arc::new(OutputTap::default()),
        };
        sink.clock.configure(SPEC);
        // Interleaved stereo: L 0.2/-0.9, R 0.4/0.1.
        sink.ring
            .push(sink.ring.current_epoch(), &[0.2, 0.4, -0.9, 0.1]);
        let mut out = [0.0f32; 4];
        sink.fill_output(&mut out);
        let (l, r) = sink.tap.levels();
        assert!((l - 0.9).abs() < 1e-6, "left peak |−0.9|, got {l}");
        assert!((r - 0.4).abs() < 1e-6, "right peak 0.4, got {r}");
    }

    // ---- engine -----------------------------------------------------------

    #[test]
    fn play_opens_backend_with_device_spec_and_feeds_ring() {
        let (backend, sink, playing) = MockBackend::new();
        let mut engine = PlaybackEngine::new(Box::new(backend));
        let seen = Arc::new(Mutex::new(None));
        let seen2 = seen.clone();
        engine.play(
            0.0,
            Box::new(move |rate, ch| {
                *seen2.lock().unwrap() = Some((rate, ch));
                ramp(1000)
            }),
        );
        assert_eq!(engine.state(), PlayState::Playing);
        assert!(engine.is_live_playing());
        assert!(playing.load(Ordering::SeqCst), "backend told to play");
        wait_until("ring fill", || !engine.ring.is_empty());
        assert_eq!(
            *seen.lock().unwrap(),
            Some((SPEC.sample_rate, SPEC.channels)),
            "premix runs at the device rate/channels"
        );
        // Drain: playback starts at premix position 0.
        let sink = sink.lock().unwrap().clone().unwrap();
        let mut out = [0.0f32; 4];
        sink.fill_output(&mut out);
        assert_eq!(out, [0.0, 1.0, 2.0, 3.0]);
    }

    #[test]
    fn play_starts_mid_timeline() {
        let (backend, sink, _) = MockBackend::new();
        let mut engine = PlaybackEngine::new(Box::new(backend));
        // 1.0 s at 100 Hz stereo → interleaved cursor 200.
        engine.play(1.0, ramp_premix(1000));
        wait_until("ring fill", || !engine.ring.is_empty());
        let sink = sink.lock().unwrap().clone().unwrap();
        let mut out = [0.0f32; 2];
        sink.fill_output(&mut out);
        assert_eq!(out, [200.0, 201.0]);
        assert!((engine.position_seconds() - 1.01).abs() < 1e-9);
    }

    #[test]
    fn pause_and_resume_at_keeps_the_armed_feeder() {
        let (backend, sink, playing) = MockBackend::new();
        let mut engine = PlaybackEngine::new(Box::new(backend));
        engine.play(0.0, ramp_premix(1000));
        wait_until("ring fill", || !engine.ring.is_empty());
        engine.pause();
        assert_eq!(engine.state(), PlayState::Paused);
        assert!(!playing.load(Ordering::SeqCst));
        assert!(!engine.is_live_playing());

        assert!(engine.resume_at(2.0), "armed feeder resumes");
        assert_eq!(engine.state(), PlayState::Playing);
        assert!(playing.load(Ordering::SeqCst));
        // 2.0 s at 100 Hz stereo → cursor 400; the seek clears stale samples.
        let sink = sink.lock().unwrap().clone().unwrap();
        let mut out = [0.0f32; 2];
        wait_until("post-resume refill", || {
            sink.fill_output(&mut out);
            out[0] >= 400.0
        });
        assert_eq!(out, [400.0, 401.0], "resume position, no stale audio");
    }

    #[test]
    fn seek_while_playing_repositions() {
        let (backend, sink, _) = MockBackend::new();
        let mut engine = PlaybackEngine::new(Box::new(backend));
        engine.play(0.0, ramp_premix(2000));
        wait_until("ring fill", || !engine.ring.is_empty());
        engine.seek(3.0); // cursor 600
        let sink = sink.lock().unwrap().clone().unwrap();
        let mut out = [0.0f32; 2];
        wait_until("post-seek refill", || {
            sink.fill_output(&mut out);
            out[0] >= 600.0
        });
        assert_eq!(out, [600.0, 601.0]);
        // Clock rebased: position restarts from the seek point.
        assert!(engine.position_seconds() >= 3.0);
        assert!(engine.position_seconds() < 3.5);
    }

    #[test]
    fn ended_after_premix_fully_drained() {
        let (backend, sink, _) = MockBackend::new();
        let mut engine = PlaybackEngine::new(Box::new(backend));
        engine.play(0.0, ramp_premix(6)); // 3 stereo frames
        wait_until("feeder end", || engine.ended.load(Ordering::Acquire));
        assert!(!engine.ended(), "ring still holds the tail");
        let sink = sink.lock().unwrap().clone().unwrap();
        let mut out = [0.0f32; 6];
        sink.fill_output(&mut out);
        assert_eq!(out, [0.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
        assert!(engine.ended(), "fed and drained");
    }

    #[test]
    fn stop_returns_to_idle_and_clears() {
        let (backend, _, playing) = MockBackend::new();
        let mut engine = PlaybackEngine::new(Box::new(backend));
        engine.play(0.0, ramp_premix(1000));
        wait_until("ring fill", || !engine.ring.is_empty());
        engine.stop();
        assert_eq!(engine.state(), PlayState::Idle);
        assert!(!playing.load(Ordering::SeqCst));
        assert_eq!(engine.ring.len(), 0);
        assert!(!engine.is_live_playing());
        assert!(engine.live_levels().is_none());
    }

    #[test]
    fn replay_fences_out_the_previous_feeder() {
        let (backend, sink, _) = MockBackend::new();
        let mut engine = PlaybackEngine::new(Box::new(backend));
        engine.play(0.0, ramp_premix(1000));
        wait_until("ring fill", || !engine.ring.is_empty());
        // Re-arm from 4.0 s (cursor 800): only the new feeder may fill.
        engine.play(4.0, ramp_premix(1000));
        wait_until("new feeder fill", || !engine.ring.is_empty());
        let sink = sink.lock().unwrap().clone().unwrap();
        let mut out = [0.0f32; 2];
        sink.fill_output(&mut out);
        assert_eq!(out, [800.0, 801.0], "no samples from the old feeder");
    }

    #[test]
    fn no_device_degrades_to_silent_playback() {
        let (mut backend, _, playing) = MockBackend::new();
        backend.fail_open = true;
        let mut engine = PlaybackEngine::new(Box::new(backend));
        engine.play(1.5, ramp_premix(1000));
        assert_eq!(engine.state(), PlayState::Playing, "transport still plays");
        assert!(engine.degraded());
        assert!(!engine.is_live_playing());
        assert!(engine.live_levels().is_none(), "meter falls back");
        assert!(!playing.load(Ordering::SeqCst), "backend never started");
        // The clock holds the start position (nothing consumes).
        assert!((engine.position_seconds() - 1.5).abs() < 1e-9);
        // Pause/resume keep working without a device.
        engine.pause();
        assert_eq!(engine.state(), PlayState::Paused);
        assert!(engine.resume_at(2.0));
        assert_eq!(engine.state(), PlayState::Playing);
    }
}
