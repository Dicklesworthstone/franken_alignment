//! Pair an original learned-generation checkpoint with the existing authority.
//! The registered restart grade and pre-incident checkpoint selection are trusted
//! supervisor claims. This does not prove secrecy, cross-host fidelity or durability.
use super::{GenerationStatus, GenerationTelemetryWork, GenerationWork, LearnedAvailability,
    ObservedLearnedGeneration, OversightBroker};
use crate::action::consequence::activation::tensor::kv::{model::MAX_MODEL_KV_VALUES,
    decoder::{DecoderRestoreBudget, DecoderWork, monitoring::{LearnedDecoderAllowance,
        restart::{KvRestartBudget, RestartAudit}}, sampling::monitored::restart::{
            GenerationKvCheckpoint, GenerationKvRestartReceipt}}};
use crate::action::consequence::gate::{ReviewBinding, TargetCeiling};
use crate::action::consequence::gate::containment::{CheckpointHandle, ResetReceipt, ResetRequest,
    RestartGrade, MAX_CHECKPOINTS, MAX_RETAINED_STATE_BYTES};
use crate::Error;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;

/// References a private ORIGINAL numerical checkpoint paired with its original
/// authority checkpoint. It carries no cache, sampler, live source or permit.
#[derive(Clone)]
pub struct HostedLearnedCheckpointHandle { issuer: Rc<()>, id: u64 }
impl HostedLearnedCheckpointHandle { pub fn id(&self) -> u64 { self.id } }
impl fmt::Debug for HostedLearnedCheckpointHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HostedLearnedCheckpointHandle").field("id", &self.id).finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
pub struct HostedLearnedResetRequest {
    pub checkpoint: HostedLearnedCheckpointHandle,
    pub expected_control_sequence: u64,
    pub expected_actor_revision: u64,
    pub expected_authority_epoch: u64,
    pub binding: ReviewBinding,
    pub retained_targets: TargetCeiling,
    pub restart_budget: KvRestartBudget,
}

/// The original authority transition and fresh numerical audit/restoration.
/// Incident escalation can refuse restoration after completing the fresh audit.
#[derive(Clone, Debug)]
pub struct HostedLearnedResetReceipt {
    pub control: ResetReceipt,
    pub actor_revision: u64,
    pub resumed_stream: Option<u64>,
    pub position: u64,
    pub sampled_draws: u64,
    pub restart: GenerationKvRestartReceipt,
}

/// Logical bounded work, not allocator memory or wall-clock isolation. At most
/// MAX_CHECKPOINTS attempts use ORIGINAL intersected per-audit ceilings. Failed
/// or interrupted attempts never reclaim these reservations. Completed costs
/// count only original reports returned before a failure; unreported partial
/// work remains covered by the reservation and permanently failed owner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostedLearnedRecoveryUsage {
    pub checkpoints: usize,
    pub checkpoint_bytes: usize,
    pub restart_attempts: usize,
    pub reserved_cache_values: u64,
    pub reserved_audit: GenerationTelemetryWork,
    pub completed_audit: GenerationTelemetryWork,
    pub values_restored: u64,
    pub bytes_written: u64,
    pub bytes_recaptured: u64,
    pub staged_write_bytes: u64,
}

#[derive(Debug)]
struct PairedCheckpoint { control: CheckpointHandle, numerical: GenerationKvCheckpoint }
#[derive(Debug)]
pub(super) struct RecoveryState {
    issuer: Rc<()>,
    checkpoints: BTreeMap<u64, PairedCheckpoint>,
    usage: HostedLearnedRecoveryUsage,
    retired: GenerationWork,
    retired_telemetry: GenerationTelemetryWork,
    reset_rounds: BTreeSet<u64>,
}
impl RecoveryState {
    pub(super) fn new() -> Self {
        Self { issuer: Rc::new(()), checkpoints: BTreeMap::new(),
            usage: HostedLearnedRecoveryUsage::default(), retired: GenerationWork::default(),
            retired_telemetry: GenerationTelemetryWork::default(), reset_rounds: BTreeSet::new() }
    }
    pub(super) fn cumulative_work(&self, active: GenerationWork) -> Result<GenerationWork, Error> {
        generation_work(self.retired, active, add)
    }
    pub(super) fn cumulative_telemetry(&self, active: GenerationTelemetryWork)
        -> Result<GenerationTelemetryWork, Error>
    {
        telemetry(telemetry(self.retired_telemetry, active, add)?, self.usage.completed_audit, add)
    }
}

