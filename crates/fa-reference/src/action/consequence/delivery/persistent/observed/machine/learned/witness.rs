//! Exact comparison-only words for the original learned source and actor copy.
//! No reader exists: these bytes can only be compared after original execution.
use super::{Machine, Transition};
use super::super::super::super::codec::shared::Writer;
use super::super::super::decoder::MAX_WITNESS_BYTES;
use crate::action::consequence::activation::{CaptureProfile, FrameIdentity};
use crate::action::consequence::activation::monitor::{MonitorOutcome, learned::{
    LearnedMonitorWork, model::LearnedModelReport,
}};
use crate::action::consequence::activation::monitor::decoder::sampled::host::replay::error_tag;
use crate::action::consequence::activation::probe::{ExactScore, ProbeIdentity, ProbeOutcome};
use crate::action::consequence::activation::probe::learned::{KvGroup, KvRow, LearnedKvView};
use crate::action::consequence::activation::tensor::kv::experiment::KvSide;
use crate::action::consequence::activation::tensor::kv::decoder::sampling::{SampledToken, monitored::{
    GenerationEvent, GenerationPhase, GenerationStatus, GenerationStop,
}};
use crate::action::consequence::oversight::learned_source::LearnedAvailability;
use crate::Error;
use std::cmp::Ordering;

impl Machine {
    pub(super) fn learned_witness(&self, result: &Transition) -> Result<Vec<u8>, Error> {
        let Transition::Learned(result) = result else { return Err(Error::Binding); };
        let mut w = Writer::new(MAX_WITNESS_BYTES);
        w.raw(b"FALSTEP\x01")?;
        match result {
            Ok(_) => w.u8(1)?,
            Err(error) => { w.u8(0)?; w.u8(error_tag(*error))?; }
        }
        self.write_learned_state(&mut w)?;
        Ok(w.finish())
    }

