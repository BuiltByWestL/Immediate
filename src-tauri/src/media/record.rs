//! Quick recording from a microphone (or any input). Captured natively (WASAPI, Core Audio,
//! ALSA/Pulse) instead of through the webview, written straight to a 16-bit WAV in the user's
//! Music folder ("Immediate Recordings"), with the header refreshed every second so a crash
//! never loses what was recorded. Level and time arrive as "rec" events.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

type Writer = hound::WavWriter<std::io::BufWriter<std::fs::File>>;

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

/// Input devices by name; the first is the system default.
#[tauri::command]
pub async fn rec_devices() -> Vec<String> {
    tauri::async_runtime::spawn_blocking(|| {
        let host = cpal::default_host();
        let default = host.default_input_device().and_then(|d| d.description().ok().map(|x| x.name().to_owned()));
        let mut names: Vec<String> = host
            .input_devices()
            .map(|it| it.filter_map(|d| d.description().ok().map(|x| x.name().to_owned())).collect())
            .unwrap_or_default();
        if let Some(def) = default {
            names.retain(|n| *n != def);
            names.insert(0, def);
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
            let dev = match &device {
                Some(name) => host
                    .input_devices()
                    .map_err(|e| e.to_string())?
                    .find(|d| d.description().map(|x| x.name() == name).unwrap_or(false)),
                None => None,
            }
            .or_else(|| host.default_input_device())
            .ok_or("No microphone was found. Plug one in or check Windows’ privacy settings for the microphone.")?;
            let supported = dev.default_input_config().map_err(|e| format!("The microphone can’t be opened: {e}"))?;
            let channels = supported.channels().min(2);
            let rate = supported.sample_rate();
            let spec = hound::WavSpec { channels, sample_rate: rate, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
            let writer: Arc<Mutex<Option<Writer>>> = Arc::new(Mutex::new(Some(hound::WavWriter::create(&p, spec).map_err(|e| e.to_string())?)));
            let peak = Arc::new(AtomicU32::new(0));
            let frames = Arc::new(std::sync::atomic::AtomicU64::new(0));
            let in_ch = supported.channels() as usize;
            let config: cpal::StreamConfig = supported.config();

            macro_rules! stream_for {
                ($t:ty, $to_f32:expr) => {{
                    let (w, pk, fr, pa) = (writer.clone(), peak.clone(), frames.clone(), paused2.clone());
                    dev.build_input_stream::<$t, _, _>(
                        config.clone(),
                        move |data: &[$t], _| {
                            if pa.load(Ordering::Relaxed) {
                                return;
                            }
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
            loop {
                match stop_rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
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
