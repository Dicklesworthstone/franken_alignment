//! Cooperative actual-model measurement feeding the original liveness authority.
use super::{DecoderBudget, DecoderIdentityProbe, OversightBroker};
use super::super::HostedLearnedInspection;
use crate::action::ElapsedTick;
use crate::action::consequence::activation::identity::{ModelManifest, decoder::{
    DecoderIdentityMeasurement, IdentityProbeProgress, IdentityProbeWork,
}};
use crate::action::consequence::oversight::identity::{IdentityChallenge, IdentityInstallation,
    IdentityObserver, IdentityOutcome, IdentityReport};
use crate::Error;
use std::rc::Rc;

/// Manifest commitments remain an independent trusted host observation. The
/// anchor frames, unlike this manifest, are computed from the owned decoder.
/// The registered passport is taken from the original gate, never this request.
#[derive(Clone, Debug)]
pub struct HostedIdentityCheckRequest {
    pub check: u64,
    pub expected_control_sequence: u64,
    pub expected_actor_revision: u64,
    pub observed_manifest: ModelManifest,
    pub measurement_sequence: u64,
    pub budget: DecoderBudget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostedIdentityCheckStatus { Measuring, ReadyToApply, Installed, Cancelled, Failed }
impl HostedIdentityCheckStatus {
    fn active(self) -> bool { matches!(self, Self::Measuring | Self::ReadyToApply) }
}

/// Setup always returns custody of the original observer on a refusal. If a
/// challenge was admitted before failure, its prior live basis is NOT restored.
#[derive(Debug)]
pub struct HostedIdentityStartFailure {
    pub error: Error,
    pub observer: IdentityObserver,
    pub challenge: Option<IdentityChallenge>,
    pub cleanup_failure: Option<Error>,
}

/// Own the sole measurement ingress while driving actual original anchor tokens.
/// A complete comparison yields before installing its original authority result.
/// There is no supplied-frame, verdict, model replacement or permit interface.
/// Dropping unfinished work releases its private cache and observer; the original
/// gate remains non-live. Use cancel before taking the observer back for reuse.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::learned_host::identity::HostedLearnedIdentityCheck;
/// fn replace(run: &mut HostedLearnedIdentityCheck) { run.submit_match(); }
/// ```
#[derive(Debug)]
pub struct HostedLearnedIdentityCheck {
    issuer: Rc<()>,
    observer: Option<IdentityObserver>,
    challenge: IdentityChallenge,
    source: HostedLearnedInspection,
    probe: Option<DecoderIdentityProbe>,
    measurements: Vec<DecoderIdentityMeasurement>,
    work: IdentityProbeWork,
    report: IdentityReport,
    installation: Option<IdentityInstallation>,
    revision: u64,
    elapsed: ElapsedTick,
    status: HostedIdentityCheckStatus,
    failure: Option<Error>,
    cleanup_failure: Option<Error>,
    interrupted: bool,
}

impl OversightBroker {
    /// Begin the SAME mandatory identity check and give it the separately owned
    /// observer. Setup performs no anchor inference. Post-admission budget,
    /// model-contract or observer failures close that check as unavailable rather
    /// than restoring a previously live identity. No mutable source escapes.
    pub fn begin_hosted_learned_identity_check(&mut self, observer: IdentityObserver,
        request: HostedIdentityCheckRequest) -> Result<HostedLearnedIdentityCheck, HostedIdentityStartFailure>
    {
        let mut admitted = None;
        let result = (|| {
            if request.measurement_sequence == 0 { return Err(Error::InvalidInput); }
            let source = self.hosted_learned_generation()?;
            if source.host_failure.is_some() { return Err(Error::WrongState); }
            let challenge = self.begin_identity_check(request.check, request.expected_control_sequence,
                request.expected_actor_revision)?;
            admitted = Some(challenge.clone());
            let report = observer.observe_manifest(&challenge, request.observed_manifest, challenge.started_at())?;
            let probe = match report.outcome {
                IdentityOutcome::Collecting => Some(self.hosted_learned_identity_probe(
                    request.expected_actor_revision, challenge.passport(), request.measurement_sequence, request.budget)?),
                IdentityOutcome::Mismatch(_) => None,
                _ => return Err(Error::WrongState),
            };
            let mut measurements = Vec::new();
            measurements.try_reserve_exact(challenge.passport().anchors().len()).map_err(|_| Error::Limit)?;
            Ok((source, challenge, report, probe, measurements))
        })();
        match result {
            Ok((source, challenge, report, probe, measurements)) => {
                let status = if probe.is_some() { HostedIdentityCheckStatus::Measuring }
                    else { HostedIdentityCheckStatus::ReadyToApply };
                let work = probe.as_ref().map_or_else(IdentityProbeWork::default, DecoderIdentityProbe::work);
                let elapsed = challenge.started_at();
                Ok(HostedLearnedIdentityCheck { issuer: Rc::clone(&self.issuer), observer: Some(observer),
                    challenge, source, probe, measurements, work, report, installation: None,
                    revision: 0, elapsed, status, failure: None, cleanup_failure: None, interrupted: false })
            }
            Err(error) => {
                let cleanup_failure = admitted.as_ref().and_then(|challenge| retire(self, challenge));
                Err(HostedIdentityStartFailure { error, observer, challenge: admitted, cleanup_failure })
            }
        }
    }
}

impl HostedLearnedIdentityCheck {
    pub fn status(&self) -> HostedIdentityCheckStatus { self.status }
    pub fn revision(&self) -> u64 { self.revision }
    pub fn challenge(&self) -> &IdentityChallenge { &self.challenge }
    pub fn work(&self) -> IdentityProbeWork { self.work }
    pub fn measurements(&self) -> &[DecoderIdentityMeasurement] { &self.measurements }
    pub fn report(&self) -> &IdentityReport { &self.report }
    pub fn installation(&self) -> Option<&IdentityInstallation> { self.installation.as_ref() }
    pub fn failure(&self) -> Option<Error> { self.failure }
    pub fn cleanup_failure(&self) -> Option<Error> { self.cleanup_failure }
    /// A token call began but did not return. Last-returned work can understate
    /// that call; its full original planned allowance remains visible in work.
    pub fn interrupted(&self) -> bool { self.interrupted }