impl OversightBroker {
    pub fn hosted_learned_recovery_usage(&self) -> Result<HostedLearnedRecoveryUsage, Error> {
        Ok(self.learned_host.as_ref().ok_or(Error::Incomplete)?.recovery.usage)
    }

    /// Capture a nonempty active quiet original prefix, with exact equality
    /// to the authority's actor copy. An audit-only profile cannot create a pair.
    /// Fresh full-prefix restart has separate original policy-capacity checks;
    /// no stronger restart grade is inferred from successful serialization.
    pub fn capture_hosted_learned_checkpoint(&mut self, id: u64, expected_actor_revision: u64)
        -> Result<HostedLearnedCheckpointHandle, Error>
    {
        if self.enforce_learned_host_stop()?.is_some() || self.enforce_consistency_stop()?.is_some()
            || self.inspect().suspended || self.stop_receipt().is_some() { return Err(Error::WrongState); }
        if expected_actor_revision != self.actor_revision() { return Err(Error::Stale); }
        if id == 0 { return Err(Error::InvalidInput); }
        let host = self.learned_host.as_ref().ok_or(Error::Incomplete)?;
        if host.text_stream.is_some() { return Err(Error::Incomplete); }
        if host.fault.is_some() || host.run.position() == 0 || !host.run.status().is_active()
            || host.run.observation().availability() != LearnedAvailability::Ready
        { return Err(Error::WrongState); }
        if host.profile.grade == RestartGrade::AuditOnly { return Err(Error::Incomplete); }
        if host.recovery.checkpoints.contains_key(&id) { return Err(Error::Duplicate); }
        if host.recovery.checkpoints.len() >= MAX_CHECKPOINTS { return Err(Error::Limit); }
        let actor = host.run.capture_host_actor(host.profile)?;
        if &actor != self.delivery.controller().actor() { return Err(Error::Binding); }
        let original = host.run.original_generation();
        let numerical = original.checkpoint_kv(DecoderRestoreBudget { cache_values: MAX_MODEL_KV_VALUES })?;
        // Fixed model/policy objects are shared. Charge retained logical state
        // including histories, logits and the original sampler representation.
        let bytes = actor.tokens().len().checked_mul(4)
            .and_then(|n| n.checked_add(actor.cache().len()))
            .and_then(|n| n.checked_add(actor.sampler().len()))
            .and_then(|n| n.checked_add(original.accepted_logits().ok()?.len().checked_mul(4)?))
            // Token (4), stream/draw/random/probability (4*8), work counts (4*8).
            .and_then(|n| n.checked_add(original.samples().len().checked_mul(68)?))
            .and_then(|n| n.checked_add(original.spec().prompt().len().checked_mul(4)?))
            .and_then(|n| n.checked_add(original.spec().stop_tokens().len().checked_mul(4)?))
            .and_then(|n| n.checked_add(host.recovery.usage.checkpoint_bytes)).ok_or(Error::Limit)?;
        if bytes > MAX_RETAINED_STATE_BYTES { return Err(Error::Limit); }
        let control = self.delivery.capture_checkpoint(id, expected_actor_revision)?;
        let host = self.learned_host.as_mut().expect("validated learned owner");
        host.recovery.checkpoints.insert(id, PairedCheckpoint { control, numerical });
        host.recovery.usage.checkpoints += 1;
        host.recovery.usage.checkpoint_bytes = bytes;
        Ok(HostedLearnedCheckpointHandle { issuer: Rc::clone(&host.recovery.issuer), id })
    }

