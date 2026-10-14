//! 系统级热键占用：先取得新键，再释放旧键；保存失败会自动回滚。

use std::path::Path;

use qingjian_platform::{Config, PracticeShortcut};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey,
};

pub(super) struct Registration {
    shortcut: PracticeShortcut,

    id: i32,

    active: bool,
}

impl Registration {
    pub fn new(shortcut: PracticeShortcut, active: bool) -> Result<Self, String> {
        let id = 0x51a;
        if active {
            register(id, shortcut)?;
        }
        Ok(Self {
            shortcut,
            id,
            active,
        })
    }

    #[cfg(test)]
    pub fn shortcut(&self) -> PracticeShortcut {
        self.shortcut
    }

    pub fn replace(
        &mut self,
        shortcut: PracticeShortcut,
        active: bool,
        path: Option<&Path>,
    ) -> Result<(), String> {
        let next_id = if self.id == 0x51a { 0x51b } else { 0x51a };
        let changed = self.shortcut != shortcut;
        let acquire = (changed || !self.active) && (active || path.is_some());
        if acquire {
            register(next_id, shortcut)?;
        }
        if let Some(path) = path
            && let Err(error) = Config::set_value(
                path,
                "shortcut",
                "voice_practice_shortcut",
                shortcut.key_string(),
            ) {
                if acquire {
                    unregister(next_id, shortcut);
                }
                return Err(format!("快捷键未保存，已保留原设置：{error}"));
        }
        if self.active && (changed || !active) {
            unregister(self.id, self.shortcut);
        }
        if acquire && !active {
            unregister(next_id, shortcut);
        }
        if acquire && active {
            self.id = next_id;
        }
        self.shortcut = shortcut;
        self.active = active;
        Ok(())
    }
}

fn register(id: i32, shortcut: PracticeShortcut) -> Result<(), String> {
    if !shortcut.is_right_ctrl_alt() {
        PracticeShortcut::from_keys(shortcut.key, shortcut.modifiers)?;
        if shortcut.right_modifiers != 0 {
            return Err("该快捷键包含不支持的左右修饰键限制".to_owned());
        }
    }
    register_key(id, shortcut.key, shortcut.modifiers, shortcut)?;
    if shortcut.is_right_ctrl_alt() {
        // RegisterHotKey 的 modifiers 不区分左右，运行时再严格核对实体右侧键。
        // 两个注册分别覆盖先 Ctrl 后 Alt、先 Alt 后 Ctrl 的最后按键。
        if let Err(error) = register_key(id + 2, 0xa3, 1, shortcut) {
            unsafe {
                let _ = UnregisterHotKey(None, id);
            }
            return Err(error);
        }
    }
    Ok(())
}

fn register_key(
    id: i32,
    key: u16,
    modifiers: u8,
    shortcut: PracticeShortcut,
) -> Result<(), String> {
    unsafe {
        RegisterHotKey(
            None,
            id,
            HOT_KEY_MODIFIERS(u32::from(modifiers)) | MOD_NOREPEAT,
            u32::from(key),
        )
    }
    .map_err(|error| {
        format!(
            "该快捷键产生冲突，请更换（{}）；原快捷键保持不变。{error}",
            shortcut.label()
        )
    })
}

fn unregister(id: i32, shortcut: PracticeShortcut) {
    unsafe {
        let _ = UnregisterHotKey(None, id);
        if shortcut.is_right_ctrl_alt() {
            let _ = UnregisterHotKey(None, id + 2);
        }
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        if self.active {
            unregister(self.id, self.shortcut);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn occupied_hotkey_is_rejected_without_releasing_previous_registration() {
        let original: PracticeShortcut = "ctrl+alt+shift+f22".parse().unwrap();
        let occupied: PracticeShortcut = "ctrl+alt+shift+f23".parse().unwrap();
        let (ready, started) = mpsc::channel();
        let (stop, stopped) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let held = Registration::new(occupied, true).unwrap();
            ready.send(()).unwrap();
            stopped.recv().unwrap();
            drop(held);
        });
        started.recv().unwrap();
        let mut held = Registration::new(original, true).unwrap();
        assert!(held.replace(occupied, true, None).is_err());
        assert_eq!(held.shortcut(), original);
        let blocked = std::thread::spawn(move || Registration::new(original, true).is_err())
            .join()
            .unwrap();
        assert!(blocked);
        stop.send(()).unwrap();
        worker.join().unwrap();
    }

    #[test]
    fn save_failure_releases_new_key_and_keeps_original() {
        let original: PracticeShortcut = "ctrl+alt+shift+f20".parse().unwrap();
        let replacement: PracticeShortcut = "ctrl+alt+shift+f21".parse().unwrap();
        let mut held = Registration::new(original, true).unwrap();
        // 目录无法作为 TOML 文件读取，稳定模拟保存失败而不依赖 ACL。
        let path = std::env::temp_dir();
        assert!(held.replace(replacement, true, Some(&path)).is_err());
        assert_eq!(held.shortcut(), original);
        assert!(
            std::thread::spawn(move || Registration::new(replacement, true).is_ok())
                .join()
                .unwrap()
        );
    }

    #[test]
    fn right_control_alt_checks_both_orders_and_rolls_back_both_registrations() {
        fn available(key: u16, modifiers: u8) -> bool {
            std::thread::spawn(move || {
                let acquired =
                    register_key(0x6200, key, modifiers, PracticeShortcut::RIGHT_CTRL_ALT).is_ok();
                if acquired {
                    unsafe {
                        let _ = UnregisterHotKey(None, 0x6200);
                    }
                }
                acquired
            })
            .join()
            .unwrap()
        }

        let original: PracticeShortcut = "ctrl+alt+shift+f19".parse().unwrap();
        let replacement = PracticeShortcut::RIGHT_CTRL_ALT;
        let mut held = Registration::new(original, true).unwrap();
        for (occupied_key, occupied_modifiers, free_key, free_modifiers) in
            [(0xa5, 2, 0xa3, 1), (0xa3, 1, 0xa5, 2)]
        {
            let (ready, started) = mpsc::channel();
            let (stop, stopped) = mpsc::channel();
            let worker = std::thread::spawn(move || {
                register_key(0x6200, occupied_key, occupied_modifiers, replacement).unwrap();
                ready.send(()).unwrap();
                let _ = stopped.recv();
                unsafe {
                    let _ = UnregisterHotKey(None, 0x6200);
                }
            });
            started.recv().unwrap();
            assert!(held.replace(replacement, true, None).is_err());
            assert_eq!(held.shortcut(), original);
            assert!(!available(original.key, original.modifiers));
            assert!(available(free_key, free_modifiers));
            stop.send(()).unwrap();
            worker.join().unwrap();
        }

        assert!(
            held.replace(replacement, true, Some(&std::env::temp_dir()))
                .is_err()
        );
        assert_eq!(held.shortcut(), original);
        assert!(available(0xa5, 2));
        assert!(available(0xa3, 1));
        assert!(!available(original.key, original.modifiers));

        held.replace(replacement, true, None).unwrap();
        assert_eq!(held.shortcut(), replacement);
        assert!(!available(0xa5, 2));
        assert!(!available(0xa3, 1));
        assert!(available(original.key, original.modifiers));
        drop(held);
        assert!(available(0xa5, 2));
        assert!(available(0xa3, 1));
    }
}
