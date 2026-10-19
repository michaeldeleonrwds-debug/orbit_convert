use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIIF_ERROR, NIIF_INFO, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW, NOTIFY_ICON_DATA_FLAGS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    FindWindowW, GetCursorPos, LoadIconW, LoadImageW, PostQuitMessage, RegisterClassW,
    SendMessageW, SetForegroundWindow, TrackPopupMenu, HICON, ICON_BIG, ICON_SMALL,
    IDI_APPLICATION, IMAGE_ICON, LR_LOADFROMFILE, MF_CHECKED, MF_GRAYED, MF_SEPARATOR, MF_STRING,
    TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RIGHTBUTTON, WINDOW_EX_STYLE, WM_COMMAND, WM_DESTROY,
    WM_LBUTTONDBLCLK, WM_RBUTTONUP, WM_SETICON, WM_USER, WNDCLASSW, WS_OVERLAPPEDWINDOW,
};

pub const WM_TRAYICON: u32 = WM_USER + 100;
pub const ID_TRAY_TITLE: usize = 2000;
pub const ID_TRAY_STATUS: usize = 2001;
pub const ID_TRAY_DIAGNOSE: usize = 2002;
pub const ID_TRAY_SETTINGS: usize = 2003;
pub const ID_TRAY_OPEN_DIR: usize = 2004;
pub const ID_TRAY_EXIT: usize = 2005;
pub const ID_TRAY_DASHBOARD: usize = 2006;

pub fn load_app_icon(size: i32) -> HICON {
    unsafe {
        // 1. Try embedded resource ID 1
        if let Ok(icon) = LoadIconW(Some(HINSTANCE::default()), PCWSTR(1 as _)) {
            if !icon.0.is_null() {
                return icon;
            }
        }

        // 2. Try loading assets/app_icon.ico from filesystem candidates
        let candidate_paths = [
            std::env::current_dir().ok().map(|p| p.join("assets").join("app_icon.ico")),
            std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.join("assets").join("app_icon.ico"))),
            std::env::current_exe().ok().and_then(|p| p.parent().and_then(|p| p.parent()).map(|p| p.join("assets").join("app_icon.ico"))),
            Some(std::path::PathBuf::from(r"C:\Users\REYMARK\Documents\App\Converter\assets\app_icon.ico")),
        ];

        for opt_path in candidate_paths.into_iter().flatten() {
            if opt_path.exists() {
                let wide: Vec<u16> = opt_path.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
                if let Ok(handle) = LoadImageW(
                    None,
                    PCWSTR(wide.as_ptr()),
                    IMAGE_ICON,
                    size,
                    size,
                    LR_LOADFROMFILE,
                ) {
                    let hicon = HICON(handle.0);
                    if !hicon.0.is_null() {
                        return hicon;
                    }
                }
            }
        }

        LoadIconW(None, IDI_APPLICATION).unwrap_or(HICON::default())
    }
}

pub fn apply_window_icon() {
    unsafe {
        let title: Vec<u16> = "OrbitConvert — Fast, Powerful File Conversion\0"
            .encode_utf16()
            .collect();
        for _ in 0..15 {
            let hwnd = FindWindowW(None, PCWSTR(title.as_ptr()));
            if let Ok(w) = hwnd {
                if !w.0.is_null() {
                    let icon_big = load_app_icon(32);
                    let icon_small = load_app_icon(16);
                    if !icon_big.0.is_null() {
                        let _ = SendMessageW(
                            w,
                            WM_SETICON,
                            Some(WPARAM(ICON_BIG as _)),
                            Some(LPARAM(icon_big.0 as _)),
                        );
                    }
                    if !icon_small.0.is_null() {
                        let _ = SendMessageW(
                            w,
                            WM_SETICON,
                            Some(WPARAM(ICON_SMALL as _)),
                            Some(LPARAM(icon_small.0 as _)),
                        );
                    }
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}

pub struct TrayManager {
    hwnd: HWND,
    nid: NOTIFYICONDATAW,
    pub is_enabled: Arc<AtomicBool>,
}

static mut GLOBAL_TRAY: Option<*mut TrayManager> = None;

impl TrayManager {
    pub fn new() -> Result<Self, String> {
        let is_enabled = Arc::new(AtomicBool::new(true));

        unsafe {
            let class_name = w!("OrbitConvertTrayWindowClass");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(tray_window_proc),
                hInstance: HINSTANCE::default(),
                lpszClassName: class_name,
                ..Default::default()
            };

            RegisterClassW(&wc);

            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class_name,
                w!("OrbitConvert Tray"),
                WS_OVERLAPPEDWINDOW,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(HINSTANCE::default()),
                None,
            )
            .map_err(|e| format!("Failed to create tray window: {}", e))?;

            let h_icon = load_app_icon(16);

            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: 1,
                uFlags: NOTIFY_ICON_DATA_FLAGS(0x01 | 0x02 | 0x04), // NIF_MESSAGE | NIF_ICON | NIF_TIP
                uCallbackMessage: WM_TRAYICON,
                hIcon: h_icon,
                ..Default::default()
            };

            let tip = "OrbitConvert - Hold Alt to Convert";
            let mut tip_chars = [0u16; 128];
            for (i, c) in tip.encode_utf16().take(127).enumerate() {
                tip_chars[i] = c;
            }
            nid.szTip = tip_chars;

            let registered = Shell_NotifyIconW(NIM_ADD, &nid).as_bool();
            if !registered {
                let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
                if !Shell_NotifyIconW(NIM_ADD, &nid).as_bool() {
                    tracing::warn!("Windows system tray notification area not ready; continuing without tray icon");
                }
            }

            let tray = Self {
                hwnd,
                nid,
                is_enabled,
            };

            Ok(tray)
        }
    }

    pub fn set_global(&mut self) {
        unsafe {
            GLOBAL_TRAY = Some(self as *mut _);
        }
    }

    /// Shows a native Windows desktop balloon / toast notification.
    pub fn show_notification(&self, title: &str, message: &str, is_error: bool) {
        unsafe {
            let mut nid = self.nid;
            nid.uFlags = NOTIFY_ICON_DATA_FLAGS(0x01 | 0x02 | 0x04 | 0x10); // + NIF_INFO

            let mut info_chars = [0u16; 256];
            for (i, c) in message.encode_utf16().take(255).enumerate() {
                info_chars[i] = c;
            }
            nid.szInfo = info_chars;

            let mut title_chars = [0u16; 64];
            for (i, c) in title.encode_utf16().take(63).enumerate() {
                title_chars[i] = c;
            }
            nid.szInfoTitle = title_chars;

            nid.dwInfoFlags = if is_error { NIIF_ERROR } else { NIIF_INFO };

            let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
    }
}

pub fn send_global_notification(title: &str, message: &str, is_error: bool) {
    unsafe {
        if let Some(tray_ptr) = GLOBAL_TRAY {
            (*tray_ptr).show_notification(title, message, is_error);
        }
    }
}

impl Drop for TrayManager {
    fn drop(&mut self) {
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.nid);
            if !self.hwnd.0.is_null() {
                let _ = DestroyWindow(self.hwnd);
            }
            GLOBAL_TRAY = None;
        }
    }
}

