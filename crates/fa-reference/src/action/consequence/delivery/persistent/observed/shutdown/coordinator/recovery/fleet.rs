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
    /// Canonical-read admission or evidence refused a retained visit. This is a session-only
    /// read outcome, NOT a newly persisted native refusal or a command retry.
    EvidenceRefused(JournalError),
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
        self.run_registered_pass(revision, clocks, false)
    }

    /// Resume the full fixed roster after reopening, without allocating new
    /// durable visits for its latest retained shutdown intents/observations.
    /// Every such member is freshly checked by resolve_shutdown_visit; saved
    /// success alone never counts. Other members use recover_and_drain.
    ///
    /// A missing, newer, conflicting or not-yet-stopped image returns a distinct
    /// EvidenceRefused outcome. It NEVER falls through into a native retry. A
    /// matching stop can remain undrained; this method does not manufacture drain
    /// progress or silently replace a saved tick with the supplied current tick.
    /// Use an explicit new recovery visit to request further native progress.
    ///
    /// The same full-roster, clock, attempt and revision checks apply. Admission
    /// charges new visit slots only to members that require new recovery, one
    /// completion revision per pending resolution, and zero for exact refreshes.
    /// Any failed coordinator acknowledgment still stops the whole pass.
    pub fn resume_registered_pass(&mut self, revision: u64, clocks: &[FileShutdownClock])
        -> Result<FileShutdownRecoveryPass, JournalError>
    {
        self.run_registered_pass(revision, clocks, true)
    }

    fn run_registered_pass(&mut self, revision: u64, clocks: &[FileShutdownClock], resume: bool)
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
        let mut resolutions = Vec::new();
        resolutions.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        let mut new_visits = 0_usize;
        let mut growth = 0_u64;
        for clock in &ordered {
            // Consider the LAST visit of any kind, not an older convenient
            // success. A later refusal/inspection must not disappear on resume.
            let latest = self.visits.iter().enumerate().rev()
                .find(|(_, visit)| visit.domain == clock.domain);
            let resolution = latest.filter(|(_, visit)| resume
                && matches!(visit.kind, ShutdownVisitKind::Advance { .. }
                    | ShutdownVisitKind::RecoverStopped { .. })
                && matches!(visit.result, ShutdownVisitResult::Entered
                    | ShutdownVisitResult::Observed { .. }));
            let cost = match resolution {
                Some((_, visit)) => u64::from(matches!(visit.result, ShutdownVisitResult::Entered)),
                None => { new_visits = new_visits.checked_add(1).ok_or(Error::Limit)?; 2 }
            };
            growth = growth.checked_add(cost).ok_or(Error::Overflow)?;
            resolutions.push(resolution.map(|(index, _)| index));
        }
        if self.visits.len().checked_add(new_visits).ok_or(Error::Limit)? > self.campaign.plan.max_attempts
            || self.campaign.attempts.len().checked_add(count).ok_or(Error::Limit)? > self.campaign.plan.max_attempts
        { return Err(Error::Limit.into()); }
        self.revision.checked_add(growth).ok_or(Error::Overflow)?;
        let mut members = Vec::new();
        members.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        members.extend(ordered.into_iter().map(|clock| FileShutdownPassMember {
            clock, result: FileShutdownPassResult::NotVisited,
        }));
        let mut pass = FileShutdownRecoveryPass { operation: self.campaign.plan.operation,
            initial_revision: revision, final_revision: revision, members };
        for (member, resolution) in pass.members.iter_mut().zip(resolutions) {
            if let Some(visit) = resolution {
                match self.resolve_shutdown_visit(self.revision(), visit) {
                    Ok(observation) => member.result = FileShutdownPassResult::Observed(Box::new(observation)),
                    Err(error) if !self.unavailable => member.result = FileShutdownPassResult::EvidenceRefused(error),
                    Err(error) => {
                        member.result = FileShutdownPassResult::Unacknowledged(error);
                        break;
                    }
                }
                continue;
            }
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

#[cfg(test)]
mod resume_tests;
