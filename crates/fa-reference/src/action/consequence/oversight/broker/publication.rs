//! Configure the original delivery owner's additional final-cut witness lane.
use super::OversightBroker;
use crate::Error;
use crate::action::consequence::delivery::publication_gate::{PublicationInputs, PublicationLimits, PublicationSourceStatus};
use crate::action::consequence::oversight::publication::{PublicationJudgment, PublicationReport};
use crate::action::consequence::delivery::publication_gate::changes::{PublicationChange, PublicationChangePolicy, PublicationChangeReport, PublicationChangeStatus};
use crate::action::consequence::delivery::publication_gate::changes::freshness::{PublicationFreshnessPolicy, PublicationFreshnessStatus, PublicationHeartbeat};
use std::rc::Rc;
use crate::action::consequence::delivery::publication_gate::changes::{PublicationInputCut, PublicationInputCutStatus, PublicationCaptureOutcome};

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

    pub fn bind_publication_source(&mut self, attempt: u64, source: u64, generation: u64,
        original: PublicationInputs) -> Result<(), Error>
    {
        self.delivery.bind_publication_source(attempt, source, generation, original)
    }

    pub fn publication_source(&self, attempt: u64) -> Result<Option<PublicationSourceStatus>, Error> {
        self.delivery.publication_source(attempt)
    }

    pub fn record_captured_publication_inputs(&mut self, attempt: u64, revision: u64,
        source: u64, generation: u64, inputs: PublicationInputs) -> Result<u64, Error>
    {
        self.delivery.record_captured_publication_inputs(attempt, revision, source, generation, inputs)
    }

    pub fn enable_publication_changes(&mut self, policy: PublicationChangePolicy) -> Result<(), Error> {
        self.delivery.enable_publication_changes(policy)
    }
    pub fn publication_change_status(&self) -> Result<PublicationChangeStatus, Error> {
        self.delivery.publication_change_status()
    }
    pub fn publication_change_report(&self) -> Result<Option<Rc<PublicationChangeReport>>, Error> {
        self.delivery.publication_change_report()
    }
    pub fn preflight_publication_change(&self, notice: PublicationChange) -> Result<(), Error> {
        self.delivery.preflight_publication_change(notice)
    }
    pub fn record_publication_change(&mut self, notice: PublicationChange) -> Result<Rc<PublicationChangeReport>, Error> {
        self.delivery.record_publication_change(notice)
    }

    pub fn enable_publication_change_freshness(&mut self, policy: PublicationFreshnessPolicy) -> Result<(), Error> {
        self.delivery.enable_publication_change_freshness(policy)
    }
    pub fn publication_change_freshness(&self) -> Result<PublicationFreshnessStatus, Error> {
        self.delivery.publication_change_freshness()
    }
    pub fn publication_changes_unavailable(&mut self, source: u64) -> Result<(), Error> {
        self.delivery.publication_changes_unavailable(source)
    }
    pub fn record_publication_heartbeat(&mut self, heartbeat: PublicationHeartbeat) -> Result<PublicationFreshnessStatus, Error> {
        self.delivery.record_publication_heartbeat(heartbeat)
    }

    /// The coupled durable publisher invokes the SAME check after dispatch and
    /// immediately before its first external publication. This is not a permit.
    #[cfg(unix)]
    pub(crate) fn revalidate_publication_witnesses(&mut self, attempt: u64) -> Result<(), Error> {
        self.delivery.check_publication(attempt, None)
    }
}

impl OversightBroker {
    pub fn bind_publication_source_at_cut(&mut self, attempt: u64, source: u64, generation: u64,
        original: PublicationInputs, cut: PublicationInputCut) -> Result<(), Error>
    {
        self.delivery.bind_publication_source_at_cut(attempt, source, generation, original, cut)
    }
    pub fn publication_input_cut(&self, attempt: u64) -> Result<Option<PublicationInputCutStatus>, Error> {
        self.delivery.publication_input_cut(attempt)
    }
    pub fn record_captured_publication_inputs_at_cut(&mut self, attempt: u64, revision: u64,
        source: u64, generation: u64, inputs: PublicationInputs, cut: PublicationInputCut) -> Result<u64, Error>
    {
        self.delivery.record_captured_publication_inputs_at_cut(attempt, revision, source, generation, inputs, cut)
    }
}

impl OversightBroker {
    /// The original pre-dispatch source observation, never another authority.
    pub fn record_captured_publication_inputs_or_defer(&mut self, attempt: u64, revision: u64,
        source: u64, generation: u64, inputs: PublicationInputs, cut: PublicationInputCut)
        -> Result<PublicationCaptureOutcome, Error>
    {
        self.delivery.record_captured_publication_inputs_or_defer(attempt, revision, source, generation, inputs, cut)
    }
}

impl OversightBroker {
    /// Bootstrap-only exact current-state fallback, not missing-history repair.
    pub fn enable_publication_snapshot_fallback(&mut self) -> Result<(), Error> {
        self.delivery.enable_publication_snapshot_fallback()
    }
    pub fn publication_snapshot_fallback_enabled(&self) -> Result<bool, Error> {
        self.delivery.publication_snapshot_fallback_enabled()
    }
}
