//! 跟读上屏门禁不访问网络、麦克风或输入目标。

use super::PracticeSession;

pub(super) fn strict_session(original: &str) -> PracticeSession {
    PracticeSession::new(
        original.to_owned(),
        qingjian_learning::PracticeLevel::Challenge,
        String::new(),
    )
}

pub(super) fn reading() -> PracticeSession {
    let mut session = strict_session("我今天想吃米饭。");
    assert!(session.translated("I want to eat rice today."));
    session.started();
    session
}

mod chinese;
mod continuation;
mod flow;
mod output;
mod word_gate;
