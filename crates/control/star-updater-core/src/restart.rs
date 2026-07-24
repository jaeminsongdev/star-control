//! Fail-closed state machine for restart-required integration updates.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

pub const RESTART_COUNTDOWN: Duration = Duration::seconds(10);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestartState {
    Planned,
    Staged,
    CandidateVerified,
    RestartArmed,
    Countdown,
    Draining,
    CodexStopped,
    Applying,
    OfflineVerified,
    Relaunching,
    OnlinePostcheck,
    Committed,
    Exited,
    Aborted,
    RollbackRequired,
    RollingBack,
    RolledBack,
    PartiallyApplied,
    RollbackFailed,
    RelaunchFailed,
    AppliedValidationPending,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RestartTransaction {
    pub operation_id: String,
    pub state: RestartState,
    pub affected_instance_count: u32,
    pub affected_task_count: Option<u32>,
    pub countdown_deadline: Option<DateTime<Utc>>,
}

impl RestartTransaction {
    pub fn new(operation_id: String) -> Self {
        Self {
            operation_id,
            state: RestartState::Planned,
            affected_instance_count: 0,
            affected_task_count: None,
            countdown_deadline: None,
        }
    }

    pub fn stage(&mut self) -> bool {
        self.transition(RestartState::Planned, RestartState::Staged)
    }

    pub fn verify_candidate(&mut self, instances: u32, tasks: Option<u32>) -> bool {
        if !self.transition(RestartState::Staged, RestartState::CandidateVerified) {
            return false;
        }
        self.affected_instance_count = instances;
        self.affected_task_count = tasks;
        true
    }

    /// Starts the only countdown.  A repeated apply request is idempotent: it
    /// returns the existing deadline rather than extending the user's 10s.
    pub fn arm(&mut self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        match self.state {
            RestartState::CandidateVerified => {
                self.state = RestartState::RestartArmed;
                let deadline = now + RESTART_COUNTDOWN;
                self.countdown_deadline = Some(deadline);
                self.state = RestartState::Countdown;
                Some(deadline)
            }
            RestartState::Countdown => self.countdown_deadline,
            _ => None,
        }
    }

    pub fn begin_draining(&mut self, now: DateTime<Utc>) -> bool {
        self.state == RestartState::Countdown
            && self
                .countdown_deadline
                .is_some_and(|deadline| now >= deadline)
            && self.transition(RestartState::Countdown, RestartState::Draining)
    }

    pub fn transition(&mut self, from: RestartState, to: RestartState) -> bool {
        if self.state != from || !allowed(from, to) {
            return false;
        }
        self.state = to;
        true
    }
}

fn allowed(from: RestartState, to: RestartState) -> bool {
    matches!(
        (from, to),
        (RestartState::Planned, RestartState::Staged)
            | (RestartState::Staged, RestartState::CandidateVerified)
            | (RestartState::Countdown, RestartState::Draining)
            | (RestartState::Draining, RestartState::CodexStopped)
            | (RestartState::CodexStopped, RestartState::Applying)
            | (RestartState::Applying, RestartState::OfflineVerified)
            | (RestartState::Applying, RestartState::RollbackRequired)
            | (RestartState::OfflineVerified, RestartState::Relaunching)
            | (RestartState::Relaunching, RestartState::OnlinePostcheck)
            | (RestartState::Relaunching, RestartState::RelaunchFailed)
            | (RestartState::OnlinePostcheck, RestartState::Committed)
            | (
                RestartState::OnlinePostcheck,
                RestartState::AppliedValidationPending
            )
            | (RestartState::RollbackRequired, RestartState::RollingBack)
            | (RestartState::RollingBack, RestartState::RolledBack)
            | (RestartState::RollingBack, RestartState::PartiallyApplied)
            | (RestartState::RollingBack, RestartState::RollbackFailed)
            | (RestartState::Committed, RestartState::Exited)
            | (RestartState::AppliedValidationPending, RestartState::Exited)
            | (_, RestartState::Aborted)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 18, 0, 0, 0).unwrap()
    }

    #[test]
    fn countdown_is_exact_idempotent_and_cannot_start_before_verification() {
        let mut update = RestartTransaction::new("op_1".to_owned());
        assert_eq!(update.arm(now()), None);
        assert!(update.stage());
        assert!(update.verify_candidate(2, Some(4)));
        let deadline = update.arm(now()).unwrap();
        assert_eq!(deadline, now() + RESTART_COUNTDOWN);
        assert_eq!(update.arm(now() + Duration::seconds(3)), Some(deadline));
        assert!(!update.begin_draining(deadline - Duration::milliseconds(1)));
        assert!(update.begin_draining(deadline));
    }

    #[test]
    fn invalid_transition_never_promotes_to_apply() {
        let mut update = RestartTransaction::new("op_1".to_owned());
        assert!(!update.transition(RestartState::Planned, RestartState::Applying));
        assert_eq!(update.state, RestartState::Planned);
    }

    #[test]
    fn close_failure_can_end_draining_as_aborted() {
        let mut update = RestartTransaction::new("op_1".to_owned());
        assert!(update.stage());
        assert!(update.verify_candidate(1, None));
        let deadline = update.arm(now()).unwrap();
        assert!(update.begin_draining(deadline));
        assert!(update.transition(RestartState::Draining, RestartState::Aborted));
        assert_eq!(update.state, RestartState::Aborted);
        assert!(!update.transition(RestartState::Aborted, RestartState::Applying));
    }

    #[test]
    fn replacement_file_residue_is_not_reported_as_full_rollback() {
        let mut update = RestartTransaction::new("op_1".to_owned());
        update.state = RestartState::Applying;
        assert!(update.transition(RestartState::Applying, RestartState::RollbackRequired));
        assert!(update.transition(RestartState::RollbackRequired, RestartState::RollingBack));
        assert!(update.transition(RestartState::RollingBack, RestartState::PartiallyApplied));
        assert_eq!(update.state, RestartState::PartiallyApplied);
        assert!(!update.transition(RestartState::PartiallyApplied, RestartState::RolledBack));
    }
}
