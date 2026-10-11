//! Bounded quotient replay over the original registered independence relation.
//!
//! This is a closed logical reference machine: reads, writes, policy decisions,
//! resource consumption, deterministic RNG state and ordered observations. It
//! does not execute production callbacks or authorize external effects. Every
//! modeled access and ordering barrier must be covered by the immutable FA-092
//! operation registration; a reviewed pair label alone is never a proof.
//!
//! Each distinct occurrence has a stable identity. The oracle enumerates every
//! permutation (including every dependent order), chooses a representative only
//! through original independent swaps, and compares complete logical outputs.
//! Operation failures are recorded per occurrence and do not halt the trace.
//! Thus failed work stays in the denominator and later operations still execute.
//! This failure semantics is deliberately narrower than a global abort executor.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    DomainId, RegisteredTraceOperation, RelationEvaluation, TraceRelationRegistry, independent,
};
use crate::Error;
use crate::action::consequence::{Decision, DecisionInputs, Restriction, decide};

pub const MAX_QUOTIENT_OPERATIONS: usize = 8;
pub const MAX_QUOTIENT_SCHEDULES: usize = 40_320;
pub const MAX_QUOTIENT_CELLS: usize = 32;
pub const MAX_QUOTIENT_RETAINED_BYTES: usize = 64 * 1_048_576;
const TRACE_HEADER: &[u8; 8] = b"FAQTRC01";

/// All cells, streams and resources use the same whole-domain namespace.
/// Check(None) reads absence; Check(Some(value)) requires that exact value.
/// Missing required evidence holds; a present wrong value produces a denial.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayInstruction {
    Read { domain: DomainId },
    Write { domain: DomainId, value: Option<u64> },
    CopyAdd { source: DomainId, target: DomainId, addend: u64 },
    Check { domain: DomainId, expected: Option<u64> },
    Consume { resource: DomainId, units: u64 },
    Draw { stream: DomainId },
    Authority { enabled: bool },
    Ordered { domain: DomainId },
    External { domain: DomainId },
}

/// Registration and typed code are bound once. Extra declared dependencies
/// remain conservative barriers; omitted actual dependencies are rejected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayOccurrence {
    id: u64,
    operation: RegisteredTraceOperation,
    instruction: ReplayInstruction,
}

impl ReplayOccurrence {
    pub fn new(
        id: u64,
        operation: RegisteredTraceOperation,
        instruction: ReplayInstruction,
    ) -> Result<Self, Error> {
        if id == 0 {
            return Err(Error::InvalidInput);
        }
        let declared = &operation.validated;
        let covered = match instruction {
            ReplayInstruction::Read { domain } => declared.positive_read_domains.contains(&domain),
            ReplayInstruction::Write { domain, .. } => declared.write_domains.contains(&domain),
            ReplayInstruction::CopyAdd { source, target, .. } => {
                declared.positive_read_domains.contains(&source)
                    && declared.write_domains.contains(&target)
            }
            ReplayInstruction::Check { domain, expected } => {
                if expected.is_some() {
                    declared.positive_read_domains.contains(&domain)
                } else {
                    declared.negative_read_domains.contains(&domain)
                }
            }
            ReplayInstruction::Consume { resource, .. } => {
                declared.consuming_resources.contains(&resource)
            }
            ReplayInstruction::Draw { stream } => declared.rng_streams.contains(&stream),
            ReplayInstruction::Authority { .. } => declared.authority_transition,
            ReplayInstruction::Ordered { domain } => {
                declared.ordered_effect && declared.positive_read_domains.contains(&domain)
            }
            ReplayInstruction::External { domain } => {
                declared.external_effect && declared.positive_read_domains.contains(&domain)
            }
        };
        if !covered {
            return Err(Error::Binding);
        }
        Ok(Self { id, operation, instruction })
    }

