//! Quick recording from a microphone (or any input). Captured natively (WASAPI, Core Audio,
//! ALSA/Pulse) instead of through the webview, written straight to a 16-bit WAV in the user's
//! Music folder ("Immediate Recordings"), with the header refreshed every second so a crash
//! never loses what was recorded. Level and time arrive as "rec" events.
//!
//! On Windows, "System sound: <speakers>" records what the computer is playing (WASAPI
//! loopback). Linux lists its "Monitor of ..." sources as ordinary inputs already.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

type Writer = hound::WavWriter<std::io::BufWriter<std::fs::File>>;

/// The recording file: 16-bit PCM WAV. `flush` (every second while recording) rewrites the
/// header, so the file is valid up to that point even if Immediate is killed.
fn new_writer(path: &std::path::Path, rate: u32, channels: u16) -> Result<Writer, String> {
    let spec = hound::WavSpec { channels, sample_rate: rate, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    hound::WavWriter::create(path, spec).map_err(|e| e.to_string())
}

struct Active {
    stop: mpsc::Sender<()>,
    paused: Arc<AtomicBool>,
    path: PathBuf,
    done: mpsc::Receiver<Result<(), String>>,
}

static ACTIVE: Mutex<Option<Active>> = Mutex::new(None);

#[derive(Serialize, Clone)]
struct Level {
    /// Peak of the last ~50 ms, 0..1.
    peak: f32,
    seconds: f64,
    paused: bool,
}

/// Names of "what the computer is playing" sources: this prefix, then the speakers' name.
const SYSTEM: &str = "System sound: ";

/// Input devices by name; the first is the system default. On Windows, each output follows as
/// "System sound: <name>" (default speakers first).
#[tauri::command]
pub async fn rec_devices() -> Vec<String> {
    tauri::async_runtime::spawn_blocking(|| {
        let host = cpal::default_host();
        let name = |d: &cpal::Device| d.description().ok().map(|x| x.name().to_owned());
        let default = host.default_input_device().and_then(|d| name(&d));
        let mut names: Vec<String> = host.input_devices().map(|it| it.filter_map(|d| name(&d)).collect()).unwrap_or_default();
        if let Some(def) = default {
            names.retain(|n| *n != def);
            names.insert(0, def);
        }
        if cfg!(windows) {
            let default_out = host.default_output_device().and_then(|d| name(&d));
            let mut outs: Vec<String> = host.output_devices().map(|it| it.filter_map(|d| name(&d)).collect()).unwrap_or_default();
            if let Some(def) = default_out {
                outs.retain(|n| *n != def);
                outs.insert(0, def);
            }
            names.extend(outs.into_iter().map(|n| format!("{SYSTEM}{n}")));
        }
        names
    })
    .await
    .unwrap_or_default()
}

fn recordings_dir<R: Runtime>(app: &AppHandle<R>) -> PathBuf {
    let base = app.path().audio_dir().or_else(|_| app.path().document_dir()).unwrap_or_else(|_| std::env::temp_dir());
    base.join("Immediate Recordings")
}

fn stamp() -> String {
    // Local time without a date library: ask the OS for it through the file system clock.
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("{now}")
}

/// Starts recording. Returns the file it is being written to.
#[tauri::command]
pub async fn rec_start<R: Runtime>(app: AppHandle<R>, device: Option<String>, name: Option<String>) -> Result<String, String> {
    if ACTIVE.lock().map_err(|e| e.to_string())?.is_some() {
        return Err("Already recording.".into());
    }
    let dir = recordings_dir(&app);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stem = name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| format!("Recording {}", stamp()));
    let path = crate::files::unique_path(&dir, &stem, "wav");

    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
    let (done_tx, done_rx) = mpsc::channel::<Result<(), String>>();
    let paused = Arc::new(AtomicBool::new(false));
    let p = path.clone();
    let paused2 = paused.clone();
    let handle = app.clone();

    // The stream lives on its own thread (it isn't Send on every platform).
    std::thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            let host = cpal::default_host();
            let named = |name: &str, d: &cpal::Device| d.description().map(|x| x.name() == name).unwrap_or(false);
            // What the computer plays: the speakers, opened for recording (WASAPI loopback).
            let system = device.as_deref().and_then(|d| d.strip_prefix(SYSTEM)).map(str::to_owned);
            let (dev, supported) = if let Some(out) = &system {
                let dev = host
                    .output_devices()
                    .map_err(|e| e.to_string())?
                    .find(|d| named(out, d))
                    .or_else(|| host.default_output_device())
                    .ok_or("No speakers or headphones were found to record from.")?;
                let cfg = dev.default_output_config().map_err(|e| format!("The system sound can’t be recorded: {e}"))?;
                (dev, cfg)
            } else {
                let dev = match &device {
                    Some(name) => host.input_devices().map_err(|e| e.to_string())?.find(|d| named(name, d)),
                    None => None,
                }
                .or_else(|| host.default_input_device())
                .ok_or("No microphone was found. Plug one in or check Windows’ privacy settings for the microphone.")?;
                let cfg = dev.default_input_config().map_err(|e| format!("The microphone can’t be opened: {e}"))?;
                (dev, cfg)
            };
            let channels = supported.channels().min(2);
            let rate = supported.sample_rate();
            let writer: Arc<Mutex<Option<Writer>>> = Arc::new(Mutex::new(Some(new_writer(&p, rate, channels)?)));
            let peak = Arc::new(AtomicU32::new(0));
            let frames = Arc::new(std::sync::atomic::AtomicU64::new(0));
            // When the sound last arrived (ms since start): loopback sends nothing during silence.
            let started = std::time::Instant::now();
            let heard = Arc::new(std::sync::atomic::AtomicU64::new(0));
            let in_ch = supported.channels() as usize;
            let config: cpal::StreamConfig = supported.config();

            macro_rules! stream_for {
                ($t:ty, $to_f32:expr) => {{
                    let (w, pk, fr, pa, hd) = (writer.clone(), peak.clone(), frames.clone(), paused2.clone(), heard.clone());
                    dev.build_input_stream::<$t, _, _>(
                        config.clone(),
                        move |data: &[$t], _| {
                            if pa.load(Ordering::Relaxed) {
                                return;
                            }
                            hd.store(started.elapsed().as_millis() as u64, Ordering::Relaxed);
                            let mut local_peak = 0f32;
                            if let Ok(mut g) = w.lock() {
                                if let Some(wr) = g.as_mut() {
                                    for frame in data.chunks(in_ch) {
                                        for c in 0..channels as usize {
                                            let v: f32 = $to_f32(frame[c.min(frame.len() - 1)]);
                                            local_peak = local_peak.max(v.abs());
                                            let _ = wr.write_sample((v.clamp(-1.0, 1.0) * 32767.0) as i16);
                                        }
                                    }
                                }
                            }
                            fr.fetch_add((data.len() / in_ch.max(1)) as u64, Ordering::Relaxed);
                            let prev = f32::from_bits(pk.load(Ordering::Relaxed));
                            pk.store(prev.max(local_peak).to_bits(), Ordering::Relaxed);
                        },
                        |e| eprintln!("recording error: {e}"),
                        None,
                    )
                }};
            }
            let stream = match supported.sample_format() {
                cpal::SampleFormat::F32 => stream_for!(f32, |v: f32| v),
                cpal::SampleFormat::I16 => stream_for!(i16, |v: i16| v as f32 / 32768.0),
                cpal::SampleFormat::I32 => stream_for!(i32, |v: i32| v as f32 / 2147483648.0),
                cpal::SampleFormat::U16 => stream_for!(u16, |v: u16| (v as f32 - 32768.0) / 32768.0),
                other => return Err(format!("Unsupported microphone format {other:?}")),
            }
            .map_err(|e| format!("The microphone can’t be opened: {e}"))?;
            stream.play().map_err(|e| e.to_string())?;
            let _ = ready_tx.send(Ok(()));

            let mut ticks = 0u32;
            // Recorded time so far (pauses excluded), to fill loopback's silent gaps.
            let mut clock = 0f64;
            let mut last = std::time::Instant::now();
            loop {
                match stop_rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
                let now = std::time::Instant::now();
                if !paused2.load(Ordering::Relaxed) {
                    clock += (now - last).as_secs_f64();
                }
                last = now;
                let quiet_ms = (started.elapsed().as_millis() as u64).saturating_sub(heard.load(Ordering::Relaxed));
                if system.is_some() && !paused2.load(Ordering::Relaxed) && quiet_ms > 150 {
                    let want = (clock * rate as f64) as u64;
                    let have = frames.load(Ordering::Relaxed);
                    if want > have {
                        if let Ok(mut g) = writer.lock() {
                            if let Some(w) = g.as_mut() {
                                for _ in 0..(want - have) * channels as u64 {
                                    let _ = w.write_sample(0i16);
                                }
                            }
                        }
                        frames.fetch_add(want - have, Ordering::Relaxed);
                    }
                }
                let pk = f32::from_bits(peak.swap(0, Ordering::Relaxed));
                let seconds = frames.load(Ordering::Relaxed) as f64 / rate as f64;
                let _ = handle.emit("rec", Level { peak: pk, seconds, paused: paused2.load(Ordering::Relaxed) });
                ticks += 1;
                // Keep the file valid on disk every second, in case anything crashes.
                if ticks % 20 == 0 {
                    if let Ok(mut g) = writer.lock() {
                        if let Some(w) = g.as_mut() {
                            let _ = w.flush();
                        }
                    }
                }
            }
            drop(stream);
            if let Some(w) = writer.lock().map_err(|e| e.to_string())?.take() {
                w.finalize().map_err(|e| e.to_string())?;
            }
            Ok(())
        })();
        if let Err(e) = &result {
            let _ = ready_tx.send(Err(e.clone()));
        }
        let _ = done_tx.send(result);
    });

    match ready_rx.recv_timeout(Duration::from_secs(10)) {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            let _ = std::fs::remove_file(&path);
            return Err(e);
        }
        Err(_) => return Err("The microphone didn’t start.".into()),
    }
    *ACTIVE.lock().map_err(|e| e.to_string())? = Some(Active { stop: stop_tx, paused, path: path.clone(), done: done_rx });
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn rec_pause(paused: bool) {
    if let Ok(g) = ACTIVE.lock() {
        if let Some(a) = g.as_ref() {
            a.paused.store(paused, Ordering::Relaxed);
        }
    }
}

