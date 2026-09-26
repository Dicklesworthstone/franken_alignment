//! Re-audit a sealed original prefix one position at a time, before exact restart.
//! No prefix inference, skipped position, mutable policy or saved quiet verdict.
use super::{MonitoredKvCheckpoint, RestartAudit, LearnedDecoderAllowance,
    LearnedDecoderSession, DecoderRestoreBudget, DecoderRestoreReceipt, CheckedLearnedKv,
    ModelKvImage, MAX_MODEL_KV_VALUES};
use crate::action::consequence::activation::monitor::MonitorOutcome;
use crate::action::consequence::activation::tensor::kv::{MAX_KV_POSITIONS, model::ModelKvDescriptor};
use crate::Error;
use std::fmt;
use std::rc::Rc;

/// A fixed number of audit calls, each bounded by the ORIGINAL per-token policy
/// intersected with `per_position`. The complete prefix must fit before work.
/// `reservation()` reports the conservative whole-prefix allowance; unused work
/// is not available for retries or additional positions in this verifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IncrementalRestartBudget {
    pub cache_values: usize,
    pub positions: usize,
    pub per_position: LearnedDecoderAllowance,
}

/// Logical work ceilings or reported costs, not elapsed time or peak memory.
/// Each field is counted independently; byte counts are not resident-byte sums.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RestartAuditCost {
    pub compression_source_values: u64,
    pub compression_encoded_bytes: u64,
    pub compression_work_units: u64,
    pub source_check_values: u64,
    pub source_check_encoded_bytes: u64,
    pub source_check_reconstruction_products: u64,
    pub monitor_encoded_bytes: u64,
    pub monitor_probe_coordinates: u64,
    pub monitor_reconstruction_products: u64,
    pub monitor_materialized_values: u64,
    pub monitor_refinements: u64,
}
impl RestartAuditCost {
    fn reserve(allowance: LearnedDecoderAllowance, positions: usize) -> Result<Self, Error> {
        let n = u64::try_from(positions).map_err(|_| Error::Overflow)?;
        let mul = |value: u64| value.checked_mul(n).ok_or(Error::Overflow);
        let count = |value: usize| mul(u64::try_from(value).map_err(|_| Error::Overflow)?);
        let p = allowance.preparation;
        let m = allowance.monitoring;
        Ok(Self {
            compression_source_values: count(p.compression.source_values)?,
            compression_encoded_bytes: count(p.compression.encoded_bytes)?,
            compression_work_units: mul(p.compression.work_units)?,
            source_check_values: count(p.source_check.source_values)?,
            source_check_encoded_bytes: count(p.source_check.encoded_bytes)?,
            source_check_reconstruction_products: mul(p.source_check.reconstruction_products)?,
            monitor_encoded_bytes: count(m.encoded_bytes)?,
            monitor_probe_coordinates: count(m.probe_coordinates)?,
            monitor_reconstruction_products: mul(m.reconstruction_products)?,
            monitor_materialized_values: count(m.materialized_values)?,
            monitor_refinements: count(m.refinements)?,
        })
    }
    fn from_audit(audit: &RestartAudit) -> Result<Self, Error> {
        let checked = audit.monitoring().source().report();
        let monitor = audit.monitoring().work();
        let count = |value: usize| u64::try_from(value).map_err(|_| Error::Overflow);
        Ok(Self {
            compression_source_values: count(checked.source_values)?,
            compression_encoded_bytes: count(audit.compression().encoded_bytes)?,
            compression_work_units: audit.compression().work_units_reserved,
            source_check_values: count(checked.source_values)?,
            source_check_encoded_bytes: count(checked.total_encoded_bytes)?,
            source_check_reconstruction_products: checked.reconstruction_products,
            monitor_encoded_bytes: count(monitor.encoded_bytes)?,
            monitor_probe_coordinates: count(monitor.probe_coordinates)?,
            monitor_reconstruction_products: monitor.reconstruction_products,
            monitor_materialized_values: count(monitor.materialized_values)?,
            monitor_refinements: count(monitor.refinements)?,
        })
    }
    fn add(self, other: Self) -> Result<Self, Error> {
        let add = |a: u64, b: u64| a.checked_add(b).ok_or(Error::Overflow);
        Ok(Self {
            compression_source_values: add(self.compression_source_values, other.compression_source_values)?,
            compression_encoded_bytes: add(self.compression_encoded_bytes, other.compression_encoded_bytes)?,
            compression_work_units: add(self.compression_work_units, other.compression_work_units)?,
            source_check_values: add(self.source_check_values, other.source_check_values)?,
            source_check_encoded_bytes: add(self.source_check_encoded_bytes, other.source_check_encoded_bytes)?,
            source_check_reconstruction_products: add(self.source_check_reconstruction_products, other.source_check_reconstruction_products)?,
            monitor_encoded_bytes: add(self.monitor_encoded_bytes, other.monitor_encoded_bytes)?,
            monitor_probe_coordinates: add(self.monitor_probe_coordinates, other.monitor_probe_coordinates)?,
            monitor_reconstruction_products: add(self.monitor_reconstruction_products, other.monitor_reconstruction_products)?,
            monitor_materialized_values: add(self.monitor_materialized_values, other.monitor_materialized_values)?,
            monitor_refinements: add(self.monitor_refinements, other.monitor_refinements)?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IncrementalRestartStatus {
    Auditing,
    Verified,
    Held(MonitorOutcome),
    Failed(Error),
}

/// Completed reports include non-quiet outcomes. A failed preparation may have
/// bounded unreported work, so attempted positions are charged BEFORE it starts
/// and the owner then fails permanently. The initial reservation never shrinks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IncrementalRestartWork {
    pub attempted_positions: usize,
    pub reported_positions: usize,
    pub source_values_recaptured: usize,
    pub quiet_positions: usize,
    pub reported: RestartAuditCost,
}

impl MonitoredKvCheckpoint {
    /// The original row cap now applies to one position's COMPLETE tap roster,
    /// not every position at once. No caller-selected windows or omissions exist.
    /// The destination is admitted first and stays private until all rows verify.
    pub fn begin_incremental_restart(&self, resumed_stream: u64, budget: IncrementalRestartBudget)
        -> Result<IncrementalKvRestart, Error>
    {
        if resumed_stream == 0 || resumed_stream == self.stream() { return Err(Error::InvalidInput); }
        let positions = self.decoder.tokens().len();
        if budget.cache_values > MAX_MODEL_KV_VALUES || self.cache_values() > budget.cache_values
            || budget.positions > MAX_KV_POSITIONS || positions > budget.positions { return Err(Error::Limit); }
        let destination = self.decoder.model().monitored_session(resumed_stream,
            self.evaluation_origin, self.policy.clone())?;
        let allowance = self.policy.restrict(budget.per_position);
        let reservation = RestartAuditCost::reserve(allowance, positions)?;
        let status = if positions == 0 { IncrementalRestartStatus::Verified }
            else { IncrementalRestartStatus::Auditing };
        Ok(IncrementalKvRestart { checkpoint: self.clone(), destination,
            restore: DecoderRestoreBudget { cache_values: budget.cache_values }, allowance,
            reservation, status, work: IncrementalRestartWork::default(), last_audit: None })
    }

    // Recapture exactly ONE retained original position through the same all-layer
    // path used by incremental inference audits. Only exact f32 bits are copied.
    // Stream/sequence/position remain original; revision 1 identifies this derived
    // one-position capture, NOT the original full-prefix snapshot revision. The
    // verifier and final receipt retain that full original descriptor separately.
    fn audit_position(&self, position: u64) -> Result<ModelKvImage, Error> {
        if position >= self.position() { return Err(Error::Missing); }
        let source = self.decoder.cache();
        let mut staged = Vec::new();
        staged.try_reserve_exact(source.profile().layers().len()).map_err(|_| Error::Limit)?;
        for id in source.profile().layers().keys() {
            let token = source.layer(*id)?.token(position)?;
            staged.push((*id, exact_bytes(&token.key().words)?, exact_bytes(&token.value().words)?));
        }
        self.decoder.model().staged_image(self.stream(), position, &staged)
    }
}
fn exact_bytes(words: &[u32]) -> Result<Vec<u8>, Error> {
    let length = words.len().checked_mul(4).ok_or(Error::Overflow)?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|_| Error::Limit)?;
    for word in words { bytes.extend_from_slice(&word.to_le_bytes()); }
    Ok(bytes)
}

/// Non-cloneable, ordered verifier. It retains the sealed prefix and at most its
/// last fresh audit, not an ever-growing second copy of all checked residuals.
/// Pausing between calls is process-local; dropping it aborts without a session.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::monitoring::restart::incremental::IncrementalKvRestart;
/// fn bypass(restart: &mut IncrementalKvRestart) { restart.session_mut(); }
/// ```
pub struct IncrementalKvRestart {
    checkpoint: MonitoredKvCheckpoint,
    destination: LearnedDecoderSession,
    restore: DecoderRestoreBudget,
    allowance: LearnedDecoderAllowance,
    reservation: RestartAuditCost,
    status: IncrementalRestartStatus,
    work: IncrementalRestartWork,
    last_audit: Option<Rc<RestartAudit>>,
}
impl fmt::Debug for IncrementalKvRestart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IncrementalKvRestart").field("position", &self.next_position())
            .field("status", &self.status).field("work", &self.work).finish_non_exhaustive()
    }
}
impl IncrementalKvRestart {
    pub fn status(&self) -> IncrementalRestartStatus { self.status }
    pub fn next_position(&self) -> u64 { self.work.quiet_positions as u64 }
    pub fn position_count(&self) -> usize { self.checkpoint.decoder.tokens().len() }
    pub fn work(&self) -> IncrementalRestartWork { self.work }
    pub fn reservation(&self) -> RestartAuditCost { self.reservation }
    pub fn source(&self) -> ModelKvDescriptor { self.checkpoint.decoder.cache().descriptor() }
    pub fn last_audit(&self) -> Option<&RestartAudit> { self.last_audit.as_deref() }
    pub fn is_ready(&self) -> bool {
        self.status == IncrementalRestartStatus::Verified && self.work.quiet_positions == self.position_count()
    }

