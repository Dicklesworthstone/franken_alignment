//! Two independent acquisitions around native dispatch in one effect cut.
//! Only the ORIGINAL journal-as-publication sink is atomic. The pre-read
//! withdrawal is durably acknowledged separately; no remote-effect claim.
pub mod feed;
use super::heartbeat::feed::{PublicationFeedFile, PublicationFeedReport};
use crate::action::consequence::delivery::publication_gate::changes::freshness::PublicationHeartbeat;
use super::super::witness_gate::freshness::FreshnessEvent;
use super::{FileCaptureError, FileCaptureIdentity, PublicationInputFile};
use super::super::{CheckedPublication, witness_gate::WitnessEvent};
use super::super::super::{BaseEvent, Event, FileHumanPermit, FileOversight, FilePermit,
    JournalError, JournalFailure, JournalIo, Machine, Reconciliation, Transition, journal};
use super::super::super::credential::FileCredentialPermit;
use crate::action::{ActionState, ElapsedTick, FrozenAction};
use crate::action::consequence::oversight::{CommitteeContract, CommitteeInput};
use crate::action::consequence::oversight::supervised::DriverEvidence;
use crate::{Error, Snapshot};
use std::rc::Rc;

/// Borrowed ORIGINAL capabilities, not an authority constructor. The optional
/// credential must belong to this owner and its current nonsecret policy.
pub struct CapturedCompletionKeys<'a> {
    pub automatic: &'a FilePermit,
    pub human: &'a FileHumanPermit,
    pub credential: Option<&'a FileCredentialPermit>,
}

/// At most two actual witness reads, including a successful read whose later
/// installation refused. Only result=Ok acknowledges publication AND settlement.
/// Read/provider failure before dispatch leaves the reservation intact. After
/// staged dispatch, ordinary missing evidence seals through the original endpoint.
#[derive(Debug)]
pub struct CapturedCompletionReport {
    pub reads: Vec<Result<FileCaptureIdentity, FileCaptureError>>,
    pub evidence_failure: Option<Error>,
    pub result: Result<CheckedPublication, JournalError>,
}

