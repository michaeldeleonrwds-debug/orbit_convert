use std::path::PathBuf;
use tracing::warn;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::{
    FileOpenDialog, IFileOpenDialog, IShellItem, FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS,
    SIGDN_FILESYSPATH,
};

/// Open a native Windows folder selection dialog.
pub fn pick_folder(owner: Option<HWND>) -> Option<PathBuf> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let dialog: IFileOpenDialog = match CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER) {
            Ok(d) => d,
            Err(e) => {
                warn!("Failed to instantiate IFileOpenDialog: {}", e);
                return None;
            }
        };

        if let Ok(opts) = dialog.GetOptions() {
            let _ = dialog.SetOptions(opts | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM);
        }

        let title = windows::core::w!("Select Output Folder");
        let _ = dialog.SetTitle(title);

        if dialog.Show(owner).is_err() {
            return None; // Cancelled
        }

        let item: IShellItem = match dialog.GetResult() {
            Ok(it) => it,
            Err(_) => return None,
        };

        let path_pwstr = match item.GetDisplayName(SIGDN_FILESYSPATH) {
            Ok(p) => p,
            Err(_) => return None,
        };

        let path_str = path_pwstr.to_string().unwrap_or_default();
        CoTaskMemFree(Some(path_pwstr.as_ptr() as *const _));

        if path_str.is_empty() {
            None
        } else {
            Some(PathBuf::from(path_str))
        }
    }
}

/// Open a native Windows file selection dialog.
pub fn pick_file(owner: Option<HWND>) -> Option<PathBuf> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let dialog: IFileOpenDialog = match CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER) {
            Ok(d) => d,
            Err(e) => {
                warn!("Failed to instantiate IFileOpenDialog: {}", e);
                return None;
            }
        };

        if let Ok(opts) = dialog.GetOptions() {
            let _ = dialog.SetOptions(opts | FOS_FORCEFILESYSTEM);
        }

        let title = windows::core::w!("Select File to Convert");
        let _ = dialog.SetTitle(title);

        if dialog.Show(owner).is_err() {
            return None; // Cancelled
        }

        let item: IShellItem = match dialog.GetResult() {
            Ok(it) => it,
            Err(_) => return None,
        };

        let path_pwstr = match item.GetDisplayName(SIGDN_FILESYSPATH) {
            Ok(p) => p,
            Err(_) => return None,
        };

        let path_str = path_pwstr.to_string().unwrap_or_default();
        CoTaskMemFree(Some(path_pwstr.as_ptr() as *const _));

        if path_str.is_empty() {
            None
        } else {
            Some(PathBuf::from(path_str))
        }
    }
}
