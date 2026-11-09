//! Finite pages of one complete native campaign, never caller-selected cases.
//! Every page retains a shared original report and immutable transport metadata.
//! Revocation and original owner health are checked again before cached retries.

use super::{
    ActorError, FileOversight, FilePolicyReplay, FilePolicyReplayBasis, FilePolicyReplayResult,
    FilePolicyReplaySummary, FileSupervisorPolicyReplayPort, Grant, Knowledge, OwnerRef, Policy,
    PolicyReplayReport, ReplayRequest, UnknownReason,
};
use crate::action::consequence::policy_campaign::PolicyReplayCase;
use std::collections::BTreeMap;
use std::fmt;
use std::mem::size_of;
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicU64, Ordering};

/// Original header plus newly admitted case pages across ALL campaigns in a
/// grant. Exact retries are bounded observations of previously admitted pages.
pub const MAX_POLICY_REPLAY_PAGE_QUERIES: usize = 1024;
/// No caller can enlarge a case page or ask the native engine to replay a slice.
pub const MAX_POLICY_REPLAY_PAGE_CASES: usize = 16;

// Map storage and every fixed field, including both weak cursor bindings and
// the original report handle, are charged conservatively. The shared report is
// already charged by the original 8 MiB cache and is never duplicated here.
const PAGE_ENTRY_BYTES: usize = size_of::<(PageKey, FilePolicyReplayPageResult)>() + 128;
pub const MAX_POLICY_REPLAY_PAGE_RETAINED_BYTES: usize =
    size_of::<PageCache>() + MAX_POLICY_REPLAY_PAGE_QUERIES * PAGE_ENTRY_BYTES;

// A scalar identifier cannot be reused by another grant or replacement mount.
// It is transport correlation only; reading still requires the sealed cursor.
static NEXT_CURSOR_ID: AtomicU64 = AtomicU64::new(1);

fn next_cursor_id(counter: &AtomicU64) -> Option<u64> {
    let mut current = counter.load(Ordering::Relaxed);
    loop {
        let next = current.checked_add(1).filter(|_| current != 0)?;
        match counter.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return Some(current),
            Err(observed) => current = observed,
        }
    }
}

/// One native continuation of one original campaign and capture. Cloning this
/// handle keeps neither its grant, report nor authority owner alive. No numeric
/// identifier or caller-supplied offset can construct a continuation.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FilePolicyReplayCursor;
/// fn forge() { let _ = FilePolicyReplayCursor::new(1); }
/// ```
#[derive(Clone)]
pub struct FilePolicyReplayCursor {
    id: u64,
    campaign: u64,
    offset: usize,
    owner: Weak<OwnerRef<FileOversight>>,
    grant: Weak<Grant>,
    capture: Weak<PolicyReplayReport>,
}

impl FilePolicyReplayCursor {
    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn campaign(&self) -> u64 {
        self.campaign
    }
}

impl fmt::Debug for FilePolicyReplayCursor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FilePolicyReplayCursor { .. }")
    }
}

impl PartialEq for FilePolicyReplayCursor {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.campaign == other.campaign
            && self.offset == other.offset
            && Weak::ptr_eq(&self.owner, &other.owner)
            && Weak::ptr_eq(&self.grant, &other.grant)
            && Weak::ptr_eq(&self.capture, &other.capture)
    }
}
impl Eq for FilePolicyReplayCursor {}

/// Sealed page of an already complete comparison. The first page is a header
/// with no cases. Subsequent pages expose at most sixteen whole original cases.
/// Enumeration completion describes transport coverage, never policy approval.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilePolicyReplayPage {
    original: FilePolicyReplay,
    request_cursor: Option<u64>,
    offset: usize,
    end: usize,
    continuation: Option<FilePolicyReplayCursor>,
    remaining_pages: usize,
}

