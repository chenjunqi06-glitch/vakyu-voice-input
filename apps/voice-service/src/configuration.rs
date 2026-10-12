//! 配置旁的密钥只装入内存；不修改环境、不输出原文，也不写回配置。

use qingjian_platform::Config;
use std::{io, path::Path, time::SystemTime};

pub fn load(path: &Path) -> io::Result<Config> {
    let mut config =
        Config::load(path).map_err(|_| io::Error::other("configuration unavailable"))?;
    if config
        .predict
        .api_key
        .as_deref()
        .is_none_or(|key| key.trim().is_empty())
    {
        match dotenvy::from_path_iter(path.with_file_name(".env")) {
            Ok(entries) => {
                let mut chosen = None;
                for entry in entries {
                    let (name, value) =
                        entry.map_err(|_| io::Error::other("credential file unavailable"))?;
                    if name == config.predict.api_key_env {
                        chosen = Some(value);
                    }
                }
                if let Some(value) = chosen {
                    qingjian_platform::logs::secrets::register(&value);
                    config.predict.api_key = Some(value);
                }
            }
            Err(dotenvy::Error::Io(error)) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return Err(io::Error::other("credential file unavailable")),
        }
    }
    Ok(config)
}

pub fn stamp(path: &Path) -> (Option<SystemTime>, Option<SystemTime>) {
    let modified = |path: &Path| {
        std::fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .ok()
    };
    (modified(path), modified(&path.with_file_name(".env")))
}

#[cfg(test)]
mod tests {
    use super::load;

    #[test]
    fn sidecar_credentials_reload_without_changing_process_environment() {
        let root = std::env::temp_dir().join(format!("vakyu-config-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("config.toml");
        let name = format!("VAKYU_TEST_ONLY_KEY_{}", std::process::id());
        let prior = std::env::var(&name).ok();
        std::fs::write(&path, format!("[predict]\napi_key_env = \"{name}\"\n")).unwrap();
        std::fs::write(
            root.join(".env"),
            format!("UNUSED=ignored\n{name}=first-test-value\n"),
        )
        .unwrap();
        let initial = load(&path).unwrap();
        std::fs::write(root.join(".env"), format!("{name}=second-test-value\n")).unwrap();
        let next = load(&path).unwrap();
        assert!(initial.predict.api_key.is_some());
        assert!(next.predict.api_key.is_some());
        assert!(initial.predict.api_key != next.predict.api_key);
        assert!(std::env::var(&name).ok() == prior);
        assert!(
            !std::fs::read_to_string(&path)
                .unwrap()
                .contains("test-value")
        );
        std::fs::remove_file(root.join(".env")).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&root).unwrap();
    }
}
