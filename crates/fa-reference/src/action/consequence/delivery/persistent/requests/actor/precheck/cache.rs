//! Supervisor-owned memoization of the ORIGINAL exact-policy evaluator.
//! Cache entries are neither disclosure grants nor admission snapshots. They
//! contain no helper judgment, request identity, reservation or effect key.
use super::{Error, FileActorPrecheckPort, FileActorSupervisor, FileOversight,
    FilePrecheckDisclosure, FilePrecheckOutcome, FrozenAction, JournalError,
    OwnerRef, Policy, PrecheckProjection, Scope, Snapshot, Truth};
use crate::action::consequence::delivery::persistent::observed::source::FileSourcePolicy;
use crate::action::consequence::gate::containment::session::policy::Predicate;
use crate::{Judgment, ReadWitness};
use std::{cell::RefCell, collections::VecDeque, fmt, mem::size_of, rc::{Rc, Weak}};

pub const MAX_PRECHECK_CACHE_ENTRIES: usize = 64;
pub const MAX_PRECHECK_CACHE_BYTES: usize = 1_048_576;

/// Retained structural bytes, not allocator overhead or process RSS. All owned
/// policy literals, action bytes, witness values and vector elements are charged.
/// Fixed allocator metadata is additionally bounded by the entry/node limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrecheckCacheLimits { pub entries: usize, pub retained_bytes: usize }
impl Default for PrecheckCacheLimits {
    fn default() -> Self { Self { entries: 16, retained_bytes: 262_144 } }
}
impl PrecheckCacheLimits {
    pub fn validate(self) -> Result<(), Error> {
        if self.entries == 0 || self.retained_bytes == 0 { return Err(Error::InvalidInput); }
        if self.entries > MAX_PRECHECK_CACHE_ENTRIES || self.retained_bytes > MAX_PRECHECK_CACHE_BYTES {
            return Err(Error::Limit);
        }
        Ok(())
    }
}

/// Supervisor-only accounting, never part of an actor response. Counters
/// saturate rather than changing a policy outcome after prolonged operator use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PrecheckCacheStatus {
    pub entries: usize,
    pub retained_bytes: usize,
    pub evaluations: u64,
    pub hits: u64,
    pub invalidations: u64,
    pub evictions: u64,
    pub skipped_retention: u64,
}

pub(super) type Shared = Rc<RefCell<State>>;

/// Bound to one live original owner, including its incarnation. Retain only in
/// the supervisor domain. This cache cannot keep the authority lock alive,
/// disclose itself, or transfer across reopened/equal-looking owners.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::precheck_wire::cache::FilePrecheckCache;
/// fn no_grant(cache: FilePrecheckCache) { cache.disclose_exact_precheck(1, 1, 1); }
/// ```
pub struct FilePrecheckCache { owner: Weak<OwnerRef<FileOversight>>, state: Shared }
impl FilePrecheckCache {
    pub fn status(&self) -> Result<PrecheckCacheStatus, JournalError> {
        let state = self.state.try_borrow().map_err(|_| JournalError::Unavailable)?;
        Ok(state.status())
    }
}
impl fmt::Debug for FilePrecheckCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilePrecheckCache").field("status", &self.status()).finish_non_exhaustive()
    }
}

impl FileActorSupervisor<FileOversight> {
    /// Allocate bounded optional memoization, not disclosure or source renewal.
    pub fn precheck_cache(&self, limits: PrecheckCacheLimits) -> Result<FilePrecheckCache, JournalError> {
        let owner = self.owner.try_borrow().map_err(|_| JournalError::Unavailable)?;
        if owner.host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
        Ok(FilePrecheckCache { owner: Rc::downgrade(&self.owner),
            state: Rc::new(RefCell::new(State::new(limits)?)) })
    }

    /// Issue a NEW grant through the original source/revision checks. Cached
    /// evaluation is optional; the old grant and its ports never become fresh.
    /// Each grant still has its explicit finite query allowance, charged on hits.
    pub fn disclose_exact_precheck_cached(&self, expected_revision: u64, disclosure: u64,
        queries: usize, cache: &FilePrecheckCache,
    ) -> Result<(FileActorPrecheckPort, FilePrecheckDisclosure), JournalError> {
        if !Weak::ptr_eq(&cache.owner, &Rc::downgrade(&self.owner)) {
            return Err(Error::Binding.into());
        }
        self.disclose_exact_precheck_with_cache(expected_revision, disclosure, queries,
            Some(Rc::clone(&cache.state)))
    }
}

