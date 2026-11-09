//! Separately custodied policy experiments over the ORIGINAL retained corpus.
//! Exact campaign retries retain their first result and acknowledged cut. No
//! source, clock, journal, helper, live campaign or promotion authority is used.

mod pages;
mod retention;
pub use pages::{
    FilePolicyReplayCursor, FilePolicyReplayPage, FilePolicyReplayPageObservation,
    FilePolicyReplayPageResult, MAX_POLICY_REPLAY_PAGE_CASES, MAX_POLICY_REPLAY_PAGE_QUERIES,
    MAX_POLICY_REPLAY_PAGE_RETAINED_BYTES,
};

use super::{
    ActorError, FileActorPort, FileActorSupervisor, JournalError, Knowledge, OwnerRef,
    UnknownReason,
};
use crate::Error;
use crate::action::consequence::delivery::persistent::observed::FileOversight;
use crate::action::consequence::gate::containment::session::policy::Policy;
use crate::action::consequence::policy_campaign::{PolicyDelta, PolicyReplayReport, ReplayLimits};
use crate::action::{ElapsedTick, Scope};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::fmt;
use std::rc::{Rc, Weak};

/// Distinct admitted campaign identities, shared across every port clone.
/// An exact retry does not spend another operation or recapture a newer corpus.
pub const MAX_POLICY_REPLAY_QUERIES: usize = 64;
/// Logical retained-data charge, including policies, cases, traces, action and
/// witness occurrences and fixed map metadata. Not a peak allocator measure.
pub const MAX_POLICY_REPLAY_RETAINED_BYTES: usize = 8 * 1_048_576;

/// The original acknowledged owner cut at which this campaign first ran.
/// Journal revision distinguishes added exact denials even when neither the
/// control sequence nor policy generation changes. This is not a promotion key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilePolicyReplayBasis {
    pub scope: Scope,
    pub query: u64,
    pub journal_revision: u64,
    pub control_sequence: u64,
    pub policy_epoch: u64,
    pub policy_generation: u64,
    /// A previously recorded observation, never the current wall/elapsed clock.
    pub recorded_at: Option<ElapsedTick>,
}

/// Complete-corpus totals computed once at the original native capture. Pages
/// share these historical values; a page is not a newly selected policy corpus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilePolicyReplaySummary {
    pub case_count: usize,
    pub input_bytes: usize,
    pub unchanged: usize,
    pub newly_blocked: usize,
    pub newly_reviewable: usize,
    pub requires_shadow_cases: usize,
    pub requires_shadow: bool,
}

impl FilePolicyReplaySummary {
    fn capture(report: &PolicyReplayReport) -> Self {
        let mut summary = Self {
            case_count: report.cases().len(),
            input_bytes: report.input_bytes(),
            unchanged: 0,
            newly_blocked: 0,
            newly_reviewable: 0,
            requires_shadow_cases: 0,
            requires_shadow: false,
        };
        for case in report.cases() {
            match case.delta() {
                PolicyDelta::Unchanged => summary.unchanged += 1,
                PolicyDelta::NewlyBlocked => summary.newly_blocked += 1,
                PolicyDelta::NewlyReviewable => summary.newly_reviewable += 1,
                PolicyDelta::RequiresShadow => summary.requires_shadow_cases += 1,
            }
        }
        summary.requires_shadow = summary.requires_shadow_cases != 0;
        summary
    }
}

/// Complete original exact-policy comparison, sealed to its native cut.
/// This privileged Rust grant discloses the original policies and retained
/// case evidence. A transport may project less but must not invent a complete
/// report from a subset. No governor, campaign review or promotion key exists.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::{FilePermit,
///     requests::actor::FilePolicyReplay};
/// fn no_effect(value: FilePolicyReplay) -> FilePermit { value }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilePolicyReplay {
    basis: FilePolicyReplayBasis,
    report: Rc<PolicyReplayReport>,
    summary: FilePolicyReplaySummary,
    limits: ReplayLimits,
    remaining_queries: usize,
}

impl FilePolicyReplay {
    pub fn basis(&self) -> FilePolicyReplayBasis {
        self.basis
    }
    pub fn report(&self) -> &PolicyReplayReport {
        &self.report
    }
    pub fn summary(&self) -> FilePolicyReplaySummary {
        self.summary
    }
    pub fn limits(&self) -> ReplayLimits {
        self.limits
    }
    /// The shared allowance after this campaign first ran. Cached retries keep
    /// this historical observation; it is not a reservation for later work.
    pub fn remaining_queries(&self) -> usize {
        self.remaining_queries
    }
}

