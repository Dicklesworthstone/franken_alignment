//! One acknowledged catch-up cut, using ONLY original notice/heartbeat events.
use super::{PublicationFeedBatch, PublicationFeedFile, MAX_FEED_RECORDS};
use super::super::{FileCaptureError, FileOversight, JournalError, JournalFailure, JournalIo};
use super::super::super::super::super::{Event, Machine, journal};
use super::super::super::super::witness_gate::{WitnessEvent, freshness::FreshnessEvent};
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::publication_gate::changes::{PublicationChangeReport, PublicationChangeStatus};
use crate::action::consequence::delivery::publication_gate::changes::freshness::{PublicationFreshnessStatus, PublicationHeartbeat};
use crate::Error;
use std::io;
use std::rc::Rc;

/// Acknowledged observations, not a permission. An empty changes vector can mean
/// a replayed window OR an uncovered gap: inspect status and freshness. Reports
/// never escape a failed final replacement or turn a read identity into a permit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicationFeedReport {
    pub heartbeat: PublicationHeartbeat,
    pub before: u64,
    pub changes: Vec<Rc<PublicationChangeReport>>,
    pub status: PublicationChangeStatus,
    pub freshness: PublicationFreshnessStatus,
}
impl FileOversight {
    /// Commit feed unavailability BEFORE concrete I/O. Decode the entire bounded
    /// window, sample trusted time, validate overlap, and atomically install its
    /// unseen contiguous suffix plus heartbeat in the ORIGINAL canonical journal.
    /// A too-new window records its head but cannot synthesize its missing prefix.
    ///
    /// Read/packet failure is an inner error with withdrawal retained. After a
    /// successful read, clock unwind, conflicting history, capacity, replay or I/O
    /// failure quarantines the owner. Recovery fences old keys; never infer an
    /// effect outcome or a refund from this API's failure.
    pub fn refresh_publication_feed<F>(&mut self, revision: u64, source: &PublicationFeedFile, mut clock: F)
        -> Result<Result<PublicationFeedReport, FileCaptureError>, JournalError>
    where F: FnMut() -> ElapsedTick {
        self.publication_changes_unavailable(revision, source.source())?;
        let batch = match source.read_batch() { Ok(batch) => batch, Err(error) => return Ok(Err(error)) };
        self.fault = Some(JournalFailure { operation: JournalIo::Stage,
            kind: io::ErrorKind::Other, replacement_may_be_visible: false });
        let now = clock();
        self.install_publication_feed(batch, now).map(Ok)
    }

    fn install_publication_feed(&mut self, batch: PublicationFeedBatch, now: ElapsedTick)
        -> Result<PublicationFeedReport, JournalError>
    {
        let before = self.machine.broker.publication_change_status()?.through;
        self.check_feed_overlap(&batch, before)?;
        // A packet's retained window is not proof of earlier omitted records.
        let pending = batch.records.iter().filter(|record| batch.after <= before && record.sequence > before);
        let added = pending.clone().count().checked_add(1).ok_or(Error::Overflow)?;
        let count = self.events.len().checked_add(added).ok_or(Error::Overflow)?;
        if count > self.profile.delivery.limits.events { return Err(Error::Limit.into()); }
        let mut history = Vec::new();
        history.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        history.extend(self.events.iter().cloned());
        for record in pending { history.push(Event::PublicationWitness(WitnessEvent::Change(*record))); }
        history.push(Event::PublicationWitness(WitnessEvent::Freshness(FreshnessEvent::Observed(batch.heartbeat, now))));
        // The original encoder checks EVERY prefix, including recovery reserves,
        // before candidate execution. No special batch exemption or new wire tag.
        let bytes = journal::encode(&self.profile, self.store.identity(), &history)?;
        let mut candidate = Machine::replay(&self.profile, &self.events)?;
        let mut changes = Vec::new();
        changes.try_reserve_exact(added - 1).map_err(|_| Error::Limit)?;
        for event in &history[self.events.len()..] {
            candidate.apply(event)?;
            if matches!(event, Event::PublicationWitness(WitnessEvent::Change(_))) {
                changes.push(candidate.broker.publication_change_report()?.ok_or(Error::Incomplete)?);
            }
        }
        let report = PublicationFeedReport { heartbeat: batch.heartbeat, before, changes,
            status: candidate.broker.publication_change_status()?,
            freshness: candidate.broker.publication_change_freshness()? };
        self.fault = Some(JournalFailure { operation: JournalIo::Stage,
            kind: io::ErrorKind::Other, replacement_may_be_visible: true });
        if let Err(error) = self.store.replace(&bytes) {
            if let JournalError::Io(failure) = &error { self.fault = Some(failure.clone()); }
            return Err(error);
        }
        for event in &history[self.events.len()..] { self.source_operation_committed(event); }
        self.events = history;
        self.machine = candidate;
        self.fault = None;
        Ok(report)
    }

    fn check_feed_overlap(&self, batch: &PublicationFeedBatch, through: u64) -> Result<(), Error> {
        let bootstrap = self.events.iter().find_map(|event| match event {
            Event::PublicationWitness(WitnessEvent::ChangeProfile(policy)) => Some(policy.after), _ => None,
        }).ok_or(Error::Incomplete)?;
        let mut seen = [false; MAX_FEED_RECORDS];
        for event in &self.events {
            let Event::PublicationWitness(WitnessEvent::Change(prior)) = event else { continue; };
            if prior.source != batch.heartbeat.source || prior.sequence <= batch.after
                || prior.sequence > batch.heartbeat.through { continue; }
            let position = usize::try_from(prior.sequence - batch.after - 1).map_err(|_| Error::Limit)?;
            // Include earlier out-of-order notices too. Same sequence different
            // contents is not a retry, even if both would conservatively withdraw.
            if batch.records.get(position) != Some(prior) { return Err(Error::Binding); }
            seen[position] = true;
        }
        for (position, record) in batch.records.iter().enumerate() {
            if record.sequence > bootstrap && record.sequence <= through && !seen[position] {
                return Err(Error::Incomplete);
            }
        }
        Ok(())
    }
}
