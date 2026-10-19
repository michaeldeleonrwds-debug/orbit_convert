use tracing::debug;
use windows::core::Result;
use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, UIA_CONTROLTYPE_ID, UIA_DataItemControlTypeId,
    UIA_ListItemControlTypeId,
};

#[derive(Debug, Clone)]
pub struct HoveredElementInfo {
    /// Raw accessible name from Windows UI Automation
    pub name: String,
    /// Cleaned filename candidate stripped of column metadata and surrounding quotes/marks
    pub clean_name: String,
    pub control_type: i32,
    pub bounding_rect: RECT,
}

/// Robustly extracts the clean filename from UI Automation strings.
///
/// In Windows 11 Explorer Details/List view, accessible names often concatenate all columns:
/// `Name: document.pdf Date modified: 10/19/2026 10:15 PM Type: Adobe Acrobat Document Size: 124 KB`
/// or use newlines, tabs, commas, or directional Unicode marks.
pub fn extract_clean_filename(raw: &str) -> String {
    let mut s = raw.trim();

    // 1. Strip "Name: " prefix if present (common in Windows 11 XAML Explorer)
    if let Some(pos) = s.find("Name: ") {
        s = &s[pos + 6..];
    } else if let Some(pos) = s.find("Name:\t") {
        s = &s[pos + 6..];
    }

    // 2. Strip column delimiters and subsequent metadata (check longer delimiters with commas first)
    for sep in &[
        ", Date modified:",
        " Date modified:",
        ", Date modified",
        " Date modified",
        ", Date created:",
        " Date created:",
        ", Date:",
        " Date:",
        ", Type:",
        " Type:",
        ", Size:",
        " Size:",
        ", Length:",
        " Length:",
        ", Authors:",
        " Authors:",
        ", Dimensions:",
        " Dimensions:",
        "\r\n",
        "\n",
        "\r",
        "\t",
    ] {
        if let Some(idx) = s.find(sep) {
            s = &s[..idx];
        }
    }

    // 3. If multiple consecutive spaces exist (column alignment), take the first token
    if let Some(idx) = s.find("   ") {
        s = &s[..idx];
    }

    // 4. Strip trailing punctuation (like commas or colons left over), quotes and LTR / RTL Unicode markers
    s.trim()
        .trim_matches(|c: char| {
            c == '"'
                || c == '\''
                || c == ','
                || c == ';'
                || c == '\u{200E}'
                || c == '\u{200F}'
                || c == '\u{202A}'
                || c == '\u{202B}'
                || c == '\u{202C}'
        })
        .trim()
        .to_string()
}

pub struct UiAutomationClient {
    automation: IUIAutomation,
}

unsafe impl Send for UiAutomationClient {}
unsafe impl Sync for UiAutomationClient {}

impl UiAutomationClient {
    pub fn new() -> Result<Self> {
        unsafe {
            debug!("Initializing UI Automation client...");
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

            let automation: IUIAutomation = CoCreateInstance(
                &CUIAutomation,
                None,
                CLSCTX_INPROC_SERVER,
            )?;
            debug!("UI Automation client initialized successfully.");

            Ok(Self { automation })
        }
    }

    /// Hit-tests the UI element directly under the cursor coordinates.
    /// Traverses up to find any candidate item, list row, text, or file icon.
    pub fn get_hovered_element(&self, pt: POINT) -> Result<Option<HoveredElementInfo>> {
        unsafe {
            let element = match self.automation.ElementFromPoint(pt) {
                Ok(el) => el,
                Err(e) => {
                    debug!("ElementFromPoint failed at ({}, {}): {}", pt.x, pt.y, e);
                    return Ok(None);
                }
            };

            let walker = self.automation.ControlViewWalker()?;
            let mut current = element;

            let mut best_raw_name = String::new();
            let mut best_clean_name = String::new();
            let mut best_type = 0;
            let mut best_rect = RECT::default();

            for _depth in 0..8 {
                let control_type = current
                    .CurrentControlType()
                    .unwrap_or(UIA_CONTROLTYPE_ID(0));

                let name = current
                    .CurrentName()
                    .map(|b| b.to_string())
                    .unwrap_or_default()
                    .trim()
                    .to_string();

                let rect = current.CurrentBoundingRectangle().unwrap_or_default();

                if !name.is_empty() {
                    let cleaned = extract_clean_filename(&name);

                    // Exact ListItem or DataItem is authoritative container
                    if control_type == UIA_ListItemControlTypeId
                        || control_type == UIA_DataItemControlTypeId
                    {
                        // If clean name from container is non-empty, use it; otherwise fallback to child clean name
                        let final_clean = if !cleaned.is_empty() {
                            cleaned
                        } else if !best_clean_name.is_empty() {
                            best_clean_name
                        } else {
                            name.clone()
                        };

                        return Ok(Some(HoveredElementInfo {
                            name,
                            clean_name: final_clean,
                            control_type: control_type.0,
                            bounding_rect: rect,
                        }));
                    }

                    // For child elements (e.g. filename text cell in Details view),
                    // prioritize if it has an extension or if we haven't found a candidate yet
                    if best_clean_name.is_empty() || cleaned.contains('.') {
                        best_raw_name = name;
                        best_clean_name = cleaned;
                        best_type = control_type.0;
                        best_rect = rect;
                    }
                }

                // Walk to parent
                match walker.GetParentElement(&current) {
                    Ok(parent) => current = parent,
                    Err(_) => break,
                }
            }

            if !best_clean_name.is_empty() || !best_raw_name.is_empty() {
                let final_clean = if !best_clean_name.is_empty() {
                    best_clean_name
                } else {
                    best_raw_name.clone()
                };

                return Ok(Some(HoveredElementInfo {
                    name: best_raw_name,
                    clean_name: final_clean,
                    control_type: best_type,
                    bounding_rect: best_rect,
                }));
            }

            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_clean_filename() {
        assert_eq!(extract_clean_filename("document.pdf"), "document.pdf");
        assert_eq!(extract_clean_filename("logo.svg"), "logo.svg");
        assert_eq!(extract_clean_filename("sample.mp4"), "sample.mp4");
        assert_eq!(extract_clean_filename("archive.tar.gz"), "archive.tar.gz");

        // Windows 11 Details view concatenation
        let win11_str = "Name: document.pdf Date modified: 10/19/2026 10:15 PM Type: Adobe Acrobat Document Size: 124 KB";
        assert_eq!(extract_clean_filename(win11_str), "document.pdf");

        let win11_video = "Name: presentation.mp4, Date modified: 10/19/2026 Type: MP4 Video";
        assert_eq!(extract_clean_filename(win11_video), "presentation.mp4");

        let multiline = "my_design.svg\r\nDate modified: 10/19/2026";
        assert_eq!(extract_clean_filename(multiline), "my_design.svg");

        // Hidden extension case
        let hidden_ext = "Name: annual_report Date modified: 10/19/2026";
        assert_eq!(extract_clean_filename(hidden_ext), "annual_report");

        // Quotes and directional marks
        let quoted = "  \"sample_model.glb\"  ";
        assert_eq!(extract_clean_filename(quoted), "sample_model.glb");

        let marks = "\u{200E}photo.jpg\u{200E}";
        assert_eq!(extract_clean_filename(marks), "photo.jpg");
    }
}
