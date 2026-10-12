use std::collections::{HashMap, VecDeque};

use qingjian_core::PredictionRequest;

use crate::prompt::Reply;

/// 最近若干次请求的结果。退格再打回同样的内容很常见，不用再花一次请求。
#[derive(Debug, Default)]
pub struct PredictionCache {
    /// 请求内容 → 结果。
    entries: HashMap<String, Reply>,

    /// 插入顺序，用来淘汰最旧的。
    order: VecDeque<String>,

    /// 容量。
    capacity: usize,
}

impl PredictionCache {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            capacity,
        }
    }

    /// 序号不参与：同样的上下文与拼音就是同一个请求。
    pub fn key(request: &PredictionRequest) -> String {
        format!(
            "{:?}\u{1}{}\u{1}{}\u{1}{}\u{1}{}\u{1}{}\u{1}{}\u{1}{}\u{1}{}\u{1}{}\u{1}{}\u{1}{}",
            request.kind,
            request.before,
            request.after,
            request.letters,
            request.pinyin,
            request.syllables,
            request.candidates.join("\u{2}"),
            request.guess,
            request.max_items,
            request.want_sentence,
            request.text,
            request.target_language
        )
    }

    pub fn get(&self, key: &str) -> Option<&Reply> {
        self.entries.get(key)
    }

    pub fn insert(&mut self, key: String, items: Reply) {
        if self.entries.insert(key.clone(), items).is_none() {
            self.order.push_back(key);
        }
        while self.order.len() > self.capacity {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> PredictionRequest {
        PredictionRequest {
            sequence: 1,
            kind: qingjian_core::PredictionKind::Compose,
            before: String::new(),
            after: String::new(),
            pinyin: "wo'chi".into(),
            letters: "wochi".into(),
            syllables: 2,
            candidates: vec!["我吃".into()],
            guess: "我吃".into(),
            max_items: 2,
            want_sentence: false,
            text: String::new(),
            target_language: String::new(),
        }
    }

    #[test]
    fn evicts_oldest_beyond_capacity() {
        let reply = |s: &str| Reply {
            words: Vec::new(),
            sentence: Some(s.to_owned()),
            english_sentence: None,
        };
        let mut cache = PredictionCache::with_capacity(2);
        cache.insert("a".into(), reply("1"));
        cache.insert("b".into(), reply("2"));
        cache.insert("a".into(), reply("3"));
        cache.insert("c".into(), reply("4"));
        assert_eq!(cache.get("a"), None);
        assert_eq!(cache.get("b").unwrap().sentence.as_deref(), Some("2"));
        assert_eq!(cache.get("c").unwrap().sentence.as_deref(), Some("4"));
    }

    #[test]
    fn key_includes_prompt_inputs_but_ignores_sequence() {
        let first = request();
        let mut sequence = first.clone();
        sequence.sequence = 99;
        assert_eq!(
            PredictionCache::key(&first),
            PredictionCache::key(&sequence)
        );

        let mut guess = first.clone();
        guess.guess = "我想吃".into();
        assert_ne!(PredictionCache::key(&first), PredictionCache::key(&guess));

        let mut syllables = first;
        syllables.syllables = 1;
        assert_ne!(
            PredictionCache::key(&sequence),
            PredictionCache::key(&syllables)
        );
    }
}
