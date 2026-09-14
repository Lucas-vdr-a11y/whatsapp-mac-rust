//! macOS CoreAudio media backend for calls (`--features calls`).
//!
//! [`CoreAudioFactory`] is the production [`CallMediaFactory`] for the desktop
//! app. It captures the default microphone and plays decoded peer audio through
//! the default output device using `cpal` (CoreAudio on macOS), adapting the
//! hardware streams to the engine contract used by
//! [`CallMedia`](crate::calls::CallMedia):
//!
//! - capture: 16 kHz mono `i16`, frames of exactly [`CODEC_FRAME_SAMPLES`]
//!   (960) samples — the engine drops frames of any other length;
//! - playback: the engine pushes 16 kHz mono `i16` frames of the same shape.
//!
//! # Pipeline
//!
//! ```text
//! mic : device(N ch, device rate, f32/i16) -> downmix -> resample -> 960 i16 -> mic channel -> engine
//! play: engine -> speaker channel -> resample -> ring buffer -> cpal callback -> output device
//! ```
//!
//! Two worker threads per call do the conversions off the CoreAudio callback
//! threads: the capture callback only downmixes one buffer and hands it over,
//! the playback callback only drains the ring buffer. Neither callback blocks
//! or resamples.
//!
//! # cpal device configuration
//!
//! Both streams are opened with the device's *default* stream configuration,
//! which CoreAudio reports as the hardware's current format (typically 48 kHz,
//! stereo, `f32`; the format can also be `i16`). cpal on macOS changes the
//! device's nominal sample rate and physical format when a stream is opened at
//! a different rate; opening the mic at 16 kHz would reconfigure the user's
//! hardware for every other app, so the hardware keeps its native rate and
//! resampling happens in software (rubato's FFT resampler) in our workers.
//!
//! # Lifetime and teardown
//!
//! `media()` registers the live call in a map keyed by chat id. The cpal
//! streams live in that entry, which is also owned by the playback worker
//! thread. That thread exits when the engine drops either half of the channels
//! (call ended, hung up, or setup failed): it stops the entry, removes it from
//! the registry, and drops the streams. Dropping a cpal stream stops its
//! callbacks, which in turn closes the capture channel and wakes the capture
//! worker. No stream outlives its call, and a new call for the same chat
//! replaces (and stops) any stale entry.
//!
//! # Permissions
//!
//! macOS gates microphone access behind TCC. When CoreAudio rejects the stream,
//! [`CoreAudioFactory::media`] reports a user-presentable
//! [`CoreError::InvalidInput`] so the manager fails cleanly. The host must ship
//! `NSMicrophoneUsageDescription` in Info.plist (and the
//! `com.apple.security.device.audio-input` entitlement under the hardened
//! runtime) or macOS never shows the prompt. Note that a denied or muted
//! microphone can also present as a stream that runs but delivers silence;
//! cpal cannot distinguish that from a muted device, so the calls UI should
//! surface "no audio from your microphone — check System Settings → Privacy &
//! Security → Microphone" when capture produces only silence.
//!
//! # Video
//!
//! Video capture is not implemented. `media(chat, true)` fails with
//! [`CoreError::InvalidInput`] before any device is opened, so a video call
//! fails cleanly instead of half-connecting.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex, MutexGuard};
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, StreamConfig};
use rubato::{FftFixedInOut, Resampler};
use whatsapp_rust::async_channel;

use crate::calls::{CallMedia, CallMediaFactory};
use crate::error::{CoreError, Result};
use crate::types::Jid;

