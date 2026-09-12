//! Feed actual complete file observations into the original mandatory live gate.
//! The adapter owns only the original writer; there is no parallel state ledger.

use super::{EvidenceError, EvidenceFile, EvidenceSnapshot, EvidenceSourceStatus, FileEvidenceSource};
use crate::action::consequence::oversight::OversightBroker;
use crate::action::consequence::oversight::policy_state::{PolicyStateWriter, StateEvent, StateFrontier, StateLimits, StateSource};
use crate::Error;
use std::fmt;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PolicyFileStatus {
    pub file: EvidenceSourceStatus,
    pub source: StateSource,
    pub recorded_through: u64,
    pub closed: Option<StateFrontier>,
    pub needs_full_image: bool,
    pub fault: Option<Error>,
}

/// Installs the original broker gate once, before proposals, and keeps its only
/// writer outside the actor/helper. Loss of this owner drops that writer and
/// closes live snapshot eligibility in the ORIGINAL authority's read path.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::evidence_source::PolicyFileSource;
/// fn duplicate(source: PolicyFileSource) { let _ = source.clone(); }
/// ```
pub struct PolicyFileSource {
    file: FileEvidenceSource,
    writer: PolicyStateWriter,
    source: StateSource,
    sequence: u64,
    marker: u64,
    recorded_generation: Option<u64>,
    closed: Option<StateFrontier>,
    needs_full: bool,
    fault: Option<Error>,
}

impl fmt::Debug for PolicyFileSource {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("PolicyFileSource").field("status", &self.status()).finish_non_exhaustive()
    }
}

impl PolicyFileSource {
    /// This is trusted bootstrap, not an actor-controlled source registration.
    /// The file producer generation and capture-source incarnation are separate:
    /// one file version creates one local full-image event, not an invented log
    /// of every external event that occurred between reads.
    pub fn attach(
        file: FileEvidenceSource, broker: &mut OversightBroker,
        source_generation: u64, limits: StateLimits,
    ) -> Result<Self, Error> {
        let source = StateSource { scope: file.scope, source: file.source, generation: source_generation };
        let writer = broker.enable_policy_state(source, limits)?;
        Ok(Self { file, writer, source, sequence: 0, marker: 0, recorded_generation: None,
            closed: None, needs_full: true, fault: None })
    }

    pub fn status(&self) -> PolicyFileStatus {
        PolicyFileStatus { file: self.file.status(), source: self.source,
            recorded_through: self.sequence, closed: self.closed,
            needs_full_image: self.needs_full, fault: self.fault }
    }

    /// Explicit observation loss, not an empty complete state. Recovery requires
    /// a NEW full-image event even if the producer file's exact bytes are restored.
    /// This does not reset the file generation floor, event quotas or any rights.
    pub fn withdraw(&mut self) {
        self.writer.withdraw();
        self.closed = None;
        self.needs_full = true;
    }

    /// Reread the immutable version file, then record/close the corresponding
    /// image through the existing PolicyStateWriter. Never set complete=true on
    /// a false-complete document. A failed reader withdraws even a prior closure.
    /// An unchanged healthy reread does not spend another retained event slot.
    pub fn read(&mut self) -> Result<Rc<EvidenceSnapshot>, EvidenceError> {
        let result = self.read_inner();
        if result.is_err() { self.withdraw(); }
        result
    }

    fn read_inner(&mut self) -> Result<Rc<EvidenceSnapshot>, EvidenceError> {
        if let Some(fault) = self.fault { return Err(fault.into()); }
        let observed = self.file.read()?;
        if !observed.snapshot().complete { return Err(Error::Incomplete.into()); }
        if !self.needs_full && self.recorded_generation == Some(observed.identity().generation) {
            return Ok(observed);
        }
        let next = match self.sequence.checked_add(1) {
            Some(next) => next,
            None => { self.fault = Some(Error::Overflow); return Err(Error::Overflow.into()); }
        };
        let marker = match self.marker.checked_add(1) {
            Some(marker) => marker,
            None => { self.fault = Some(Error::Overflow); return Err(Error::Overflow.into()); }
        };
        let event = StateEvent::Snapshot {
            semantic_epoch: observed.snapshot().semantic_epoch,
            values: observed.snapshot().values.clone(),
        };
        if let Err(error) = self.writer.record(next, &event) {
            self.fault = Some(error);
            return Err(error.into());
        }
        // Record progress before a later closure error. It is never rolled back
        // or retried as a new identity to hide missing/invalid source evidence.
        self.sequence = next;
        let frontier = match self.writer.close(next, marker) {
            Ok(frontier) => frontier,
            Err(error) => { self.fault = Some(error); return Err(error.into()); }
        };
        self.marker = marker;
        self.recorded_generation = Some(observed.identity().generation);
        self.closed = Some(frontier);
        self.needs_full = false;
        Ok(observed)
    }
}

impl super::sealed::Sealed for PolicyFileSource {}
impl EvidenceFile for PolicyFileSource {
    fn read_evidence(&mut self) -> Result<Rc<EvidenceSnapshot>, EvidenceError> { self.read() }
}
