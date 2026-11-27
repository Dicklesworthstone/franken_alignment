//! Internal exact-Fence reduction after the original unbound observer is checked.
//! The public observer entry point prevents losing RAM-only source operations.
use super::{FileOversight, JournalError, TopologyCheckpoint, check_work, copy};
use crate::Error;
use crate::action::consequence::delivery::persistent::observed::{
    BaseEvent, Event, credibility::CredibilityEvent,
};

impl TopologyCheckpoint {
    /// Advance this independently retained checkpoint across exactly ONE
    /// acknowledged original recovery Fence, before subsequent startup work.
    /// The same healthy owner's profile, canonical identity and complete retained
    /// prefix must agree. The original graph and historical availability must
    /// match that cut; the native successor must retain the graph, withdraw its
    /// availability and preserve the retained control and authority floors.
    ///
    /// This permits a supervisor to retain honest recovery custody before a
    /// fallible source rebind. It does not require credential registration.
    /// Source work, time observations, reconciliation, credential changes, Stop,
    /// multiple fences and all other successors refuse. They cannot silently
    /// inherit older source expenditure. Repeating this call with the ORIGINAL
    /// checkpoint and unchanged healthy owner returns the same successor.
    ///
    /// Original documents, spent work and the last source operation are copied
    /// unchanged. Only acknowledged history, withdrawn availability and native
    /// floors advance. No file access, source check, replay, clock call, journal
    /// write, topology certification or authority is created. Independently
    /// retaining the successor remains the caller's obligation; keep the old
    /// checkpoint until that export has been durably acknowledged.
    // An unchanged journal alone cannot prove that an already-bound withdrawn
    // source has not consumed a later operation ID. Only the public observer
    // boundary may call this reduction for external consumers; binding consumes
    // that noncloneable observer before any source operation can occur.
    pub(super) fn after_recovery_fence(&self, host: &FileOversight) -> Result<Self, JournalError> {
        if host.storage_failure().is_some() {
            return Err(JournalError::Unavailable);
        }
        check_work(&self.expected, self.work, self.last_operation)?;
        let snapshot = host.inspect();
        if snapshot.control.sequence < self.control_sequence
            || snapshot.control.ledger.epoch < self.authority_epoch
        {
            return Err(Error::Stale.into());
        }
        self.history
            .check(&host.profile, host.store.identity(), &host.events)?;
        let after = usize::try_from(self.history.revision()).map_err(|_| Error::Limit)?;
        if !matches!(&host.events[after..], [Event::Core(BaseEvent::Fence)]) {
            return Err(Error::Stale.into());
        }
        let (graph, available) = super::terminal::topology_at(&host.events[..after])?;
        if graph != self.expected.graph() || available != self.available {
            return Err(Error::Binding.into());
        }
        let current = host.mediation_snapshot()?;
        if current.graph != *self.expected.graph() || current.available {
            return Err(Error::Binding.into());
        }
        Ok(Self {
            history: host.history_anchor()?,
            topology: copy(&self.topology)?,
            inventory: copy(&self.inventory)?,
            expected: self.expected.clone(),
            work: self.work,
            last_operation: self.last_operation,
            available: false,
            control_sequence: snapshot.control.sequence,
            authority_epoch: snapshot.control.ledger.epoch,
        })
    }

    /// Carry the same unbound source work through historical evaluation only.
    /// The separately held observer proves that no live source can consume
    /// RAM-only work. A complete retained cut must be followed by one recovery
    /// Fence, then only independent assessments and their clock observations.
    /// An original terminal Stop may end that interval; no later label, clock,
    /// source, numerical, policy or publication transition is accepted.
    pub(super) fn after_evaluated_recovery(
        &self,
        host: &FileOversight,
    ) -> Result<Self, JournalError> {
        if host.storage_failure().is_some() {
            return Err(JournalError::Unavailable);
        }
        check_work(&self.expected, self.work, self.last_operation)?;
        let snapshot = host.inspect();
        if snapshot.control.sequence < self.control_sequence
            || snapshot.control.ledger.epoch < self.authority_epoch
        {
            return Err(Error::Stale.into());
        }
        self.history
            .check(&host.profile, host.store.identity(), &host.events)?;
        let after = usize::try_from(self.history.revision()).map_err(|_| Error::Limit)?;
        let Some((Event::Core(BaseEvent::Fence), events)) = host.events[after..].split_first()
        else {
            return Err(Error::Stale.into());
        };
        let mut stopped = host.events[..after]
            .iter()
            .rev()
            .find_map(|event| match event {
                Event::Core(BaseEvent::Stop(request)) => Some(*request),
                _ => None,
            });
        let mut terminal_fence = false;
        for event in events {
            match event {
                Event::Core(BaseEvent::Time(_))
                | Event::Credibility(CredibilityEvent::Assess(_, _))
                    if stopped.is_none() => {}
                Event::Core(BaseEvent::Stop(request)) if stopped.is_none() => {
                    stopped = Some(*request);
                }
                Event::Core(BaseEvent::Fence) if stopped.is_some() && !terminal_fence => {
                    terminal_fence = true;
                }
                Event::Core(BaseEvent::StopProgress(_)) if stopped.is_some() => {}
                _ => return Err(Error::Stale.into()),
            }
        }
        if snapshot.stop.as_ref().map(|receipt| receipt.request()) != stopped {
            return Err(Error::Binding.into());
        }
        let (graph, available) = super::terminal::topology_at(&host.events[..after])?;
        if graph != self.expected.graph() || available != self.available {
            return Err(Error::Binding.into());
        }
        let current = host.mediation_snapshot()?;
        if current.graph != *self.expected.graph() || current.available {
            return Err(Error::Binding.into());
        }
        Ok(Self {
            history: host.history_anchor()?,
            topology: copy(&self.topology)?,
            inventory: copy(&self.inventory)?,
            expected: self.expected.clone(),
            work: self.work,
            last_operation: self.last_operation,
            available: false,
            control_sequence: snapshot.control.sequence,
            authority_epoch: snapshot.control.ledger.epoch,
        })
    }
}
