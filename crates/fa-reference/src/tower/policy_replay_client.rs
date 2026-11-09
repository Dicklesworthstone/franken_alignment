//! Bounded structural validation of one original paged policy comparison.
//!
//! This reader authenticates no peer and creates no native authority. Its
//! caller must authenticate transport before supplying bytes. Accepted pages
//! remain historical observations; only complete, contiguous coverage permits
//! successful enumeration. No payload or undisclosed witness is reconstructed.

mod cases;
mod envelope;
mod syntax;

use super::{MAX_REQUEST_STRING_BYTES, ReadArguments, TowerRequest, Verb, encode_request};
use crate::action::consequence::delivery::persistent::requests::actor::{
    MAX_POLICY_REPLAY_PAGE_CASES, MAX_POLICY_REPLAY_PAGE_QUERIES, MAX_POLICY_REPLAY_QUERIES,
};
use crate::action::consequence::gate::containment::session::policy::Policy;
use crate::action::consequence::policy_campaign::{
    MAX_REPLAY_CASES, MAX_REPLAY_INPUT_BYTES, ReplayCaseId,
};
use crate::action::{Purpose, Scope};
use crate::address_resolver::Noun;
use crate::strict_json::{self, Json, Limits};
use std::collections::BTreeSet;
use std::fmt;
use syntax::{array, boolean, ensure, natural, number, object, optional_number, text};

/// One header or at most sixteen complete cases, with the existing wire bounds.
pub const POLICY_REPLAY_PAGE_JSON_LIMITS: Limits = Limits {
    max_bytes: super::publication::MAX_PUBLICATION_RESPONSE_BYTES,
    max_depth: 16,
    max_items: 65_536,
    max_string_bytes: MAX_REQUEST_STRING_BYTES,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyReplayClientError {
    InvalidRequest,
    InvalidResponse,
    PageLimit,
    Finished,
}

impl fmt::Display for PolicyReplayClientError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str(match self {
            Self::InvalidRequest => "invalid policy replay enumeration request",
            Self::InvalidResponse => "invalid or inconsistent original policy replay page",
            Self::PageLimit => "policy replay enumeration page allowance exhausted",
            Self::Finished => "policy replay enumeration already completed or stopped",
        })
    }
}

impl std::error::Error for PolicyReplayClientError {}

type Result<T> = std::result::Result<T, PolicyReplayClientError>;

/// Progress after a fully validated original response. Incomplete with no
/// cursor means stop, never capture again. A local page cap may leave a native
/// cursor visible, but does not authorize this reader to follow it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PolicyReplayProgress {
    pub next_cursor: Option<u64>,
    pub complete: bool,
    pub cases_seen: usize,
    pub pages_seen: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Counts {
    unchanged: usize,
    blocked: usize,
    reviewable: usize,
    shadow: usize,
}

impl Counts {
    fn total(self) -> Option<usize> {
        self.unchanged
            .checked_add(self.blocked)?
            .checked_add(self.reviewable)?
            .checked_add(self.shadow)
    }

    fn within(self, total: Self) -> bool {
        self.unchanged <= total.unchanged
            && self.blocked <= total.blocked
            && self.reviewable <= total.reviewable
            && self.shadow <= total.shadow
    }
}

#[derive(Debug)]
struct Summary {
    total: usize,
    input_bytes: usize,
    counts: Counts,
}

#[derive(Debug)]
struct Header {
    basis: Json,
    summary: Json,
    previous: Policy,
    totals: Summary,
    control_sequence: u64,
    policy_epoch: u64,
}

/// Retains two bounded policies, one basis/summary, at most 5,120 case IDs and
/// 1,024 cursor IDs. It never stores case payloads, traces or all response bytes.
#[derive(Debug)]
pub struct PolicyReplayCapture {
    scope: Scope,
    initial: TowerRequest,
    campaign: u64,
    expected_generation: u64,
    max_pages: usize,
    header: Option<Header>,
    progress: PolicyReplayProgress,
    remaining_pages: Option<usize>,
    ids: BTreeSet<ReplayCaseId>,
    cursors: BTreeSet<u64>,
    order: cases::Order,
    counts: Counts,
    payload_bytes: usize,
    stopped: bool,
}

