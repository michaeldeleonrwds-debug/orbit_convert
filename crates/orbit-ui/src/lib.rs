slint::include_modules!();

use orbit_core::formats::FileFormat;
use slint::{Color, ModelRc, VecModel};

pub fn format_visual_meta(fmt: FileFormat) -> (&'static str, Color, &'static str) {
    match fmt {
        FileFormat::Png => ("IMG", Color::from_rgb_u8(37, 99, 235), "🖼️"),
        FileFormat::Jpg => ("IMG", Color::from_rgb_u8(234, 88, 12), "📷"),
        FileFormat::Webp => ("IMG", Color::from_rgb_u8(16, 185, 129), "🌐"),
        FileFormat::Ico => ("ICO", Color::from_rgb_u8(139, 92, 246), "🔲"),
        FileFormat::Bmp => ("IMG", Color::from_rgb_u8(6, 182, 212), "🎨"),
        FileFormat::Tiff => ("IMG", Color::from_rgb_u8(236, 72, 153), "🗂️"),
        FileFormat::Gif => ("GIF", Color::from_rgb_u8(217, 70, 239), "🎞️"),
        FileFormat::Tga => ("IMG", Color::from_rgb_u8(14, 165, 233), "📐"),
        FileFormat::Ppm => ("IMG", Color::from_rgb_u8(100, 116, 139), "🖨️"),
        FileFormat::Dds => ("IMG", Color::from_rgb_u8(168, 85, 247), "🎮"),
        FileFormat::Pcx => ("IMG", Color::from_rgb_u8(245, 158, 11), "🎨"),
        FileFormat::Dpx => ("IMG", Color::from_rgb_u8(239, 68, 68), "📽️"),

        FileFormat::Avif => ("NEXT", Color::from_rgb_u8(56, 189, 248), "⚡"),
        FileFormat::Heic => ("NEXT", Color::from_rgb_u8(244, 63, 94), "📱"),
        FileFormat::Jxl => ("NEXT", Color::from_rgb_u8(34, 197, 94), "🚀"),

        FileFormat::Svg => ("VEC", Color::from_rgb_u8(245, 158, 11), "✨"),
        FileFormat::Eps => ("VEC", Color::from_rgb_u8(99, 102, 241), "✒️"),
        FileFormat::Art => ("VEC", Color::from_rgb_u8(236, 72, 153), "🎭"),
        FileFormat::Dcs => ("VEC", Color::from_rgb_u8(139, 92, 246), "📑"),
        FileFormat::Wmz => ("VEC", Color::from_rgb_u8(100, 116, 139), "📦"),
        FileFormat::Emz => ("VEC", Color::from_rgb_u8(100, 116, 139), "📦"),

        FileFormat::Cr2
        | FileFormat::Cr3
        | FileFormat::Crw
        | FileFormat::Nef
        | FileFormat::Nrw
        | FileFormat::Arw
        | FileFormat::Sr2
        | FileFormat::Srf
        | FileFormat::Raf
        | FileFormat::Orf
        | FileFormat::Rw2
        | FileFormat::Dng
        | FileFormat::Pef
        | FileFormat::Ptx
        | FileFormat::Rwl
        | FileFormat::Srw
        | FileFormat::X3f
        | FileFormat::Dcr
        | FileFormat::Drf
        | FileFormat::Kdc
        | FileFormat::K25 => ("RAW", Color::from_rgb_u8(249, 115, 22), "📸"),

        FileFormat::Psd | FileFormat::Psb => ("SPEC", Color::from_rgb_u8(59, 130, 246), "🖌️"),
        FileFormat::Djvu => ("DOC", Color::from_rgb_u8(168, 85, 247), "📖"),
        FileFormat::Cbr | FileFormat::Cbz => ("BOOK", Color::from_rgb_u8(234, 88, 12), "📚"),
        FileFormat::Odd => ("SPEC", Color::from_rgb_u8(100, 116, 139), "📄"),

        FileFormat::Mp4 => ("VID", Color::from_rgb_u8(59, 130, 246), "🎬"),
        FileFormat::Mov => ("VID", Color::from_rgb_u8(99, 102, 241), "🎥"),
        FileFormat::Mkv => ("VID", Color::from_rgb_u8(147, 51, 234), "📼"),
        FileFormat::Avi => ("VID", Color::from_rgb_u8(168, 85, 247), "📹"),
        FileFormat::Webm => ("VID", Color::from_rgb_u8(20, 184, 166), "📺"),

        FileFormat::Mp3 => ("AUD", Color::from_rgb_u8(234, 88, 12), "🎵"),
        FileFormat::Wav => ("AUD", Color::from_rgb_u8(245, 158, 11), "🔊"),
        FileFormat::Flac => ("AUD", Color::from_rgb_u8(16, 185, 129), "🎼"),
        FileFormat::Aac => ("AUD", Color::from_rgb_u8(14, 165, 233), "🎧"),
        FileFormat::M4a => ("AUD", Color::from_rgb_u8(99, 102, 241), "🎶"),
        FileFormat::Ogg => ("AUD", Color::from_rgb_u8(244, 63, 94), "🎙️"),

        FileFormat::Csv => ("DATA", Color::from_rgb_u8(16, 185, 129), "📊"),
        FileFormat::Xlsx => ("SHEET", Color::from_rgb_u8(34, 197, 94), "📈"),
        FileFormat::Json => ("DATA", Color::from_rgb_u8(245, 158, 11), "📄"),
        FileFormat::Tsv => ("DATA", Color::from_rgb_u8(20, 184, 166), "📋"),

        FileFormat::Pdf => ("DOC", Color::from_rgb_u8(239, 68, 68), "📕"),
        FileFormat::Docx => ("DOC", Color::from_rgb_u8(59, 130, 246), "📘"),
        FileFormat::Txt => ("DOC", Color::from_rgb_u8(148, 163, 184), "📝"),
        FileFormat::Html => ("WEB", Color::from_rgb_u8(249, 115, 22), "🌐"),
    }
}