pub(super) struct Binding { state: Shared, source: FileSourcePolicy }
impl Binding {
    pub(super) fn new(state: Shared, source: FileSourcePolicy) -> Self { Self { state, source } }
    pub(super) fn evaluate(&self, projection: &PrecheckProjection, action: &FrozenAction)
        -> Result<Option<FilePrecheckOutcome>, Error>
    {
        match self.state.try_borrow_mut() {
            Ok(mut state) => state.evaluate(projection, self.source, action),
            // Cache availability never bypasses or replaces the real evaluator.
            Err(_) => evaluate_uncached(projection, action),
        }
    }
}

pub(super) fn evaluate_uncached(projection: &PrecheckProjection, action: &FrozenAction)
    -> Result<Option<FilePrecheckOutcome>, Error>
{
    let evaluation = projection.policy.evaluate(action, &projection.snapshot)?;
    Ok(outcome(&evaluation))
}
fn outcome(evaluation: &crate::action::consequence::gate::containment::session::policy::Evaluation)
    -> Option<FilePrecheckOutcome>
{
    if evaluation.trace().iter().any(|step| step.result == Truth::Unknown) { return None; }
    Some(if evaluation.certifiable() { FilePrecheckOutcome::PrecheckPassed }
        else { FilePrecheckOutcome::DeniedByPolicy })
}

struct Context {
    policy: Policy,
    source: FileSourcePolicy,
    scope: Scope,
    epoch: u64,
    semantic_epoch: u64,
}
impl Context {
    fn matches(&self, projection: &PrecheckProjection, source: FileSourcePolicy) -> bool {
        self.policy == projection.policy && self.source == source && self.scope == projection.scope
            && self.epoch == projection.epoch && self.semantic_epoch == projection.snapshot.semantic_epoch
    }
}
struct RangeHead { start: u64, end: u64, key: Option<u64> }
struct Entry { action: FrozenAction, judgment: Judgment, heads: Vec<RangeHead>, value: FilePrecheckOutcome, bytes: usize }
impl Entry {
    fn valid_at(&self, snapshot: &Snapshot) -> bool {
        self.judgment.valid_at(snapshot) == Ok(true) && self.heads.iter().all(|head|
            snapshot.values.range(head.start..head.end).next().map(|(key, _)| *key) == head.key)
    }
}