impl PolicyReplayCapture {
    pub fn new(scope: Scope, initial: &TowerRequest, max_pages: usize) -> Result<Self> {
        let ReadArguments::PolicyReplayPaged {
            campaign,
            expected_policy_generation,
            candidate,
        } = &initial.arguments
        else {
            return Err(PolicyReplayClientError::InvalidRequest);
        };
        if scope.purpose != Purpose::Effect
            || [
                scope.tenant,
                scope.principal,
                scope.run,
                scope.branch,
                scope.authority,
            ]
            .contains(&0)
            || !(1..=MAX_POLICY_REPLAY_PAGE_QUERIES).contains(&max_pages)
            || initial.verb != Verb::PolicyReplay
            || initial.address.kind != Noun::Run
            || initial.address.tenant != scope.tenant.to_string()
            || initial.address.id != scope.run.to_string()
            || initial
                .address
                .generation
                .is_none_or(|generation| generation == 0)
            || candidate.generation() <= *expected_policy_generation
            || encode_request(initial).is_err()
        {
            return Err(PolicyReplayClientError::InvalidRequest);
        }
        Ok(Self {
            scope,
            initial: initial.clone(),
            campaign: *campaign,
            expected_generation: *expected_policy_generation,
            max_pages,
            header: None,
            progress: PolicyReplayProgress {
                next_cursor: None,
                complete: false,
                cases_seen: 0,
                pages_seen: 0,
            },
            remaining_pages: None,
            ids: BTreeSet::new(),
            cursors: BTreeSet::new(),
            order: cases::Order::default(),
            counts: Counts::default(),
            payload_bytes: 0,
            stopped: false,
        })
    }

    pub fn progress(&self) -> PolicyReplayProgress {
        self.progress
    }

    /// Invalid input leaves all accepted progress unchanged. A valid native
    /// refusal or unavailable observation may be emitted, but ends enumeration.
    pub fn accept(
        &mut self,
        request: &TowerRequest,
        response: &[u8],
    ) -> Result<PolicyReplayProgress> {
        if self.stopped || self.progress.complete {
            return Err(PolicyReplayClientError::Finished);
        }
        if self.progress.pages_seen == self.max_pages {
            return Err(PolicyReplayClientError::PageLimit);
        }
        self.check_request(request)?;
        let parsed = strict_json::parse(response, POLICY_REPLAY_PAGE_JSON_LIMITS)
            .map_err(|_| PolicyReplayClientError::InvalidResponse)?;
        let root = object(&parsed, &["envelope", "result"])?;
        let envelope = envelope::check(&root["envelope"], request, response.len())?;
        let result = object(&root["result"], &["kind", "value"])?;
        if text(&result["kind"])? == "refused" {
            envelope::refusal(&result["value"], &envelope)?;
            return Ok(self.stop());
        }
        ensure(text(&result["kind"])? == "policy-replay")?;
        let value = result["value"]
            .as_object()
            .ok_or(PolicyReplayClientError::InvalidResponse)?;
        let has_error = value.contains_key("error");
        let observation = if has_error { "error" } else { "knowledge" };
        let wrapper = object(
            &result["value"],
            &[
                "schema",
                "operation",
                "scope",
                "historical",
                "rehearsal",
                "policy_changed",
                "helpers_rerun",
                "live_evidence_read",
                "effect_authority",
                "promotion_authority",
                "payload_disclosed",
                "witness_values_disclosed",
                "current_eligibility",
                observation,
            ],
        )?;
        ensure(
            text(&wrapper["schema"])? == "fa.publication-policy-replay/2"
                && text(&wrapper["operation"])? == "policy-replay"
                && text(&wrapper["scope"])? == "retained_current_policy_corpus"
                && text(&wrapper["current_eligibility"])? == "not_observed",
        )?;
        for name in ["historical", "rehearsal"] {
            ensure(boolean(&wrapper[name])?)?;
        }
        for name in [
            "policy_changed",
            "helpers_rerun",
            "live_evidence_read",
            "effect_authority",
            "promotion_authority",
            "payload_disclosed",
            "witness_values_disclosed",
        ] {
            ensure(!boolean(&wrapper[name])?)?;
        }
        if has_error {
            let error = object(&wrapper["error"], &["code"])?;
            let reason = match text(&error["code"])? {
                "capacity" => "BudgetExhausted",
                "unavailable" => "LowerLayerUnverified",
                "withheld" => "DeniedByPolicy",
                "malformed" | "idempotency_conflict" => "IntegrityFailure",
                _ => return Err(PolicyReplayClientError::InvalidResponse),
            };
            envelope.error(reason)?;
            return Ok(self.stop());
        }
        let knowledge = &wrapper["knowledge"];
        match knowledge.get("state").and_then(Json::as_str) {
            Some("known") => {
                envelope.known()?;
                self.known(knowledge)
            }
            Some("unknown") => {
                let unknown = object(knowledge, &["state", "reason"])?;
                ensure(text(&unknown["reason"])? == "controller_unavailable")?;
                envelope.error("LowerLayerUnverified")?;
                Ok(self.stop())
            }
            Some("withheld") => {
                let withheld = object(knowledge, &["state", "authority_required"])?;
                ensure(text(&withheld["authority_required"])? == "supervisor_policy_replay")?;
                envelope.error("DeniedByPolicy")?;
                Ok(self.stop())
            }
            Some("stale") => {
                let stale = object(knowledge, &["state", "generation", "current"])?;
                ensure(
                    self.header.is_none()
                        && number(&stale["generation"])? == self.expected_generation
                        && number(&stale["current"])? != self.expected_generation
                        && number(&stale["current"])? != 0,
                )?;
                envelope.error("StaleGeneration")?;
                Ok(self.stop())
            }
            _ => Err(PolicyReplayClientError::InvalidResponse),
        }
    }

