use tracing::debug;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, HWND_TOPMOST,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOPMOST, WS_EX_TOOLWINDOW,
};

/// Configures a native Win32 window handle to behave as a non-activating,
/// borderless, topmost, transparent overlay.
pub fn configure_overlay_window(hwnd: HWND) {
    unsafe {
        let current_ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let desired_ex_style = current_ex_style
            | (WS_EX_TOPMOST.0 as isize)
            | (WS_EX_TOOLWINDOW.0 as isize)
            | (WS_EX_NOACTIVATE.0 as isize)
            | (WS_EX_LAYERED.0 as isize);

        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, desired_ex_style);

        // Ensure topmost ordering without stealing focus
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );

        debug!("Window HWND {:?} configured as topmost toolwindow overlay", hwnd.0);
    }
}

/// Moves and displays the overlay window at (x, y) without stealing focus.
pub fn position_overlay_window(hwnd: HWND, x: i32, y: i32, width: i32, height: i32) {
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}
