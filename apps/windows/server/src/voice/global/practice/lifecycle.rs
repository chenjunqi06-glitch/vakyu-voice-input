//! The practice window stays open until Escape or a successful text insertion.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::voice::global) enum ExitEvent {
    Escape,
    Inserted,
    InsertFailed,
}

pub(in crate::voice::global) const fn should_close(event: ExitEvent) -> bool {
    matches!(event, ExitEvent::Escape | ExitEvent::Inserted)
}

#[cfg(test)]
mod tests {
    use super::{ExitEvent, should_close};

    #[test]
    fn only_escape_or_successful_insertion_closes_practice() {
        assert!(should_close(ExitEvent::Escape));
        assert!(should_close(ExitEvent::Inserted));
        assert!(!should_close(ExitEvent::InsertFailed));
    }
}