pub type FilePolicyReplayObservation = Knowledge<FilePolicyReplay, FilePolicyReplayBasis>;
pub type FilePolicyReplayResult = Result<FilePolicyReplayObservation, ActorError>;

struct CachedReplay {
    expected_policy_generation: u64,
    candidate: Policy,
    result: FilePolicyReplayResult,
}

struct ReplayRequest {
    query: u64,
    expected_policy_generation: u64,
    candidate: Policy,
}

#[derive(Default)]
struct Cache {
    retained_bytes: usize,
    campaigns: BTreeMap<u64, CachedReplay>,
}

struct Grant {
    remaining: Cell<usize>,
    limits: ReplayLimits,
    cache: RefCell<Cache>,
    pages: Option<RefCell<pages::PageCache>>,
}

/// Noncopyable governance disclosure custody for a finite set of experiments.
/// Revocation withdraws all port clones and their cached results without
/// retaining the original owner or its exclusive authority lock.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorPolicyReplayGrant;
/// fn no_copy(grant: FileSupervisorPolicyReplayGrant) { grant.clone(); }
/// ```
#[must_use = "retain this custodian for the explicitly granted policy experiments"]
pub struct FileSupervisorPolicyReplayGrant {
    grant: Rc<Grant>,
}

impl FileSupervisorPolicyReplayGrant {
    pub fn remaining_queries(&self) -> usize {
        self.grant.remaining.get()
    }
    pub fn replay_limits(&self) -> ReplayLimits {
        self.grant.limits
    }
    pub fn retained_bytes(&self) -> usize {
        self.grant.cache.borrow().retained_bytes
    }
    /// Remaining independently granted page observations. Exact retries keep
    /// their original observation and do not spend another page.
    pub fn remaining_pages(&self) -> usize {
        self.grant
            .pages
            .as_ref()
            .map_or(0, |pages| pages.borrow().remaining())
    }
    pub fn pages_enabled(&self) -> bool {
        self.grant.pages.is_some()
    }
    /// Logical page metadata only. All pages share the original report already
    /// charged by retained_bytes; no page retains a second case corpus.
    pub fn retained_page_bytes(&self) -> usize {
        self.grant
            .pages
            .as_ref()
            .map_or(0, |pages| pages.borrow().retained_bytes())
    }
    pub fn revoke(self) {
        drop(self);
    }
}

impl fmt::Debug for FileSupervisorPolicyReplayGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSupervisorPolicyReplayGrant")
            .field("remaining_queries", &self.remaining_queries())
            .field("replay_limits", &self.replay_limits())
            .field("remaining_pages", &self.remaining_pages())
            .finish_non_exhaustive()
    }
}

/// Weak experiment access to one original owner. Neither an actor nor an
/// inspection/explanation/precheck port can issue this stronger disclosure.
/// The caller chooses a candidate, never the historical cases or source data.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::{observed::FileOversight,
///     requests::actor::FileActorPort};
/// fn no_self_grant(actor: FileActorPort<FileOversight>) { actor.policy_replay_port(1); }
/// ```
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorPolicyReplayPort;
/// fn no_owner(port: FileSupervisorPolicyReplayPort) { port.host_mut(); }
/// ```
#[derive(Clone)]
pub struct FileSupervisorPolicyReplayPort {
    owner: Weak<OwnerRef<FileOversight>>,
    grant: Weak<Grant>,
    scope: Scope,
}

impl fmt::Debug for FileSupervisorPolicyReplayPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FileSupervisorPolicyReplayPort { .. }")
    }
}

impl FileActorSupervisor<FileOversight> {
    /// Grant complete-corpus exact-policy experiments with immutable work caps.
    /// Provisioning reads no source or clock, captures no report, consumes no
    /// admission snapshot and changes no journal or governance authority.
    pub fn policy_replay_port(
        &self,
        queries: usize,
        limits: ReplayLimits,
    ) -> Result<
        (
            FileSupervisorPolicyReplayPort,
            FileSupervisorPolicyReplayGrant,
        ),
        JournalError,
    > {
        self.policy_replay_port_with_page_allowance(queries, limits, None)
    }

