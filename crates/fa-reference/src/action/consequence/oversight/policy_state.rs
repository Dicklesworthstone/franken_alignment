//! Ordered capture of a bounded policy-state domain, not an authenticated provider.
//! A separately held writer supplies observations; the consumer derives snapshots
//! only from a complete, explicitly closed prefix. Neither role can issue permits.

use crate::action::Scope;
use crate::{Error, Snapshot};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

pub const MAX_STATE_EVENTS: usize = 1_024;
pub const MAX_STATE_PENDING: u64 = 64;
pub const MAX_STATE_ENTRIES: usize = 256;
pub const MAX_STATE_VALUE_BYTES: usize = 8_192;
pub const MAX_STATE_SNAPSHOT_BYTES: usize = 65_536;
pub const MAX_STATE_CHANGES: usize = 64;
pub const MAX_STATE_RETAINED_BYTES: usize = 8 * 1_048_576;

/// A host-provisioned exact domain. Identity is not evidence of authentication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateSource {
    pub scope: Scope,
    pub source: u64,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateLimits {
    pub events: usize,
    pub retained_bytes: usize,
}
impl Default for StateLimits {
    fn default() -> Self { Self { events: MAX_STATE_EVENTS, retained_bytes: MAX_STATE_RETAINED_BYTES } }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateChange {
    pub key: u64,
    pub before: Option<Vec<u8>>,
    pub after: Option<Vec<u8>>,
}

/// A snapshot replaces the entire declared domain, including an empty domain.
/// A delta compares ALL preimages against the same predecessor before applying
/// any edits. Semantic changes require a new full snapshot, not a delta label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateEvent {
    Snapshot { semantic_epoch: u64, values: BTreeMap<u64, Vec<u8>> },
    Delta { semantic_epoch: u64, changes: Vec<StateChange> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateFrontier {
    pub source: StateSource,
    pub through: u64,
    pub marker_generation: u64,
}

/// Historical capture, not permission or an assertion of continued eligibility.
/// The original consumer must compare against its LIVE source again at dispatch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapturedSnapshot {
    frontier: StateFrontier,
    snapshot: Rc<Snapshot>,
}
impl CapturedSnapshot {
    pub fn frontier(&self) -> StateFrontier { self.frontier }
    pub fn snapshot(&self) -> &Snapshot { &self.snapshot }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateCaptureStatus {
    pub source: StateSource,
    pub observed_through: u64,
    pub applied_through: u64,
    pub first_missing: Option<u64>,
    pub retained_events: usize,
    pub retained_bytes: usize,
    pub closed: Option<StateFrontier>,
    pub requires_full_snapshot: bool,
    pub writer_live: bool,
    pub fault: Option<Error>,
}

#[derive(Debug)]
struct State {
    source: StateSource,
    limits: StateLimits,
    events: BTreeMap<u64, StateEvent>,
    bytes: usize,
    high: u64,
    applied: u64,
    current: Option<Snapshot>,
    closed: Option<CapturedSnapshot>,
    marker_floor: u64,
    full_snapshot_after: Option<u64>,
    last_full_snapshot: u64,
    writer_live: bool,
    fault: Option<Error>,
}

/// Read-side owner. Cloning a captured value never clones this source or a writer.
#[derive(Debug)]
pub struct PolicyStateCapture { state: Rc<RefCell<State>> }

/// Provision only to the trusted observation adapter, never to the actor/helper.
/// One writer belongs to one consumer. Dropping it makes live admission unavailable.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::policy_state::PolicyStateWriter;
/// fn duplicate(writer: PolicyStateWriter) { let _ = writer.clone(); }
/// ```
#[derive(Debug)]
pub struct PolicyStateWriter { state: Rc<RefCell<State>> }

impl PolicyStateCapture {
    pub fn new(source: StateSource, limits: StateLimits) -> Result<(Self, PolicyStateWriter), Error> {
        let scope = source.scope;
        if [scope.tenant, scope.principal, scope.run, scope.branch, scope.authority,
            source.source, source.generation].contains(&0)
            || limits.events == 0 || limits.retained_bytes == 0 { return Err(Error::InvalidInput); }
        if limits.events > MAX_STATE_EVENTS || limits.retained_bytes > MAX_STATE_RETAINED_BYTES {
            return Err(Error::Limit);
        }
        let state = Rc::new(RefCell::new(State {
            source, limits, events: BTreeMap::new(), bytes: 0, high: 0, applied: 0,
            current: None, closed: None, marker_floor: 0, full_snapshot_after: None,
            last_full_snapshot: 0, writer_live: true, fault: None,
        }));
        Ok((Self { state: Rc::clone(&state) }, PolicyStateWriter { state }))
    }

