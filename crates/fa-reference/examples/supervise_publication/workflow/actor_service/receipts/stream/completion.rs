//! Supervisor projection of one original acknowledged recovery cut.
//! No outcome reducer, status query, settlement retry or permission lives here.
use super::{FileOversight, ServiceReport, debug};
use fa_reference::action::ActionState;
use fa_reference::action::consequence::delivery::persistent::{
    Reconciliation, requests::PendingReconciliations,
};
use std::collections::BTreeMap;

#[derive(Debug)]
pub(super) struct RecoveryReport {
    revision: u64,
    charged: u64,
    outstanding: BTreeMap<u64, ActionState>,
    // None is not attempted, not an empty successful reconciliation.
    sweep: Option<PendingReconciliations>,
}
impl RecoveryReport {
    pub(super) fn capture(
        host: &FileOversight,
        sweep: Option<PendingReconciliations>,
    ) -> Result<Self, String> {
        // Refuse a faulted owner instead of calling its older RAM state current.
        let state = host.stream_snapshot().map_err(debug)?.publication;
        let outstanding = state
            .control
            .ledger
            .stages
            .into_iter()
            .filter(|(_, stage)| {
                matches!(
                    stage,
                    ActionState::Dispatching
                        | ActionState::Unknown
                        | ActionState::IrrecoverablyUnknown
                )
            })
            .collect();
        Ok(Self {
            revision: state.revision,
            charged: state.control.ledger.charged,
            outstanding,
            sweep,
        })
    }

    fn incomplete(&self) -> bool {
        self.sweep.as_ref().is_some_and(|outcomes| {
            !self.outstanding.is_empty()
                || outcomes
                    .values()
                    .any(|result| !matches!(result, Ok(Reconciliation::Resolved(_))))
        })
    }

    pub(super) fn diagnostic(&self) -> String {
        // Original statuses only; no private payload, evidence or external path.
        // revision names recovery, not a claim about a later failed operation.
        format!(
            "acknowledged stream recovery at revision {}; sweep={:?}; outstanding={:?}; charged={}; no effect was resent",
            self.revision, self.sweep, self.outstanding, self.charged
        )
    }

    pub(super) fn finish(
        self,
        service: Result<ServiceReport, String>,
    ) -> Result<ServiceReport, String> {
        let diagnostic = self.diagnostic();
        eprintln!("{diagnostic}");
        match service {
            Err(error) => Err(format!("{error}; {diagnostic}")),
            Ok(report) if self.incomplete() => Err(format!(
                "receipt service completed (frames={}, candidates={}, final_revision={}), but reconciliation is incomplete; {diagnostic}",
                report.frames, report.candidates, report.revision
            )),
            Ok(report) => Ok(report),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_empty_sweep_retains_explicit_unknown_liability_beside_completed_positive() {
        // Pure diagnostic inputs, not an endpoint receipt or a persisted
        // abandonment history. Native expiry/recovery is tested separately.
        for stage in [ActionState::Unknown, ActionState::IrrecoverablyUnknown] {
            let retained = RecoveryReport {
                revision: 7,
                charged: 16,
                outstanding: BTreeMap::from([(2, stage)]),
                sweep: Some(BTreeMap::new()),
            };
            assert!(retained.incomplete());
            let error = retained
                .finish(Ok(ServiceReport {
                    frames: 1,
                    candidates: 1,
                    revision: 7,
                }))
                .unwrap_err();
            assert!(error.contains("reconciliation is incomplete"));
            assert!(error.contains("sweep=Some({})"));
            assert!(error.contains(&format!("outstanding={{2: {stage:?}}}")));
            assert!(error.contains("charged=16"));

            // The same nonzero charge can belong to a completed execution.
            // Refusal depends on unresolved status, not a positive balance.
            let completed = RecoveryReport {
                revision: 7,
                charged: 16,
                outstanding: BTreeMap::new(),
                sweep: Some(BTreeMap::new()),
            };
            assert!(!completed.incomplete());
            completed
                .finish(Ok(ServiceReport {
                    frames: 1,
                    candidates: 1,
                    revision: 7,
                }))
                .unwrap();
        }
    }
}