    /// Separately opt into finite transport pages of the original complete
    /// campaigns. The old constructor grants no paging. Neither allowance can
    /// replenish the other; every clone and facade shares the same custody.
    pub fn policy_replay_port_with_pages(
        &self,
        queries: usize,
        limits: ReplayLimits,
        page_queries: usize,
    ) -> Result<
        (
            FileSupervisorPolicyReplayPort,
            FileSupervisorPolicyReplayGrant,
        ),
        JournalError,
    > {
        if page_queries == 0 {
            return Err(Error::InvalidInput.into());
        }
        if page_queries > MAX_POLICY_REPLAY_PAGE_QUERIES {
            return Err(Error::Limit.into());
        }
        self.policy_replay_port_with_page_allowance(queries, limits, Some(page_queries))
    }

    fn policy_replay_port_with_page_allowance(
        &self,
        queries: usize,
        limits: ReplayLimits,
        page_queries: Option<usize>,
    ) -> Result<
        (
            FileSupervisorPolicyReplayPort,
            FileSupervisorPolicyReplayGrant,
        ),
        JournalError,
    > {
        if queries == 0 {
            return Err(Error::InvalidInput.into());
        }
        if queries > MAX_POLICY_REPLAY_QUERIES {
            return Err(Error::Limit.into());
        }
        limits.validate()?;
        let owner = self
            .owner
            .try_borrow()
            .map_err(|_| JournalError::Unavailable)?;
        if owner.host.storage_failure().is_some() {
            return Err(JournalError::Unavailable);
        }
        let grant = Rc::new(Grant {
            remaining: Cell::new(queries),
            limits,
            cache: RefCell::new(Cache::default()),
            pages: page_queries.map(|queries| RefCell::new(pages::PageCache::new(queries))),
        });
        Ok((
            FileSupervisorPolicyReplayPort {
                owner: Rc::downgrade(&self.owner),
                grant: Rc::downgrade(&grant),
                scope: owner.host.actor_explanation_scope(),
            },
            FileSupervisorPolicyReplayGrant { grant },
        ))
    }
}

impl FileSupervisorPolicyReplayPort {
    pub(crate) fn belongs_to(&self, actor: &FileActorPort<FileOversight>) -> bool {
        Weak::ptr_eq(&self.owner, &actor.owner) && self.scope == actor.scope
    }

    /// Run one keyed experiment on ALL current-policy proposals and applied
    /// reviews retained by the original controller. Unsupported candidate reads
    /// keep its original RequiresShadow result. There is no numerical replay,
    /// new helper judgment, source acquisition, authority event or policy change.
    ///
    /// Each new admitted query spends once, including stale or unavailable
    /// corpora and complete-report capacity failures. Its bounded exact request
    /// and result stay shared by every clone. An exact retry returns that first
    /// historical result even after the owner advances; conflicting content
    /// refuses. Custody and healthy owner availability are checked before cached
    /// results. Dropping/reopening the owner cannot transfer an old grant.
    ///
    /// A request that cannot fit the remaining request/cache admission bound
    /// does not run. A computed report that exceeds retained capacity is discarded
    /// whole and retains a charged Capacity result, never a selective prefix.
    pub fn replay_candidate_policy(
        &self,
        query: u64,
        expected_policy_generation: u64,
        candidate: Policy,
    ) -> FilePolicyReplayResult {
        if query == 0
            || expected_policy_generation == 0
            || candidate.generation() <= expected_policy_generation
        {
            return Err(ActorError::MalformedProposal);
        }
        let Some(grant) = self.grant.upgrade() else {
            return Ok(Knowledge::Withheld {
                authority_required: "supervisor_policy_replay",
            });
        };
        let Some(owner) = self.owner.upgrade() else {
            return unknown();
        };
        let Ok(owner) = owner.try_borrow() else {
            return unknown();
        };
        if owner.host.storage_failure().is_some()
            || owner.host.actor_explanation_scope() != self.scope
        {
            return unknown();
        }
        let Ok(mut cache) = grant.cache.try_borrow_mut() else {
            return unknown();
        };
        let admission = Self::admission(
            &grant,
            &cache,
            query,
            expected_policy_generation,
            &candidate,
        )?;
        self.replay_with_cache(
            &owner.host,
            &grant,
            &mut cache,
            ReplayRequest {
                query,
                expected_policy_generation,
                candidate,
            },
            admission,
        )
    }

