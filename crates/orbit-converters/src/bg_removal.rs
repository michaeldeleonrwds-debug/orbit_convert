use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::info;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Attempts background removal on an image file.
/// Prioritizes the Python AI neural network worker (IS-Net / BiRefNet / u2net with alpha matting).
/// Falls back to external rembg CLI or pure-Rust adaptive alpha-matting if Python is unavailable.
pub fn remove_background(input_path: &Path, output_path: &Path) -> Result<PathBuf, String> {
    remove_background_with_options(input_path, output_path, None, true, None)
}

/// Runs background removal with model selection, alpha-matting controls, and progress streaming.
pub fn remove_background_with_options(
    input_path: &Path,
    output_path: &Path,
    model: Option<&str>,
    alpha_matting: bool,
    progress_cb: Option<std::sync::Arc<dyn Fn(f32, &str) + Send + Sync>>,
) -> Result<PathBuf, String> {
    info!(
        input = %input_path.display(),
        output = %output_path.display(),
        model = ?model,
        alpha_matting = alpha_matting,
        "Starting background removal"
    );

    // 1. Try Python AI neural network worker first
    let py_worker = crate::python_worker::get_python_worker();
    if py_worker.is_available() {
        match py_worker.remove_background(input_path, output_path, model, alpha_matting, progress_cb) {
            Ok(saved) => {
                info!(
                    output = %saved.display(),
                    "Background removal successfully completed via Python AI neural network"
                );
                return Ok(saved);
            }
            Err(e) => {
                tracing::warn!(
                    "Python AI background removal failed ({}), falling back to external CLI / native matting",
                    e
                );
            }
        }
    }

    // 2. Try external AI tool if available (e.g. rembg CLI)
    if try_external_rembg(input_path, output_path) {
        info!("Background removal completed via external rembg tool");
        return Ok(output_path.to_path_buf());
    }

    // 3. Pure Rust native smart matting fallback
    pure_rust_remove_background(input_path, output_path)
}

fn try_external_rembg(input_path: &Path, output_path: &Path) -> bool {
    let mut cmd = Command::new("rembg");
    cmd.arg("i").arg(input_path).arg(output_path);

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    if let Ok(status) = cmd.status() {
        if status.success() && output_path.exists() {
            if let Ok(meta) = std::fs::metadata(output_path) {
                if meta.len() > 0 {
                    return true;
                }
            }
        }
    }
    false
}

