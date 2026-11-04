//! A restrictive disposition retains the complete original mediation checks.
use super::super::FileAbandonedHistory;
use super::{FileMediatedRecovery, JournalError, check_machine};
use crate::action::consequence::delivery::persistent::abandonment::FileAbandonRequest;

impl FileMediatedRecovery {
    /// Consume the same fully verified private cursor into one explicit
    /// abandonment, without a Fence, source refresh or live authority. Initial
    /// and current graph, availability, prediction/evaluation, guard inventory,
    /// effective policy, credentials, floors and retained history all stay bound
    /// by this profile's original checker. No weaker base cursor is exported.
    ///
    /// This changes only the selected attempt's disposition and local sendable
    /// envelope. Charges, topology availability and stream position stay fixed.
    /// Exact historical retries return their original receipt with the current
    /// acknowledged snapshot and anchor, without spending another journal event.
    pub fn finish_abandoned_with_anchor(
        self,
        request: FileAbandonRequest,
    ) -> Result<FileAbandonedHistory, JournalError> {
        let expected = self.expected;
        self.inner
            .finish_abandoned_checked_with_anchor(request, |run| {
                check_machine(&run.profile, &run.machine, &run.events, &expected)
            })
    }
}