    fn check_request(&self, request: &TowerRequest) -> Result<()> {
        let valid = if self.progress.pages_seen == 0 {
            request == &self.initial
        } else {
            matches!(&request.arguments, ReadArguments::PolicyReplayContinue { campaign, cursor }
                if *campaign == self.campaign && Some(*cursor) == self.progress.next_cursor)
                && request.verb == Verb::PolicyReplay
                && request.address == self.initial.address
                && request.idempotency_key == self.initial.idempotency_key
                && encode_request(request).is_ok()
        };
        if valid {
            Ok(())
        } else {
            Err(PolicyReplayClientError::InvalidRequest)
        }
    }

    fn stop(&mut self) -> PolicyReplayProgress {
        self.progress.pages_seen += 1;
        self.progress.next_cursor = None;
        self.stopped = true;
        self.progress
    }

    fn known(&mut self, knowledge: &Json) -> Result<PolicyReplayProgress> {
        let knowledge = object(knowledge, &["state", "basis", "value"])?;
        let (control_sequence, policy_epoch) = self.check_basis(&knowledge["basis"])?;
        let body = object(
            &knowledge["value"],
            &[
                "page",
                "summary",
                "previous_policy",
                "candidate_policy",
                "cases",
                "newly_reviewable",
                "newly_blocked",
            ],
        )?;
        let page = object(
            &body["page"],
            &[
                "kind",
                "request_cursor",
                "offset",
                "count",
                "total",
                "enumeration_complete",
                "next_cursor",
                "remaining_pages_at_capture",
            ],
        )?;
        let totals = check_summary(&body["summary"])?;
        let header_page = text(&page["kind"])? == "header";
        ensure(header_page || text(&page["kind"])? == "cases")?;
        let offset = natural(&page["offset"])?;
        let count = natural(&page["count"])?;
        let rows = array(&body["cases"])?;
        let complete = boolean(&page["enumeration_complete"])?;
        let next = optional_number(&page["next_cursor"])?;
        let remaining = natural(&page["remaining_pages_at_capture"])?;
        let end = offset
            .checked_add(count)
            .ok_or(PolicyReplayClientError::InvalidResponse)?;
        ensure(
            natural(&page["total"])? == totals.total
                && count == rows.len()
                && offset == self.progress.cases_seen
                && end <= totals.total
                && optional_number(&page["request_cursor"])? == self.progress.next_cursor
                && remaining < MAX_POLICY_REPLAY_PAGE_QUERIES
                && self
                    .remaining_pages
                    .is_none_or(|previous| remaining < previous)
                && next.is_some() == (!complete && remaining != 0)
                && next.is_none_or(|cursor| cursor != 0 && !self.cursors.contains(&cursor))
                && self.cursors.len() < MAX_POLICY_REPLAY_PAGE_QUERIES,
        )?;
        let ReadArguments::PolicyReplayPaged { candidate, .. } = &self.initial.arguments else {
            unreachable!()
        };
        let pending_header = if header_page {
            ensure(
                self.header.is_none()
                    && self.progress.pages_seen == 0
                    && offset == 0
                    && count == 0
                    && !complete
                    && page["request_cursor"].is_null(),
            )?;
            let previous = super::request::policy::decode(&body["previous_policy"])
                .map_err(|_| PolicyReplayClientError::InvalidResponse)?;
            let selected = super::request::policy::decode(&body["candidate_policy"])
                .map_err(|_| PolicyReplayClientError::InvalidResponse)?;
            ensure(previous.generation() == self.expected_generation && &selected == candidate)?;
            Some(Header {
                basis: knowledge["basis"].clone(),
                summary: body["summary"].clone(),
                previous,
                totals,
                control_sequence,
                policy_epoch,
            })
        } else {
            ensure(
                self.header.is_some()
                    && body["previous_policy"].is_null()
                    && body["candidate_policy"].is_null()
                    && count == (totals.total - offset).min(MAX_POLICY_REPLAY_PAGE_CASES)
                    && count != 0
                    && offset.is_multiple_of(MAX_POLICY_REPLAY_PAGE_CASES)
                    && complete == (end == totals.total),
            )?;
            None
        };
        let header = pending_header
            .as_ref()
            .or(self.header.as_ref())
            .ok_or(PolicyReplayClientError::InvalidResponse)?;
        ensure(header.basis == knowledge["basis"] && header.summary == body["summary"])?;
        let mut ids = BTreeSet::new();
        let mut order = self.order;
        let mut counts = self.counts;
        let mut payload_bytes = self.payload_bytes;
        let mut blocked = Vec::new();
        let mut reviewable = Vec::new();
        for row in rows {
            let checked = cases::check(
                row,
                &header.previous,
                candidate,
                header.control_sequence,
                header.policy_epoch,
            )?;
            ensure(!self.ids.contains(&checked.id) && ids.insert(checked.id))?;
            order.accept(checked.id)?;
            checked.add(&mut counts)?;
            payload_bytes = payload_bytes
                .checked_add(checked.payload_bytes)
                .ok_or(PolicyReplayClientError::InvalidResponse)?;
            if checked.blocked() {
                blocked.push(checked.id);
            }
            if checked.reviewable() {
                reviewable.push(checked.id);
            }
        }
        ensure(
            self.ids.len().checked_add(ids.len()) == Some(end)
                && end <= MAX_REPLAY_CASES
                && counts.within(header.totals.counts)
                && payload_bytes <= header.totals.input_bytes,
        )?;
        cases::check_ids(&body["newly_blocked"], &blocked)?;
        cases::check_ids(&body["newly_reviewable"], &reviewable)?;
        if complete {
            ensure(counts == header.totals.counts && counts.total() == Some(header.totals.total))?;
        }
        // Commit only after the whole envelope, header/page and every row pass.
        if let Some(header) = pending_header {
            self.header = Some(header);
        }
        self.ids.extend(ids);
        self.order = order;
        self.counts = counts;
        self.payload_bytes = payload_bytes;
        self.remaining_pages = Some(remaining);
        if let Some(cursor) = next {
            self.cursors.insert(cursor);
        }
        self.progress = PolicyReplayProgress {
            next_cursor: next,
            complete,
            cases_seen: end,
            pages_seen: self.progress.pages_seen + 1,
        };
        self.stopped = !complete && next.is_none();
        Ok(self.progress)
    }

