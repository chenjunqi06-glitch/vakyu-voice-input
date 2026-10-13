//! 独立工作线程的目标上下文；录音中只快查焦点，上屏前完整复核编辑能力。

use std::mem::size_of;
use std::rc::Rc;

use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::UI::Accessibility::{
    CUIAutomation8, IUIAutomation2, IUIAutomationElement, UIA_CONTROLTYPE_ID,
    UIA_CustomControlTypeId, UIA_DocumentControlTypeId, UIA_EditControlTypeId,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{INPUT, SendInput};

use super::access::ProcessAccess;
use super::apartment::Apartment;
use super::editable::{automation_editable, automation_focusable, automation_writable_editor};
use super::error::TargetError;
use super::input::{modifiers_released, unicode_inputs};
use super::locked::LockedTarget;
use super::native::NativeFocus;
use super::proof::EditProof;
use crate::dispatch::voice::delivery::{DeliveryBridge, DeliveryFailure};

const MAX_EDITOR_ANCESTORS: u8 = 4;

pub(crate) struct TargetContext {
    automation: IUIAutomation2,

    _apartment: Rc<Apartment>,

    delivery: DeliveryBridge,
}

impl TargetContext {
    pub(crate) fn new(delivery: DeliveryBridge) -> Result<Self, TargetError> {
        let apartment = Apartment::initialize()?;
        let automation: IUIAutomation2 =
            unsafe { CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER) }
                .map_err(|_| TargetError::Initialization)?;
        (|| unsafe {
            automation.SetConnectionTimeout(500)?;
            automation.SetTransactionTimeout(500)?;
            automation.SetAutoSetFocus(false)?;
            Ok::<_, windows::core::Error>(())
        })()
        .map_err(|_| TargetError::Initialization)?;
        Ok(Self {
            automation,
            _apartment: apartment,
            delivery,
        })
    }

    pub(crate) fn capture(&self) -> Result<LockedTarget, TargetError> {
        let (native, rect) = NativeFocus::capture()?;
        let process = ProcessAccess::capture(native.focus_pid)?;
        let proof = if native.native_editable()? {
            EditProof::Native
        } else if let Some(ticket) = self
            .delivery
            .capture(native.focus_pid, native.thread)
            .map_err(|_| TargetError::DeliveryRejected)?
        {
            tracing::info!(
                process = native.focus_pid,
                thread = native.thread,
                "语音目标使用宿主 TSF 插入点"
            );
            EditProof::TextService(ticket)
        } else {
            let element = unsafe { self.automation.GetFocusedElement() }
                .map_err(|_| TargetError::NoEditableTarget)?;
            let editor = self.find_editor(&element, native.focus_pid)?;
            EditProof::Automation {
                focused: element,
                editor,
            }
        };
        native.validate()?;
        Ok(LockedTarget {
            native,
            proof,
            _process: process,
            rect,
            _apartment: Rc::clone(&self._apartment),
        })
    }

    pub(crate) fn validate_focus(&self, target: &LockedTarget) -> Result<(), TargetError> {
        target.native.validate()
    }

    pub(crate) fn validate(&self, target: &LockedTarget) -> Result<(), TargetError> {
        self.validate_focus(target)?;
        match &target.proof {
            EditProof::TextService(ticket) => {
                if !ticket.current() {
                    return Err(TargetError::FocusChanged);
                }
            }
            EditProof::Native => {
                if !target.native.native_editable()? {
                    return Err(TargetError::FocusChanged);
                }
            }
            EditProof::Automation { focused, editor } => {
                let current = unsafe { self.automation.GetFocusedElement() }
                    .map_err(|_| TargetError::FocusChanged)?;
                if !unsafe { self.automation.CompareElements(focused, &current) }
                    .map_err(|_| TargetError::FocusChanged)?
                    .as_bool()
                {
                    return Err(TargetError::FocusChanged);
                }
                let current_editor = self.find_editor(&current, target.native.focus_pid)?;
                if !unsafe { self.automation.CompareElements(editor, &current_editor) }
                    .map_err(|_| TargetError::FocusChanged)?
                    .as_bool()
                {
                    return Err(TargetError::FocusChanged);
                }
            }
        }
        self.validate_focus(target)
    }

    /// 成功表示系统接收了整批键盘事件，调用方不能据此声称应用正文已视觉验证。
    pub(crate) fn insert(
        &self,
        target: &LockedTarget,
        text: &str,
        still_current: impl Fn() -> bool,
    ) -> Result<usize, TargetError> {
        let inputs = unicode_inputs(text)?;
        if inputs.is_empty() {
            return Ok(0);
        }
        if !still_current() {
            return Err(TargetError::Cancelled);
        }
        self.validate(target)?;
        // UIA 调用可能等到超时，返回之后必须再查取消代次与物理修饰键。
        if !still_current() {
            return Err(TargetError::Cancelled);
        }
        if !modifiers_released() {
            return Err(TargetError::ModifierHeld);
        }
        self.validate_focus(target)?;
        if !still_current() {
            return Err(TargetError::Cancelled);
        }
        if let EditProof::TextService(ticket) = &target.proof {
            match ticket.insert(text) {
                Ok(()) => return Ok(inputs.len() / 2),
                Err(DeliveryFailure::Unconfirmed) => return Err(TargetError::DeliveryUnconfirmed),
                Err(DeliveryFailure::Rejected) => {
                    return Err(TargetError::DeliveryRejected);
                }
            }
        }
        let inserted = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
        let expected = inputs.len() as u32;
        if inserted != expected {
            return Err(TargetError::Injection { inserted, expected });
        }
        Ok(inputs.len() / 2)
    }

    fn find_editor(
        &self,
        focused: &IUIAutomationElement,
        expected_process: u32,
    ) -> Result<IUIAutomationElement, TargetError> {
        automation_focusable(focused)?;
        if automation_process_id(focused)? != expected_process {
            return Err(TargetError::NoEditableTarget);
        }
        match automation_editable(focused) {
            Ok(()) => return Ok(focused.clone()),
            Err(TargetError::NoEditableTarget) => {}
            Err(error) => return Err(error),
        }

        if !may_search_ancestors(
            unsafe { focused.CurrentControlType() }.map_err(|_| TargetError::NoEditableTarget)?,
        ) {
            return Err(TargetError::NoEditableTarget);
        }

        let walker = unsafe { self.automation.ControlViewWalker() }
            .map_err(|_| TargetError::NoEditableTarget)?;
        let mut child = focused.clone();
        for _ in 0..MAX_EDITOR_ANCESTORS {
            let parent = unsafe { walker.GetParentElement(&child) }
                .map_err(|_| TargetError::NoEditableTarget)?;
            if automation_process_id(&parent)? != expected_process {
                return Err(TargetError::NoEditableTarget);
            }
            match automation_writable_editor(&parent) {
                Ok(()) => return Ok(parent),
                Err(TargetError::NoEditableTarget) => child = parent,
                Err(error) => return Err(error),
            }
        }
        Err(TargetError::NoEditableTarget)
    }
}

