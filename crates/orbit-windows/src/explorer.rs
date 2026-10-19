use crate::automation::UiAutomationClient;
use crate::cursor::Point;
use orbit_core::formats::FileFormat;
use std::path::{Path, PathBuf};
use tracing::warn;
use windows::core::{Interface, GUID};
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::System::Com::{
    CoCreateInstance, CoTaskMemFree, IServiceProvider, CLSCTX_ALL,
};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Shell::{
    FOLDERID_Desktop, IFolderView2, IShellBrowser, IShellItem, IShellView, IShellWindows,
    SHCreateItemFromIDList, SHGetKnownFolderPath, ShellWindows, KNOWN_FOLDER_FLAG,
    SIGDN_FILESYSPATH, SIGDN_NORMALDISPLAY, SVGIO_ALLVIEW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetAncestor, GetClassNameW, WindowFromPoint, GA_ROOT,
};

// SID_STopLevelBrowser GUID: {4C96BE40-915C-11CF-99D3-00AA004AE837}
const SID_S_TOP_LEVEL_BROWSER: GUID = GUID::from_u128(0x4C96BE40_915C_11CF_99D3_00AA004AE837);

#[derive(Debug, Clone)]
pub struct ExplorerDiagnosticReport {
    pub cursor_point: (f32, f32),
    pub window_hwnd: isize,
    pub root_hwnd: isize,
    pub window_class: String,
    pub is_explorer_or_desktop: bool,
    pub uia_element_name: Option<String>,
    pub uia_control_type: Option<i32>,
    pub shell_folder_path: Option<PathBuf>,
    pub resolved_item_path: Option<PathBuf>,
    pub detected_format: Option<FileFormat>,
    pub rejection_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedExplorerItem {
    pub path: PathBuf,
    pub filename: String,
    pub format: FileFormat,
}

pub struct ExplorerResolver {
    uia: Option<UiAutomationClient>,
}

unsafe impl Send for ExplorerResolver {}
unsafe impl Sync for ExplorerResolver {}

impl ExplorerResolver {
    pub fn new() -> Self {
        let uia = UiAutomationClient::new().ok();
        Self { uia }
    }

    /// Resolves the file currently under the cursor in Explorer or Desktop.
    pub fn resolve_file_under_cursor(&self, cursor_pos: Point) -> Option<ResolvedExplorerItem> {
        let report = self.diagnose_cursor(cursor_pos);
        if let (Some(path), Some(format)) = (report.resolved_item_path, report.detected_format) {
            let filename = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            Some(ResolvedExplorerItem {
                path,
                filename,
                format,
            })
        } else {
            None
        }
    }

    /// Comprehensive diagnostic run for the item under the cursor.
    pub fn diagnose_cursor(&self, cursor_pos: Point) -> ExplorerDiagnosticReport {
        let pt = POINT {
            x: cursor_pos.x.round() as i32,
            y: cursor_pos.y.round() as i32,
        };

        let mut report = ExplorerDiagnosticReport {
            cursor_point: (cursor_pos.x, cursor_pos.y),
            window_hwnd: 0,
            root_hwnd: 0,
            window_class: String::new(),
            is_explorer_or_desktop: false,
            uia_element_name: None,
            uia_control_type: None,
            shell_folder_path: None,
            resolved_item_path: None,
            detected_format: None,
            rejection_reason: None,
        };

        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
            );

            let hwnd = WindowFromPoint(pt);
            if hwnd.0.is_null() {
                report.rejection_reason = Some("No window under cursor".into());
                return report;
            }
            report.window_hwnd = hwnd.0 as isize;

            let root_hwnd = GetAncestor(hwnd, GA_ROOT);
            report.root_hwnd = root_hwnd.0 as isize;

            let mut class_buf = [0u16; 256];
            let len = GetClassNameW(root_hwnd, &mut class_buf);
            let class_name = if len > 0 {
                String::from_utf16_lossy(&class_buf[..len as usize])
            } else {
                String::new()
            };
            report.window_class = class_name.clone();