    // Shared comparison-only state; existing token witness bytes are unchanged.
    pub(super) fn write_learned_state(&self, w: &mut Writer) -> Result<(), Error> {
        let run = self.broker.hosted_learned_original()?;
        let actual = self.broker.hosted_learned_generation()?;
        for value in [actual.actor_revision, actual.position, actual.sampled_draws] { w.u64(value)?; }
        status(w, actual.status)?;
        w.u8(match actual.availability {
            LearnedAvailability::Empty => 0, LearnedAvailability::InProgress => 1,
            LearnedAvailability::Ready => 2, LearnedAvailability::Held => 3,
            LearnedAvailability::Failed => 4, LearnedAvailability::Closed => 5,
        })?;
        match actual.host_failure {
            None => w.u8(0)?, Some(error) => { w.u8(1)?; w.u8(error_tag(error))?; }
        }
        let g = run.work(); let d = g.accepted_decoder;
        for value in [g.admitted_tokens, g.reserved_decoder_products, g.sampling_attempts,
            g.reserved_vocabulary_scores, d.tokens, d.matrix_products, d.attention_products,
            d.attention_exponentials, d.normalization_coordinates, d.rotary_pairs,
            d.gate_coordinates, d.cache_values_appended] { w.u64(value)?; }
        let t = run.telemetry_work();
        for value in [t.compression_source_values, t.compression_encoded_bytes, t.compression_work_units,
            t.source_check_values, t.source_check_encoded_bytes, t.source_check_reconstruction_products,
            t.monitor_encoded_bytes, t.monitor_probe_coordinates, t.monitor_reconstruction_products,
            t.monitor_materialized_values, t.monitor_refinements] { w.u64(value)?; }
        w.count(run.accepted_tokens().len())?;
        for token in run.accepted_tokens() { w.u32(*token)?; }
        w.blob(&run.sampler_state().encode())?;
        w.count(run.samples().len())?;
        for sample in run.samples() { write_sample(w, sample)?; }
        match run.accepted_logits() {
            Err(error) => { w.u8(0)?; w.u8(error_tag(error))?; }
            Ok(logits) => {
                w.u8(1)?; w.count(logits.len())?;
                for logit in logits { w.u32(logit.to_bits())?; }
            }
        }
        match run.accepted_cache_image() {
            Err(error) => { w.u8(0)?; w.u8(error_tag(error))?; }
            Ok(image) => {
                if image.descriptor().image_len()? > MAX_WITNESS_BYTES { return Err(Error::Limit); }
                w.u8(1)?; w.blob(&image.encode()?)?;
            }
        }
        // Actor synchronization is separately checked, including a failure after
        // numerical acceptance. A saved actor copy is NEVER installed by replay.
        let actor = self.broker.retained_actor_state();
        w.u64(actor.next_position())?; w.count(actor.tokens().len())?;
        for token in actor.tokens() { w.u32(*token)?; }
        w.blob(actor.cache())?; w.blob(actor.sampler())?;
        match run.last_event() {
            None => w.u8(0)?,
            Some(event) => { w.u8(1)?; write_event(w, event)?; }
        }
        w.u8(u8::from(self.learned_paused()))?;
        match self.broker.stop_receipt() {
            None => w.u8(0)?,
            Some(receipt) => {
                w.u8(1)?;
                let request = receipt.request();
                for value in [request.operation, request.expected_control_sequence, request.expected_authority_epoch,
                    receipt.control_sequence(), receipt.revocation_floor(), receipt.dispatcher_epoch(), receipt.refunded_units()]
                { w.u64(value)?; }
                w.count(receipt.cancelled().len())?;
                for id in receipt.cancelled() { w.u64(*id)?; }
            }
        }
        Ok(())
    }
}
pub(super) fn status(w: &mut Writer, value: GenerationStatus) -> Result<(), Error> {
    match value {
        GenerationStatus::Prefilling => w.u8(0),
        GenerationStatus::Generating => w.u8(1),
        GenerationStatus::Finished(GenerationStop::TokenLimit) => w.u8(2),
        GenerationStatus::Finished(GenerationStop::StopToken(token)) => { w.u8(3)?; w.u32(token) }
        GenerationStatus::Held(value) => { w.u8(4)?; outcome(w, value) }
        GenerationStatus::Failed(error) => { w.u8(5)?; w.u8(error_tag(error)) }
    }
}
fn outcome(w: &mut Writer, value: MonitorOutcome) -> Result<(), Error> {
    w.u8(match value {
        MonitorOutcome::NoAlarm => 0, MonitorOutcome::Alarm => 1,
        MonitorOutcome::AtThreshold => 2, MonitorOutcome::BudgetExhausted => 3,
        MonitorOutcome::Unresolved => 4,
    })
}
fn write_sample(w: &mut Writer, sample: &SampledToken) -> Result<(), Error> {
    w.u32(sample.token)?;
    for value in [sample.stream, sample.draw, sample.random_word, sample.probability.to_bits()] { w.u64(value)?; }
    for count in [sample.work.logits_scanned, sample.work.exponentials,
        sample.work.retained_candidates, sample.work.zero_weights] { w.count(count)?; }
    Ok(())
}
fn write_event(w: &mut Writer, event: &GenerationEvent) -> Result<(), Error> {
    w.u8(match event.phase() { GenerationPhase::Prompt => 0, GenerationPhase::Continuation => 1 })?;
    w.u64(event.position())?; status(w, event.status())?;
    match event.sample() {
        None => w.u8(0)?, Some(sample) => { w.u8(1)?; write_sample(w, sample)?; }
    }
    match event.accepted() {
        None => w.u8(0)?,
        Some(step) => {
            w.u8(1)?; w.u32(step.token)?; w.u64(step.position)?; w.count(step.layers.len())?;
            for layer in &step.layers {
                w.u64(layer.layer)?;
                w.blob(&layer.query.source().encode_initial(23)?)?;
                w.blob(&layer.residual.source().encode_initial(23)?)?;
            }
        }
    }
    // Descriptive compression-error aggregates are not a decision certificate.
    // Bind the actual checked representation and all monitor outcomes instead.
    w.count(event.compression().encoded_bytes)?;
    w.u64(event.compression().work_units_reserved)?;
    report(w, event.audit())
}
fn profile(w: &mut Writer, p: CaptureProfile) -> Result<(), Error> {
    for value in [p.tenant, p.model, p.model_generation, p.tap, p.layout_generation] { w.u64(value)?; }
    Ok(())
}
fn frame(w: &mut Writer, f: FrameIdentity) -> Result<(), Error> {
    profile(w, f.profile)?;
    for value in [f.stream, f.sequence, f.position] { w.u64(value)?; }
    Ok(())
}
fn probe(w: &mut Writer, p: ProbeIdentity) -> Result<(), Error> {
    w.u64(p.id)?; w.u64(p.generation)?; profile(w, p.profile)?; w.count(p.dimensions)
}
fn row(w: &mut Writer, row: KvRow) -> Result<(), Error> {
    w.u64(row.layer)?; w.u8(match row.side { KvSide::Key => 0, KvSide::Value => 1 })?; w.u64(row.position)
}
fn group(w: &mut Writer, g: KvGroup) -> Result<(), Error> { row(w, g.row)?; w.count(g.head) }
fn score(w: &mut Writer, value: &ExactScore) -> Result<(), Error> {
    w.u8(u8::from(value.sign() == Ordering::Less))?;
    for word in value.magnitude_words() { w.u64(*word)?; }
    Ok(())
}
fn work(w: &mut Writer, value: LearnedMonitorWork) -> Result<(), Error> {
    w.count(value.encoded_bytes)?; w.count(value.probe_coordinates)?;
    w.u64(value.reconstruction_products)?; w.count(value.materialized_values)?; w.count(value.refinements)
}
fn view(w: &mut Writer, value: &LearnedKvView) -> Result<(), Error> {
    w.u64(value.revision())?; w.count(value.materialized_values())?;
    w.count(value.refined_groups().count())?;
    for g in value.refined_groups() {
        group(w, g)?;
        let channels = value.source().channels(g)?;
        w.count(channels)?;
        for channel in 0..channels {
            let [lower, upper] = value.interval(g, channel)?;
            w.u32(lower.to_bits())?; w.u32(upper.to_bits())?;
        }
    }
    Ok(())
}
pub(super) fn report(w: &mut Writer, value: &LearnedModelReport) -> Result<(), Error> {
    let source = value.source(); let c = source.report();
    if c.total_encoded_bytes > MAX_WITNESS_BYTES { return Err(Error::Limit); }
    w.blob(&source.encode()?)?;
    for count in [c.source_values, c.source_coordinate_visits, c.groups, c.retained_groups,
        c.residual_changed_words, c.base_encoded_bytes, c.retained_residual_bytes, c.total_encoded_bytes] { w.count(count)?; }
    w.u64(c.reconstruction_products)?;
    w.u64(value.first_position())?; w.u64(value.end_position())?;
    w.count(value.planned_rows())?; w.count(value.quiet_rows())?;
    outcome(w, value.outcome())?; work(w, value.work())?;
    match value.blocked_row() { None => w.u8(0)?, Some(r) => { w.u8(1)?; row(w, r)?; } }
    w.count(value.rows().len())?;
    for r in value.rows() {
        row(w, r.row())?; frame(w, r.frame())?; outcome(w, r.outcome())?; work(w, r.work())?;
        w.count(r.probes().len())?;
        for p in r.probes() { probe(w, *p)?; }
        view(w, r.view())?;
        w.count(r.unavailable_groups().len())?;
        for g in r.unavailable_groups() { group(w, *g)?; }
        w.count(r.steps().len())?;
        for step in r.steps() {
            work(w, step.cumulative_work)?;
            match step.refinement {
                None => w.u8(0)?,
                Some(refinement) => {
                    w.u8(1)?; group(w, refinement.group)?; w.u64(refinement.revision)?;
                    w.count(refinement.encoded_bytes)?; w.count(refinement.materialized_values)?;
                    w.u64(refinement.reconstruction_products)?;
                }
            }
            w.count(step.observations.len())?;
            for observation in &step.observations {
                row(w, observation.row())?; frame(w, observation.frame())?; probe(w, observation.probe())?;
                view(w, observation.view())?;
                score(w, &observation.interval().lower)?; score(w, &observation.interval().upper)?;
                w.u8(match observation.outcome() {
                    ProbeOutcome::CertifiedAlarm => 0, ProbeOutcome::CertifiedQuiet => 1,
                    ProbeOutcome::NeedsRefinement => 2, ProbeOutcome::AtThreshold => 3,
                })?;
                let cost = observation.work();
                w.count(cost.coordinates)?; w.u64(cost.reconstruction_products)?;
            }
        }
    }
    Ok(())
}
