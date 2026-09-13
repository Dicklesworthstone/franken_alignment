//! Private evidence adapters for the ONE supervised state machine.
//! An observation refusal is different from failure to persist that observation.
use super::{CommitteeContract, DriverEvidence, ElapsedTick, Error, FileOversight, FrozenAction, JournalError};

/// Not a public provider extension point. The callback adapter preserves the
/// original API; the concrete file adapter can update the same borrowed owner.
/// Outer failure stops the transition. Inner failure can feed a restrictive
/// review or original endpoint seal, but never a permitting evidence fallback.
pub(super) trait EvidenceProvider {
    fn capture<F>(&mut self, host: &mut FileOversight, action: &FrozenAction, clock: &mut F)
        -> Result<Result<DriverEvidence, Error>, JournalError>
    where F: FnMut() -> ElapsedTick;
}

pub(super) struct Callback<P>(pub(super) P);
impl<P> EvidenceProvider for Callback<P>
where P: FnMut(&FrozenAction, &CommitteeContract) -> Result<DriverEvidence, Error> {
    fn capture<F>(&mut self, host: &mut FileOversight, action: &FrozenAction, _clock: &mut F)
        -> Result<Result<DriverEvidence, Error>, JournalError>
    where F: FnMut() -> ElapsedTick {
        // Do not add clock calls or mutate a source for the legacy callback API.
        Ok(self.0(action, &host.profile.committee))
    }
}

pub(super) fn validate(evidence: DriverEvidence, action: &FrozenAction, contracts: &CommitteeContract)
    -> Result<DriverEvidence, Error>
{
    if !evidence.snapshot.complete { return Err(Error::Incomplete); }
    evidence.inputs.as_ref().ok_or(Error::Incomplete)?.validate_for(action, contracts)?;
    Ok(evidence)
}
