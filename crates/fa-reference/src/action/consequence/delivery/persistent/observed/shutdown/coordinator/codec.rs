//! Bounded coordinator bookkeeping, never imported native decisions or rights.
use super::*;
use crate::action::consequence::delivery::persistent::codec::shared::{Reader, Writer};
use std::os::unix::ffi::OsStrExt;

const DOMAIN: &[u8; 8] = b"FASHCOO\x01";
const RECOVERY_DOMAIN: &[u8; 8] = b"FASHCOO\x02";
const MAX_PATH_BYTES: usize = 4096;

pub(super) struct Restored {
    pub(super) campaign: FileShutdownCampaign,
    pub(super) revision: u64,
    pub(super) visits: Vec<ShutdownVisit>,
}

pub(super) fn encode(path: &Path, plan: &[u8], max_bytes: usize, revision: u64,
    visits: &[ShutdownVisit], campaign: &FileShutdownCampaign) -> Result<Vec<u8>, Error>
{
    if path.as_os_str().as_bytes().len() > MAX_PATH_BYTES { return Err(Error::Limit); }
    let mut w = Writer::new(max_bytes);
    // Existing histories retain byte-identical version 1. A recovery intent
    // explicitly requires version 2; an old reader must refuse the new operation.
    let domain = if visits.iter().any(|visit| matches!(visit.kind, ShutdownVisitKind::RecoverStopped { .. })) {
        RECOVERY_DOMAIN
    } else { DOMAIN };
    w.raw(domain)?; w.blob(path.as_os_str().as_bytes())?;
    w.count(max_bytes)?; w.blob(plan)?; w.u64(revision)?; w.count(visits.len())?;
    for visit in visits {
        w.u64(visit.domain)?;
        match visit.kind {
            ShutdownVisitKind::Advance { at } => { w.u8(0)?; w.u64(at.0)?; }
            ShutdownVisitKind::InspectCanonical => w.u8(1)?,
            ShutdownVisitKind::Unavailable => w.u8(2)?,
            ShutdownVisitKind::RecoverStopped { at } => { w.u8(3)?; w.u64(at.0)?; }
        }
        match &visit.result {
            ShutdownVisitResult::Entered => w.u8(0)?,
            ShutdownVisitResult::Observed { domain_revision } => { w.u8(1)?; w.u64(*domain_revision)?; }
            ShutdownVisitResult::Refused { diagnostic } => {
                if diagnostic.len() > MAX_REFUSAL_BYTES { return Err(Error::Limit); }
                w.u8(2)?; w.blob(diagnostic.as_bytes())?;
            }
        }
    }
    w.count(campaign.slots.len())?;
    for (domain, slot) in campaign.plan.domains.iter().zip(&campaign.slots) {
        w.u64(domain.id)?; w.count(slot.revision)?; w.blob(&slot.head)?;
    }
    Ok(w.finish())
}

pub(super) fn decode(path: &Path, plan: &FileShutdownPlan, plan_bytes: &[u8], max_bytes: usize,
    minimum_revision: u64, bytes: &[u8]) -> Result<Restored, Error>
{
    if bytes.len() > max_bytes { return Err(Error::Limit); }
    let mut r = Reader::new(bytes);
    let version = match r.take(DOMAIN.len())? {
        bytes if bytes == DOMAIN => 1,
        bytes if bytes == RECOVERY_DOMAIN => 2,
        _ => return Err(Error::Binding),
    };
    if r.blob(MAX_PATH_BYTES)? != path.as_os_str().as_bytes()
        || r.count(MAX_JOURNAL_BYTES)? != max_bytes
        || r.blob(MAX_SHUTDOWN_PLAN_BYTES)? != plan_bytes { return Err(Error::Binding); }
    let revision = r.u64()?;
    if revision < minimum_revision { return Err(Error::Stale); }
    let count = r.count(plan.max_attempts)?;
    let mut visits = Vec::new(); visits.try_reserve_exact(count).map_err(|_| Error::Limit)?;
    let mut completed = 0_u64;
    for _ in 0..count {
        let domain = r.u64()?;
        if plan.domains.binary_search_by_key(&domain, |entry| entry.id).is_err() { return Err(Error::Binding); }
        let kind = match r.u8()? {
            0 => ShutdownVisitKind::Advance { at: ElapsedTick(r.u64()?) },
            1 => ShutdownVisitKind::InspectCanonical,
            2 => ShutdownVisitKind::Unavailable,
            3 if version == 2 => ShutdownVisitKind::RecoverStopped { at: ElapsedTick(r.u64()?) },
            _ => return Err(Error::InvalidInput),
        };
        let result = match r.u8()? {
            0 => ShutdownVisitResult::Entered,
            1 => {
                if kind == ShutdownVisitKind::Unavailable { return Err(Error::Binding); }
                ShutdownVisitResult::Observed { domain_revision: r.u64()? }
            }
            2 => {
                let diagnostic = std::str::from_utf8(r.blob(MAX_REFUSAL_BYTES)?).map_err(|_| Error::InvalidInput)?;
                if diagnostic.is_empty() { return Err(Error::InvalidInput); }
                ShutdownVisitResult::Refused { diagnostic: diagnostic.to_owned() }
            }
            _ => return Err(Error::InvalidInput),
        };
        if result != ShutdownVisitResult::Entered { completed += 1; }
        visits.push(ShutdownVisit { domain, kind, result });
    }
    if revision != count as u64 + completed { return Err(Error::Binding); }
    if r.count(MAX_SHUTDOWN_DOMAINS)? != plan.domains.len() { return Err(Error::Binding); }
    let mut campaign = plan.start(); let mut retained = 0_usize;
    for (domain, slot) in plan.domains.iter().zip(&mut campaign.slots) {
        if r.u64()? != domain.id { return Err(Error::Binding); }
        let revision = r.count(domain.profile.delivery.limits.events)?;
        let head = r.blob(domain.profile.delivery.limits.bytes)?;
        retained = retained.checked_add(head.len()).ok_or(Error::Limit)?;
        if retained > plan.max_head_bytes { return Err(Error::Limit); }
        let events = journal::decode(&domain.profile, &domain.directory, head)?;
        if events.len() != revision || revision < domain.revision { return Err(Error::Binding); }
        if journal::encode(&domain.profile, &domain.directory, &events[..domain.revision])?.as_slice() != domain.anchor.as_ref() {
            return Err(Error::Binding);
        }
        let mut observed = domain.revision as u64;
        for visit in visits.iter().filter(|visit| visit.domain == domain.id) {
            if let ShutdownVisitResult::Observed { domain_revision } = &visit.result {
                if *domain_revision < observed || *domain_revision > revision as u64 { return Err(Error::Binding); }
                observed = *domain_revision;
            }
        }
        // A head advances only with an acknowledged successful visit. Pending
        // and failed visits cannot assert an unobserved newer comparison frontier.
        if observed != revision as u64 { return Err(Error::Binding); }
        slot.head = Rc::from(head); slot.revision = revision;
    }
    campaign.head_bytes = retained;
    r.end()?;
    if encode(path, plan_bytes, max_bytes, revision, &visits, &campaign)?.as_slice() != bytes { return Err(Error::Binding); }
    // No archived observation is installed. Full native semantic replay occurs
    // only on a fresh canonical inspection (or in the supplied original owner).
    Ok(Restored { campaign, revision, visits })
}
