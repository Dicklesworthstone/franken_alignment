//! First-publication orchestration over the ORIGINAL durable guard.
//! Observation failure is input to sealing, not permission to skip the guard.
use super::{DriverEvidence, FileDriverEvent, FileOversight, Job, JournalError, Phase};
use crate::action::consequence::oversight::CommitteeContract;
use crate::action::{ElapsedTick, FrozenAction};
use crate::{Error, Snapshot};

pub(super) fn publish_job<F, P>(host: &mut FileOversight, job: &mut Job,
    clock: &mut F, provider: &mut P) -> FileDriverEvent
where F: FnMut() -> ElapsedTick,
    P: FnMut(&FrozenAction, &CommitteeContract) -> Result<DriverEvidence, Error>,
{
    let request = job.request;
    // Retire the send path BEFORE invoking external provider code. Even a
    // caller-caught unwind may only be followed by original reconciliation.
    job.phase = Phase::Reconcile;
    if !host.publication_guard_required() {
        let revision = host.revision();
        return match host.publish(revision, job.attempt) {
            Ok(outcome) => FileDriverEvent::Published { request, outcome },
            Err(error) => FileDriverEvent::PublicationUnknown { request, error },
        };
    }
    let result = (|| {
        // This internal query does not grant authority. The guarded transaction
        // checks the same endpoint again, and is the only publication operation.
        let needs_evidence = host.machine.publication_needs_evidence(job.attempt)?;
        let (inputs, snapshot, source_failure) = if needs_evidence {
            let captured = provider(&job.action, &host.profile.committee).and_then(|evidence| {
                if !evidence.snapshot.complete { return Err(Error::Incomplete); }
                evidence.inputs.as_ref().ok_or(Error::Incomplete)?
                    .validate_for(&job.action, &host.profile.committee)?;
                Ok(evidence)
            });
            match captured {
                Ok(evidence) => (evidence.inputs, evidence.snapshot, None),
                Err(error) => (None, Snapshot::default(), Some(error)),
            }
        } else {
            // Resolved, expired or unsendable obligations need no provider.
            // Never replace original endpoint evidence with a source assertion.
            (None, Snapshot::default(), None)
        };
        // Do not use sample(): pre-dispatch eligibility withdrawal is not this
        // operation. Current time and publication/sealing commit together here.
        let now = clock();
        let revision = host.revision();
        let publication = host.publish_checked(revision, job.attempt, inputs.as_ref(), snapshot, now)?;
        Ok::<_, JournalError>((publication, source_failure))
    })();
    match result {
        Ok((publication, source_failure)) => FileDriverEvent::PublicationChecked { request, publication, source_failure },
        Err(error) => FileDriverEvent::PublicationUnknown { request, error },
    }
}
