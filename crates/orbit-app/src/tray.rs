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
    GetCursorPos, LoadIconW, PostQuitMessage, RegisterClassW, SetForegroundWindow,
    TrackPopupMenu, HICON, IDI_APPLICATION, MF_CHECKED, MF_GRAYED, MF_SEPARATOR, MF_STRING,
    TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RIGHTBUTTON, WINDOW_EX_STYLE, WM_COMMAND, WM_DESTROY,
    WM_LBUTTONDBLCLK, WM_RBUTTONUP, WM_USER, WNDCLASSW, WS_OVERLAPPEDWINDOW,
};

pub const WM_TRAYICON: u32 = WM_USER + 100;
pub const ID_TRAY_TITLE: usize = 2000;
pub const ID_TRAY_STATUS: usize = 2001;
pub const ID_TRAY_DIAGNOSE: usize = 2002;
pub const ID_TRAY_SETTINGS: usize = 2003;
pub const ID_TRAY_OPEN_DIR: usize = 2004;
pub const ID_TRAY_EXIT: usize = 2005;
pub const ID_TRAY_DASHBOARD: usize = 2006;

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

            let h_icon = {
                let embedded = LoadIconW(Some(HINSTANCE::default()), PCWSTR(1 as _));
                if let Ok(icon) = embedded {
                    if !icon.0.is_null() {
                        icon
                    } else {
                        LoadIconW(None, IDI_APPLICATION).unwrap_or(HICON::default())
                    }
                } else {
                    LoadIconW(None, IDI_APPLICATION).unwrap_or(HICON::default())
                }
            };

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

            if !Shell_NotifyIconW(NIM_ADD, &nid).as_bool() {
                return Err("Failed to register system tray icon".into());
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