    pub fn id(&self) -> u64 { self.id }
    pub fn operation(&self) -> &RegisteredTraceOperation { &self.operation }
    pub fn instruction(&self) -> ReplayInstruction { self.instruction }
}

/// An explicit fixture state, never a live authority or storage handle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayState {
    cells: BTreeMap<DomainId, u64>,
    authority_enabled: bool,
}

impl ReplayState {
    pub fn new(cells: BTreeMap<DomainId, u64>, authority_enabled: bool) -> Result<Self, Error> {
        if cells.len() > MAX_QUOTIENT_CELLS {
            return Err(Error::Limit);
        }
        Ok(Self { cells, authority_enabled })
    }

    pub fn cells(&self) -> &BTreeMap<DomainId, u64> { &self.cells }
    pub fn authority_enabled(&self) -> bool { self.authority_enabled }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplayValue {
    Observed(Option<u64>),
    Written,
    Consumed { remaining: u64 },
    Drawn(u64),
    Decision(Decision),
    Rejected(Error),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OccurrenceResult {
    pub occurrence: u64,
    pub value: ReplayValue,
}

/// Includes held external attempts as well as admitted fixture observations.
/// Retaining this order prevents a same-final-state comparison hiding an effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OrderedObservation {
    pub occurrence: u64,
    pub domain: DomainId,
    pub observed: Option<u64>,
    pub external: bool,
    pub admitted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogicalReplay {
    state: ReplayState,
    results: Vec<OccurrenceResult>,
    ordered: Vec<OrderedObservation>,
}

impl LogicalReplay {
    pub fn state(&self) -> &ReplayState { &self.state }
    /// Sorted by stable occurrence identity, not execution position.
    pub fn results(&self) -> &[OccurrenceResult] { &self.results }
    /// Ordered attempts retain execution order, including rejected attempts.
    pub fn ordered(&self) -> &[OrderedObservation] { &self.ordered }
}

/// Order-only trace encoding. These bytes require the original program, initial
/// fixture state and registration to replay; this is not a self-contained log.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TraceSchedule {
    occurrences: Vec<u64>,
}

impl TraceSchedule {
    pub fn occurrences(&self) -> &[u64] { &self.occurrences }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(16 + 8 * self.occurrences.len());
        bytes.extend_from_slice(TRACE_HEADER);
        bytes.extend_from_slice(&(self.occurrences.len() as u64).to_be_bytes());
        for occurrence in &self.occurrences {
            bytes.extend_from_slice(&occurrence.to_be_bytes());
        }
        bytes
    }
}

/// A finite admission limit, all dimensions checked before any replay.
/// Work counts are logical instructions, relation checks and compared entries,
/// not CPU cycles. Retained bytes bound modeled payload (including keys and
/// complete worst-case representative results), excluding borrowed declarations,
/// allocator/tree overhead, stack footprint and process RSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplayBudget {
    pub schedules: usize,
    pub replay_steps: usize,
    pub canonical_checks: usize,
    pub comparison_entries: usize,
    pub retained_bytes: usize,
}

