use crossbeam_channel::{Receiver, Sender};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tracing::info;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_ESCAPE, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_MENU,
    VK_OEM_3, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_SHIFT, VK_SPACE, VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetAncestor, GetClassNameW, GetForegroundWindow,
    GetWindowThreadProcessId, SetWindowsHookExW, UnhookWindowsHookEx, WindowFromPoint,
    GA_ROOT, HHOOK, KBDLLHOOKSTRUCT, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN,
    WM_SYSKEYUP,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeySettings {
    pub wheel_a: String,
    pub wheel_b: String,
    pub command_palette: String,
}

impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            wheel_a: "Hold Alt".to_string(),
            wheel_b: "Hold ` (Backtick)".to_string(),
            command_palette: "Ctrl+Shift+Space".to_string(),
        }
    }
}

static HOTKEY_SETTINGS: Mutex<Option<HotkeySettings>> = Mutex::new(None);

pub fn configure_hotkeys(settings: HotkeySettings) {
    if let Ok(mut lock) = HOTKEY_SETTINGS.lock() {
        *lock = Some(settings);
    }
}

fn get_current_hotkeys() -> HotkeySettings {
    HOTKEY_SETTINGS
        .lock()
        .map(|guard| guard.clone().unwrap_or_default())
        .unwrap_or_default()
}

fn matches_wheel_a_down(vk: u16, _config_str: &str) -> bool {
    // Strictly locked to Alt key (left or right) to prevent conflicts with Explorer (Ctrl multi-select, Shift range-select)
    vk == VK_MENU.0 || vk == VK_LMENU.0 || vk == VK_RMENU.0
}

fn matches_wheel_b_down(vk: u16, _config_str: &str) -> bool {
    // Strictly locked to Backtick key (`) to prevent conflicts with Explorer navigation, renaming, or common hotkeys
    vk == VK_OEM_3.0
}

