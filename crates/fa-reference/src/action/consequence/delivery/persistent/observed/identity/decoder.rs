//! Cooperative actual-decoder measurements feeding the ORIGINAL durable gate.
//! This adapter owns no effect key, alternate verdict reducer or journal format.

use super::{FileIdentityChallenge, FileIdentityObservation, FileIdentityObserver, FileOversight, JournalError};
use crate::action::ElapsedTick;
use crate::action::consequence::activation::identity::ModelManifest;
use crate::action::consequence::activation::identity::decoder::{
    DecoderIdentityProbe, IdentityProbeProgress, IdentityProbeWork,
};
use crate::action::consequence::oversight::identity::{IdentityInstallation, IdentityOutcome};
use crate::Error;
use std::fmt;

#[derive(Debug)]
pub enum FileDecoderIdentityEvent {
    Manifest(FileIdentityObservation),
    Advanced,
    Measured { anchor: u64, observation: FileIdentityObservation },
    Installed(IdentityInstallation),
    /// Numeric failure or deadline loss and its separate durable withdrawal.
    /// Failed withdrawal is NOT permission to use a saved matching identity.
    Withdrawn { reason: Error, withdrawal: Result<u64, JournalError> },
}

#[derive(Debug)]
pub struct FileDecoderIdentityProgress {
    pub work: IdentityProbeWork,
    pub event: FileDecoderIdentityEvent,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase { Manifest, Compute, Install }

/// A single original challenge with a frozen, initially unexecuted decoder probe.
/// No model replacement, sequence relabeling, resume-after-reopen or raw-frame
/// submission method exists here. Correspondence to the deployed model and the
/// manifest's digest fields still require trusted host provisioning.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::identity::decoder::FileDecoderIdentityProbe;
/// fn duplicate(probe: FileDecoderIdentityProbe) { let _ = probe.clone(); }
/// ```
pub struct FileDecoderIdentityProbe {
    challenge: FileIdentityChallenge,
    probe: DecoderIdentityProbe,
    manifest: ModelManifest,
    phase: Phase,
    active: bool,
}
impl fmt::Debug for FileDecoderIdentityProbe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileDecoderIdentityProbe").field("check", &self.challenge.id())
            .field("active", &self.active).field("work", &self.probe.work()).finish_non_exhaustive()
    }
}

impl FileIdentityObserver {
    /// Bind before any measurement or numerical work. The already begun native
    /// challenge has withdrawn earlier eligibility. A setup refusal cannot restore
    /// it. This accepts no cached frame, caller-selected anchor subset or verdict.
    pub fn decoder_probe(&self, host: &FileOversight, challenge: &FileIdentityChallenge,
        probe: DecoderIdentityProbe, manifest: ModelManifest) -> Result<FileDecoderIdentityProbe, JournalError>
    {
        self.check(host, challenge)?;
        if probe.passport() != challenge.evidence().passport() { return Err(Error::Binding.into()); }
        if probe.work().entered_tokens != 0 || probe.failure().is_some() || probe.complete() {
            return Err(Error::WrongState.into());
        }
        let run = FileDecoderIdentityProbe { challenge: challenge.clone(), probe, manifest,
            phase: Phase::Manifest, active: true };
        run.check_context(host)?;
        Ok(run)
    }
}

impl FileDecoderIdentityProbe {
    pub fn work(&self) -> IdentityProbeWork { self.probe.work() }
    pub fn is_closed(&self) -> bool { !self.active }
    pub fn challenge(&self) -> &FileIdentityChallenge { &self.challenge }