/// Stops recording. `keep: false` deletes the file. Returns its path.
#[tauri::command]
pub async fn rec_stop<R: Runtime>(app: AppHandle<R>, keep: bool) -> Result<String, String> {
    let active = ACTIVE.lock().map_err(|e| e.to_string())?.take().ok_or("Not recording.")?;
    let _ = active.stop.send(());
    let path = active.path.clone();
    let result = tauri::async_runtime::spawn_blocking(move || active.done.recv_timeout(Duration::from_secs(10)).unwrap_or(Ok(())))
        .await
        .map_err(|e| e.to_string())?;
    if !keep {
        let _ = std::fs::remove_file(&path);
        return Ok(String::new());
    }
    result?;
    let _ = app.asset_protocol_scope().allow_file(&path);
    Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A recording that is never finished (Immediate killed mid-recording) is still a valid
    /// WAV holding everything up to the last once-a-second flush.
    #[test]
    fn a_killed_recording_is_still_a_valid_wav() {
        let path = std::env::temp_dir().join(format!("immediate-rec-{}.wav", std::process::id()));
        let mut w = new_writer(&path, 48000, 2).unwrap();
        for i in 0..48000 * 2 {
            w.write_sample(((i % 200) as i16 - 100) * 50).unwrap();
        }
        w.flush().unwrap();
        // Half a second more that never reached a flush, then "killed": no finalize, no Drop.
        for _ in 0..24000 * 2 {
            w.write_sample(1000i16).unwrap();
        }
        std::mem::forget(w);
        let r = hound::WavReader::open(&path).expect("valid WAV after a crash");
        assert_eq!(r.spec().sample_rate, 48000);
        assert_eq!(r.spec().channels, 2);
        // At least the flushed second, and every sample readable.
        assert!(r.duration() >= 48000, "{}", r.duration());
        let n = r.into_samples::<i16>().filter(|s| s.is_ok()).count();
        assert!(n >= 48000 * 2);
        let _ = std::fs::remove_file(path);
    }
}