    pub fn source(&self) -> StateSource { self.state.borrow().source }
    pub fn status(&self) -> StateCaptureStatus {
        let state = self.state.borrow();
        StateCaptureStatus {
            source: state.source, observed_through: state.high, applied_through: state.applied,
            first_missing: (state.applied < state.high).then(|| state.applied + 1),
            retained_events: state.events.len(), retained_bytes: state.bytes,
            closed: state.closed.as_ref().map(CapturedSnapshot::frontier),
            requires_full_snapshot: state.full_snapshot_after.is_some(),
            writer_live: state.writer_live, fault: state.fault,
        }
    }

    pub fn capture(&self) -> Result<CapturedSnapshot, Error> {
        let state = self.state.try_borrow().map_err(|_| Error::WrongState)?;
        if let Some(error) = state.fault { return Err(error); }
        if !state.writer_live || state.full_snapshot_after.is_some() { return Err(Error::Incomplete); }
        state.closed.clone().ok_or(Error::Incomplete)
    }

    /// Compare the whole supplied snapshot, not merely policy-selected keys.
    /// Matching data is still not a permit; the existing policy/witness checks run.
    pub fn validate(&self, supplied: &Snapshot) -> Result<CapturedSnapshot, Error> {
        let captured = self.capture()?;
        if captured.snapshot() != supplied { return Err(Error::Binding); }
        Ok(captured)
    }

    /// Retained source event for bounded replay/investigation, not mutable state.
    pub fn event(&self, sequence: u64) -> Option<StateEvent> {
        self.state.borrow().events.get(&sequence).cloned()
    }
}

impl PolicyStateWriter {
    /// Observe a frame once. Exact duplicates are idempotent even after closure.
    /// Conflicts, invalid new frames and capacity loss poison this source: an Err
    /// never leaves its previous permitting snapshot eligible. No ledger changes.
    pub fn record(&self, sequence: u64, event: &StateEvent) -> Result<bool, Error> {
        let mut state = self.state.try_borrow_mut().map_err(|_| Error::WrongState)?;
        if state.fault.is_some() { return Err(Error::WrongState); }
        if let Some(previous) = state.events.get(&sequence) {
            if previous == event { return Ok(false); }
            return state.fail(Error::Binding);
        }
        state.closed = None;
        state.high = state.high.max(sequence);
        if sequence == 0 { return state.fail(Error::InvalidInput); }
        if sequence > state.applied.saturating_add(MAX_STATE_PENDING)
            || state.events.len() >= state.limits.events { return state.fail(Error::Limit); }
        let bytes = match event_bytes(event).and_then(|n| state.bytes.checked_add(n).ok_or(Error::Limit)) {
            Ok(bytes) if bytes <= state.limits.retained_bytes => bytes,
            Ok(_) => return state.fail(Error::Limit),
            Err(error) => return state.fail(error),
        };
        state.events.insert(sequence, event.clone());
        state.bytes = bytes;
        loop {
            let next = state.applied.checked_add(1).ok_or(Error::Overflow)?;
            let Some(event) = state.events.get(&next) else { break; };
            let full = matches!(event, StateEvent::Snapshot { .. });
            let snapshot = match apply(state.current.as_ref(), event) {
                Ok(snapshot) => snapshot,
                Err(error) => return state.fail(error),
            };
            state.current = Some(snapshot);
            state.applied = next;
            if full { state.last_full_snapshot = next; }
        }
        Ok(true)
    }

