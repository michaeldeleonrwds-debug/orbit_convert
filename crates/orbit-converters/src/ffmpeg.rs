use orbit_core::formats::FileFormat;
use orbit_core::probe::probe_media;
use orbit_core::registry::{ConversionError, ConversionOptions, Converter, ProgressReporter};
use std::io::BufRead;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;
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
}

impl Converter for FfmpegConverter {
    fn id(&self) -> &'static str {
        "ffmpeg-media-engine"
    }

    fn supports(&self, from: FileFormat, to: FileFormat) -> bool {
        if !self.available {
            return false;
        }

        // 1. Video to Video (and Video to Animated GIF/WebP)
        if from.is_video() {
            if to.is_video() || to == FileFormat::Gif || to == FileFormat::Webp {
                return true;
            }
            // Video to Audio (extraction)
            if to.is_audio() {
                return true;
            }
        }

        // 2. Audio to Audio (transcoding)
        if from.is_audio() && to.is_audio() {
            return true;
        }

        false
    }

    fn convert(
        &self,
        input: &Path,
        output: &Path,
        options: &ConversionOptions,
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

        // Audio extraction verification:
        // Ensure source file actually has an audio stream before attempting extraction
        if from_fmt.is_video() && to_fmt.is_audio() {
            if let Some(info) = probe_media(input) {
                if !info.has_audio {
                    return Err(ConversionError::ProcessingError(
                        format!(
                            "Source video '{}' does not contain an audio stream to extract to {}",
                            input.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
                            to_fmt.display_label()
                        ),
                    ));
                }
            }
        }

        if let Some(cb) = progress {
            cb(0.02);
        }

        let total_duration_secs = Self::get_media_duration_secs(input)
            .or_else(|| probe_media(input).and_then(|i| i.duration_secs));
        let total_duration_us = total_duration_secs
            .map(|s| (s * 1_000_000.0) as u64)
            .unwrap_or(0);

        let mut cmd = Command::new("ffmpeg");
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);

        // Hardware acceleration if specified
        if let Some(ref hw) = options.hardware_accel {
            if hw != "none" {
                cmd.arg("-hwaccel").arg(hw);
            }
        }

        cmd.arg("-y") // Overwrite output file
            .arg("-progress")
            .arg("pipe:1")
            .arg("-nostats")
            .arg("-i")
            .arg(input);

        if options.preserve_metadata {
            cmd.args(&["-map_metadata", "0"]);
        }

        // Configure transcoding arguments depending on destination format
        if options.stream_copy && from_fmt == to_fmt {
            cmd.args(&["-c", "copy"]);
        } else {
            match (from_fmt.is_video(), from_fmt.is_audio(), to_fmt) {
                // Video -> Audio (Stream extraction) or Audio -> Audio (Transcoding)
                (_, _, FileFormat::Mp3) => {
                    let bitrate = options.audio_bitrate.as_deref().unwrap_or("320k");
                    cmd.args(&["-vn", "-c:a", "libmp3lame", "-b:a", bitrate]);
                }
                (_, _, FileFormat::Wav) => {
                    cmd.args(&["-vn", "-c:a", "pcm_s16le"]);
                }
                (_, _, FileFormat::Aac) => {
                    let bitrate = options.audio_bitrate.as_deref().unwrap_or("192k");
                    cmd.args(&["-vn", "-c:a", "aac", "-b:a", bitrate]);
                }
                (_, _, FileFormat::M4a) => {
                    let bitrate = options.audio_bitrate.as_deref().unwrap_or("192k");
                    cmd.args(&["-vn", "-c:a", "aac", "-b:a", bitrate]);
                }
                (_, _, FileFormat::Flac) => {
                    cmd.args(&["-vn", "-c:a", "flac"]);
                }
                (_, _, FileFormat::Ogg) => {
                    cmd.args(&["-vn", "-c:a", "libvorbis", "-q:a", "6"]);
                }
                (_, _, FileFormat::Opus) => {
                    let bitrate = options.audio_bitrate.as_deref().unwrap_or("160k");
                    cmd.args(&["-vn", "-c:a", "libopus", "-b:a", bitrate]);
                }

                // Video -> Animated GIF (High-Quality Palette Generation)
                (true, _, FileFormat::Gif) => {
                    cmd.args(&[
                        "-vf",
                        "fps=15,scale=480:-1:flags=lanczos,split[s0][s1];[s0]palettegen=max_colors=128[p];[s1][p]paletteuse=dither=bayer",
                        "-c:v",
                        "gif",
                    ]);
                }

                // Video -> Animated WebP
                (true, _, FileFormat::Webp) => {
                    cmd.args(&[
                        "-vcodec",
                        "libwebp",
                        "-loop",
                        "0",
                        "-vf",
                        "fps=15,scale=480:-1:flags=lanczos",
                        "-q:v",
                        "75",
                    ]);
                }

                // Video -> Video Transcoding
                (true, _, FileFormat::Mp4) => {
                    let v_codec = options.video_codec.as_deref().unwrap_or("libx264");
                    let a_codec = options.audio_codec.as_deref().unwrap_or("aac");
                    let crf_str = options.video_crf.unwrap_or(23).to_string();

                    cmd.args(&[
                        "-c:v",
                        v_codec,
                        "-pix_fmt",
                        "yuv420p",
                        "-preset",
                        "medium",
                        "-crf",
                        &crf_str,
                        "-c:a",
                        a_codec,
                        "-b:a",
                        "192k",
                    ]);
                }
                (true, _, FileFormat::Webm) => {
                    let v_codec = options.video_codec.as_deref().unwrap_or("libvpx-vp9");
                    let a_codec = options.audio_codec.as_deref().unwrap_or("libopus");
                    let crf_str = options.video_crf.unwrap_or(30).to_string();

                    cmd.args(&[
                        "-c:v",
                        v_codec,
                        "-b:v",
                        "0",
                        "-crf",
                        &crf_str,
                        "-c:a",
                        a_codec,
                        "-b:a",
                        "128k",
                    ]);
                }
                (true, _, FileFormat::Mkv) => {
                    cmd.args(&[
                        "-c:v",
                        "libx264",
                        "-pix_fmt",
                        "yuv420p",
                        "-preset",
                        "medium",
                        "-crf",
                        "23",
                        "-c:a",
                        "aac",
                        "-b:a",
                        "192k",
                    ]);
                }
                (true, _, FileFormat::Mov) => {
                    let is_prores = options
                        .video_codec
                        .as_deref()
                        .map_or(false, |c| c.contains("prores"));
                    if is_prores {
                        cmd.args(&[
                            "-c:v",
                            "prores_ks",
                            "-profile:v",
                            "3",
                            "-c:a",
                            "pcm_s16le",
                        ]);
                    } else {
                        cmd.args(&[
                            "-c:v",
                            "libx264",
                            "-pix_fmt",
                            "yuv420p",
                            "-preset",
                            "medium",
                            "-crf",
                            "23",
                            "-c:a",
                            "aac",
                            "-b:a",
                            "192k",
                        ]);
                    }
                }
                (true, _, FileFormat::Avi) => {
                    cmd.args(&[
                        "-c:v",
                        "mpeg4",
                        "-q:v",
                        "3",
                        "-c:a",
                        "libmp3lame",
                        "-b:a",
                        "192k",
                    ]);
                }
                (true, _, FileFormat::Wmv) => {
                    cmd.args(&[
                        "-c:v",
                        "wmv2",
                        "-b:v",
                        "3000k",
                        "-c:a",
                        "wmav2",
                        "-b:a",
                        "192k",
                    ]);
                }
                (true, _, FileFormat::Flv) => {
                    cmd.args(&[
                        "-c:v",
                        "flv",
                        "-c:a",
                        "libmp3lame",
                        "-b:a",
                        "128k",
                    ]);
                }
                (true, _, FileFormat::Mpeg) => {
                    cmd.args(&[
                        "-c:v",
                        "mpeg2video",
                        "-b:v",
                        "4000k",
                        "-c:a",
                        "mp2",
                        "-b:a",
                        "192k",
                    ]);
                }
                (true, _, FileFormat::M4v) => {
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
                (true, _, FileFormat::Ts) => {
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

                _ => {
                    return Err(ConversionError::UnsupportedConversion {
                        from: from_fmt,
                        to: to_fmt,
                    });
                }
            }
        }

        // Apply resolution scaling if specified
        if let Some(ref res) = options.resolution {
            if res != "original" && !res.is_empty() {
                cmd.args(&["-vf", &format!("scale={}", res)]);
            }
        }

        // Apply custom framerate if specified
        if let Some(fps) = options.frame_rate {
            if fps > 0.0 {
                cmd.args(&["-r", &fps.to_string()]);
            }
        }

        cmd.arg(output);

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        debug!("Executing FFmpeg command: {:?}", cmd);

        let mut child = cmd
            .spawn()
            .map_err(|e| ConversionError::ProcessingError(format!("Failed to spawn ffmpeg: {}", e)))?;

        // Drain stderr in background thread to prevent buffer deadlocks
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
                // Check cancellation request
                if let Some(cancel) = options.cancel_flag.as_ref() {
                    if cancel.load(Ordering::Relaxed) {
                        let _ = child.kill();
                        return Err(ConversionError::Cancelled);
                    }
                }

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
            // Check if process was killed due to user cancellation
            if let Some(cancel) = options.cancel_flag.as_ref() {
                if cancel.load(Ordering::Relaxed) {
                    return Err(ConversionError::Cancelled);
                }
            }

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