/// Sample rate the call engine works with (WhatsApp MLOW, 60 ms frames).
const CODEC_SAMPLE_RATE: u32 = 16_000;
/// Samples per engine audio frame: 60 ms at [`CODEC_SAMPLE_RATE`].
const CODEC_FRAME_SAMPLES: usize = 960;
/// Frames buffered in the mic channel before capture drops one.
const MIC_CHANNEL_CAPACITY: usize = 4;
/// Frames buffered in the speaker channel before the engine drops one.
const SPEAKER_CHANNEL_CAPACITY: usize = 8;
/// Device-rate blocks buffered between the capture callback and its worker.
const CAPTURE_CHANNEL_CAPACITY: usize = 8;
/// Playback ring-buffer length, in milliseconds of output-device audio.
const PLAYBACK_RING_MS: usize = 500;
/// Output-device audio buffered before playback starts (jitter headroom).
const PLAYBACK_PREFILL_MS: usize = 50;
/// How long the playback worker waits between polls of the speaker channel.
const PLAYBACK_POLL: Duration = Duration::from_millis(5);
/// Length of each resampler input chunk, in milliseconds of source audio.
const RESAMPLER_CHUNK_MS: u32 = 20;

/// Live calls, keyed by chat id string; values are cheap `Arc` clones.
type CallRegistry = Arc<StdMutex<HashMap<String, Arc<ActiveCall>>>>;

/// macOS CoreAudio capture/playback backend for [`CallMediaFactory`].
///
/// Construct one per [`WaClient`](crate::WaClient) and put it in
/// [`ClientConfig::call_media`](crate::ClientConfig) before connecting.
pub struct CoreAudioFactory {
    calls: CallRegistry,
}

impl CoreAudioFactory {
    /// Create an idle factory. No audio device is touched until
    /// [`CallMediaFactory::media`] is called.
    pub fn new() -> Self {
        Self {
            calls: Arc::new(StdMutex::new(HashMap::new())),
        }
    }
}

impl Default for CoreAudioFactory {
    fn default() -> Self {
        Self::new()
    }
}

/// Everything that must stay alive for one call: the cpal streams (dropping
/// them stops capture/playback) and channel probes used to notice teardown.
struct ActiveCall {
    stop: Arc<AtomicBool>,
    input: cpal::Stream,
    output: cpal::Stream,
    /// Clone of the mic sender; `is_closed()` flips once the engine drops the
    /// matching receiver (call teardown).
    mic_probe: async_channel::Sender<Vec<i16>>,
    /// Clone of the speaker receiver; `is_closed()` flips once the engine
    /// drops the matching sender.
    speaker_probe: async_channel::Receiver<Vec<i16>>,
}

