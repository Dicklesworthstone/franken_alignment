//! A distinct profile of the same bounded canonical publication journal.
//! Records are original transition INPUTS, never asserted decisions or balances.
use super::{FileOversightProfile, ReviewWindow};
use super::super::{codec, Event as BaseEvent};
use super::super::codec::shared::{Reader, Writer};
use super::views::{self, Views};
use crate::action::ElapsedTick;
use crate::round::{Digest, Verdict, MAX_FIELD_LEN};
use crate::{Error, Snapshot};
use std::path::Path;

const DOMAIN: &[u8; 8] = b"FAOVR\0\0\x01";
const MAX_CONFIG_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Copy)]
pub(super) enum HumanDecision { Approve, Reject, Revoke }
#[derive(Clone)]
pub(super) enum Event {
    Core(BaseEvent),
    Inputs(u64, u64, Views),
    InputsUnavailable(u64, u64),
    Begin(u64, u64, [u8; 32], ReviewWindow, Snapshot),
    Commit(u64, String, Digest),
    OpenReveals(u64),
    Reveal(u64, String, Verdict, Vec<u8>),
    Finish(u64, Option<Views>, Snapshot),
    Authorize(u64, u64, Snapshot),
    RequestHuman(u64, u64, u64, ElapsedTick),
    Human(u64, HumanDecision),
    RevokeHumans,
    Dispatch(u64, u64, u64, Snapshot),
}

fn core_allowed(event: &BaseEvent) -> bool {
    // New events added to the simpler profile are NOT admitted implicitly.
    matches!(event, BaseEvent::Time(_) | BaseEvent::Propose(..) | BaseEvent::Publish(_)
        | BaseEvent::Reconcile(_) | BaseEvent::Seal(_) | BaseEvent::Cancel(_) | BaseEvent::Fence
        | BaseEvent::Sweep | BaseEvent::Stop(_) | BaseEvent::StopProgress(_))
}

fn config(p: &FileOversightProfile) -> Result<Vec<u8>, Error> {
    let mut w = Writer::new(MAX_CONFIG_BYTES);
    w.count(p.committee.members().len())?;
    for (member, helper) in p.committee.members() {
        views::write_name(&mut w, member)?;
        // The original contract explicitly substitutes the ACTION's policy epoch
        // at every use. Normalize only that unused bootstrap epoch, not any
        // served-model/tokenizer/profile identity or recorded observation epoch.
        views::write_profile(&mut w, &helper.profile_at(0))?;
        w.u64(helper.projection_id())?; w.blob(helper.question())?;
    }
    w.u64(p.human.reviewer_id)?; w.u64(p.human.max_validity_ticks)?;
    w.count(p.human.max_requests)?;
    Ok(w.finish())
}

pub(super) fn encode(p: &FileOversightProfile, path: &Path, events: &[Event]) -> Result<Vec<u8>, Error> {
    encode_iter(p, path, events.len(), events.iter())
}
pub(super) fn encode_appended(p: &FileOversightProfile, path: &Path, events: &[Event], next: &Event) -> Result<Vec<u8>, Error> {
    encode_iter(p, path, events.len().checked_add(1).ok_or(Error::Overflow)?, events.iter().chain(std::iter::once(next)))
}
fn encode_iter<'a>(p: &FileOversightProfile, path: &Path, count: usize, events: impl Iterator<Item = &'a Event>) -> Result<Vec<u8>, Error> {
    codec::validate_profile(&p.delivery)?;
    if count > p.delivery.limits.events { return Err(Error::Limit); }
    let mut w = Writer::new(p.delivery.limits.bytes);
    w.raw(DOMAIN)?; w.bootstrap(&p.delivery, path)?; w.blob(&config(p)?)?; w.count(count)?;
    for event in events {
        let mut record = Writer::new(p.delivery.limits.bytes);
        write_event(&mut record, event)?;
        w.blob(&record.finish())?;
    }
    Ok(w.finish())
}

pub(super) fn decode(p: &FileOversightProfile, path: &Path, bytes: &[u8]) -> Result<Vec<Event>, Error> {
    codec::validate_profile(&p.delivery)?;
    if bytes.len() > p.delivery.limits.bytes { return Err(Error::Limit); }
    let mut r = Reader::new(bytes);
    if r.take(DOMAIN.len())? != DOMAIN { return Err(Error::Binding); }
    r.bootstrap(&p.delivery, path)?;
    if r.blob(MAX_CONFIG_BYTES)? != config(p)?.as_slice() { return Err(Error::Binding); }
    let count = r.count(p.delivery.limits.events)?;
    let mut events = Vec::new();
    events.try_reserve_exact(count).map_err(|_| Error::Limit)?;
    for _ in 0..count {
        let mut record = Reader::new(r.blob(p.delivery.limits.bytes)?);
        events.push(read_event(&mut record)?);
        record.end()?;
    }
    r.end()?;
    if encode(p, path, &events)?.as_slice() != bytes { return Err(Error::Binding); }
    Ok(events)
}

