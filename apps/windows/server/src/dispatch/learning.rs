//! 空闲时也按已有节拍保存学习，避免最后一次选词只能等下一条输入消息落盘。

use super::{LEARNING_FLUSH_INTERVAL, Router};

impl Router {
    pub(super) fn flush_learning_if_due(&mut self) {
        if self.last_flush.elapsed() >= LEARNING_FLUSH_INTERVAL {
            self.flush_learning();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use qingjian_core::{Engine, Learner};
    use qingjian_dictionary::Dictionary;
    use qingjian_learning::FrequencyLearner;

    use crate::dispatch::{LEARNING_FLUSH_INTERVAL, Router, RouterConfig};

    #[test]
    fn idle_tick_persists_final_choice_without_more_keyboard_messages() {
        let directory =
            std::env::temp_dir().join(format!("vakyu-idle-learning-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("user.tsv");
        let dictionary = Dictionary::parse("拿\tna\t10000\n那\tna\t100\n").unwrap();
        let mut engine = Engine::new(dictionary)
            .with_learner(Box::new(FrequencyLearner::from_path(&path).unwrap()));
        engine.set_input("na");
        let candidate = engine
            .query()
            .unwrap()
            .candidates
            .items
            .into_iter()
            .find(|candidate| candidate.text == "那")
            .unwrap();
        engine.commit(&candidate);
        let mut router = Router::new(engine, RouterConfig::default());
        router.tick();
        assert_eq!(FrequencyLearner::from_path(&path).unwrap().weight("那"), 0);

        router.last_flush = Instant::now() - LEARNING_FLUSH_INTERVAL;
        router.tick();
        let restored = FrequencyLearner::from_path(&path).unwrap();
        assert_eq!(restored.weight("那"), 1);
        assert_eq!(restored.choice_weight("na", "那"), 1);
        let flushed = router.last_flush;
        router.tick();
        assert_eq!(router.last_flush, flushed, "间隔内不重复保存");
        drop(router);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
