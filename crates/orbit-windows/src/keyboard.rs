use crossbeam_channel::{Receiver, Sender};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tracing::info;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    VK_ESCAPE, VK_LMENU, VK_MENU, VK_RMENU, VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetForegroundWindow, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK,
    KBDLLHOOKSTRUCT, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

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
    EscapeDown,
    TabDown,
}

// Store raw pointer as isize so Mutex implements Send and Sync safely
static HOOK_RAW: Mutex<Option<isize>> = Mutex::new(None);
static EVENT_SENDER: Mutex<Option<Sender<RawKeyEvent>>> = Mutex::new(None);
static ALT_HELD: AtomicBool = AtomicBool::new(false);

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

            let is_alt = vk == VK_MENU.0 || vk == VK_LMENU.0 || vk == VK_RMENU.0;
            let is_escape = vk == VK_ESCAPE.0;
            let is_tab = vk == VK_TAB.0;

            if msg == WM_SYSKEYDOWN || msg == WM_KEYDOWN {
                if is_alt {
                    // Suppress repeat events when already held
                    let was_held = ALT_HELD.swap(true, Ordering::SeqCst);
                    if !was_held {
                        if let Some(tx) = EVENT_SENDER.lock().unwrap().as_ref() {
                            let _ = tx.try_send(RawKeyEvent::AltDown);
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
                if is_alt {
                    ALT_HELD.store(false, Ordering::SeqCst);
                    if let Some(tx) = EVENT_SENDER.lock().unwrap().as_ref() {
                        let _ = tx.try_send(RawKeyEvent::AltUp);
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

/// Helper to check whether the current foreground window is Explorer or Desktop.
pub fn is_explorer_or_desktop_active() -> bool {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return false;
        }

        let mut class_buf = [0u16; 256];
        let len = windows::Win32::UI::WindowsAndMessaging::GetClassNameW(hwnd, &mut class_buf);
        if len > 0 {
            let class_name = String::from_utf16_lossy(&class_buf[..len as usize]);
            matches!(
                class_name.as_str(),
                "CabinetWClass" | "WorkerW" | "Progman" | "ExploreWClass"
            )
        } else {
            false
        }
    }
}