    /// A trusted adapter explicitly closes its ENTIRE observed prefix. A marker
    /// cannot hide a pending tail or missing middle, and old markers cannot heal
    /// observation loss. Closure does not imply an authenticated physical source.
    pub fn close(&self, through: u64, marker_generation: u64) -> Result<StateFrontier, Error> {
        let mut state = self.state.try_borrow_mut().map_err(|_| Error::WrongState)?;
        if state.fault.is_some() { return Err(Error::WrongState); }
        if through == 0 || marker_generation == 0 { return Err(Error::InvalidInput); }
        let frontier = StateFrontier { source: state.source, through, marker_generation };
        if state.closed.as_ref().is_some_and(|cut| cut.frontier == frontier) { return Ok(frontier); }
        if marker_generation <= state.marker_floor { return Err(Error::Stale); }
        if state.applied != through || state.high != through { return Err(Error::Incomplete); }
        if state.full_snapshot_after.is_some_and(|after| state.last_full_snapshot <= after) {
            return Err(Error::Incomplete);
        }
        let mut snapshot = state.current.clone().ok_or(Error::Incomplete)?;
        snapshot.complete = true;
        state.closed = Some(CapturedSnapshot { frontier, snapshot: Rc::new(snapshot) });
        state.marker_floor = marker_generation;
        state.full_snapshot_after = None;
        Ok(frontier)
    }

    /// Missing observation is not an empty snapshot. Require a newly observed
    /// complete image after this cut before delta-based admission can resume.
    pub fn withdraw(&self) {
        let mut state = self.state.borrow_mut();
        state.closed = None;
        state.full_snapshot_after = Some(state.high);
    }
}

impl Drop for PolicyStateWriter {
    fn drop(&mut self) {
        let mut state = self.state.borrow_mut();
        state.writer_live = false;
        state.closed = None;
    }
}

impl State {
    fn fail<T>(&mut self, error: Error) -> Result<T, Error> {
        self.closed = None;
        self.fault = Some(error);
        Err(error)
    }
}

fn values_bytes(values: &BTreeMap<u64, Vec<u8>>) -> Result<usize, Error> {
    if values.len() > MAX_STATE_ENTRIES { return Err(Error::Limit); }
    let mut bytes = 0_usize;
    for value in values.values() {
        if value.len() > MAX_STATE_VALUE_BYTES { return Err(Error::Limit); }
        bytes = bytes.checked_add(value.len()).ok_or(Error::Limit)?;
    }
    if bytes > MAX_STATE_SNAPSHOT_BYTES { return Err(Error::Limit); }
    Ok(bytes)
}

fn event_bytes(event: &StateEvent) -> Result<usize, Error> {
    match event {
        StateEvent::Snapshot { values, .. } => values_bytes(values),
        StateEvent::Delta { changes, .. } => {
            if changes.is_empty() { return Err(Error::InvalidInput); }
            if changes.len() > MAX_STATE_CHANGES { return Err(Error::Limit); }
            let mut keys = BTreeSet::new();
            let mut bytes = 0_usize;
            for change in changes {
                if !keys.insert(change.key) { return Err(Error::Duplicate); }
                for value in change.before.iter().chain(change.after.iter()) {
                    if value.len() > MAX_STATE_VALUE_BYTES { return Err(Error::Limit); }
                    bytes = bytes.checked_add(value.len()).ok_or(Error::Limit)?;
                }
            }
            Ok(bytes)
        }
    }
}

fn apply(previous: Option<&Snapshot>, event: &StateEvent) -> Result<Snapshot, Error> {
    match event {
        StateEvent::Snapshot { semantic_epoch, values } => {
            if previous.is_some_and(|old| *semantic_epoch < old.semantic_epoch) { return Err(Error::Stale); }
            Ok(Snapshot { semantic_epoch: *semantic_epoch, complete: false, values: values.clone() })
        }
        StateEvent::Delta { semantic_epoch, changes } => {
            let previous = previous.ok_or(Error::Incomplete)?;
            if *semantic_epoch != previous.semantic_epoch { return Err(Error::Stale); }
            for change in changes {
                if previous.values.get(&change.key) != change.before.as_ref() { return Err(Error::Binding); }
            }
            let mut next = previous.clone();
            for change in changes {
                match &change.after {
                    Some(value) => { next.values.insert(change.key, value.clone()); }
                    None => { next.values.remove(&change.key); }
                }
            }
            values_bytes(&next.values)?;
            Ok(next)
        }
    }
}
