//! Read acknowledged original outcomes after a lost response or process restart.
//! This projection cannot complete a round, reacquire a handle or grant authority.
use super::{FileLearnedSidecarFinish, FileOversight, JournalError, Machine};
use super::super::{FileLearnedConfig, FileOversightProfile, bind_history, journal, storage};
use std::path::Path;

/// Exact original completion plus its journaled source/input binding. These are
/// historical observations, including a refinement or an application refusal.
/// No live source, independent authenticity or endpoint execution is asserted.
///
/// ```compile_fail,E0308
/// use fa_reference::action::Permit;
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::sidecar::FileLearnedSidecarOutcome;
/// fn approve(history: FileLearnedSidecarOutcome) -> Permit { history }
/// ```
#[derive(Clone, Debug)]
pub struct FileLearnedSidecarOutcome {
    pub attempt: u64,
    pub actor_revision: u64,
    /// The reviewed input, before any refinement recorded by result.
    pub input_revision: u64,
    pub round: u64,
    pub result: FileLearnedSidecarFinish,
}

impl FileOversight {
    /// Return the original ACKNOWLEDGED result without recomputing or completing
    /// a round. Missing means no such retained completion, not a nonexecution
    /// proof. A faulted owner refuses rather than returning its older RAM cut.
    pub fn learned_sidecar_outcome(&self, round: u64) -> Result<FileLearnedSidecarOutcome, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.learned_sidecar_outcome(round)?.clone())
    }

    /// Verify one canonical image while a cooperating writer may stay active.
    /// Require the independently retained exact recipe before original replay.
    /// Original witnessed completion, not serialized assertions, supplies the
    /// returned result. All later events must also validate; a good prefix cannot
    /// hide a corrupt suffix. This performs no writer lock, cleanup, fence, role
    /// provisioning, clock refresh or external-effect replay. It returns a
    /// historical cut, NOT necessarily the latest concurrently published image.
    pub fn read_learned_sidecar_outcome(directory: impl AsRef<Path>, profile: &FileOversightProfile,
        expected: &FileLearnedConfig, round: u64) -> Result<FileLearnedSidecarOutcome, JournalError>
    {
        super::super::super::super::super::codec::validate_profile(&profile.delivery)?;
        let identity = storage::identity(directory.as_ref())?;
        let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
        let mut events = journal::decode(profile, &identity, &bytes)?;
        bind_history(&mut events, expected)?;
        let machine = Machine::replay(profile, &events)?;
        Ok(machine.learned_sidecar_outcome(round)?.clone())
    }
}
