//! Canonical bounded inputs to the ORIGINAL delivery reducers, not saved rights
//! counters or caller-asserted terminal outcomes. This format is not authenticated.
use super::*;
use crate::action::{Purpose, VERSION, MAX_ATTEMPTS, MAX_PAYLOAD_BYTES};
use crate::action::consequence::gate::containment::RestartGrade;
use crate::action::consequence::gate::containment::session::policy::Predicate;
use crate::reducer::MAX_IDENTIFIER_BYTES;
use crate::round::{MAX_FIELD_LEN, MAX_MEMBERS};
use std::os::unix::ffi::OsStrExt;

const DOMAIN: &[u8; 8] = b"FADLGR\0\x01";
const MAX_BOOT_BYTES: usize = 2 * 1024 * 1024;
const MAX_PATH_BYTES: usize = 4096;
const MAX_BALLOT_BYTES: usize = 256 * 1024;

pub(super) fn validate_profile(p: &FileDeliveryProfile) -> Result<(), Error> {
    p.limits.check()?;
    if p.clock_domain == 0 { return Err(Error::InvalidInput); }
    if p.narrowed_targets.len() > MAX_ATTEMPTS || p.initial_payload.len() > MAX_PAYLOAD_BYTES
        || p.congress.members.len() > MAX_MEMBERS
        || p.congress.members.iter().any(|(name, member)| name.len() > MAX_IDENTIFIER_BYTES
            || member.cohort.len() > MAX_IDENTIFIER_BYTES) { return Err(Error::Limit); }
    Ok(())
}

fn profile_bytes(p: &FileDeliveryProfile) -> Result<Vec<u8>, Error> {
    validate_profile(p)?;
    let mut w = Writer::new(MAX_BOOT_BYTES);
    w.scope(p.scope)?;
    for value in [p.total, p.max_attempts as u64, p.suspend_at_incident, p.retention_ticks,
        p.max_deliveries as u64, p.clock_domain, p.limits.events as u64, p.limits.bytes as u64] { w.u64(value)?; }
    w.target(p.target)?; w.blob(&p.initial_payload)?;
    let actor = &p.actor; let a = actor.profile();
    for value in [a.id, a.generation, a.host_generation, a.model_generation,
        a.tokenizer_generation, a.state_schema_generation, actor.next_position()] { w.u64(value)?; }
    w.u8(match a.grade { RestartGrade::AuditOnly => 0, RestartGrade::FunctionalRestart => 1, RestartGrade::ExactRestart => 2 })?;
    w.count(actor.tokens().len())?;
    for token in actor.tokens() { w.u32(*token)?; }
    w.blob(actor.cache())?; w.blob(actor.sampler())?;
    w.u64(p.policy.generation())?; w.count(p.policy.nodes().len())?;
    for node in p.policy.nodes() {
        match node {
            Predicate::TargetIs(target) => { w.u8(0)?; w.target(*target)?; }
            Predicate::PayloadIs(bytes) => { w.u8(1)?; w.blob(bytes)?; }
            Predicate::PayloadAtMost(limit) => { w.u8(2)?; w.u64(*limit as u64)?; }
            Predicate::UnitsAtMost(limit) => { w.u8(3)?; w.u64(*limit)?; }
            Predicate::ExactValue { key, value } => { w.u8(4)?; w.u64(*key)?; w.blob(value)?; }
            Predicate::Absent { key } => { w.u8(5)?; w.u64(*key)?; }
            Predicate::EmptyRange { start, end } => { w.u8(6)?; w.u64(*start)?; w.u64(*end)?; }
            Predicate::All(children) | Predicate::Any(children) => {
                w.u8(if matches!(node, Predicate::All(_)) { 7 } else { 8 })?;
                w.count(children.len())?;
                for child in children { w.u64(*child as u64)?; }
            }
            Predicate::Not(child) => { w.u8(9)?; w.u64(*child as u64)?; }
        }
    }
    let c = &p.congress;
    for value in [c.generation, c.caps.per_member, c.caps.per_cohort, c.continue_minimum,
        c.continue_hold_maximum, c.narrow_at, c.suspend_at, c.minimum_members as u64,
        c.minimum_cohorts as u64] { w.u64(value)?; }
    w.count(c.members.len())?;
    for (name, member) in &c.members { w.blob(name.as_bytes())?; w.blob(member.cohort.as_bytes())?; w.u64(member.weight)?; }
    w.count(p.narrowed_targets.len())?;
    for target in &p.narrowed_targets { w.target(*target)?; }
    Ok(w.bytes)
}

