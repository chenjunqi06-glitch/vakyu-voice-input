//! 本机当前用户的练习进度；只保存经验与完成次数，不保存句子或录音。

use std::path::PathBuf;

use qingjian_learning::{PracticeLevel, PracticeProfile, PracticeProgress, PracticeRound};

pub struct PracticeGame {
    profile: PracticeProfile,

    path: Option<PathBuf>,

    round: Option<PracticeRound>,

    learn_english: bool,
}

impl PracticeGame {
    pub fn load(data: &std::path::Path, learn_english: bool) -> Self {
        let mut path = Some(data.join(if learn_english {
            "english-practice-progress.json"
        } else {
            "chinese-practice-progress.json"
        }));
        let profile = path.as_deref().map(PracticeProfile::load);
        let profile = match profile {
            Some(Ok(profile)) => profile,
            Some(Err(error)) => {
                // 保留无法解析的旧文件，不能用默认值把已有进度覆盖掉。
                tracing::warn!(kind = ?error.kind(), "英语练习进度读取失败，本次只在内存中累计");
                path = None;
                PracticeProfile::default()
            }
            None => PracticeProfile::default(),
        };
        tracing::info!(xp = profile.xp(), level = ?profile.level(), persistent = path.is_some(), "英语练习进度已加载");
        Self {
            profile,
            path,
            round: None,
            learn_english,
        }
    }

    pub fn begin(&mut self) -> (PracticeLevel, String) {
        let round = self.profile.start_round();
        let level = round.level();
        let xp = self.profile.xp();
        let progress = if !self.learn_english {
            match level.next_threshold() {
                Some(next) => format!(
                    "{xp} / {next} XP · {} to next level",
                    next.saturating_sub(xp)
                ),
                None => format!(
                    "{xp} XP · {} sessions completed",
                    self.profile.completed_sessions()
                ),
            }
        } else {
            match level.next_threshold() {
                Some(next) => format!("{xp} / {next} XP · 再积累 {} 升级", next.saturating_sub(xp)),
                None => format!(
                    "{xp} XP · 已完成 {} 次练习",
                    self.profile.completed_sessions()
                ),
            }
        };
        self.round = Some(round);
        (level, progress)
    }

    pub fn cancel(&mut self) {
        self.round = None;
    }

    pub fn complete(&mut self, progress: PracticeProgress) {
        let Some(mut round) = self.round.take() else {
            return;
        };
        let earned = self.profile.award_completed(&mut round, progress);
        if earned == 0 {
            return;
        }
        tracing::info!(earned, xp = self.profile.xp(), level = ?self.profile.level(), "英语练习经验已累计");
        if let Some(path) = &self.path
            && let Err(error) = self.profile.save(path)
        {
            tracing::warn!(kind = ?error.kind(), "英语练习进度保存失败，本轮进度保留在内存");
        }
    }
}