    fn check_basis(&self, value: &Json) -> Result<(u64, u64)> {
        let basis = object(
            value,
            &[
                "scope",
                "campaign",
                "origin",
                "journal_revision",
                "control_sequence",
                "policy_epoch",
                "policy_generation",
                "recorded_at",
            ],
        )?;
        let scope = object(
            &basis["scope"],
            &[
                "tenant",
                "principal",
                "run",
                "branch",
                "authority",
                "purpose",
            ],
        )?;
        for (name, expected) in [
            ("tenant", self.scope.tenant),
            ("principal", self.scope.principal),
            ("run", self.scope.run),
            ("branch", self.scope.branch),
            ("authority", self.scope.authority),
        ] {
            ensure(number(&scope[name])? == expected)?;
        }
        ensure(
            text(&scope["purpose"])? == "effect"
                && text(&basis["origin"])? == "acknowledged_owner"
                && number(&basis["campaign"])? == self.campaign
                && number(&basis["policy_generation"])? == self.expected_generation,
        )?;
        number(&basis["journal_revision"])?;
        optional_number(&basis["recorded_at"])?;
        Ok((
            number(&basis["control_sequence"])?,
            number(&basis["policy_epoch"])?,
        ))
    }
}

fn check_summary(value: &Json) -> Result<Summary> {
    let summary = object(
        value,
        &[
            "case_count",
            "input_bytes",
            "requires_shadow",
            "delta_counts",
            "limits",
            "remaining_campaigns_at_capture",
        ],
    )?;
    let raw_counts = object(
        &summary["delta_counts"],
        &[
            "unchanged",
            "newly_blocked",
            "newly_reviewable",
            "requires_shadow",
        ],
    )?;
    let counts = Counts {
        unchanged: natural(&raw_counts["unchanged"])?,
        blocked: natural(&raw_counts["newly_blocked"])?,
        reviewable: natural(&raw_counts["newly_reviewable"])?,
        shadow: natural(&raw_counts["requires_shadow"])?,
    };
    let limits = object(&summary["limits"], &["cases", "input_bytes"])?;
    let case_limit = natural(&limits["cases"])?;
    let input_limit = natural(&limits["input_bytes"])?;
    let total = natural(&summary["case_count"])?;
    let input_bytes = natural(&summary["input_bytes"])?;
    ensure(
        (1..=MAX_REPLAY_CASES).contains(&case_limit)
            && (1..=MAX_REPLAY_INPUT_BYTES).contains(&input_limit)
            && total != 0
            && total <= case_limit
            && counts.total() == Some(total)
            && input_bytes <= input_limit
            && boolean(&summary["requires_shadow"])? == (counts.shadow != 0)
            && natural(&summary["remaining_campaigns_at_capture"])? < MAX_POLICY_REPLAY_QUERIES,
    )?;
    Ok(Summary {
        total,
        input_bytes,
        counts,
    })
}
