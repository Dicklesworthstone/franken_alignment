//! One completed learned message and an independently reviewed stream finish.
//! The ORIGINAL receipt-confirmed stream is the only progress ledger. No token
//! is released early; no loss of acknowledgment is interpreted as nonexecution.
use super::{ByteBpe, DecoderModel, LearnedDecoderBindingLimits, LearnedTextConfig, OversightBroker};
use crate::action::{ActionSpec, ElapsedTick, ResolvedTarget};
use crate::action::consequence::delivery::stream::StreamView;
use crate::action::consequence::gate::containment::session::policy::controller::Proposal;
use crate::{Error, ReadWitness, Snapshot};

/// Immutable audience basis, not another delivery/approval ledger. Source and
/// tokenizer remain owned by the original host; only current receipts move the
/// real audience history. At most one new message belongs to this generation.
#[derive(Debug)]
pub(in super::super) struct LearnedTextStreamBasis {
    target: ResolvedTarget,
    view: StreamView,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Release { Message, Finish }

impl OversightBroker {
    /// Explicit stream alternative to own_learned_text_generation. Freeze the
    /// ORIGINAL confirmed audience basis before inference. A raw endpoint, a
    /// pending send, insufficient full-message capacity or finished stream
    /// refuses before installing any numerical owner or evidence requirement.
    /// This profile covers one generation/message and its later explicit close;
    /// it does not silently launch a second generation or reuse output twice.
    pub fn own_learned_text_stream(&mut self, model: DecoderModel, tokenizer: ByteBpe,
        config: LearnedTextConfig, limits: LearnedDecoderBindingLimits) -> Result<(), Error>
    {
        self.check_learned_bootstrap()?;
        if self.stream_pending().is_some() { return Err(Error::Incomplete); }
        let (target, view) = self.stream_state().ok_or(Error::Binding)?;
        if view.finished() { return Err(Error::WrongState); }
        let profile = view.profile();
        if view.message_count() >= profile.max_messages()
            || config.output.max_bytes > profile.max_message_bytes()
            || view.visible().len().checked_add(config.output.max_bytes).ok_or(Error::Overflow)?
                > profile.max_stream_bytes() { return Err(Error::Limit); }
        // One append and one explicit finish must both have representable target
        // versions. This does not reserve rights or waive subsequent validation.
        target.expected_version.checked_add(2).ok_or(Error::Overflow)?;
        let basis = LearnedTextStreamBasis { target, view: view.clone() };
        let source = model.observed_learned_text_generation(tokenizer, config)?;
        self.install_fresh_learned_host(source, limits)?;
        self.learned_host.as_mut().expect("installed learned source").text_stream = Some(basis);
        Ok(())
    }

    pub fn learned_text_stream_required(&self) -> bool {
        self.learned_host.as_ref().is_some_and(|host| host.text_stream.is_some())
    }

    /// Derive the complete cumulative frame and its FULL byte charge from the
    /// original stream builder. No caller output, message boundary, target,
    /// version, policy epoch or claimed smaller byte charge is accepted here.
    /// The returned proposal still needs ordinary sidecar/congress/human review.
    pub fn propose_learned_text_stream_message(&mut self, id: u64, deadline: ElapsedTick,
        required_witnesses: Vec<ReadWitness>, snapshot: &Snapshot) -> Result<Proposal, Error>
    {
        let mut spec = self.learned_stream_spec(Release::Message, deadline)?;
        spec.required_witnesses = required_witnesses;
        self.propose(id, spec, snapshot)
    }

    /// Close only AFTER a receipt confirms this generation's exact message.
    /// Closing consumes no inference or RNG but remains a separate reviewed
    /// effect with a new permit and any configured human key. Cancellation,
    /// timeout or a dispatched-but-unconfirmed append cannot stand in for it.
    pub fn propose_learned_text_stream_finish(&mut self, id: u64, deadline: ElapsedTick,
        required_witnesses: Vec<ReadWitness>, snapshot: &Snapshot) -> Result<Proposal, Error>
    {
        let mut spec = self.learned_stream_spec(Release::Finish, deadline)?;
        spec.required_witnesses = required_witnesses;
        self.propose(id, spec, snapshot)
    }