pub type TrayActionCallback = Box<dyn Fn(usize) + Send + Sync>;
static mut TRAY_ACTION_HANDLER: Option<TrayActionCallback> = None;

pub fn set_tray_action_handler<F: Fn(usize) + Send + Sync + 'static>(handler: F) {
    unsafe {
        TRAY_ACTION_HANDLER = Some(Box::new(handler));
    }
}

unsafe extern "system" fn tray_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAYICON => {
            let event = lparam.0 as u32;
            if event == WM_RBUTTONUP {
                show_tray_menu(hwnd);
            } else if event == WM_LBUTTONDBLCLK {
                if let Some(ref handler) = TRAY_ACTION_HANDLER {
                    handler(ID_TRAY_DASHBOARD);
                }
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let cmd_id = (wparam.0 & 0xffff) as usize;
            if let Some(ref handler) = TRAY_ACTION_HANDLER {
                handler(cmd_id);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn show_tray_menu(hwnd: HWND) {
    let mut pt = POINT::default();
    let _ = GetCursorPos(&mut pt);

    let hmenu = match CreatePopupMenu() {
        Ok(m) => m,
        Err(_) => return,
    };

    let is_active = if let Some(tray_ptr) = GLOBAL_TRAY {
        (*tray_ptr).is_enabled.load(Ordering::Relaxed)
    } else {
        true
    };

    let status_str = if is_active {
        w!("Status: Active (Hold Alt in Explorer)")
    } else {
        w!("Status: Paused")
    };

    let _ = AppendMenuW(hmenu, MF_STRING | MF_GRAYED, ID_TRAY_TITLE, w!("OrbitConvert v0.1.0"));
    let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
    let _ = AppendMenuW(hmenu, MF_STRING, ID_TRAY_DASHBOARD, w!("Open Dashboard..."));
    let _ = AppendMenuW(hmenu, MF_STRING, ID_TRAY_SETTINGS, w!("Settings..."));
    let status_flags = if is_active {
        MF_STRING | MF_CHECKED
    } else {
        MF_STRING
    };
    let _ = AppendMenuW(hmenu, status_flags, ID_TRAY_STATUS, status_str);
    let _ = AppendMenuW(hmenu, MF_STRING, ID_TRAY_DIAGNOSE, w!("Diagnose Hovered Item in Explorer"));
    let _ = AppendMenuW(hmenu, MF_STRING, ID_TRAY_OPEN_DIR, w!("Open Output Directory"));
    let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
    let _ = AppendMenuW(hmenu, MF_STRING, ID_TRAY_EXIT, w!("Exit OrbitConvert"));

    let _ = SetForegroundWindow(hwnd);
    let _ = TrackPopupMenu(
        hmenu,
        TPM_BOTTOMALIGN | TPM_LEFTALIGN | TPM_RIGHTBUTTON,
        pt.x,
        pt.y,
        Some(0),
        hwnd,
        None,
    );

    let _ = DestroyMenu(hmenu);
}
