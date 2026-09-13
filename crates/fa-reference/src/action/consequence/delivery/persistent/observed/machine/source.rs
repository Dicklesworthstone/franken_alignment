//! Replay source observations through the original leased capture/writer pair.
//! These operations never reopen a file during replay and never mint effect rights.
use super::{Machine, Transition};
use super::super::source::{FileSourcePolicy, FileSourceReplacement, FileSourceStatus, SourceEvent};
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::PolicySourceChange;
use crate::action::consequence::oversight::CommitteeInput;
use crate::action::consequence::oversight::evidence_source::EvidenceSnapshot;
use crate::action::consequence::oversight::policy_state::{PolicyStateWriter, StateEvent, StateFrontier};
use crate::Error;
use std::rc::Rc;

pub(super) struct SourceState {
    policy: FileSourcePolicy,
    writer: PolicyStateWriter,
    latest: Option<Rc<EvidenceSnapshot>>,
    sequence: u64,
    last_refusal: Option<Error>,
    replacements: Vec<(FileSourceReplacement, PolicySourceChange)>,
}

impl Machine {
    pub(in super::super) fn file_source_status(&self) -> Option<FileSourceStatus> {
        self.file_source.as_ref().map(|source| FileSourceStatus {
            policy: source.policy,
            producer: source.latest.as_ref().map(|capture| capture.identity()),
            semantic_epoch: source.latest.as_ref().map(|capture| capture.snapshot().semantic_epoch),
            capture: self.broker.policy_state_status().expect("registered original capture"),
            last_refusal: source.last_refusal,
            interrupted: false,
        })
    }

    pub(in super::super) fn source_replacement(&self, operation: u64)
        -> Option<&(FileSourceReplacement, PolicySourceChange)>
    {
        self.file_source.as_ref()?.replacements.iter().find(|(request, _)| request.operation == operation)
    }

    pub(super) fn apply_source(&mut self, event: &SourceEvent) -> Result<Transition, Error> {
        match event {
            SourceEvent::Enable(policy) => {
                if self.file_source.is_some() { return Err(Error::Duplicate); }
                if !self.actions.is_empty() || self.requests.len() != 0 || !self.sessions.is_empty()
                    || self.broker.inspect().sequence != 0 || self.broker.stop_receipt().is_some()
                { return Err(Error::WrongState); }
                if policy.source.scope != self.scope { return Err(Error::Binding); }
                let writer = self.broker.enable_fresh_policy_state(policy.source, policy.limits, policy.freshness)?;
                if !self.publication_guard { self.enable_publication_guard()?; }
                self.file_source = Some(SourceState { policy: *policy, writer, latest: None,
                    sequence: 0, last_refusal: None, replacements: Vec::new() });
                Ok(Transition::Unit)
            }
            SourceEvent::Withdraw => {
                if self.file_source.is_none() { return Err(Error::WrongState); }
                self.withdraw_source()?;
                Ok(Transition::Unit)
            }
            SourceEvent::Observe(captured, at) => self.observe_source(captured, *at),
            SourceEvent::Replace(request) => self.replace_source(*request),
        }
    }

    fn replace_source(&mut self, request: FileSourceReplacement) -> Result<Transition, Error> {
        if request.operation == 0 { return Err(Error::InvalidInput); }
        if let Some((original, _)) = self.source_replacement(request.operation) {
            return if original == &request { Ok(Transition::Unit) } else { Err(Error::Binding) };
        }
        if self.broker.stop_receipt().is_some() { return Err(Error::WrongState); }
        let source = self.file_source.as_mut().ok_or(Error::WrongState)?;
        let mut next = source.policy.source;
        next.generation = request.next_generation;
        // The original gate bounds the lifetime number of generations. Reserve
        // our corresponding receipt slot before changing its authority ledger.
        source.replacements.try_reserve(1).map_err(|_| Error::Limit)?;
        let (writer, receipt) = self.broker.replace_policy_state(request.expected_generation,
            request.expected_authority_epoch, next, source.policy.limits)?;
        source.writer = writer;
        source.policy.source = next;
        source.sequence = 0;
        source.last_refusal = None;
        source.replacements.push((request, receipt));
        // Keep latest solely as the producer/semantic/equal-version byte floor.
        // The replacement capture is empty; there is no saved-input fallback.
        self.invalidate_source_basis()?;
        self.sessions.clear();
        self.automatic.clear();
        // Retain envelopes for the ORIGINAL guarded publication/status path.
        // Revocation prevents first execution, but a later terminal receipt is
        // still authoritative and an unknown dispatch is still charged.
        Ok(Transition::Unit)
    }