impl Default for ReplayBudget {
    fn default() -> Self {
        Self {
            schedules: MAX_QUOTIENT_SCHEDULES,
            replay_steps: 2 * MAX_QUOTIENT_SCHEDULES * MAX_QUOTIENT_OPERATIONS,
            canonical_checks: MAX_QUOTIENT_SCHEDULES * 92 + 28,
            comparison_entries: MAX_QUOTIENT_SCHEDULES * 49,
            retained_bytes: MAX_QUOTIENT_RETAINED_BYTES,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplayAdmission {
    pub occurrences: usize,
    pub maximum_cells: usize,
    pub schedule_bytes: usize,
    /// Worst case: every schedule has its own representative, each replayed
    /// cold in addition to the complete unreduced population.
    pub required: ReplayBudget,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReplayProgress {
    pub unreduced_replays: usize,
    pub representative_replays: usize,
    /// Only successful full logical comparisons; excludes a mismatching row.
    pub compared_schedules: usize,
    pub replay_steps: usize,
    pub canonical_checks: usize,
    /// Conservatively charged full logical entries, even if equality exits early.
    pub comparison_entries: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayRepresentative {
    schedule: TraceSchedule,
    replay: LogicalReplay,
    members: usize,
}

impl ReplayRepresentative {
    pub fn schedule(&self) -> &TraceSchedule { &self.schedule }
    pub fn replay(&self) -> &LogicalReplay { &self.replay }
    pub fn members(&self) -> usize { self.members }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuotientReport {
    admission: ReplayAdmission,
    relation: RelationEvaluation,
    progress: ReplayProgress,
    representatives: Vec<ReplayRepresentative>,
}

impl QuotientReport {
    pub fn admission(&self) -> ReplayAdmission { self.admission }
    pub fn relation(&self) -> RelationEvaluation { self.relation }
    pub fn progress(&self) -> ReplayProgress { self.progress }
    pub fn representatives(&self) -> &[ReplayRepresentative] { &self.representatives }
    pub fn unreduced_trace_bytes(&self) -> usize {
        self.admission.required.schedules * self.admission.schedule_bytes
    }
    pub fn stored_trace_bytes(&self) -> usize {
        self.representatives.len() * self.admission.schedule_bytes
    }
}

/// A failure retains the original full denominator and the exact failing order,
/// its proposed representative, and both actual computed logical outputs.
/// Partial replay is never returned as a successful population report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayMismatch {
    pub admission: ReplayAdmission,
    pub progress: ReplayProgress,
    pub schedule: TraceSchedule,
    pub representative: TraceSchedule,
    pub actual: LogicalReplay,
    pub expected: LogicalReplay,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplayFailure {
    Admission(Error),
    Mismatch(Box<ReplayMismatch>),
}

impl From<Error> for ReplayFailure {
    fn from(error: Error) -> Self { Self::Admission(error) }
}

/// Count factorial schedules and worst-case work/storage before execution.
/// This is conservative even if most schedules later share one representative.
pub fn estimate(
    occurrences: &[ReplayOccurrence],
    initial: &ReplayState,
) -> Result<ReplayAdmission, Error> {
    validate_occurrences(occurrences)?;
    let n = occurrences.len();
    let mut domains = initial.cells.keys().copied().collect::<BTreeSet<_>>();
    for occurrence in occurrences {
        match occurrence.instruction {
            ReplayInstruction::Read { domain }
            | ReplayInstruction::Write { domain, .. }
            | ReplayInstruction::Check { domain, .. }
            | ReplayInstruction::Ordered { domain }
            | ReplayInstruction::External { domain } => { domains.insert(domain); }
            ReplayInstruction::CopyAdd { source, target, .. } => {
                domains.insert(source);
                domains.insert(target);
            }
            ReplayInstruction::Consume { resource, .. } => { domains.insert(resource); }
            ReplayInstruction::Draw { stream } => { domains.insert(stream); }
            ReplayInstruction::Authority { .. } => {}
        }
    }
    if domains.len() > MAX_QUOTIENT_CELLS {
        return Err(Error::Limit);
    }
    let schedules = (1..=n).try_fold(1usize, |count, next| {
        count.checked_mul(next).ok_or(Error::Overflow)
    })?;
    // All following products are bounded by n <= 8 and domains <= 32.
    let pairs = n * n.saturating_sub(1) / 2;
    let schedule_bytes = 16 + 8 * n;
    let result_bytes = 32 + 16 * domains.len() + 64 * n;
    let required = ReplayBudget {
        schedules,
        replay_steps: 2 * schedules * n,
        canonical_checks: pairs + schedules * (pairs + n * n),
        comparison_entries: schedules * (domains.len() + 2 * n + 1),
        retained_bytes: schedules * (64 + schedule_bytes + result_bytes)
            + 2 * result_bytes + 512,
    };
    if !fits(required, ReplayBudget::default()) {
        return Err(Error::Limit);
    }
    Ok(ReplayAdmission {
        occurrences: n,
        maximum_cells: domains.len(),
        schedule_bytes,
        required,
    })
}

/// Lexicographically least order reachable using only original independent
/// swaps. Every dependent pair retains its order from the supplied schedule.
/// This is a bounded reference canonicalizer, not a production DPOR algorithm.
pub fn canonicalize(
    registry: &TraceRelationRegistry,
    occurrences: &[ReplayOccurrence],
    schedule: &[u64],
) -> Result<TraceSchedule, Error> {
    let order = validate_schedule(occurrences, schedule)?;
    let (matrix, _) = relation(registry, occurrences);
    let mut checks = 0;
    Ok(trace(occurrences, &canonical_order(occurrences, &matrix, &order, &mut checks)))
}

/// Replay one exact bounded schedule from the original fixture state.
/// Rejected operations remain explicit results and do not halt later work.
pub fn replay(
    occurrences: &[ReplayOccurrence],
    initial: &ReplayState,
    schedule: &[u64],
) -> Result<LogicalReplay, Error> {
    estimate(occurrences, initial)?;
    let order = validate_schedule(occurrences, schedule)?;
    Ok(execute(occurrences, initial, &order))
}

/// Exhaustively compare every unreduced permutation against its independently
/// cold-replayed canonical representative. No callback can assert success and
/// no alternate independence matrix is accepted through this public API.
pub fn verify(
    registry: &TraceRelationRegistry,
    occurrences: &[ReplayOccurrence],
    initial: &ReplayState,
    budget: ReplayBudget,
) -> Result<QuotientReport, ReplayFailure> {
    let admission = estimate(occurrences, initial)?;
    if !fits(budget, ReplayBudget::default()) || !fits(admission.required, budget) {
        return Err(Error::Limit.into());
    }
    let (matrix, evaluation) = relation(registry, occurrences);
    verify_relation(occurrences, initial, admission, matrix, evaluation)
}

fn fits(required: ReplayBudget, limit: ReplayBudget) -> bool {
    required.schedules <= limit.schedules
        && required.replay_steps <= limit.replay_steps
        && required.canonical_checks <= limit.canonical_checks
        && required.comparison_entries <= limit.comparison_entries
        && required.retained_bytes <= limit.retained_bytes
}

fn validate_occurrences(occurrences: &[ReplayOccurrence]) -> Result<(), Error> {
    if occurrences.len() > MAX_QUOTIENT_OPERATIONS {
        return Err(Error::Limit);
    }
    let mut seen = BTreeSet::new();
    for occurrence in occurrences {
        if !seen.insert(occurrence.id) {
            return Err(Error::Duplicate);
        }
    }
    Ok(())
}

fn validate_schedule(
    occurrences: &[ReplayOccurrence],
    schedule: &[u64],
) -> Result<Vec<usize>, Error> {
    validate_occurrences(occurrences)?;
    if schedule.len() != occurrences.len() {
        return Err(Error::Binding);
    }
    let mut order = Vec::with_capacity(schedule.len());
    let mut seen = BTreeSet::new();
    for occurrence_id in schedule {
        if !seen.insert(*occurrence_id) {
            return Err(Error::Duplicate);
        }
        let index = occurrences.iter().position(|occurrence| occurrence.id == *occurrence_id)
            .ok_or(Error::Binding)?;
        order.push(index);
    }
    Ok(order)
}

type Matrix = [[bool; MAX_QUOTIENT_OPERATIONS]; MAX_QUOTIENT_OPERATIONS];

fn relation(
    registry: &TraceRelationRegistry,
    occurrences: &[ReplayOccurrence],
) -> (Matrix, RelationEvaluation) {
    let mut matrix = [[false; MAX_QUOTIENT_OPERATIONS]; MAX_QUOTIENT_OPERATIONS];
    let mut evaluated_pairs = 0;
    let mut independent_pairs = 0;
    for left in 0..occurrences.len() {
        for right in left + 1..occurrences.len() {
            let commutes = independent(registry, &occurrences[left].operation, &occurrences[right].operation);
            matrix[left][right] = commutes;
            matrix[right][left] = commutes;
            evaluated_pairs += 1;
            independent_pairs += usize::from(commutes);
        }
    }
    (matrix, RelationEvaluation {
        evaluated_pairs,
        independent_pairs,
        ordered_pairs: evaluated_pairs - independent_pairs,
    })
}

fn canonical_order(
    occurrences: &[ReplayOccurrence],
    matrix: &Matrix,
    order: &[usize],
    checks: &mut usize,
) -> Vec<usize> {
    let mut predecessors = [0u16; MAX_QUOTIENT_OPERATIONS];
    for (position, left) in order.iter().copied().enumerate() {
        for right in order[position + 1..].iter().copied() {
            *checks += 1;
            if !matrix[left][right] {
                predecessors[right] |= 1u16 << left;
            }
        }
    }
    let mut choices = (0..occurrences.len()).collect::<Vec<_>>();
    choices.sort_by_key(|index| occurrences[*index].id);
    let mut done = 0u16;
    let mut canonical = Vec::with_capacity(order.len());
    for _ in 0..order.len() {
        for index in choices.iter().copied() {
            *checks += 1;
            let bit = 1u16 << index;
            if done & bit == 0 && predecessors[index] & !done == 0 {
                canonical.push(index);
                done |= bit;
                break;
            }
        }
    }
    // Edges all follow the original total order, so the graph is acyclic.
    debug_assert_eq!(canonical.len(), order.len());
    canonical
}

fn trace(occurrences: &[ReplayOccurrence], order: &[usize]) -> TraceSchedule {
    TraceSchedule {
        occurrences: order.iter().map(|index| occurrences[*index].id).collect(),
    }
}

fn verify_relation(
    occurrences: &[ReplayOccurrence],
    initial: &ReplayState,
    admission: ReplayAdmission,
    matrix: Matrix,
    evaluation: RelationEvaluation,
) -> Result<QuotientReport, ReplayFailure> {
    let mut order = (0..occurrences.len()).collect::<Vec<_>>();
    order.sort_by_key(|index| occurrences[*index].id);
    let mut progress = ReplayProgress {
        canonical_checks: evaluation.evaluated_pairs,
        ..ReplayProgress::default()
    };
    let mut representatives = Vec::<ReplayRepresentative>::new();
    // Reserve the maximum number of records before executing any instruction.
    representatives.try_reserve_exact(admission.required.schedules)
        .map_err(|_| ReplayFailure::Admission(Error::Limit))?;
    let mut by_order = BTreeMap::<[u64; MAX_QUOTIENT_OPERATIONS], usize>::new();
    loop {
        let canonical = canonical_order(occurrences, &matrix, &order, &mut progress.canonical_checks);
        let mut key = [0u64; MAX_QUOTIENT_OPERATIONS];
        for (position, index) in canonical.iter().copied().enumerate() {
            key[position] = occurrences[index].id;
        }
        let representative_index = match by_order.get(&key).copied() {
            Some(index) => index,
            None => {
                let replay = execute(occurrences, initial, &canonical);
                progress.representative_replays += 1;
                progress.replay_steps += occurrences.len();
                let index = representatives.len();
                representatives.push(ReplayRepresentative {
                    schedule: trace(occurrences, &canonical),
                    replay,
                    members: 0,
                });
                by_order.insert(key, index);
                index
            }
        };
        let actual = execute(occurrences, initial, &order);
        progress.unreduced_replays += 1;
        progress.replay_steps += occurrences.len();
        progress.comparison_entries += admission.maximum_cells + 2 * occurrences.len() + 1;
        let representative = &mut representatives[representative_index];
        if actual != representative.replay {
            return Err(ReplayFailure::Mismatch(Box::new(ReplayMismatch {
                admission,
                progress,
                schedule: trace(occurrences, &order),
                representative: representative.schedule.clone(),
                actual,
                expected: representative.replay.clone(),
            })));
        }
        progress.compared_schedules += 1;
        representative.members += 1;
        if !next_order(occurrences, &mut order) {
            break;
        }
    }
    debug_assert_eq!(progress.compared_schedules, admission.required.schedules);
    Ok(QuotientReport { admission, relation: evaluation, progress, representatives })
}

fn next_order(occurrences: &[ReplayOccurrence], order: &mut [usize]) -> bool {
    if order.len() < 2 {
        return false;
    }
    let Some(pivot) = (0..order.len() - 1).rev()
        .find(|index| occurrences[order[*index]].id < occurrences[order[*index + 1]].id)
    else { return false };
    let successor = (pivot + 1..order.len()).rev()
        .find(|index| occurrences[order[pivot]].id < occurrences[order[*index]].id)
        .expect("a successor exists for an increasing pivot");
    order.swap(pivot, successor);
    order[pivot + 1..].reverse();
    true
}

fn execute(
    occurrences: &[ReplayOccurrence],
    initial: &ReplayState,
    order: &[usize],
) -> LogicalReplay {
    let mut result = LogicalReplay {
        state: initial.clone(),
        results: Vec::with_capacity(order.len()),
        ordered: Vec::with_capacity(order.len()),
    };
    for index in order.iter().copied() {
        let occurrence = &occurrences[index];
        let value = match occurrence.instruction {
            ReplayInstruction::Read { domain } => {
                ReplayValue::Observed(result.state.cells.get(&domain).copied())
            }
            ReplayInstruction::Write { domain, value } => {
                match value {
                    Some(value) => { result.state.cells.insert(domain, value); }
                    None => { result.state.cells.remove(&domain); }
                }
                ReplayValue::Written
            }
            ReplayInstruction::CopyAdd { source, target, addend } => {
                match result.state.cells.get(&source).copied() {
                    None => ReplayValue::Rejected(Error::Missing),
                    Some(value) => match value.checked_add(addend) {
                        None => ReplayValue::Rejected(Error::Overflow),
                        Some(value) => {
                            result.state.cells.insert(target, value);
                            ReplayValue::Written
                        }
                    }
                }
            }
            ReplayInstruction::Check { domain, expected } => {
                let actual = result.state.cells.get(&domain).copied();
                let mandatory_absent = actual.is_none() && expected.is_some();
                ReplayValue::Decision(decide(DecisionInputs {
                    empirical: Restriction::Continue,
                    exact_disqualifier: actual != expected && !mandatory_absent,
                    mandatory_absent,
                    contradiction: false,
                }))
            }
            ReplayInstruction::Consume { resource, units } => {
                match result.state.cells.get(&resource).copied() {
                    None => ReplayValue::Rejected(Error::Missing),
                    Some(available) => match available.checked_sub(units) {
                        None => ReplayValue::Rejected(Error::Limit),
                        Some(remaining) => {
                            result.state.cells.insert(resource, remaining);
                            ReplayValue::Consumed { remaining }
                        }
                    }
                }
            }
            ReplayInstruction::Draw { stream } => {
                match result.state.cells.get(&stream).copied() {
                    None => ReplayValue::Rejected(Error::Missing),
                    Some(previous) => {
                        // Fixed fixture transition; no cryptographic/statistical claim.
                        let next = previous.wrapping_mul(6_364_136_223_846_793_005)
                            .wrapping_add(1_442_695_040_888_963_407);
                        result.state.cells.insert(stream, next);
                        ReplayValue::Drawn(next)
                    }
                }
            }
            ReplayInstruction::Authority { enabled } => {
                result.state.authority_enabled = enabled;
                ReplayValue::Written
            }
            ReplayInstruction::Ordered { domain } | ReplayInstruction::External { domain } => {
                let observed = result.state.cells.get(&domain).copied();
                let external = matches!(occurrence.instruction, ReplayInstruction::External { .. });
                let admitted = !external || (result.state.authority_enabled && observed.is_some());
                result.ordered.push(OrderedObservation {
                    occurrence: occurrence.id, domain, observed, external, admitted,
                });
                if external {
                    ReplayValue::Decision(decide(DecisionInputs {
                        empirical: if result.state.authority_enabled {
                            Restriction::Continue
                        } else {
                            Restriction::HoldEffect
                        },
                        exact_disqualifier: false,
                        mandatory_absent: observed.is_none(),
                        contradiction: false,
                    }))
                } else {
                    ReplayValue::Observed(observed)
                }
            }
        };
        result.results.push(OccurrenceResult { occurrence: occurrence.id, value });
    }
    result.results.sort_by_key(|row| row.occurrence);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{OperatorId, OperatorPair, TraceOperationSpec};

    #[test]
    fn a_bad_quotient_fails_on_computed_decisions_even_with_equal_final_state() {
        let domain = DomainId::new(1).unwrap();
        let operator = |value| OperatorId::new(value).unwrap();
        let spec = |id| TraceOperationSpec {
            operator: operator(id),
            positive_read_domains: BTreeSet::new(),
            write_domains: BTreeSet::new(),
            rng_streams: BTreeSet::new(),
            consuming_resources: BTreeSet::new(),
            authority_transition: false,
            ordered_effect: false,
            external_effect: false,
            unknown_dependencies: false,
            inputs: Vec::new(),
        };
        let mut writer = spec(1);
        writer.write_domains.insert(domain);
        let mut reader = spec(2);
        reader.positive_read_domains.insert(domain);
        let occurrences = [
            ReplayOccurrence::new(
                1, RegisteredTraceOperation::try_from_spec(writer).unwrap(),
                ReplayInstruction::Write { domain, value: Some(2) },
            ).unwrap(),
            ReplayOccurrence::new(
                2, RegisteredTraceOperation::try_from_spec(reader).unwrap(),
                ReplayInstruction::Check { domain, expected: Some(1) },
            ).unwrap(),
        ];
        let registry = TraceRelationRegistry::new(BTreeSet::from([
            OperatorPair::new(operator(1), operator(2)).unwrap(),
        ])).unwrap();
        let initial = ReplayState::new(BTreeMap::from([(domain, 1)]), true).unwrap();
        assert_eq!(verify(&registry, &occurrences, &initial, ReplayBudget::default())
            .unwrap().representatives().len(), 2);
        let admission = estimate(&occurrences, &initial).unwrap();
        let (mut matrix, evaluation) = relation(&registry, &occurrences);
        // Deliberately mutate this private relation to test the real comparator.
        // The public API always derives its matrix from the original registry.
        matrix[0][1] = true;
        matrix[1][0] = true;
        let Err(ReplayFailure::Mismatch(mismatch)) =
            verify_relation(&occurrences, &initial, admission, matrix, evaluation)
        else { panic!("computed decision mismatch must fail the quotient") };
        assert_eq!(mismatch.admission.required.schedules, 2);
        assert_eq!(mismatch.progress.compared_schedules, 1);
        assert_eq!(mismatch.progress.unreduced_replays, 2);
        assert_eq!(mismatch.schedule.occurrences(), &[2, 1]);
        assert_eq!(mismatch.representative.occurrences(), &[1, 2]);
        assert_eq!(mismatch.actual.state(), mismatch.expected.state());
        assert_ne!(mismatch.actual.results(), mismatch.expected.results());
    }
}
