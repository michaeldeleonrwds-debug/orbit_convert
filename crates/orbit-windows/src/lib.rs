pub mod automation;
pub mod cursor;
pub mod dialogs;
pub mod explorer;
pub mod keyboard;
pub mod overlay;

pub use automation::UiAutomationClient;
pub use cursor::{
    calculate_clamped_window_pos, calculate_clamped_window_pos_physical,
    calculate_selected_sector, calculate_two_level_selection,
    calculate_two_level_selection_normalized, get_current_cursor_pos,
    get_monitor_scale_factor, get_monitor_work_area, is_more_sector_active,
    normalize_cursor_to_logical, set_current_cursor_pos, Point, TwoLevelSelection,
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
