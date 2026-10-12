//! 按需查询随包英语释义表；不构建常驻索引或启动后台任务。

use crate::PracticeWordCard;
use qingjian_core::Language;

pub fn pronunciation_language(word: &str, learn_english: bool) -> Language {
    if learn_english
        || word
            .chars()
            .all(|character| !('\u{3400}'..='\u{9fff}').contains(&character))
    {
        Language::English
    } else {
        Language::Chinese
    }
}

pub fn word_card(word: &str) -> PracticeWordCard {
    let word = word
        .trim_matches(|ch: char| !(ch.is_alphanumeric() || matches!(ch, '\'' | '-')))
        .to_owned();
    let meaning = qingjian_translate::offline_english_meaning(&word);
    let pronunciation =
        qingjian_learning::english_ipa_variants(&word).map(|variants| variants.join(" · "));
    let hint = match (meaning.is_some(), pronunciation.is_some()) {
        (false, false) => Some("本机词典暂未收录这个词".to_owned()),
        (false, true) => Some("本机词典暂未收录释义".to_owned()),
        (true, false) => Some("本机发音词典暂未收录音标".to_owned()),
        (true, true) => None,
    };
    PracticeWordCard {
        word,
        meaning,
        pronunciation,
        hint,
    }
}

pub fn word_card_for_language(word: &str, learn_english: bool) -> PracticeWordCard {
    if learn_english {
        return word_card(word);
    }
    if pronunciation_language(word, false) == Language::English {
        let mut card = word_card(word);
        card.meaning = None;
        card.hint = Some("Latin name or word · Listen to its pronunciation".to_owned());
        return card;
    }
    let word = word
        .trim_matches(|ch: char| !ch.is_alphanumeric())
        .to_owned();
    let meaning = qingjian_translate::offline_chinese_meaning(&word);
    let pronunciation = qingjian_translate::chinese_pinyin(&word);
    let hint = meaning
        .is_none()
        .then(|| "No entry in the offline dictionary".to_owned());
    PracticeWordCard {
        word,
        meaning,
        pronunciation,
        hint,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn word_card_keeps_a_friendly_fallback_for_unlisted_names() {
        let card = super::word_card("Chen Junqi");
        assert_eq!(card.word, "Chen Junqi");
        assert!(card.meaning.is_none());
        assert!(card.pronunciation.is_none());
        assert_eq!(card.hint.as_deref(), Some("本机词典暂未收录这个词"));
    }

    #[test]
    fn chinese_card_and_latin_name_choose_their_own_pronunciation_language() {
        use qingjian_core::Language;
        let chinese = super::word_card_for_language("银行", false);
        assert!(chinese.meaning.is_some());
        assert_eq!(chinese.pronunciation.as_deref(), Some("yín háng"));
        assert_eq!(
            super::pronunciation_language("银行", false),
            Language::Chinese
        );
        let latin = super::word_card_for_language("Alice", false);
        assert_eq!(latin.word, "Alice");
        assert!(latin.hint.as_deref().unwrap().contains("Latin"));
        assert_eq!(
            super::pronunciation_language("Alice", false),
            Language::English
        );
    }
}