pub(super) fn encode(p: &FileDeliveryProfile, path: &Path, events: &[Event]) -> Result<Vec<u8>, Error> {
    encode_iter(p, path, events.len(), events.iter())
}
pub(super) fn encode_appended(p: &FileDeliveryProfile, path: &Path, events: &[Event], next: &Event) -> Result<Vec<u8>, Error> {
    let count = events.len().checked_add(1).ok_or(Error::Overflow)?;
    encode_iter(p, path, count, events.iter().chain(std::iter::once(next)))
}
fn encode_iter<'a>(p: &FileDeliveryProfile, path: &Path, count: usize,
    events: impl Iterator<Item = &'a Event>) -> Result<Vec<u8>, Error>
{
    validate_profile(p)?;
    if count > p.limits.events || path.as_os_str().as_bytes().len() > MAX_PATH_BYTES { return Err(Error::Limit); }
    let mut w = Writer::new(p.limits.bytes);
    w.raw(DOMAIN)?; w.blob(path.as_os_str().as_bytes())?; w.blob(&profile_bytes(p)?)?; w.count(count)?;
    for event in events {
        let mut record = Writer::new(p.limits.bytes);
        write_event(&mut record, event)?;
        w.blob(&record.bytes)?;
    }
    Ok(w.bytes)
}

pub(super) fn decode(p: &FileDeliveryProfile, path: &Path, bytes: &[u8]) -> Result<Vec<Event>, Error> {
    validate_profile(p)?;
    if bytes.len() > p.limits.bytes { return Err(Error::Limit); }
    let mut r = Reader { bytes, offset: 0 };
    if r.take(8)? != DOMAIN { return Err(Error::InvalidInput); }
    let expected = profile_bytes(p)?;
    if r.blob(MAX_PATH_BYTES)? != path.as_os_str().as_bytes() || r.blob(MAX_BOOT_BYTES)? != expected.as_slice() {
        return Err(Error::Binding);
    }
    let count = r.count(p.limits.events)?;
    let mut events = Vec::new();
    events.try_reserve_exact(count).map_err(|_| Error::Limit)?;
    for _ in 0..count {
        let mut record = Reader { bytes: r.blob(p.limits.bytes)?, offset: 0 };
        events.push(read_event(&mut record)?);
        record.end()?;
    }
    r.end()?;
    if encode(p, path, &events)?.as_slice() != bytes { return Err(Error::Binding); }
    Ok(events)
}

