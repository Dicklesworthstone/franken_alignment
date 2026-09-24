//! Durable complete-message disclosure through the ORIGINAL two-key owner.
//! Stream bootstrap is a first-event contract, never a live endpoint mode change.
pub mod actor_wire;
pub mod generated;
use super::{BaseEvent, Event, FileHumanReviewer, FileOversight, FileOversightProfile,
    JournalError, Machine, storage};
use super::super::{FileDeliverySnapshot, codec::shared::{Reader, Writer}};
use crate::action::{ActionSpec, ElapsedTick, ResolvedTarget, VERSION};
use crate::action::consequence::delivery::stream::{ReleaseFrame, StreamProfile, StreamView,
    MAX_MESSAGE_BYTES, MAX_STREAM_BYTES, MAX_STREAM_MESSAGES};
use crate::action::consequence::oversight::actor::ActorProposal;
use crate::Error;
use std::path::Path;

/// Two historical cuts in the SAME acknowledged image. The endpoint's published
/// messages can be ahead of the broker's receipt-confirmed prefix. A pending
/// attempt is not evidence that its message was unseen. Neither view is a key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileStreamSnapshot {
    pub publication: FileDeliverySnapshot,
    pub confirmed_target: ResolvedTarget,
    pub confirmed: StreamView,
    pub published: StreamView,
    pub pending: Option<u64>,
}

/// Actor intent, NOT a full review packet or a publishable frame. None is an
/// explicit finish request; Some must contain a nonempty complete UTF-8 message.
/// The supplied target/version and epoch pin the history being continued. The
/// gateway derives cumulative context privately and retains the ORIGINAL raw
/// proposal under the existing durable request key, including refused requests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileStreamProposal {
    pub target: ResolvedTarget,
    pub expected_policy_epoch: u64,
    pub deadline: ElapsedTick,
    pub message: Option<String>,
}

impl FileOversight {
    /// Create a new stream domain. The independent resource contract must name
    /// complete-message semantics. Nonempty replacement bootstrap bytes refuse:
    /// they cannot be silently dropped or called previously reviewed messages.
    /// Every stream has the original mandatory first-publication guard.
    pub fn create_stream(directory: impl AsRef<Path>, profile: FileOversightProfile,
        stream: StreamProfile) -> Result<(Self, FileHumanReviewer), JournalError>
    {
        let events = vec![Event::StreamBootstrap(stream)];
        // Validate the complete native stream/broker bootstrap before creating
        // storage. No actor, helper or human key is published during this replay.
        let machine = Machine::replay(&profile, &events)?;
        let store = storage::Store::create(directory.as_ref())?;
        store.replace(&super::journal::encode(&profile, store.identity(), &events)?)?;
        Ok(Self::owner(profile, store, events, machine))
    }

    /// Pin the independent stream contract BEFORE cleanup or recovery writes.
    /// The ordinary recovery fence still discards every old sendable envelope;
    /// published messages and original receipt-confirmed history survive it.
    pub fn open_stream(directory: impl AsRef<Path>, profile: FileOversightProfile,
        expected: StreamProfile) -> Result<(Self, FileHumanReviewer), JournalError>
    {
        profile.delivery.limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        let bytes = store.read(profile.delivery.limits.bytes)?;
        let events = super::journal::decode(&profile, store.identity(), &bytes)?;
        check_contract(&events, expected)?;
        let machine = Machine::replay(&profile, &events)?;
        store.confirm_and_cleanup()?;
        let (mut host, reviewer) = Self::owner(profile, store, events, machine);
        host.transact(host.revision(), Event::Core(BaseEvent::Fence))?;
        Ok((host, reviewer))
    }

