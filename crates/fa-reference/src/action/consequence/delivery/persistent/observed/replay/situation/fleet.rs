//! Declared-roster historical inspection, never a global atomic cut or authority.
//! Each member retains its original reader, reducer, accounting and read basis.
use super::{FileSituation, FileSituationObservation, FileSituationPageToken,
    FileSituationReplay, FileSituationReplayStatus, JournalError, Knowledge,
    MAX_SITUATION_PAGE_ATTEMPTS, MAX_SITUATION_REPLAY_EVENTS};
use crate::action::consequence::oversight::actor::UnknownReason;
use crate::Error;
use std::{collections::BTreeSet, fmt, rc::Rc};

pub const MAX_FLEET_SITUATION_MEMBERS: usize = 64;
pub const MAX_FLEET_SITUATION_EVENTS: usize = 1_048_576;

/// Aggregate replay admission, not a bound on bytes already read by each reader.
/// Callers must also bound source/recipe intake and elapsed work. An individual
/// original event (including numerical inference) remains synchronous.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileSituationFleetLimits { pub members: usize, pub events: usize }
impl Default for FileSituationFleetLimits {
    fn default() -> Self {
        Self { members: MAX_FLEET_SITUATION_MEMBERS, events: MAX_FLEET_SITUATION_EVENTS }
    }
}

/// No paths, payloads, arbitrary error strings or inferred nonexecution receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileSituationReadFailure {
    InputUnavailable,
    ReplayRejected,
    CaptureRejected,
    Interrupted,
}

/// Coverage of the CALLER'S DECLARED roster only. Unknown members contribute no
/// fabricated zero balances. Independent cuts/currencies are never added here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileSituationFleetCoverage {
    pub declared_members: usize,
    pub known_members: usize,
    pub unknown_members: usize,
}
impl FileSituationFleetCoverage {
    pub fn all_members_known(self) -> bool { self.unknown_members == 0 }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileSituationFleetReplayStatus { Reading, Ready }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileSituationFleetProgress {
    pub status: FileSituationFleetReplayStatus,
    pub declared_members: usize,
    pub pending_members: usize,
    pub known_members: usize,
    pub unknown_members: usize,
    pub attempted_events: usize,
    pub checked_events: usize,
    /// Includes unread events in members later rejected during replay.
    pub admitted_events: usize,
}

enum Member {
    Reading(Box<FileSituationReplay>),
    Known(Box<FileSituation>),
    Unknown(FileSituationReadFailure),
}

/// Cooperatively visit original replay cursors in deterministic round-robin
/// order. None means a declared member's input could not be admitted; it is not
/// an empty history. Admission validates the whole roster and event allowance
/// before replay. Only pristine cursors may enter, so prior work is not hidden.
///
/// A failed member is retired as Unknown, without blocking the other members or
/// retrying through a weaker reader. Dropping this cursor cancels private work.
/// No member page or partially verified history is exported before finish.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::replay::situation::fleet::FileSituationFleetReplay;
/// fn no_prefix(cursor: FileSituationFleetReplay) { cursor.page(None, 1); }
/// ```
pub struct FileSituationFleetReplay {
    members: Vec<(u64, Member)>,
    next: usize,
    attempted: usize,
    checked: usize,
    admitted: usize,
}
impl fmt::Debug for FileSituationFleetReplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSituationFleetReplay").field("progress", &self.progress()).finish_non_exhaustive()
    }
}
impl FileSituationFleetReplay {
    pub fn new(inputs: Vec<(u64, Option<FileSituationReplay>)>, limits: FileSituationFleetLimits)
        -> Result<Self, JournalError>
    {
        if limits.members == 0 || inputs.is_empty() { return Err(Error::InvalidInput.into()); }
        if limits.members > MAX_FLEET_SITUATION_MEMBERS || inputs.len() > limits.members
            || limits.events > MAX_FLEET_SITUATION_EVENTS { return Err(Error::Limit.into()); }
        let mut ids = BTreeSet::new();
        let mut admitted = 0usize;
        for (id, cursor) in &inputs {
            if *id == 0 { return Err(Error::InvalidInput.into()); }
            if !ids.insert(*id) { return Err(Error::Duplicate.into()); }
            if let Some(cursor) = cursor {
                let progress = cursor.progress();
                if progress.checked_events != 0 || progress.status == FileSituationReplayStatus::Failed {
                    return Err(Error::Binding.into());
                }
                admitted = admitted.checked_add(progress.total_events).ok_or(Error::Overflow)?;
                if admitted > limits.events { return Err(Error::Limit.into()); }
            }
        }
        let mut members: Vec<_> = inputs.into_iter().map(|(id, cursor)| {
            let member = match cursor {
                None => Member::Unknown(FileSituationReadFailure::InputUnavailable),
                Some(cursor) if cursor.progress().status == FileSituationReplayStatus::Ready => capture(cursor),
                Some(cursor) => Member::Reading(Box::new(cursor)),
            };
            (id, member)
        }).collect();
        members.sort_unstable_by_key(|(id, _)| *id);
        Ok(Self { members, next: 0, attempted: 0, checked: 0, admitted })
    }