    /// A fresh complete ORIGINAL audit and exact KV restore precede the original
    /// containment reset. Old keys/epochs/approvals and abandoned continuation
    /// budgets never return. Stream publication owners require their separate
    /// frontier protocol and are deliberately not reset by this entry point.
    /// The destination begins without current proposal evidence. It must be an
    /// active checkpoint so a fresh accepted token can establish that evidence;
    /// a finished live continuation may still rewind to an earlier active pair.
    ///
    /// ```compile_fail,E0308
    /// use fa_reference::action::{Permit, consequence::oversight::learned_host::checkpoint::HostedLearnedCheckpointHandle};
    /// fn grant(saved: HostedLearnedCheckpointHandle) -> Permit { saved }
    /// ```
    pub fn reset_hosted_learned(&mut self, request: HostedLearnedResetRequest)
        -> Result<HostedLearnedResetReceipt, Error>
    {
        if self.enforce_learned_host_stop()?.is_some() || self.enforce_consistency_stop()?.is_some() {
            return Err(Error::WrongState);
        }
        let result = self.reset_hosted_learned_inner(request);
        if result.is_err() { self.enforce_learned_host_stop()?; }
        result
    }

    fn reset_hosted_learned_inner(&mut self, request: HostedLearnedResetRequest)
        -> Result<HostedLearnedResetReceipt, Error>
    {
        let inspection = self.inspect();
        if inspection.suspended || self.stop_receipt().is_some() { return Err(Error::WrongState); }
        if request.expected_control_sequence != inspection.sequence
            || request.expected_authority_epoch != inspection.ledger.epoch
            || request.expected_actor_revision != self.actor_revision() { return Err(Error::Stale); }
        request.expected_actor_revision.checked_add(2).ok_or(Error::Overflow)?;
        if request.binding.round == 0 || request.binding.reducer_generation == 0
            || request.binding.evidence_root == [0; 32] { return Err(Error::InvalidInput); }
        let host = self.learned_host.as_mut().ok_or(Error::Incomplete)?;
        if !Rc::ptr_eq(&request.checkpoint.issuer, &host.recovery.issuer) { return Err(Error::Binding); }
        if host.text_stream.is_some() { return Err(Error::Incomplete); }
        if host.fault.is_some() || matches!(host.run.status(), GenerationStatus::Failed(_))
            || !matches!(host.run.observation().availability(), LearnedAvailability::Ready | LearnedAvailability::Held)
        { return Err(Error::WrongState); }
        if self.started_rounds.contains(&request.binding.round)
            || host.recovery.reset_rounds.contains(&request.binding.round) { return Err(Error::Duplicate); }
        let saved = host.recovery.checkpoints.get(&request.checkpoint.id).ok_or(Error::Missing)?;
        if !saved.numerical.status().is_active() { return Err(Error::WrongState); }
        // A checkpoint ahead of the active logical timeline is not a rewind.
        generation_work(host.run.work(), saved.numerical.work(), subtract)?;
        telemetry(host.run.telemetry_work(), saved.numerical.telemetry_work(), subtract)?;
        if host.recovery.usage.restart_attempts >= MAX_CHECKPOINTS { return Err(Error::Limit); }
        if request.restart_budget.cache_values > MAX_MODEL_KV_VALUES
            || saved.numerical.cache_values() > request.restart_budget.cache_values { return Err(Error::Limit); }
        let rows = usize::try_from(saved.numerical.position()).map_err(|_| Error::Limit)?
            .checked_mul(saved.numerical.policy().monitor().taps().len()).ok_or(Error::Limit)?;
        if rows > saved.numerical.policy().monitor().budget().rows { return Err(Error::Limit); }
        let stream = host.run.observation().stream().checked_add(1).ok_or(Error::Overflow)?;
        let reserved = reservation(request.restart_budget.audit, saved.numerical.policy().allowance())?;
        let audit_total = telemetry(host.recovery.usage.reserved_audit, reserved, add)?;
        let cache_total = add(host.recovery.usage.reserved_cache_values, count(request.restart_budget.cache_values)?)?;
        let retired = generation_work(host.recovery.cumulative_work(host.run.work())?, saved.numerical.work(), subtract)?;
        let retired_telemetry = telemetry(telemetry(host.recovery.retired_telemetry,
            host.run.telemetry_work(), add)?, saved.numerical.telemetry_work(), subtract)?;
        let mut reset_rounds = host.recovery.reset_rounds.clone();
        reset_rounds.insert(request.binding.round);
        let numerical = saved.numerical.clone();
        let original = ResetRequest { checkpoint: saved.control.clone(),
            expected_control_sequence: request.expected_control_sequence,
            expected_actor_revision: request.expected_actor_revision,
            binding: request.binding, retained_targets: request.retained_targets };
        host.recovery.usage.restart_attempts += 1;
        host.recovery.usage.reserved_audit = audit_total;
        host.recovery.usage.reserved_cache_values = cache_total;
        host.fault = Some(Error::Incomplete);
        let guard = host.run.guard_host_restart();
        let prepared = (|| {
            let restart = numerical.begin_restart(stream, request.restart_budget)?;
            if let Some(audit) = restart.audit() {
                host.recovery.usage.completed_audit = telemetry(host.recovery.usage.completed_audit,
                    audit_work(audit)?, add)?;
            }
            let (mut generation, receipt) = restart.finish()?;
            let restoration = receipt.kv().restoration();
            // Stage all additions before publishing their completed-work tuple.
            let values = add(host.recovery.usage.values_restored, count(restoration.values_restored)?)?;
            let written = add(host.recovery.usage.bytes_written, count(restoration.bytes_written)?)?;
            let recaptured = add(host.recovery.usage.bytes_recaptured, count(restoration.bytes_recaptured)?)?;
            let staged = add(host.recovery.usage.staged_write_bytes, count(restoration.staged_write_bytes)?)?;
            host.recovery.usage.values_restored = values;
            host.recovery.usage.bytes_written = written;
            host.recovery.usage.bytes_recaptured = recaptured;
            host.recovery.usage.staged_write_bytes = staged;
            generation.conserve_host_continuation_budget(host.run.original_generation())?;
            let next = ObservedLearnedGeneration::from_host_restart(generation, &receipt, &host.run)?;
            let actor = next.capture_host_actor(host.profile)?;
            Ok::<_, Error>((next, actor, receipt))
        })();
        let (next, actor, restart) = match prepared {
            Ok(value) => value,
            Err(error) => { host.fault = Some(error); return Err(error); }
        };
        let source = next.observation();
        let result = (|| {
            self.decoder.as_ref().ok_or(Error::Incomplete)?.validate_learned_successor(&source)?;
            self.delivery.reset(original)
        })();
        let control = match result {
            Ok(receipt) => receipt,
            Err(error) => {
                self.learned_host.as_mut().expect("owned learned source").fault = Some(error);
                return Err(error);
            }
        };
        self.learned_host.as_mut().expect("owned learned source").recovery.reset_rounds = reset_rounds;
        for slot in self.inputs.values_mut() { slot.approved = None; }
        let resumed_stream = if control.restored {
            self.delivery.replace_actor_state(control.actor_revision, actor)
                .expect("prevalidated same-profile learned reset synchronization");
            self.decoder.as_mut().expect("validated learned gate").publish_learned_successor(source);
            let host = self.learned_host.as_mut().expect("owned learned source");
            host.run = next;
            host.recovery.retired = retired;
            host.recovery.retired_telemetry = retired_telemetry;
            Some(stream)
        } else { None };
        let host = self.learned_host.as_mut().expect("owned learned source");
        if control.restored {
            host.fault = None;
            guard.confirm();
        } else {
            host.fault = Some(Error::WrongState);
            // The original incident threshold suspended instead of restoring.
            // Dropping the guard leaves the abandoned source unavailable too.
            drop(guard);
        }
        Ok(HostedLearnedResetReceipt { control, actor_revision: self.actor_revision(), resumed_stream,
            position: self.learned_host.as_ref().expect("owned learned source").run.position(),
            sampled_draws: self.learned_host.as_ref().expect("owned learned source").run.sampled_draws(), restart })
    }
}

