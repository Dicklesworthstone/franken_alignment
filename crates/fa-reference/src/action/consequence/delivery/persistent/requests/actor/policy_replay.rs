//! Separately custodied policy experiments over the ORIGINAL retained corpus.
//! Exact campaign retries retain their first result and acknowledged cut. No
//! source, clock, journal, helper, live campaign or promotion authority is used.

mod retention;

use super::{
    ActorError, FileActorPort, FileActorSupervisor, JournalError, Knowledge, OwnerRef,
    UnknownReason,
};
use crate::Error;
use crate::action::consequence::delivery::persistent::observed::FileOversight;
use crate::action::consequence::gate::containment::session::policy::Policy;
use crate::action::consequence::policy_campaign::{PolicyReplayReport, ReplayLimits};
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
    report: PolicyReplayReport,
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

#[derive(Default)]
struct Cache {
    retained_bytes: usize,
    campaigns: BTreeMap<u64, CachedReplay>,
}

struct Grant {
    remaining: Cell<usize>,
    limits: ReplayLimits,
    cache: RefCell<Cache>,
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
    pub fn revoke(self) {
        drop(self);
    }
}

impl fmt::Debug for FileSupervisorPolicyReplayGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSupervisorPolicyReplayGrant")
            .field("remaining_queries", &self.remaining_queries())
            .field("replay_limits", &self.replay_limits())
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
        if let Some(cached) = cache.campaigns.get(&query) {
            return if cached.expected_policy_generation == expected_policy_generation
                && cached.candidate == candidate
            {
                cached.result.clone()
            } else {
                Err(ActorError::IdempotencyConflict)
            };
        }
        let remaining = grant.remaining.get();
        if remaining == 0 || cache.campaigns.len() >= MAX_POLICY_REPLAY_QUERIES {
            return Err(ActorError::Capacity);
        }
        let request_bytes = retention::request_bytes(&candidate).ok_or(ActorError::Capacity)?;
        let retained_bytes = cache
            .retained_bytes
            .checked_add(request_bytes)
            .filter(|bytes| *bytes <= MAX_POLICY_REPLAY_RETAINED_BYTES)
            .ok_or(ActorError::Capacity)?;
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
            &owner.host,
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
                report,
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
