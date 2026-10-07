//! Cooperative preparation, followed by ONE original verified identity record.
//! Partial measurements never enter the live matcher or escape as observations.
use super::super::{FileIdentityChallenge, FileIdentityObserver, FileOversight, JournalError,
    Event, IdentityEvent, Machine, Transition, journal};
use super::super::computed::{ComputedIdentityEvent, FileComputedIdentityObservation,
    FileLearnedIdentityInput, IdentityComputation};
use crate::action::ElapsedTick;
use crate::action::consequence::activation::identity::decoder::IdentityProbeWork;
use crate::action::consequence::delivery::persistent::{JournalFailure, JournalIo};
use crate::Error;
use std::{fmt, rc::Rc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComputedIdentityStatus { Computing, ReadyToCommit, Committed, Cancelled, Failed }

/// No challenge or work is consumed by failed setup. The independently held
/// observer is returned, not silently discarded or cloned into another owner.
#[derive(Debug)]
pub struct ComputedIdentitySetupError {
    pub error: JournalError,
    pub observer: FileIdentityObserver,
}

/// Pins one acknowledged journal revision and consumes the separate observer.
/// One advance computes at most one original token. Canonical commit records the
/// same complete witness as the synchronous API; it does not repeat these tokens.
/// Journal reconstruction at setup and filesystem sync at commit are synchronous.
/// This is not durable partial-computation recovery or a global work escrow.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::identity::decoder::transaction::FileComputedIdentityRun;
/// fn replace(run: &mut FileComputedIdentityRun) { run.probe_mut(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::identity::decoder::transaction::FileComputedIdentityRun;
/// fn duplicate(run: FileComputedIdentityRun) { let _ = run.clone(); }
/// ```
#[must_use = "commit or cancel the prepared check before releasing observer custody"]
pub struct FileComputedIdentityRun {
    observer: Option<FileIdentityObserver>,
    challenge: FileIdentityChallenge,
    event: ComputedIdentityEvent,
    base_revision: u64,
    revision: u64,
    elapsed: ElapsedTick,
    status: ComputedIdentityStatus,
    staged: Option<(Machine, IdentityComputation)>,
    work: IdentityProbeWork,
    interrupted: bool,
    failure: Option<JournalError>,
    result: Option<FileComputedIdentityObservation>,
}
impl fmt::Debug for FileComputedIdentityRun {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileComputedIdentityRun").field("check", &self.challenge.id())
            .field("status", &self.status).field("revision", &self.revision)
            .field("work", &self.work).finish_non_exhaustive()
    }
}

impl FileIdentityObserver {
    /// The original challenge must already have withdrawn prior eligibility.
    /// Freeze custody and prepare a private original machine, but perform no
    /// new stimulus inference or canonical write. Earlier history replay is
    /// separately priced work. Only a completed acknowledged record can expose
    /// the private manifest/matching results, including an immediate mismatch.
    pub fn begin_computed_learned(self, host: &FileOversight, revision: u64,
        challenge: &FileIdentityChallenge, input: FileLearnedIdentityInput, started_at: ElapsedTick)
        -> Result<FileComputedIdentityRun, ComputedIdentitySetupError>
    {
        let prepared = (|| {
            self.check(host, challenge)?;
            if revision != host.revision() { return Err(Error::Stale.into()); }
            let event = ComputedIdentityEvent { check: challenge.id(), input, started_at,
                completed_at: started_at, witness: Rc::from(&b""[..]) };
            event.validate()?;
            host.machine.preflight_computed_identity(challenge.id(), &event.input)?;
            if started_at < host.inspect().control.ledger.elapsed.ok_or(Error::Incomplete)? {
                return Err(Error::Stale.into());
            }
            if host.events.len() >= host.profile.delivery.limits.events { return Err(Error::Limit.into()); }
            host.check_source_admission(&Event::Identity(IdentityEvent::Computed(event.clone())))?;
            let mut candidate = Machine::replay(&host.profile, &host.events)?;
            let computation = candidate.start_computed_identity(&event)?;
            Ok::<_, JournalError>((event, candidate, computation))
        })();
        match prepared {
            Err(error) => Err(ComputedIdentitySetupError { error, observer: self }),
            Ok((event, candidate, computation)) => {
                let status = if computation.ready() { ComputedIdentityStatus::ReadyToCommit }
                    else { ComputedIdentityStatus::Computing };
                let work = computation.work();
                Ok(FileComputedIdentityRun { observer: Some(self), challenge: challenge.clone(), event,
                    base_revision: revision, revision: 0, elapsed: started_at, status,
                    staged: Some((candidate, computation)), work, interrupted: false, failure: None, result: None })
            }
        }
    }
}

impl FileComputedIdentityRun {
    pub fn status(&self) -> ComputedIdentityStatus { self.status }
    pub fn revision(&self) -> u64 { self.revision }
    pub fn work(&self) -> IdentityProbeWork { self.work }
    /// True only when an admitted native token did not return its work report.
    /// The original planned bound covers that unreported partial operation.
    pub fn interrupted(&self) -> bool { self.interrupted }
    pub fn failure(&self) -> Option<&JournalError> { self.failure.as_ref() }
    /// Never returns the private candidate or a pre-acknowledgment measurement.
    pub fn result(&self) -> Option<&FileComputedIdentityObservation> { self.result.as_ref() }

    fn bind(&self, host: &FileOversight, revision: u64) -> Result<(), JournalError> {
        self.observer.as_ref().ok_or(Error::WrongState)?.check(host, &self.challenge)?;
        if revision != self.revision { return Err(Error::Stale.into()); }
        Ok(())
    }
    fn current(&self, host: &FileOversight) -> Result<(), JournalError> {
        if host.revision() != self.base_revision { return Err(Error::Stale.into()); }
        host.check_source_admission(&Event::Identity(IdentityEvent::Computed(self.event.clone())))?;
        Ok(())
    }
    fn latch(&mut self) -> Result<(), JournalError> {
        self.revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        self.status = ComputedIdentityStatus::Failed;
        self.failure = Some(Error::Incomplete.into());
        Ok(())
    }