impl FileOversight {
    /// Complete an already authorized source-bound attempt without an externally
    /// visible dispatch/publication/accounting gap. The provider is invoked twice;
    /// each successful committee capture is followed by a NEW concrete file read
    /// and a trusted clock sample. No capture is duplicated to satisfy two gates.
    ///
    /// The first withdrawal commits before any provider/clock code. Subsequent
    /// events run on a private replay of the SAME broker and endpoint. The second
    /// provider sees only action/contracts, never the candidate authority. Changed
    /// evidence can seal the original request, not recapture its reviewed binding.
    ///
    /// A malformed installation, stale clock, exhausted journal or failed final
    /// replacement after the first successful read quarantines this owner. Reopen
    /// the original canonical journal: it contains either withdrawn undispatched
    /// work or the whole completed effect. Never infer an outcome from an I/O error.
    /// Callback evidence does NOT renew a separately configured policy-source lease.
    pub fn complete_publication_from_source<F, P>(&mut self, revision: u64,
        keys: CapturedCompletionKeys<'_>, source: &PublicationInputFile, clock: F, provider: P)
        -> CapturedCompletionReport
    where F: FnMut() -> ElapsedTick,
        P: FnMut(&FrozenAction, &CommitteeContract) -> Result<DriverEvidence, Error>,
    {
        let mut completion = Completion { keys, source, clock, provider,
            reads: Vec::with_capacity(2), evidence_failure: None, feed: None, feed_reads: Vec::new(), feeds: Vec::new() };
        let result = completion.run(self, revision);
        CapturedCompletionReport { reads: completion.reads,
            evidence_failure: completion.evidence_failure, result }
    }
}

struct Completion<'a, F, P> {
    keys: CapturedCompletionKeys<'a>,
    source: &'a PublicationInputFile,
    feed: Option<&'a PublicationFeedFile>,
    feed_reads: Vec<Result<PublicationHeartbeat, FileCaptureError>>,
    feeds: Vec<PublicationFeedReport>,
    clock: F,
    provider: P,
    reads: Vec<Result<FileCaptureIdentity, FileCaptureError>>,
    evidence_failure: Option<Error>,
}
impl<F, P> Completion<'_, F, P>
where F: FnMut() -> ElapsedTick,
    P: FnMut(&FrozenAction, &CommitteeContract) -> Result<DriverEvidence, Error>,
{
    fn sample(&mut self, action: &FrozenAction, contracts: &CommitteeContract)
        -> Result<DriverEvidence, Error>
    {
        let result = (self.provider)(action, contracts).and_then(|evidence| {
            if !evidence.snapshot.complete { return Err(Error::Incomplete); }
            evidence.inputs.as_ref().ok_or(Error::Incomplete)?.validate_for(action, contracts)?;
            let snapshot = &evidence.snapshot;
            if snapshot.values.len() > super::super::super::super::MAX_SNAPSHOT_ENTRIES {
                return Err(Error::Limit);
            }
            let bytes = snapshot.values.values().try_fold(0_usize, |n, value| n.checked_add(value.len()))
                .ok_or(Error::Limit)?;
            if bytes > super::super::super::super::MAX_SNAPSHOT_BYTES { return Err(Error::Limit); }
            Ok(evidence)
        });
        if let Err(error) = &result { self.evidence_failure = Some(*error); }
        result
    }

    fn read(&mut self) -> Result<super::FilePublicationCapture, FileCaptureError> {
        let captured = self.source.read_capture();
        self.reads.push(captured.as_ref().map(|capture| capture.identity()).map_err(|error| *error));
        captured
    }

    fn run(&mut self, host: &mut FileOversight, revision: u64) -> Result<CheckedPublication, JournalError> {
        if host.fault.is_some() { return Err(JournalError::Unavailable); }
        if host.revision() != revision { return Err(Error::Stale.into()); }
        let attempt = self.keys.automatic.attempt;
        if !Rc::ptr_eq(&host.issuer, &self.keys.automatic.issuer)
            || !Rc::ptr_eq(&host.issuer, &self.keys.human.issuer)
            || self.keys.human.attempt != attempt { return Err(Error::Binding.into()); }
        if host.inspect().control.ledger.stages.get(&attempt) != Some(&ActionState::Authorized) {
            return Err(Error::WrongState.into());
        }
        if !host.publication_guard_required() { return Err(Error::Incomplete.into()); }
        match self.keys.credential {
            Some(permit) => host.check_credential_permit(permit)?,
            None if host.credential_policy().is_some() => return Err(Error::Incomplete.into()),
            None => {}
        }
        // Bound the fixed completion events before I/O. Feed suffix lengths are
        // known only after reading; every added notice still passes canonical
        // event/byte and recovery-reserve admission before candidate execution.
        let fixed = if self.feed.is_some() { 12 } else { 8 };
        if host.events.len().checked_add(fixed).ok_or(Error::Overflow)? > host.profile.delivery.limits.events {
            return Err(Error::Limit.into());
        }
        let action = host.machine.actions.get(&attempt).ok_or(Error::Missing)?.clone();
        if let Some(feed) = self.feed {
            let bound = host.publication_source(attempt)?.ok_or(Error::Incomplete)?;
            if bound.source != self.source.source() { return Err(Error::Binding.into()); }
            host.publication_change_freshness()?;
            if host.publication_change_status()?.source != feed.source() { return Err(Error::Binding.into()); }
            // Both live feed and witness eligibility are withdrawn before any
            // external read. Staged catch-up can then change the witness revision.
            host.publication_changes_unavailable(revision, feed.source())?;
        }
        host.begin_publication_capture(host.revision(), attempt, self.source.source())?;
        let mut cut = SourceCut::new(host)?;
        if self.feed.is_some() {
            self.capture_feed(host, &mut cut)?.map_err(|error| JournalError::from(error.contract_error()))?;
        }
        let expected = cut.machine.broker.publication_input_revision(attempt)?;
        let first = self.sample(&action, &host.profile.committee)?;
        let first_inputs = first.inputs.as_ref().ok_or(Error::Incomplete)?;
        let committee_revision = host.current_reference(attempt, first_inputs)?;
        let captured = self.read().map_err(|error| JournalError::from(error.contract_error()))?;
        // An admitted producer observation may carry a new high-water mark. From
        // here, no failure/unwind may revive an older, quieter live observation.
        host.fault = Some(failure(false));
        let now = (self.clock)();
        cut.stage(host, Event::Core(BaseEvent::Time(now)))?;
        cut.stage(host, Event::PublicationWitness(WitnessEvent::Captured(attempt, expected, Rc::new(captured))))?;
        cut.stage(host, Event::Dispatch(attempt, self.keys.human.request, committee_revision, first.snapshot))?;

        if let Some(feed) = self.feed {
            cut.stage(host, Event::PublicationWitness(WitnessEvent::Freshness(
                FreshnessEvent::Unavailable(feed.source()))))?;
            // An ordinary second read failure leaves the staged feed unavailable.
            // Continue to the original publication/seal and receipt path; do not
            // fabricate changed committee evidence or a successful installation.
            let _read = self.capture_feed(host, &mut cut)?;
        }
        // This withdrawal is staged after native dispatch, but the canonical owner
        // has ALREADY durably withdrawn before both external reads. A caught panic
        // cannot expose this candidate or leave an old sendable source capture.
        let expected = cut.machine.broker.publication_input_revision(attempt)?;
        cut.stage(host, Event::PublicationWitness(WitnessEvent::Inputs(attempt, expected, None)))?;
        let second = self.sample(&action, &host.profile.committee);
        let (inputs, snapshot) = match second {
            Ok(evidence) => match self.read() {
                Ok(captured) => {
                    let expected = cut.machine.broker.publication_input_revision(attempt)?;
                    cut.stage(host, Event::PublicationWitness(WitnessEvent::Captured(attempt, expected, Rc::new(captured))))?;
                    (evidence.inputs, evidence.snapshot)
                }
                Err(_) => (None, Snapshot::default()),
            },
            Err(_) => (None, Snapshot::default()),
        };
        let now = (self.clock)();
        let views = inputs.as_ref().map(CommitteeInput::views).cloned();
        let publication = match self.keys.credential {
            Some(permit) => {
                host.check_credential_permit(permit)?;
                Event::PublishCredentialed(attempt, views, snapshot, now)
            }
            None => Event::PublishChecked(attempt, views, snapshot, now),
        };
        let Transition::PublicationChecked(publication) = cut.stage(host, publication)? else {
            return Err(Error::Binding.into());
        };
        match cut.stage(host, Event::Core(BaseEvent::Reconcile(attempt)))? {
            Transition::Reconciled(Reconciliation::Resolved(outcome)) if outcome == publication.outcome => {}
            _ => return Err(Error::Binding.into()),
        }
        cut.persist(host)?;
        Ok(publication)
    }
}

