//! Private numerical continuation shared by synchronous and cooperative callers.
//! No saved measurement, alternate model or partial result can construct this.
use super::FileIdentityObservation;
use crate::action::consequence::activation::identity::decoder::{
    DecoderIdentityMeasurement, DecoderIdentityProbe, IdentityProbeProgress, IdentityProbeWork,
};
use crate::action::consequence::oversight::identity::IdentityChallenge;
use crate::Error;

pub(in super::super::super) struct IdentityComputation {
    pub challenge: IdentityChallenge,
    pub probe: DecoderIdentityProbe,
    pub measured: Vec<DecoderIdentityMeasurement>,
    pub observation: FileIdentityObservation,
    pub collecting: bool,
    pub numerical_failure: Option<Error>,
    pub remaining: u64,
}
impl IdentityComputation {
    pub fn work(&self) -> IdentityProbeWork { self.probe.work() }
    pub fn ready(&self) -> bool {
        !self.collecting || self.numerical_failure.is_some() || self.remaining == 0
    }
    /// Enter at most one ORIGINAL token. Ordinary numerical failures become
    /// original completion evidence, not a discarded or retried success prefix.
    pub fn advance(&mut self) -> Result<(), Error> {
        if self.ready() { return Err(Error::WrongState); }
        self.remaining -= 1;
        match self.probe.advance() {
            Ok(IdentityProbeProgress::Advanced) => {}
            Ok(IdentityProbeProgress::Measured(frame)) => self.measured.push(*frame),
            Ok(IdentityProbeProgress::Complete) => self.numerical_failure = Some(Error::Binding),
            Err(error) => self.numerical_failure = Some(error),
        }
        if self.remaining == 0 && self.numerical_failure.is_none() && !self.probe.complete() {
            self.numerical_failure = Some(Error::Incomplete);
        }
        Ok(())
    }
}