    /// One token with fresh pre/post receipt times. Foreign or stale cursor calls
    /// perform no work. An admitted error closes private computation; custody
    /// remains held until explicit cancellation withdraws this original basis.
    pub fn advance_with_clock<F>(&mut self, host: &FileOversight, revision: u64, mut clock: F)
        -> Result<ComputedIdentityStatus, JournalError>
    where F: FnMut() -> ElapsedTick {
        self.bind(host, revision)?;
        if self.status != ComputedIdentityStatus::Computing { return Err(Error::WrongState.into()); }
        self.latch()?;
        // Taking both objects before callbacks makes unwinding discard partial
        // numerical state without rolling back the in-place work/latch records.
        let staged = self.staged.take().ok_or(Error::WrongState)?;
        let (candidate, mut computation) = staged;
        let result = (|| {
            self.current(host)?;
            let started = clock();
            if started < self.elapsed || started >= self.challenge.evidence().deadline() {
                return Err(Error::Stale.into());
            }
            self.elapsed = started;
            self.interrupted = true;
            let numerical = computation.advance();
            self.work = computation.work();
            self.interrupted = false;
            numerical?;
            let received = clock();
            if received < self.elapsed || received >= self.challenge.evidence().deadline() {
                return Err(Error::Stale.into());
            }
            self.elapsed = received;
            Ok::<_, JournalError>(if computation.ready() { ComputedIdentityStatus::ReadyToCommit }
                else { ComputedIdentityStatus::Computing })
        })();
        match result {
            Ok(status) => {
                self.staged = Some((candidate, computation));
                self.status = status; self.failure = None; Ok(status)
            }
            Err(error) => { self.failure = Some(error.clone()); Err(error) }
        }
    }

    /// No new stimulus inference. Final receipt time and original comparisons
    /// are sealed into the same tag-6 event used by synchronous execution. A
    /// complete computation whose commit is late records original withdrawal.
    /// Matching still needs separate identity installation and both effect keys.
    pub fn commit_with_clock<F>(&mut self, host: &mut FileOversight, revision: u64, mut clock: F)
        -> Result<&FileComputedIdentityObservation, JournalError>
    where F: FnMut() -> ElapsedTick {
        self.bind(host, revision)?;
        if self.status != ComputedIdentityStatus::ReadyToCommit { return Err(Error::Incomplete.into()); }
        self.latch()?;
        let (mut candidate, computation) = self.staged.take().ok_or(Error::WrongState)?;
        let result = (|| {
            self.current(host)?;
            if host.events.len() >= host.profile.delivery.limits.events { return Err(Error::Limit.into()); }
            host.events.try_reserve(1).map_err(|_| Error::Limit)?;
            // Match the original synchronous publication boundary: a callback,
            // encoding or ambiguous storage failure cannot expose older RAM as
            // permission to retry a computation whose result may be committed.
            host.fault = Some(JournalFailure { operation: JournalIo::Stage,
                kind: std::io::ErrorKind::Other, replacement_may_be_visible: false });
            let completed_at = clock();
            if completed_at < self.elapsed { return Err(Error::Stale.into()); }
            let (event, result) = candidate.finish_prepared_computed_identity(
                self.event.clone(), computation, completed_at)?;
            let event = Event::Identity(IdentityEvent::Computed(event));
            let bytes = journal::encode_appended(&host.profile, host.store.identity(), &host.events, &event)?;
            host.persist_candidate(event, bytes, candidate, Transition::Unit)?;
            Ok::<_, JournalError>(result)
        })();
        match result {
            Ok(result) => {
                self.result = Some(result); self.status = ComputedIdentityStatus::Committed; self.failure = None;
                Ok(self.result.as_ref().expect("canonical acknowledgment precedes result"))
            }
            Err(error) => { self.failure = Some(error.clone()); Err(error) }
        }
    }

    /// Release private work and withdraw only this check's still-current basis.
    /// Cleanup remains available after failed preparation. It never withdraws a
    /// successor basis, issues a key, settles an effect or refunds spent work.
    /// Failed cleanup retains observer custody and its original failure record.
    pub fn cancel(&mut self, host: &mut FileOversight, revision: u64) -> Result<(), JournalError> {
        self.bind(host, revision)?;
        if matches!(self.status, ComputedIdentityStatus::Committed | ComputedIdentityStatus::Cancelled) {
            return Err(Error::WrongState.into());
        }
        self.revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        self.status = ComputedIdentityStatus::Failed;
        self.staged = None;
        let result = (|| {
            let basis = host.identity_basis()?;
            if basis == self.challenge.evidence().basis() {
                host.identity_unavailable(host.revision(), basis)?;
            }
            Ok::<_, JournalError>(())
        })();
        match result {
            Ok(()) => { self.status = ComputedIdentityStatus::Cancelled; Ok(()) }
            Err(error) => { self.failure.get_or_insert_with(|| error.clone()); Err(error) }
        }
    }

    /// Return the unique original role only after acknowledged completion or
    /// cancellation. A failed/abandoned private computation cannot recover a
    /// manual or synchronous fallback for its still-pending challenge.
    pub fn take_observer(&mut self) -> Result<FileIdentityObserver, Error> {
        if !matches!(self.status, ComputedIdentityStatus::Committed | ComputedIdentityStatus::Cancelled) {
            return Err(Error::WrongState);
        }
        self.observer.take().ok_or(Error::Missing)
    }
}
