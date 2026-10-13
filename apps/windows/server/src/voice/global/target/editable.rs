//! 从 UIA 的明确编辑能力判定目标；兼容 LegacyIAccessible，未知控件不猜测。

use windows::Win32::System::Variant::{VARIANT, VT_BOOL, VariantClear};
use windows::Win32::UI::Accessibility::{
    IUIAutomationElement, IUIAutomationLegacyIAccessiblePattern, IUIAutomationTextPattern,
    IUIAutomationValuePattern, ROLE_SYSTEM_TEXT, UIA_DocumentControlTypeId, UIA_EditControlTypeId,
    UIA_IsReadOnlyAttributeId, UIA_LegacyIAccessiblePatternId, UIA_TextPatternId,
    UIA_ValuePatternId,
};
use windows::Win32::UI::WindowsAndMessaging::{
    STATE_SYSTEM_FOCUSED, STATE_SYSTEM_PROTECTED, STATE_SYSTEM_READONLY,
};

use super::error::TargetError;

const STATE_UNAVAILABLE: u32 = 0x0000_0001;

pub(super) fn automation_editable(element: &IUIAutomationElement) -> Result<(), TargetError> {
    automation_focusable(element)?;
    let property_failed = |_| TargetError::NoEditableTarget;
    let control = unsafe { element.CurrentControlType() }.map_err(property_failed)?;
    if control == UIA_EditControlTypeId || control == UIA_DocumentControlTypeId {
        if let Ok(value) =
            unsafe { element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) }
        {
            return match unsafe { value.CurrentIsReadOnly() } {
                Ok(read_only) if !read_only.as_bool() => Ok(()),
                Ok(_) => Err(TargetError::ReadOnly),
                Err(_) => Err(TargetError::NoEditableTarget),
            };
        }
        if let Ok(text) =
            unsafe { element.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) }
            && let Ok(range) = unsafe { text.DocumentRange() }
            && let Ok(attribute) = unsafe { range.GetAttributeValue(UIA_IsReadOnlyAttributeId) }
        {
            match variant_bool(attribute) {
                Some(false) => return Ok(()),
                Some(true) => return Err(TargetError::ReadOnly),
                None => {}
            }
        }
    }
    // 直接获得焦点的旧式输入控件可由 MSAA 证明；祖先节点不使用此宽松回退。
    if let Ok(legacy) = unsafe {
        element.GetCurrentPatternAs::<IUIAutomationLegacyIAccessiblePattern>(
            UIA_LegacyIAccessiblePatternId,
        )
    } {
        let role = unsafe { legacy.CurrentRole() }.map_err(property_failed)?;
        let state = unsafe { legacy.CurrentState() }.map_err(property_failed)?;
        return legacy_state_allows(role, state);
    }
    Err(TargetError::NoEditableTarget)
}

pub(super) fn automation_focusable(element: &IUIAutomationElement) -> Result<(), TargetError> {
    let property_failed = |_| TargetError::NoEditableTarget;
    if unsafe { element.CurrentIsPassword() }
        .map_err(property_failed)?
        .as_bool()
    {
        return Err(TargetError::Password);
    }
    if !unsafe { element.CurrentHasKeyboardFocus() }
        .map_err(property_failed)?
        .as_bool()
        || !unsafe { element.CurrentIsEnabled() }
            .map_err(property_failed)?
            .as_bool()
    {
        return Err(TargetError::NoEditableTarget);
    }
    Ok(())
}

pub(super) fn automation_writable_editor(
    element: &IUIAutomationElement,
) -> Result<(), TargetError> {
    let property_failed = |_| TargetError::NoEditableTarget;
    if unsafe { element.CurrentIsPassword() }
        .map_err(property_failed)?
        .as_bool()
    {
        return Err(TargetError::Password);
    }
    if !unsafe { element.CurrentIsEnabled() }
        .map_err(property_failed)?
        .as_bool()
    {
        return Err(TargetError::NoEditableTarget);
    }
    let control = unsafe { element.CurrentControlType() }.map_err(property_failed)?;
    if control != UIA_EditControlTypeId && control != UIA_DocumentControlTypeId {
        return Err(TargetError::NoEditableTarget);
    }
    if let Ok(value) =
        unsafe { element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) }
    {
        return match unsafe { value.CurrentIsReadOnly() } {
            Ok(read_only) if !read_only.as_bool() => Ok(()),
            Ok(_) => Err(TargetError::ReadOnly),
            Err(_) => Err(TargetError::NoEditableTarget),
        };
    }
    if let Ok(text) =
        unsafe { element.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) }
        && let Ok(range) = unsafe { text.DocumentRange() }
        && let Ok(attribute) = unsafe { range.GetAttributeValue(UIA_IsReadOnlyAttributeId) }
    {
        return match variant_bool(attribute) {
            Some(false) => Ok(()),
            Some(true) => Err(TargetError::ReadOnly),
            None => Err(TargetError::NoEditableTarget),
        };
    }
    Err(TargetError::NoEditableTarget)
}

pub(super) fn legacy_state_allows(role: u32, state: u32) -> Result<(), TargetError> {
    if state & STATE_SYSTEM_PROTECTED != 0 {
        return Err(TargetError::Password);
    }
    if state & STATE_SYSTEM_READONLY != 0 {
        return Err(TargetError::ReadOnly);
    }
    if role != ROLE_SYSTEM_TEXT
        || state & STATE_SYSTEM_FOCUSED == 0
        || state & STATE_UNAVAILABLE != 0
    {
        return Err(TargetError::NoEditableTarget);
    }
    Ok(())
}

fn variant_bool(mut value: VARIANT) -> Option<bool> {
    let result = unsafe {
        let inner = &value.Anonymous.Anonymous;
        (inner.vt == VT_BOOL).then(|| inner.Anonymous.boolVal.0 != 0)
    };
    let _ = unsafe { VariantClear(&mut value) };
    result
}