impl CallMediaFactory for CoreAudioFactory {
    fn media(&self, chat_id: &Jid, video: bool) -> Result<CallMedia> {
        if video {
            return Err(CoreError::InvalidInput(
                "video calls are not implemented by the macOS media backend yet; \
                 start an audio call instead"
                    .to_owned(),
            ));
        }

        let chat_key = chat_id.to_string();
        let host = cpal::default_host();

        // Default devices and their native configurations. Requesting 16 kHz
        // here would reconfigure the hardware on macOS, so we resample instead.
        let input_device = host.default_input_device().ok_or_else(|| {
            let message = "no default microphone is available; connect an input device and retry";
            tracing::warn!(chat = %chat_key, "{message}");
            CoreError::InvalidInput(message.to_owned())
        })?;
        let input_config = input_device.default_input_config().map_err(|error| {
            tracing::warn!(chat = %chat_key, %error, "failed to query the default microphone");
            capture_error(&error)
        })?;
        let output_device = host.default_output_device().ok_or_else(|| {
            let message = "no default speaker is available; connect an output device and retry";
            tracing::warn!(chat = %chat_key, "{message}");
            CoreError::InvalidInput(message.to_owned())
        })?;
        let output_config = output_device.default_output_config().map_err(|error| {
            tracing::warn!(chat = %chat_key, %error, "failed to query the default speaker");
            playback_error(&error)
        })?;

        let input_rate = input_config.sample_rate();
        let input_channels = usize::from(input_config.channels());
        let input_format = input_config.sample_format();
        let output_rate = output_config.sample_rate();
        let output_channels = usize::from(output_config.channels());
        let output_format = output_config.sample_format();

        // Resamplers are built up front so a failure surfaces from `media()`
        // instead of silently degrading a call.
        let capture_resampler = StreamResampler::new(input_rate, CODEC_SAMPLE_RATE)?;
        let playback_resampler = StreamResampler::new(CODEC_SAMPLE_RATE, output_rate)?;

        let (mic_tx, mic) = async_channel::bounded::<Vec<i16>>(MIC_CHANNEL_CAPACITY);
        let (speaker, speaker_rx) = async_channel::bounded::<Vec<i16>>(SPEAKER_CHANNEL_CAPACITY);
        let (raw_tx, raw_rx) = async_channel::bounded::<Vec<f32>>(CAPTURE_CHANNEL_CAPACITY);
        let stop = Arc::new(AtomicBool::new(false));

        let input = build_input_stream(
            &input_device,
            &input_config.config(),
            input_channels,
            input_format,
            Arc::clone(&stop),
            raw_tx,
        )
        .map_err(|error| {
            tracing::warn!(chat = %chat_key, %error, "failed to open the microphone for a call");
            capture_error(&error)
        })?;

        let ring_capacity = output_rate as usize * PLAYBACK_RING_MS / 1000;
        let prefill = output_rate as usize * PLAYBACK_PREFILL_MS / 1000;
        let ring = Arc::new(StdMutex::new(PcmRing::with_capacity(ring_capacity)));
        let output = build_output_stream(
            &output_device,
            &output_config.config(),
            output_channels,
            output_format,
            Arc::clone(&ring),
            prefill,
        )
        .map_err(|error| {
            tracing::warn!(chat = %chat_key, %error, "failed to open the speaker for a call");
            playback_error(&error)
        })?;

        let call = Arc::new(ActiveCall {
            stop: Arc::clone(&stop),
            input,
            output,
            mic_probe: mic_tx.clone(),
            speaker_probe: speaker_rx.clone(),
        });

        {
            let mut calls = lock(&self.calls);
            if let Some(previous) = calls.insert(chat_key.clone(), Arc::clone(&call)) {
                // A stale entry for the same chat (double-start or a very
                // recent teardown): stop it; its worker drops the streams.
                previous.stop.store(true, Ordering::SeqCst);
                tracing::warn!(chat = %chat_key, "replacing stale call audio streams");
            }
        }

        // Start the streams only once every fallible step has succeeded; on
        // failure the registry entry is removed and the locals drop, which
        // stops whichever stream did start.
        if let Err(error) = call.input.play() {
            tracing::warn!(chat = %chat_key, %error, "failed to start microphone capture");
            stop_and_remove(&self.calls, &chat_key, &call);
            return Err(capture_error(&error));
        }
        if let Err(error) = call.output.play() {
            tracing::warn!(chat = %chat_key, %error, "failed to start call playback");
            stop_and_remove(&self.calls, &chat_key, &call);
            return Err(playback_error(&error));
        }

        let mic_worker = {
            let stop = Arc::clone(&stop);
            thread::Builder::new()
                .name("rustwa-call-mic".to_owned())
                .spawn(move || run_capture(raw_rx, mic_tx, capture_resampler, stop))
        };
        if let Err(error) = mic_worker {
            tracing::warn!(chat = %chat_key, %error, "failed to spawn the capture worker");
            stop_and_remove(&self.calls, &chat_key, &call);
            return Err(CoreError::Internal(format!(
                "could not spawn the call capture thread: {error}"
            )));
        }

        let playback = PlaybackJob {
            call: Arc::clone(&call),
            speaker_rx,
            ring,
            resampler: playback_resampler,
            registry: Arc::clone(&self.calls),
            chat_key: chat_key.clone(),
        };
        if let Err(error) = thread::Builder::new()
            .name("rustwa-call-spk".to_owned())
            .spawn(move || run_playback(playback))
        {
            tracing::warn!(chat = %chat_key, %error, "failed to spawn the playback worker");
            stop_and_remove(&self.calls, &chat_key, &call);
            return Err(CoreError::Internal(format!(
                "could not spawn the call playback thread: {error}"
            )));
        }

        Ok(CallMedia::audio(mic, speaker))
    }
}