    pub fn advance(&mut self, expected_position: u64) -> Result<Rc<RestartAudit>, Error> {
        if self.status != IncrementalRestartStatus::Auditing { return Err(Error::WrongState); }
        if expected_position != self.next_position() { return Err(Error::Stale); }
        self.status = IncrementalRestartStatus::Failed(Error::Incomplete);
        self.last_audit = None;
        self.work.attempted_positions = self.work.attempted_positions.checked_add(1).ok_or(Error::Overflow)?;
        match self.audit_next() {
            Ok(audit) => {
                self.status = if audit.monitoring().complete_quiet() {
                    self.work.quiet_positions += 1;
                    if self.work.quiet_positions == self.position_count() { IncrementalRestartStatus::Verified }
                    else { IncrementalRestartStatus::Auditing }
                } else { IncrementalRestartStatus::Held(audit.monitoring().outcome()) };
                let audit = Rc::new(audit);
                self.last_audit = Some(Rc::clone(&audit));
                Ok(audit)
            }
            Err(error) => { self.status = IncrementalRestartStatus::Failed(error); Err(error) }
        }
    }

    fn audit_next(&mut self) -> Result<RestartAudit, Error> {
        let position = self.next_position();
        let source = self.checkpoint.audit_position(position)?;
        self.work.source_values_recaptured = self.work.source_values_recaptured
            .checked_add(source.normalized_values()).ok_or(Error::Overflow)?;
        let policy = &self.checkpoint.policy;
        let (image, compression) = policy.codec.evaluate_held_out(self.checkpoint.evaluation_origin,
            &source, self.allowance.preparation.compression)?;
        let checked = CheckedLearnedKv::new(image, &source, policy.retention.at(position),
            self.allowance.preparation.source_check)?;
        let monitoring = policy.monitor.analyze_with_budget(&checked, self.allowance.monitoring)?;
        let audit = RestartAudit { compression, monitoring };
        self.work.reported = self.work.reported.add(RestartAuditCost::from_audit(&audit)?)?;
        self.work.reported_positions += 1;
        Ok(audit)
    }

