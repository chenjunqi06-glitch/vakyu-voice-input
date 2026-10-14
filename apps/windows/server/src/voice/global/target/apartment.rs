//! 将 COM apartment 的寿命绑定到目标与上下文，避免先退出 COM 再释放目标代理。

use std::marker::PhantomData;
use std::rc::Rc;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};

use super::error::TargetError;

pub(super) struct Apartment {
    _thread: PhantomData<Rc<()>>,
}

impl Apartment {
    pub(super) fn initialize() -> Result<Rc<Self>, TargetError> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }
            .map_err(|_| TargetError::Initialization)?;
        Ok(Rc::new(Self {
            _thread: PhantomData,
        }))
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}
