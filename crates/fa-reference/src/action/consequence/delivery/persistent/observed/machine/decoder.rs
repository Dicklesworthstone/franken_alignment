//! The original numerical owner and gate, reconstructed from exact INPUTS.
//! Expected bytes only reject divergence; they are never loaded into the actor.
use super::{Machine, Transition};
use super::super::{Event, BaseEvent, journal::HumanDecision};
use super::super::decoder::{DecoderEvent, FileDecoderConfig, StepRequest, MAX_WITNESS_BYTES};
use super::super::super::codec::shared::Writer;
use crate::action::consequence::activation::monitor::decoder::{MonitoredStep, MonitoringStatus, ReviewedStep};
use crate::action::consequence::activation::monitor::decoder::sampled::{MonitoredSampledStep, host::replay::error_tag};
use crate::action::consequence::activation::tensor::kv::decoder::DecoderBudget;
use crate::action::consequence::activation::tensor::kv::decoder::sampling::{SampleBudget, SamplingBudget};
use crate::Error;
use std::rc::Rc;

mod checkpoint;
use super::super::decoder::checkpoint::CheckpointRequest;

pub(super) struct DecoderState {
    config: Rc<FileDecoderConfig>,
    paused: bool,
    checkpoints: checkpoint::CheckpointHistory,
}

impl Machine {
    pub(in super::super) fn decoder_contract(&self) -> Option<&FileDecoderConfig> {
        self.decoder.as_ref().map(|state| state.config.as_ref())
    }
    pub(in super::super) fn decoder_paused(&self) -> bool { self.decoder.as_ref().is_some_and(|state| state.paused) }
    pub(super) fn pause_decoder(&mut self) { if let Some(state) = &mut self.decoder { state.paused = true; } }

    /// Saved numerical state cannot support new permitting work before explicit
    /// supervisor resume. Restrictive operations and evidence acquisition remain
    /// available. Recovery has already removed every old sendable envelope.
    pub(super) fn check_decoder_admission(&self, event: &Event) -> Result<(), Error> {
        if !self.decoder_paused() { return Ok(()); }
        if matches!(event,
            Event::Decoder(DecoderEvent::Resume { .. }
                | DecoderEvent::Checkpoint(CheckpointRequest::Reset { .. }, _))
            | Event::Core(BaseEvent::Time(_) | BaseEvent::Cancel(_) | BaseEvent::Fence
                | BaseEvent::Stop(_) | BaseEvent::StopProgress(_) | BaseEvent::Reconcile(_)
                | BaseEvent::Seal(_) | BaseEvent::Sweep | BaseEvent::ReplacePolicy(_) | BaseEvent::ReserveRecovery(_))
            | Event::InputsUnavailable(..) | Event::RevokeHumans
            | Event::Human(_, HumanDecision::Reject | HumanDecision::Revoke)
            | Event::Source(_) | Event::Identity(_) | Event::Campaign(_)
            | Event::CredentialRotate(_) | Event::CredentialRevoke(_)
            | Event::PublishChecked(..) | Event::PublishCredentialed(..)) { Ok(()) }
        else { Err(Error::Incomplete) }
    }

    pub(super) fn apply_decoder(&mut self, event: &DecoderEvent) -> Result<Transition, Error> {
        match event {
            DecoderEvent::Enable(config) => {
                if self.decoder.is_some() { return Err(Error::Duplicate); }
                if !self.actions.is_empty() || self.requests.len() != 0 || !self.sessions.is_empty()
                    || !self.containment.updates.is_empty() || !self.containment.checkpoints.is_empty()
                    || self.broker.stop_receipt().is_some() { return Err(Error::WrongState); }
                let run = config.build()?;
                self.broker.own_sampled_decoder(run, config.limits)?;
                if !self.publication_guard { self.enable_publication_guard()?; }
                self.decoder = Some(DecoderState { config: Rc::clone(config), paused: false,
                    checkpoints: checkpoint::CheckpointHistory::default() });
                Ok(Transition::Unit)
            }
            DecoderEvent::Step(request, expected) => {
                let result = self.execute_decoder_step(*request)?;
                if self.decoder_witness(&result)?.as_slice() != expected.as_ref() { return Err(Error::Binding); }
                Ok(result)
            }
            DecoderEvent::Checkpoint(request, expected) => {
                self.execute_decoder_checkpoint(request)?;
                if self.decoder_checkpoint_witness(request)?.as_slice() != expected.as_ref() {
                    return Err(Error::Binding);
                }
                Ok(Transition::Unit)
            }
            DecoderEvent::Resume { revision, position } => {
                if !self.clock_ready { return Err(Error::Incomplete); }
                let state = self.decoder.as_ref().ok_or(Error::Incomplete)?;
                if !state.paused { return Err(Error::WrongState); }
                let actual = self.broker.hosted_decoder()?;
                if actual.actor_revision != *revision || actual.position != *position { return Err(Error::Stale); }
                if actual.status != MonitoringStatus::Ready || self.broker.inspect().suspended
                    || self.broker.stop_receipt().is_some() { return Err(Error::WrongState); }
                self.decoder.as_mut().expect("checked decoder").paused = false;
                Ok(Transition::Unit)
            }
        }
    }

