//! Persist change notifications in the same owner, journal and recovery cut.
use super::*;
use crate::action::consequence::delivery::publication_gate::changes::{
    PublicationChange, PublicationChangePolicy, PublicationChangeReport, PublicationChangeStatus,
};
use crate::witness::refinement::index::routing::{RoutingBudget, WitnessChange};
use crate::witness::DomainProjection;
use crate::product_frontier::ProjectionKey;

impl FileOversight {
    pub fn enable_publication_changes(&mut self, revision: u64, policy: PublicationChangePolicy) -> Result<(), JournalError> {
        self.transact(revision, Event::PublicationWitness(WitnessEvent::ChangeProfile(policy)))?;
        Ok(())
    }
    pub fn publication_change_status(&self) -> Result<PublicationChangeStatus, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.publication_change_status()?)
    }
    /// Last acknowledged notification only, never a permission to reuse inputs.
    pub fn publication_change_report(&self) -> Result<Option<Rc<PublicationChangeReport>>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.publication_change_report()?)
    }
    /// A known gap is an acknowledged restrictive outcome, not a rolled-back
    /// candidate error. Stale/foreign writers refuse before mutation. Once an
    /// accepted notice starts, ANY encoding/capacity/replay/storage failure closes
    /// the live owner; an old quiet observation cannot remain eligible.
    pub fn record_publication_change(&mut self, revision: u64, notice: PublicationChange)
        -> Result<Rc<PublicationChangeReport>, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        self.machine.broker.preflight_publication_change(notice)?;
        // Unlike a permitting input replacement, this operation remains useful
        // during source interruption. It neither clears that latch nor installs
        // an input/key; the original source checks still block any future effect.
        self.fault = Some(JournalFailure { operation: JournalIo::Stage,
            kind: io::ErrorKind::Other, replacement_may_be_visible: false });
        let event = Event::PublicationWitness(WitnessEvent::Change(notice));
        if self.events.len() >= self.profile.delivery.limits.events { return Err(Error::Limit.into()); }
        let bytes = journal::encode_appended(&self.profile, self.store.identity(), &self.events, &event)?;
        let mut candidate = Machine::replay(&self.profile, &self.events)?;
        let result = candidate.apply(&event)?;
        let report = candidate.broker.publication_change_report()?.ok_or(Error::Incomplete)?;
        self.persist_candidate(event, bytes, candidate, result)?;
        Ok(report)
    }
}

pub(super) fn write_policy(w: &mut Writer, p: PublicationChangePolicy) -> Result<(), Error> {
    w.u64(p.source)?; w.u64(p.after)?; w.u64(p.lookup.steps)?; w.u64(p.lookup.bytes)
}
pub(super) fn read_policy(r: &mut Reader<'_>) -> Result<PublicationChangePolicy, Error> {
    Ok(PublicationChangePolicy { source: r.u64()?, after: r.u64()?,
        lookup: RoutingBudget { steps: r.u64()?, bytes: r.u64()? } })
}
fn write_domain(w: &mut Writer, d: DomainProjection) -> Result<(), Error> {
    let p = d.projection();
    for value in [d.domain_id(), d.domain_epoch(), p.source, p.branch, p.projection, p.source_epoch] { w.u64(value)?; }
    Ok(())
}
fn read_domain(r: &mut Reader<'_>) -> Result<DomainProjection, Error> {
    let id = r.u64()?; let epoch = r.u64()?;
    Ok(DomainProjection::new(id, epoch, ProjectionKey { source: r.u64()?, branch: r.u64()?,
        projection: r.u64()?, source_epoch: r.u64()? }))
}
pub(super) fn write_change(w: &mut Writer, notice: PublicationChange) -> Result<(), Error> {
    w.u64(notice.source)?; w.u64(notice.sequence)?;
    match notice.change {
        WitnessChange::Key { domain, key } => { w.u8(0)?; write_domain(w, domain)?; w.u64(key)?; }
        WitnessChange::Range { domain, start, end } => { w.u8(1)?; write_domain(w, domain)?; w.u64(start)?; w.u64(end)?; }
        WitnessChange::Domain { domain } => { w.u8(2)?; write_domain(w, domain)?; }
        WitnessChange::All => w.u8(3)?,
    }
    Ok(())
}
pub(super) fn read_change(r: &mut Reader<'_>) -> Result<PublicationChange, Error> {
    let source = r.u64()?; let sequence = r.u64()?;
    let change = match r.u8()? {
        0 => WitnessChange::Key { domain: read_domain(r)?, key: r.u64()? },
        1 => WitnessChange::Range { domain: read_domain(r)?, start: r.u64()?, end: r.u64()? },
        2 => WitnessChange::Domain { domain: read_domain(r)? },
        3 => WitnessChange::All,
        _ => return Err(Error::InvalidInput),
    };
    // A syntactically decoded malformed range is retained as a conservative
    // all-slot withdrawal; do not discard known negative evidence on replay.
    Ok(PublicationChange { source, sequence, change })
}
