//! Independent, deliberately small logical models. No external effects,
//! persistence, cryptographic claims, calibration, or deployment guarantees.
#![forbid(unsafe_code)]

pub mod action;
pub mod canonical_json;
pub mod evidence_view;
pub mod full_input;
pub mod history;
pub mod perimeter;
pub mod product_frontier;
pub mod reducer;
pub mod round;
pub mod strict_json;
pub mod trace_independence;
pub mod witness;

use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidInput,
    Incomplete,
    Limit,
    Overflow,
    Duplicate,
    Missing,
    WrongState,
    Stale,
    Binding,
}

/// A complete logical snapshot in a bounded reference universe, not a database.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub semantic_epoch: u64,
    pub complete: bool,
    pub values: BTreeMap<u64, Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadWitness {
    Exact { key: u64, value: Option<Vec<u8>> },
    EmptyRange { start: u64, end: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Judgment {
    semantic_epoch: u64,
    witnesses: Vec<ReadWitness>,
}

impl Judgment {
    pub fn capture(snapshot: &Snapshot, witnesses: Vec<ReadWitness>) -> Result<Self, Error> {
        let judgment = Self {
            semantic_epoch: snapshot.semantic_epoch,
            witnesses,
        };
        if judgment.valid_at(snapshot)? {
            Ok(judgment)
        } else {
            Err(Error::Binding)
        }
    }

    pub fn valid_at(&self, snapshot: &Snapshot) -> Result<bool, Error> {
        if !snapshot.complete {
            return Err(Error::Incomplete);
        }
        if snapshot.semantic_epoch != self.semantic_epoch {
            return Ok(false);
        }
        for witness in &self.witnesses {
            match witness {
                ReadWitness::Exact { key, value } => {
                    if snapshot.values.get(key) != value.as_ref() {
                        return Ok(false);
                    }
                }
                ReadWitness::EmptyRange { start, end } => {
                    if start >= end {
                        return Err(Error::InvalidInput);
                    }
                    if snapshot.values.range(*start..*end).next().is_some() {
                        return Ok(false);
                    }
                }
            }
        }
        Ok(true)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frontier {
    contiguous: u64,
    pending: BTreeSet<u64>,
    max_gap: u64,
}

impl Frontier {
    pub fn new(max_gap: u64) -> Result<Self, Error> {
        if max_gap == 0 || max_gap > 1_000_000 {
            return Err(Error::InvalidInput);
        }
        Ok(Self {
            contiguous: 0,
            pending: BTreeSet::new(),
            max_gap,
        })
    }

    pub fn contains(&self, sequence: u64) -> bool {
        sequence > 0 && (sequence <= self.contiguous || self.pending.contains(&sequence))
    }

    pub fn contiguous(&self) -> u64 {
        self.contiguous
    }

    pub fn accept(&mut self, sequence: u64) -> Result<(), Error> {
        if sequence == 0 {
            return Err(Error::InvalidInput);
        }
        if sequence <= self.contiguous {
            return Ok(());
        }
        if sequence - self.contiguous > self.max_gap {
            return Err(Error::Limit);
        }
        self.pending.insert(sequence);
        while let Some(next) = self.contiguous.checked_add(1) {
            if !self.pending.remove(&next) {
                break;
            }
            self.contiguous = next;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvidenceFrontiers {
    captured: Frontier,
    verified: Frontier,
    judged: Frontier,
}

impl EvidenceFrontiers {
    pub fn new(max_gap: u64) -> Result<Self, Error> {
        Ok(Self {
            captured: Frontier::new(max_gap)?,
            verified: Frontier::new(max_gap)?,
            judged: Frontier::new(max_gap)?,
        })
    }

    pub fn capture(&mut self, sequence: u64) -> Result<(), Error> {
        self.captured.accept(sequence)
    }

    /// Caller supplies a verification fact; this model performs no cryptography.
    pub fn verify(&mut self, sequence: u64) -> Result<(), Error> {
        if !self.captured.contains(sequence) {
            return Err(Error::Incomplete);
        }
        self.verified.accept(sequence)
    }

    pub fn judge(&mut self, sequence: u64) -> Result<(), Error> {
        if !self.verified.contains(sequence) {
            return Err(Error::Incomplete);
        }
        self.judged.accept(sequence)
    }

    pub fn covers_closed_scope(&self, last: u64) -> bool {
        last > 0 && self.judged.contiguous() >= last
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Effect {
    pub principal: String,
    pub resolved_target: String,
    pub payload: Vec<u8>,
    pub units: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Reserved,
    Dispatched,
    Unknown,
    Committed,
    Aborted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Reservation {
    effect: Effect,
    epoch: u64,
    state: State,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rights {
    total: u64,
    available: u64,
    spent: u64,
    epoch: u64,
    incident_count: u64,
    reservations: BTreeMap<u64, Reservation>,
}

impl Rights {
    pub fn new(total: u64) -> Self {
        Self {
            total,
            available: total,
            spent: 0,
            epoch: 0,
            incident_count: 0,
            reservations: BTreeMap::new(),
        }
    }

    pub fn available(&self) -> u64 {
        self.available
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn incident_count(&self) -> u64 {
        self.incident_count
    }

    pub fn state(&self, id: u64) -> Result<State, Error> {
        self.reservations
            .get(&id)
            .map(|r| r.state)
            .ok_or(Error::Missing)
    }

    pub fn revoke_epoch(&mut self) -> Result<(), Error> {
        self.epoch = self.epoch.checked_add(1).ok_or(Error::Overflow)?;
        Ok(())
    }

    pub fn reserve(&mut self, id: u64, effect: Effect) -> Result<(), Error> {
        if self.reservations.contains_key(&id) {
            return Err(Error::Duplicate);
        }
        if effect.units == 0 || effect.principal.is_empty() || effect.resolved_target.is_empty() {
            return Err(Error::InvalidInput);
        }
        if effect.units > self.available {
            return Err(Error::Limit);
        }
        self.available -= effect.units;
        self.reservations.insert(
            id,
            Reservation {
                effect,
                epoch: self.epoch,
                state: State::Reserved,
            },
        );
        Ok(())
    }

    /// Records a simulated dispatch; no external operation occurs.
    pub fn dispatch(&mut self, id: u64, effect: &Effect) -> Result<(), Error> {
        let reservation = self.reservations.get_mut(&id).ok_or(Error::Missing)?;
        if reservation.state != State::Reserved {
            return Err(Error::WrongState);
        }
        if reservation.epoch != self.epoch {
            return Err(Error::Stale);
        }
        if &reservation.effect != effect {
            return Err(Error::Binding);
        }
        reservation.state = State::Dispatched;
        Ok(())
    }

    pub fn mark_unknown(&mut self, id: u64) -> Result<(), Error> {
        let reservation = self.reservations.get_mut(&id).ok_or(Error::Missing)?;
        if reservation.state != State::Dispatched {
            return Err(Error::WrongState);
        }
        reservation.state = State::Unknown;
        Ok(())
    }

    pub fn abort_before_dispatch(&mut self, id: u64) -> Result<(), Error> {
        let reservation = self.reservations.get_mut(&id).ok_or(Error::Missing)?;
        if reservation.state != State::Reserved {
            return Err(Error::WrongState);
        }
        self.available = self
            .available
            .checked_add(reservation.effect.units)
            .ok_or(Error::Overflow)?;
        reservation.state = State::Aborted;
        Ok(())
    }

    /// A trusted reference input, not a verified remote receipt implementation.
    pub fn reconcile(&mut self, id: u64, occurred: bool) -> Result<(), Error> {
        let reservation = self.reservations.get_mut(&id).ok_or(Error::Missing)?;
        if !matches!(reservation.state, State::Dispatched | State::Unknown) {
            return Err(Error::WrongState);
        }
        if occurred {
            self.spent = self
                .spent
                .checked_add(reservation.effect.units)
                .ok_or(Error::Overflow)?;
            reservation.state = State::Committed;
        } else {
            self.available = self
                .available
                .checked_add(reservation.effect.units)
                .ok_or(Error::Overflow)?;
            reservation.state = State::Aborted;
        }
        Ok(())
    }

    /// Models the control-ledger portion of containment reset only.
    ///
    /// Actor state restoration is outside this reference model. Reserved work
    /// outside the retained checkpoint set has not dispatched and is therefore
    /// aborted; dispatched and unknown effects remain historical liabilities.
    pub fn reset_to_checkpoint(&mut self, keep: &BTreeSet<u64>) -> Result<(), Error> {
        if !self.conserved() {
            return Err(Error::WrongState);
        }

        let refund = self
            .reservations
            .iter()
            .filter(|(id, reservation)| !keep.contains(*id) && reservation.state == State::Reserved)
            .try_fold(0_u64, |total, (_, reservation)| {
                total.checked_add(reservation.effect.units)
            })
            .ok_or(Error::Overflow)?;
        let available = self.available.checked_add(refund).ok_or(Error::Overflow)?;
        let incident_count = self.incident_count.checked_add(1).ok_or(Error::Overflow)?;

        for (id, reservation) in &mut self.reservations {
            if !keep.contains(id) && reservation.state == State::Reserved {
                reservation.state = State::Aborted;
            }
        }
        self.available = available;
        self.incident_count = incident_count;

        if self.conserved() {
            Ok(())
        } else {
            Err(Error::WrongState)
        }
    }

    pub fn conserved(&self) -> bool {
        let held = self
            .reservations
            .values()
            .filter(|r| {
                matches!(
                    r.state,
                    State::Reserved | State::Dispatched | State::Unknown
                )
            })
            .try_fold(0_u64, |n, r| n.checked_add(r.effect.units));
        held.and_then(|n| n.checked_add(self.available))
            .and_then(|n| n.checked_add(self.spent))
            == Some(self.total)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Operation {
    pub reads: BTreeSet<u64>,
    pub writes: BTreeSet<u64>,
    pub consuming_resources: BTreeSet<u64>,
    pub external_or_ordered: bool,
}

pub fn independent(a: &Operation, b: &Operation) -> bool {
    !a.external_or_ordered
        && !b.external_or_ordered
        && a.writes.is_disjoint(&b.writes)
        && a.writes.is_disjoint(&b.reads)
        && b.writes.is_disjoint(&a.reads)
        && a.consuming_resources.is_disjoint(&b.consuming_resources)
}

/// Independent bounded reachability oracle, not the scalable graph engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Graph {
    edges: Vec<Vec<usize>>,
}

impl Graph {
    pub fn new(nodes: usize, edges: &[(usize, usize)]) -> Result<Self, Error> {
        if nodes == 0 || nodes > 128 || edges.len() > 16_384 {
            return Err(Error::Limit);
        }
        let mut graph = Self {
            edges: vec![Vec::new(); nodes],
        };
        for &(from, to) in edges {
            if from >= nodes || to >= nodes {
                return Err(Error::InvalidInput);
            }
            graph.edges[from].push(to);
        }
        for row in &mut graph.edges {
            row.sort_unstable();
            row.dedup();
        }
        Ok(graph)
    }

    pub fn reachable(
        &self,
        sources: &[usize],
        blocked: &BTreeSet<usize>,
    ) -> Result<BTreeSet<usize>, Error> {
        if sources.is_empty()
            || sources
                .iter()
                .chain(blocked.iter())
                .any(|&n| n >= self.edges.len())
        {
            return Err(Error::InvalidInput);
        }
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::new();
        for &source in sources {
            if !blocked.contains(&source) && seen.insert(source) {
                queue.push_back(source);
            }
        }
        while let Some(node) = queue.pop_front() {
            for &next in &self.edges[node] {
                if !blocked.contains(&next) && seen.insert(next) {
                    queue.push_back(next);
                }
            }
        }
        Ok(seen)
    }

    pub fn cut_disconnects(
        &self,
        sources: &[usize],
        sinks: &[usize],
        cut: &BTreeSet<usize>,
    ) -> Result<bool, Error> {
        if sinks.is_empty() || sinks.iter().any(|&n| n >= self.edges.len()) {
            return Err(Error::InvalidInput);
        }
        let reachable = self.reachable(sources, cut)?;
        Ok(sinks.iter().all(|sink| !reachable.contains(sink)))
    }

    /// None means the sink was already unreachable: do not report vacuous coverage.
    pub fn dominates(
        &self,
        source: usize,
        candidate: usize,
        sink: usize,
    ) -> Result<Option<bool>, Error> {
        if candidate >= self.edges.len() || sink >= self.edges.len() {
            return Err(Error::InvalidInput);
        }
        if !self.reachable(&[source], &BTreeSet::new())?.contains(&sink) {
            return Ok(None);
        }
        Ok(Some(
            !self
                .reachable(&[source], &BTreeSet::from([candidate]))?
                .contains(&sink),
        ))
    }
}

/// Integer linear probe under an externally established per-coordinate error bound.
/// Some(true/false) certifies a strict positive/negative sign only in this model.
/// None requests refinement. This neither validates the bound nor certifies a model.
pub fn linear_sign(
    weights: &[i64],
    reconstruction: &[i64],
    threshold: i128,
    max_coordinate_error: u64,
) -> Result<Option<bool>, Error> {
    if weights.is_empty() || weights.len() != reconstruction.len() {
        return Err(Error::InvalidInput);
    }
    let mut dot = 0_i128;
    let mut norm = 0_i128;
    for (&w, &x) in weights.iter().zip(reconstruction) {
        dot = dot
            .checked_add(i128::from(w) * i128::from(x))
            .ok_or(Error::Overflow)?;
        norm = norm
            .checked_add(i128::from(w).abs())
            .ok_or(Error::Overflow)?;
    }
    let margin = dot.checked_sub(threshold).ok_or(Error::Overflow)?;
    let bound = norm
        .checked_mul(i128::from(max_coordinate_error))
        .ok_or(Error::Overflow)?;
    if margin > bound {
        Ok(Some(true))
    } else if margin < -bound {
        Ok(Some(false))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> Snapshot {
        Snapshot {
            semantic_epoch: 1,
            complete: true,
            values: BTreeMap::from([(1, b"artifact".to_vec())]),
        }
    }

    fn effect() -> Effect {
        Effect {
            principal: "actor".into(),
            resolved_target: "object:7".into(),
            payload: b"exact bytes".to_vec(),
            units: 3,
        }
    }

    #[test]
    fn witness_reuses_unrelated_changes() {
        let mut s = snapshot();
        let j = Judgment::capture(
            &s,
            vec![ReadWitness::Exact {
                key: 1,
                value: Some(b"artifact".to_vec()),
            }],
        )
        .unwrap();
        s.values.insert(99, vec![0]);
        assert_eq!(j.valid_at(&s), Ok(true));
        s.values.insert(1, vec![1]);
        assert_eq!(j.valid_at(&s), Ok(false));
    }

    #[test]
    fn absent_key_detects_insertion() {
        let mut s = snapshot();
        let j = Judgment::capture(
            &s,
            vec![ReadWitness::Exact {
                key: 7,
                value: None,
            }],
        )
        .unwrap();
        s.values.insert(7, vec![]);
        assert_eq!(j.valid_at(&s), Ok(false));
    }

    #[test]
    fn empty_range_detects_phantom() {
        let mut s = snapshot();
        let j =
            Judgment::capture(&s, vec![ReadWitness::EmptyRange { start: 10, end: 20 }]).unwrap();
        s.values.insert(20, vec![]);
        assert_eq!(j.valid_at(&s), Ok(true));
        s.values.insert(19, vec![]);
        assert_eq!(j.valid_at(&s), Ok(false));
    }

    #[test]
    fn epoch_and_incompleteness_invalidate() {
        let mut s = snapshot();
        let j = Judgment::capture(&s, vec![]).unwrap();
        s.semantic_epoch += 1;
        assert_eq!(j.valid_at(&s), Ok(false));
        s.complete = false;
        assert_eq!(j.valid_at(&s), Err(Error::Incomplete));
    }

    #[test]
    fn invalid_witness_is_not_admitted() {
        let s = snapshot();
        assert_eq!(
            Judgment::capture(&s, vec![ReadWitness::EmptyRange { start: 2, end: 2 }]),
            Err(Error::InvalidInput)
        );
        assert_eq!(
            Judgment::capture(
                &s,
                vec![ReadWitness::Exact {
                    key: 1,
                    value: None
                }]
            ),
            Err(Error::Binding)
        );
    }

    #[test]
    fn frontier_does_not_skip_a_gap() {
        let mut f = Frontier::new(8).unwrap();
        f.accept(3).unwrap();
        f.accept(1).unwrap();
        assert_eq!(f.contiguous(), 1);
        f.accept(2).unwrap();
        assert_eq!(f.contiguous(), 3);
        f.accept(2).unwrap();
        assert_eq!(f.contiguous(), 3);
    }

    #[test]
    fn frontier_limits_fail_without_mutation() {
        let mut f = Frontier::new(2).unwrap();
        let before = f.clone();
        assert_eq!(f.accept(3), Err(Error::Limit));
        assert_eq!(f.accept(0), Err(Error::InvalidInput));
        assert_eq!(f, before);
    }

    #[test]
    fn evidence_stages_are_distinct() {
        let mut f = EvidenceFrontiers::new(10).unwrap();
        assert_eq!(f.verify(1), Err(Error::Incomplete));
        f.capture(1).unwrap();
        assert_eq!(f.judge(1), Err(Error::Incomplete));
        f.verify(1).unwrap();
        f.judge(1).unwrap();
        assert!(f.covers_closed_scope(1));
        assert!(!f.covers_closed_scope(2));
    }

    #[test]
    fn rights_unknown_cannot_be_refunded_by_cancel() {
        let mut r = Rights::new(10);
        let e = effect();
        r.reserve(1, e.clone()).unwrap();
        r.dispatch(1, &e).unwrap();
        r.mark_unknown(1).unwrap();
        assert_eq!(r.abort_before_dispatch(1), Err(Error::WrongState));
        assert_eq!(r.available(), 7);
        assert!(r.conserved());
        r.reconcile(1, true).unwrap();
        assert!(r.conserved());
        assert_eq!(r.reconcile(1, false), Err(Error::WrongState));
    }

    #[test]
    fn effect_binding_and_one_shot_dispatch() {
        let mut r = Rights::new(10);
        let e = effect();
        r.reserve(1, e.clone()).unwrap();
        let mut changed = e.clone();
        changed.payload.push(0);
        assert_eq!(r.dispatch(1, &changed), Err(Error::Binding));
        r.dispatch(1, &e).unwrap();
        assert_eq!(r.dispatch(1, &e), Err(Error::WrongState));
        assert!(r.conserved());
    }

    #[test]
    fn revocation_fences_reserved_not_history() {
        let mut r = Rights::new(10);
        let e = effect();
        r.reserve(1, e.clone()).unwrap();
        r.revoke_epoch().unwrap();
        assert_eq!(r.dispatch(1, &e), Err(Error::Stale));
        r.abort_before_dispatch(1).unwrap();
        r.reserve(2, e.clone()).unwrap();
        r.dispatch(2, &e).unwrap();
        r.revoke_epoch().unwrap();
        assert_eq!(r.state(2), Ok(State::Dispatched));
        assert!(r.conserved());
    }

    #[test]
    fn reserve_budget_and_duplicate_fail_closed() {
        let mut r = Rights::new(3);
        r.reserve(1, effect()).unwrap();
        assert_eq!(r.reserve(1, effect()), Err(Error::Duplicate));
        assert_eq!(r.reserve(2, effect()), Err(Error::Limit));
        assert!(r.conserved());
    }

    #[test]
    fn resolved_non_effect_returns_rights_once() {
        let mut r = Rights::new(3);
        let e = effect();
        r.reserve(1, e.clone()).unwrap();
        r.dispatch(1, &e).unwrap();
        r.mark_unknown(1).unwrap();
        r.reconcile(1, false).unwrap();
        assert_eq!(r.available(), 3);
        assert_eq!(r.reconcile(1, false), Err(Error::WrongState));
        assert!(r.conserved());
    }

    #[test]
    fn reset_keeps_spent_and_epoch() {
        let mut r = Rights::new(10);
        let e = effect();
        r.reserve(1, e.clone()).unwrap();
        r.dispatch(1, &e).unwrap();
        r.reconcile(1, true).unwrap();
        r.revoke_epoch().unwrap();
        let spent = r.spent;
        let epoch = r.epoch();
        let available = r.available();

        r.reset_to_checkpoint(&BTreeSet::new()).unwrap();

        assert_eq!(r.state(1), Ok(State::Committed));
        assert_eq!(r.spent, spent);
        assert_eq!(r.epoch(), epoch);
        assert_eq!(r.available(), available);
        assert_eq!(r.incident_count(), 1);
        assert!(r.conserved());
    }

    #[test]
    fn reset_increments_incident_counter() {
        let mut r = Rights::new(10);
        r.reset_to_checkpoint(&BTreeSet::new()).unwrap();
        r.reset_to_checkpoint(&BTreeSet::new()).unwrap();
        assert_eq!(r.incident_count(), 2);
        assert!(r.conserved());
    }

    #[test]
    fn reset_keeps_dispatched_liability() {
        let mut r = Rights::new(10);
        let e = effect();
        r.reserve(1, e.clone()).unwrap();
        r.dispatch(1, &e).unwrap();
        let available = r.available();
        let spent = r.spent;

        r.reset_to_checkpoint(&BTreeSet::new()).unwrap();

        assert_eq!(r.state(1), Ok(State::Dispatched));
        assert_eq!(r.available(), available);
        assert_eq!(r.spent, spent);
        assert_eq!(r.incident_count(), 1);
        assert!(r.conserved());
    }

    #[test]
    fn reset_cannot_refund_unknown() {
        let mut r = Rights::new(10);
        let e = effect();
        r.reserve(1, e.clone()).unwrap();
        r.dispatch(1, &e).unwrap();
        r.mark_unknown(1).unwrap();
        let available = r.available();
        let spent = r.spent;
        let epoch = r.epoch();

        r.reset_to_checkpoint(&BTreeSet::new()).unwrap();

        assert_eq!(r.state(1), Ok(State::Unknown));
        assert_eq!(r.available(), available);
        assert_eq!(r.spent, spent);
        assert_eq!(r.epoch(), epoch);
        assert_eq!(r.incident_count(), 1);
        assert!(r.conserved());
    }

    #[test]
    fn reset_refunds_only_reserved_outside_keep() {
        let mut r = Rights::new(10);
        let kept = effect();
        let mut refunded = effect();
        refunded.resolved_target = "object:8".into();
        refunded.units = 5;
        r.reserve(1, kept).unwrap();
        r.reserve(2, refunded).unwrap();

        r.reset_to_checkpoint(&BTreeSet::from([1])).unwrap();

        assert_eq!(r.state(1), Ok(State::Reserved));
        assert_eq!(r.state(2), Ok(State::Aborted));
        assert_eq!(r.available(), 7);
        assert_eq!(r.incident_count(), 1);
        assert!(r.conserved());
    }

    #[test]
    fn reset_overflow_is_before_mutation() {
        let mut r = Rights::new(10);
        r.reserve(1, effect()).unwrap();
        r.incident_count = u64::MAX;
        let before = r.clone();

        assert_eq!(
            r.reset_to_checkpoint(&BTreeSet::new()),
            Err(Error::Overflow)
        );
        assert_eq!(r, before);
    }

    #[test]
    fn cross_reads_defeat_disjoint_writes() {
        let a = Operation {
            reads: BTreeSet::from([2]),
            writes: BTreeSet::from([1]),
            ..Operation::default()
        };
        let b = Operation {
            reads: BTreeSet::from([1]),
            writes: BTreeSet::from([2]),
            ..Operation::default()
        };
        assert!(!independent(&a, &b));
    }

    #[test]
    fn resources_and_order_barriers_block_reordering() {
        let mut a = Operation::default();
        let mut b = Operation::default();
        assert!(independent(&a, &b));
        a.consuming_resources.insert(7);
        b.consuming_resources.insert(7);
        assert!(!independent(&a, &b));
        b.consuming_resources.clear();
        b.external_or_ordered = true;
        assert!(!independent(&a, &b));
    }

    #[test]
    fn dominator_and_bypass_are_distinguished() {
        let g = Graph::new(3, &[(0, 1), (1, 2)]).unwrap();
        assert_eq!(g.dominates(0, 1, 2), Ok(Some(true)));
        let bypass = Graph::new(3, &[(0, 1), (1, 2), (0, 2)]).unwrap();
        assert_eq!(bypass.dominates(0, 1, 2), Ok(Some(false)));
        let empty = Graph::new(3, &[]).unwrap();
        assert_eq!(empty.dominates(0, 1, 2), Ok(None));
    }

    #[test]
    fn graph_limits_and_empty_claims_are_rejected() {
        assert_eq!(Graph::new(129, &[]), Err(Error::Limit));
        assert_eq!(Graph::new(2, &[(0, 2)]), Err(Error::InvalidInput));
        let g = Graph::new(2, &[]).unwrap();
        assert_eq!(
            g.cut_disconnects(&[0], &[], &BTreeSet::new()),
            Err(Error::InvalidInput)
        );
    }

    #[test]
    fn exhaustive_four_vertex_cut_and_dominator_consistency() {
        let pairs: Vec<_> = (0..4)
            .flat_map(|a| (0..4).filter(move |&b| a != b).map(move |b| (a, b)))
            .collect();
        for mask in 0..(1_u32 << pairs.len()) {
            let edges: Vec<_> = pairs
                .iter()
                .enumerate()
                .filter_map(|(i, &edge)| ((mask & (1 << i)) != 0).then_some(edge))
                .collect();
            let g = Graph::new(4, &edges).unwrap();
            if let Some(dominates) = g.dominates(0, 1, 3).unwrap() {
                assert_eq!(
                    dominates,
                    g.cut_disconnects(&[0], &[3], &BTreeSet::from([1])).unwrap()
                );
            }
        }
    }

    #[test]
    fn integer_probe_requests_refinement_at_margin() {
        assert_eq!(linear_sign(&[2, -1], &[10, 2], 0, 1), Ok(Some(true)));
        assert_eq!(linear_sign(&[1], &[1], 0, 1), Ok(None));
        assert_eq!(linear_sign(&[1], &[-4], 0, 1), Ok(Some(false)));
        assert_eq!(linear_sign(&[], &[], 0, 1), Err(Error::InvalidInput));
        assert_eq!(linear_sign(&[0], &[0], i128::MIN, 0), Err(Error::Overflow));
    }

    #[test]
    fn integer_probe_bound_matches_exhaustive_small_errors() {
        for x in -10..=10 {
            for y in -10..=10 {
                if let Some(sign) = linear_sign(&[2, -3], &[x, y], 1, 2).unwrap() {
                    for dx in -2..=2 {
                        for dy in -2..=2 {
                            assert_eq!(2 * (x + dx) - 3 * (y + dy) - 1 > 0, sign);
                        }
                    }
                }
            }
        }
    }
}
