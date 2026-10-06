//! The original learned-K/V owner supplies its own accepted residual forecast.
//! The predictor still owns calibration, sequence/age checks and lifetime evidence.
use super::OversightBroker;
use crate::action::consequence::activation::{CaptureProfile, SourceFrame};
use crate::action::consequence::oversight::learned_source::LearnedAvailability;
use crate::Error;

impl OversightBroker {
    // Registration lives in the original consistency owner, OUTSIDE numerical
    // checkpoints. Reject late binding even after rewinding the active timeline.
    pub(super) fn bind_learned_forecast_source(&self, layer: u64, profile: CaptureProfile,
        dimensions: usize, stream: u64) -> Result<(), Error>
    {
        let state = self.hosted_learned_generation()?;
        if state.host_failure.is_some() || state.position != 0
            || state.cumulative_work.admitted_tokens != 0
            || state.availability != LearnedAvailability::Empty {
            return Err(Error::WrongState);
        }
        if self.hosted_learned_observation()?.stream() != stream { return Err(Error::Binding); }
        self.hosted_learned_original()?.check_host_forecast_residual(layer, profile, dimensions)
    }

    // No raw residual escapes a public source accessor. Held/failed computation,
    // interrupted actor synchronization and a restored prefix without a current
    // accepted event cannot fall back to an older quiet frame.
    pub(super) fn learned_forecast_source(&self, layer: u64) -> Result<SourceFrame, Error> {
        let state = self.hosted_learned_generation()?;
        if state.host_failure.is_some() || state.availability != LearnedAvailability::Ready {
            return Err(Error::Incomplete);
        }
        let run = self.hosted_learned_original()?;
        let event = run.last_event().ok_or(Error::Incomplete)?;
        let step = event.accepted().ok_or(Error::Incomplete)?;
        if event.status() != state.status || event.position() != step.position
            || step.position.checked_add(1) != Some(state.position)
            || run.position() != state.position || run.accepted_tokens().last() != Some(&step.token)
            || !event.audit().complete_quiet() || event.audit().end_position() != state.position {
            return Err(Error::Stale);
        }
        let index = usize::try_from(layer).map_err(|_| Error::Limit)?
            .checked_sub(1).ok_or(Error::InvalidInput)?;
        let actual = step.layers.get(index).ok_or(Error::Binding)?;
        if actual.layer != layer { return Err(Error::Binding); }
        let source = actual.residual.source();
        let identity = source.identity();
        run.check_host_forecast_residual(layer, identity.profile, source.dimensions())?;
        if identity.stream != self.hosted_learned_observation()?.stream()
            || identity.position != step.position || identity.sequence != state.position {
            return Err(Error::Binding);
        }
        Ok(source.clone())
    }
}
