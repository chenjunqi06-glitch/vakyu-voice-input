//! 补读保留通过进度，整句重读复用译文；旧录音与已交付原文不能重复上屏。

use super::reading;
use crate::protocol::{AsrEvent, Command, Effect, Event};
use qingjian_practice::PracticeAction;
use std::time::{Duration, Instant};

#[test]
fn continue_keeps_passed_words_and_restart_explicitly_resets_them() {
    for english in [true, false] {
        let mut c = reading(english);
        let prefix = if english { "I want" } else { "我想" };
        c.backend(AsrEvent::Interim {
            generation: c.generation,
            text: prefix.to_owned(),
        });
        assert_eq!(c.practice.as_ref().unwrap().view().matched_words, 2);
        let words = c.practice.as_ref().unwrap().view().words;
        c.command(Command::Action {
            action: PracticeAction::Retry,
            index: None,
        });
        assert_eq!(c.practice.as_ref().unwrap().view().matched_words, 2);
        assert!(c.capturing);
        let old_generation = c.generation;
        c.drain();

        c.command(Command::Action {
            action: PracticeAction::Restart,
            index: None,
        });
        let view = c.practice.as_ref().unwrap().view();
        assert_eq!(view.matched_words, 0);
        assert_eq!(view.words, words);
        assert_eq!(view.original, "Original text");
        assert!(view.recognized.is_empty());
        assert!(c.capturing);
        assert_ne!(c.generation, old_generation);
        let effects = c.drain();
        assert!(
            effects.iter().any(|effect| matches!(
                effect,
                Effect::Native(Event::Capture { active: true, .. })
            ))
        );
        assert!(
            !effects
                .iter()
                .any(|effect| matches!(effect, Effect::Native(Event::Commit { .. } | Event::Hide)))
        );

        c.backend(AsrEvent::Interim {
            generation: old_generation,
            text: prefix.to_owned(),
        });
        c.backend(AsrEvent::Final {
            generation: old_generation,
            text: if english {
                "I want rice"
            } else {
                "我想吃饭"
            }
            .to_owned(),
        });
        assert_eq!(c.practice.as_ref().unwrap().view().matched_words, 0);
        assert!(c.pending_commit.is_none());
        assert!(c.drain().is_empty());
    }
}

#[test]
fn restart_during_or_after_delivery_never_repeats_insertion() {
    let mut c = reading(true);
    c.backend(AsrEvent::Interim {
        generation: c.generation,
        text: "I want rice".to_owned(),
    });
    c.tick(Instant::now() + Duration::from_secs(1));
    c.backend(AsrEvent::Final {
        generation: c.generation,
        text: "I want rice".to_owned(),
    });
    let id = c.pending_commit.as_ref().unwrap().0;
    c.drain();
    c.command(Command::Action {
        action: PracticeAction::Restart,
        index: None,
    });
    assert_eq!(c.pending_commit.as_ref().unwrap().0, id);
    assert!(c.drain().is_empty());

    c.command(Command::CommitResult {
        id,
        ok: true,
        message: None,
    });
    c.drain();
    c.command(Command::Action {
        action: PracticeAction::Restart,
        index: None,
    });
    assert!(!c.active);
    assert!(c.practice.is_none());
    assert!(c.pending_commit.is_none());
    assert!(c.drain().is_empty());
}
