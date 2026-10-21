use std::f32::consts::PI;
use windows::core::Result;
use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, SetCursorPos};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn distance_to(&self, other: &Point) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// Retrieve current cursor position in screen coordinates.
pub fn get_current_cursor_pos() -> Result<Point> {
    unsafe {
        let mut pt = POINT::default();
        GetCursorPos(&mut pt)?;
        Ok(Point::new(pt.x as f32, pt.y as f32))
    }
}

/// Set current cursor position in screen coordinates.
pub fn set_current_cursor_pos(pt: Point) -> Result<()> {
    unsafe {
        SetCursorPos(pt.x.round() as i32, pt.y.round() as i32)?;
        Ok(())
    }
}

/// Retrieve the DPI scale factor (1.0 = 100%, 1.25 = 125%, 1.5 = 150%, 2.0 = 200%)
/// for the monitor containing the specified point.
pub fn get_monitor_scale_factor(pt: Point) -> f32 {
    unsafe {
        let win_pt = POINT {
            x: pt.x.round() as i32,
            y: pt.y.round() as i32,
        };
        let h_monitor = MonitorFromPoint(win_pt, MONITOR_DEFAULTTONEAREST);
        let mut dpi_x = 96u32;
        let mut dpi_y = 96u32;
        if GetDpiForMonitor(h_monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y).is_ok() {
            (dpi_x as f32 / 96.0).max(1.0)
        } else {
            1.0
        }
    }
}

