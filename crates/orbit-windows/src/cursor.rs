use std::f32::consts::PI;
use windows::core::Result;
use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

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

/// Calculates the clamped top-left (x, y) coordinates for the radial window
/// so it centers around `cursor_pos` but does not bleed off-screen.
pub fn calculate_clamped_window_pos(
    cursor_pos: Point,
    window_width: f32,
    window_height: f32,
) -> (i32, i32) {
    let work_area = get_monitor_work_area(cursor_pos).unwrap_or(RECT {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    });

    let half_w = window_width / 2.0;
    let half_h = window_height / 2.0;

    let mut left = cursor_pos.x - half_w;
    let mut top = cursor_pos.y - half_h;

    // Clamp within monitor work area
    let min_x = work_area.left as f32;
    let max_x = (work_area.right as f32) - window_width;
    let min_y = work_area.top as f32;
    let max_y = (work_area.bottom as f32) - window_height;

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

    (left.round() as i32, top.round() as i32)
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
    let dist = center.distance_to(&cursor);
    if dist < hub_radius {
        return TwoLevelSelection::CenterHub;
    }

    let dx = cursor.x - center.x;
    let dy = cursor.y - center.y;
    let raw_angle = dy.atan2(dx);
    let mut angle_from_top = raw_angle + (PI / 2.0);
    if angle_from_top < 0.0 {
        angle_from_top += 2.0 * PI;
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
}
