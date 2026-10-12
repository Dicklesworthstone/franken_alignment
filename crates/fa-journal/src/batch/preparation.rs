//! Yield at original event boundaries without exposing an incomplete candidate.
use super::{BatchLimits, Encoder};
use crate::{Error, Event, Journal};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchPreparationStage { Preparing, Ready, Failed }

/// Logical work, not an applied prefix, durable head or external-effect receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BatchPreparationReport {
    pub stage: BatchPreparationStage,
    pub total_records: usize,
    pub prepared_records: usize,
    /// Includes an event whose encoding or reducer attempt failed or unwound.
    pub attempted_records: usize,
}

#[derive(Debug)]
pub enum BatchPreparationError {
    Journal(Error),
    NotReady,
    Failed,
}
impl From<Error> for BatchPreparationError {
    fn from(error: Error) -> Self { Self::Journal(error) }
}
impl fmt::Display for BatchPreparationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "journal batch preparation: {self:?}")
    }
}
impl std::error::Error for BatchPreparationError {}

/// A frozen new batch and ONE private original-ledger clone. No live state,
/// encoded partial records, mutable events or prepared-state import is exposed.
/// Cancellation drops the private work; it cannot refund external effects.
///
/// ```compile_fail,E0599
/// use fa_journal::batch::BatchPreparation;
/// fn partial(run: &BatchPreparation) { let _ = run.rights(); }
/// ```
/// ```compile_fail,E0599
/// use fa_journal::batch::BatchPreparation;
/// fn fork(run: BatchPreparation) { let _ = run.clone(); }
/// ```
#[must_use = "finish the complete batch or cancel/drop its private preparation"]
pub struct BatchPreparation {
    encoder: Encoder,
    events: Vec<Event>,
    prepared: usize,
    attempted: usize,
    stage: BatchPreparationStage,
}
impl fmt::Debug for BatchPreparation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BatchPreparation").field("progress", &self.progress()).finish_non_exhaustive()
    }
}

impl Journal {
    /// Freeze a NEW ordered batch for cooperative in-memory preparation. Cheap
    /// whole-group bounds precede the initial ledger clone; no event runs here.
    /// Each advance uses the SAME encoder/reducer as synchronous append_batch.
    ///
    /// The source Journal is unchanged, even if a late event fails. Ready/finish
    /// means in-memory validity only, NOT durable commit or authority to send.
    /// The initial clone and a single bounded event's work are not time budgets.
    pub fn begin_append_batch(&self, events: Vec<Event>, limits: BatchLimits)
        -> Result<BatchPreparation, Error>
    {
        let encoder = Encoder::new(self, events.len(), limits)?;
        Ok(BatchPreparation { encoder, events, prepared: 0, attempted: 0,
            stage: BatchPreparationStage::Preparing })
    }
}

impl BatchPreparation {
    /// Attempt at most max_records original encodings/reductions. Zero performs
    /// no work. The cumulative record/byte/recovery limits never reset on yield.
    /// A failure or unwind latches Failed before an incomplete candidate escapes.
    pub fn advance(&mut self, max_records: usize)
        -> Result<BatchPreparationReport, BatchPreparationError>
    {
        match self.stage {
            BatchPreparationStage::Ready => return Ok(self.progress()),
            BatchPreparationStage::Failed => return Err(BatchPreparationError::Failed),
            BatchPreparationStage::Preparing => {}
        }
        let count = max_records.min(self.events.len() - self.prepared);
        for _ in 0..count {
            self.stage = BatchPreparationStage::Failed;
            self.attempted += 1; // Admission caps the entire owned list at 1,024.
            self.encoder.push(&self.events[self.prepared])?;
            self.prepared += 1;
            self.stage = if self.prepared == self.events.len() { BatchPreparationStage::Ready }
                else { BatchPreparationStage::Preparing };
        }
        Ok(self.progress())
    }

    pub fn progress(&self) -> BatchPreparationReport {
        BatchPreparationReport { stage: self.stage, total_records: self.events.len(),
            prepared_records: self.prepared, attempted_records: self.attempted }
    }

    /// Return the complete IN-MEMORY journal and canonical group. Persisting it
    /// is separate. There is no partial-state extraction on error or cancellation.
    pub fn finish(self) -> Result<(Journal, Vec<u8>), BatchPreparationError> {
        match self.stage {
            BatchPreparationStage::Ready => Ok(self.encoder.finish()),
            BatchPreparationStage::Preparing => Err(BatchPreparationError::NotReady),
            BatchPreparationStage::Failed => Err(BatchPreparationError::Failed),
        }
    }

    pub fn cancel(self) -> BatchPreparationReport { self.progress() }
}
