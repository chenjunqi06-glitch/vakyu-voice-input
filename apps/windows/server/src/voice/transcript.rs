//! 保留已完成语段，并用每个新假设替换当前未完成语段。

#[derive(Default)]
pub(super) struct Transcript {
    complete: String,

    partial: String,
}

impl Transcript {
    pub fn hypothesis(&mut self, text: String) {
        self.partial = text;
    }

    pub fn recognize(&mut self, text: &str) {
        append_phrase(&mut self.complete, text);
        self.partial.clear();
    }

    pub fn text(&self) -> String {
        let mut text = self.complete.clone();
        append_phrase(&mut text, &self.partial);
        text
    }
}

fn append_phrase(target: &mut String, phrase: &str) {
    let phrase = phrase.trim();
    if target.ends_with(|c: char| c.is_ascii_alphanumeric())
        && phrase.starts_with(|c: char| c.is_ascii_alphanumeric())
    {
        target.push(' ');
    }
    target.push_str(phrase);
}

#[cfg(test)]
mod tests {
    use super::Transcript;

    #[test]
    fn corrections_replace_only_the_pending_phrase() {
        let mut transcript = Transcript::default();
        transcript.hypothesis("今天".to_owned());
        transcript.hypothesis("今天下午".to_owned());
        assert_eq!(transcript.text(), "今天下午");
        transcript.recognize("今天下午，");
        transcript.hypothesis("我去".to_owned());
        transcript.hypothesis("我去吃饭".to_owned());
        assert_eq!(transcript.text(), "今天下午，我去吃饭");
        transcript.recognize("我去吃饭。");
        assert_eq!(transcript.text(), "今天下午，我去吃饭。");
    }

    #[test]
    fn separate_english_phrases_keep_the_word_boundary() {
        let mut transcript = Transcript::default();
        transcript.recognize("hello");
        transcript.hypothesis("world".to_owned());
        assert_eq!(transcript.text(), "hello world");
    }
}
