slint::include_modules!();

use orbit_core::formats::FileFormat;
use slint::{Color, ModelRc, VecModel};

/// Generates radial sector metadata for display on the wheel.
/// Calculates the annular donut wedge SVG path commands, midpoint coordinates,
/// and distinctive colors for each format.
pub fn build_wheel_sectors(
    center_x: f32,
    center_y: f32,
    radius: f32,
    formats: &[FileFormat],
) -> (ModelRc<WheelSector>, Vec<FileFormat>) {
    let count = formats.len();
    let mut sector_list = Vec::with_capacity(count);

    if count == 0 {
        return (ModelRc::new(VecModel::from(sector_list)), Vec::new());
    }

    let outer_radius = radius; // 170.0 px
    let inner_radius = 72.5;   // 145.0 px diameter center hub

    let step = 2.0 * std::f32::consts::PI / count as f32;
    let half_step = step / 2.0;
    let start_angle = -std::f32::consts::FRAC_PI_2; // Start at 12 o'clock

    // Angular gap between wedges in radians
    let gap = if count <= 3 { 0.015 } else { 0.022 };

    for (i, &fmt) in formats.iter().enumerate() {
        let center_angle = start_angle + i as f32 * step;
        let a1 = center_angle - half_step + gap;
        let a2 = center_angle + half_step - gap;

        let x1 = center_x + outer_radius * a1.cos();
        let y1 = center_y + outer_radius * a1.sin();

        let x2 = center_x + outer_radius * a2.cos();
        let y2 = center_y + outer_radius * a2.sin();

        let x3 = center_x + inner_radius * a2.cos();
        let y3 = center_y + inner_radius * a2.sin();

        let x4 = center_x + inner_radius * a1.cos();
        let y4 = center_y + inner_radius * a1.sin();

        let large = if (a2 - a1) > std::f32::consts::PI { 1 } else { 0 };

        // Precise SVG annular donut wedge path:
        let path_commands = format!(
            "M {:.2} {:.2} A {:.2} {:.2} 0 {} 1 {:.2} {:.2} L {:.2} {:.2} A {:.2} {:.2} 0 {} 0 {:.2} {:.2} Z",
            x1, y1, outer_radius, outer_radius, large, x2, y2, x3, y3, inner_radius, inner_radius, large, x4, y4
        );

        // Icon and text positioned in the middle of the annular band
        let mid_radius = (outer_radius + inner_radius) / 2.0;
        let mid_x = center_x + mid_radius * center_angle.cos();
        let mid_y = center_y + mid_radius * center_angle.sin();

        let angle_deg = (center_angle.to_degrees() + 360.0) % 360.0;

        let (category_str, color) = match fmt {
            FileFormat::Png => ("IMG", Color::from_rgb_u8(37, 99, 235)),
            FileFormat::Jpg => ("IMG", Color::from_rgb_u8(234, 88, 12)),
            FileFormat::Webp => ("IMG", Color::from_rgb_u8(16, 185, 129)),
            FileFormat::Ico => ("ICO", Color::from_rgb_u8(139, 92, 246)),
            FileFormat::Bmp => ("IMG", Color::from_rgb_u8(6, 182, 212)),
            FileFormat::Tiff => ("IMG", Color::from_rgb_u8(236, 72, 153)),
            FileFormat::Svg => ("VEC", Color::from_rgb_u8(245, 158, 11)),
            FileFormat::Gif => ("GIF", Color::from_rgb_u8(217, 70, 239)),
            FileFormat::Mp4 => ("VID", Color::from_rgb_u8(59, 130, 246)),
            FileFormat::Mov => ("VID", Color::from_rgb_u8(99, 102, 241)),
            FileFormat::Mkv => ("VID", Color::from_rgb_u8(147, 51, 234)),
            FileFormat::Avi => ("VID", Color::from_rgb_u8(168, 85, 247)),
            FileFormat::Webm => ("VID", Color::from_rgb_u8(20, 184, 166)),
            FileFormat::Mp3 => ("AUD", Color::from_rgb_u8(234, 88, 12)),
            FileFormat::Wav => ("AUD", Color::from_rgb_u8(245, 158, 11)),
            FileFormat::Flac => ("AUD", Color::from_rgb_u8(16, 185, 129)),
            FileFormat::Aac => ("AUD", Color::from_rgb_u8(14, 165, 233)),
            FileFormat::M4a => ("AUD", Color::from_rgb_u8(99, 102, 241)),
            FileFormat::Ogg => ("AUD", Color::from_rgb_u8(244, 63, 94)),
            FileFormat::Csv => ("DATA", Color::from_rgb_u8(16, 185, 129)),
            FileFormat::Xlsx => ("SHEET", Color::from_rgb_u8(34, 197, 94)),
            FileFormat::Json => ("DATA", Color::from_rgb_u8(245, 158, 11)),
            FileFormat::Tsv => ("DATA", Color::from_rgb_u8(20, 184, 166)),
            FileFormat::Pdf => ("DOC", Color::from_rgb_u8(239, 68, 68)),
            FileFormat::Docx => ("DOC", Color::from_rgb_u8(59, 130, 246)),
            FileFormat::Txt => ("DOC", Color::from_rgb_u8(148, 163, 184)),
            FileFormat::Html => ("WEB", Color::from_rgb_u8(249, 115, 22)),
        };

        sector_list.push(WheelSector {
            label: fmt.display_label().into(),
            extension: fmt.primary_extension().into(),
            category: category_str.into(),
            color_hex: color,
            pos_x: mid_x,
            pos_y: mid_y,
            angle_deg,
            index: i as i32,
            path_commands: path_commands.into(),
        });
    }

    (
        ModelRc::new(VecModel::from(sector_list)),
        formats.to_vec(),
    )
}
