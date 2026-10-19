use orbit_core::formats::FileFormat;
use orbit_core::registry::{ConversionError, ConversionOptions, Converter, ProgressReporter};
use std::io::BufRead;
use std::path::Path;
use std::process::{Command, Stdio};
use tracing::{debug, error, info};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub struct FfmpegConverter {
    available: bool,
}

impl FfmpegConverter {
    pub fn new() -> Self {
        let available = Self::check_ffmpeg_available();
        if available {
            info!("FFmpeg detected and registered for video/audio conversions");
        } else {
            debug!("FFmpeg executable not found in PATH");
        }
        Self { available }
    }

    pub fn is_available(&self) -> bool {
        self.available
    }

    pub fn check_ffmpeg_available() -> bool {
        let mut cmd = Command::new("ffmpeg");
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd.arg("-version")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    pub fn get_media_duration_secs(input: &Path) -> Option<f64> {
        let mut cmd = Command::new("ffprobe");
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd.args(&[
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(input);

        let out = cmd.output().ok()?;
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            s.trim().parse::<f64>().ok()
        } else {
            None
        }
    }

    fn is_video(fmt: FileFormat) -> bool {
        matches!(
            fmt,
            FileFormat::Mp4
                | FileFormat::Mov
                | FileFormat::Mkv
                | FileFormat::Avi
                | FileFormat::Webm
        )
    }

    fn is_audio(fmt: FileFormat) -> bool {
        matches!(
            fmt,
            FileFormat::Mp3
                | FileFormat::Wav
                | FileFormat::Flac
                | FileFormat::Aac
                | FileFormat::M4a
                | FileFormat::Ogg
        )
    }
}

impl Converter for FfmpegConverter {
    fn id(&self) -> &'static str {
        "ffmpeg-media-engine"
    }

    fn supports(&self, from: FileFormat, to: FileFormat) -> bool {
        if !self.available {
            return false;
        }

        // 1. Video to Video
        if Self::is_video(from) && Self::is_video(to) {
            return true;
        }

        // 2. Video to Audio (extraction)
        if Self::is_video(from) && Self::is_audio(to) {
            return true;
        }

        // 3. Audio to Audio (transcoding)
        if Self::is_audio(from) && Self::is_audio(to) {
            return true;
        }

        // 4. Video to Animated GIF
        if Self::is_video(from) && to == FileFormat::Gif {
            return true;
        }

        false
    }

    fn convert(
        &self,
        input: &Path,
        output: &Path,
        _options: &ConversionOptions,
        progress: ProgressReporter<'_>,
    ) -> Result<(), ConversionError> {
        if !self.available {
            return Err(ConversionError::ProcessingError(
                "FFmpeg is not installed or not found in system PATH".into(),
            ));
        }

        let from_ext = input
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();
        let to_ext = output
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();

        let from_fmt = FileFormat::from_extension(from_ext).ok_or_else(|| {
            ConversionError::InvalidInput(format!("Unrecognized source extension '{}'", from_ext))
        })?;
        let to_fmt = FileFormat::from_extension(to_ext).ok_or_else(|| {
            ConversionError::InvalidInput(format!("Unrecognized target extension '{}'", to_ext))
        })?;

        if let Some(cb) = progress {
            cb(0.05);
        }

        let total_duration_secs = Self::get_media_duration_secs(input);
        let total_duration_us = total_duration_secs
            .map(|s| (s * 1_000_000.0) as u64)
            .unwrap_or(0);

        let mut cmd = Command::new("ffmpeg");
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);

        cmd.arg("-y") // Overwrite output file
            .arg("-progress")
            .arg("pipe:1")
            .arg("-nostats")
            .arg("-i")
            .arg(input);

        // Configure transcoding arguments depending on destination format
        match (Self::is_video(from_fmt), Self::is_audio(from_fmt), to_fmt) {
            // Video -> Audio (Stream extraction)
            (true, _, FileFormat::Mp3) | (false, true, FileFormat::Mp3) => {
                cmd.args(&["-vn", "-c:a", "libmp3lame", "-q:a", "2"]);
            }
            (true, _, FileFormat::Wav) | (false, true, FileFormat::Wav) => {
                cmd.args(&["-vn", "-c:a", "pcm_s16le"]);
            }
            (true, _, FileFormat::Aac) | (false, true, FileFormat::Aac) => {
                cmd.args(&["-vn", "-c:a", "aac", "-b:a", "192k"]);
            }
            (true, _, FileFormat::M4a) | (false, true, FileFormat::M4a) => {
                cmd.args(&["-vn", "-c:a", "aac", "-b:a", "192k"]);
            }
            (true, _, FileFormat::Flac) | (false, true, FileFormat::Flac) => {
                cmd.args(&["-vn", "-c:a", "flac"]);
            }
            (true, _, FileFormat::Ogg) | (false, true, FileFormat::Ogg) => {
                cmd.args(&["-vn", "-c:a", "libvorbis", "-q:a", "5"]);
            }

            // Video -> Animated GIF
            (true, _, FileFormat::Gif) => {
                cmd.args(&["-vf", "fps=15,scale=480:-1:flags=lanczos", "-c:v", "gif"]);
            }

            // Video -> Video Transcoding
            (true, _, FileFormat::Mp4) => {
                cmd.args(&[
                    "-c:v",
                    "libx264",
                    "-pix_fmt",
                    "yuv420p",
                    "-c:a",
                    "aac",
                    "-b:a",
                    "192k",
                ]);
            }
            (true, _, FileFormat::Webm) => {
                cmd.args(&[
                    "-c:v",
                    "libvpx-vp9",
                    "-b:v",
                    "0",
                    "-crf",
                    "30",
                    "-c:a",
                    "libopus",
                ]);
            }
            (true, _, FileFormat::Mkv) => {
                cmd.args(&["-c:v", "libx264", "-c:a", "aac"]);
            }
            (true, _, FileFormat::Mov) => {
                cmd.args(&["-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac"]);
            }
            (true, _, FileFormat::Avi) => {
                cmd.args(&["-c:v", "mpeg4", "-q:v", "4", "-c:a", "mp3"]);
            }

            _ => {
                return Err(ConversionError::UnsupportedConversion {
                    from: from_fmt,
                    to: to_fmt,
                });
            }
        }

        cmd.arg(output);

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        debug!("Executing FFmpeg command: {:?}", cmd);

        let mut child = cmd
            .spawn()
            .map_err(|e| ConversionError::ProcessingError(format!("Failed to spawn ffmpeg: {}", e)))?;

        // Drain stderr in background thread to prevent pipe buffer deadlock
        let stderr_pipe = child.stderr.take();
        let stderr_thread = std::thread::spawn(move || {
            let mut err_data = Vec::new();
            if let Some(mut pipe) = stderr_pipe {
                let _ = std::io::Read::read_to_end(&mut pipe, &mut err_data);
            }
            err_data
        });

        if let Some(stdout_pipe) = child.stdout.take() {
            let reader = std::io::BufReader::new(stdout_pipe);
            for line in reader.lines() {
                if let Ok(l) = line {
                    if let Some(val) = l.strip_prefix("out_time_us=") {
                        if let Ok(us) = val.trim().parse::<u64>() {
                            if total_duration_us > 0 {
                                let ratio = (us as f64 / total_duration_us as f64).clamp(0.05, 0.98) as f32;
                                if let Some(cb) = progress {
                                    cb(ratio);
                                }
                            }
                        }
                    } else if l.trim() == "progress=end" {
                        if let Some(cb) = progress {
                            cb(0.99);
                        }
                    }
                }
            }
        }

        let status = child
            .wait()
            .map_err(|e| ConversionError::ProcessingError(format!("Failed to wait for ffmpeg: {}", e)))?;

        let stderr_bytes = stderr_thread.join().unwrap_or_default();
        let stderr = String::from_utf8_lossy(&stderr_bytes);

        if !status.success() {
            error!("FFmpeg execution failed: {}", stderr);

            if stderr.contains("does not contain any stream") || stderr.contains("no audio streams")
            {
                return Err(ConversionError::ProcessingError(
                    "Source media does not contain an audio stream to extract".into(),
                ));
            }

            let clean_err = stderr
                .lines()
                .rev()
                .find(|l| {
                    let trim = l.trim();
                    !trim.is_empty()
                        && !trim.starts_with("frame=")
                        && !trim.starts_with("out_time")
                        && !trim.starts_with("progress=")
                        && !trim.starts_with("bitrate=")
                        && !trim.starts_with("speed=")
                })
                .unwrap_or("Unknown transcoding error");

            return Err(ConversionError::ProcessingError(format!(
                "FFmpeg conversion failed: {}",
                clean_err
            )));
        }

        if let Some(cb) = progress {
            cb(1.0);
        }

        Ok(())
    }
}