fn write_event(w: &mut Writer, event: &Event) -> Result<(), Error> {
    match event {
        Event::Time(tick) => { w.u8(0)?; w.u64(tick.0)?; }
        Event::Propose(id, action, snapshot) => {
            if !action.required_witnesses.is_empty() || action.version != VERSION
                || action.payload.len() > MAX_PAYLOAD_BYTES { return Err(Error::InvalidInput); }
            w.u8(1)?; w.u64(*id)?; w.u32(action.version)?; w.scope(action.scope)?;
            w.target(action.target.ok_or(Error::Incomplete)?)?; w.blob(&action.payload)?;
            w.u64(action.policy_epoch)?; w.u64(action.deadline.0)?; w.u64(action.units)?; w.snapshot(snapshot)?;
        }
        Event::Review(input) => {
            if input.ballots.len() > MAX_MEMBERS { return Err(Error::Limit); }
            let mut total = 0_usize;
            for (member, ballot) in &input.ballots {
                if member.is_empty() || member.len() > MAX_IDENTIFIER_BYTES || ballot.salt.is_empty()
                    || ballot.salt.len() > MAX_FIELD_LEN { return Err(Error::InvalidInput); }
                total = total.checked_add(member.len()).and_then(|n| n.checked_add(ballot.salt.len())).ok_or(Error::Overflow)?;
            }
            if total > MAX_BALLOT_BYTES { return Err(Error::Limit); }
            w.u8(2)?; w.u64(input.attempt)?; w.u64(input.round)?; w.raw(&input.evidence_root)?;
            w.snapshot(&input.snapshot)?; w.count(input.ballots.len())?;
            for (member, ballot) in &input.ballots {
                w.blob(member.as_bytes())?;
                w.u8(match ballot.verdict { Verdict::Allow => 1, Verdict::Hold => 2, Verdict::Deny => 3, Verdict::Abstain => 4 })?;
                w.blob(&ballot.salt)?;
            }
        }
        Event::Authorize(id, snapshot) | Event::Dispatch(id, snapshot) => {
            w.u8(if matches!(event, Event::Authorize(..)) { 3 } else { 4 })?;
            w.u64(*id)?; w.snapshot(snapshot)?;
        }
        Event::Publish(id) => { w.u8(5)?; w.u64(*id)?; }
        Event::Reconcile(id) => { w.u8(6)?; w.u64(*id)?; }
        Event::Seal(id) => { w.u8(7)?; w.u64(*id)?; }
        Event::Cancel(id) => { w.u8(8)?; w.u64(*id)?; }
        Event::Fence => w.u8(9)?,
        Event::Sweep => w.u8(10)?,
    }
    Ok(())
}
fn read_event(r: &mut Reader<'_>) -> Result<Event, Error> {
    Ok(match r.u8()? {
        0 => Event::Time(ElapsedTick(r.u64()?)),
        1 => {
            let id = r.u64()?; let version = r.u32()?; let scope = r.scope()?; let target = r.target()?;
            let payload = r.blob(MAX_PAYLOAD_BYTES)?.to_vec();
            let spec = ActionSpec { version, scope, target: Some(target), payload, required_witnesses: Vec::new(),
                policy_epoch: r.u64()?, deadline: ElapsedTick(r.u64()?), units: r.u64()? };
            Event::Propose(id, spec, r.snapshot()?)
        }
        2 => {
            let attempt = r.u64()?; let round = r.u64()?;
            let evidence_root = r.take(32)?.try_into().map_err(|_| Error::Incomplete)?;
            let snapshot = r.snapshot()?; let count = r.count(MAX_MEMBERS)?;
            let mut ballots = BTreeMap::new(); let mut total = 0_usize;
            for _ in 0..count {
                let name = std::str::from_utf8(r.blob(MAX_IDENTIFIER_BYTES)?).map_err(|_| Error::InvalidInput)?.to_owned();
                if ballots.last_key_value().is_some_and(|(last, _)| last >= &name) { return Err(Error::InvalidInput); }
                let verdict = match r.u8()? { 1 => Verdict::Allow, 2 => Verdict::Hold, 3 => Verdict::Deny,
                    4 => Verdict::Abstain, _ => return Err(Error::InvalidInput) };
                let salt = r.blob(MAX_FIELD_LEN)?;
                total = total.checked_add(name.len()).and_then(|n| n.checked_add(salt.len())).ok_or(Error::Overflow)?;
                if total > MAX_BALLOT_BYTES { return Err(Error::Limit); }
                ballots.insert(name, ReferenceBallot { verdict, salt: salt.to_vec() });
            }
            Event::Review(ReferenceReview { attempt, round, evidence_root, snapshot, ballots })
        }
        3 => Event::Authorize(r.u64()?, r.snapshot()?),
        4 => Event::Dispatch(r.u64()?, r.snapshot()?),
        5 => Event::Publish(r.u64()?),
        6 => Event::Reconcile(r.u64()?),
        7 => Event::Seal(r.u64()?),
        8 => Event::Cancel(r.u64()?),
        9 => Event::Fence,
        10 => Event::Sweep,
        _ => return Err(Error::InvalidInput),
    })
}