fn automation_process_id(element: &IUIAutomationElement) -> Result<u32, TargetError> {
    let pid = unsafe { element.CurrentProcessId() }.map_err(|_| TargetError::NoEditableTarget)?;
    u32::try_from(pid)
        .ok()
        .filter(|pid| *pid != 0)
        .ok_or(TargetError::NoEditableTarget)
}

fn may_search_ancestors(control_type: UIA_CONTROLTYPE_ID) -> bool {
    control_type == UIA_EditControlTypeId
        || control_type == UIA_DocumentControlTypeId
        || control_type == UIA_CustomControlTypeId
}

#[cfg(test)]
mod policy_tests {
    use super::*;
    use windows::Win32::UI::Accessibility::{
        UIA_ButtonControlTypeId, UIA_CustomControlTypeId, UIA_DocumentControlTypeId,
        UIA_EditControlTypeId, UIA_WindowControlTypeId,
    };

    #[test]
    fn ancestor_search_is_limited_to_input_like_focus_nodes() {
        assert!(may_search_ancestors(UIA_EditControlTypeId));
        assert!(may_search_ancestors(UIA_DocumentControlTypeId));
        assert!(may_search_ancestors(UIA_CustomControlTypeId));
        assert!(!may_search_ancestors(UIA_ButtonControlTypeId));
        assert!(!may_search_ancestors(UIA_WindowControlTypeId));
        assert_eq!(MAX_EDITOR_ANCESTORS, 4);
    }
}