fn add(a: u64, b: u64) -> Result<u64, Error> { a.checked_add(b).ok_or(Error::Overflow) }
fn subtract(a: u64, b: u64) -> Result<u64, Error> { a.checked_sub(b).ok_or(Error::Stale) }
fn count(value: usize) -> Result<u64, Error> { u64::try_from(value).map_err(|_| Error::Overflow) }
type CounterOp = fn(u64, u64) -> Result<u64, Error>;

fn generation_work(a: GenerationWork, b: GenerationWork, op: CounterOp) -> Result<GenerationWork, Error> {
    let x = a.accepted_decoder; let y = b.accepted_decoder;
    Ok(GenerationWork {
        admitted_tokens: op(a.admitted_tokens, b.admitted_tokens)?,
        reserved_decoder_products: op(a.reserved_decoder_products, b.reserved_decoder_products)?,
        sampling_attempts: op(a.sampling_attempts, b.sampling_attempts)?,
        reserved_vocabulary_scores: op(a.reserved_vocabulary_scores, b.reserved_vocabulary_scores)?,
        accepted_decoder: DecoderWork { tokens: op(x.tokens, y.tokens)?,
            matrix_products: op(x.matrix_products, y.matrix_products)?,
            attention_products: op(x.attention_products, y.attention_products)?,
            attention_exponentials: op(x.attention_exponentials, y.attention_exponentials)?,
            normalization_coordinates: op(x.normalization_coordinates, y.normalization_coordinates)?,
            rotary_pairs: op(x.rotary_pairs, y.rotary_pairs)?, gate_coordinates: op(x.gate_coordinates, y.gate_coordinates)?,
            cache_values_appended: op(x.cache_values_appended, y.cache_values_appended)? },
    })
}