impl FilePolicyReplayPage {
    pub fn original(&self) -> &FilePolicyReplay {
        &self.original
    }
    pub fn basis(&self) -> FilePolicyReplayBasis {
        self.original.basis()
    }
    pub fn summary(&self) -> FilePolicyReplaySummary {
        self.original.summary()
    }
    pub fn is_header(&self) -> bool {
        self.request_cursor.is_none()
    }
    pub fn request_cursor(&self) -> Option<u64> {
        self.request_cursor
    }
    pub fn offset(&self) -> usize {
        self.offset
    }
    pub fn cases(&self) -> &[PolicyReplayCase] {
        &self.original.report().cases()[self.offset..self.end]
    }
    /// None also covers exhausted page allowance. Check enumeration_complete
    /// before claiming that every case of the comparison has been delivered.
    pub fn continuation(&self) -> Option<&FilePolicyReplayCursor> {
        self.continuation.as_ref()
    }
    pub fn enumeration_complete(&self) -> bool {
        self.end == self.original.summary().case_count
    }
    /// Historical shared allowance after this page was first admitted. Exact
    /// retries preserve it; it is not a reservation for another observation.
    pub fn remaining_pages_at_capture(&self) -> usize {
        self.remaining_pages
    }
}

pub type FilePolicyReplayPageObservation = Knowledge<FilePolicyReplayPage, FilePolicyReplayBasis>;
pub type FilePolicyReplayPageResult = Result<FilePolicyReplayPageObservation, ActorError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum PageKey {
    Header(u64),
    Continuation(u64),
}

pub(super) struct PageCache {
    remaining: usize,
    retained_bytes: usize,
    responses: BTreeMap<PageKey, FilePolicyReplayPageResult>,
}

impl PageCache {
    pub(super) fn new(remaining: usize) -> Self {
        Self {
            remaining,
            retained_bytes: size_of::<Self>(),
            responses: BTreeMap::new(),
        }
    }
    pub(super) fn remaining(&self) -> usize {
        self.remaining
    }
    pub(super) fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    fn admit(&mut self, key: PageKey) -> Result<(), ActorError> {
        if self.remaining == 0 || self.responses.len() >= MAX_POLICY_REPLAY_PAGE_QUERIES {
            return Err(ActorError::Capacity);
        }
        let retained = self
            .retained_bytes
            .checked_add(PAGE_ENTRY_BYTES)
            .filter(|bytes| *bytes <= MAX_POLICY_REPLAY_PAGE_RETAINED_BYTES)
            .ok_or(ActorError::Capacity)?;
        // A caught unwind cannot repeat a paid page operation. The original
        // campaign has its own independently charged exact-request tombstone.
        self.remaining -= 1;
        self.retained_bytes = retained;
        self.responses.insert(key, unknown());
        Ok(())
    }
    fn finish(
        &mut self,
        key: PageKey,
        result: FilePolicyReplayPageResult,
    ) -> FilePolicyReplayPageResult {
        *self.responses.get_mut(&key).expect("admitted page") = result.clone();
        result
    }
}