    pub(in super::super) fn prepare_decoder_step(&mut self, request: StepRequest)
        -> Result<(DecoderEvent, Transition), Error>
    {
        let result = self.execute_decoder_step(request)?;
        let witness = self.decoder_witness(&result)?;
        self.requests.refresh(&self.broker.inspect())?;
        self.bootstrap = None;
        Ok((DecoderEvent::Step(request, witness.into()), result))
    }
    fn execute_decoder_step(&mut self, request: StepRequest) -> Result<Transition, Error> {
        if self.decoder.is_none() || self.decoder_paused() || !self.clock_ready { return Err(Error::Incomplete); }
        // Original failures may latch monitoring or follow a committed draw.
        // Store that result rather than rolling back a failed numerical observation.
        Ok(match request {
            StepRequest::Forced { revision, position, token, products } => Transition::DecoderForced(Box::new(
                self.broker.advance_hosted_forced(revision, position, token, DecoderBudget { scalar_products: products }))),
            StepRequest::Sampled { revision, position, products, vocabulary } => Transition::DecoderSampled(Box::new(
                self.broker.advance_hosted_sampled(revision, position, SampleBudget {
                    decoder: DecoderBudget { scalar_products: products }, sampling: SamplingBudget { vocabulary } }))),
        })
    }
    fn decoder_witness(&self, result: &Transition) -> Result<Vec<u8>, Error> {
        let mut w = Writer::new(MAX_WITNESS_BYTES); w.raw(b"FADSTEP\x01")?;
        w.blob(&self.broker.hosted_replay_bytes()?)?;
        // Retain the ORIGINAL actor's projection too, including a failed sync.
        // No saved ActorState is installed by replaying this comparison material.
        let actor = self.broker.retained_actor_state();
        w.u64(self.broker.actor_revision())?; w.u64(actor.next_position())?;
        w.count(actor.tokens().len())?;
        for token in actor.tokens() { w.u32(*token)?; }
        w.blob(actor.cache())?; w.blob(actor.sampler())?;
        match result {
            Transition::DecoderForced(result) => {
                w.u8(0)?;
                match result.as_ref() {
                    Err(error) => { w.u8(0)?; w.u8(error_tag(*error))?; }
                    Ok(MonitoredStep::Held(_)) => w.u8(1)?,
                    Ok(MonitoredStep::Released(step)) => { w.u8(2)?; released(&mut w, step)?; }
                }
            }
            Transition::DecoderSampled(result) => {
                w.u8(1)?;
                match result.as_ref() {
                    Err(error) => { w.u8(0)?; w.u8(error_tag(*error))?; }
                    Ok(MonitoredSampledStep::Held(_)) => w.u8(1)?,
                    Ok(MonitoredSampledStep::Released(step)) => {
                        w.u8(2)?; released(&mut w, step.reviewed())?;
                        let c = step.choice(); w.u32(c.token)?;
                        for value in [c.stream, c.draw, c.random_word, c.probability.to_bits()] { w.u64(value)?; }
                        for value in [c.work.logits_scanned, c.work.exponentials, c.work.retained_candidates, c.work.zero_weights] { w.count(value)?; }
                    }
                }
            }
            _ => return Err(Error::Binding),
        }
        Ok(w.finish())
    }
}

fn released(w: &mut Writer, reviewed: &ReviewedStep) -> Result<(), Error> {
    let step = reviewed.step(); w.u32(step.token)?; w.u64(step.position)?;
    w.count(step.layers.len())?;
    for layer in &step.layers {
        w.u64(layer.layer)?;
        w.blob(&layer.query.source().encode_initial(23)?)?;
        w.blob(&layer.residual.source().encode_initial(23)?)?;
    }
    Ok(())
}
