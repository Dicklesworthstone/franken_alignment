//! Reuse native trigger selection AND the existing durable manual-stop cleanup.
use super::{BaseEvent, Machine, Writer, error_tag};
use crate::action::consequence::activation::monitor::MonitorOutcome;
use crate::action::consequence::oversight::decoder_host::{HostedStopCause, HostedStopPolicy};
use crate::Error;

impl Machine {
    pub(super) fn enable_decoder_stop_policy(&mut self, policy: HostedStopPolicy) -> Result<(), Error> {
        if !self.actions.is_empty() || self.requests.len() != 0 || !self.sessions.is_empty()
            || self.decoder_paused() { return Err(Error::WrongState); }
        if self.broker.hosted_decoder()?.position != 0 { return Err(Error::WrongState); }
        self.broker.enable_hosted_stop(policy)
    }

    /// The numerical owner has already made the original stop transition. Replay
    /// its exact idempotent request through the SAME durable cleanup used for a
    /// manual stop: pause, withdraw identity/campaign/human roles, discard sends.
    /// No second stop decision or endpoint outcome is invented. Any error here
    /// leaves the live FileOversight transaction unavailable, not an old fallback.
    pub(super) fn finish_decoder_stop(&mut self, already_stopped: bool) -> Result<(), Error> {
        if already_stopped { return Ok(()); }
        if let Some(receipt) = self.broker.stop_receipt() {
            let request = receipt.request();
            self.apply_core(&BaseEvent::Stop(request))?;
        }
        Ok(())
    }

    /// Additional comparison material ONLY for the newly enabled profile. Old
    /// numerical/checkpoint witness bytes remain exactly unchanged without it.
    /// This is never decoded into a cause, receipt, stopped flag or authority.
    pub(super) fn write_decoder_stop_witness(&self, w: &mut Writer) -> Result<(), Error> {
        let Some(policy) = self.broker.hosted_stop_policy() else { return Ok(()); };
        w.raw(b"FADSTOP\x01")?;
        for value in [policy.id(), policy.generation(), policy.operation()] { w.u64(value)?; }
        w.u8(u8::from(self.decoder_paused()))?;
        let Some(incident) = self.broker.hosted_stop_incident() else { return w.u8(0); };
        w.u8(1)?;
        match incident.cause() {
            HostedStopCause::Numerical(error) => { w.u8(0)?; w.u8(error_tag(error))?; }
            HostedStopCause::Monitoring(outcome) => {
                w.u8(1)?;
                w.u8(match outcome {
                    MonitorOutcome::NoAlarm => 0, MonitorOutcome::Alarm => 1,
                    MonitorOutcome::AtThreshold => 2, MonitorOutcome::BudgetExhausted => 3,
                    MonitorOutcome::Unresolved => 4,
                })?;
            }
        }
        for value in [incident.actor_revision(), incident.stream(), incident.position(), incident.sampled_draws()] {
            w.u64(value)?;
        }
        let work = incident.monitoring(); w.u64(work.frame_reviews)?;
        for value in [work.encoded_bytes, work.probe_coordinates, work.codec_coordinates] { w.count(value)?; }
        match incident.last_stop_error() {
            None => w.u8(0)?, Some(error) => { w.u8(1)?; w.u8(error_tag(error))?; }
        }
        match incident.stop_receipt() {
            None => w.u8(0)?,
            Some(receipt) => {
                w.u8(1)?; let request = receipt.request();
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
