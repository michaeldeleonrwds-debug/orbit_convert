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
    pub name: String,
    pub control_type: i32,
    pub bounding_rect: RECT,
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
    /// Traverses up to the parent ListItem or DataItem if a sub-control (like text or icon) was clicked.
    pub fn get_hovered_element(&self, pt: POINT) -> Result<Option<HoveredElementInfo>> {
        unsafe {
            let element = match self.automation.ElementFromPoint(pt) {
                Ok(el) => el,
                Err(e) => {
                    debug!("ElementFromPoint failed at ({}, {}): {}", pt.x, pt.y, e);
                    return Ok(None);
                }
            };

            // Inspect element or traverse up to find ListItem/DataItem
            let walker = self.automation.ControlViewWalker()?;
            let mut current = element;

            for _ in 0..6 {
                let control_type = current
                    .CurrentControlType()
                    .unwrap_or(UIA_CONTROLTYPE_ID(0));

                if control_type == UIA_ListItemControlTypeId
                    || control_type == UIA_DataItemControlTypeId
                {
                    let name = current
                        .CurrentName()
                        .map(|b| b.to_string())
                        .unwrap_or_default();

                    let bounding_rect = current.CurrentBoundingRectangle().unwrap_or_default();

                    return Ok(Some(HoveredElementInfo {
                        name,
                        control_type: control_type.0,
                        bounding_rect,
                    }));
                }

                // Walk to parent
                match walker.GetParentElement(&current) {
                    Ok(parent) => current = parent,
                    Err(_) => break,
                }
            }

            Ok(None)
        }
    }
}