/// Helper to generate annular SVG donut wedge path commands
fn build_annular_wedge_path(
    cx: f32,
    cy: f32,
    inner_r: f32,
    outer_r: f32,
    center_angle: f32,
    half_step: f32,
    gap: f32,
) -> (String, f32, f32) {
    let a1 = center_angle - half_step + gap;
    let a2 = center_angle + half_step - gap;

    let x1 = cx + outer_r * a1.cos();
    let y1 = cy + outer_r * a1.sin();

    let x2 = cx + outer_r * a2.cos();
    let y2 = cy + outer_r * a2.sin();

    let x3 = cx + inner_r * a2.cos();
    let y3 = cy + inner_r * a2.sin();

    let x4 = cx + inner_r * a1.cos();
    let y4 = cy + inner_r * a1.sin();

    let large = if (a2 - a1) > std::f32::consts::PI { 1 } else { 0 };

    let path_commands = format!(
        "M {:.2} {:.2} A {:.2} {:.2} 0 {} 1 {:.2} {:.2} L {:.2} {:.2} A {:.2} {:.2} 0 {} 0 {:.2} {:.2} Z",
        x1, y1, outer_r, outer_r, large, x2, y2, x3, y3, inner_r, inner_r, large, x4, y4
    );

    let mid_r = (inner_r + outer_r) / 2.0;
    let mid_x = cx + mid_r * center_angle.cos();
    let mid_y = cy + mid_r * center_angle.sin();

    (path_commands, mid_x, mid_y)
}

