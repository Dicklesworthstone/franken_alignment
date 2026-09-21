//! Durable feed lease observations in original publication event 30.
//! A restrictive observation is acknowledged; failed persistence never is.
use super::*;
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::publication_gate::changes::freshness::{
    PublicationFreshnessPolicy, PublicationFreshnessStatus, PublicationHeartbeat,
};

#[derive(Clone, Copy)]
pub(in super::super::super) enum FreshnessEvent {
    Enable(PublicationFreshnessPolicy),
    SnapshotFallback,
    Unavailable(u64),
    Observed(PublicationHeartbeat, ElapsedTick),
}

impl FileOversight {
    /// Explicit bootstrap alternative to refusing a retained history gap. All
    /// current-state witnesses still undergo exact comparison at each boundary.
    pub fn enable_publication_snapshot_fallback(&mut self, revision: u64) -> Result<(), JournalError> {
        self.transact(revision, Event::PublicationWitness(WitnessEvent::Freshness(FreshnessEvent::SnapshotFallback)))?;
        Ok(())
    }
    pub fn publication_snapshot_fallback_enabled(&self) -> Result<bool, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.publication_snapshot_fallback_enabled()?)
    }

    /// The existing durable clock identity must match the producer's named
    /// elapsed domain. Enable before proposals; a new profile cannot widen it.
    pub fn enable_publication_change_freshness(&mut self, revision: u64, policy: PublicationFreshnessPolicy)
        -> Result<(), JournalError>
    {
        policy.check()?;
        if policy.clock_domain != self.profile.delivery.clock_domain { return Err(Error::Binding.into()); }
        self.transact(revision, Event::PublicationWitness(WitnessEvent::Freshness(FreshnessEvent::Enable(policy))))?;
        Ok(())
    }
    pub fn publication_change_freshness(&self) -> Result<PublicationFreshnessStatus, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.publication_change_freshness()?)
    }
    /// Commit loss before I/O. Keep the exact producer floors, incomplete tail,
    /// original observations, keys and accounting. This cannot clear the
    /// independent policy-source interruption latch or resume an actor.
    pub fn publication_changes_unavailable(&mut self, revision: u64, source: u64) -> Result<(), JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        self.machine.broker.publication_change_freshness()?;
        if self.machine.broker.publication_change_status()?.source != source { return Err(Error::Binding.into()); }
        self.fault = Some(JournalFailure { operation: JournalIo::Stage,
            kind: io::ErrorKind::Other, replacement_may_be_visible: false });
        self.persist_publication_freshness(FreshnessEvent::Unavailable(source))?;
        Ok(())
    }

    // Only the concrete reader can supply a positive durable observation. Its
    // withdrawal revision is retained and it quarantines BEFORE the clock call.
    pub(in super::super::super) fn finish_publication_heartbeat(&mut self, expected: u64,
        heartbeat: PublicationHeartbeat, now: ElapsedTick) -> Result<PublicationFreshnessStatus, JournalError>
    {
        if expected != self.revision() { return Err(Error::Stale.into()); }
        self.persist_publication_freshness(FreshnessEvent::Observed(heartbeat, now))?;
        self.publication_change_freshness()
    }

    fn persist_publication_freshness(&mut self, observation: FreshnessEvent) -> Result<(), JournalError> {
        // The caller already closed live admission. All following failures keep
        // it closed, including allocation/replay before entering Store::replace.
        let event = Event::PublicationWitness(WitnessEvent::Freshness(observation));
        if self.events.len() >= self.profile.delivery.limits.events { return Err(Error::Limit.into()); }
        let bytes = journal::encode_appended(&self.profile, self.store.identity(), &self.events, &event)?;
        let mut candidate = Machine::replay(&self.profile, &self.events)?;
        let result = candidate.apply(&event)?;
        self.persist_candidate(event, bytes, candidate, result)?;
        Ok(())
    }
}

pub(in super::super::super) fn write_heartbeat(w: &mut Writer, heartbeat: PublicationHeartbeat) -> Result<(), Error> {
    for value in [heartbeat.source, heartbeat.clock_domain, heartbeat.generation, heartbeat.through, heartbeat.produced_at.0] {
        w.u64(value)?;
    }
    Ok(())
}
pub(in super::super::super) fn read_heartbeat(r: &mut Reader<'_>) -> Result<PublicationHeartbeat, Error> {
    Ok(PublicationHeartbeat { source: r.u64()?, clock_domain: r.u64()?, generation: r.u64()?,
        through: r.u64()?, produced_at: ElapsedTick(r.u64()?) })
}
pub(super) fn write(w: &mut Writer, event: FreshnessEvent) -> Result<(), Error> {
    match event {
        FreshnessEvent::SnapshotFallback => w.u8(3)?,
        FreshnessEvent::Enable(policy) => { w.u8(0)?; w.u64(policy.clock_domain)?; w.u64(policy.max_age_ticks)?; }
        FreshnessEvent::Unavailable(source) => { w.u8(1)?; w.u64(source)?; }
        FreshnessEvent::Observed(heartbeat, now) => { w.u8(2)?; write_heartbeat(w, heartbeat)?; w.u64(now.0)?; }
    }
    Ok(())
}
pub(super) fn read(r: &mut Reader<'_>) -> Result<FreshnessEvent, Error> {
    Ok(match r.u8()? {
        0 => FreshnessEvent::Enable(PublicationFreshnessPolicy { clock_domain: r.u64()?, max_age_ticks: r.u64()? }),
        1 => FreshnessEvent::Unavailable(r.u64()?),
        2 => FreshnessEvent::Observed(read_heartbeat(r)?, ElapsedTick(r.u64()?)),
        3 => FreshnessEvent::SnapshotFallback,
        _ => return Err(Error::InvalidInput),
    })
}

#[cfg(test)]
mod encoding_tests {
    use super::*;

    #[test]
    fn snapshot_selection_has_a_distinct_bootstrap_tag_and_strict_framing() {
        let mut w = Writer::new(1);
        write(&mut w, FreshnessEvent::SnapshotFallback).unwrap();
        assert_eq!(w.finish(), vec![3]);
        assert!(WitnessEvent::Freshness(FreshnessEvent::SnapshotFallback).bootstrap());
        let mut r = Reader::new(&[3]);
        assert!(matches!(read(&mut r), Ok(FreshnessEvent::SnapshotFallback)));
        r.end().unwrap();
        let mut r = Reader::new(&[3, 0]);
        assert!(matches!(read(&mut r), Ok(FreshnessEvent::SnapshotFallback)));
        assert!(r.end().is_err());
        assert!(read(&mut Reader::new(&[])).is_err());
        assert!(matches!(read(&mut Reader::new(&[4])), Err(Error::InvalidInput)));
    }
}