    /// Preflight both first-page and legacy requests before either optional
    /// page work or native campaign work is charged. Conflicts always win over
    /// exhausted paging and cannot reuse a header from different policy data.
    fn admission(
        grant: &Grant,
        cache: &Cache,
        query: u64,
        expected_policy_generation: u64,
        candidate: &Policy,
    ) -> Result<Option<usize>, ActorError> {
        if let Some(cached) = cache.campaigns.get(&query) {
            return if cached.expected_policy_generation == expected_policy_generation
                && &cached.candidate == candidate
            {
                Ok(None)
            } else {
                Err(ActorError::IdempotencyConflict)
            };
        }
        let remaining = grant.remaining.get();
        if remaining == 0 || cache.campaigns.len() >= MAX_POLICY_REPLAY_QUERIES {
            return Err(ActorError::Capacity);
        }
        let request_bytes = retention::request_bytes(candidate).ok_or(ActorError::Capacity)?;
        let retained_bytes = cache
            .retained_bytes
            .checked_add(request_bytes)
            .filter(|bytes| *bytes <= MAX_POLICY_REPLAY_RETAINED_BYTES)
            .ok_or(ActorError::Capacity)?;
        Ok(Some(retained_bytes))
    }

    fn replay_with_cache(
        &self,
        host: &FileOversight,
        grant: &Grant,
        cache: &mut Cache,
        request: ReplayRequest,
        admission: Option<usize>,
    ) -> FilePolicyReplayResult {
        let ReplayRequest {
            query,
            expected_policy_generation,
            candidate,
        } = request;
        let Some(retained_bytes) = admission else {
            return cache
                .campaigns
                .get(&query)
                .expect("checked campaign")
                .result
                .clone();
        };
        let remaining = grant.remaining.get();
        // Retain normalized bounded content, not a caller's spare Vec capacity.
        // The initial Unknown tombstone makes an admitted operation one-shot even
        // if the embedding host catches an unwind from the original evaluator.
        let candidate = candidate.clone();
        grant.remaining.set(remaining - 1);
        cache.retained_bytes = retained_bytes;
        cache.campaigns.insert(
            query,
            CachedReplay {
                expected_policy_generation,
                candidate: candidate.clone(),
                result: unknown(),
            },
        );
        let result = self.run(
            host,
            query,
            expected_policy_generation,
            candidate,
            grant.limits,
            remaining - 1,
        );
        let result = match result {
            Ok(Knowledge::Known { value, basis }) => {
                match retention::report_bytes(value.report())
                    .and_then(|bytes| cache.retained_bytes.checked_add(bytes))
                    .filter(|bytes| *bytes <= MAX_POLICY_REPLAY_RETAINED_BYTES)
                {
                    Some(bytes) => {
                        cache.retained_bytes = bytes;
                        Ok(Knowledge::Known { value, basis })
                    }
                    None => Err(ActorError::Capacity),
                }
            }
            other => other,
        };
        cache
            .campaigns
            .get_mut(&query)
            .expect("admitted campaign")
            .result = result.clone();
        result
    }

    fn run(
        &self,
        host: &FileOversight,
        query: u64,
        expected_policy_generation: u64,
        candidate: Policy,
        limits: ReplayLimits,
        remaining_queries: usize,
    ) -> FilePolicyReplayResult {
        let Ok(policy) = host.current_policy() else {
            return unknown();
        };
        let policy_generation = policy.generation();
        if policy_generation != expected_policy_generation {
            return Ok(Knowledge::Stale {
                generation: expected_policy_generation,
                current: policy_generation,
            });
        }
        let report = match host.replay_candidate_policy(candidate, limits) {
            Ok(report) => report,
            Err(JournalError::Contract(Error::Limit | Error::Overflow)) => {
                return Err(ActorError::Capacity);
            }
            Err(_) => return unknown(),
        };
        if report.previous_policy() != policy {
            return unknown();
        }
        let snapshot = host.inspect();
        let basis = FilePolicyReplayBasis {
            scope: self.scope,
            query,
            journal_revision: snapshot.revision,
            control_sequence: snapshot.control.sequence,
            policy_epoch: snapshot.control.ledger.epoch,
            policy_generation,
            recorded_at: snapshot.control.ledger.elapsed,
        };
        Ok(Knowledge::Known {
            value: FilePolicyReplay {
                basis,
                summary: FilePolicyReplaySummary::capture(&report),
                report: Rc::new(report),
                limits,
                remaining_queries,
            },
            basis,
        })
    }
}

fn unknown() -> FilePolicyReplayResult {
    Ok(Knowledge::Unknown {
        reason: UnknownReason::ControllerUnavailable,
    })
}

#[cfg(test)]
mod tests;
