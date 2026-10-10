//! 日志里的密钥掩码：进程加载到密钥就登记一次，日志落盘前把登记过的值换成 `***`。
//! 逐处「别把密钥记进日志」防不住第三方库的报错原文（dotenvy 带出过整行 `.env`），所以在写入口统一兜底。

mod masking_writer;

use std::borrow::Cow;
use std::sync::RwLock;

pub use self::masking_writer::MaskingWriter;

/// 比这短的值不登记：太短的串会误伤正常日志，也不是真密钥。
const MIN_SECRET_LEN: usize = 8;

const MASK: &str = "***";

static SECRETS: RwLock<Vec<String>> = RwLock::new(Vec::new());

/// 登记一个密钥；重复登记、过短的值忽略。
pub fn register(secret: &str) {
    let secret = secret.trim();
    if secret.len() < MIN_SECRET_LEN {
        return;
    }
    let Ok(mut secrets) = SECRETS.write() else {
        return;
    };
    if !secrets.iter().any(|known| known == secret) {
        secrets.push(secret.to_owned());
    }
}

/// 把文本里登记过的密钥换成 `***`；没有命中就原样借回去。
pub fn mask(text: &str) -> Cow<'_, str> {
    let Ok(secrets) = SECRETS.read() else {
        return Cow::Borrowed(text);
    };
    let mut masked = Cow::Borrowed(text);
    for secret in secrets.iter() {
        if masked.contains(secret.as_str()) {
            masked = Cow::Owned(masked.replace(secret.as_str(), MASK));
        }
    }
    masked
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn registered_secrets_are_masked_everywhere_they_appear() {
        register("sk-test-0123456789abcdef");
        register("short");
        let line =
            "error parsing line: 'KEY=sk-test-0123456789abcdef' and again sk-test-0123456789abcdef";
        assert_eq!(mask(line), "error parsing line: 'KEY=***' and again ***");
        assert_eq!(mask("short stays"), "short stays");
        assert!(matches!(mask("nothing here"), Cow::Borrowed(_)));
    }

    #[test]
    fn writer_masks_before_the_bytes_reach_the_file() {
        register("sk-writer-0123456789abcdef");
        let mut writer = MaskingWriter::new(Vec::new());
        writer
            .write_all("WARN 重载失败 line=sk-writer-0123456789abcdef\n".as_bytes())
            .unwrap();
        assert_eq!(
            String::from_utf8(writer.into_inner()).unwrap(),
            "WARN 重载失败 line=***\n"
        );
    }
}
