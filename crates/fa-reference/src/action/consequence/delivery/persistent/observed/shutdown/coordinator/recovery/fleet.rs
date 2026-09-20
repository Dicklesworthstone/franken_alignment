//! One bounded operator pass over EVERY independently registered domain.
//! Per-domain acknowledgments are not a simultaneous or distributed atomic stop.
use super::*;

/// A trusted operator observation in this registered domain's clock system.
/// The binding is checked; this data type does not authenticate a clock source.
/// An already drained canonical read need not consume the supplied tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileShutdownClock {
    pub domain: u64,
    pub clock_domain: u64,
    pub at: ElapsedTick,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileShutdownPassResult {
    /// No visit began for this member in THIS pass. Older success is irrelevant.
    NotVisited,
    Observed(Box<FileShutdownObservation>),
    /// The native refusal was durably recorded by the coordinator.
    Refused(JournalError),
    /// Coordinator intent/completion did not acknowledge. The domain may have
    /// changed. Never interpret this error as an endpoint nonexecution receipt.
    Unacknowledged(JournalError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileShutdownPassMember {
    pub clock: FileShutdownClock,
    pub result: FileShutdownPassResult,
}

/// Immutable result of this pass, not a reuse of earlier session successes.
/// Every fixed member remains visible even when an earlier write interrupted
/// the pass. No method extracts a domain owner, reviewer or publication key.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// use fa_reference::action::consequence::delivery::persistent::observed::shutdown::coordinator::FileShutdownRecoveryPass;
/// fn authorize(pass: FileShutdownRecoveryPass) -> FilePermit { pass }
/// ```
#[must_use = "Inspect the member outcomes; Ok alone does not mean shutdown completed."]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileShutdownRecoveryPass {
    operation: u64,
    initial_revision: u64,
    final_revision: u64,
    members: Vec<FileShutdownPassMember>,
}
impl FileShutdownRecoveryPass {
    pub fn operation(&self) -> u64 { self.operation }
    pub fn initial_revision(&self) -> u64 { self.initial_revision }
    pub fn final_revision(&self) -> u64 { self.final_revision }
    pub fn members(&self) -> &[FileShutdownPassMember] { &self.members }
    pub fn unobserved_stops(&self) -> Vec<u64> {
        self.members.iter().filter(|member| !self.stopped(member))
            .map(|member| member.clock.domain).collect()
    }
    pub fn all_observed_stopped(&self) -> bool {
        !self.members.is_empty() && self.members.iter().all(|member| self.stopped(member))
    }
    pub fn all_observed_drained(&self) -> bool {
        self.all_observed_stopped() && self.members.iter().all(|member| {
            matches!(&member.result, FileShutdownPassResult::Observed(observation)
                if observation.stop.as_ref().is_some_and(|stop| stop.drained()))
        })
    }
    fn stopped(&self, member: &FileShutdownPassMember) -> bool {
        matches!(&member.result, FileShutdownPassResult::Observed(observation)
            if observation.stop.as_ref().is_some_and(|stop| stop.receipt.request().operation == self.operation))
    }
}

impl FileShutdownCoordinator {
    /// Drive one original recovery visit per registered member, in stable domain
    /// order, with no worker callback, arbitrary path or shared-clock assumption.
    /// `clocks` must contain EXACTLY the full registered roster and each member's
    /// bound clock domain. Missing, extra, duplicate and foreign-clock entries
    /// refuse before any coordinator or domain change. Caller order is irrelevant.
    ///
    /// The entire visit/attempt allowance and revision growth are checked before
    /// starting. Byte admission and native semantic checks still run per visit;
    /// no recovery reserve or observed history floor is enlarged or bypassed.
    /// Busy/offline/stale-clock native refusals are recorded and do not prevent
    /// visiting later members. A coordinator acknowledgment failure stops work
    /// immediately and leaves every subsequent member explicitly NotVisited.
    ///
    /// Outer Err means the pass failed admission before starting. After work
    /// starts, Ok carries EVERY member's actual outcome, including any failed
    /// coordinator acknowledgment; inspect the pass predicates, not Result alone.
    /// Old successes never make this pass complete. Original unresolved effects
    /// remain charged, and no publication is resent. This is bounded synchronous
    /// supervision, not a watchdog, OS kill or simultaneous fleet-stop protocol.
    pub fn recover_registered_pass(&mut self, revision: u64, clocks: &[FileShutdownClock])
        -> Result<FileShutdownRecoveryPass, JournalError>
    {
        if self.unavailable { return Err(JournalError::Unavailable); }
        if revision != self.revision { return Err(Error::Stale.into()); }
        if clocks.len() > MAX_SHUTDOWN_DOMAINS { return Err(Error::Limit.into()); }
        let count = self.campaign.plan.domains.len();
        if clocks.len() != count { return Err(Error::Binding.into()); }
        let mut ordered = Vec::new();
        ordered.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        ordered.extend_from_slice(clocks);
        ordered.sort_by_key(|clock| clock.domain);
        if ordered.windows(2).any(|pair| pair[0].domain == pair[1].domain) {
            return Err(Error::Duplicate.into());
        }
        for (clock, domain) in ordered.iter().zip(self.campaign.plan.domains.iter()) {
            if clock.domain != domain.id() || clock.clock_domain != domain.clock_domain() {
                return Err(Error::Binding.into());
            }
        }
        if self.visits.len().checked_add(count).ok_or(Error::Limit)? > self.campaign.plan.max_attempts
            || self.campaign.attempts.len().checked_add(count).ok_or(Error::Limit)? > self.campaign.plan.max_attempts
        { return Err(Error::Limit.into()); }
        let growth = u64::try_from(count).map_err(|_| Error::Overflow)?
            .checked_mul(2).ok_or(Error::Overflow)?;
        self.revision.checked_add(growth).ok_or(Error::Overflow)?;
        let mut members = Vec::new();
        members.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        members.extend(ordered.into_iter().map(|clock| FileShutdownPassMember {
            clock, result: FileShutdownPassResult::NotVisited,
        }));
        let mut pass = FileShutdownRecoveryPass { operation: self.campaign.plan.operation,
            initial_revision: revision, final_revision: revision, members };
        for member in &mut pass.members {
            match self.recover_and_drain(self.revision(), member.clock.domain, member.clock.at) {
                Ok(Ok(observation)) => member.result = FileShutdownPassResult::Observed(Box::new(observation)),
                Ok(Err(error)) => member.result = FileShutdownPassResult::Refused(error),
                Err(error) => {
                    member.result = FileShutdownPassResult::Unacknowledged(error);
                    break;
                }
            }
        }
        pass.final_revision = self.revision();
        Ok(pass)
    }
}

#[cfg(test)]
mod tests;