    /// One manifest operation, one decoder token, or one original installation.
    /// Read the trusted clock again AFTER numerical work. Neither collecting a
    /// matched set nor a partial progress report constitutes an installed check.
    /// The runner retires before callbacks/storage; a caught unwind or ambiguous
    /// write cannot automatically recompute, resubmit a frame or install a result.
    /// Original native recovery/withdrawal remains available independently.
    pub fn step_with_clock<F>(&mut self, host: &mut FileOversight,
        observer: &FileIdentityObserver, mut clock: F) -> Result<FileDecoderIdentityProgress, JournalError>
    where F: FnMut() -> ElapsedTick {
        if !self.active { return Err(Error::WrongState.into()); }
        observer.check(host, &self.challenge)?;
        self.active = false;
        self.check_context(host)?;
        let now = clock();
        observe(host, now)?;
        let cutoff = if self.phase == Phase::Install {
            self.challenge.evidence().valid_until()
        } else { self.challenge.evidence().deadline() };
        if now >= cutoff { return Ok(self.withdraw(host, Error::Stale)); }
        let (event, continuing) = match self.phase {
            Phase::Manifest => {
                let revision = host.revision();
                let observation = observer.observe_manifest(host, revision, &self.challenge,
                    self.manifest.clone(), now)?;
                let continuing = observation.measurement.as_ref()
                    .is_ok_and(|report| report.outcome == IdentityOutcome::Collecting);
                self.phase = Phase::Compute;
                (FileDecoderIdentityEvent::Manifest(observation), continuing)
            }
            Phase::Compute => {
                let computed = self.probe.advance();
                let completed_at = clock();
                observe(host, completed_at)?;
                if completed_at >= self.challenge.evidence().deadline() {
                    return Ok(self.withdraw(host, Error::Stale));
                }
                match computed {
                    Err(error) => return Ok(self.withdraw(host, error)),
                    Ok(IdentityProbeProgress::Advanced) => (FileDecoderIdentityEvent::Advanced, true),
                    Ok(IdentityProbeProgress::Measured(measurement)) => {
                        let revision = host.revision();
                        let observation = observer.observe_anchor(host, revision, &self.challenge,
                            measurement.anchor(), measurement.source(), completed_at)?;
                        let continuing = match observation.measurement.as_ref().map(|report| report.outcome) {
                            Ok(IdentityOutcome::Collecting) => !self.probe.complete(),
                            Ok(IdentityOutcome::Matched) if self.probe.complete() => {
                                self.phase = Phase::Install;
                                true
                            }
                            _ => false,
                        };
                        (FileDecoderIdentityEvent::Measured { anchor: measurement.anchor(), observation }, continuing)
                    }
                    Ok(IdentityProbeProgress::Complete) => return Ok(self.withdraw(host, Error::WrongState)),
                }
            }
            Phase::Install => {
                let evidence = self.challenge.evidence();
                let installed = host.apply_identity_check(host.revision(), &self.challenge,
                    evidence.control_sequence(), evidence.revocation_epoch())?;
                (FileDecoderIdentityEvent::Installed(installed), false)
            }
        };
        self.active = continuing;
        Ok(FileDecoderIdentityProgress { work: self.work(), event })
    }

    fn check_context(&self, host: &FileOversight) -> Result<(), JournalError> {
        if !host.clock_ready() { return Err(Error::Incomplete.into()); }
        let expected = self.challenge.evidence();
        let control = host.inspect().control;
        if host.identity_basis()? != expected.basis() || control.sequence != expected.control_sequence()
            || control.ledger.epoch != expected.revocation_epoch()
            || host.machine.broker.actor_revision() != expected.actor_revision()
        { return Err(Error::Stale.into()); }
        if control.suspended || host.identity_installation(self.challenge.id())?.is_some() {
            return Err(Error::WrongState.into());
        }
        let report = host.identity_report(self.challenge.id())?;
        // Do not mix manual observations into a partially computed check.
        if report.observations.len() != self.probe.work().measured_anchors {
            return Err(Error::Binding.into());
        }
        match self.phase {
            Phase::Manifest if report.manifest.is_none() && report.outcome == IdentityOutcome::Collecting => Ok(()),
            Phase::Compute if report.manifest.as_ref() == Some(&self.manifest)
                && report.outcome == IdentityOutcome::Collecting => Ok(()),
            Phase::Install if report.manifest.as_ref() == Some(&self.manifest)
                && report.outcome == IdentityOutcome::Matched && self.probe.complete() => Ok(()),
            _ => Err(Error::WrongState.into()),
        }
    }

    fn withdraw(&self, host: &mut FileOversight, reason: Error) -> FileDecoderIdentityProgress {
        let withdrawal = host.identity_unavailable(host.revision(), self.challenge.evidence().basis());
        FileDecoderIdentityProgress { work: self.work(), event: FileDecoderIdentityEvent::Withdrawn { reason, withdrawal } }
    }
}

fn observe(host: &mut FileOversight, now: ElapsedTick) -> Result<(), JournalError> {
    let previous = host.inspect().control.ledger.elapsed.ok_or(Error::Incomplete)?;
    if now < previous { return Err(Error::Stale.into()); }
    if now > previous { host.observe_time(host.revision(), now)?; }
    Ok(())
}