fn write_event(w: &mut Writer, event: &Event) -> Result<(), Error> {
    match event {
        Event::Core(event) => {
            if !core_allowed(event) { return Err(Error::Binding); }
            w.u8(0)?; w.event(event)?;
        }
        Event::Inputs(id, revision, inputs) => { w.u8(1)?; w.u64(*id)?; w.u64(*revision)?; views::write(w, inputs)?; }
        Event::InputsUnavailable(id, revision) => { w.u8(2)?; w.u64(*id)?; w.u64(*revision)?; }
        Event::Begin(id, round, root, window, snapshot) => {
            w.u8(3)?; w.u64(*id)?; w.u64(*round)?; w.raw(root)?;
            w.u64(window.commit_by.0)?; w.u64(window.reveal_by.0)?; w.snapshot(snapshot)?;
        }
        Event::Commit(round, member, digest) => { w.u8(4)?; w.u64(*round)?; views::write_name(w, member)?; w.u64(*digest)?; }
        Event::OpenReveals(round) => { w.u8(5)?; w.u64(*round)?; }
        Event::Reveal(round, member, verdict, salt) => {
            if salt.len() > MAX_FIELD_LEN { return Err(Error::Limit); }
            w.u8(6)?; w.u64(*round)?; views::write_name(w, member)?;
            w.u8(match verdict { Verdict::Allow => 0, Verdict::Hold => 1, Verdict::Deny => 2, Verdict::Abstain => 3 })?;
            w.blob(salt)?;
        }
        Event::Finish(round, current, snapshot) => {
            w.u8(7)?; w.u64(*round)?;
            match current { None => w.u8(0)?, Some(views) => { w.u8(1)?; views::write(w, views)?; } }
            w.snapshot(snapshot)?;
        }
        Event::Authorize(id, revision, snapshot) => { w.u8(8)?; w.u64(*id)?; w.u64(*revision)?; w.snapshot(snapshot)?; }
        Event::RequestHuman(request, id, revision, expires) => {
            w.u8(9)?; w.u64(*request)?; w.u64(*id)?; w.u64(*revision)?; w.u64(expires.0)?;
        }
        Event::Human(request, decision) => {
            w.u8(10)?; w.u64(*request)?;
            w.u8(match decision { HumanDecision::Approve => 0, HumanDecision::Reject => 1, HumanDecision::Revoke => 2 })?;
        }
        Event::RevokeHumans => w.u8(11)?,
        Event::Dispatch(id, human, revision, snapshot) => {
            w.u8(12)?; w.u64(*id)?; w.u64(*human)?; w.u64(*revision)?; w.snapshot(snapshot)?;
        }
    }
    Ok(())
}
fn read_event(r: &mut Reader<'_>) -> Result<Event, Error> {
    Ok(match r.u8()? {
        0 => { let event = r.event()?; if !core_allowed(&event) { return Err(Error::Binding); } Event::Core(event) }
        1 => Event::Inputs(r.u64()?, r.u64()?, views::read(r)?),
        2 => Event::InputsUnavailable(r.u64()?, r.u64()?),
        3 => Event::Begin(r.u64()?, r.u64()?, r.take(32)?.try_into().map_err(|_| Error::Incomplete)?,
            ReviewWindow { commit_by: ElapsedTick(r.u64()?), reveal_by: ElapsedTick(r.u64()?) }, r.snapshot()?),
        4 => Event::Commit(r.u64()?, views::read_name(r)?, r.u64()?),
        5 => Event::OpenReveals(r.u64()?),
        6 => {
            let round = r.u64()?; let member = views::read_name(r)?;
            let verdict = match r.u8()? { 0 => Verdict::Allow, 1 => Verdict::Hold, 2 => Verdict::Deny,
                3 => Verdict::Abstain, _ => return Err(Error::InvalidInput) };
            Event::Reveal(round, member, verdict, r.blob(MAX_FIELD_LEN)?.to_vec())
        }
        7 => {
            let round = r.u64()?;
            let current = match r.u8()? { 0 => None, 1 => Some(views::read(r)?), _ => return Err(Error::InvalidInput) };
            Event::Finish(round, current, r.snapshot()?)
        }
        8 => Event::Authorize(r.u64()?, r.u64()?, r.snapshot()?),
        9 => Event::RequestHuman(r.u64()?, r.u64()?, r.u64()?, ElapsedTick(r.u64()?)),
        10 => {
            let request = r.u64()?;
            let decision = match r.u8()? { 0 => HumanDecision::Approve, 1 => HumanDecision::Reject,
                2 => HumanDecision::Revoke, _ => return Err(Error::InvalidInput) };
            Event::Human(request, decision)
        }
        11 => Event::RevokeHumans,
        12 => Event::Dispatch(r.u64()?, r.u64()?, r.u64()?, r.snapshot()?),
        _ => return Err(Error::InvalidInput),
    })
}