fn telemetry(a: GenerationTelemetryWork, b: GenerationTelemetryWork, op: CounterOp)
    -> Result<GenerationTelemetryWork, Error>
{
    Ok(GenerationTelemetryWork {
        compression_source_values: op(a.compression_source_values, b.compression_source_values)?,
        compression_encoded_bytes: op(a.compression_encoded_bytes, b.compression_encoded_bytes)?,
        compression_work_units: op(a.compression_work_units, b.compression_work_units)?,
        source_check_values: op(a.source_check_values, b.source_check_values)?,
        source_check_encoded_bytes: op(a.source_check_encoded_bytes, b.source_check_encoded_bytes)?,
        source_check_reconstruction_products: op(a.source_check_reconstruction_products, b.source_check_reconstruction_products)?,
        monitor_encoded_bytes: op(a.monitor_encoded_bytes, b.monitor_encoded_bytes)?,
        monitor_probe_coordinates: op(a.monitor_probe_coordinates, b.monitor_probe_coordinates)?,
        monitor_reconstruction_products: op(a.monitor_reconstruction_products, b.monitor_reconstruction_products)?,
        monitor_materialized_values: op(a.monitor_materialized_values, b.monitor_materialized_values)?,
        monitor_refinements: op(a.monitor_refinements, b.monitor_refinements)?,
    })
}

fn reservation(request: LearnedDecoderAllowance, fixed: LearnedDecoderAllowance)
    -> Result<GenerationTelemetryWork, Error>
{
    let r = request.preparation; let f = fixed.preparation;
    let m = request.monitoring; let n = fixed.monitoring;
    Ok(GenerationTelemetryWork {
        compression_source_values: count(r.compression.source_values.min(f.compression.source_values))?,
        compression_encoded_bytes: count(r.compression.encoded_bytes.min(f.compression.encoded_bytes))?,
        compression_work_units: r.compression.work_units.min(f.compression.work_units),
        source_check_values: count(r.source_check.source_values.min(f.source_check.source_values))?,
        source_check_encoded_bytes: count(r.source_check.encoded_bytes.min(f.source_check.encoded_bytes))?,
        source_check_reconstruction_products: r.source_check.reconstruction_products.min(f.source_check.reconstruction_products),
        monitor_encoded_bytes: count(m.encoded_bytes.min(n.encoded_bytes))?,
        monitor_probe_coordinates: count(m.probe_coordinates.min(n.probe_coordinates))?,
        monitor_reconstruction_products: m.reconstruction_products.min(n.reconstruction_products),
        monitor_materialized_values: count(m.materialized_values.min(n.materialized_values))?,
        monitor_refinements: count(m.refinements.min(n.refinements))?,
    })
}

fn audit_work(audit: &RestartAudit) -> Result<GenerationTelemetryWork, Error> {
    let checked = audit.monitoring().source().report(); let monitor = audit.monitoring().work();
    Ok(GenerationTelemetryWork {
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
