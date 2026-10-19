use orbit_core::actions::QuickAction;
use orbit_core::formats::FileFormat;
use orbit_core::jobs::JobManager;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use tracing::{error, info};

const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Copies plain text directly to the Windows clipboard via clip.exe
pub fn copy_text_to_clipboard(text: &str) {
    use std::io::Write;
    if let Ok(mut child) = Command::new("clip")
        .stdin(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
    {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = child.wait();
    }
}

/// Computes SHA-256 hash formatted as lowercase hex string
pub fn compute_file_sha256(path: &Path) -> Result<String, String> {
    use std::fs::File;
    use std::io::Read;

    let mut file = File::open(path).map_err(|e| format!("Could not open file: {}", e))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .map_err(|e| format!("Could not read file: {}", e))?;

    // Self-contained, standard SHA-256 implementation
    let hash = sha256_digest(&buffer);
    Ok(hash)
}

fn sha256_digest(data: &[u8]) -> String {
    // Standard SHA-256 constants
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for (i, w_val) in w.iter_mut().take(16).enumerate() {
            *w_val = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut h_val = h[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h_val
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h_val = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(h_val);
    }

    format!(
        "{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}",
        h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]
    )
}

/// Executes a contextual Quick Action on a target file.
pub fn execute_quick_action(
    action: QuickAction,
    source_path: &Path,
    source_format: FileFormat,
    job_manager: &JobManager,
    show_notifications: bool,
) {
    info!(
        action = ?action,
        path = %source_path.display(),
        format = %source_format,
        "Executing Quick Action"
    );

    let fname = source_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());
    let stem = source_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());
    let parent = source_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    match action {
        QuickAction::CopyPath => {
            let path_str = source_path.display().to_string();
            copy_text_to_clipboard(&path_str);
            if show_notifications {
                crate::tray::send_global_notification(
                    "Path Copied to Clipboard",
                    &format!("Copied: {}", path_str),
                    false,
                );
            }
        }

        QuickAction::ShowInExplorer => {
            let _ = Command::new("explorer")
                .arg(format!("/select,{}", source_path.display()))
                .creation_flags(CREATE_NO_WINDOW)
                .spawn();
        }

        QuickAction::CalculateHash => {
            let p = source_path.to_path_buf();
            let fname_clone = fname.clone();
            std::thread::spawn(move || match compute_file_sha256(&p) {
                Ok(hash) => {
                    copy_text_to_clipboard(&hash);
                    let preview = if hash.len() > 16 {
                        format!("{}...", &hash[..16])
                    } else {
                        hash.clone()
                    };
                    if show_notifications {
                        crate::tray::send_global_notification(
                            "SHA-256 Calculated",
                            &format!("{}: {}\n(Copied full hash to clipboard)", fname_clone, preview),
                            false,
                        );
                    }
                }
                Err(e) => {
                    error!("SHA-256 computation failed: {}", e);
                    if show_notifications {
                        crate::tray::send_global_notification(
                            "Hash Failed",
                            &format!("Failed to calculate hash: {}", e),
                            true,
                        );
                    }
                }
            });
        }

        QuickAction::ConvertToIcon => {
            let _ = job_manager.submit(
                source_path.to_path_buf(),
                source_format,
                FileFormat::Ico,
                None,
            );
        }

        QuickAction::ExtractAudio => {
            let _ = job_manager.submit(
                source_path.to_path_buf(),
                source_format,
                FileFormat::Mp3,
                None,
            );
        }

        QuickAction::ConvertToPdf => {
            let _ = job_manager.submit(
                source_path.to_path_buf(),
                source_format,
                FileFormat::Pdf,
                None,
            );
        }

        QuickAction::ConvertToGlb => {
            let _ = job_manager.submit(
                source_path.to_path_buf(),
                source_format,
                FileFormat::Glb,
                None,
            );
        }

        QuickAction::ConvertToStl => {
            let _ = job_manager.submit(
                source_path.to_path_buf(),
                source_format,
                FileFormat::Stl,
                None,
            );
        }

        QuickAction::RotateImage => {
            let src = source_path.to_path_buf();
            let ext = source_format.primary_extension();
            let out = parent.join(format!("{}_rotated.{}", stem, ext));
            std::thread::spawn(move || {
                if let Ok(img) = image::open(&src) {
                    let rotated = img.rotate90();
                    if rotated.save(&out).is_ok() {
                        if show_notifications {
                            crate::tray::send_global_notification(
                                "Image Rotated",
                                &format!("Saved 90° rotated image:\n{}", out.display()),
                                false,
                            );
                        }
                    } else {
                        error!("Failed to save rotated image to {}", out.display());
                    }
                } else {
                    error!("Failed to open image for rotation: {}", src.display());
                }
            });
        }

        QuickAction::RemoveAudio => {
            let src = source_path.to_path_buf();
            let ext = source_format.primary_extension();
            let out = parent.join(format!("{}_muted.{}", stem, ext));
            std::thread::spawn(move || {
                let status = Command::new("ffmpeg")
                    .args(&["-y", "-i"])
                    .arg(&src)
                    .args(&["-c:v", "copy", "-an"])
                    .arg(&out)
                    .creation_flags(CREATE_NO_WINDOW)
                    .status();
                if let Ok(s) = status {
                    if s.success() && show_notifications {
                        crate::tray::send_global_notification(
                            "Audio Removed",
                            &format!("Muted video created:\n{}", out.display()),
                            false,
                        );
                    }
                }
            });
        }

        QuickAction::GenerateThumbnail => {
            let src = source_path.to_path_buf();
            let out = parent.join(format!("{}_thumb.png", stem));
            std::thread::spawn(move || {
                let status = Command::new("ffmpeg")
                    .args(&["-y", "-ss", "00:00:01", "-i"])
                    .arg(&src)
                    .args(&["-vframes", "1"])
                    .arg(&out)
                    .creation_flags(CREATE_NO_WINDOW)
                    .status();
                if let Ok(s) = status {
                    if s.success() && show_notifications {
                        crate::tray::send_global_notification(
                            "Thumbnail Generated",
                            &format!("Thumbnail snapshot saved:\n{}", out.display()),
                            false,
                        );
                    }
                }
            });
        }

        QuickAction::NormalizeVolume => {
            let src = source_path.to_path_buf();
            let ext = source_format.primary_extension();
            let out = parent.join(format!("{}_normalized.{}", stem, ext));
            std::thread::spawn(move || {
                let status = Command::new("ffmpeg")
                    .args(&["-y", "-i"])
                    .arg(&src)
                    .args(&["-filter:a", "loudnorm"])
                    .arg(&out)
                    .creation_flags(CREATE_NO_WINDOW)
                    .status();
                if let Ok(s) = status {
                    if s.success() && show_notifications {
                        crate::tray::send_global_notification(
                            "Volume Normalized",
                            &format!("EBU R128 balanced audio saved:\n{}", out.display()),
                            false,
                        );
                    }
                }
            });
        }

        QuickAction::ConvertToMono => {
            let src = source_path.to_path_buf();
            let ext = source_format.primary_extension();
            let out = parent.join(format!("{}_mono.{}", stem, ext));
            std::thread::spawn(move || {
                let status = Command::new("ffmpeg")
                    .args(&["-y", "-i"])
                    .arg(&src)
                    .args(&["-ac", "1"])
                    .arg(&out)
                    .creation_flags(CREATE_NO_WINDOW)
                    .status();
                if let Ok(s) = status {
                    if s.success() && show_notifications {
                        crate::tray::send_global_notification(
                            "Converted to Mono",
                            &format!("Mono audio saved:\n{}", out.display()),
                            false,
                        );
                    }
                }
            });
        }

        QuickAction::ConvertToStereo => {
            let src = source_path.to_path_buf();
            let ext = source_format.primary_extension();
            let out = parent.join(format!("{}_stereo.{}", stem, ext));
            std::thread::spawn(move || {
                let status = Command::new("ffmpeg")
                    .args(&["-y", "-i"])
                    .arg(&src)
                    .args(&["-ac", "2"])
                    .arg(&out)
                    .creation_flags(CREATE_NO_WINDOW)
                    .status();
                if let Ok(s) = status {
                    if s.success() && show_notifications {
                        crate::tray::send_global_notification(
                            "Converted to Stereo",
                            &format!("Stereo audio saved:\n{}", out.display()),
                            false,
                        );
                    }
                }
            });
        }

        QuickAction::CompressVideo => {
            let src = source_path.to_path_buf();
            let ext = source_format.primary_extension();
            let out = parent.join(format!("{}_compressed.{}", stem, ext));
            std::thread::spawn(move || {
                let status = Command::new("ffmpeg")
                    .args(&["-y", "-i"])
                    .arg(&src)
                    .args(&[
                        "-c:v", "libx264", "-crf", "28", "-preset", "medium", "-c:a", "aac",
                        "-b:a", "128k",
                    ])
                    .arg(&out)
                    .creation_flags(CREATE_NO_WINDOW)
                    .status();
                if let Ok(s) = status {
                    if s.success() && show_notifications {
                        crate::tray::send_global_notification(
                            "Video Compressed",
                            &format!("Compressed video saved:\n{}", out.display()),
                            false,
                        );
                    }
                }
            });
        }

        QuickAction::ExtractHere => {
            let src = source_path.to_path_buf();
            let target_dir = parent.clone();
            std::thread::spawn(move || {
                // Try tar.exe (included in modern Windows 10/11) to extract zip/tar/etc.
                let status = Command::new("tar")
                    .args(&["-xf"])
                    .arg(&src)
                    .arg("-C")
                    .arg(&target_dir)
                    .creation_flags(CREATE_NO_WINDOW)
                    .status();
                if let Ok(s) = status {
                    if s.success() && show_notifications {
                        crate::tray::send_global_notification(
                            "Archive Extracted",
                            &format!("Extracted files into:\n{}", target_dir.display()),
                            false,
                        );
                    }
                }
            });
        }

        QuickAction::ConvertToZip => {
            let _src = source_path.to_path_buf();
            let out = parent.join(format!("{}.zip", stem));
            std::thread::spawn(move || {
                let status = Command::new("tar")
                    .args(&["-acf"])
                    .arg(&out)
                    .arg(&fname)
                    .current_dir(&parent)
                    .creation_flags(CREATE_NO_WINDOW)
                    .status();
                if let Ok(s) = status {
                    if s.success() && show_notifications {
                        crate::tray::send_global_notification(
                            "ZIP Created",
                            &format!("Created archive:\n{}", out.display()),
                            false,
                        );
                    }
                }
            });
        }

        QuickAction::RemoveBackground => {
            let src = source_path.to_path_buf();
            let out = parent.join(format!("{}_nobg.png", stem));
            std::thread::spawn(move || {
                match orbit_converters::bg_removal::remove_background(&src, &out) {
                    Ok(saved) => {
                        if show_notifications {
                            crate::tray::send_global_notification(
                                "Background Removed",
                                &format!("Transparent PNG created:\n{}", saved.display()),
                                false,
                            );
                        }
                    }
                    Err(e) => {
                        error!("Background removal failed: {}", e);
                        if show_notifications {
                            crate::tray::send_global_notification(
                                "Background Removal Failed",
                                &format!("{}", e),
                                true,
                            );
                        }
                    }
                }
            });
        }

        QuickAction::ResizeImage => {
            let src = source_path.to_path_buf();
            let ext = source_format.primary_extension();
            let out = parent.join(format!("{}_resized.{}", stem, ext));
            std::thread::spawn(move || {
                if let Ok(img) = image::open(&src) {
                    let (w, h) = (img.width(), img.height());
                    let target_w = (w / 2).max(1);
                    let target_h = (h / 2).max(1);
                    let resized = img.resize(target_w, target_h, image::imageops::FilterType::Lanczos3);
                    if resized.save(&out).is_ok() {
                        if show_notifications {
                            crate::tray::send_global_notification(
                                "Image Resized",
                                &format!("Resized (50%) saved:\n{}", out.display()),
                                false,
                            );
                        }
                    }
                }
            });
        }

        QuickAction::CompressImage | QuickAction::OptimizeImage => {
            let src = source_path.to_path_buf();
            let ext = source_format.primary_extension();
            let out = parent.join(format!("{}_opt.{}", stem, ext));
            std::thread::spawn(move || {
                if let Ok(img) = image::open(&src) {
                    if img.save(&out).is_ok() {
                        if show_notifications {
                            crate::tray::send_global_notification(
                                "Image Optimized",
                                &format!("Optimized image saved:\n{}", out.display()),
                                false,
                            );
                        }
                    }
                }
            });
        }

        QuickAction::UpscaleImage => {
            let src = source_path.to_path_buf();
            let ext = source_format.primary_extension();
            let out = parent.join(format!("{}_2x.{}", stem, ext));
            std::thread::spawn(move || {
                if let Ok(img) = image::open(&src) {
                    let (w, h) = (img.width(), img.height());
                    let upscaled = img.resize(w * 2, h * 2, image::imageops::FilterType::Lanczos3);
                    if upscaled.save(&out).is_ok() {
                        if show_notifications {
                            crate::tray::send_global_notification(
                                "Image Upscaled 2x",
                                &format!("2x Lanczos upscaled image saved:\n{}", out.display()),
                                false,
                            );
                        }
                    }
                }
            });
        }

        QuickAction::CropImage => {
            let src = source_path.to_path_buf();
            let ext = source_format.primary_extension();
            let out = parent.join(format!("{}_cropped.{}", stem, ext));
            std::thread::spawn(move || {
                if let Ok(img) = image::open(&src) {
                    let rgba = img.to_rgba8();
                    let (w, h) = rgba.dimensions();
                    let mut min_x = w;
                    let mut max_x = 0;
                    let mut min_y = h;
                    let mut max_y = 0;
                    for y in 0..h {
                        for x in 0..w {
                            let p = rgba.get_pixel(x, y);
                            if p[3] > 10 && (p[0] < 250 || p[1] < 250 || p[2] < 250) {
                                if x < min_x { min_x = x; }
                                if x > max_x { max_x = x; }
                                if y < min_y { min_y = y; }
                                if y > max_y { max_y = y; }
                            }
                        }
                    }
                    if min_x < max_x && min_y < max_y {
                        let cropped = image::imageops::crop_imm(&rgba, min_x, min_y, max_x - min_x + 1, max_y - min_y + 1);
                        if cropped.to_image().save(&out).is_ok() {
                            if show_notifications {
                                crate::tray::send_global_notification(
                                    "Image Auto-Cropped",
                                    &format!("Cropped image saved:\n{}", out.display()),
                                    false,
                                );
                            }
                            return;
                        }
                    }
                    // Fallback save
                    let _ = img.save(&out);
                }
            });
        }

        QuickAction::RemoveMetadata => {
            let src = source_path.to_path_buf();
            let ext = source_format.primary_extension();
            let out = parent.join(format!("{}_clean.{}", stem, ext));
            std::thread::spawn(move || {
                if let Ok(img) = image::open(&src) {
                    if img.save(&out).is_ok() {
                        if show_notifications {
                            crate::tray::send_global_notification(
                                "Metadata Stripped",
                                &format!("Clean image without EXIF saved:\n{}", out.display()),
                                false,
                            );
                        }
                    }
                }
            });
        }

        QuickAction::TrimVideo => {
            let src = source_path.to_path_buf();
            let ext = source_format.primary_extension();
            let out = parent.join(format!("{}_trimmed.{}", stem, ext));
            std::thread::spawn(move || {
                let status = Command::new("ffmpeg")
                    .args(&["-y", "-ss", "00:00:00", "-to", "00:00:30", "-i"])
                    .arg(&src)
                    .args(&["-c", "copy"])
                    .arg(&out)
                    .creation_flags(CREATE_NO_WINDOW)
                    .status();
                if let Ok(s) = status {
                    if s.success() && show_notifications {
                        crate::tray::send_global_notification(
                            "Video Trimmed",
                            &format!("Trimmed video saved:\n{}", out.display()),
                            false,
                        );
                    }
                }
            });
        }

        QuickAction::ResizeVideo => {
            let src = source_path.to_path_buf();
            let ext = source_format.primary_extension();
            let out = parent.join(format!("{}_720p.{}", stem, ext));
            std::thread::spawn(move || {
                let status = Command::new("ffmpeg")
                    .args(&["-y", "-i"])
                    .arg(&src)
                    .args(&["-vf", "scale=-2:720", "-c:v", "libx264", "-crf", "23", "-c:a", "copy"])
                    .arg(&out)
                    .creation_flags(CREATE_NO_WINDOW)
                    .status();
                if let Ok(s) = status {
                    if s.success() && show_notifications {
                        crate::tray::send_global_notification(
                            "Video Resized",
                            &format!("720p video saved:\n{}", out.display()),
                            false,
                        );
                    }
                }
            });
        }

        QuickAction::ChangeFps => {
            let src = source_path.to_path_buf();
            let ext = source_format.primary_extension();
            let out = parent.join(format!("{}_30fps.{}", stem, ext));
            std::thread::spawn(move || {
                let status = Command::new("ffmpeg")
                    .args(&["-y", "-i"])
                    .arg(&src)
                    .args(&["-filter:v", "fps=30", "-c:a", "copy"])
                    .arg(&out)
                    .creation_flags(CREATE_NO_WINDOW)
                    .status();
                if let Ok(s) = status {
                    if s.success() && show_notifications {
                        crate::tray::send_global_notification(
                            "Video FPS Changed",
                            &format!("30 FPS video saved:\n{}", out.display()),
                            false,
                        );
                    }
                }
            });
        }

        QuickAction::ExtractFrames => {
            let src = source_path.to_path_buf();
            let frames_dir = parent.join(format!("{}_frames", stem));
            std::thread::spawn(move || {
                let _ = std::fs::create_dir_all(&frames_dir);
                let out_pattern = frames_dir.join("frame_%04d.png");
                let status = Command::new("ffmpeg")
                    .args(&["-y", "-i"])
                    .arg(&src)
                    .args(&["-vf", "fps=1"])
                    .arg(&out_pattern)
                    .creation_flags(CREATE_NO_WINDOW)
                    .status();
                if let Ok(s) = status {
                    if s.success() && show_notifications {
                        crate::tray::send_global_notification(
                            "Frames Extracted",
                            &format!("PNG frames extracted into:\n{}", frames_dir.display()),
                            false,
                        );
                    }
                }
            });
        }

        QuickAction::TrimAudio => {
            let src = source_path.to_path_buf();
            let ext = source_format.primary_extension();
            let out = parent.join(format!("{}_trimmed.{}", stem, ext));
            std::thread::spawn(move || {
                let status = Command::new("ffmpeg")
                    .args(&["-y", "-ss", "00:00:00", "-t", "30", "-i"])
                    .arg(&src)
                    .args(&["-c", "copy"])
                    .arg(&out)
                    .creation_flags(CREATE_NO_WINDOW)
                    .status();
                if let Ok(s) = status {
                    if s.success() && show_notifications {
                        crate::tray::send_global_notification(
                            "Audio Trimmed",
                            &format!("First 30 seconds saved:\n{}", out.display()),
                            false,
                        );
                    }
                }
            });
        }

        QuickAction::CompressAudio => {
            let src = source_path.to_path_buf();
            let out = parent.join(format!("{}_128k.mp3", stem));
            std::thread::spawn(move || {
                let status = Command::new("ffmpeg")
                    .args(&["-y", "-i"])
                    .arg(&src)
                    .args(&["-c:a", "libmp3lame", "-b:a", "128k"])
                    .arg(&out)
                    .creation_flags(CREATE_NO_WINDOW)
                    .status();
                if let Ok(s) = status {
                    if s.success() && show_notifications {
                        crate::tray::send_global_notification(
                            "Audio Compressed",
                            &format!("128 kbps MP3 saved:\n{}", out.display()),
                            false,
                        );
                    }
                }
            });
        }

        QuickAction::ChangeSampleRate => {
            let src = source_path.to_path_buf();
            let ext = source_format.primary_extension();
            let out = parent.join(format!("{}_44k.{}", stem, ext));
            std::thread::spawn(move || {
                let status = Command::new("ffmpeg")
                    .args(&["-y", "-i"])
                    .arg(&src)
                    .args(&["-ar", "44100"])
                    .arg(&out)
                    .creation_flags(CREATE_NO_WINDOW)
                    .status();
                if let Ok(s) = status {
                    if s.success() && show_notifications {
                        crate::tray::send_global_notification(
                            "Sample Rate Changed",
                            &format!("44.1 kHz audio saved:\n{}", out.display()),
                            false,
                        );
                    }
                }
            });
        }

        QuickAction::ExtractText => {
            let src = source_path.to_path_buf();
            let out = parent.join(format!("{}_text.txt", stem));
            std::thread::spawn(move || {
                let text_opt = if src.extension().map_or(false, |e| e == "docx") {
                    orbit_converters::documents::DocumentConverter::extract_docx_text(&src).ok()
                } else if src.extension().map_or(false, |e| e == "pdf") {
                    orbit_converters::documents::DocumentConverter::extract_pdf_text(&src).ok()
                } else if let Ok(s) = std::fs::read_to_string(&src) {
                    Some(s)
                } else {
                    None
                };

                if let Some(txt) = text_opt {
                    if std::fs::write(&out, txt).is_ok() {
                        if show_notifications {
                            crate::tray::send_global_notification(
                                "Text Extracted",
                                &format!("Text content saved to:\n{}", out.display()),
                                false,
                            );
                        }
                    }
                }
            });
        }

        QuickAction::ListContents => {
            let src = source_path.to_path_buf();
            std::thread::spawn(move || {
                let output = Command::new("tar")
                    .args(&["-tf"])
                    .arg(&src)
                    .creation_flags(CREATE_NO_WINDOW)
                    .output();
                if let Ok(out) = output {
                    let list = String::from_utf8_lossy(&out.stdout).to_string();
                    copy_text_to_clipboard(&list);
                    if show_notifications {
                        let lines = list.lines().take(5).collect::<Vec<_>>().join("\n");
                        crate::tray::send_global_notification(
                            "Archive Contents Copied",
                            &format!("List copied to clipboard:\n{}", lines),
                            false,
                        );
                    }
                }
            });
        }

        other => {
            if show_notifications {
                crate::tray::send_global_notification(
                    other.label(),
                    &format!("{}: {} initiated for {}", other.label(), other.subtitle(), fname),
                    false,
                );
            }
        }
    }
}