/// Generates radial sectors for a two-level concentric radial wheel.
/// - Level 1 (Inner Ring): Common formats + "More" expansion sector.
/// - Level 2 (Outer Ring): Extended destination formats when expanded.
pub fn build_two_level_wheel_sectors(
    cx: f32,
    cy: f32,
    inner_formats: &[FileFormat],
    has_more: bool,
    outer_formats: &[FileFormat],
) -> (
    ModelRc<WheelSector>,
    ModelRc<WheelSector>,
    Vec<FileFormat>,
    Vec<FileFormat>,
) {
    // -------------------------------------------------------------
    // Inner Ring: Radius 75px - 150px
    // -------------------------------------------------------------
    let inner_count = inner_formats.len() + if has_more { 1 } else { 0 };
    let mut inner_list = Vec::with_capacity(inner_count);

    if inner_count > 0 {
        let step = 2.0 * std::f32::consts::PI / inner_count as f32;
        let half_step = step / 2.0;
        let start_angle = -std::f32::consts::FRAC_PI_2; // 12 o'clock
        let gap = if inner_count <= 3 { 0.015 } else { 0.018 };

        for (i, &fmt) in inner_formats.iter().enumerate() {
            let angle = start_angle + i as f32 * step;
            let (path, mx, my) = build_annular_wedge_path(cx, cy, 75.0, 150.0, angle, half_step, gap);
            let (cat, color, icon) = format_visual_meta(fmt);

            inner_list.push(WheelSector {
                label: fmt.display_label().into(),
                extension: fmt.primary_extension().into(),
                category: cat.into(),
                color_hex: color,
                pos_x: mx,
                pos_y: my,
                angle_deg: (angle.to_degrees() + 360.0) % 360.0,
                index: i as i32,
                path_commands: path.into(),
                icon_symbol: icon.into(),
                is_outer: false,
                is_more: false,
            });
        }

        if has_more {
            let more_idx = inner_formats.len();
            let angle = start_angle + more_idx as f32 * step;
            let (path, mx, my) = build_annular_wedge_path(cx, cy, 75.0, 150.0, angle, half_step, gap);

            inner_list.push(WheelSector {
                label: "More".into(),
                extension: "...".into(),
                category: "EXP".into(),
                color_hex: Color::from_rgb_u8(59, 130, 246),
                pos_x: mx,
                pos_y: my,
                angle_deg: (angle.to_degrees() + 360.0) % 360.0,
                index: more_idx as i32,
                path_commands: path.into(),
                icon_symbol: "➕".into(),
                is_outer: false,
                is_more: true,
            });
        }
    }

    // -------------------------------------------------------------
    // Outer Ring: Radius 154px - 220px
    // -------------------------------------------------------------
    let outer_count = outer_formats.len();
    let mut outer_list = Vec::with_capacity(outer_count);

    if outer_count > 0 {
        let step = 2.0 * std::f32::consts::PI / outer_count as f32;
        let half_step = step / 2.0;
        let start_angle = -std::f32::consts::FRAC_PI_2; // 12 o'clock
        let gap = 0.015;

        for (i, &fmt) in outer_formats.iter().enumerate() {
            let angle = start_angle + i as f32 * step;
            let (path, mx, my) = build_annular_wedge_path(cx, cy, 154.0, 220.0, angle, half_step, gap);
            let (cat, color, icon) = format_visual_meta(fmt);

            outer_list.push(WheelSector {
                label: fmt.display_label().into(),
                extension: fmt.primary_extension().into(),
                category: cat.into(),
                color_hex: color,
                pos_x: mx,
                pos_y: my,
                angle_deg: (angle.to_degrees() + 360.0) % 360.0,
                index: i as i32,
                path_commands: path.into(),
                icon_symbol: icon.into(),
                is_outer: true,
                is_more: false,
            });
        }
    }

    (
        ModelRc::new(VecModel::from(inner_list)),
        ModelRc::new(VecModel::from(outer_list)),
        inner_formats.to_vec(),
        outer_formats.to_vec(),
    )
}

/// Backward compatibility wrapper for single-level wheel calls
pub fn build_wheel_sectors(
    center_x: f32,
    center_y: f32,
    _radius: f32,
    formats: &[FileFormat],
) -> (ModelRc<WheelSector>, Vec<FileFormat>) {
    let (inner, _, inner_fmts, _) =
        build_two_level_wheel_sectors(center_x, center_y, formats, false, &[]);
    (inner, inner_fmts)
}
