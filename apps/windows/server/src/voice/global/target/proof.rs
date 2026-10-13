//! 目标被确认可编辑的依据；UIA 目标保留原元素供最终身份比较。

use windows::Win32::UI::Accessibility::IUIAutomationElement;

pub(super) enum EditProof {
    TextService(crate::dispatch::voice::delivery::DeliveryTicket),

    Native,

    Automation {
        focused: IUIAutomationElement,
        editor: IUIAutomationElement,
    },
}