/// Get the bounding rectangle of the monitor containing the given point.
pub fn get_monitor_work_area(pt: Point) -> Result<RECT> {
    unsafe {
        let win_pt = POINT {
            x: pt.x.round() as i32,
            y: pt.y.round() as i32,
        };
        let h_monitor = MonitorFromPoint(win_pt, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(h_monitor, &mut mi).as_bool() {
            Ok(mi.rcWork)
        } else {
            // Fallback default screen 1920x1080
            Ok(RECT {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1080,
            })
        }
    }
}

/// Calculates the clamped top-left (x, y) physical coordinates for the radial window
/// given its logical dimensions and display scale factor, and returns (clamped_left, clamped_top, physical_center).
pub fn calculate_clamped_window_pos_physical(
    cursor_pos: Point,
    logical_width: f32,
    logical_height: f32,
    scale_factor: f32,
) -> (i32, i32, Point) {
    let scale = scale_factor.max(0.5);
    let phys_w = (logical_width * scale).round();
    let phys_h = (logical_height * scale).round();

    let work_area = get_monitor_work_area(cursor_pos).unwrap_or(RECT {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    });

    let half_w = phys_w / 2.0;
    let half_h = phys_h / 2.0;

    let mut left = cursor_pos.x - half_w;
    let mut top = cursor_pos.y - half_h;

    // Clamp within monitor work area (in physical pixels)
    let min_x = work_area.left as f32;
    let max_x = (work_area.right as f32) - phys_w;
    let min_y = work_area.top as f32;
    let max_y = (work_area.bottom as f32) - phys_h;

    if left < min_x {
        left = min_x;
    } else if left > max_x {
        left = max_x;
    }

    if top < min_y {
        top = min_y;
    } else if top > max_y {
        top = max_y;
    }

    let clamped_left = left.round() as i32;
    let clamped_top = top.round() as i32;
    let center_x = clamped_left as f32 + half_w;
    let center_y = clamped_top as f32 + half_h;

    (clamped_left, clamped_top, Point::new(center_x, center_y))
}

/// Calculates the clamped top-left (x, y) coordinates for the radial window
/// so it centers around `cursor_pos` but does not bleed off-screen.
pub fn calculate_clamped_window_pos(
    cursor_pos: Point,
    window_width: f32,
    window_height: f32,
) -> (i32, i32) {
    let scale = get_monitor_scale_factor(cursor_pos);
    let (left, top, _) = calculate_clamped_window_pos_physical(
        cursor_pos,
        window_width,
        window_height,
        scale,
    );
    (left, top)
}

/// Converts physical cursor coordinates to normalized logical offsets relative to the wheel physical center.
/// Returns `(logical_cursor_offset, logical_distance, angle_from_top_radians)`.
pub fn normalize_cursor_to_logical(
    cursor_pos: Point,
    physical_center: Point,
    scale_factor: f32,
) -> (Point, f32, f32) {
    let scale = scale_factor.max(0.5);
    let dx = (cursor_pos.x - physical_center.x) / scale;
    let dy = (cursor_pos.y - physical_center.y) / scale;
    let dist = (dx * dx + dy * dy).sqrt();

    let raw_angle = dy.atan2(dx);
    let mut angle_from_top = raw_angle + (PI / 2.0);
    if angle_from_top < 0.0 {
        angle_from_top += 2.0 * PI;
    }

    (Point::new(dx, dy), dist, angle_from_top)
}

/// Determines if the cursor is pointing towards or hovering over the "More" expansion sector.
/// Triggers when the cursor is outside the neutral center hub (dist >= hub_radius)
/// and its angle aligns with `more_index` (even if cursor rapidly jumped past the inner ring,
/// up to `max_radius`).
pub fn is_more_sector_active(
    angle_from_top: f32,
    dist: f32,
    hub_radius: f32,
    max_radius: f32,
    more_index: usize,
    num_inner: usize,
) -> bool {
    if num_inner == 0 || dist < hub_radius || dist > max_radius {
        return false;
    }

    let sector_angle = 2.0 * PI / (num_inner as f32);
    let offset_angle = (angle_from_top + (sector_angle / 2.0)) % (2.0 * PI);
    let index = (offset_angle / sector_angle).floor() as usize;

    index.min(num_inner - 1) == more_index
}

/// Computes the highlighted sector index based on cursor position relative to the wheel center.
///
/// Returns:
/// - `None` if within inner dead zone (neutral center hub)
/// - `None` if outside max radius
/// - `None` if `num_sectors == 0`
/// - `Some(index)` from 0 to `num_sectors - 1`
pub fn calculate_selected_sector(
    center: Point,
    cursor: Point,
    dead_zone_radius: f32,
    max_radius: f32,
    num_sectors: usize,
) -> Option<usize> {
    if num_sectors == 0 {
        return None;
    }

    let dist = center.distance_to(&cursor);
    if dist < dead_zone_radius || dist > max_radius {
        return None;
    }

    let dx = cursor.x - center.x;
    let dy = cursor.y - center.y;

    // atan2 gives angle in [-PI, PI] where 0 is (1, 0) [East/Right], PI/2 is South/Down.
    // Our radial wheel starts sector 0 at top (North/Up, -PI/2).
    let raw_angle = dy.atan2(dx);

    // Normalize so top (-PI/2) is 0 radians:
    // angle_from_top = raw_angle - (-PI/2) = raw_angle + PI/2
    let mut angle_from_top = raw_angle + (PI / 2.0);
    if angle_from_top < 0.0 {
        angle_from_top += 2.0 * PI;
    }
    // Now angle_from_top is in [0, 2*PI) starting at 12 o'clock going clockwise.

    let sector_angle = 2.0 * PI / (num_sectors as f32);

    // Center each sector around its primary ray by offsetting by half sector angle
    let offset_angle = (angle_from_top + (sector_angle / 2.0)) % (2.0 * PI);
    let index = (offset_angle / sector_angle).floor() as usize;

    Some(index.min(num_sectors - 1))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TwoLevelSelection {
    None,
    CenterHub,
    Inner(usize),
    Outer(usize),
}

/// Hit-tests normalized logical cursor coordinates against the two-level concentric radial wheel.
/// All distances and coordinates are in logical pixels (matching Slint design).
pub fn calculate_two_level_selection_normalized(
    dist: f32,
    angle_from_top: f32,
    hub_radius: f32,
    inner_outer_radius: f32,
    outer_max_radius: f32,
    is_expanded: bool,
    num_inner: usize,
    num_outer: usize,
) -> TwoLevelSelection {
    if dist < hub_radius {
        return TwoLevelSelection::CenterHub;
    }

    if dist < inner_outer_radius {
        if num_inner == 0 {
            return TwoLevelSelection::None;
        }
        let sector_angle = 2.0 * PI / (num_inner as f32);
        let offset_angle = (angle_from_top + (sector_angle / 2.0)) % (2.0 * PI);
        let index = (offset_angle / sector_angle).floor() as usize;
        return TwoLevelSelection::Inner(index.min(num_inner - 1));
    }

    if is_expanded && dist <= outer_max_radius {
        if num_outer == 0 {
            return TwoLevelSelection::None;
        }
        let sector_angle = 2.0 * PI / (num_outer as f32);
        let offset_angle = (angle_from_top + (sector_angle / 2.0)) % (2.0 * PI);
        let index = (offset_angle / sector_angle).floor() as usize;
        return TwoLevelSelection::Outer(index.min(num_outer - 1));
    }

    TwoLevelSelection::None
}

/// Hit-tests cursor position against a two-level concentric radial wheel.
/// - `dist < hub_radius`: Center hub
/// - `hub_radius <= dist < inner_outer_radius`: Inner wheel sectors
/// - `inner_outer_radius <= dist <= outer_max_radius`: Outer wheel sectors (if expanded)
/// - `dist > outer_max_radius`: Outside wheel (None)
pub fn calculate_two_level_selection(
    center: Point,
    cursor: Point,
    hub_radius: f32,
    inner_outer_radius: f32,
    outer_max_radius: f32,
    is_expanded: bool,
    num_inner: usize,
    num_outer: usize,
) -> TwoLevelSelection {
    let (_, dist, angle_from_top) = normalize_cursor_to_logical(cursor, center, 1.0);
    calculate_two_level_selection_normalized(
        dist,
        angle_from_top,
        hub_radius,
        inner_outer_radius,
        outer_max_radius,
        is_expanded,
        num_inner,
        num_outer,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dead_zone_and_max_radius() {
        let center = Point::new(100.0, 100.0);
        let dead_zone = 40.0;
        let max_radius = 200.0;

        // Inside dead zone
        assert_eq!(
            calculate_selected_sector(center, Point::new(110.0, 110.0), dead_zone, max_radius, 4),
            None
        );

        // Outside max radius
        assert_eq!(
            calculate_selected_sector(center, Point::new(400.0, 100.0), dead_zone, max_radius, 4),
            None
        );

        // Exactly at dead zone boundary
        assert_eq!(
            calculate_selected_sector(center, Point::new(100.0, 70.0), dead_zone, max_radius, 4),
            None
        );
    }

    #[test]
    fn test_4_sectors_clockwise() {
        let center = Point::new(200.0, 200.0);
        let dead_zone = 30.0;
        let max_radius = 200.0;

        // 4 sectors:
        // Sector 0: Top (North)
        // Sector 1: Right (East)
        // Sector 2: Bottom (South)
        // Sector 3: Left (West)

        // North: cursor at (200, 100) -> sector 0
        assert_eq!(
            calculate_selected_sector(center, Point::new(200.0, 100.0), dead_zone, max_radius, 4),
            Some(0)
        );

        // East: cursor at (300, 200) -> sector 1
        assert_eq!(
            calculate_selected_sector(center, Point::new(300.0, 200.0), dead_zone, max_radius, 4),
            Some(1)
        );

        // South: cursor at (200, 300) -> sector 2
        assert_eq!(
            calculate_selected_sector(center, Point::new(200.0, 300.0), dead_zone, max_radius, 4),
            Some(2)
        );

        // West: cursor at (100, 200) -> sector 3
        assert_eq!(
            calculate_selected_sector(center, Point::new(100.0, 200.0), dead_zone, max_radius, 4),
            Some(3)
        );
    }

    #[test]
    fn test_two_level_selection() {
        let center = Point::new(230.0, 230.0);
        let hub_radius = 75.0;
        let inner_outer = 150.0;
        let outer_max = 220.0;

        // Inside center hub
        assert_eq!(
            calculate_two_level_selection(center, Point::new(230.0, 240.0), hub_radius, inner_outer, outer_max, true, 6, 8),
            TwoLevelSelection::CenterHub
        );

        // In inner ring (North, dist=100 from center -> (230, 130))
        assert_eq!(
            calculate_two_level_selection(center, Point::new(230.0, 130.0), hub_radius, inner_outer, outer_max, false, 6, 8),
            TwoLevelSelection::Inner(0)
        );

        // In outer ring when NOT expanded (dist=180) -> None
        assert_eq!(
            calculate_two_level_selection(center, Point::new(230.0, 50.0), hub_radius, inner_outer, outer_max, false, 6, 8),
            TwoLevelSelection::None
        );

        // In outer ring when EXPANDED (North, dist=180 -> (230, 50))
        assert_eq!(
            calculate_two_level_selection(center, Point::new(230.0, 50.0), hub_radius, inner_outer, outer_max, true, 6, 8),
            TwoLevelSelection::Outer(0)
        );

        // Beyond outer ring (dist=240) -> None
        assert_eq!(
            calculate_two_level_selection(center, Point::new(230.0, -20.0), hub_radius, inner_outer, outer_max, true, 6, 8),
            TwoLevelSelection::None
        );
    }

    #[test]
    fn test_dpi_normalization_across_scales() {
        let center_phys = Point::new(1000.0, 1000.0);

        // Test scales: 100%, 125%, 150%, 200%
        for scale in [1.0, 1.25, 1.5, 2.0] {
            // Place cursor 100 logical pixels to the East in physical pixels
            let cur_phys = Point::new(1000.0 + 100.0 * scale, 1000.0);
            let (offset, dist, angle) = normalize_cursor_to_logical(cur_phys, center_phys, scale);

            assert!((offset.x - 100.0).abs() < 1e-4, "offset.x mismatch at scale {scale}");
            assert!(offset.y.abs() < 1e-4, "offset.y mismatch at scale {scale}");
            assert!((dist - 100.0).abs() < 1e-4, "dist mismatch at scale {scale}");
            // East is PI/2 radians from top
            assert!((angle - std::f32::consts::FRAC_PI_2).abs() < 1e-4);

            // Hit test using normalized selection
            let sel = calculate_two_level_selection_normalized(
                dist, angle, 72.5, 152.0, 230.0, false, 4, 8,
            );
            // In 4 inner sectors: 0=Top, 1=East, 2=South, 3=West
            assert_eq!(sel, TwoLevelSelection::Inner(1));
        }
    }

    #[test]
    fn test_is_more_sector_active_rapid_gesture() {
        // Assume 6 inner sectors (0 to 4 formats, sector 5 is More)
        // Sector 5 in 6 sectors:
        // Sector step is 2*PI/6 = PI/3 (60 deg).
        // Sectors are centered at 0 deg, 60 deg, 120 deg, 180 deg, 240 deg, 300 deg.
        // Sector 5 is centered at 300 deg (5 * PI / 3 = 10 * PI / 6).
        let more_angle = 5.0 * (std::f32::consts::PI / 3.0);
        let more_index = 5;
        let num_inner = 6;

        // Inside center hub (dist=50 < 72.5) -> false (neutral)
        assert!(!is_more_sector_active(more_angle, 50.0, 72.5, 230.0, more_index, num_inner));

        // Inside inner ring (dist=100) -> true
        assert!(is_more_sector_active(more_angle, 100.0, 72.5, 230.0, more_index, num_inner));

        // Swept fast past inner ring into outer zone (dist=180) -> true!
        assert!(is_more_sector_active(more_angle, 180.0, 72.5, 230.0, more_index, num_inner));

        // Outside wheel (dist=250 > 230) -> false
        assert!(!is_more_sector_active(more_angle, 250.0, 72.5, 230.0, more_index, num_inner));

        // Different angle (e.g. Sector 0 at 0 radians) -> false
        assert!(!is_more_sector_active(0.0, 180.0, 72.5, 230.0, more_index, num_inner));
    }
}
