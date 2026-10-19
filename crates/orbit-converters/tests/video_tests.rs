use orbit_converters::ffmpeg::FfmpegConverter;
use orbit_converters::register_all_converters;
use orbit_core::formats::FileFormat;
use orbit_core::probe::probe_media;
use orbit_core::registry::{ConversionOptions, ConversionRegistry};
use std::process::Command;
use tempfile::tempdir;

fn generate_test_media(
    dir: &std::path::Path,
    filename: &str,
    has_audio: bool,
    vcodec: &str,
    acodec: &str,
) -> std::path::PathBuf {
    let out_path = dir.join(filename);
    let mut cmd = Command::new("ffmpeg");
    cmd.args(&[
        "-y",
        "-f",
        "lavfi",
        "-i",
        "testsrc=duration=1:size=320x240:rate=15",
    ]);

    if has_audio {
        cmd.args(&[
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=1000:duration=1",
            "-c:v",
            vcodec,
            "-c:a",
            acodec,
        ]);
    } else {
        cmd.args(&["-c:v", vcodec, "-an"]);
    }

    cmd.arg(&out_path);
    let output = cmd.output().expect("Failed to execute ffmpeg to generate test media");
    assert!(
        output.status.success(),
        "FFmpeg failed to create test fixture {}: {}",
        filename,
        String::from_utf8_lossy(&output.stderr)
    );
    out_path
}

#[test]
fn test_video_conversions_and_probe_verification() {
    if !FfmpegConverter::check_ffmpeg_available() {
        eprintln!("FFmpeg not installed; skipping video conversion integration test");
        return;
    }

    let mut reg = ConversionRegistry::new();
    register_all_converters(&mut reg);
    let options = ConversionOptions::default();

    let dir = tempdir().unwrap();

    // Generate input test clips
    let mp4_path = generate_test_media(dir.path(), "input.mp4", true, "libx264", "aac");
    let mov_path = generate_test_media(dir.path(), "input.mov", true, "libx264", "aac");
    let mkv_path = generate_test_media(dir.path(), "input.mkv", true, "libx264", "aac");
    let avi_path = generate_test_media(dir.path(), "input.avi", true, "mpeg4", "mp3");

    // 1. MP4 -> MP3 (Audio extraction)
    {
        let out_path = dir.path().join("output.mp3");
        let converter = reg
            .find_converter(FileFormat::Mp4, FileFormat::Mp3)
            .expect("MP4 -> MP3 converter not found");
        converter
            .convert(&mp4_path, &out_path, &options, None)
            .expect("MP4 -> MP3 conversion failed");
        assert!(out_path.exists());

        let probe = probe_media(&out_path).expect("FFprobe failed on output.mp3");
        assert!(probe.has_audio, "output.mp3 should have audio");
        assert!(!probe.has_video, "output.mp3 must not contain video stream");
        let codec = probe.audio_codec.unwrap_or_default().to_lowercase();
        assert!(
            codec.contains("mp3"),
            "Expected MP3 audio codec, got: {}",
            codec
        );
    }

    // 2. MOV -> MP4
    {
        let out_path = dir.path().join("output.mp4");
        let converter = reg
            .find_converter(FileFormat::Mov, FileFormat::Mp4)
            .expect("MOV -> MP4 converter not found");
        converter
            .convert(&mov_path, &out_path, &options, None)
            .expect("MOV -> MP4 conversion failed");
        assert!(out_path.exists());

        let probe = probe_media(&out_path).expect("FFprobe failed on output.mp4");
        assert!(probe.has_video, "output.mp4 should have video");
        assert_eq!(probe.width, Some(320));
        assert_eq!(probe.height, Some(240));
    }

    // 3. MKV -> WebM
    {
        let out_path = dir.path().join("output.webm");
        let converter = reg
            .find_converter(FileFormat::Mkv, FileFormat::Webm)
            .expect("MKV -> WebM converter not found");
        converter
            .convert(&mkv_path, &out_path, &options, None)
            .expect("MKV -> WebM conversion failed");
        assert!(out_path.exists());

        let probe = probe_media(&out_path).expect("FFprobe failed on output.webm");
        assert!(probe.has_video, "output.webm should have video");
        assert!(probe.has_audio, "output.webm should have audio");
    }

    // 4. MP4 -> GIF (High quality palettegen)
    {
        let out_path = dir.path().join("output.gif");
        let converter = reg
            .find_converter(FileFormat::Mp4, FileFormat::Gif)
            .expect("MP4 -> GIF converter not found");
        converter
            .convert(&mp4_path, &out_path, &options, None)
            .expect("MP4 -> GIF conversion failed");
        assert!(out_path.exists());
        assert!(
            std::fs::metadata(&out_path).unwrap().len() > 100,
            "output.gif must not be empty"
        );
    }

    // 5. MP4 -> WAV (Lossless master extraction)
    {
        let out_path = dir.path().join("output.wav");
        let converter = reg
            .find_converter(FileFormat::Mp4, FileFormat::Wav)
            .expect("MP4 -> WAV converter not found");
        converter
            .convert(&mp4_path, &out_path, &options, None)
            .expect("MP4 -> WAV conversion failed");
        assert!(out_path.exists());

        let probe = probe_media(&out_path).expect("FFprobe failed on output.wav");
        assert!(probe.has_audio, "output.wav should have audio");
        assert!(!probe.has_video, "output.wav must not contain video stream");
        let codec = probe.audio_codec.unwrap_or_default().to_lowercase();
        assert!(
            codec.contains("pcm"),
            "Expected PCM audio codec for WAV, got: {}",
            codec
        );
    }

    // 6. AVI -> MP4
    {
        let out_path = dir.path().join("output_avi_to_mp4.mp4");
        let converter = reg
            .find_converter(FileFormat::Avi, FileFormat::Mp4)
            .expect("AVI -> MP4 converter not found");
        converter
            .convert(&avi_path, &out_path, &options, None)
            .expect("AVI -> MP4 conversion failed");
        assert!(out_path.exists());

        let probe = probe_media(&out_path).expect("FFprobe failed on output_avi_to_mp4.mp4");
        assert!(probe.has_video, "Converted MP4 should have video");
        assert!(probe.has_audio, "Converted MP4 should have audio");
    }

    // 7. Verify no audio extraction targets if video has no audio stream
    {
        let silent_mp4 = generate_test_media(dir.path(), "silent.mp4", false, "libx264", "aac");
        let targets = reg.get_available_targets_for_file(FileFormat::Mp4, Some(&silent_mp4));

        assert!(targets.contains(&FileFormat::Webm));
        assert!(targets.contains(&FileFormat::Mkv));
        assert!(targets.contains(&FileFormat::Gif));

        // Audio-only targets MUST be filtered out
        assert!(
            !targets.contains(&FileFormat::Mp3),
            "Silent MP4 must not offer MP3 extraction"
        );
        assert!(
            !targets.contains(&FileFormat::Wav),
            "Silent MP4 must not offer WAV extraction"
        );
        assert!(
            !targets.contains(&FileFormat::Flac),
            "Silent MP4 must not offer FLAC extraction"
        );
    }
}