struct Writer { bytes: Vec<u8>, maximum: usize }
impl Writer {
    fn new(maximum: usize) -> Self { Self { bytes: Vec::new(), maximum } }
    fn raw(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let next = self.bytes.len().checked_add(bytes.len()).ok_or(Error::Overflow)?;
        if next > self.maximum { return Err(Error::Limit); }
        self.bytes.try_reserve(bytes.len()).map_err(|_| Error::Limit)?;
        self.bytes.extend_from_slice(bytes); Ok(())
    }
    fn u8(&mut self, v: u8) -> Result<(), Error> { self.raw(&[v]) }
    fn u32(&mut self, v: u32) -> Result<(), Error> { self.raw(&v.to_be_bytes()) }
    fn u64(&mut self, v: u64) -> Result<(), Error> { self.raw(&v.to_be_bytes()) }
    fn count(&mut self, v: usize) -> Result<(), Error> { self.u32(u32::try_from(v).map_err(|_| Error::Limit)?) }
    fn blob(&mut self, bytes: &[u8]) -> Result<(), Error> { self.count(bytes.len())?; self.raw(bytes) }
    fn scope(&mut self, scope: Scope) -> Result<(), Error> {
        for v in [scope.tenant, scope.principal, scope.run, scope.branch, scope.authority] { self.u64(v)?; }
        self.u8(match scope.purpose { Purpose::Effect => 0, Purpose::Experiment => 1 })
    }
    fn target(&mut self, target: ResolvedTarget) -> Result<(), Error> {
        for v in [target.adapter, target.object, target.contract_version, target.expected_version, target.generation] { self.u64(v)?; }
        Ok(())
    }
    fn snapshot(&mut self, snapshot: &Snapshot) -> Result<(), Error> {
        if snapshot.values.len() > MAX_SNAPSHOT_ENTRIES { return Err(Error::Limit); }
        let total = snapshot.values.values().try_fold(0_usize, |sum, v| sum.checked_add(v.len()).ok_or(Error::Overflow))?;
        if total > MAX_SNAPSHOT_BYTES { return Err(Error::Limit); }
        self.u64(snapshot.semantic_epoch)?; self.u8(u8::from(snapshot.complete))?; self.count(snapshot.values.len())?;
        for (key, value) in &snapshot.values { self.u64(*key)?; self.blob(value)?; }
        Ok(())
    }
}
struct Reader<'a> { bytes: &'a [u8], offset: usize }
impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], Error> {
        let end = self.offset.checked_add(count).ok_or(Error::Overflow)?;
        let bytes = self.bytes.get(self.offset..end).ok_or(Error::Incomplete)?;
        self.offset = end; Ok(bytes)
    }
    fn u8(&mut self) -> Result<u8, Error> { Ok(self.take(1)?[0]) }
    fn u32(&mut self) -> Result<u32, Error> { Ok(u32::from_be_bytes(self.take(4)?.try_into().map_err(|_| Error::Incomplete)?)) }
    fn u64(&mut self) -> Result<u64, Error> { Ok(u64::from_be_bytes(self.take(8)?.try_into().map_err(|_| Error::Incomplete)?)) }
    fn count(&mut self, max: usize) -> Result<usize, Error> {
        let count = usize::try_from(self.u32()?).map_err(|_| Error::Limit)?;
        if count > max { return Err(Error::Limit); } Ok(count)
    }
    fn blob(&mut self, max: usize) -> Result<&'a [u8], Error> { let count = self.count(max)?; self.take(count) }
    fn scope(&mut self) -> Result<Scope, Error> {
        Ok(Scope { tenant: self.u64()?, principal: self.u64()?, run: self.u64()?, branch: self.u64()?, authority: self.u64()?,
            purpose: match self.u8()? { 0 => Purpose::Effect, 1 => Purpose::Experiment, _ => return Err(Error::InvalidInput) } })
    }
    fn target(&mut self) -> Result<ResolvedTarget, Error> {
        Ok(ResolvedTarget { adapter: self.u64()?, object: self.u64()?, contract_version: self.u64()?,
            expected_version: self.u64()?, generation: self.u64()? })
    }
    fn snapshot(&mut self) -> Result<Snapshot, Error> {
        let semantic_epoch = self.u64()?;
        let complete = match self.u8()? { 0 => false, 1 => true, _ => return Err(Error::InvalidInput) };
        let count = self.count(MAX_SNAPSHOT_ENTRIES)?;
        let mut values = BTreeMap::new(); let mut total = 0_usize;
        for _ in 0..count {
            let key = self.u64()?;
            if values.last_key_value().is_some_and(|(last, _)| *last >= key) { return Err(Error::InvalidInput); }
            let value = self.blob(MAX_SNAPSHOT_BYTES)?;
            total = total.checked_add(value.len()).ok_or(Error::Overflow)?;
            if total > MAX_SNAPSHOT_BYTES { return Err(Error::Limit); }
            values.insert(key, value.to_vec());
        }
        Ok(Snapshot { semantic_epoch, complete, values })
    }
    fn end(&self) -> Result<(), Error> {
        if self.offset == self.bytes.len() { Ok(()) } else { Err(Error::InvalidInput) }
    }
}
