//! Fixed resident round identities are owned before a live operator can choose
//! an additional round. Reservations never rename a configured review or issue
//! a helper, source, publication or convening capability.
use super::*;
use crate::action::consequence::oversight::MAX_OBSERVED_ROUNDS;

impl FileActorSupervisor<FileOversight> {
    /// Reserve a complete fixed review schedule as `(request, round)` pairs.
    /// This is one bounded, atomic, supervisor-only startup selection, made
    /// before the original convening grant is issued. Every planned ID remains
    /// bound to its request for this owner, including after grant revocation.
    /// An explicitly queued successor cannot consume a later release's ID.
    pub fn reserve_review_rounds(&self, schedule: &[(u64, u64)]) -> Result<(), JournalError> {
        if schedule.is_empty() {
            return Err(Error::InvalidInput.into());
        }
        if schedule.len() > MAX_OBSERVED_ROUNDS {
            return Err(Error::Limit.into());
        }
        let mut owner = self
            .owner
            .try_borrow_mut()
            .map_err(|_| JournalError::Unavailable)?;
        if owner.host.storage_failure().is_some() {
            return Err(JournalError::Unavailable);
        }
        if owner.convening.is_some() {
            return Err(Error::Duplicate.into());
        }
        let mut planned = BTreeMap::new();
        for &(request, round) in schedule {
            if request == 0 || round == 0 {
                return Err(Error::InvalidInput.into());
            }
            if planned.insert(round, request).is_some() || owner.host.convene_round_used(round) {
                return Err(Error::Duplicate.into());
            }
        }
        owner.convening = Some(Selection {
            grant: Weak::new(),
            issued: false,
            planned: Rc::new(planned),
            rounds: BTreeSet::new(),
        });
        Ok(())
    }

    /// Exact initial-review admission, not an allocator. The full fixed roster
    /// for a planned request must match, while separately admitted successors
    /// remain unavailable even if their handoff failed or its grant was revoked.
    pub(crate) fn check_initial_review_rounds(
        &self,
        request: u64,
        rounds: impl IntoIterator<Item = u64>,
    ) -> Result<(), JournalError> {
        if request == 0 {
            return Err(Error::InvalidInput.into());
        }
        let owner = self
            .owner
            .try_borrow()
            .map_err(|_| JournalError::Unavailable)?;
        if owner.host.storage_failure().is_some() {
            return Err(JournalError::Unavailable);
        }
        let mut seen = BTreeSet::new();
        for round in rounds {
            if seen.len() == MAX_OBSERVED_ROUNDS {
                return Err(Error::Limit.into());
            }
            if round == 0 {
                return Err(Error::InvalidInput.into());
            }
            if !seen.insert(round) || owner.host.convene_round_used(round) {
                return Err(Error::Duplicate.into());
            }
            if let Some(selected) = &owner.convening {
                if selected.rounds.contains(&round) {
                    return Err(Error::Duplicate.into());
                }
                if !selected.planned.is_empty() && selected.planned.get(&round) != Some(&request) {
                    return Err(Error::Binding.into());
                }
            }
        }
        if seen.is_empty() {
            return Err(Error::InvalidInput.into());
        }
        if let Some(selected) = &owner.convening
            && !selected.planned.is_empty()
            && selected
                .planned
                .iter()
                .filter(|(_, owner)| **owner == request)
                .map(|(round, _)| *round)
                .ne(seen.iter().copied())
        {
            return Err(Error::Binding.into());
        }
        Ok(())
    }
}
