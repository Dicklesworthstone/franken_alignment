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

