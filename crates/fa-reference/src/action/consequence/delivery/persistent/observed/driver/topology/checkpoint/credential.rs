//! Preserve source custody across acknowledged offline credential maintenance.
//! Only the original healthy owner can attest the successor; no input files,
//! source work, numerical replay, clock or new authority are created here.
use super::{FileOversight, JournalError, TopologyCheckpoint, check_work, copy};
use crate::action::consequence::delivery::persistent::observed::{BaseEvent, Event};
use crate::Error;

impl TopologyCheckpoint {
    /// Pair this independently retained source expenditure with the SAME healthy
    /// owner's acknowledged credential-maintenance cut. The exact original
    /// profile, canonical identity and retained prefix must still agree.
    ///
    /// Only original recovery Fence, CredentialRotate and CredentialRevoke
    /// events may follow the saved cut. Later source checks, topology changes,
    /// numerical work, time, reconciliation and terminal events all refuse:
    /// they cannot silently inherit an older source-work counter. Every accepted
    /// event was already reduced and acknowledged by this owner.
    ///
    /// This also captures a Fence-only cut before a lifecycle write, permitting
    /// callers to retain that honest prefix after a later ambiguous failure.
    /// A poisoned owner's RAM never certifies a candidate transition. The
    /// returned checkpoint preserves original input bytes, spent work and last
    /// source operation; availability and floors come from the actual native
    /// state. A recovery Fence never becomes fresh topology certification.
    ///
    /// No filesystem read/write, clock call, source check or lifecycle mutation
    /// occurs. Independent durable export remains the caller's obligation.
    pub fn after_credential_lifecycle(&self, host: &FileOversight)
        -> Result<Self, JournalError>
    {
        if host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
        check_work(&self.expected, self.work, self.last_operation)?;
        if host.credential_status()?.is_none() { return Err(Error::Missing.into()); }
        let snapshot = host.inspect();
        if snapshot.control.sequence < self.control_sequence
            || snapshot.control.ledger.epoch < self.authority_epoch {
            return Err(Error::Stale.into());
        }
        self.history.check(&host.profile, host.store.identity(), &host.events)?;
        let after = usize::try_from(self.history.revision()).map_err(|_| Error::Limit)?;
        let (graph, available) = super::terminal::topology_at(&host.events[..after])?;
        if graph != self.expected.graph() || available != self.available {
            return Err(Error::Binding.into());
        }
        let mut available = available;
        for event in &host.events[after..] {
            match event {
                Event::Core(BaseEvent::Fence) => available = false,
                Event::CredentialRotate(_) | Event::CredentialRevoke(_) => {}
                _ => return Err(Error::Stale.into()),
            }
        }
        let current = host.mediation_snapshot()?;
        if current.graph != *self.expected.graph() || current.available != available {
            return Err(Error::Binding.into());
        }
        Ok(Self {
            history: host.history_anchor()?,
            topology: copy(&self.topology)?,
            inventory: copy(&self.inventory)?,
            expected: self.expected.clone(),
            work: self.work,
            last_operation: self.last_operation,
            available,
            control_sequence: snapshot.control.sequence,
            authority_epoch: snapshot.control.ledger.epoch,
        })
    }
}

