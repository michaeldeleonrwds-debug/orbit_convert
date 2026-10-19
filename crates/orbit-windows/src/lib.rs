pub mod automation;
pub mod cursor;
pub mod dialogs;
pub mod explorer;
pub mod keyboard;
pub mod overlay;

pub use automation::UiAutomationClient;
pub use cursor::{
    calculate_clamped_window_pos, calculate_selected_sector, get_current_cursor_pos,
    get_monitor_work_area, Point,
};
pub use dialogs::{pick_file, pick_folder};
pub use explorer::{
    ExplorerDiagnosticReport, ExplorerResolver, ResolvedExplorerItem,
};
pub use keyboard::{
    configure_hotkeys, is_explorer_or_desktop_active, HotkeySettings, InteractionState,
    KeyboardHookManager, RawKeyEvent,
};
pub use overlay::{configure_overlay_window, position_overlay_window};