/// Spawn a capture stream for the device's default sample format.
///
/// `f32` and `i16` cover everything CoreAudio reports on macOS; anything else
/// fails with a clear error rather than opening a stream we cannot convert.
fn build_input_stream(
    device: &cpal::Device,
    config: &StreamConfig,
    channels: usize,
    format: SampleFormat,
    stop: Arc<AtomicBool>,
    raw_tx: async_channel::Sender<Vec<f32>>,
) -> std::result::Result<cpal::Stream, cpal::Error> {
    let errors_reported = Arc::new(AtomicBool::new(false));
    match format {
        SampleFormat::F32 => {
            open_input::<f32>(device, config, channels, stop, raw_tx, errors_reported)
        }
        SampleFormat::I16 => {
            open_input::<i16>(device, config, channels, stop, raw_tx, errors_reported)
        }
        other => Err(unsupported_format_error("microphone", other)),
    }
}

/// Spawn a playback stream for the device's default sample format.
fn build_output_stream(
    device: &cpal::Device,
    config: &StreamConfig,
    channels: usize,
    format: SampleFormat,
    ring: Arc<StdMutex<PcmRing>>,
    prefill_samples: usize,
) -> std::result::Result<cpal::Stream, cpal::Error> {
    let errors_reported = Arc::new(AtomicBool::new(false));
    match format {
        SampleFormat::F32 => open_output::<f32>(
            device,
            config,
            channels,
            ring,
            prefill_samples,
            errors_reported,
        ),
        SampleFormat::I16 => open_output::<i16>(
            device,
            config,
            channels,
            ring,
            prefill_samples,
            errors_reported,
        ),
        other => Err(unsupported_format_error("speaker", other)),
    }
}

fn open_input<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    channels: usize,
    stop: Arc<AtomicBool>,
    raw_tx: async_channel::Sender<Vec<f32>>,
    errors_reported: Arc<AtomicBool>,
) -> std::result::Result<cpal::Stream, cpal::Error>
where
    T: SizedSample + Send + 'static,
    f32: FromSample<T>,
{
    device.build_input_stream::<T, _, _>(
        *config,
        move |data: &[T], _info| {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            let mono = downmix_to_mono(data, channels);
            if mono.is_empty() {
                return;
            }
            // Full (`the engine is behind`) and closed (`call ended`) both
            // mean "drop this block"; VoIP is loss tolerant.
            let _ = raw_tx.try_send(mono);
        },
        move |error| log_stream_error(&error, "capture", &errors_reported),
        None,
    )
}

fn open_output<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    channels: usize,
    ring: Arc<StdMutex<PcmRing>>,
    prefill_samples: usize,
    errors_reported: Arc<AtomicBool>,
) -> std::result::Result<cpal::Stream, cpal::Error>
where
    T: SizedSample + Send + 'static,
    T: FromSample<f32>,
{
    // The ring only holds decoded peer audio; until a jitter margin has
    // accumulated, playback stays silent so the first frames do not underrun
    // immediately. Once primed, an empty ring just means silence.
    let mut primed = false;
    device.build_output_stream::<T, _, _>(
        *config,
        move |data: &mut [T], _info| {
            let mut ring = lock(&ring);
            if !primed && ring.len() < prefill_samples {
                data.fill(T::EQUILIBRIUM);
                return;
            }
            primed = true;
            for frame in data.chunks_mut(channels) {
                let sample = ring.pop().unwrap_or(0.0);
                let value = T::from_sample(sample);
                for slot in frame.iter_mut() {
                    *slot = value;
                }
            }
        },
        move |error| log_stream_error(&error, "playback", &errors_reported),
        None,
    )
}