    /// Recover the SAME role after a terminal result, not a cloned ingress.
    pub fn take_observer(&mut self) -> Result<IdentityObserver, Error> {
        if self.status.active() { return Err(Error::WrongState); }
        self.observer.take().ok_or(Error::Missing)
    }
    fn bind(&self, owner: &OversightBroker, revision: u64) -> Result<(), Error> {
        if !Rc::ptr_eq(&self.issuer, &owner.issuer) { return Err(Error::Binding); }
        if revision != self.revision { return Err(Error::Stale); }
        if !self.status.active() { return Err(Error::WrongState); }
        Ok(())
    }
    fn current(&self, owner: &OversightBroker) -> Result<(), Error> {
        let control = owner.inspect();
        if owner.identity_basis()? != self.challenge.basis()
            || control.sequence != self.challenge.control_sequence()
            || control.ledger.epoch != self.challenge.revocation_epoch()
            || owner.hosted_learned_generation()? != self.source { return Err(Error::Stale); }
        if owner.identity_report(self.challenge.id())? != self.report { return Err(Error::Binding); }
        Ok(())
    }

    /// One ORIGINAL anchor token, or the original completed-check installation.
    /// Stale revisions, foreign owners and backward clocks enter no operation.
    /// Source/control changes terminate this run without adopting a newer basis.
    pub fn advance(&mut self, owner: &mut OversightBroker, expected_revision: u64, now: ElapsedTick)
        -> Result<HostedIdentityCheckStatus, Error>
    {
        self.bind(owner, expected_revision)?;
        if now < self.elapsed || owner.inspect().ledger.elapsed.is_some_and(|time| now < time) {
            return Err(Error::Stale);
        }
        self.revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        let previous = self.status;
        // An unwind cannot re-enter the same token or install a partial result.
        self.status = HostedIdentityCheckStatus::Failed;
        self.failure = Some(Error::Incomplete);
        let result = (|| {
            self.current(owner)?;
            owner.observe_time(now)?;
            self.elapsed = now;
            if previous == HostedIdentityCheckStatus::ReadyToApply {
                let installed = owner.apply_identity_check(&self.challenge,
                    self.challenge.control_sequence(), self.challenge.revocation_epoch())?;
                self.installation = Some(installed);
                return Ok(HostedIdentityCheckStatus::Installed);
            }
            if now >= self.challenge.deadline() {
                owner.expire_identity_check(&self.challenge)?;
                return Err(Error::Stale);
            }
            self.measure()
        })();
        match result {
            Ok(status) => { self.status = status; self.failure = None; Ok(status) }
            Err(error) => {
                self.probe = None;
                self.failure = Some(error);
                self.cleanup_failure = retire(owner, &self.challenge);
                if let Ok(report) = owner.identity_report(self.challenge.id()) { self.report = report; }
                Err(error)
            }
        }
    }
    fn measure(&mut self) -> Result<HostedIdentityCheckStatus, Error> {
        // The private numerical owner is stack-owned across computation, so a
        // caught unwind releases it while preserving the cursor's failure latch.
        let mut probe = self.probe.take().ok_or(Error::WrongState)?;
        self.interrupted = true;
        let result = probe.advance();
        self.work = probe.work();
        self.interrupted = false;
        match result? {
            IdentityProbeProgress::Advanced => {
                self.probe = Some(probe);
                Ok(HostedIdentityCheckStatus::Measuring)
            }
            IdentityProbeProgress::Measured(measurement) => {
                self.measurements.push(*measurement);
                let measured = self.measurements.last().expect("just retained original measurement");
                self.report = self.observer.as_ref().ok_or(Error::Missing)?.observe_anchor(
                    &self.challenge, measured.anchor(), measured.source(), self.elapsed)?;
                match self.report.outcome {
                    IdentityOutcome::Matched if probe.complete() => Ok(HostedIdentityCheckStatus::ReadyToApply),
                    IdentityOutcome::Mismatch(_) => Ok(HostedIdentityCheckStatus::ReadyToApply),
                    IdentityOutcome::Collecting if !probe.complete() => {
                        self.probe = Some(probe);
                        Ok(HostedIdentityCheckStatus::Measuring)
                    }
                    _ => Err(Error::Incomplete),
                }
            }
            IdentityProbeProgress::Complete => Err(Error::Incomplete),
        }
    }
    /// Release unfinished inference and invalidate only this original basis.
    /// A shared mismatch remains latched; cancellation cannot turn it into a pass.
    pub fn cancel(&mut self, owner: &mut OversightBroker, expected_revision: u64) -> Result<(), Error> {
        self.bind(owner, expected_revision)?;
        self.revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        self.probe = None;
        self.cleanup_failure = retire(owner, &self.challenge);
        if let Ok(report) = owner.identity_report(self.challenge.id()) { self.report = report; }
        self.status = HostedIdentityCheckStatus::Cancelled;
        self.failure = None;
        match self.cleanup_failure { Some(error) => Err(error), None => Ok(()) }
    }
}

fn retire(owner: &mut OversightBroker, challenge: &IdentityChallenge) -> Option<Error> {
    match owner.identity_basis() {
        Ok(basis) if basis == challenge.basis() => owner.identity_unavailable(basis).err(),
        Ok(_) => None, // Never invalidate a newer, independently begun challenge.
        Err(error) => Some(error),
    }
}
