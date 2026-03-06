use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use std::io::Write;
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

    guard.sample_rate = config.sample_rate().0;
    guard.channels = config.channels();

    let temp_file =
        NamedTempFile::new().map_err(|e| format!("Failed to create temp file: {}", e))?;
    let temp_path = temp_file.path().to_path_buf();

    let writer = Arc::new(Mutex::new(temp_file));
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
                            let bytes: Vec<u8> = data
                                .iter()
                                .flat_map(|sample| {
                                    let clamped = sample.clamp(-1.0, 1.0);
                                    let as_i16 = (clamped * 32767.0) as i16;
                                    as_i16.to_le_bytes()
                                })
                                .collect();
                            if let Ok(mut file) = wc.lock() {
                                let _ = file.write_all(&bytes);
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
                            let bytes: Vec<u8> = data
                                .iter()
                                .flat_map(|sample| sample.to_le_bytes())
                                .collect();
                            if let Ok(mut file) = wc.lock() {
                                let _ = file.write_all(&bytes);
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

    std::thread::sleep(std::time::Duration::from_millis(200));

    let pcm_path = guard
        .temp_file
        .take()
        .ok_or_else(|| "No temp file found".to_string())?;

    let sample_rate = guard.sample_rate;
    let channels = guard.channels;
    guard.is_recording = false;

    crate::tray::set_recording_state(false);

    let pcm_data = std::fs::read(&pcm_path)
        .map_err(|e| format!("Failed to read PCM data: {}", e))?;
    let _ = std::fs::remove_file(&pcm_path);

    let samples: Vec<i16> = pcm_data
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();

    match encode_opus(&samples, sample_rate, channels) {
        Ok(opus_path) => Ok(opus_path),
        Err(_) => {
            let wav_path = write_wav(&pcm_data, sample_rate, channels)?;
            Ok(wav_path)
        }
    }
}

fn encode_opus(samples: &[i16], sample_rate: u32, channels: u16) -> Result<String, String> {
    let opus_channels = if channels >= 2 {
        opus::Channels::Stereo
    } else {
        opus::Channels::Mono
    };

    let mut encoder = opus::Encoder::new(sample_rate, opus_channels, opus::Application::Audio)
        .map_err(|e| format!("Opus encoder error: {}", e))?;

    let frame_size = (sample_rate as usize / 50) * channels as usize;
    let mut encoded_frames: Vec<Vec<u8>> = Vec::new();

    for chunk in samples.chunks(frame_size) {
        let mut padded = chunk.to_vec();
        if padded.len() < frame_size {
            padded.resize(frame_size, 0);
        }
        let mut output = vec![0u8; 4000];
        match encoder.encode(&padded, &mut output) {
            Ok(len) => {
                output.truncate(len);
                encoded_frames.push(output);
            }
            Err(_) => continue,
        }
    }

    let mut out_file = NamedTempFile::with_suffix(".opus")
        .map_err(|e| format!("Temp file error: {}", e))?;
    for frame in &encoded_frames {
        let len_bytes = (frame.len() as u32).to_le_bytes();
        out_file.write_all(&len_bytes).map_err(|e| format!("Write error: {}", e))?;
        out_file.write_all(frame).map_err(|e| format!("Write error: {}", e))?;
    }

    let path = out_file.into_temp_path();
    let final_path = path.to_string_lossy().to_string();
    let _ = path.keep();

    Ok(final_path)
}

fn write_wav(pcm_data: &[u8], sample_rate: u32, channels: u16) -> Result<String, String> {
    let mut out_file = NamedTempFile::with_suffix(".wav")
        .map_err(|e| format!("Temp file error: {}", e))?;

    let data_len = pcm_data.len() as u32;
    let file_len = 36 + data_len;
    let byte_rate = sample_rate * channels as u32 * 2;
    let block_align = channels * 2;

    out_file.write_all(b"RIFF").map_err(|e| format!("Write error: {}", e))?;
    out_file.write_all(&file_len.to_le_bytes()).map_err(|e| format!("Write error: {}", e))?;
    out_file.write_all(b"WAVE").map_err(|e| format!("Write error: {}", e))?;
    out_file.write_all(b"fmt ").map_err(|e| format!("Write error: {}", e))?;
    out_file.write_all(&16u32.to_le_bytes()).map_err(|e| format!("Write error: {}", e))?;
    out_file.write_all(&1u16.to_le_bytes()).map_err(|e| format!("Write error: {}", e))?;
    out_file.write_all(&channels.to_le_bytes()).map_err(|e| format!("Write error: {}", e))?;
    out_file.write_all(&sample_rate.to_le_bytes()).map_err(|e| format!("Write error: {}", e))?;
    out_file.write_all(&byte_rate.to_le_bytes()).map_err(|e| format!("Write error: {}", e))?;
    out_file.write_all(&block_align.to_le_bytes()).map_err(|e| format!("Write error: {}", e))?;
    out_file.write_all(&16u16.to_le_bytes()).map_err(|e| format!("Write error: {}", e))?;
    out_file.write_all(b"data").map_err(|e| format!("Write error: {}", e))?;
    out_file.write_all(&data_len.to_le_bytes()).map_err(|e| format!("Write error: {}", e))?;
    out_file.write_all(pcm_data).map_err(|e| format!("Write error: {}", e))?;

    let path = out_file.into_temp_path();
    let final_path = path.to_string_lossy().to_string();
    let _ = path.keep();

    Ok(final_path)
}
