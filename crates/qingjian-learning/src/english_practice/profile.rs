//! 跟读只持久化词数经验与完成轮数；损坏文件报错，留给调用方保留原文件。

use std::{fs, io, path::Path};

use qingjian_core::storage::write_atomic_str;
use serde::{Deserialize, Serialize};

use super::{level::PracticeLevel, progress::PracticeProgress, round::PracticeRound};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PracticeProfile {
    xp: u64,

    completed_sessions: u64,
}

impl PracticeProfile {
    /// 一次长口述也不会直接跳过新手阶段。
    pub const MAX_ROUND_XP: u64 = 60;

    pub fn load(path: &Path) -> io::Result<Self> {
        match fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error),
        }
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        let text = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }
        write_atomic_str(path, &format!("{text}\n"))
    }

    pub fn xp(&self) -> u64 {
        self.xp
    }

    pub fn completed_sessions(&self) -> u64 {
        self.completed_sessions
    }

    pub fn level(&self) -> PracticeLevel {
        PracticeLevel::from_xp(self.xp)
    }

    pub fn start_round(&self) -> PracticeRound {
        PracticeRound::new(self.level())
    }

    /// 仅用于最终结果确认后的完成轮次；临时识别不能调用奖励入口。
    pub fn award_completed(
        &mut self,
        round: &mut PracticeRound,
        progress: PracticeProgress,
    ) -> u64 {
        let Some(words) = round.claim(progress) else {
            return 0;
        };
        let previous = self.xp;
        self.xp = self.xp.saturating_add(words.min(Self::MAX_ROUND_XP));
        self.completed_sessions = self.completed_sessions.saturating_add(1);
        self.xp - previous
    }
}