            let is_explorer = matches!(
                class_name.as_str(),
                "CabinetWClass" | "WorkerW" | "Progman" | "ExploreWClass"
            );
            report.is_explorer_or_desktop = is_explorer;

            if !is_explorer {
                report.rejection_reason = Some(format!(
                    "Window class '{}' is not File Explorer or Desktop",
                    class_name
                ));
                return report;
            }

            // Hit test UI Automation element
            let uia_element = if let Some(ref uia) = self.uia {
                match uia.get_hovered_element(pt) {
                    Ok(elem) => elem,
                    Err(e) => {
                        warn!("UI Automation hit test failed: {}", e);
                        None
                    }
                }
            } else {
                None
            };

            let target_name = if let Some(el) = uia_element {
                report.uia_element_name = Some(el.name.clone());
                report.uia_control_type = Some(el.control_type);
                el.name
            } else {
                report.rejection_reason =
                    Some("Cursor is not hovering over a list item or file icon".into());
                return report;
            };

            if target_name.trim().is_empty() {
                report.rejection_reason = Some("Hovered item has empty display name".into());
                return report;
            }

            // Resolve folder and items through Shell COM
            if class_name == "Progman" || class_name == "WorkerW" {
                // Desktop path
                if let Ok(desktop_path_pwstr) =
                    SHGetKnownFolderPath(&FOLDERID_Desktop, KNOWN_FOLDER_FLAG(0), None)
                {
                    let desktop_path_str = desktop_path_pwstr.to_string().unwrap_or_default();
                    CoTaskMemFree(Some(desktop_path_pwstr.as_ptr() as *const _));
                    let desktop_folder = PathBuf::from(desktop_path_str);
                    report.shell_folder_path = Some(desktop_folder.clone());

                    Self::match_file_in_folder(&desktop_folder, &target_name, &mut report);
                }
            } else {
                // Regular File Explorer window via IShellWindows
                Self::resolve_via_shell_windows(root_hwnd, &target_name, &mut report);
            }

            if report.resolved_item_path.is_none() && report.rejection_reason.is_none() {
                report.rejection_reason = Some("Could not resolve item to filesystem path".into());
            }