    // The generic proposal path AND every existing decoder permission check
    // reach this comparison. No alternative stream builder bypasses provenance.
    pub(super) fn check_learned_text_stream_spec(&self, spec: &ActionSpec) -> Result<(), Error> {
        let (_, expected) = self.current_learned_stream_spec(spec.deadline)?;
        if spec.version != expected.version || spec.scope != expected.scope
            || spec.target != expected.target || spec.policy_epoch != expected.policy_epoch
            || spec.units != expected.units || spec.payload != expected.payload {
            return Err(Error::Binding);
        }
        // Witnesses deliberately remain caller-declared normal prerequisites;
        // the existing policy and Snapshot validator still bind their full set.
        Ok(())
    }

    pub(crate) fn learned_stream_spec(&self, requested: Release, deadline: ElapsedTick) -> Result<ActionSpec, Error> {
        let (available, spec) = self.current_learned_stream_spec(deadline)?;
        if available != requested { return Err(Error::WrongState); }
        Ok(spec)
    }

    fn current_learned_stream_spec(&self, deadline: ElapsedTick) -> Result<(Release, ActionSpec), Error> {
        let (release, bytes) = self.learned_stream_release(None)?;
        let spec = match release {
            Release::Message => self.stream_message_spec(
                std::str::from_utf8(&bytes).map_err(|_| Error::InvalidInput)?, deadline)?,
            Release::Finish => self.stream_finish_spec(deadline)?,
        };
        Ok((release, spec))
    }

    /// The coupled durable endpoint revalidates AFTER dispatch set pending.
    /// Check that exact original attempt against the still-confirmed audience
    /// cut; never use this path to admit a new proposal or clear pending state.
    pub(super) fn check_dispatched_learned_text_stream_spec(&self, attempt: u64,
        spec: &ActionSpec) -> Result<(), Error>
    {
        if self.stream_pending() != Some(attempt) { return Err(Error::Incomplete); }
        let (release, bytes) = self.learned_stream_release(Some(attempt))?;
        let (target, view) = self.stream_state().ok_or(Error::Binding)?;
        // Same ORIGINAL frame encoders as the pre-dispatch stream builder.
        // That builder intentionally refuses every pending send; only the
        // already-dispatched action reaches this final-publication comparison.
        let payload = match release {
            Release::Message => view.encode_message(
                std::str::from_utf8(&bytes).map_err(|_| Error::InvalidInput)?)?,
            Release::Finish => view.encode_finish()?,
        };
        let control = self.inspect();
        if control.ledger.elapsed.ok_or(Error::Incomplete)? >= spec.deadline { return Err(Error::Stale); }
        if spec.version != crate::action::VERSION || spec.scope != self.scope
            || spec.target != Some(target) || spec.policy_epoch != control.ledger.epoch
            || spec.units != u64::try_from(payload.len()).map_err(|_| Error::Limit)?
            || spec.payload != payload { return Err(Error::Binding); }
        // Original approval, live-source, human-key, witness and endpoint
        // validation still run. No new key, receipt or effect ledger is created.
        Ok(())
    }

    fn learned_stream_release(&self, dispatched: Option<u64>) -> Result<(Release, Vec<u8>), Error> {
        if self.inspect().suspended || self.stop_receipt().is_some() { return Err(Error::WrongState); }
        if let Some(pending) = self.stream_pending() {
            if dispatched != Some(pending) { return Err(Error::Incomplete); }
            // Only an existing ORIGINAL dispatch record may be revalidated.
            // This is not admission for another message or another attempt.
            self.delivery.status_query(pending)?;
        }
        let host = self.learned_host.as_ref().ok_or(Error::Incomplete)?;
        if host.fault.is_some() { return Err(Error::Incomplete); }
        let basis = host.text_stream.as_ref().ok_or(Error::Binding)?;
        let (bytes, _, _) = host.run.decode_text_output()?;
        let text = std::str::from_utf8(&bytes).map_err(|_| Error::InvalidInput)?;
        let (target, view) = self.stream_state().ok_or(Error::Binding)?;
        if view.finished() { return Err(Error::WrongState); }
        if target == basis.target && view == &basis.view {
            return Ok((Release::Message, bytes));
        }
        let mut after_message = basis.target;
        after_message.expected_version = after_message.expected_version.checked_add(1).ok_or(Error::Overflow)?;
        // Exact boundaries, not just concatenated audience bytes. Equal bytes
        // split into different messages are NOT the original publication basis.
        if target == after_message && view.profile() == basis.view.profile()
            && view.messages().eq(basis.view.messages().chain(std::iter::once(text))) {
            return Ok((Release::Finish, bytes));
        }
        Err(Error::Stale)
    }
}