fn matches_command_palette(vk: u16, _config_str: &str) -> bool {
    unsafe {
        let ctrl_held = GetAsyncKeyState(VK_CONTROL.0 as i32) < 0
            || GetAsyncKeyState(VK_LCONTROL.0 as i32) < 0
            || GetAsyncKeyState(VK_RCONTROL.0 as i32) < 0;
        let shift_held = GetAsyncKeyState(VK_SHIFT.0 as i32) < 0
            || GetAsyncKeyState(VK_LSHIFT.0 as i32) < 0
            || GetAsyncKeyState(VK_RSHIFT.0 as i32) < 0;

        // Standard global shortcut: Ctrl+Shift+Space
        ctrl_held && shift_held && vk == VK_SPACE.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionState {
    Idle,
    ResolvingFile,
    WheelVisible,
    FormatHighlighted,
    Converting,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawKeyEvent {
    AltDown,
    AltUp,
    BacktickDown,
    BacktickUp,
    EscapeDown,
    TabDown,
    CommandPalette,
}

// Store raw pointer as isize so Mutex implements Send and Sync safely
static HOOK_RAW: Mutex<Option<isize>> = Mutex::new(None);
static EVENT_SENDER: Mutex<Option<Sender<RawKeyEvent>>> = Mutex::new(None);
static ALT_HELD: AtomicBool = AtomicBool::new(false);
static BACKTICK_HELD: AtomicBool = AtomicBool::new(false);

unsafe extern "system" fn low_level_keyboard_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 {
        let msg = w_param.0 as u32;
        let kb_ptr = l_param.0 as *const KBDLLHOOKSTRUCT;
        if !kb_ptr.is_null() {
            let kb = *kb_ptr;
            let vk = kb.vkCode as u16;

            let hotkeys = get_current_hotkeys();
            let is_wheel_a = matches_wheel_a_down(vk, &hotkeys.wheel_a);
            let is_wheel_b = matches_wheel_b_down(vk, &hotkeys.wheel_b);
            let is_command_palette = matches_command_palette(vk, &hotkeys.command_palette);
            let is_escape = vk == VK_ESCAPE.0;
            let is_tab = vk == VK_TAB.0;

            if msg == WM_SYSKEYDOWN || msg == WM_KEYDOWN {
                if is_command_palette {
                    if let Some(tx) = EVENT_SENDER.lock().unwrap().as_ref() {
                        let _ = tx.try_send(RawKeyEvent::CommandPalette);
                    }
                    // Consume key combo to avoid typing into active app
                    return LRESULT(1);
                } else if is_wheel_a {
                    // Suppress repeat events when already held
                    let was_held = ALT_HELD.swap(true, Ordering::SeqCst);
                    if !was_held {
                        if let Some(tx) = EVENT_SENDER.lock().unwrap().as_ref() {
                            let _ = tx.try_send(RawKeyEvent::AltDown);
                        }
                    }
                } else if is_wheel_b {
                    let shift_held = GetAsyncKeyState(VK_SHIFT.0 as i32) < 0;
                    // Ignore Shift+Backtick (~) so user can type tilde normally
                    if !shift_held {
                        let was_held = BACKTICK_HELD.swap(true, Ordering::SeqCst);
                        if !was_held {
                            if let Some(tx) = EVENT_SENDER.lock().unwrap().as_ref() {
                                let _ = tx.try_send(RawKeyEvent::BacktickDown);
                            }
                        }
                    }
                } else if is_tab {
                    if let Some(tx) = EVENT_SENDER.lock().unwrap().as_ref() {
                        let _ = tx.try_send(RawKeyEvent::TabDown);
                    }
                } else if is_escape {
                    if let Some(tx) = EVENT_SENDER.lock().unwrap().as_ref() {
                        let _ = tx.try_send(RawKeyEvent::EscapeDown);
                    }
                }
            } else if msg == WM_SYSKEYUP || msg == WM_KEYUP {
                if is_wheel_a {
                    ALT_HELD.store(false, Ordering::SeqCst);
                    if let Some(tx) = EVENT_SENDER.lock().unwrap().as_ref() {
                        let _ = tx.try_send(RawKeyEvent::AltUp);
                    }
                } else if is_wheel_b {
                    BACKTICK_HELD.store(false, Ordering::SeqCst);
                    if let Some(tx) = EVENT_SENDER.lock().unwrap().as_ref() {
                        let _ = tx.try_send(RawKeyEvent::BacktickUp);
                    }
                }
            }
        }
    }

    CallNextHookEx(None, n_code, w_param, l_param)
}

pub struct KeyboardHookManager {
    _receiver: Receiver<RawKeyEvent>,
}

impl KeyboardHookManager {
    /// Installs the global low-level keyboard hook and returns the event receiver.
    pub fn install() -> Result<(Self, Receiver<RawKeyEvent>), String> {
        let (tx, rx) = crossbeam_channel::unbounded();

        {
            let mut sender_guard = EVENT_SENDER.lock().unwrap();
            *sender_guard = Some(tx);
        }

        unsafe {
            let hook = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(low_level_keyboard_proc),
                None,
                0,
            )
            .map_err(|e| format!("SetWindowsHookExW failed: {}", e))?;

            let mut hook_guard = HOOK_RAW.lock().unwrap();
            *hook_guard = Some(hook.0 as isize);
        }

        info!("Low-level keyboard hook installed successfully");

        Ok((
            Self {
                _receiver: rx.clone(),
            },
            rx,
        ))
    }
}

impl Drop for KeyboardHookManager {
    fn drop(&mut self) {
        unsafe {
            let mut hook_guard = HOOK_RAW.lock().unwrap();
            if let Some(h_raw) = hook_guard.take() {
                let _ = UnhookWindowsHookEx(HHOOK(h_raw as *mut _));
                info!("Low-level keyboard hook uninstalled");
            }
            let mut sender_guard = EVENT_SENDER.lock().unwrap();
            sender_guard.take();
        }
    }
}

/// Checks whether an HWND belongs to Windows File Explorer or Desktop.
pub fn is_explorer_hwnd(hwnd: HWND) -> bool {
    if hwnd.0.is_null() {
        return false;
    }
    unsafe {
        let root = GetAncestor(hwnd, GA_ROOT);
        let check_hwnd = if root.0.is_null() { hwnd } else { root };

        let mut class_buf = [0u16; 256];
        let len = GetClassNameW(check_hwnd, &mut class_buf);
        if len > 0 {
            let class_name = String::from_utf16_lossy(&class_buf[..len as usize]);
            if matches!(
                class_name.as_str(),
                "CabinetWClass" | "WorkerW" | "Progman" | "ExploreWClass" | "ShellTabWindowClass"
            ) {
                return true;
            }
        }

        // Check process executable name
        let mut pid = 0u32;
        GetWindowThreadProcessId(check_hwnd, Some(&mut pid));
        if pid > 0 {
            if let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                let mut path_buf = [0u16; 1024];
                let mut size = path_buf.len() as u32;
                if QueryFullProcessImageNameW(
                    handle,
                    PROCESS_NAME_FORMAT(0),
                    windows::core::PWSTR(path_buf.as_mut_ptr()),
                    &mut size,
                )
                .is_ok()
                {
                    let path_str = String::from_utf16_lossy(&path_buf[..size as usize]);
                    let is_explorer = path_str.to_ascii_lowercase().ends_with("explorer.exe");
                    let _ = windows::Win32::Foundation::CloseHandle(handle);
                    return is_explorer;
                }
                let _ = windows::Win32::Foundation::CloseHandle(handle);
            }
        }
        false
    }
}

/// Helper to check whether the current foreground window is Explorer or Desktop.
pub fn is_explorer_or_desktop_active() -> bool {
    unsafe {
        let hwnd = GetForegroundWindow();
        is_explorer_hwnd(hwnd)
    }
}

/// Robust check verifying if either the window directly under cursor or the active window is Explorer/Desktop.
pub fn is_cursor_or_foreground_explorer(cursor_x: f32, cursor_y: f32) -> bool {
    unsafe {
        let pt = POINT {
            x: cursor_x.round() as i32,
            y: cursor_y.round() as i32,
        };
        let hwnd_under_cursor = WindowFromPoint(pt);
        if is_explorer_hwnd(hwnd_under_cursor) {
            return true;
        }

        let fg = GetForegroundWindow();
        is_explorer_hwnd(fg)
    }
}
