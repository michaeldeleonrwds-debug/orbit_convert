use orbit_core::formats::FileFormat;
use orbit_core::registry::{ConversionError, ConversionOptions, Converter, ProgressReporter};
use std::path::Path;
use std::process::Command;
use tracing::{debug, error, info};

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
        Command::new("ffmpeg")
            .arg("-version")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
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
            cb(0.1);
        }

        let mut cmd = Command::new("ffmpeg");
        cmd.arg("-y") // Overwrite output file
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

        debug!("Executing FFmpeg command: {:?}", cmd);

        let output_res = cmd
            .output()
            .map_err(|e| ConversionError::ProcessingError(format!("Failed to spawn ffmpeg: {}", e)))?;

        if !output_res.status.success() {
            let stderr = String::from_utf8_lossy(&output_res.stderr);
            error!("FFmpeg execution failed: {}", stderr);

            if stderr.contains("does not contain any stream") || stderr.contains("no audio streams")
            {
                return Err(ConversionError::ProcessingError(
                    "Source media does not contain an audio stream to extract".into(),
                ));
            }

            return Err(ConversionError::ProcessingError(format!(
                "FFmpeg conversion failed: {}",
                stderr.lines().last().unwrap_or("Unknown transcoding error")
            )));
        }

        if let Some(cb) = progress {
            cb(1.0);
        }

        Ok(())
    }
}
