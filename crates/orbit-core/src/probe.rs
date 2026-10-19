use crate::formats::FileFormat;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::SystemTime;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MediaStreamInfo {
    pub has_video: bool,
    pub has_audio: bool,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f64>,
    pub duration_secs: Option<f64>,
    pub bit_rate: Option<u64>,
    pub format_name: Option<String>,
    pub detected_format: Option<FileFormat>,
}

static PROBE_CACHE: Mutex<Option<HashMap<PathBuf, (SystemTime, MediaStreamInfo)>>> =
    Mutex::new(None);

/// Probes media container and stream information using ffprobe.
/// Results are cached by file path and modified time.
pub fn probe_media(path: &Path) -> Option<MediaStreamInfo> {
    let mtime = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);

    {
        let mut guard = PROBE_CACHE.lock().unwrap();
        if guard.is_none() {
            *guard = Some(HashMap::new());
        }
        if let Some(cache) = guard.as_ref() {
            if let Some((cached_mtime, info)) = cache.get(path) {
                if *cached_mtime == mtime {
                    return Some(info.clone());
                }
            }
        }
    }

    let mut cmd = Command::new("ffprobe");
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.args(&[
        "-v",
        "quiet",
        "-print_format",
        "json",
        "-show_format",
        "-show_streams",
    ])
    .arg(path);

    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }

    let val: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let streams = val.get("streams").and_then(|s| s.as_array())?;

    let mut has_video = false;
    let mut has_audio = false;
    let mut video_codec = None;
    let mut audio_codec = None;
    let mut width = None;
    let mut height = None;
    let mut fps = None;

    for stream in streams {
        let codec_type = stream
            .get("codec_type")
            .and_then(|ct| ct.as_str())
            .unwrap_or("");
        if codec_type == "video" && !has_video {
            has_video = true;
            video_codec = stream
                .get("codec_name")
                .and_then(|c| c.as_str())
                .map(|s| s.to_string());
            width = stream.get("width").and_then(|w| w.as_u64()).map(|w| w as u32);
            height = stream.get("height").and_then(|h| h.as_u64()).map(|h| h as u32);
            if let Some(r_fps) = stream.get("r_frame_rate").and_then(|r| r.as_str()) {
                if let Some((num, den)) = r_fps.split_once('/') {
                    if let (Ok(n), Ok(d)) = (num.parse::<f64>(), den.parse::<f64>()) {
                        if d > 0.0 {
                            fps = Some(n / d);
                        }
                    }
                }
            }
        } else if codec_type == "audio" && !has_audio {
            has_audio = true;
            audio_codec = stream
                .get("codec_name")
                .and_then(|c| c.as_str())
                .map(|s| s.to_string());
        }
    }

    let format_obj = val.get("format");
    let duration_secs = format_obj
        .and_then(|f| f.get("duration"))
        .and_then(|d| d.as_str())
        .and_then(|s| s.parse::<f64>().ok());
    let bit_rate = format_obj
        .and_then(|f| f.get("bit_rate"))
        .and_then(|b| b.as_str())
        .and_then(|s| s.parse::<u64>().ok());
    let format_name = format_obj
        .and_then(|f| f.get("format_name"))
        .and_then(|fn_| fn_.as_str())
        .map(|s| s.to_string());

    let detected_format = format_name.as_deref().and_then(|fn_| {
        if fn_.contains("mp4") || fn_.contains("mov") || fn_.contains("m4a") {
            Some(FileFormat::Mp4)
        } else if fn_.contains("matroska") || fn_.contains("webm") {
            if fn_.contains("webm") {
                Some(FileFormat::Webm)
            } else {
                Some(FileFormat::Mkv)
            }
        } else if fn_.contains("avi") {
            Some(FileFormat::Avi)
        } else if fn_.contains("asf") {
            Some(FileFormat::Wmv)
        } else if fn_.contains("flv") {
            Some(FileFormat::Flv)
        } else if fn_.contains("mpegts") {
            Some(FileFormat::Ts)
        } else if fn_.contains("mpeg") {
            Some(FileFormat::Mpeg)
        } else if fn_.contains("ogg") {
            Some(FileFormat::Ogg)
        } else if fn_.contains("mp3") {
            Some(FileFormat::Mp3)
        } else if fn_.contains("wav") {
            Some(FileFormat::Wav)
        } else if fn_.contains("flac") {
            Some(FileFormat::Flac)
        } else {
            None
        }
    });

    let info = MediaStreamInfo {
        has_video,
        has_audio,
        video_codec,
        audio_codec,
        width,
        height,
        fps,
        duration_secs,
        bit_rate,
        format_name,
        detected_format,
    };

    {
        let mut guard = PROBE_CACHE.lock().unwrap();
        if let Some(cache) = guard.as_mut() {
            cache.insert(path.to_path_buf(), (mtime, info.clone()));
        }
    }

    Some(info)
}
