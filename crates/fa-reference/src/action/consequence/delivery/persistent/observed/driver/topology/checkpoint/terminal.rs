//! Carry retained source work across only acknowledged native terminal events.
//! The sealed receipt supplies the cut; arbitrary snapshot/anchor pairs do not.
use super::{TopologyCheckpoint, check_work, copy};
use crate::action::consequence::delivery::{StopProgress,
    persistent::{FileDeliverySnapshot, JournalError,
        observed::{BaseEvent, Event, FileOversightProfile,
            guarded::anchored::FileHistoryAnchor,
            mediation::MediationEvent,
            shutdown::recovery::{FileAdmissionStoppedHistory, FileStoppedHistory}}}};
use crate::action::consequence::mediation::AuthorityGraph;
use crate::Error;
use std::path::Path;

impl TopologyCheckpoint {
    // Admission for the dedicated terminal-only learned cursor. It retains the
    // original locked image for full semantic replay; this bounded scan grants
    // no owner and does not acknowledge a candidate checkpoint. Only terminal
    // history can explain a successor without spending additional source work.
    pub(in super::super::super::super) fn terminal_successor_available(&self,
        profile: &FileOversightProfile, identity: &Path, events: &[Event], operation: u64)
        -> Result<bool, Error>
    {
        if operation == 0 { return Err(Error::InvalidInput); }
        check_work(&self.expected, self.work, self.last_operation)?;
        self.history.check(profile, identity, events)?;
        let after = usize::try_from(self.history.revision()).map_err(|_| Error::Limit)?;
        let (graph, available) = topology_at(&events[..after])?;
        if graph != self.expected.graph() || available != self.available {
            return Err(Error::Binding);
        }
        let mut stopped = events[..after].iter().rev().find_map(|event| match event {
            Event::Core(BaseEvent::Stop(request)) => Some(*request), _ => None,
        });
        if stopped.is_some_and(|request| request.operation != operation) {
            return Err(Error::Duplicate);
        }
        for event in &events[after..] {
            match event {
                Event::Core(BaseEvent::Stop(request)) if request.operation == operation => {
                    if stopped.is_some_and(|original| original != *request) {
                        return Err(Error::Binding);
                    }
                    stopped = Some(*request);
                }
                Event::Core(BaseEvent::Fence | BaseEvent::StopProgress(_)) if stopped.is_some() => {}
                _ => return Err(Error::Stale),
            }
        }
        // An exact source cut may start its first native stop. A stale cut must
        // already contain the selected Stop before any later terminal event;
        // an ordinary recovery Fence cannot silently become terminal custody.
        let (graph, available) = topology_at(events)?;
        if graph != self.expected.graph() { return Err(Error::Binding); }
        Ok(available)
    }

    /// Advance independently retained source custody to the SAME acknowledged
    /// admission-stop cut. Original documents, spent work and last source
    /// operation remain unchanged; only the captured history and native floors
    /// advance. A native fence withdraws topology availability; an exact
    /// Stop-only retry preserves its original historical availability.
    ///
    /// `profile` and `identity` are the independently selected original bootstrap
    /// and canonical store path. They bind the original decoder, not live files.
    /// Exact retained-prefix comparison and a terminal-only suffix are required:
    /// later source checks, ordinary work, changed graphs and divergent histories
    /// cannot silently inherit older expenditure. A native Stop without topology
    /// withdrawal is never relabeled as unavailable or fresh certification.
    ///
    /// This performs bounded parsing/copying of the acknowledged anchor only.
    /// No filesystem access, clock, semantic replay, source check, journal write
    /// or live authority is created. Independent durable export remains a separate
    /// obligation; export refusal cannot undo the already acknowledged stop.
    pub fn after_admission_stop(&self, profile: &FileOversightProfile, identity: &Path,
        history: &FileAdmissionStoppedHistory) -> Result<Self, JournalError>
    {
        self.after_terminal(profile, identity, history.snapshot(), history.progress(),
            history.history_anchor())
    }

    /// Carry the SAME original source work across an acknowledged combined stop,
    /// later drain, or exact completed retry. Only Stop/Fence/StopProgress may
    /// follow the retained cut. The returned anchor is the receipt's captured
    /// revision, even if disk changes later; this never reopens the authority
    /// directory or claims that charged/unknown effects have drained.
    ///
    /// The original checkpoint remains necessary until the successor is durably
    /// retained. Trusted custody and protection against rolling back BOTH records
    /// are unchanged. No current topology observation or permission is issued.
    pub fn after_stopped(&self, profile: &FileOversightProfile, identity: &Path,
        history: &FileStoppedHistory) -> Result<Self, JournalError>
    {
        self.after_terminal(profile, identity, history.stopped().snapshot(),
            history.stopped().progress(), history.history_anchor())
    }

    fn after_terminal(&self, profile: &FileOversightProfile, identity: &Path,
        snapshot: &FileDeliverySnapshot, progress: &StopProgress, history: &FileHistoryAnchor)
        -> Result<Self, JournalError>
    {
        check_work(&self.expected, self.work, self.last_operation)?;
        if snapshot.revision != history.revision()
            || snapshot.stop.as_ref() != Some(&progress.receipt) {
            return Err(Error::Binding.into());
        }
        if snapshot.control.sequence < self.control_sequence
            || snapshot.control.ledger.epoch < self.authority_epoch {
            return Err(Error::Stale.into());
        }
        let events = history.decoded_events(profile, identity)?;
        self.history.check(profile, identity, &events)?;
        let after = usize::try_from(self.history.revision()).map_err(|_| Error::Limit)?;
        let (graph, available) = topology_at(&events[..after])?;
        if graph != self.expected.graph() || available != self.available {
            return Err(Error::Binding.into());
        }
        let request = progress.receipt.request();
        for event in &events[after..] {
            match event {
                Event::Core(BaseEvent::Stop(original)) if *original == request => {}
                Event::Core(BaseEvent::Fence | BaseEvent::StopProgress(_)) => {}
                _ => return Err(Error::Stale.into()),
            }
        }
        // Stop alone closes admission but does not withdraw the retained graph.
        // Native Fence does. Check the acknowledged inputs so a Stop-only retry
        // cannot manufacture unavailable topology or claim fresh certification.
        let (graph, available) = topology_at(&events)?;
        if graph != self.expected.graph() { return Err(Error::Binding.into()); }
        Ok(Self { history: history.clone(), topology: copy(&self.topology)?,
            inventory: copy(&self.inventory)?, expected: self.expected.clone(),
            work: self.work, last_operation: self.last_operation, available,
            control_sequence: snapshot.control.sequence,
            authority_epoch: snapshot.control.ledger.epoch })
    }
}

// The sealed native terminal result already completed semantic replay. Inspect
// only the original declarations which select/withdraw the graph; no graph cut
// is certified and no journal event is executed again. Fence withdraws topology
// through Machine::recover_mediation; a check never restores an unavailable graph.
fn topology_at(events: &[Event]) -> Result<(&AuthorityGraph, bool), Error> {
    let mut graph = None;
    let mut available = false;
    for event in events {
        match event {
            Event::Mediation(MediationEvent::Enable(original)) => {
                graph = Some(original); available = true;
            }
            Event::Mediation(MediationEvent::Update(update)) => {
                if let Some(next) = &update.next { graph = Some(next); }
                available = update.next.is_some();
            }
            Event::Core(BaseEvent::Fence) => available = false,
            _ => {}
        }
    }
    Ok((graph.ok_or(Error::Missing)?, available))
}