    /// Release only after EVERY original position's complete fresh audit. The
    /// original all-layer writer/recapture still restores exact KV transactionally.
    /// No saved verdict, partial frontier or approximate reconstruction is used.
    pub fn finish(self) -> Result<(LearnedDecoderSession, IncrementalKvRestartReceipt), Error> {
        if !self.is_ready() { return Err(Error::Incomplete); }
        let Self { checkpoint, mut destination, restore, reservation, work, .. } = self;
        let (session, restoration) = checkpoint.decoder.model().restore_checkpoint(&checkpoint.decoder,
            destination.session.stream, restore)?;
        destination.session = session;
        Ok((destination, IncrementalKvRestartReceipt { restoration, reservation, work,
            evaluation_origin: checkpoint.evaluation_origin }))
    }
}

/// Cumulative fresh audit work and the exact original restore receipt. Empty
/// prefixes have zero audit counts, never a fabricated quiet observation.
#[derive(Clone, Debug)]
pub struct IncrementalKvRestartReceipt {
    restoration: DecoderRestoreReceipt,
    reservation: RestartAuditCost,
    work: IncrementalRestartWork,
    evaluation_origin: u64,
}
impl IncrementalKvRestartReceipt {
    pub fn restoration(&self) -> &DecoderRestoreReceipt { &self.restoration }
    pub fn reservation(&self) -> RestartAuditCost { self.reservation }
    pub fn work(&self) -> IncrementalRestartWork { self.work }
    pub fn evaluation_origin(&self) -> u64 { self.evaluation_origin }
}