fn unsupported_format_error(direction: &str, format: SampleFormat) -> cpal::Error {
    cpal::Error::with_message(
        cpal::ErrorKind::UnsupportedConfig,
        format!("the default {direction} uses an unsupported sample format ({format})"),
    )
}

/// Capture worker: device-rate mono blocks in, 960-sample engine frames out.
fn run_capture(
    raw_rx: async_channel::Receiver<Vec<f32>>,
    mic_tx: async_channel::Sender<Vec<i16>>,
    mut resampler: StreamResampler,
    stop: Arc<AtomicBool>,
) {
    let mut framer = MicFramer::default();
    let mut dropped: u64 = 0;
    let mut engine_gone = false;
    while !stop.load(Ordering::Relaxed) {
        let Ok(block) = raw_rx.recv_blocking() else {
            // The input stream was dropped; the call is over.
            break;
        };
        resampler.push(&block, |chunk| {
            for &sample in chunk {
                let Some(frame) = framer.push(f32_to_i16(sample)) else {
                    continue;
                };
                match mic_tx.try_send(frame) {
                    Ok(()) => {}
                    Err(async_channel::TrySendError::Full(_)) => dropped += 1,
                    Err(async_channel::TrySendError::Closed(_)) => {
                        engine_gone = true;
                        return;
                    }
                }
            }
        });
        if engine_gone {
            break;
        }
    }
    if dropped > 0 {
        tracing::debug!(
            dropped,
            "microphone frames dropped because the call engine fell behind"
        );
    }
}

/// Playback job owned by the worker thread that also keeps the streams alive.
struct PlaybackJob {
    call: Arc<ActiveCall>,
    speaker_rx: async_channel::Receiver<Vec<i16>>,
    ring: Arc<StdMutex<PcmRing>>,
    resampler: StreamResampler,
    registry: CallRegistry,
    chat_key: String,
}

/// Playback worker: engine frames in, device-rate samples into the ring.
///
/// Owning the call entry here means call teardown (either channel closing)
/// drops the cpal streams as soon as this loop notices, which stops capture
/// as well via the closed capture channel.
fn run_playback(mut job: PlaybackJob) {
    let mut scratch: Vec<f32> = Vec::new();
    loop {
        let engine_open = !job.call.speaker_probe.is_closed() && !job.call.mic_probe.is_closed();
        if job.call.stop.load(Ordering::Relaxed) || !engine_open {
            break;
        }
        match job.speaker_rx.try_recv() {
            Ok(frame) => {
                scratch.clear();
                scratch.extend(frame.iter().map(|&sample| i16_to_f32(sample)));
                let ring = Arc::clone(&job.ring);
                job.resampler.push(&scratch, |chunk| {
                    lock(&ring).push_slice(chunk);
                });
            }
            // No frame yet: check again shortly. The ring absorbs the jitter.
            Err(async_channel::TryRecvError::Empty) => thread::sleep(PLAYBACK_POLL),
            Err(async_channel::TryRecvError::Closed) => break,
        }
    }
    job.call.stop.store(true, Ordering::SeqCst);
    stop_and_remove(&job.registry, &job.chat_key, &job.call);
    // `job` drops here, which drops the cpal streams: capture and playback
    // stop and the capture worker's channel closes.
}

/// Flag a call as stopping and forget it, unless a newer call replaced it.
fn stop_and_remove(
    registry: &StdMutex<HashMap<String, Arc<ActiveCall>>>,
    key: &str,
    call: &Arc<ActiveCall>,
) {
    call.stop.store(true, Ordering::SeqCst);
    let mut calls = lock(registry);
    if calls
        .get(key)
        .is_some_and(|current| Arc::ptr_eq(current, call))
    {
        calls.remove(key);
    }
}

