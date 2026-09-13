//! Fresh registered-source capture into the ORIGINAL one-use actor intake slot.
use super::{FileEvidenceReport, FileOversight, FileSourceError, FileSupervisedDriver,
    JournalError, durable_capture, evidence_error, observe};
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::persistent::requests::actor::FileActorSupervisor;
use crate::action::consequence::oversight::evidence_source::{EvidenceError, EvidenceFile, EvidenceIdentity};
use crate::Error;

impl FileActorSupervisor<FileOversight> {
    /// Publish one intake snapshot only after an acknowledged registered-source
    /// read and a fresh post-read lease check. No proposal, request key, review,
    /// resource reservation or human approval is supplied by this operation.
    ///
    /// The previous slot is withdrawn BEFORE any preflight, clock or file I/O.
    /// A failed source transaction retains its complete native diagnostics; an
    /// acknowledged observation does not imply successful slot preparation.
    /// Subsequent host mutation still clears the slot through host_mut(). The
    /// host must observe time at later scheduling boundaries; this is no watchdog.
    ///
    /// ```compile_fail,E0308
    /// use fa_reference::action::consequence::delivery::persistent::FilePermit;
    /// use fa_reference::action::consequence::delivery::persistent::observed::driver::evidence::FileEvidenceReport;
    /// use fa_reference::action::consequence::oversight::evidence_source::EvidenceIdentity;
    /// fn grant(report: FileEvidenceReport<EvidenceIdentity>) -> FilePermit { report }
    /// ```
    pub fn prepare_file_intake<S, F>(&mut self, source: &mut S, mut clock: F)
        -> FileEvidenceReport<EvidenceIdentity>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let mut observations = Vec::with_capacity(1);
        let mut source_updates = Vec::with_capacity(1);
        let result = (|| {
            // host_mut withdraws the old slot before returning this exclusive
            // borrow. Reentrant port/supervisor calls during I/O cannot use it.
            let mut host = self.host_mut()?;
            if host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
            if !host.file_source_required() || host.inspect().control.suspended
                || host.inspect().stop.is_some() { return Err(Error::WrongState.into()); }
            let captured = durable_capture(&mut host, source, &mut clock);
            source_updates.push(captured.as_ref().map(|value| value.identity()).map_err(Clone::clone));
            let captured = match captured {
                Ok(value) => { observations.push(Ok(value.identity())); value }
                Err(FileSourceError::Refused(error)) => {
                    observations.push(Err(EvidenceError::Data(error)));
                    return Err(error.into());
                }
                Err(FileSourceError::Read { error, withdrawal }) => {
                    observations.push(Err(error));
                    return Err(withdrawal.unwrap_or_else(|| evidence_error(error).into()));
                }
                Err(FileSourceError::Journal(error)) => return Err(error),
            };
            // Read/parse/persistence latency consumes the conservative read-start
            // lease. Neither captured bytes nor a saved tick can extend it.
            observe(&mut host, clock())?;
            let cut = host.machine.broker.capture_policy_state()?;
            if cut.snapshot() != captured.snapshot() { return Err(Error::Binding.into()); }
            let revision = host.revision();
            drop(host);
            self.set_snapshot(revision, Some(captured.snapshot().clone()))?;
            Ok(captured.identity())
        })();
        FileEvidenceReport { observations, source_updates, result }
    }
}

impl FileSupervisedDriver {
    /// Same original gateway operation, with existing helper maintenance even
    /// on failure. No second intake queue or source owner is introduced.
    pub fn prepare_file_intake<S, F>(&mut self, source: &mut S, clock: F)
        -> FileEvidenceReport<EvidenceIdentity>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let report = self.supervisor.prepare_file_intake(source, clock);
        self.reap_helpers();
        report
    }
}