    pub fn progress(&self) -> FileSituationFleetProgress {
        let mut pending = 0; let mut known = 0; let mut unknown = 0;
        for (_, member) in &self.members {
            match member {
                Member::Reading(_) => pending += 1,
                Member::Known(_) => known += 1,
                Member::Unknown(_) => unknown += 1,
            }
        }
        FileSituationFleetProgress {
            status: if pending == 0 { FileSituationFleetReplayStatus::Ready }
                else { FileSituationFleetReplayStatus::Reading },
            declared_members: self.members.len(), pending_members: pending,
            known_members: known, unknown_members: unknown, attempted_events: self.attempted,
            checked_events: self.checked, admitted_events: self.admitted,
        }
    }

    /// At most `events` original transitions TOTAL, not per member. A long or
    /// broken journal cannot consume another member's turn. Zero is a no-op.
    pub fn advance(&mut self, events: usize) -> Result<FileSituationFleetProgress, JournalError> {
        if events > MAX_SITUATION_REPLAY_EVENTS { return Err(Error::Limit.into()); }
        for _ in 0..events {
            let len = self.members.len();
            let Some(index) = (0..len).map(|offset| (self.next + offset) % len)
                .find(|index| matches!(&self.members[*index].1, Member::Reading(_))) else { break; };
            self.next = (index + 1) % len;
            // Retire before entering original synchronous work. If a caller
            // catches an unwind, the failed member stays Unknown, not resumable.
            let member = std::mem::replace(&mut self.members[index].1,
                Member::Unknown(FileSituationReadFailure::Interrupted));
            let Member::Reading(mut cursor) = member else { unreachable!("selected reading member"); };
            self.attempted += 1;
            self.members[index].1 = match cursor.advance(1) {
                Err(_) => Member::Unknown(FileSituationReadFailure::ReplayRejected),
                Ok(progress) => {
                    self.checked += 1;
                    if progress.status == FileSituationReplayStatus::Ready { capture(*cursor) }
                    else { Member::Reading(cursor) }
                }
            };
        }
        Ok(self.progress())
    }

    pub fn finish(self) -> Result<FileSituationFleet, JournalError> {
        if self.progress().pending_members != 0 { return Err(Error::Incomplete.into()); }
        let mut members = Vec::with_capacity(self.members.len());
        let mut known = 0;
        for (id, member) in self.members {
            let result = match member {
                Member::Known(situation) => { known += 1; Ok(*situation) }
                Member::Unknown(reason) => Err(reason),
                Member::Reading(_) => return Err(Error::Incomplete.into()),
            };
            members.push((id, result));
        }
        // Missing reads first, then unresolved outcomes, then pending work.
        // This is lifecycle triage, not planner relevance or a live action grant.
        members.sort_unstable_by_key(|(id, result)| (match result {
            Err(_) => 0,
            Ok(situation) if situation.summary.unresolved_attempts != 0 => 1,
            Ok(situation) if situation.summary.pending_attempts != 0 => 2,
            Ok(_) => 3,
        }, *id));
        let coverage = FileSituationFleetCoverage { declared_members: members.len(),
            known_members: known, unknown_members: members.len() - known };
        Ok(FileSituationFleet { capture: Rc::new(()), members, coverage })
    }