fn capture_error(error: &cpal::Error) -> CoreError {
    if error.kind() == cpal::ErrorKind::PermissionDenied {
        CoreError::InvalidInput(
            "microphone access denied: allow RustWA under System Settings → \
             Privacy & Security → Microphone and try the call again"
                .to_owned(),
        )
    } else {
        CoreError::InvalidInput(format!(
            "failed to start microphone capture: {error}. Check System Settings → \
             Privacy & Security → Microphone"
        ))
    }
}

fn playback_error(error: &cpal::Error) -> CoreError {
    if error.kind() == cpal::ErrorKind::PermissionDenied {
        CoreError::InvalidInput(
            "audio playback was blocked by the system; check the output device and \
             System Settings → Privacy & Security"
                .to_owned(),
        )
    } else {
        CoreError::InvalidInput(format!("failed to start call playback: {error}"))
    }
}

/// Log the first error from a stream loudly, later ones at debug level.
fn log_stream_error(error: &cpal::Error, direction: &str, reported: &AtomicBool) {
    if reported.swap(true, Ordering::Relaxed) {
        tracing::debug!(%error, direction, "call audio stream error");
    } else {
        tracing::warn!(%error, direction, "call audio stream error");
    }
}

fn lock<T>(mutex: &StdMutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Downmix an interleaved buffer to mono `f32`.
fn downmix_to_mono<T>(data: &[T], channels: usize) -> Vec<f32>
where
    T: Sample + Copy,
    f32: FromSample<T>,
{
    let channels = channels.max(1);
    let mut mono = Vec::with_capacity(data.len() / channels);
    for frame in data.chunks_exact(channels) {
        let sum: f32 = frame.iter().map(|sample| f32::from_sample(*sample)).sum();
        mono.push(sum / channels as f32);
    }
    mono
}

/// Convert a `f32` sample to `i16`, clamping out-of-range values.
///
/// Scaling by 32768 rather than 32767 keeps the mapping symmetric and makes
/// [`i16_to_f32`] round-trip exactly across the whole `i16` range.
fn f32_to_i16(sample: f32) -> i16 {
    (sample * 32768.0).clamp(-32768.0, 32767.0) as i16
}

/// Convert an `i16` sample to the `[-1.0, 1.0)` `f32` range.
fn i16_to_f32(sample: i16) -> f32 {
    f32::from(sample) / 32768.0
}

/// Splits a mono `i16` stream into the engine's fixed-size frames.
#[derive(Debug, Default)]
struct MicFramer {
    frame: Vec<i16>,
}

impl MicFramer {
    /// Push one sample; returns a complete frame when one is ready.
    fn push(&mut self, sample: i16) -> Option<Vec<i16>> {
        self.frame.push(sample);
        if self.frame.len() >= CODEC_FRAME_SAMPLES {
            Some(std::mem::take(&mut self.frame))
        } else {
            None
        }
    }
}

/// Bounded mono `f32` ring buffer shared by the playback worker (producer)
/// and the cpal output callback (consumer). Overflow drops the oldest samples,
/// bounding the call's playout latency.
struct PcmRing {
    samples: VecDeque<f32>,
    capacity: usize,
}

impl PcmRing {
    fn with_capacity(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            samples: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    fn len(&self) -> usize {
        self.samples.len()
    }

    fn pop(&mut self) -> Option<f32> {
        self.samples.pop_front()
    }

    fn push_slice(&mut self, samples: &[f32]) {
        if samples.len() >= self.capacity {
            self.samples.clear();
            let tail = &samples[samples.len() - self.capacity..];
            self.samples.extend(tail.iter().copied());
            return;
        }
        let overflow = (self.samples.len() + samples.len()).saturating_sub(self.capacity);
        for _ in 0..overflow {
            self.samples.pop_front();
        }
        self.samples.extend(samples.iter().copied());
    }
}

/// Sample-rate conversion for one direction, using rubato's fixed-ratio FFT
/// resampler. Identical rates are a passthrough and skip the FFT entirely.
struct StreamResampler {
    resampler: Option<FftFixedInOut<f32>>,
    pending: Vec<f32>,
}

impl StreamResampler {
    fn new(from_rate: u32, to_rate: u32) -> Result<Self> {
        if from_rate == to_rate {
            return Ok(Self {
                resampler: None,
                pending: Vec::new(),
            });
        }
        let chunk_in = (from_rate * RESAMPLER_CHUNK_MS / 1000).max(1) as usize;
        let resampler = FftFixedInOut::<f32>::new(
            from_rate as usize,
            to_rate as usize,
            chunk_in,
            1,
        )
        .map_err(|error| {
            CoreError::Internal(format!(
                "could not build the {from_rate} Hz -> {to_rate} Hz audio resampler: {error}"
            ))
        })?;
        Ok(Self {
            resampler: Some(resampler),
            pending: Vec::new(),
        })
    }

    /// Push source-rate samples; `emit` is called with each resampled chunk.
    fn push(&mut self, samples: &[f32], mut emit: impl FnMut(&[f32])) {
        let Some(resampler) = self.resampler.as_mut() else {
            emit(samples);
            return;
        };
        let mut offset = 0;
        while offset < samples.len() {
            let needed = resampler.input_frames_next() - self.pending.len();
            let take = needed.min(samples.len() - offset);
            self.pending
                .extend_from_slice(&samples[offset..offset + take]);
            offset += take;
            if self.pending.len() < resampler.input_frames_next() {
                continue;
            }
            match resampler.process(&[&self.pending], None) {
                Ok(output) => emit(&output[0]),
                Err(error) => {
                    tracing::warn!(%error, "dropping a call audio block after a resampler error");
                    self.pending.clear();
                    return;
                }
            }
            self.pending.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn factory() -> CoreAudioFactory {
        CoreAudioFactory::new()
    }

    #[test]
    fn video_calls_fail_before_any_device_is_opened() {
        let error = match factory().media(&Jid::new("alice@s.whatsapp.net"), true) {
            Ok(_) => panic!("video calls must be rejected"),
            Err(error) => error,
        };
        match error {
            CoreError::InvalidInput(message) => {
                assert!(message.contains("video"), "unexpected message: {message}");
            }
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn permission_denied_maps_to_a_clear_microphone_error() {
        let error = capture_error(&cpal::Error::new(cpal::ErrorKind::PermissionDenied));
        match error {
            CoreError::InvalidInput(message) => {
                assert!(message.contains("microphone access denied"), "{message}");
                assert!(message.contains("System Settings"), "{message}");
            }
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn playback_failures_stay_invalid_input() {
        for kind in [
            cpal::ErrorKind::DeviceNotAvailable,
            cpal::ErrorKind::PermissionDenied,
        ] {
            let error = playback_error(&cpal::Error::new(kind));
            assert!(
                matches!(error, CoreError::InvalidInput(_)),
                "expected InvalidInput for {kind:?}"
            );
        }
    }

    #[test]
    fn resampler_rejects_zero_rate() {
        assert!(StreamResampler::new(0, CODEC_SAMPLE_RATE).is_err());
    }

    #[test]
    fn f32_to_i16_scales_and_clamps() {
        assert_eq!(f32_to_i16(0.0), 0);
        assert_eq!(f32_to_i16(1.0), i16::MAX);
        assert_eq!(f32_to_i16(-1.0), i16::MIN);
        assert_eq!(f32_to_i16(2.0), i16::MAX);
        assert_eq!(f32_to_i16(-2.0), i16::MIN);
        assert_eq!(f32_to_i16(0.5), 16_384);
        assert_eq!(f32_to_i16(-0.5), -16_384);
    }

    #[test]
    fn i16_to_f32_round_trips_all_extremes() {
        for sample in [i16::MIN, -1, 0, 1, i16::MAX] {
            assert_eq!(f32_to_i16(i16_to_f32(sample)), sample);
        }
        assert_eq!(i16_to_f32(i16::MIN), -1.0);
        assert!(i16_to_f32(i16::MAX) < 1.0);
    }

    #[test]
    fn downmix_averages_channels() {
        assert_eq!(downmix_to_mono(&[0.5_f32, 0.0], 2), vec![0.25]);
        assert_eq!(downmix_to_mono(&[1.0_f32, 0.0], 2), vec![0.5]);
        assert_eq!(
            downmix_to_mono(&[0.25_f32, 0.5, -0.25], 1),
            vec![0.25, 0.5, -0.25]
        );
        assert_eq!(downmix_to_mono(&[i16::MAX, i16::MAX], 2), vec![0.999_969_5]);
    }

    #[test]
    fn mic_framer_emits_exact_960_sample_frames() {
        let mut framer = MicFramer::default();
        for _ in 0..(CODEC_FRAME_SAMPLES - 1) {
            assert!(framer.push(1).is_none());
        }
        let first = framer.push(1).expect("960th sample completes a frame");
        assert_eq!(first.len(), CODEC_FRAME_SAMPLES);
        assert!(first.iter().all(|sample| *sample == 1));

        let mut second = None;
        for _ in 0..CODEC_FRAME_SAMPLES {
            second = framer.push(-1);
        }
        let second = second.expect("the next frame completes");
        assert_eq!(second.len(), CODEC_FRAME_SAMPLES);
        assert!(second.iter().all(|sample| *sample == -1));
    }

    #[test]
    fn pcm_ring_drops_oldest_when_full() {
        let mut ring = PcmRing::with_capacity(3);
        ring.push_slice(&[1.0, 2.0]);
        assert_eq!(ring.len(), 2);
        ring.push_slice(&[3.0, 4.0]);
        assert_eq!(ring.len(), 3);
        assert_eq!(ring.pop(), Some(2.0));
        assert_eq!(ring.pop(), Some(3.0));
        assert_eq!(ring.pop(), Some(4.0));
        assert_eq!(ring.pop(), None);
    }

    #[test]
    fn pcm_ring_keeps_tail_of_oversized_push() {
        let mut ring = PcmRing::with_capacity(2);
        ring.push_slice(&[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(ring.len(), 2);
        assert_eq!(ring.pop(), Some(3.0));
        assert_eq!(ring.pop(), Some(4.0));
    }

    #[test]
    fn resampler_passthrough_is_identity() {
        let mut resampler = StreamResampler::new(16_000, 16_000).expect("passthrough");
        let input = vec![0.25_f32; 480];
        let mut emitted = Vec::new();
        resampler.push(&input, |chunk| emitted.extend_from_slice(chunk));
        assert_eq!(emitted, input);
    }

    #[test]
    fn resampler_converts_common_device_rates() {
        for rate in [8_000_u32, 44_100, 48_000] {
            let mut resampler =
                StreamResampler::new(rate, CODEC_SAMPLE_RATE).expect("resampler builds");
            // One second of a 440 Hz tone at the device rate.
            let input: Vec<f32> = (0..rate)
                .map(|index| {
                    (2.0 * std::f32::consts::PI * 440.0 * index as f32 / rate as f32).sin() * 0.5
                })
                .collect();
            let mut emitted = 0usize;
            resampler.push(&input, |chunk| {
                assert!(chunk.iter().all(|sample| sample.is_finite()));
                emitted += chunk.len();
            });
            // The fixed-ratio resampler consumes whole chunks; a second of
            // input yields roughly a second of 16 kHz output.
            let expected = input.len() * CODEC_SAMPLE_RATE as usize / rate as usize;
            assert!(
                emitted.abs_diff(expected) <= CODEC_FRAME_SAMPLES,
                "{rate} Hz: expected about {expected} samples, got {emitted}"
            );
        }
    }
}