fn failure(visible: bool) -> JournalFailure {
    JournalFailure { operation: JournalIo::Stage, kind: std::io::ErrorKind::Other,
        replacement_may_be_visible: visible }
}

/// Private original-event composition, never a public event importer. No native
/// handle escapes before the locked canonical Store acknowledges the whole cut.
struct SourceCut {
    base_revision: u64,
    history: Vec<Event>,
    machine: Machine,
    bytes: Vec<u8>,
}
impl SourceCut {
    fn new(host: &FileOversight) -> Result<Self, JournalError> {
        let mut history = Vec::new();
        history.try_reserve_exact(host.events.len().checked_add(7).ok_or(Error::Overflow)?)
            .map_err(|_| Error::Limit)?;
        history.extend(host.events.iter().cloned());
        Ok(Self { base_revision: host.revision(), history,
            machine: Machine::replay(&host.profile, &host.events)?, bytes: Vec::new() })
    }
    fn stage(&mut self, host: &FileOversight, event: Event) -> Result<Transition, JournalError> {
        host.check_source_admission(&event)?;
        self.history.try_reserve(1).map_err(|_| Error::Limit)?;
        self.bytes = journal::encode_appended(&host.profile, host.store.identity(), &self.history, &event)?;
        self.machine.preflight_consistency(&event)?;
        let result = self.machine.apply(&event)?;
        self.history.push(event);
        Ok(result)
    }
    fn persist(self, host: &mut FileOversight) -> Result<(), JournalError> {
        if host.revision() != self.base_revision { return Err(Error::Stale.into()); }
        host.fault = Some(failure(true));
        if let Err(error) = host.store.replace(&self.bytes) {
            host.fault = Some(match &error {
                JournalError::Io(failure) => failure.clone(), _ => failure(true),
            });
            return Err(error);
        }
        // No source event is generated here that clears source_interrupted. Keep
        // the same original hook so acknowledgments retain their native meaning.
        for event in &self.history[host.events.len()..] { host.source_operation_committed(event); }
        host.events = self.history;
        host.machine = self.machine;
        host.fault = None;
        Ok(())
    }
}
