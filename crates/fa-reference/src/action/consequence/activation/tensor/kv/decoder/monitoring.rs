//! Incremental learned-KV checks at the ORIGINAL decoder's publication boundary.
//! A private pending computation is audited before committing any cache layer,
//! token history, logits or original layer observations. Quiet remains numerical
//! evidence about frozen probes, not a harmfulness or external-effect permit.
use super::{BufferIdentity, ComputedLayers, DecoderBudget, DecoderModel, DecoderSession,
    DecoderStep, HostTensor, KvAppend, ModelKvBudget, ModelKvCapture, ModelKvImage, MAX_DECODER_PRODUCTS};
use super::super::model::MAX_MODEL_KV_VALUES;
use super::super::model::learned::{CompressionBudget, CompressionReport, GroupKey, LearnedKvCodec,
    MAX_COMPRESSION_WORK, MAX_LEARNED_IMAGE_BYTES};
use crate::action::consequence::activation::monitor::{MonitorOutcome, learned::{LearnedMonitorBudget,
    model::{LearnedAuditPreparationBudget, LearnedModelMonitor, LearnedModelReport}}};
use crate::action::consequence::activation::probe::learned::{CheckedKvBudget, CheckedLearnedKv, KvGroup, KvRow,
    ResidualRetention, MAX_CHECKED_KV_BYTES, MAX_CHECKED_KV_PRODUCTS};
use crate::Error;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

/// Structural head selection is fixed before the first token and applied at its
/// ORIGINAL absolute position. Missing escape blocks remain unresolved evidence.
#[derive(Clone, Debug)]
pub enum LearnedStreamRetention { None, All, Heads(BTreeSet<GroupKey>) }
impl LearnedStreamRetention {
    fn at(&self, position: u64) -> ResidualRetention {
        match self {
            Self::None => ResidualRetention::None,
            Self::All => ResidualRetention::All,
            Self::Heads(heads) => ResidualRetention::Groups(heads.iter().map(|head| KvGroup {
                row: KvRow { layer: head.layer, side: head.side, position }, head: head.head,
            }).collect()),
        }
    }
}

/// Per-attempt allowance supplied by a higher-level stream owner. Every field is
/// intersected with the policy's original cap; it can only reduce authority to
/// spend observation work. No caller can enlarge compression or monitoring.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LearnedDecoderAllowance {
    pub preparation: LearnedAuditPreparationBudget,
    pub monitoring: LearnedMonitorBudget,
}

/// Frozen codec, complete layer/K/V probe inventory and per-token admission caps.
/// No mutable accessor, retuning, skipped taps or per-call budget enlargement.
#[derive(Clone, Debug)]
pub struct LearnedDecoderPolicy {
    codec: LearnedKvCodec,
    monitor: LearnedModelMonitor,
    retention: LearnedStreamRetention,
    preparation: LearnedAuditPreparationBudget,
    inference: DecoderBudget,
}
impl LearnedDecoderPolicy {
    pub fn new(codec: LearnedKvCodec, monitor: LearnedModelMonitor, retention: LearnedStreamRetention,
        preparation: LearnedAuditPreparationBudget, inference: DecoderBudget) -> Result<Self, Error>
    {
        if codec.profile() != monitor.profile() { return Err(Error::Binding); }
        let compression = preparation.compression;
        let checked = preparation.source_check;
        if inference.scalar_products > MAX_DECODER_PRODUCTS
            || compression.source_values > MAX_MODEL_KV_VALUES || compression.encoded_bytes > MAX_LEARNED_IMAGE_BYTES
            || compression.work_units > MAX_COMPRESSION_WORK || checked.source_values > MAX_MODEL_KV_VALUES
            || checked.encoded_bytes > MAX_CHECKED_KV_BYTES || checked.reconstruction_products > MAX_CHECKED_KV_PRODUCTS
            || monitor.budget().rows < monitor.taps().len() { return Err(Error::Limit); }
        if let LearnedStreamRetention::Heads(heads) = &retention {
            if heads.iter().any(|head| !codec.groups().contains_key(head)) { return Err(Error::Missing); }
        }
        Ok(Self { codec, monitor, retention, preparation, inference })
    }
    pub fn codec(&self) -> &LearnedKvCodec { &self.codec }
    pub fn monitor(&self) -> &LearnedModelMonitor { &self.monitor }
    pub fn preparation(&self) -> LearnedAuditPreparationBudget { self.preparation }
    pub fn inference(&self) -> DecoderBudget { self.inference }
    pub(crate) fn retention_policy(&self) -> &LearnedStreamRetention { &self.retention }
    pub fn allowance(&self) -> LearnedDecoderAllowance {
        LearnedDecoderAllowance { preparation: self.preparation, monitoring: self.monitor.budget().monitoring }
    }

