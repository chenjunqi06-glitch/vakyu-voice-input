//! 划词翻译的方向判断与保留换行的译文解析。

use qingjian_core::Language;

use crate::voice_practice::reading::{parse_reply, reading_target, system_prompt};
use crate::voice_practice::{VoicePracticeError, VoicePracticeTranslator};

#[test]
fn direction_follows_the_dominant_script() {
    assert_eq!(reading_target("今天下午三点开会"), Language::English);
    assert_eq!(reading_target("Meeting at 3 pm"), Language::Chinese);
    assert_eq!(reading_target("我在用 Vakyu 输入法"), Language::English);
    assert_eq!(reading_target("Bonjour, ça va ?"), Language::Chinese);
}

#[test]
fn reading_reply_keeps_line_breaks_but_tidies_spaces() {
    let json = r#"{"english":"  First   line.\n\nSecond\tline.  \n"}"#;
    assert_eq!(
        parse_reply(json, Language::English).unwrap(),
        "First line.\n\nSecond line."
    );
    let json = r#"{"chinese":"第一行。\n第二行，提到 José。"}"#;
    assert_eq!(
        parse_reply(json, Language::Chinese).unwrap(),
        "第一行。\n第二行，提到 José。"
    );
    for json in [
        r#"{"english":"你好"}"#,
        r#"{"chinese":"Hello"}"#,
        "{}",
        "nope",
    ] {
        let target = if json.contains("chinese") {
            Language::Chinese
        } else {
            Language::English
        };
        assert_eq!(
            parse_reply(json, target),
            Err(VoicePracticeError::InvalidTranslation)
        );
    }
    assert!(system_prompt(Language::English).unwrap().contains("换行"));
}

#[test]
fn reading_request_picks_its_own_target() {
    let (mut translator, mut requests, _responses) = VoicePracticeTranslator::channels();
    assert_eq!(
        translator.request_reading(7, "Good morning").unwrap(),
        Language::Chinese
    );
    assert_eq!(
        requests.borrow_and_update().as_ref().unwrap().target,
        Language::Chinese
    );
    assert_eq!(
        translator.request_reading(8, "早上好").unwrap(),
        Language::English
    );
}
