//! Direct exact-KV restart of a typed, originally monitored numerical prefix.
//!
//! A fresh COMPLETE audit of the retained original cache must finish quietly
//! before the original all-layer restorer can release a new monitored session.
//! No imported image, saved verdict, candidate policy or external effect enters.
use super::{LearnedDecoderAllowance, LearnedDecoderPolicy, LearnedDecoderSession,
    LearnedDecoderStatus, LearnedStreamRetention, CompressionReport, CheckedLearnedKv,
    KvGroup, KvRow, ResidualRetention, ModelKvImage, MAX_MODEL_KV_VALUES};
use super::super::{DecoderCheckpoint, DecoderRestoreBudget, DecoderRestoreReceipt};
use crate::action::consequence::activation::monitor::learned::model::LearnedModelReport;
use crate::Error;
use std::collections::BTreeSet;
use std::fmt;

/// Fresh restore/audit work, independent of a generation's conserved lifetime
/// allowances. Audit ceilings are also intersected with the ORIGINAL policy.
/// Full-prefix auditing may need more capacity than one incremental observation;
/// insufficient capacity refuses restart rather than silently skipping rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KvRestartBudget {
    pub cache_values: usize,
    pub audit: LearnedDecoderAllowance,
}

/// Original model, exact full cache/logits and frozen learned policy. Construction
/// is possible only from an active original guard, never an image or a held
/// guard's shorter accepted prefix. Cloning shares numerical data, not authority.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::monitoring::restart::MonitoredKvCheckpoint;
/// fn import(bytes: &[u8]) { let _ = MonitoredKvCheckpoint::decode(bytes); }
/// ```
#[derive(Clone)]
pub struct MonitoredKvCheckpoint {
    decoder: DecoderCheckpoint,
    policy: LearnedDecoderPolicy,
    evaluation_origin: u64,
}
impl fmt::Debug for MonitoredKvCheckpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MonitoredKvCheckpoint").field("stream", &self.stream())
            .field("position", &self.position()).finish_non_exhaustive()
    }
}
impl LearnedDecoderSession {
    /// Captures the original complete accepted state; no inference is repeated.
    /// The cache-value limit is checked before materializing checkpoint metadata.
    pub fn checkpoint_kv(&self, limits: DecoderRestoreBudget) -> Result<MonitoredKvCheckpoint, Error> {
        if self.status != LearnedDecoderStatus::Active { return Err(Error::WrongState); }
        if limits.cache_values > MAX_MODEL_KV_VALUES
            || self.session.cache.normalized_values() > limits.cache_values { return Err(Error::Limit); }
        Ok(MonitoredKvCheckpoint { decoder: self.session.checkpoint()?,
            policy: self.policy.clone(), evaluation_origin: self.evaluation_origin })
    }
}
impl MonitoredKvCheckpoint {
    pub fn stream(&self) -> u64 { self.decoder.stream() }
    pub fn position(&self) -> u64 { self.decoder.tokens().len() as u64 }
    pub fn cache_values(&self) -> usize { self.decoder.cache().normalized_values() }
    pub fn evaluation_origin(&self) -> u64 { self.evaluation_origin }
    pub fn policy(&self) -> &LearnedDecoderPolicy { &self.policy }

    /// Fresh compression, source checking and complete original monitor roster.
    /// The source still has its ORIGINAL lineage. The destination's new stream
    /// identifies derived buffers; it does not create independent observations.
    /// A blocked audit remains inspectable, but its preparation cannot finish.
    pub fn begin_restart(&self, resumed_stream: u64, budget: KvRestartBudget)
        -> Result<MonitoredKvRestart, Error>
    {
        if resumed_stream == 0 || resumed_stream == self.stream() { return Err(Error::InvalidInput); }
        if budget.cache_values > MAX_MODEL_KV_VALUES || self.cache_values() > budget.cache_values {
            return Err(Error::Limit);
        }
        // Original admission rejects a foreign/training destination BEFORE
        // audit work. No existing session or arbitrary cache is adopted here.
        let destination = self.decoder.model().monitored_session(resumed_stream,
            self.evaluation_origin, self.policy.clone())?;
        let source = self.decoder.cache();
        let rows = source.len().checked_mul(self.policy.monitor.taps().len()).ok_or(Error::Overflow)?;
        if rows > self.policy.monitor.budget().rows { return Err(Error::Limit); }
        let allowance = self.policy.restrict(budget.audit);
        let audit = if source.is_empty() { None } else {
            let (image, compression) = self.policy.codec.evaluate_held_out(self.evaluation_origin,
                source, allowance.preparation.compression)?;
            let retention = retained_groups(&self.policy.retention, source)?;
            let checked = CheckedLearnedKv::new(image, source, retention, allowance.preparation.source_check)?;
            let monitoring = self.policy.monitor.analyze_with_budget(&checked, allowance.monitoring)?;
            Some(RestartAudit { compression, monitoring })
        };
        Ok(MonitoredKvRestart { checkpoint: self.clone(), destination,
            restore: DecoderRestoreBudget { cache_values: budget.cache_values }, audit })
    }
}

