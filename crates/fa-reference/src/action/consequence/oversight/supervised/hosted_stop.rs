//! Close the gap between an owned numerical trip and an endpoint stop barrier.
//! Inference, actor projection and external outcome are different observations.

use super::SupervisedDriver;
use super::super::OversightBroker;
use crate::action::ElapsedTick;
use crate::action::consequence::activation::monitor::decoder::MonitoredStep;
use crate::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledStep;
use crate::action::consequence::activation::tensor::kv::decoder::DecoderBudget;
use crate::action::consequence::activation::tensor::kv::decoder::sampling::SampleBudget;
use crate::action::consequence::delivery::StopSweep;
use crate::Error;

/// Successful numerical inference does not imply an acknowledged endpoint fence.
/// Inspect all three results. In particular, a Held result can coexist with a
/// failed fence write; the original charges remain until endpoint reconciliation.
/// No new effect is proposed, authorized or dispatched by this result or API.
#[derive(Debug)]
pub struct HostedDriverStep<T> {
    pub inference: Result<T, Error>,
    pub synchronization: Result<(), Error>,
    pub containment: Option<Result<StopSweep, Error>>,
}

impl SupervisedDriver {
    /// One owned forced-token step with the original hosted fault policy and,
    /// when triggered, endpoint fencing/draining before returning. The host clock
    /// is sampled before computation and freshly before the stop's endpoint work.
    /// Active review jobs do not authorize a token or bypass this supervision.
    pub fn advance_hosted_forced<F>(
        &mut self, expected_actor_revision: u64, expected_position: u64, token: u32,
        budget: DecoderBudget, clock: F,
    ) -> HostedDriverStep<MonitoredStep>
    where F: FnMut() -> ElapsedTick {
        self.drive_hosted(clock, |broker| broker.advance_hosted_forced(
            expected_actor_revision, expected_position, token, budget,
        ))
    }

    /// The original sampler commits exactly with numerical success. Stopping,
    /// endpoint I/O failure or a late clock refusal cannot rewind its spent draw.
    pub fn advance_hosted_sampled<F>(
        &mut self, expected_actor_revision: u64, expected_position: u64,
        budget: SampleBudget, clock: F,
    ) -> HostedDriverStep<MonitoredSampledStep>
    where F: FnMut() -> ElapsedTick {
        self.drive_hosted(clock, |broker| broker.advance_hosted_sampled(
            expected_actor_revision, expected_position, budget,
        ))
    }

    /// Progress a configured trip, including one initiated through broker_mut or
    /// left poisoned by a caught unwind. None means no automatic incident, not
    /// an assertion that the decoder or the world is safe. Manual stops keep
    /// their existing separate API. A completed trip remains idempotently visible.
    /// The ordinary step loop also invokes this before helper I/O or evidence.
    pub fn service_hosted_stop<F>(&mut self, mut clock: F) -> Option<Result<StopSweep, Error>>
    where F: FnMut() -> ElapsedTick {
        match self.supervisor.broker_mut().enforce_hosted_stop() {
            Ok(None) => return None,
            Ok(Some(_)) => {}
            Err(error) => {
                // A failed local transition must still request child cleanup,
                // but cannot discard its possibly still-reserved original job.
                self.stop_helper_processes(); self.reap_helpers();
                return Some(Err(error));
            }
        }
        // The ORIGINAL stop has already cancelled undispatched attempts. Drop
        // that cancelled job and publish mailbox closure before fallible clock
        // or endpoint I/O; neither failure can keep an active helper cohort alive.
        self.job = None;
        let synchronized = self.supervisor.synchronize();
        self.reap_helpers();
        let result = synchronized.and_then(|()| {
            self.observe_time(clock())?;
            self.supervisor.progress_stop(&mut self.endpoint)
        });
        self.reap_helpers();
        Some(result)
    }

    // No caller-supplied inference callback is exposed; the two typed methods
    // above always use the original broker-owned numerical implementation.
    fn drive_hosted<T, F>(
        &mut self, mut clock: F, advance: impl FnOnce(&mut OversightBroker) -> Result<T, Error>,
    ) -> HostedDriverStep<T>
    where F: FnMut() -> ElapsedTick {
        if let Some(containment) = self.service_hosted_stop(&mut clock) {
            return HostedDriverStep { inference: Err(Error::WrongState),
                synchronization: self.supervisor.synchronize(), containment: Some(containment) };
        }
        self.reap_helpers();
        let inference = (|| {
            self.supervisor.synchronize()?;
            self.observe_time(clock())?;
            advance(self.supervisor.broker_mut())
        })();
        let synchronization = self.supervisor.synchronize();
        // Even a numerical Err, including a held replay/source, must progress
        // containment independently. No effect is resent and no helper rerun.
        let containment = self.service_hosted_stop(&mut clock);
        self.reap_helpers();
        HostedDriverStep { inference, synchronization, containment }
    }
}
