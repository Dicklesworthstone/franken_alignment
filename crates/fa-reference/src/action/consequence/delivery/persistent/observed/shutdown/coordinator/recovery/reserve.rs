//! Admit negative completion and retained-head capacity before terminal work.
//! Scratch metadata is encoded by the original codec, never installed as truth.
use super::*;
use super::super::codec;

impl FileShutdownCoordinator {
    pub(super) fn begin_registered_terminal(&mut self, revision: u64, domain: u64, kind: ShutdownVisitKind)
        -> Result<(usize, usize), JournalError>
    {
        if self.unavailable { return Err(JournalError::Unavailable); }
        if revision != self.revision { return Err(Error::Stale.into()); }
        if self.visits.len() >= self.campaign.plan.max_attempts
            || self.campaign.attempts.len() >= self.campaign.plan.max_attempts
        { return Err(Error::Limit.into()); }
        let index = self.campaign.plan.domains.binary_search_by_key(&domain, |entry| entry.id())
            .map_err(|_| Error::Missing)?;
        // This reserve is for these native recovery consumers, not a live owner
        // supplied by a caller or an alternate passive-resolution policy.
        if !matches!(kind, ShutdownVisitKind::RecoverStopped { .. }
            | ShutdownVisitKind::RecoverStoppedAtCompletion | ShutdownVisitKind::StopAdmission
            | ShutdownVisitKind::DrainStoppedAtCompletion)
        { return Err(Error::WrongState.into()); }
        let entered = revision.checked_add(1).ok_or(Error::Overflow)?;
        let completed = entered.checked_add(1).ok_or(Error::Overflow)?;
        let mut visits = Vec::new();
        visits.try_reserve_exact(self.visits.len() + 1).map_err(|_| Error::Limit)?;
        visits.extend(self.visits.iter().cloned());
        visits.push(ShutdownVisit { domain, kind, result: ShutdownVisitResult::Entered });
        codec::encode(self.store.identity(), &self.plan_bytes, self.max_bytes,
            entered, &visits, &self.campaign)?;
        // Worst admitted diagnostic byte length; this placeholder is NEVER
        // persisted. Any real refusal still comes from the original finish path.
        visits.last_mut().ok_or(Error::Missing)?.result = ShutdownVisitResult::Refused {
            diagnostic: "x".repeat(MAX_REFUSAL_BYTES),
        };
        codec::encode(self.store.identity(), &self.plan_bytes, self.max_bytes,
            completed, &visits, &self.campaign)?;
        // Encode the same pending visit as Observed using the current heads.
        // Only head payload bytes can grow before finish: counts and revisions
        // are fixed-width in the original codec, and no second visit can enter.
        visits.last_mut().ok_or(Error::Missing)?.result = ShutdownVisitResult::Observed {
            domain_revision: self.campaign.slots[index].revision as u64,
        };
        let observed = codec::encode(self.store.identity(), &self.plan_bytes, self.max_bytes,
            completed, &visits, &self.campaign)?;
        let overhead = observed.len().checked_sub(self.campaign.head_bytes).ok_or(Error::Binding)?;
        let head_limit = self.max_bytes.checked_sub(overhead).ok_or(Error::Limit)?;
        // No attempt, intent, owner poisoning, domain I/O or callback preceded
        // admission. The original registered preflights consume this ceiling
        // BEFORE native mutation. It is not stored or a reserve replenishment.
        let pair = self.begin(revision, domain, kind)?;
        self.campaign.completion_head_limit = Some(head_limit);
        Ok(pair)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod head_tests;
