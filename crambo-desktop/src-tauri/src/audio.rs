use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use tauri::command;
use tempfile::NamedTempFile;

struct RecordingState {
    temp_file: Option<std::path::PathBuf>,
    is_recording: bool,
    stop_signal: Option<Arc<std::sync::atomic::AtomicBool>>,
    sample_rate: u32,
    channels: u16,
}

unsafe impl Send for RecordingState {}
unsafe impl Sync for RecordingState {}

static RECORDING: std::sync::OnceLock<Mutex<RecordingState>> = std::sync::OnceLock::new();

fn get_state() -> &'static Mutex<RecordingState> {
    RECORDING.get_or_init(|| {
        Mutex::new(RecordingState {
            temp_file: None,
            is_recording: false,
            stop_signal: None,
            sample_rate: 48000,
            channels: 1,
        })
    })
}

fn find_device_by_name(host: &cpal::Host, name: &str) -> Option<cpal::Device> {
    if let Ok(devices) = host.input_devices() {
        for device in devices {
            if let Ok(dev_name) = device.name() {
                if dev_name.to_lowercase().contains(&name.to_lowercase()) {
                    return Some(device);
                }
            }
        }
    }
    None
}

fn get_input_device(host: &cpal::Host, mode: &str) -> Result<cpal::Device, String> {
    match mode {
        "system" => {
            #[cfg(target_os = "macos")]
            {
                if let Some(device) = find_device_by_name(host, "BlackHole") {
                    return Ok(device);
                }
                return host
                    .default_input_device()
                    .ok_or_else(|| "No input device available. Install BlackHole for system audio capture on macOS.".to_string());
            }

            #[cfg(target_os = "windows")]
            {
                return host
                    .default_input_device()
                    .ok_or_else(|| "No loopback device available. WASAPI loopback requires a compatible audio device.".to_string());
            }

            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            {
                return host
                    .default_input_device()
                    .ok_or_else(|| "No input device available".to_string());
            }
        }
        _ => host
            .default_input_device()
            .ok_or_else(|| "No input device available".to_string()),
    }
}

#[command]
pub fn start_recording(mode: String) -> Result<(), String> {
    let state = get_state();
    let mut guard = state.lock().map_err(|e| format!("Lock error: {}", e))?;

    if guard.is_recording {
        return Err("Already recording".to_string());
    }

    let host = cpal::default_host();
    let device = get_input_device(&host, &mode)?;

    let config = device
        .default_input_config()
        .map_err(|e| format!("Failed to get input config: {}", e))?;

    let sample_rate = config.sample_rate().0;
    let channels = config.channels();
    guard.sample_rate = sample_rate;
    guard.channels = channels;

    let wav_spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let temp_file = NamedTempFile::with_suffix(".wav")
        .map_err(|e| format!("Failed to create temp file: {}", e))?;
    let temp_path = temp_file.path().to_path_buf();

    let wav_writer = hound::WavWriter::create(temp_file.path(), wav_spec)
        .map_err(|e| format!("Failed to create WAV writer: {}", e))?;
    let writer = Arc::new(Mutex::new(wav_writer));
    let writer_clone = Arc::clone(&writer);

    let stop_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_flag_clone = stop_flag.clone();

    crate::tray::set_recording_state(true);

    std::thread::spawn(move || {
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => {
                let wc = Arc::clone(&writer_clone);
                device
                    .build_input_stream(
                        &config.into(),
                        move |data: &[f32], _: &cpal::InputCallbackInfo| {
                            if let Ok(mut w) = wc.lock() {
                                for &sample in data {
                                    let clamped = sample.clamp(-1.0, 1.0);
                                    let as_i16 = (clamped * 32767.0) as i16;
                                    let _ = w.write_sample(as_i16);
                                }
                            }
                        },
                        move |err| {
                            eprintln!("Audio stream error: {}", err);
                        },
                        None,
                    )
                    .ok()
            }
            cpal::SampleFormat::I16 => {
                let wc = Arc::clone(&writer_clone);
                device
                    .build_input_stream(
                        &config.into(),
                        move |data: &[i16], _: &cpal::InputCallbackInfo| {
                            if let Ok(mut w) = wc.lock() {
                                for &sample in data {
                                    let _ = w.write_sample(sample);
                                }
                            }
                        },
                        move |err| {
                            eprintln!("Audio stream error: {}", err);
                        },
                        None,
                    )
                    .ok()
            }
            _ => None,
        };

        if let Some(stream) = stream {
            let _ = stream.play();
            while !stop_flag_clone.load(std::sync::atomic::Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            drop(stream);
        }

        if let Ok(mut w) = writer_clone.lock() {
            let _ = w.flush();
        }
    });

    guard.temp_file = Some(temp_path);
    guard.is_recording = true;
    guard.stop_signal = Some(stop_flag);

    Ok(())
}

#[command]
pub fn stop_recording() -> Result<String, String> {
    let state = get_state();
    let mut guard = state.lock().map_err(|e| format!("Lock error: {}", e))?;

    if !guard.is_recording {
        return Err("Not recording".to_string());
    }

    if let Some(signal) = guard.stop_signal.take() {
        signal.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    std::thread::sleep(std::time::Duration::from_millis(300));

    let wav_path = guard
        .temp_file
        .take()
        .ok_or_else(|| "No temp file found".to_string())?;

    guard.is_recording = false;

    crate::tray::set_recording_state(false);

    wav_path
        .to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Invalid path".to_string())
}
