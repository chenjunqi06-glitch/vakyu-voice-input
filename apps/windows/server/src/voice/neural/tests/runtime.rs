//! 安装目录优先、旧路径兼容及组件损坏检查；不依赖当前用户的已安装环境。

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::voice::neural::runtime::{is_complete, select_root, write_fixture};

fn with_roots(test: impl FnOnce(&Path, &Path)) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "vakyu-runtime-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        test(
            &root.join("Program Files/Vakyu/server.exe"),
            &root.join("fresh-user"),
        );
    }));
    std::fs::remove_dir_all(&root).unwrap();
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}

#[test]
fn bundled_runtime_works_without_any_user_installation() {
    with_roots(|executable, _| {
        let bundled = executable.parent().unwrap().join("voice-runtime");
        write_fixture(&bundled);
        assert_eq!(select_root(Some(executable), None), Some(bundled));
    });
}

#[test]
fn complete_bundle_takes_precedence_over_previous_user_runtime() {
    with_roots(|executable, local_data| {
        let bundled = executable.parent().unwrap().join("voice-runtime");
        let legacy = local_data.join("Qingjian/voice-runtime");
        write_fixture(&bundled);
        write_fixture(&legacy);
        assert_eq!(
            select_root(Some(executable), Some(local_data)),
            Some(bundled)
        );
    });
}

#[test]
fn incomplete_bundle_can_use_complete_legacy_runtime() {
    with_roots(|executable, local_data| {
        let bundled = executable.parent().unwrap().join("voice-runtime");
        let legacy = local_data.join("Qingjian/voice-runtime");
        write_fixture(&bundled);
        std::fs::remove_file(bundled.join("models/streaming/decoder.onnx")).unwrap();
        write_fixture(&legacy);
        assert_eq!(
            select_root(Some(executable), Some(local_data)),
            Some(legacy)
        );
    });
}

#[test]
fn empty_model_or_missing_native_library_is_incomplete() {
    with_roots(|executable, _| {
        let root = executable.parent().unwrap().join("voice-runtime");
        write_fixture(&root);
        std::fs::write(root.join("models/streaming/encoder.int8.onnx"), []).unwrap();
        assert!(!is_complete(&root));
        write_fixture(&root);
        std::fs::remove_file(root.join("python/Lib/site-packages/sherpa_onnx/lib/onnxruntime.dll"))
            .unwrap();
        assert!(!is_complete(&root));
    });
}

#[test]
fn damaged_install_is_returned_for_explicit_error_instead_of_silent_fallback() {
    with_roots(|executable, local_data| {
        let bundled = executable.parent().unwrap().join("voice-runtime");
        std::fs::create_dir_all(&bundled).unwrap();
        assert_eq!(
            select_root(Some(executable), Some(local_data)),
            Some(bundled)
        );
    });
}

#[test]
fn no_runtime_retains_uninstalled_behavior() {
    with_roots(|executable, local_data| {
        assert_eq!(select_root(Some(executable), Some(local_data)), None);
        assert_eq!(select_root(None, None), None);
    });
}

#[test]
fn missing_dictation_context_requires_runtime_repair() {
    with_roots(|executable, _| {
        let root = executable.parent().unwrap().join("voice-runtime");
        write_fixture(&root);
        std::fs::remove_file(root.join("app/dictation_context.py")).unwrap();
        assert!(!is_complete(&root));
    });
}
