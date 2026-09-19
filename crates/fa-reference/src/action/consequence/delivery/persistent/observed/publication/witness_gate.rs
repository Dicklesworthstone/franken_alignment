//! Durable FA-062 configuration and observations in the ORIGINAL owner/journal.
//! No approvals, permits, mutable broker handles or serialized success bits.
mod changes;
use crate::action::consequence::delivery::publication_gate::changes::{PublicationChange, PublicationChangePolicy};
use super::witnesses::{FilePublicationEvidence, FilePublicationInputs, MAX_PUBLICATION_PACKET_BYTES};
use super::capture::{FilePublicationCapture, SourceBinding, MAX_CAPTURE_BYTES};
use super::super::{BaseEvent, Event, FileHumanReviewer, FileOversight, FileOversightProfile,
    JournalError, JournalFailure, JournalIo, Machine, Transition, journal, storage};
use crate::action::consequence::delivery::publication_gate::{PublicationLimits, MAX_PUBLICATION_BINDINGS};
use crate::action::consequence::delivery::persistent::codec::shared::{Reader, Writer};
use crate::action::consequence::oversight::publication::PublicationReport;
use crate::witness::refinement::RefinementBudget;
use crate::Error;
use std::io;
use std::path::Path;
use std::rc::Rc;

#[derive(Clone)]
pub(in super::super) enum WitnessEvent {
    Enable(PublicationLimits),
    Bind(u64, Rc<FilePublicationEvidence>),
    Inputs(u64, u64, Option<Rc<FilePublicationInputs>>),
    SourceBind(u64, Rc<SourceBinding>),
    Captured(u64, u64, Rc<FilePublicationCapture>),
    ChangeProfile(PublicationChangePolicy),
    Change(PublicationChange),
}

impl FileOversight {
    /// Publish the mandatory profile in the first canonical image. A crash
    /// cannot leave a successfully initialized, silently unguarded owner.
    pub fn create_with_publication_validation(
        directory: impl AsRef<Path>, profile: FileOversightProfile, limits: PublicationLimits,
    ) -> Result<(Self, FileHumanReviewer), JournalError> {
        let events = vec![Event::PublicationWitness(WitnessEvent::Enable(limits))];
        let machine = Machine::replay(&profile, &events)?;
        let store = storage::Store::create(directory.as_ref())?;
        store.replace(&journal::encode(&profile, store.identity(), &events)?)?;
        Ok(Self::owner(profile, store, events, machine))
    }

    /// Require exactly the original limits BEFORE replay, cleanup or the recovery
    /// fence. Generic open also replays this gate; omitting this pin cannot disable
    /// it. Recovery keeps requirements/history but withdraws all old sendable keys.
    pub fn open_with_publication_validation(
        directory: impl AsRef<Path>, profile: FileOversightProfile, expected: PublicationLimits,
    ) -> Result<(Self, FileHumanReviewer), JournalError> {
        profile.delivery.limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        let bytes = store.read(profile.delivery.limits.bytes)?;
        let events = journal::decode(&profile, store.identity(), &bytes)?;
        let mut profiles = events.iter().filter_map(|event| match event {
            Event::PublicationWitness(WitnessEvent::Enable(limits)) => Some(*limits),
            _ => None,
        });
        if profiles.next() != Some(expected) || profiles.next().is_some() { return Err(Error::Binding.into()); }
        let machine = Machine::replay(&profile, &events)?;
        store.confirm_and_cleanup()?;
        let (mut host, reviewer) = Self::owner(profile, store, events, machine);
        host.transact(host.revision(), Event::Core(BaseEvent::Fence))?;
        Ok((host, reviewer))
    }

    /// Bootstrap only. This also enables the existing first-publication guard;
    /// raw publish cannot skip current witness checks after a delayed dispatch.
    pub fn enable_publication_validation(&mut self, revision: u64, limits: PublicationLimits) -> Result<(), JournalError> {
        self.transact(revision, Event::PublicationWitness(WitnessEvent::Enable(limits)))?;
        Ok(())
    }

    pub fn bind_publication_evidence(&mut self, revision: u64, attempt: u64,
        evidence: FilePublicationEvidence) -> Result<(), JournalError>
    {
        self.transact(revision, Event::PublicationWitness(WitnessEvent::Bind(attempt, Rc::new(evidence))))?;
        Ok(())
    }

