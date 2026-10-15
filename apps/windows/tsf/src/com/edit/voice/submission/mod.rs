//! 提交状态门闸：COM 回调可能重入，排队与写入都必须阻止重复提交。

mod phase;

#[cfg(test)]
mod tests;

use std::cell::Cell;

use self::phase::Phase;

#[derive(Default)]
pub(super) struct Submission {
    phase: Cell<Phase>,
}

impl Submission {
    pub(super) fn queue(&self) -> bool {
        if self.phase.get() != Phase::Idle {
            return false;
        }
        self.phase.set(Phase::Queued);
        true
    }

    pub(super) fn begin(&self) -> bool {
        if self.phase.get() != Phase::Queued {
            return false;
        }
        self.phase.set(Phase::Writing);
        true
    }

    pub(super) fn commit(&self) {
        self.phase.set(Phase::Committed);
    }

    pub(super) fn finish(&self) {
        if self.phase.get() != Phase::Committed {
            self.phase.set(Phase::Idle);
        }
    }

    pub(super) fn queued(&self) -> bool {
        matches!(self.phase.get(), Phase::Queued | Phase::Writing)
    }

    pub(super) fn committed(&self) -> bool {
        self.phase.get() == Phase::Committed
    }
}