    /// Last acknowledged state; refuse after ambiguous storage rather than call
    /// it current. Use the separate read-only image reader to inspect disk then.
    pub fn stream_snapshot(&self) -> Result<FileStreamSnapshot, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.stream_snapshot(self.events.len())?)
    }

    /// Read one entire canonical image without a writer, cleanup, recovery fence,
    /// provider call or fresh time. This is historical publication, not delivery
    /// to a remote audience and not an independently authenticated receipt.
    pub fn read_stream_publication(directory: impl AsRef<Path>, profile: &FileOversightProfile,
        expected: StreamProfile) -> Result<FileStreamSnapshot, JournalError>
    {
        super::super::codec::validate_profile(&profile.delivery)?;
        let identity = storage::identity(directory.as_ref())?;
        let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
        let events = super::journal::decode(profile, &identity, &bytes)?;
        check_contract(&events, expected)?;
        Ok(Machine::replay(profile, &events)?.stream_snapshot(events.len())?)
    }

    /// An UNREVIEWED proposal containing every confirmed message and boundary.
    /// Uses the original builder, including its full-frame resource charge.
    /// Submit through the original actor or supervisor proposal API afterwards.
    pub fn stream_message_spec(&self, message: &str, deadline: ElapsedTick)
        -> Result<ActionSpec, JournalError>
    {
        self.check_stream_builder()?;
        Ok(self.machine.broker.stream_message_spec(message, deadline)?)
    }

    /// Finish is an explicit separately reviewed effect, not cancellation or a
    /// conclusion inferred from a timeout. It leaves the disclosed prefix intact.
    pub fn stream_finish_spec(&self, deadline: ElapsedTick) -> Result<ActionSpec, JournalError> {
        self.check_stream_builder()?;
        Ok(self.machine.broker.stream_finish_spec(deadline)?)
    }

    fn check_stream_builder(&self) -> Result<(), JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if !self.clock_ready() || self.source_interrupted { return Err(Error::Incomplete.into()); }
        if self.machine.broker.stop_receipt().is_some() { return Err(Error::WrongState.into()); }
        Ok(())
    }

    // Used only by the existing actor gateway. No public retained-packet getter,
    // callback, alternate request book or actor-selected provider is introduced.
    pub(in crate::action::consequence::delivery::persistent) fn stream_actor_proposal(
        &self, request: u64, intent: &FileStreamProposal,
    ) -> Result<ActorProposal, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        let (_, view) = self.machine.broker.stream_state().ok_or(Error::WrongState)?;
        let spec = if let Some(original) = self.machine.requests.original_spec(request) {
            // Never rebuild a retry against today's prefix or policy. The raw
            // request survives even when proposal admission returned no action.
            let frame = ReleaseFrame::decode(&original.payload).map_err(|_| Error::Binding)?;
            if original.target != Some(intent.target) || original.policy_epoch != intent.expected_policy_epoch
                || original.deadline != intent.deadline || frame.message() != intent.message.as_deref()
                || frame.profile() != view.profile() || original.version != VERSION
                || original.scope != self.profile.delivery.scope || !original.required_witnesses.is_empty()
                || original.units != original.payload.len() as u64
            { return Err(Error::Binding.into()); }
            original.clone()
        } else {
            let spec = match intent.message.as_deref() {
                Some(message) => self.stream_message_spec(message, intent.deadline),
                None => self.stream_finish_spec(intent.deadline),
            }?;
            if spec.target != Some(intent.target) || spec.policy_epoch != intent.expected_policy_epoch {
                return Err(Error::Stale.into());
            }
            spec
        };
        Ok(ActorProposal { target: spec.target.ok_or(Error::Binding)?, payload: spec.payload,
            expected_policy_epoch: spec.policy_epoch, deadline: spec.deadline, units: spec.units })
    }
}

fn check_contract(events: &[Event], expected: StreamProfile) -> Result<(), Error> {
    match events.first() {
        Some(Event::StreamBootstrap(actual) | Event::GeneratedStreamBootstrap(actual))
            if *actual == expected => Ok(()),
        _ => Err(Error::Binding),
    }
}

pub(super) fn write_profile(w: &mut Writer, profile: StreamProfile) -> Result<(), Error> {
    w.u64(profile.id())?; w.u64(profile.generation())?;
    w.count(profile.max_messages())?; w.count(profile.max_message_bytes())?;
    w.count(profile.max_stream_bytes())
}
pub(super) fn read_profile(r: &mut Reader<'_>) -> Result<StreamProfile, Error> {
    StreamProfile::new(r.u64()?, r.u64()?, r.count(MAX_STREAM_MESSAGES)?,
        r.count(MAX_MESSAGE_BYTES)?, r.count(MAX_STREAM_BYTES)?)
}

#[cfg(test)]
mod tests;
