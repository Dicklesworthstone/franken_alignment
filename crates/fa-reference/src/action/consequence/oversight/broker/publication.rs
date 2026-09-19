//! Configure the original delivery owner's additional final-cut witness lane.
use super::OversightBroker;
use crate::Error;
use crate::action::consequence::delivery::publication_gate::{PublicationInputs, PublicationLimits};
use crate::action::consequence::oversight::publication::{PublicationJudgment, PublicationReport};

impl OversightBroker {
    /// Mandatory on every subsequent attempt. Ordinary committee, policy,
    /// decoder and human-key checks remain independent conjunctive requirements.
    pub fn enable_publication_validation(&mut self, limits: PublicationLimits) -> Result<(), Error> {
        self.delivery.enable_publication_validation(limits)
    }

    pub fn bind_publication_judgment(&mut self, attempt: u64, judgment: PublicationJudgment) -> Result<(), Error> {
        self.delivery.bind_publication_judgment(attempt, judgment)
    }

    /// Host-supplied current data, not an authenticated capture or permit.
    pub fn record_publication_inputs(
        &mut self, attempt: u64, expected_revision: u64, inputs: Option<PublicationInputs>,
    ) -> Result<u64, Error> {
        self.delivery.record_publication_inputs(attempt, expected_revision, inputs)
    }

    pub fn publication_input_revision(&self, attempt: u64) -> Result<u64, Error> {
        self.delivery.publication_input_revision(attempt)
    }

    /// Historical diagnostics; these never substitute for dispatch revalidation.
    pub fn publication_validation(&self, attempt: u64) -> Result<Option<PublicationReport>, Error> {
        self.delivery.publication_validation(attempt)
    }
}