    pub fn complete(mut self) -> Result<FileSituationFleet, JournalError> {
        while self.progress().status == FileSituationFleetReplayStatus::Reading {
            self.advance(MAX_SITUATION_REPLAY_EVENTS)?;
        }
        self.finish()
    }
}
fn capture(cursor: FileSituationReplay) -> Member {
    match cursor.finish() {
        Ok(situation) => Member::Known(Box::new(situation)),
        Err(_) => Member::Unknown(FileSituationReadFailure::CaptureRejected),
    }
}

/// Immutable declared-roster data. There is no common journal revision, fleet
/// clock, global balance or authentication/completeness claim across members.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::replay::situation::fleet::FileSituationFleet;
/// fn no_effect(report: FileSituationFleet) { report.publish(1, 1); }
/// ```
pub struct FileSituationFleet {
    capture: Rc<()>,
    members: Vec<(u64, Result<FileSituation, FileSituationReadFailure>)>,
    coverage: FileSituationFleetCoverage,
}
impl fmt::Debug for FileSituationFleet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSituationFleet").field("coverage", &self.coverage).finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct FileSituationFleetPageToken {
    capture: Rc<()>, member: usize, within: Option<FileSituationPageToken>,
}
impl fmt::Debug for FileSituationFleetPageToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSituationFleetPageToken").field("member", &self.member).finish_non_exhaustive()
    }
}
#[derive(Debug)]
pub struct FileSituationFleetPage {
    pub coverage: FileSituationFleetCoverage,
    pub member: u64,
    pub observation: FileSituationObservation,
    pub failure: Option<FileSituationReadFailure>,
    /// None completes enumeration, even if some members remain Unknown.
    pub next: Option<FileSituationFleetPageToken>,
}
impl FileSituationFleet {
    pub fn coverage(&self) -> FileSituationFleetCoverage { self.coverage }

    /// One member's bounded page, or its explicit unavailable observation.
    /// Every member appears, including empty valid journals and failed inputs.
    /// The inner page's continuation covers that member only; use `next` on the
    /// fleet page to enumerate the declared roster. Tokens cannot cross captures.
    pub fn page(&self, continuation: Option<&FileSituationFleetPageToken>, limit: usize)
        -> Result<FileSituationFleetPage, JournalError>
    {
        if limit == 0 { return Err(Error::InvalidInput.into()); }
        if limit > MAX_SITUATION_PAGE_ATTEMPTS { return Err(Error::Limit.into()); }
        let (index, within) = match continuation {
            Some(token) if Rc::ptr_eq(&token.capture, &self.capture) => (token.member, token.within.as_ref()),
            Some(_) => return Err(Error::Binding.into()),
            None => (0, None),
        };
        let (id, situation) = self.members.get(index).ok_or(Error::Binding)?;
        let (observation, failure, inner) = match situation {
            Ok(situation) => {
                let observation = situation.page(within, limit)?;
                let inner = match &observation {
                    Knowledge::Known { value, .. } => value.next.clone(),
                    _ => return Err(Error::Binding.into()),
                };
                (observation, None, inner)
            }
            Err(reason) => {
                if within.is_some() { return Err(Error::Binding.into()); }
                (Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable }, Some(*reason), None)
            }
        };
        let next = if inner.is_some() {
            Some(FileSituationFleetPageToken { capture: Rc::clone(&self.capture), member: index, within: inner })
        } else if index + 1 < self.members.len() {
            Some(FileSituationFleetPageToken { capture: Rc::clone(&self.capture), member: index + 1, within: None })
        } else { None };
        Ok(FileSituationFleetPage { coverage: self.coverage, member: *id, observation, failure, next })
    }
}