pub(super) struct State {
    limits: PrecheckCacheLimits,
    context: Option<Context>,
    entries: VecDeque<Entry>,
    stats: PrecheckCacheStatus,
}
impl State {
    fn new(limits: PrecheckCacheLimits) -> Result<Self, Error> {
        limits.validate()?;
        let mut entries = VecDeque::new();
        entries.try_reserve_exact(limits.entries).map_err(|_| Error::Limit)?;
        Ok(Self { limits, context: None, entries, stats: PrecheckCacheStatus::default() })
    }
    fn status(&self) -> PrecheckCacheStatus { PrecheckCacheStatus { entries: self.entries.len(), ..self.stats } }
    fn clear_context(&mut self) {
        self.stats.invalidations = self.stats.invalidations.saturating_add(self.entries.len() as u64);
        self.entries.clear(); self.context = None; self.stats.retained_bytes = 0;
    }
    fn evaluate(&mut self, projection: &PrecheckProjection, source: FileSourcePolicy, action: &FrozenAction)
        -> Result<Option<FilePrecheckOutcome>, Error>
    {
        if !self.context.as_ref().is_some_and(|context| context.matches(projection, source)) {
            self.clear_context();
            let Some(bytes) = context_bytes(&projection.policy).filter(|bytes| *bytes <= self.limits.retained_bytes) else {
                self.stats.evaluations = self.stats.evaluations.saturating_add(1);
                self.stats.skipped_retention = self.stats.skipped_retention.saturating_add(1);
                return evaluate_uncached(projection, action);
            };
            self.context = Some(Context { policy: projection.policy.clone(), source,
                scope: projection.scope, epoch: projection.epoch,
                semantic_epoch: projection.snapshot.semantic_epoch });
            self.stats.retained_bytes = bytes;
        }
        // Validate every original positive/negative witness, including unselected
        // Boolean arms. Retire invalid entries rather than resurrecting them if
        // a later snapshot happens to return to their old bytes.
        let mut removed_bytes = 0;
        let mut removed = 0_u64;
        self.entries.retain(|entry| {
            let valid = entry.valid_at(&projection.snapshot);
            if !valid { removed_bytes += entry.bytes; removed += 1; }
            valid
        });
        self.stats.retained_bytes -= removed_bytes;
        self.stats.invalidations = self.stats.invalidations.saturating_add(removed);
        if let Some(entry) = self.entries.iter().find(|entry| entry.action == *action) {
            self.stats.hits = self.stats.hits.saturating_add(1);
            return Ok(Some(entry.value));
        }
        self.stats.evaluations = self.stats.evaluations.saturating_add(1);
        let evaluation = projection.policy.evaluate(action, &projection.snapshot)?;
        let value = outcome(&evaluation);
        let Some(known) = value.filter(|_| projection.snapshot.complete) else {
            self.stats.skipped_retention = self.stats.skipped_retention.saturating_add(1);
            return Ok(value);
        };
        let ranges = projection.policy.nodes().iter().filter(|node| matches!(node, Predicate::EmptyRange { .. })).count();
        let context_bytes = context_bytes(&projection.policy).expect("retained bounded context");
        let Some(bytes) = entry_bytes(action, evaluation.witnesses(), ranges)
            .filter(|bytes| *bytes <= self.limits.retained_bytes - context_bytes) else {
                self.stats.skipped_retention = self.stats.skipped_retention.saturating_add(1);
                return Ok(value);
            };
        let judgment = Judgment::capture(&projection.snapshot, evaluation.witnesses().to_vec())?;
        // Nonempty-range witnesses prove truth, but a newly earlier member could
        // change the original evaluator's retained-byte preflight. Bind its exact
        // selected head too, so a hit cannot hide an original Limit error.
        let heads = projection.policy.nodes().iter().filter_map(|node| match node {
            Predicate::EmptyRange { start, end } => Some(RangeHead { start: *start, end: *end,
                key: projection.snapshot.values.range(*start..*end).next().map(|(key, _)| *key) }),
            _ => None,
        }).collect();
        while self.entries.len() == self.limits.entries
            || self.stats.retained_bytes > self.limits.retained_bytes - bytes {
            let old = self.entries.pop_front().expect("entry alone fits retained context");
            self.stats.retained_bytes -= old.bytes;
            self.stats.evictions = self.stats.evictions.saturating_add(1);
        }
        self.entries.push_back(Entry { action: action.clone(), judgment, heads, value: known, bytes });
        self.stats.retained_bytes += bytes;
        Ok(value)
    }
}

fn context_bytes(policy: &Policy) -> Option<usize> {
    let mut bytes = size_of::<Context>().checked_add(policy.nodes().len().checked_mul(size_of::<Predicate>())?)?;
    for node in policy.nodes() {
        let extra = match node {
            Predicate::PayloadIs(value) | Predicate::ExactValue { value, .. } => value.len(),
            Predicate::All(children) | Predicate::Any(children) => children.len().checked_mul(size_of::<usize>())?,
            _ => 0,
        };
        bytes = bytes.checked_add(extra)?;
    }
    Some(bytes)
}
fn witnesses_bytes(witnesses: &[ReadWitness]) -> Option<usize> {
    let mut bytes = witnesses.len().checked_mul(size_of::<ReadWitness>())?;
    for witness in witnesses {
        if let ReadWitness::Exact { value: Some(value), .. } = witness { bytes = bytes.checked_add(value.len())?; }
    }
    Some(bytes)
}
fn entry_bytes(action: &FrozenAction, witnesses: &[ReadWitness], ranges: usize) -> Option<usize> {
    size_of::<Entry>().checked_add(action.spec().payload.len())?
        .checked_add(witnesses_bytes(&action.spec().required_witnesses)?)?
        .checked_add(witnesses_bytes(witnesses)?)?
        .checked_add(ranges.checked_mul(size_of::<RangeHead>())?)
}

#[cfg(test)]
mod tests;