    fn restrict(&self, allowance: LearnedDecoderAllowance) -> LearnedDecoderAllowance {
        let fixed = self.allowance();
        LearnedDecoderAllowance {
            preparation: LearnedAuditPreparationBudget {
                compression: CompressionBudget {
                    source_values: fixed.preparation.compression.source_values.min(allowance.preparation.compression.source_values),
                    encoded_bytes: fixed.preparation.compression.encoded_bytes.min(allowance.preparation.compression.encoded_bytes),
                    work_units: fixed.preparation.compression.work_units.min(allowance.preparation.compression.work_units),
                },
                source_check: CheckedKvBudget {
                    source_values: fixed.preparation.source_check.source_values.min(allowance.preparation.source_check.source_values),
                    encoded_bytes: fixed.preparation.source_check.encoded_bytes.min(allowance.preparation.source_check.encoded_bytes),
                    reconstruction_products: fixed.preparation.source_check.reconstruction_products.min(allowance.preparation.source_check.reconstruction_products),
                },
            },
            monitoring: LearnedMonitorBudget {
                encoded_bytes: fixed.monitoring.encoded_bytes.min(allowance.monitoring.encoded_bytes),
                probe_coordinates: fixed.monitoring.probe_coordinates.min(allowance.monitoring.probe_coordinates),
                reconstruction_products: fixed.monitoring.reconstruction_products.min(allowance.monitoring.reconstruction_products),
                materialized_values: fixed.monitoring.materialized_values.min(allowance.monitoring.materialized_values),
                refinements: fixed.monitoring.refinements.min(allowance.monitoring.refinements),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LearnedDecoderStatus { Active, Held(MonitorOutcome), Failed(Error) }

/// Held events own their checked numerical evidence but NEVER pending logits or
/// unchecked query/residual observations. Only accepted events contain a step.
///
/// ```compile_fail,E0308
/// use fa_reference::action::{Permit, consequence::activation::tensor::kv::decoder::monitoring::LearnedDecoderEvent};
/// fn authorize(event: LearnedDecoderEvent) -> Permit { event }
/// ```
#[derive(Clone, Debug)]
pub struct LearnedDecoderEvent {
    token: u32,
    position: u64,
    compression: CompressionReport,
    audit: LearnedModelReport,
    step: Option<DecoderStep>,
}
impl LearnedDecoderEvent {
    pub fn token(&self) -> u32 { self.token }
    pub fn position(&self) -> u64 { self.position }
    pub fn compression(&self) -> &CompressionReport { &self.compression }
    pub fn audit(&self) -> &LearnedModelReport { &self.audit }
    pub fn step(&self) -> Option<&DecoderStep> { self.step.as_ref() }
}

/// Owns an initially empty original session. It cannot adopt an unaudited prefix,
/// expose a mutable decoder, or resume past a latched hold. Not Clone: old event
/// snapshots share observations but cannot fork or reset this execution path.
#[derive(Debug)]
pub struct LearnedDecoderSession {
    session: DecoderSession,
    evaluation_origin: u64,
    policy: LearnedDecoderPolicy,
    status: LearnedDecoderStatus,
    last_event: Option<Rc<LearnedDecoderEvent>>,
}
impl DecoderModel {
    pub fn monitored_session(&self, stream: u64, evaluation_origin: u64, policy: LearnedDecoderPolicy)
        -> Result<LearnedDecoderSession, Error>
    {
        if self.cache_profile() != policy.codec.profile() { return Err(Error::Binding); }
        if evaluation_origin == 0 { return Err(Error::InvalidInput); }
        if policy.codec.fit_report().sources.contains_key(&evaluation_origin) { return Err(Error::Duplicate); }
        let session = self.session(stream)?;
        // This one EMPTY metadata snapshot checks the original declared split;
        // no successful prefix is ever snapshotted by the incremental hot path.
        if policy.codec.training_source_overlap(&session.cache_image()?) { return Err(Error::Duplicate); }
        Ok(LearnedDecoderSession { session, evaluation_origin, policy,
            status: LearnedDecoderStatus::Active, last_event: None })
    }

    fn staged_image(&self, stream: u64, position: u64, staged: &ComputedLayers) -> Result<ModelKvImage, Error> {
        let sequence = position.checked_add(1).ok_or(Error::Overflow)?;
        let data = &self.data;
        let mut capture = ModelKvCapture::new(data.cache.clone(), stream, 0, position, sequence,
            ModelKvBudget { positions: 1, normalized_values: data.cache.values_per_token() })?;
        let requests = staged.iter().map(|(id, keys, values)| {
            let contract = &data.cache.layers()[id];
            (*id, KvAppend {
                keys: HostTensor { identity: BufferIdentity { object: contract.keys().profile().tap, generation: sequence },
                    layout: &data.key_layout, bytes: keys },
                values: HostTensor { identity: BufferIdentity { object: contract.values().profile().tap, generation: sequence },
                    layout: &data.value_layout, bytes: values },
                first_token: 0, token_count: 1, buffer_first_position: position, first_sequence: sequence,
            })
        }).collect::<BTreeMap<_, _>>();
        capture.append(capture.revision(), requests)?;
        capture.snapshot(capture.revision())
    }
}
impl LearnedDecoderSession {
    pub fn policy(&self) -> &LearnedDecoderPolicy { &self.policy }
    pub fn evaluation_origin(&self) -> u64 { self.evaluation_origin }
    pub fn status(&self) -> LearnedDecoderStatus { self.status }
    pub fn position(&self) -> u64 { self.session.position() }
    pub fn accepted_tokens(&self) -> &[u32] { self.session.tokens() }
    /// Last ACCEPTED logits remain inspectable after a hold, never pending ones.
    pub fn accepted_logits(&self) -> Result<&[f32], Error> { self.session.logits() }
    /// Explicit diagnostic export, not used by advance or a resumable guard.
    pub fn accepted_cache_image(&self) -> Result<ModelKvImage, Error> { self.session.cache_image() }
    pub fn last_event(&self) -> Option<&LearnedDecoderEvent> { self.last_event.as_deref() }

    pub fn advance(&mut self, expected_position: u64, token: u32) -> Result<Rc<LearnedDecoderEvent>, Error> {
        self.advance_with_allowance(expected_position, token, self.policy.allowance())
    }

    /// Higher-level stream owners can conserve one budget across many tokens by
    /// supplying the remaining allowance here. Stale/invalid calls remain free;
    /// once real computation begins, any failure still latches this owner.
    pub fn advance_with_allowance(&mut self, expected_position: u64, token: u32,
        allowance: LearnedDecoderAllowance) -> Result<Rc<LearnedDecoderEvent>, Error>
    {
        if self.status != LearnedDecoderStatus::Active { return Err(Error::WrongState); }
        if expected_position != self.position() { return Err(Error::Stale); }
        if token as usize >= self.session.model.profile().shape().vocabulary { return Err(Error::InvalidInput); }
        self.status = LearnedDecoderStatus::Failed(Error::Incomplete);
        match self.advance_inner(token, self.policy.restrict(allowance)) {
            Ok(event) => {
                self.status = if event.step.is_some() { LearnedDecoderStatus::Active }
                    else { LearnedDecoderStatus::Held(event.audit.outcome()) };
                let event = Rc::new(event);
                self.last_event = Some(Rc::clone(&event));
                Ok(event)
            }
            Err(error) => { self.status = LearnedDecoderStatus::Failed(error); Err(error) }
        }
    }

    fn advance_inner(&mut self, token: u32, allowance: LearnedDecoderAllowance) -> Result<LearnedDecoderEvent, Error> {
        let position = self.position();
        let work = self.session.model.estimate(self.session.tokens.len(), 1)?;
        work.check(self.policy.inference)?;
        let next_work = self.session.work.add(work)?;
        let computed = self.session.model.forward_token(&self.session.cache, self.session.stream, position, token, true)?;
        let source = self.session.model.staged_image(self.session.stream, position, &computed.staged)?;
        let (image, compression) = self.policy.codec.evaluate_held_out(self.evaluation_origin, &source,
            allowance.preparation.compression)?;
        let checked = CheckedLearnedKv::new(image, &source, self.policy.retention.at(position),
            allowance.preparation.source_check)?;
        let audit = self.policy.monitor.analyze_with_budget(&checked, allowance.monitoring)?;
        // These are the same original computed bytes, never reconstructed KV.
        // The original all-layer publication transaction remains the only commit.
        let step = if audit.complete_quiet() {
            Some(self.session.publish_computed(token, work, next_work, computed)?)
        } else { None };
        Ok(LearnedDecoderEvent { token, position, compression, audit, step })
    }
}
