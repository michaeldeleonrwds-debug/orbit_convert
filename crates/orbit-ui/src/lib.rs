slint::include_modules!();

use orbit_core::formats::FileFormat;
use slint::{Color, ModelRc, VecModel};

/// Generates radial sector metadata for display on the wheel.
/// Calculates the (x, y) coordinates and distinctive branding colors for each format.
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

    // Angle step in radians
    let step = 2.0 * std::f32::consts::PI / count as f32;
    // Start at top (-PI/2)
    let start_angle = -std::f32::consts::FRAC_PI_2;

    for (i, &fmt) in formats.iter().enumerate() {
        let angle = start_angle + i as f32 * step;
        let x = center_x + radius * angle.cos();
        let y = center_y + radius * angle.sin();

        let angle_deg = (angle.to_degrees() + 360.0) % 360.0;

        let color = match fmt {
            FileFormat::Png => Color::from_rgb_u8(37, 99, 235),   // Blue
            FileFormat::Jpg => Color::from_rgb_u8(234, 88, 12),   // Orange
            FileFormat::Webp => Color::from_rgb_u8(16, 185, 129), // Emerald
            FileFormat::Ico => Color::from_rgb_u8(139, 92, 246),  // Purple
            FileFormat::Bmp => Color::from_rgb_u8(6, 182, 212),   // Cyan
            FileFormat::Tiff => Color::from_rgb_u8(236, 72, 153), // Pink
            FileFormat::Svg => Color::from_rgb_u8(245, 158, 11),  // Amber
        };

        sector_list.push(WheelSector {
            label: fmt.display_label().into(),
            extension: fmt.primary_extension().into(),
            color_hex: color,
            pos_x: x,
            pos_y: y,
            angle_deg,
            index: i as i32,
        });
    }

    (
        ModelRc::new(VecModel::from(sector_list)),
        formats.to_vec(),
    )
}