impl FileSupervisorPolicyReplayPort {
    /// Capture or recover the complete original campaign, returning its header
    /// only. A newly admitted header spends one page and, for a new campaign,
    /// one native query. Native stale/unknown/capacity results remain unchanged
    /// and their exact header retries spend neither allowance again.
    ///
    /// Disabled paging, malformed/conflicting requests and preflight capacity
    /// refusal cannot spend a campaign. A header with no continuation may still
    /// be explicitly incomplete when the selected page allowance is exhausted.
    pub fn replay_candidate_policy_page(
        &self,
        query: u64,
        expected_policy_generation: u64,
        candidate: Policy,
    ) -> FilePolicyReplayPageResult {
        if query == 0
            || expected_policy_generation == 0
            || candidate.generation() <= expected_policy_generation
        {
            return Err(ActorError::MalformedProposal);
        }
        let Some(grant) = self.grant.upgrade() else {
            return withheld();
        };
        let Some(pages) = &grant.pages else {
            return Err(ActorError::Withheld);
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
        let Ok(mut pages) = pages.try_borrow_mut() else {
            return unknown();
        };
        let key = PageKey::Header(query);
        if let Some(cached) = pages.responses.get(&key) {
            return cached.clone();
        }
        pages.admit(key)?;
        let native = self.replay_with_cache(
            &owner.host,
            &grant,
            &mut cache,
            ReplayRequest {
                query,
                expected_policy_generation,
                candidate,
            },
            admission,
        );
        let result = self.header(native, pages.remaining);
        pages.finish(key, result)
    }

    /// Read the next fixed slice of the cursor's original captured report. All
    /// grants and facades share both the allowance and exact page-result cache.
    /// A healthy owner may advance without recapturing or re-evaluating cases.
    /// Revocation, owner loss, conflicting borrow and storage failure suppress
    /// cached disclosure; a new owner cannot adopt an old cursor or namespace.
    pub fn continue_policy_replay(
        &self,
        query: u64,
        cursor: &FilePolicyReplayCursor,
    ) -> FilePolicyReplayPageResult {
        if query == 0 {
            return Err(ActorError::MalformedProposal);
        }
        let Some(grant) = self.grant.upgrade() else {
            return withheld();
        };
        let Some(pages) = &grant.pages else {
            return Err(ActorError::Withheld);
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
        if cursor.campaign != query
            || cursor.id == 0
            || !Weak::ptr_eq(&cursor.grant, &self.grant)
            || !Weak::ptr_eq(&cursor.owner, &self.owner)
        {
            return Err(ActorError::IdempotencyConflict);
        }
        let Ok(cache) = grant.cache.try_borrow() else {
            return unknown();
        };
        let Some(Ok(Knowledge::Known { value, .. })) =
            cache.campaigns.get(&query).map(|cached| &cached.result)
        else {
            return Err(ActorError::IdempotencyConflict);
        };
        if !Weak::ptr_eq(&cursor.capture, &Rc::downgrade(&value.report))
            || cursor.offset >= value.summary().case_count
        {
            return Err(ActorError::IdempotencyConflict);
        }
        let Ok(mut pages) = pages.try_borrow_mut() else {
            return unknown();
        };
        let key = PageKey::Continuation(cursor.id);
        if let Some(cached) = pages.responses.get(&key) {
            return cached.clone();
        }
        pages.admit(key)?;
        let end = cursor
            .offset
            .saturating_add(MAX_POLICY_REPLAY_PAGE_CASES)
            .min(value.summary().case_count);
        let result = self
            .page(
                value.clone(),
                Some(cursor.id),
                cursor.offset,
                end,
                pages.remaining,
            )
            .map(|page| Knowledge::Known {
                basis: page.basis(),
                value: page,
            });
        pages.finish(key, result)
    }

    fn header(
        &self,
        result: FilePolicyReplayResult,
        remaining: usize,
    ) -> FilePolicyReplayPageResult {
        match result {
            Ok(Knowledge::Known { value, basis }) => self
                .page(value, None, 0, 0, remaining)
                .map(|page| Knowledge::Known { value: page, basis }),
            Ok(Knowledge::Pending { request }) => Ok(Knowledge::Pending { request }),
            Ok(Knowledge::Unknown { reason }) => Ok(Knowledge::Unknown { reason }),
            Ok(Knowledge::Withheld { authority_required }) => {
                Ok(Knowledge::Withheld { authority_required })
            }
            Ok(Knowledge::Stale {
                generation,
                current,
            }) => Ok(Knowledge::Stale {
                generation,
                current,
            }),
            Ok(Knowledge::Absent {
                closed_domain,
                frontier,
            }) => Ok(Knowledge::Absent {
                closed_domain,
                frontier,
            }),
            Err(error) => Err(error),
        }
    }

    fn page(
        &self,
        original: FilePolicyReplay,
        request_cursor: Option<u64>,
        offset: usize,
        end: usize,
        remaining_pages: usize,
    ) -> Result<FilePolicyReplayPage, ActorError> {
        let continuation = if end < original.summary().case_count && remaining_pages != 0 {
            Some(FilePolicyReplayCursor {
                id: next_cursor_id(&NEXT_CURSOR_ID).ok_or(ActorError::Capacity)?,
                campaign: original.basis().query,
                offset: end,
                owner: self.owner.clone(),
                grant: self.grant.clone(),
                capture: Rc::downgrade(&original.report),
            })
        } else {
            None
        };
        Ok(FilePolicyReplayPage {
            original,
            request_cursor,
            offset,
            end,
            continuation,
            remaining_pages,
        })
    }
}

fn unknown() -> FilePolicyReplayPageResult {
    Ok(Knowledge::Unknown {
        reason: UnknownReason::ControllerUnavailable,
    })
}

fn withheld() -> FilePolicyReplayPageResult {
    Ok(Knowledge::Withheld {
        authority_required: "supervisor_policy_replay",
    })
}

#[cfg(test)]
#[path = "pages/tests.rs"]
mod tests;