    /// Record a current observation or explicit loss, never replace requirements.
    /// A foreign/stale predecessor refuses without mutation. Once a current update
    /// begins, any capacity, encoding, replay, allocation or storage failure leaves
    /// this owner unavailable: an older quiet image must not remain eligible.
    /// Recovery uses the same canonical journal and fences old sendable work.
    pub fn record_publication_inputs(&mut self, revision: u64, attempt: u64,
        expected_input_revision: u64, inputs: Option<FilePublicationInputs>) -> Result<u64, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if revision != self.revision() || self.machine.broker.publication_input_revision(attempt)? != expected_input_revision {
            return Err(Error::Stale.into());
        }
        let event = Event::PublicationWitness(WitnessEvent::Inputs(attempt, expected_input_revision, inputs.map(Rc::new)));
        self.check_source_admission(&event)?;
        self.fault = Some(JournalFailure { operation: JournalIo::Stage,
            kind: io::ErrorKind::Other, replacement_may_be_visible: false });
        if self.events.len() >= self.profile.delivery.limits.events { return Err(Error::Limit.into()); }
        let bytes = journal::encode_appended(&self.profile, self.store.identity(), &self.events, &event)?;
        let mut candidate = Machine::replay(&self.profile, &self.events)?;
        let result = candidate.apply(&event)?;
        match self.persist_candidate(event, bytes, candidate, result)? {
            Transition::Inputs(revision) => Ok(revision),
            _ => unreachable!("publication input transition"),
        }
    }

    pub fn publication_validation_profile(&self) -> Result<Option<PublicationLimits>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.events.iter().find_map(|event| match event {
            Event::PublicationWitness(WitnessEvent::Enable(limits)) => Some(*limits), _ => None,
        }))
    }
    pub fn publication_input_revision(&self, attempt: u64) -> Result<u64, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.publication_input_revision(attempt)?)
    }
    /// Last acknowledged comparison, not a fresh permission or proof that an
    /// unsuccessful authorization attempt was journaled. Publication rechecks it.
    pub fn publication_validation(&self, attempt: u64) -> Result<Option<PublicationReport>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.publication_validation(attempt)?)
    }
    pub fn retained_publication_evidence(&self, attempt: u64) -> Result<&FilePublicationEvidence, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        self.events.iter().find_map(|event| match event {
            Event::PublicationWitness(WitnessEvent::Bind(id, evidence)) if *id == attempt => Some(evidence.as_ref()),
            Event::PublicationWitness(WitnessEvent::SourceBind(id, binding)) if *id == attempt => Some(&binding.evidence),
            _ => None,
        }).ok_or_else(|| Error::Missing.into())
    }
}

pub(in super::super) fn write(w: &mut Writer, event: &WitnessEvent) -> Result<(), Error> {
    match event {
        WitnessEvent::Enable(limits) => {
            w.u8(0)?; w.count(limits.bindings)?;
            w.u64(limits.validation.steps)?; w.u64(limits.validation.value_bytes)?;
        }
        WitnessEvent::Bind(id, evidence) => { w.u8(1)?; w.u64(*id)?; w.blob(&evidence.to_bytes()?)?; }
        WitnessEvent::Inputs(id, revision, inputs) => {
            w.u8(2)?; w.u64(*id)?; w.u64(*revision)?;
            match inputs {
                None => w.u8(0)?,
                Some(inputs) => { w.u8(1)?; w.blob(&inputs.to_bytes()?)?; }
            }
        }
        WitnessEvent::SourceBind(id, binding) => { w.u8(3)?; w.u64(*id)?; binding.write(w)?; }
        WitnessEvent::Captured(id, revision, capture) => {
            w.u8(4)?; w.u64(*id)?; w.u64(*revision)?; w.blob(&capture.to_bytes()?)?;
        }
        WitnessEvent::ChangeProfile(policy) => { w.u8(5)?; changes::write_policy(w, *policy)?; }
        WitnessEvent::Change(notice) => { w.u8(6)?; changes::write_change(w, *notice)?; }
    }
    Ok(())
}
pub(in super::super) fn read(r: &mut Reader<'_>) -> Result<WitnessEvent, Error> {
    Ok(match r.u8()? {
        0 => WitnessEvent::Enable(PublicationLimits { bindings: r.count(MAX_PUBLICATION_BINDINGS)?,
            validation: RefinementBudget { steps: r.u64()?, value_bytes: r.u64()? } }),
        1 => WitnessEvent::Bind(r.u64()?, Rc::new(FilePublicationEvidence::from_bytes(r.blob(MAX_PUBLICATION_PACKET_BYTES)?)?)),
        2 => { let id = r.u64()?; let revision = r.u64()?;
            let inputs = match r.u8()? {
                0 => None,
                1 => Some(Rc::new(FilePublicationInputs::from_bytes(r.blob(MAX_PUBLICATION_PACKET_BYTES)?)?)),
                _ => return Err(Error::InvalidInput),
            };
            WitnessEvent::Inputs(id, revision, inputs)
        }
        3 => WitnessEvent::SourceBind(r.u64()?, Rc::new(SourceBinding::read(r)?)),
        4 => WitnessEvent::Captured(r.u64()?, r.u64()?,
            Rc::new(FilePublicationCapture::from_bytes(r.blob(MAX_CAPTURE_BYTES)?)?)),
        5 => WitnessEvent::ChangeProfile(changes::read_policy(r)?),
        6 => WitnessEvent::Change(changes::read_change(r)?),
        _ => return Err(Error::InvalidInput),
    })
}
