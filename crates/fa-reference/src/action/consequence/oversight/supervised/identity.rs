//! Keep actual identity comparison, mailbox projection and endpoint fencing distinct.
use super::SupervisedDriver;
use crate::action::ElapsedTick;
use crate::action::consequence::oversight::{ReconciliationResults, identity::{
    IdentityOutcome, IdentityStatus,
}, learned_host::identity::{HostedIdentityCheckStatus, HostedLearnedIdentityCheck}};
use crate::Error;

/// An acknowledged original endpoint fence, plus independent per-effect status
/// observations. AwaitingResolution and per-attempt errors remain explicit;
/// success does not mean all effects drained or all helper children reaped.
#[derive(Debug)]
pub struct IdentityFenceSweep {
    pub check: u64,
    pub revocation_floor: u64,
    pub outcomes: ReconciliationResults,
}

/// A numerical comparison can succeed while external containment fails. None
/// means no latched identity mismatch, never a blanket assertion of safety.
#[derive(Debug)]
pub struct HostedIdentityDriverStep {
    pub observation: Result<HostedIdentityCheckStatus, Error>,
    pub synchronization: Result<(), Error>,
    pub containment: Option<Result<IdentityFenceSweep, Error>>,
}

impl SupervisedDriver {
    /// Drive the same owned-model cursor with real pre/post-compute timestamps.
    /// After its original authority installation, synchronize tickets and attempt
    /// endpoint containment independently. This does not create or retry a vote.
    pub fn advance_hosted_learned_identity<F>(&mut self, check: &mut HostedLearnedIdentityCheck,
        expected_revision: u64, mut clock: F) -> HostedIdentityDriverStep
    where F: FnMut() -> ElapsedTick {
        let observation = check.advance_with_clock(self.supervisor.broker_mut(), expected_revision, &mut clock);
        let synchronization = self.supervisor.synchronize();
        let containment = self.service_identity_containment(&mut clock);
        self.reap_helpers();
        HostedIdentityDriverStep { observation, synchronization, containment }
    }

    /// Service only the broker's ORIGINAL latched mismatch and retained installed
    /// fence. No caller-supplied report, timestamp or receipt can stand in for it.
    /// An unapplied mismatch stops the driver before helper I/O but cannot invent
    /// a new authority epoch: the original check must first be installed.
    pub fn service_identity_containment<F>(&mut self, mut clock: F)
        -> Option<Result<IdentityFenceSweep, Error>>
    where F: FnMut() -> ElapsedTick {
        let check = match self.supervisor.broker().identity_status() {
            Ok(IdentityStatus::Mismatch { check }) => check,
            Ok(_) => return None,
            // Let the ordinary loop make its first clock observation. A real
            // mismatch is returned by identity_status even before this branch.
            Err(Error::Incomplete) if self.supervisor.broker().inspect().ledger.elapsed.is_none() => return None,
            Err(error) => return Some(Err(error)),
        };
        // A mismatch already bars new permission, even before installation.
        // Request bounded child cleanup now, but keep any original reservation
        // and job until the authority has actually made its transition.
        self.stop_helper_processes();
        let result = (|| {
            let installed = self.supervisor.broker().identity_installation(check)?.ok_or(Error::Incomplete)?;
            if !matches!(installed.report.outcome, IdentityOutcome::Mismatch(_))
                || !self.supervisor.synchronize_identity_fence()? { return Err(Error::WrongState); }
            // The original authority cancelled unsent work. Sent/unknown effects
            // stay in its ledger after the obsolete driving job is released.
            self.job = None;
            self.reap_helpers();
            // The original identity fence only changes the authority ledger.
            // Advance the distinct dispatcher epoch ONCE before asking the
            // endpoint to reject delayed old envelopes. A failed acknowledgment
            // retries this same fence, not a new restart on every service call.
            if self.identity_restart != Some(check) {
                self.supervisor.broker_mut().restart_dispatcher()?;
                self.identity_restart = Some(check);
            }
            // Closure and cleanup precede any fallible/unwinding clock or I/O.
            self.observe_time(clock())?;
            self.confirm_dispatcher_fence()?;
            let outcomes = self.supervisor.reconcile_pending(&mut self.endpoint)?;
            Ok(IdentityFenceSweep { check, revocation_floor: installed.revocation_floor, outcomes })
        })();
        self.reap_helpers();
        Some(result)
    }
}