/// Pure-Rust adaptive background removal using perimeter color estimation,
/// multi-seed flood-fill with gradient stopping, and boundary alpha feathering.
pub fn pure_rust_remove_background(input_path: &Path, output_path: &Path) -> Result<PathBuf, String> {
    let img = image::open(input_path)
        .map_err(|e| format!("Failed to open image {}: {}", input_path.display(), e))?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    if width == 0 || height == 0 {
        return Err("Image has zero dimensions".into());
    }

    let w = width as usize;
    let h = height as usize;

    // 1. Sample perimeter pixels to determine background reference colors
    let mut perimeter_samples: Vec<[u8; 3]> = Vec::with_capacity(2 * (w + h));
    for x in 0..w {
        let p1 = rgba.get_pixel(x as u32, 0);
        let p2 = rgba.get_pixel(x as u32, (height - 1) as u32);
        perimeter_samples.push([p1[0], p1[1], p1[2]]);
        perimeter_samples.push([p2[0], p2[1], p2[2]]);
    }
    for y in 1..(h.saturating_sub(1)) {
        let p1 = rgba.get_pixel(0, y as u32);
        let p2 = rgba.get_pixel((width - 1) as u32, y as u32);
        perimeter_samples.push([p1[0], p1[1], p1[2]]);
        perimeter_samples.push([p2[0], p2[1], p2[2]]);
    }

    if perimeter_samples.is_empty() {
        return Err("Not enough perimeter pixels to estimate background".into());
    }

    // Compute average perimeter color
    let mut sum_r = 0u64;
    let mut sum_g = 0u64;
    let mut sum_b = 0u64;
    for c in &perimeter_samples {
        sum_r += c[0] as u64;
        sum_g += c[1] as u64;
        sum_b += c[2] as u64;
    }
    let count = perimeter_samples.len() as u64;
    let avg_bg = [
        (sum_r / count) as f32,
        (sum_g / count) as f32,
        (sum_b / count) as f32,
    ];

    // Compute standard deviation of perimeter colors to dynamically adjust tolerance
    let mut variance_sum = 0.0f32;
    for c in &perimeter_samples {
        let dr = c[0] as f32 - avg_bg[0];
        let dg = c[1] as f32 - avg_bg[1];
        let db = c[2] as f32 - avg_bg[2];
        variance_sum += dr * dr + dg * dg + db * db;
    }
    let std_dev = (variance_sum / count as f32).sqrt();
    let tolerance = (std_dev * 1.5 + 32.0).clamp(25.0, 75.0);
    let tol_sq = tolerance * tolerance;

    // Helper: color distance squared from a pixel to the background color
    let color_dist_sq = |px: &[u8; 4]| -> f32 {
        let dr = px[0] as f32 - avg_bg[0];
        let dg = px[1] as f32 - avg_bg[1];
        let db = px[2] as f32 - avg_bg[2];
        dr * dr + dg * dg + db * db
    };

    // 2. Flood-fill from borders
    let mut is_background = vec![false; w * h];
    let mut queue = VecDeque::new();

    let enqueue_if_bg = |x: usize, y: usize, q: &mut VecDeque<(usize, usize)>, is_bg: &mut [bool]| {
        let idx = y * w + x;
        if is_bg[idx] {
            return;
        }
        let px = rgba.get_pixel(x as u32, y as u32);
        // If already transparent or matches background color
        if px[3] < 20 || color_dist_sq(&[px[0], px[1], px[2], px[3]]) <= tol_sq {
            is_bg[idx] = true;
            q.push_back((x, y));
        }
    };

    // Seed top and bottom borders
    for x in 0..w {
        enqueue_if_bg(x, 0, &mut queue, &mut is_background);
        enqueue_if_bg(x, h - 1, &mut queue, &mut is_background);
    }
    // Seed left and right borders
    for y in 0..h {
        enqueue_if_bg(0, y, &mut queue, &mut is_background);
        enqueue_if_bg(w - 1, y, &mut queue, &mut is_background);
    }

    // BFS Expansion
    while let Some((cx, cy)) = queue.pop_front() {
        let neighbors = [
            (cx.wrapping_sub(1), cy),
            (cx + 1, cy),
            (cx, cy.wrapping_sub(1)),
            (cx, cy + 1),
        ];

        for (nx, ny) in neighbors {
            if nx < w && ny < h {
                let nidx = ny * w + nx;
                if !is_background[nidx] {
                    let npx = rgba.get_pixel(nx as u32, ny as u32);
                    if npx[3] < 20 || color_dist_sq(&[npx[0], npx[1], npx[2], npx[3]]) <= tol_sq {
                        is_background[nidx] = true;
                        queue.push_back((nx, ny));
                    }
                }
            }
        }
    }

    // 3. Construct output image with soft feathered alpha along boundary
    let mut output = image::RgbaImage::new(width, height);

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let src_px = rgba.get_pixel(x as u32, y as u32);

            if is_background[idx] {
                // Background -> completely transparent
                output.put_pixel(x as u32, y as u32, image::Rgba([src_px[0], src_px[1], src_px[2], 0]));
            } else {
                // Check if adjacent to background for soft anti-aliased edge
                let mut bg_neighbors = 0;
                let mut total_neighbors = 0;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let nx = x as isize + dx;
                        let ny = y as isize + dy;
                        if nx >= 0 && nx < w as isize && ny >= 0 && ny < h as isize {
                            total_neighbors += 1;
                            if is_background[(ny as usize) * w + (nx as usize)] {
                                bg_neighbors += 1;
                            }
                        }
                    }
                }

                let alpha = if bg_neighbors > 0 {
                    // Soft falloff based on neighborhood ratio
                    let ratio = (total_neighbors - bg_neighbors) as f32 / total_neighbors as f32;
                    let feathered = (ratio * 255.0) as u8;
                    src_px[3].min(feathered)
                } else {
                    src_px[3]
                };

                output.put_pixel(x as u32, y as u32, image::Rgba([src_px[0], src_px[1], src_px[2], alpha]));
            }
        }
    }

    output
        .save(output_path)
        .map_err(|e| format!("Failed to save transparent PNG {}: {}", output_path.display(), e))?;

    info!(
        output = %output_path.display(),
        "Saved background-removed image"
    );

    Ok(output_path.to_path_buf())
}