    /// Withdrawal affects original full-input eligibility and reviewer keys,
    /// never reservations, historical execution receipts or unknown liabilities.
    pub(super) fn withdraw_source(&mut self) -> Result<(), Error> {
        let Some(source) = &self.file_source else { return Ok(()); };
        source.writer.withdraw();
        self.invalidate_source_basis()
    }

    fn invalidate_source_basis(&mut self) -> Result<(), Error> {
        for id in self.actions.keys() {
            let revision = self.broker.input_revision(*id)?;
            self.broker.inputs_unavailable(*id, revision)?;
        }
        self.withdraw_keys()?;
        Ok(())
    }

    fn observe_source(&mut self, captured: &Rc<EvidenceSnapshot>, at: ElapsedTick) -> Result<Transition, Error> {
        let source = self.file_source.as_mut().ok_or(Error::WrongState)?;
        let unchanged = source.latest.as_deref() == Some(captured.as_ref());
        // Withdraw before validating ANY newly observed value. Failed checks
        // below are committed results, not discarded speculative transitions.
        source.writer.withdraw();
        let admitted = (|| {
            let identity = captured.identity();
            if identity.scope != source.policy.source.scope || identity.source != source.policy.source.source {
                return Err(Error::Binding);
            }
            if let Some(previous) = &source.latest {
                if identity.generation < previous.identity().generation
                    || captured.snapshot().semantic_epoch < previous.snapshot().semantic_epoch
                { return Err(Error::Stale); }
                if identity.generation == previous.identity().generation && !unchanged { return Err(Error::Binding); }
            }
            // A well-formed, correctly scoped newer producer version remains a
            // floor even if it is incomplete or fails the native capture limits.
            // An earlier complete version cannot become the recovery fallback.
            source.latest = Some(Rc::clone(captured));
            if !captured.snapshot().complete { return Err(Error::Incomplete); }
            if !captured.contexts().keys().eq(self.broker.contracts().members().keys()) { return Err(Error::Binding); }
            Ok(())
        })();
        let result: Result<StateFrontier, Error> = (|| {
            admitted?;
            self.observe(at)?;
            let source = self.file_source.as_mut().expect("registered source");
            let sequence = source.sequence.checked_add(1).ok_or(Error::Overflow)?;
            source.sequence = sequence;
            source.writer.record(sequence, &StateEvent::Snapshot {
                semantic_epoch: captured.snapshot().semantic_epoch,
                values: captured.snapshot().values.clone(),
            })?;
            // Every refresh, even equal producer bytes, is a NEW observation.
            // Native freshness cannot be renewed by relabeling an old closure.
            source.writer.close_observed(sequence, sequence, at)
        })();
        if !unchanged || result.is_err() { self.invalidate_source_basis()?; }
        self.file_source.as_mut().expect("registered source").last_refusal = result.as_ref().err().copied();
        Ok(Transition::SourceObserved(result))
    }

    /// Prevent a direct manual input call from installing unrelated helper
    /// contexts while using a valid policy snapshot from the registered source.
    /// Legacy owners remain unchanged. The ORIGINAL builder defines equality.
    pub(super) fn check_source_inputs(&self, inputs: &CommitteeInput) -> Result<(), Error> {
        let Some(source) = &self.file_source else { return Ok(()); };
        let _ = self.broker.capture_policy_state()?;
        let capture = source.latest.as_ref().ok_or(Error::Incomplete)?;
        let expected = capture.inputs_for(inputs.action(), self.broker.contracts())?;
        if &expected != inputs { return Err(Error::Binding); }
        Ok(())
    }
}