            report
        }
    }

    unsafe fn resolve_via_shell_windows(
        target_hwnd: HWND,
        target_name: &str,
        report: &mut ExplorerDiagnosticReport,
    ) {
        let shell_windows: IShellWindows = match CoCreateInstance(&ShellWindows, None, CLSCTX_ALL) {
            Ok(sw) => sw,
            Err(e) => {
                report.rejection_reason = Some(format!("Failed to connect to IShellWindows: {}", e));
                return;
            }
        };

        let count = shell_windows.Count().unwrap_or(0);
        for i in 0..count {
            let var_index = VARIANT::from(i);
            let dispatch = match shell_windows.Item(&var_index) {
                Ok(d) => d,
                Err(_) => continue,
            };

            let sp: IServiceProvider = match dispatch.cast() {
                Ok(sp) => sp,
                Err(_) => continue,
            };

            let shell_browser: IShellBrowser = match sp.QueryService(&SID_S_TOP_LEVEL_BROWSER) {
                Ok(sb) => sb,
                Err(_) => continue,
            };

            // Check if this window corresponds to our target HWND
            if let Ok(window_hwnd) = shell_browser.GetWindow() {
                let browser_root = GetAncestor(window_hwnd, GA_ROOT);
                if browser_root != target_hwnd && window_hwnd != target_hwnd {
                    continue;
                }
            }

            let shell_view: IShellView = match shell_browser.QueryActiveShellView() {
                Ok(sv) => sv,
                Err(_) => continue,
            };

            let folder_view: IFolderView2 = match shell_view.cast() {
                Ok(fv) => fv,
                Err(_) => continue,
            };

            // Get folder path
            if let Ok(folder_item) = folder_view.GetFolder::<IShellItem>() {
                if let Ok(path_pwstr) = folder_item.GetDisplayName(SIGDN_FILESYSPATH) {
                    let folder_str = path_pwstr.to_string().unwrap_or_default();
                    CoTaskMemFree(Some(path_pwstr.as_ptr() as *const _));
                    let folder_path = PathBuf::from(folder_str);
                    report.shell_folder_path = Some(folder_path.clone());

                    // Inspect items in view
                    let item_count = folder_view.ItemCount(SVGIO_ALLVIEW).unwrap_or(0);
                    for item_idx in 0..item_count {
                        if let Ok(pidl) = folder_view.Item(item_idx as i32) {
                            if let Ok(item) = SHCreateItemFromIDList::<IShellItem>(pidl) {
                                let display_name = item
                                    .GetDisplayName(SIGDN_NORMALDISPLAY)
                                    .map(|p| {
                                        let s = p.to_string().unwrap_or_default();
                                        CoTaskMemFree(Some(p.as_ptr() as *const _));
                                        s
                                    })
                                    .unwrap_or_default();

                                let item_fs_path = item
                                    .GetDisplayName(SIGDN_FILESYSPATH)
                                    .map(|p| {
                                        let s = p.to_string().unwrap_or_default();
                                        CoTaskMemFree(Some(p.as_ptr() as *const _));
                                        PathBuf::from(s)
                                    })
                                    .ok();

                                let matches = display_name.eq_ignore_ascii_case(target_name)
                                    || item_fs_path.as_ref().map_or(false, |p| {
                                        p.file_stem()
                                            .map(|s| s.to_string_lossy().eq_ignore_ascii_case(target_name))
                                            .unwrap_or(false)
                                            || p.file_name()
                                                .map(|s| s.to_string_lossy().eq_ignore_ascii_case(target_name))
                                                .unwrap_or(false)
                                    });

                                if matches {
                                    if let Some(path) = item_fs_path {
                                        if path.is_dir() {
                                            report.rejection_reason =
                                                Some("Hovered item is a directory, not a file".into());
                                            return;
                                        }

                                        if path.is_file() {
                                            report.resolved_item_path = Some(path.clone());
                                            report.detected_format = FileFormat::from_path(&path);
                                            if report.detected_format.is_none() {
                                                report.rejection_reason = Some(format!(
                                                    "File '{}' has unsupported format",
                                                    path.display()
                                                ));
                                            }
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Fallback search directly in folder
                    Self::match_file_in_folder(&folder_path, target_name, report);
                    return;
                }
            }
        }
    }

    fn match_file_in_folder(
        folder: &Path,
        target_name: &str,
        report: &mut ExplorerDiagnosticReport,
    ) {
        if !folder.is_dir() {
            return;
        }

        // Direct exact match
        let exact = folder.join(target_name);
        if exact.is_dir() {
            report.rejection_reason = Some("Hovered item is a directory, not a file".into());
            return;
        }
        if exact.is_file() {
            report.resolved_item_path = Some(exact.clone());
            report.detected_format = FileFormat::from_path(&exact);
            return;
        }

        // Search directory entries
        if let Ok(entries) = std::fs::read_dir(folder) {
            for entry in entries.flatten() {
                let path = entry.path();
                let stem_matches = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().eq_ignore_ascii_case(target_name))
                    .unwrap_or(false);

                let name_matches = path
                    .file_name()
                    .map(|s| s.to_string_lossy().eq_ignore_ascii_case(target_name))
                    .unwrap_or(false);

                if stem_matches || name_matches {
                    if path.is_dir() {
                        report.rejection_reason =
                            Some("Hovered item is a directory, not a file".into());
                        return;
                    }
                    if path.is_file() {
                        report.resolved_item_path = Some(path.clone());
                        report.detected_format = FileFormat::from_path(&path);
                        if report.detected_format.is_none() {
                            report.rejection_reason = Some(format!(
                                "File '{}' format is currently unsupported",
                                path.display()
                            ));
                        }
                        return;
                    }
                }
            }
        }
    }
}