fn retained_groups(policy: &LearnedStreamRetention, source: &ModelKvImage) -> Result<ResidualRetention, Error> {
    match policy {
        LearnedStreamRetention::None => Ok(ResidualRetention::None),
        LearnedStreamRetention::All => Ok(ResidualRetention::All),
        LearnedStreamRetention::Heads(heads) => {
            // Same structural head selection at EVERY original absolute position.
            // No caller-controlled omission and no extra residual promotion.
            let groups = heads.len().checked_mul(source.len()).ok_or(Error::Overflow)?;
            if groups > MAX_MODEL_KV_VALUES { return Err(Error::Limit); }
            let mut retained = BTreeSet::new();
            let first = source.descriptor().layers().values().next().ok_or(Error::Incomplete)?.first_position;
            let end = first.checked_add(source.len() as u64).ok_or(Error::Overflow)?;
            for position in first..end {
                for head in heads {
                    retained.insert(KvGroup { row: KvRow { layer: head.layer, side: head.side, position }, head: head.head });
                }
            }
            Ok(ResidualRetention::Groups(retained))
        }
    }
}

/// Newly computed evidence over the original exact prefix, not saved success.
#[derive(Clone, Debug)]
pub struct RestartAudit {
    compression: CompressionReport,
    monitoring: LearnedModelReport,
}
impl RestartAudit {
    pub fn compression(&self) -> &CompressionReport { &self.compression }
    pub fn monitoring(&self) -> &LearnedModelReport { &self.monitoring }
}

/// A new session is private until the fresh full-prefix audit is quiet AND
/// original transactional restoration succeeds. No mutable candidate accessor.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::monitoring::restart::MonitoredKvRestart;
/// fn bypass(restart: &mut MonitoredKvRestart) { restart.session_mut(); }
/// ```
pub struct MonitoredKvRestart {
    checkpoint: MonitoredKvCheckpoint,
    destination: LearnedDecoderSession,
    restore: DecoderRestoreBudget,
    audit: Option<RestartAudit>,
}
impl fmt::Debug for MonitoredKvRestart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MonitoredKvRestart").field("position", &self.checkpoint.position())
            .field("ready", &self.is_ready()).finish_non_exhaustive()
    }
}
impl MonitoredKvRestart {
    pub fn audit(&self) -> Option<&RestartAudit> { self.audit.as_ref() }
    /// Empty-prefix readiness is admission only, not a quiet observation.
    pub fn is_ready(&self) -> bool {
        match &self.audit {
            Some(audit) => audit.monitoring.complete_quiet(),
            None => self.checkpoint.position() == 0,
        }
    }
    pub fn finish(self) -> Result<(LearnedDecoderSession, MonitoredKvRestartReceipt), Error> {
        if !self.is_ready() { return Err(Error::Incomplete); }
        let Self { checkpoint, mut destination, restore, audit } = self;
        let (session, restoration) = checkpoint.decoder.model().restore_checkpoint(&checkpoint.decoder,
            destination.session.stream, restore)?;
        // Use the original restored decoder, then the original publication guard
        // on every NEW token. No last-event receipt is relabeled as a new event.
        destination.session = session;
        Ok((destination, MonitoredKvRestartReceipt { restoration, audit,
            evaluation_origin: checkpoint.evaluation_origin }))
    }
}

/// Restore bytes and fresh audit work, not historical inference reexecution.
/// No prefix matrix/attention products or random draws are performed by restart.
#[derive(Clone, Debug)]
pub struct MonitoredKvRestartReceipt {
    restoration: DecoderRestoreReceipt,
    audit: Option<RestartAudit>,
    evaluation_origin: u64,
}
impl MonitoredKvRestartReceipt {
    pub fn restoration(&self) -> &DecoderRestoreReceipt { &self.restoration }
    pub fn audit(&self) -> Option<&RestartAudit> { self.audit.as_ref() }
    pub fn evaluation_origin(&self) -> u64 { self.evaluation_origin }
}
