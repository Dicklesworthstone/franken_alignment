//! FA-135 typed event journal and `fa tail` (plan §6.5, §6.7, §11.1, §17.8; FI-A06, FI-A13, FI-I15).
//!
//! Each authority domain maintains one ordered `DomainEventJournal`. Every L3–L8 transition
//! (proposal, hold, permit, dispatch, outcome, round open/commit/reveal/reduce, consequence,
//! fence, reset, promotion, finding, branch outcome, rehearsal outcome) emits one typed
//! [`JournalEvent`] ordered strictly by `control_seq`. Wall-clock timestamps are annotation-only
//! (`wall_clock_annotation_ms`) and never participate in ordering or causal digests (`FA-INV-011`).
//!
//! Rehearsal events (`rehearsal: true`) require `&ExperimentCx` and may only be appended to a
//! branch-scoped journal, never to a production authority domain's journal (`FA-INV-040`).

use std::collections::BTreeMap;
use std::ops::Bound::{Excluded, Unbounded};
use std::sync::Arc;

use crate::action::consequence::{Consequence, Rule};
use crate::address_resolver::{Address, Noun};
use crate::observation_algebra::UnknownReason;
use crate::typestate_cx::ExperimentCx;

/// Pure-Rust deterministic 256-bit digest accumulator for the reference model (zero external deps).
#[derive(Clone, Copy, Debug)]
pub struct ReferenceDigestHasher {
    state: [u64; 4],
}

impl Default for ReferenceDigestHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl ReferenceDigestHasher {
    pub const fn new() -> Self {
        Self {
            state: [
                0x6a09_e667_f3bc_c908,
                0xbb67_ae85_84ca_a73b,
                0x3c6e_f372_fe94_f82b,
                0xa54f_f53a_5f1d_36f1,
            ],
        }
    }

    pub fn update(&mut self, bytes: &[u8]) {
        for (idx, &byte) in bytes.iter().enumerate() {
            let lane = idx & 3;
            self.state[lane] = self.state[lane]
                .wrapping_mul(0x1000_0000_01b3)
                .wrapping_add(u64::from(byte) ^ ((idx as u64) << 8))
                .rotate_left(13);
            let next = (lane + 1) & 3;
            self.state[next] ^= self.state[lane].wrapping_mul(0x9e37_79b9_7f4a_7c15);
        }
    }

    pub fn finalize(self) -> [u8; 32] {
        let mut out = [0u8; 32];
        for (i, word) in self.state.into_iter().enumerate() {
            out[i * 8..(i + 1) * 8].copy_from_slice(&word.to_be_bytes());
        }
        out
    }
}

pub const EVENT_JOURNAL_SCHEMA_V1: &str = "fa.event_journal.v1";
pub const MAX_EVENT_ADDRESSES: usize = 32;
pub const MAX_SOURCE_FRONTIERS: usize = 32;

/// Source frontier coordinate pinned by a journal event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceFrontierEntry {
    pub source_id: u64,
    pub frontier_seq: u64,
}

/// Every L3–L8 transition kind enumerated from the system map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JournalEventKind {
    Proposal {
        attempt_id: u64,
    },
    Hold {
        attempt_id: u64,
        reason: String,
    },
    Permit {
        attempt_id: u64,
        cost_units: u64,
    },
    Dispatch {
        attempt_id: u64,
        adapter_id: u64,
    },
    Outcome {
        attempt_id: u64,
        outcome_class: String,
    },
    RoundOpen {
        round_id: u64,
    },
    RoundCommit {
        round_id: u64,
        helper_id: u64,
    },
    RoundReveal {
        round_id: u64,
        helper_id: u64,
    },
    RoundReduce {
        round_id: u64,
        attempt_id: u64,
        consequence: Consequence,
        rules: Vec<Rule>,
    },
    ConsequenceApplied {
        attempt_id: u64,
        consequence: Consequence,
    },
    Fence {
        fence_id: u64,
        ack_count: u64,
    },
    Reset {
        checkpoint_id: u64,
        incident_counter: u64,
    },
    Promotion {
        from_epoch: u64,
        to_epoch: u64,
        campaign_digest: [u8; 32],
    },
    Finding {
        finding_id: u64,
        detector_id: String,
        severity: u8,
    },
    BranchOutcome {
        branch_id: u64,
        summary_digest: [u8; 32],
    },
    RehearsalOutcome {
        branch_id: u64,
        would_consequence: Consequence,
        rehearsal: bool,
    },
}

impl JournalEventKind {
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::Proposal { .. } => "proposal",
            Self::Hold { .. } => "hold",
            Self::Permit { .. } => "permit",
            Self::Dispatch { .. } => "dispatch",
            Self::Outcome { .. } => "outcome",
            Self::RoundOpen { .. } => "round_open",
            Self::RoundCommit { .. } => "round_commit",
            Self::RoundReveal { .. } => "round_reveal",
            Self::RoundReduce { .. } => "round_reduce",
            Self::ConsequenceApplied { .. } => "consequence_applied",
            Self::Fence { .. } => "fence",
            Self::Reset { .. } => "reset",
            Self::Promotion { .. } => "promotion",
            Self::Finding { .. } => "finding",
            Self::BranchOutcome { .. } => "branch_outcome",
            Self::RehearsalOutcome { .. } => "rehearsal_outcome",
        }
    }

    pub const fn layer(&self) -> u8 {
        match self {
            Self::RoundOpen { .. }
            | Self::RoundCommit { .. }
            | Self::RoundReveal { .. }
            | Self::RoundReduce { .. } => 3,
            Self::Proposal { .. }
            | Self::Hold { .. }
            | Self::Permit { .. }
            | Self::ConsequenceApplied { .. }
            | Self::Fence { .. } => 4,
            Self::Dispatch { .. } | Self::Outcome { .. } | Self::Reset { .. } => 5,
            Self::BranchOutcome { .. } | Self::RehearsalOutcome { .. } => 7,
            Self::Promotion { .. } | Self::Finding { .. } => 8,
        }
    }
}

/// Errors returned by event schema validation, journal emission, tailing, and replay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JournalError {
    MissingControlSeq,
    ControlSequenceExhausted,
    EmptyAddresses,
    InvalidAddress {
        index: usize,
    },
    TenantMismatch,
    BranchMismatch {
        expected: u64,
        actual: u64,
    },
    TooManyAddresses {
        count: usize,
        max: usize,
    },
    TooManySourceFrontiers {
        count: usize,
        max: usize,
    },
    NonMonotonicControlSeq {
        previous: u64,
        attempted: u64,
    },
    DuplicateControlSeq {
        control_seq: u64,
    },
    RehearsalForbiddenInProduction,
    UnflaggedRehearsal,
    JournalGapDetected {
        missing_from_seq: u64,
        missing_through_seq: u64,
    },
    IntegrityFailure {
        round_id: u64,
        reason: String,
    },
}

/// Canonical journal event emitted by an L3–L8 transition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JournalEvent {
    pub control_seq: u64,
    pub source_frontiers: Vec<SourceFrontierEntry>,
    pub addresses: Vec<Address>,
    pub kind: JournalEventKind,
    pub payload_digest: [u8; 32],
    pub realized_cost: u64,
    /// Wall clock is annotation-only (§6.5) and never orders events or affects causal digests.
    pub wall_clock_annotation_ms: u64,
}

impl JournalEvent {
    pub fn new(
        control_seq: u64,
        source_frontiers: Vec<SourceFrontierEntry>,
        addresses: Vec<Address>,
        kind: JournalEventKind,
        payload_digest: [u8; 32],
        realized_cost: u64,
        wall_clock_annotation_ms: u64,
    ) -> Result<Self, JournalError> {
        let mut event = Self {
            control_seq,
            source_frontiers,
            addresses,
            kind,
            payload_digest,
            realized_cost,
            wall_clock_annotation_ms,
        };
        event.validate()?;
        event.source_frontiers.sort();
        Ok(event)
    }

    /// Validate again at journal admission because events and addresses have public fields.
    fn validate(&self) -> Result<(), JournalError> {
        if self.control_seq == 0 {
            return Err(JournalError::MissingControlSeq);
        }
        if self.addresses.is_empty() {
            return Err(JournalError::EmptyAddresses);
        }
        if self.addresses.len() > MAX_EVENT_ADDRESSES {
            return Err(JournalError::TooManyAddresses {
                count: self.addresses.len(),
                max: MAX_EVENT_ADDRESSES,
            });
        }
        if self.source_frontiers.len() > MAX_SOURCE_FRONTIERS {
            return Err(JournalError::TooManySourceFrontiers {
                count: self.source_frontiers.len(),
                max: MAX_SOURCE_FRONTIERS,
            });
        }
        for (index, address) in self.addresses.iter().enumerate() {
            Address::new(
                address.tenant.clone(),
                address.kind,
                address.id.clone(),
                address.generation,
            )
            .map_err(|_| JournalError::InvalidAddress { index })?;
        }
        if let JournalEventKind::RehearsalOutcome { rehearsal, .. } = &self.kind
            && !rehearsal
        {
            return Err(JournalError::UnflaggedRehearsal);
        }
        Ok(())
    }

    /// Canonical causal encoding independent of `wall_clock_annotation_ms`.
    pub fn encode_causal_canonical(&self) -> String {
        let frontiers: Vec<String> = self
            .source_frontiers
            .iter()
            .map(|f| format!("{}:{}", f.source_id, f.frontier_seq))
            .collect();
        let addrs: Vec<String> = self
            .addresses
            .iter()
            .map(Address::canonical_string)
            .collect();
        let kind_detail = encode_kind_canonical(&self.kind);
        format!(
            "{{\"schema\":\"{}\",\"control_seq\":{},\"layer\":{},\"kind\":\"{}\",\"detail\":{},\"frontiers\":[{}],\"addresses\":[{}],\"payload_digest\":\"{}\",\"realized_cost\":{}}}",
            EVENT_JOURNAL_SCHEMA_V1,
            self.control_seq,
            self.kind.layer(),
            self.kind.tag(),
            kind_detail,
            frontiers
                .iter()
                .map(|s| format!("\"{s}\""))
                .collect::<Vec<_>>()
                .join(","),
            addrs
                .iter()
                .map(|s| encode_json_string(s))
                .collect::<Vec<_>>()
                .join(","),
            hex_bytes(&self.payload_digest),
            self.realized_cost,
        )
    }

    /// Full wire encoding including the non-authoritative `wall_clock_annotation_ms`.
    pub fn encode_wire_canonical(&self) -> String {
        let causal = self.encode_causal_canonical();
        let prefix = &causal[..causal.len() - 1];
        format!(
            "{prefix},\"wall_clock_annotation_ms\":{}}}",
            self.wall_clock_annotation_ms
        )
    }

    /// Deterministic 256-bit digest of the causal content (excludes `wall_clock_annotation_ms`).
    pub fn causal_digest(&self) -> [u8; 32] {
        let mut hasher = ReferenceDigestHasher::new();
        hasher.update(b"fa-event-causal-v1:");
        hasher.update(self.encode_causal_canonical().as_bytes());
        hasher.finalize()
    }
}

fn encode_kind_canonical(kind: &JournalEventKind) -> String {
    match kind {
        JournalEventKind::Proposal { attempt_id } => format!("{{\"attempt_id\":{attempt_id}}}"),
        JournalEventKind::Hold { attempt_id, reason } => {
            format!(
                "{{\"attempt_id\":{attempt_id},\"reason\":{}}}",
                encode_json_string(reason)
            )
        }
        JournalEventKind::Permit {
            attempt_id,
            cost_units,
        } => format!("{{\"attempt_id\":{attempt_id},\"cost_units\":{cost_units}}}"),
        JournalEventKind::Dispatch {
            attempt_id,
            adapter_id,
        } => format!("{{\"attempt_id\":{attempt_id},\"adapter_id\":{adapter_id}}}"),
        JournalEventKind::Outcome {
            attempt_id,
            outcome_class,
        } => format!(
            "{{\"attempt_id\":{attempt_id},\"outcome_class\":{}}}",
            encode_json_string(outcome_class)
        ),
        JournalEventKind::RoundOpen { round_id } => format!("{{\"round_id\":{round_id}}}"),
        JournalEventKind::RoundCommit {
            round_id,
            helper_id,
        } => format!("{{\"round_id\":{round_id},\"helper_id\":{helper_id}}}"),
        JournalEventKind::RoundReveal {
            round_id,
            helper_id,
        } => format!("{{\"round_id\":{round_id},\"helper_id\":{helper_id}}}"),
        JournalEventKind::RoundReduce {
            round_id,
            attempt_id,
            consequence,
            rules,
        } => {
            let rule_tags: Vec<&'static str> = rules.iter().copied().map(rule_tag).collect();
            format!(
                "{{\"round_id\":{round_id},\"attempt_id\":{attempt_id},\"consequence\":{},\"rules\":[{}]}}",
                *consequence as u8,
                rule_tags
                    .iter()
                    .map(|r| format!("\"{r}\""))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
        JournalEventKind::ConsequenceApplied {
            attempt_id,
            consequence,
        } => format!(
            "{{\"attempt_id\":{attempt_id},\"consequence\":{}}}",
            *consequence as u8
        ),
        JournalEventKind::Fence {
            fence_id,
            ack_count,
        } => format!("{{\"fence_id\":{fence_id},\"ack_count\":{ack_count}}}"),
        JournalEventKind::Reset {
            checkpoint_id,
            incident_counter,
        } => {
            format!("{{\"checkpoint_id\":{checkpoint_id},\"incident_counter\":{incident_counter}}}")
        }
        JournalEventKind::Promotion {
            from_epoch,
            to_epoch,
            campaign_digest,
        } => format!(
            "{{\"from_epoch\":{from_epoch},\"to_epoch\":{to_epoch},\"campaign_digest\":\"{}\"}}",
            hex_bytes(campaign_digest)
        ),
        JournalEventKind::Finding {
            finding_id,
            detector_id,
            severity,
        } => format!(
            "{{\"finding_id\":{finding_id},\"detector_id\":{},\"severity\":{severity}}}",
            encode_json_string(detector_id)
        ),
        JournalEventKind::BranchOutcome {
            branch_id,
            summary_digest,
        } => format!(
            "{{\"branch_id\":{branch_id},\"summary_digest\":\"{}\"}}",
            hex_bytes(summary_digest)
        ),
        JournalEventKind::RehearsalOutcome {
            branch_id,
            would_consequence,
            rehearsal,
        } => format!(
            "{{\"branch_id\":{branch_id},\"would_consequence\":{},\"rehearsal\":{rehearsal}}}",
            *would_consequence as u8
        ),
    }
}

fn encode_json_string(value: &str) -> String {
    let mut out = String::new();
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{00}'..='\u{1f}' => out.push_str(&format!("\\u{:04x}", ch as u32)),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

pub const fn rule_tag(rule: Rule) -> &'static str {
    match rule {
        Rule::EmpiricalRecommendation => "empirical_recommendation",
        Rule::ExactDisqualifier => "exact_disqualifier",
        Rule::MandatoryAbsent => "mandatory_absent",
        Rule::Contradiction => "contradiction",
    }
}

fn hex_bytes(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// Authority scope of a journal: production authority domain vs isolated experiment branch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JournalScope {
    Production { domain_id: String },
    Branch { domain_id: String, branch_id: u64 },
}

impl JournalScope {
    pub fn domain_id(&self) -> &str {
        match self {
            Self::Production { domain_id } | Self::Branch { domain_id, .. } => domain_id,
        }
    }

    pub const fn is_production(&self) -> bool {
        matches!(self, Self::Production { .. })
    }
}

/// Missing control-sequence interval reported by `fa tail` and journal folds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JournalGapRange {
    pub missing_from_seq: u64,
    pub missing_through_seq: u64,
}

/// Result of `fa tail --domain <id> --since <seq>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TailBatch {
    pub domain_id: String,
    pub since_seq: u64,
    pub events: Vec<JournalEvent>,
    /// Reported gaps with explicit `UnknownReason::Incomplete` provenance; never filled.
    pub gaps: Vec<(JournalGapRange, UnknownReason)>,
    pub next_cursor_seq: u64,
}

/// Deterministic fold over a [`DomainEventJournal`].
pub trait JournalFold: Sized {
    type Output;

    fn initial_state() -> Self;
    fn step(&mut self, event: &JournalEvent);
    fn on_gap(&mut self, gap: JournalGapRange);
    fn finalize(self, exact_at_control_seq: u64) -> Self::Output;
}

/// Ordered event journal for a single authority domain or isolated branch.
#[derive(Debug)]
pub struct DomainEventJournal {
    scope: JournalScope,
    tenant: String,
    branch_context_id: Option<u64>,
    instance_token: Arc<()>,
    events_by_seq: BTreeMap<u64, JournalEvent>,
    last_seq: u64,
}

impl Clone for DomainEventJournal {
    fn clone(&self) -> Self {
        Self {
            scope: self.scope.clone(),
            tenant: self.tenant.clone(),
            branch_context_id: self.branch_context_id,
            // Each independently mutable copy owns a distinct retry identity.
            instance_token: Arc::new(()),
            events_by_seq: self.events_by_seq.clone(),
            last_seq: self.last_seq,
        }
    }
}

impl DomainEventJournal {
    pub fn new_production(tenant: &str, domain_id: &str) -> Self {
        Self {
            scope: JournalScope::Production {
                domain_id: domain_id.to_owned(),
            },
            tenant: tenant.to_owned(),
            branch_context_id: None,
            instance_token: Arc::new(()),
            events_by_seq: BTreeMap::new(),
            last_seq: 0,
        }
    }

    pub fn new_branch(cx: &ExperimentCx, tenant: &str, domain_id: &str, branch_id: u64) -> Self {
        Self {
            scope: JournalScope::Branch {
                domain_id: domain_id.to_owned(),
                branch_id,
            },
            tenant: tenant.to_owned(),
            branch_context_id: Some(cx.branch_id()),
            instance_token: Arc::new(()),
            events_by_seq: BTreeMap::new(),
            last_seq: 0,
        }
    }

    /// In-memory journal identity for retry isolation; neither a wire identity nor authority.
    pub(crate) fn instance_token(&self) -> Arc<()> {
        Arc::clone(&self.instance_token)
    }

    pub fn scope(&self) -> &JournalScope {
        &self.scope
    }

    pub fn tenant(&self) -> &str {
        &self.tenant
    }

    pub const fn last_control_seq(&self) -> u64 {
        self.last_seq
    }

    pub fn len(&self) -> usize {
        self.events_by_seq.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events_by_seq.is_empty()
    }

    /// Validate and append an event atomically within this journal's tenant and branch scope.
    pub fn append(&mut self, mut event: JournalEvent) -> Result<u64, JournalError> {
        event.validate()?;
        if event
            .addresses
            .iter()
            .any(|address| address.tenant != self.tenant)
        {
            return Err(JournalError::TenantMismatch);
        }
        match &self.scope {
            JournalScope::Production { .. } => {
                if matches!(&event.kind, JournalEventKind::RehearsalOutcome { .. }) {
                    return Err(JournalError::RehearsalForbiddenInProduction);
                }
            }
            JournalScope::Branch { branch_id, .. } => {
                if let Some(context_branch_id) = self.branch_context_id
                    && context_branch_id != *branch_id
                {
                    return Err(JournalError::BranchMismatch {
                        expected: context_branch_id,
                        actual: *branch_id,
                    });
                }
                if let JournalEventKind::BranchOutcome {
                    branch_id: actual, ..
                }
                | JournalEventKind::RehearsalOutcome {
                    branch_id: actual, ..
                } = &event.kind
                    && actual != branch_id
                {
                    return Err(JournalError::BranchMismatch {
                        expected: *branch_id,
                        actual: *actual,
                    });
                }
            }
        }
        if self.events_by_seq.contains_key(&event.control_seq) {
            return Err(JournalError::DuplicateControlSeq {
                control_seq: event.control_seq,
            });
        }
        if event.control_seq <= self.last_seq {
            return Err(JournalError::NonMonotonicControlSeq {
                previous: self.last_seq,
                attempted: event.control_seq,
            });
        }
        event.source_frontiers.sort();
        let seq = event.control_seq;
        self.events_by_seq.insert(seq, event);
        self.last_seq = seq;
        Ok(seq)
    }

    /// Emit the next sequential transition event (`last_seq + 1`).
    pub fn emit_transition(
        &mut self,
        source_frontiers: Vec<SourceFrontierEntry>,
        addresses: Vec<Address>,
        kind: JournalEventKind,
        payload_digest: [u8; 32],
        realized_cost: u64,
        wall_clock_annotation_ms: u64,
    ) -> Result<JournalEvent, JournalError> {
        let next_seq = self
            .last_seq
            .checked_add(1)
            .ok_or(JournalError::ControlSequenceExhausted)?;
        let event = JournalEvent::new(
            next_seq,
            source_frontiers,
            addresses,
            kind,
            payload_digest,
            realized_cost,
            wall_clock_annotation_ms,
        )?;
        self.append(event.clone())?;
        Ok(event)
    }

    /// Emit a rehearsal outcome to a branch journal. Requires `&ExperimentCx`.
    pub fn emit_rehearsal_outcome(
        &mut self,
        cx: &ExperimentCx,
        branch_id: u64,
        would_consequence: Consequence,
        addresses: Vec<Address>,
        payload_digest: [u8; 32],
        would_cost: u64,
        wall_clock_annotation_ms: u64,
    ) -> Result<JournalEvent, JournalError> {
        let JournalScope::Branch {
            branch_id: expected,
            ..
        } = &self.scope
        else {
            return Err(JournalError::RehearsalForbiddenInProduction);
        };
        if cx.branch_id() != *expected {
            return Err(JournalError::BranchMismatch {
                expected: *expected,
                actual: cx.branch_id(),
            });
        }
        self.emit_transition(
            Vec::new(),
            addresses,
            JournalEventKind::RehearsalOutcome {
                branch_id,
                would_consequence,
                rehearsal: true,
            },
            payload_digest,
            would_cost,
            wall_clock_annotation_ms,
        )
    }

    /// Stream events strictly in `control_seq` order starting strictly after `since_seq`.
    /// Any missing sequence numbers are reported in `gaps` and never synthesized.
    pub fn fa_tail(&self, since_seq: u64, max_events: usize) -> TailBatch {
        let mut events = Vec::new();
        let mut gaps = Vec::new();
        let mut next_cursor_seq = since_seq;

        for (&seq, event) in self.events_by_seq.range((Excluded(since_seq), Unbounded)) {
            if events.len() >= max_events {
                break;
            }
            if let Some(missing_from_seq) =
                next_cursor_seq.checked_add(1).filter(|next| *next < seq)
            {
                gaps.push((
                    JournalGapRange {
                        missing_from_seq,
                        missing_through_seq: seq - 1,
                    },
                    UnknownReason::Incomplete,
                ));
            }
            events.push(event.clone());
            next_cursor_seq = seq;
        }

        TailBatch {
            domain_id: self.scope.domain_id().to_owned(),
            since_seq,
            events,
            gaps,
            next_cursor_seq,
        }
    }

    /// Run a deterministic [`JournalFold`] over the entire journal through `up_to_seq`.
    pub fn run_fold<F: JournalFold>(&self, up_to_seq: u64) -> F::Output {
        let mut fold = F::initial_state();
        let mut exact_seq = 0u64;

        for (&seq, event) in self.events_by_seq.range(..=up_to_seq) {
            if let Some(missing_from_seq) = exact_seq.checked_add(1).filter(|next| *next < seq) {
                fold.on_gap(JournalGapRange {
                    missing_from_seq,
                    missing_through_seq: seq - 1,
                });
            }
            fold.step(event);
            exact_seq = seq;
        }

        fold.finalize(exact_seq)
    }

    pub fn events(&self) -> impl Iterator<Item = &JournalEvent> {
        self.events_by_seq.values()
    }
}

/// Stored closure digest for a completed round reduction (`prl6.4`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredClosureRecord {
    pub round_id: u64,
    pub attempt_id: u64,
    pub control_seq: u64,
    pub consequence: Consequence,
    pub rules: Vec<Rule>,
    pub closure_digest: [u8; 32],
}

impl StoredClosureRecord {
    pub fn compute_digest(
        round_id: u64,
        attempt_id: u64,
        control_seq: u64,
        consequence: Consequence,
        rules: &[Rule],
    ) -> [u8; 32] {
        let mut hasher = ReferenceDigestHasher::new();
        hasher.update(b"fa-decision-closure-replay-v1:");
        hasher.update(&round_id.to_be_bytes());
        hasher.update(&attempt_id.to_be_bytes());
        hasher.update(&control_seq.to_be_bytes());
        hasher.update(&[consequence as u8]);
        for rule in rules {
            hasher.update(rule_tag(*rule).as_bytes());
            hasher.update(b";");
        }
        hasher.finalize()
    }
}

/// Fold that reconstructs all `StoredClosureRecord`s purely from the journal (`prl6.4`).
#[derive(Default)]
pub struct ReplayClosureFold {
    closures: Vec<StoredClosureRecord>,
    gaps: Vec<JournalGapRange>,
}

impl JournalFold for ReplayClosureFold {
    type Output = (Vec<StoredClosureRecord>, Vec<JournalGapRange>);

    fn initial_state() -> Self {
        Self::default()
    }

    fn step(&mut self, event: &JournalEvent) {
        if let JournalEventKind::RoundReduce {
            round_id,
            attempt_id,
            consequence,
            rules,
        } = &event.kind
        {
            let closure_digest = StoredClosureRecord::compute_digest(
                *round_id,
                *attempt_id,
                event.control_seq,
                *consequence,
                rules,
            );
            self.closures.push(StoredClosureRecord {
                round_id: *round_id,
                attempt_id: *attempt_id,
                control_seq: event.control_seq,
                consequence: *consequence,
                rules: rules.clone(),
                closure_digest,
            });
        }
    }

    fn on_gap(&mut self, gap: JournalGapRange) {
        self.gaps.push(gap);
    }

    fn finalize(self, _exact_at_control_seq: u64) -> Self::Output {
        (self.closures, self.gaps)
    }
}

/// Replay every decision closure from the journal alone and verify byte-identical digests
/// against `stored_closures` (`prl6.4`, `FA-INV-016`). Any gap or digest mismatch yields
/// `Err(JournalError::IntegrityFailure)`.
pub fn decision_replay_from_journal(
    journal: &DomainEventJournal,
    stored_closures: &[StoredClosureRecord],
) -> Result<Vec<StoredClosureRecord>, JournalError> {
    let (replayed, gaps) = journal.run_fold::<ReplayClosureFold>(journal.last_control_seq());
    if let Some(first_gap) = gaps.first() {
        return Err(JournalError::IntegrityFailure {
            round_id: 0,
            reason: format!(
                "journal gap detected in range {}..={}",
                first_gap.missing_from_seq, first_gap.missing_through_seq
            ),
        });
    }
    if replayed.len() != stored_closures.len() {
        return Err(JournalError::IntegrityFailure {
            round_id: 0,
            reason: format!(
                "closure count mismatch: journal replayed {}, store has {}",
                replayed.len(),
                stored_closures.len()
            ),
        });
    }
    for (rep, stored) in replayed.iter().zip(stored_closures.iter()) {
        if rep != stored {
            return Err(JournalError::IntegrityFailure {
                round_id: stored.round_id,
                reason: "replayed closure digest or fields diverged from stored closure".to_owned(),
            });
        }
    }
    Ok(replayed)
}

/// Helper to construct a standard L4/L5/L6 address for transition tests.
pub fn transition_address(tenant: &str, noun: Noun, id: &str) -> Address {
    Address::new(tenant, noun, id, Some(1)).expect("valid transition address")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typestate_cx::RootCx;

    #[test]
    fn event_schema_refuses_zero_seq_and_wall_clock_never_affects_causal_digest() {
        let addr = transition_address("org", Noun::Attempt, "att-1");
        let missing_seq = JournalEvent::new(
            0,
            vec![SourceFrontierEntry {
                source_id: 1,
                frontier_seq: 10,
            }],
            vec![addr.clone()],
            JournalEventKind::Proposal { attempt_id: 1 },
            [0x11; 32],
            5,
            1_000,
        );
        assert_eq!(missing_seq, Err(JournalError::MissingControlSeq));

        let empty_addrs = JournalEvent::new(
            1,
            Vec::new(),
            Vec::new(),
            JournalEventKind::Proposal { attempt_id: 1 },
            [0x11; 32],
            5,
            1_000,
        );
        assert_eq!(empty_addrs, Err(JournalError::EmptyAddresses));

        let ev_early_clock = JournalEvent::new(
            1,
            vec![SourceFrontierEntry {
                source_id: 1,
                frontier_seq: 10,
            }],
            vec![addr.clone()],
            JournalEventKind::Proposal { attempt_id: 1 },
            [0x11; 32],
            5,
            1_000,
        )
        .expect("valid event");
        let ev_late_clock = JournalEvent::new(
            1,
            vec![SourceFrontierEntry {
                source_id: 1,
                frontier_seq: 10,
            }],
            vec![addr],
            JournalEventKind::Proposal { attempt_id: 1 },
            [0x11; 32],
            5,
            9_999_999,
        )
        .expect("valid event");

        assert_eq!(
            ev_early_clock.encode_causal_canonical(),
            ev_late_clock.encode_causal_canonical()
        );
        assert_eq!(
            ev_early_clock.causal_digest(),
            ev_late_clock.causal_digest()
        );
    }

    #[test]
    fn every_l3_to_l8_transition_emits_and_rehearsal_is_forbidden_in_production() {
        let mut prod = DomainEventJournal::new_production("org", "dom-main");
        let transitions = vec![
            (
                Noun::Attempt,
                "a1",
                JournalEventKind::Proposal { attempt_id: 1 },
            ),
            (
                Noun::Round,
                "r1",
                JournalEventKind::RoundOpen { round_id: 1 },
            ),
            (
                Noun::Round,
                "r1",
                JournalEventKind::RoundCommit {
                    round_id: 1,
                    helper_id: 10,
                },
            ),
            (
                Noun::Round,
                "r1",
                JournalEventKind::RoundReveal {
                    round_id: 1,
                    helper_id: 10,
                },
            ),
            (
                Noun::Closure,
                "c1",
                JournalEventKind::RoundReduce {
                    round_id: 1,
                    attempt_id: 1,
                    consequence: Consequence::Continue,
                    rules: vec![Rule::EmpiricalRecommendation],
                },
            ),
            (
                Noun::Permit,
                "p1",
                JournalEventKind::Permit {
                    attempt_id: 1,
                    cost_units: 4,
                },
            ),
            (
                Noun::Attempt,
                "a1",
                JournalEventKind::Dispatch {
                    attempt_id: 1,
                    adapter_id: 2,
                },
            ),
            (
                Noun::Attempt,
                "a1",
                JournalEventKind::Outcome {
                    attempt_id: 1,
                    outcome_class: "verified_success".to_owned(),
                },
            ),
            (
                Noun::Attempt,
                "a2",
                JournalEventKind::Hold {
                    attempt_id: 2,
                    reason: "mandatory_absent".to_owned(),
                },
            ),
            (
                Noun::Closure,
                "c2",
                JournalEventKind::ConsequenceApplied {
                    attempt_id: 2,
                    consequence: Consequence::HoldEffect,
                },
            ),
            (
                Noun::Fence,
                "f1",
                JournalEventKind::Fence {
                    fence_id: 1,
                    ack_count: 3,
                },
            ),
            (
                Noun::Checkpoint,
                "chk1",
                JournalEventKind::Reset {
                    checkpoint_id: 1,
                    incident_counter: 1,
                },
            ),
            (
                Noun::Policy,
                "camp1",
                JournalEventKind::Promotion {
                    from_epoch: 1,
                    to_epoch: 2,
                    campaign_digest: [0x22; 32],
                },
            ),
            (
                Noun::Finding,
                "find1",
                JournalEventKind::Finding {
                    finding_id: 1,
                    detector_id: "rt_rubber_stamp".to_owned(),
                    severity: 2,
                },
            ),
            (
                Noun::Branch,
                "br1",
                JournalEventKind::BranchOutcome {
                    branch_id: 7,
                    summary_digest: [0x33; 32],
                },
            ),
        ];

        for (idx, (noun, id, kind)) in transitions.into_iter().enumerate() {
            // Deliberately invert wall-clock timestamps to prove control_seq alone orders events.
            let inverted_clock = 10_000 - (idx as u64 * 100);
            prod.emit_transition(
                vec![SourceFrontierEntry {
                    source_id: 1,
                    frontier_seq: (idx + 1) as u64,
                }],
                vec![transition_address("org", noun, id)],
                kind,
                [idx as u8; 32],
                1,
                inverted_clock,
            )
            .expect("emit transition");
        }
        assert_eq!(prod.len(), 15);

        // Attempting to append a RehearsalOutcome to production fails closed (FA-INV-040).
        let rehearsal_ev = JournalEvent::new(
            16,
            Vec::new(),
            vec![transition_address("org", Noun::Branch, "reh-1")],
            JournalEventKind::RehearsalOutcome {
                branch_id: 7,
                would_consequence: Consequence::HoldEffect,
                rehearsal: true,
            },
            [0x99; 32],
            0,
            500,
        )
        .expect("valid rehearsal event");
        assert_eq!(
            prod.append(rehearsal_ev),
            Err(JournalError::RehearsalForbiddenInProduction)
        );

        // Branch journal accepts rehearsal outcomes under &ExperimentCx.
        let root = RootCx::new_nucleus(1, 1, 1);
        let exp_cx = root.mint_experiment(7).expect("experiment cx");
        let mut branch_journal = DomainEventJournal::new_branch(&exp_cx, "org", "dom-main", 7);
        let emitted = branch_journal
            .emit_rehearsal_outcome(
                &exp_cx,
                7,
                Consequence::HoldEffect,
                vec![transition_address("org", Noun::Branch, "reh-1")],
                [0x99; 32],
                3,
                500,
            )
            .expect("branch journal accepts rehearsal outcome");
        assert_eq!(emitted.control_seq, 1);
    }

    #[test]
    fn fa_tail_reports_gaps_and_replay_from_journal_detects_tampering() {
        let mut journal = DomainEventJournal::new_production("org", "dom-replay");
        let addr_c1 = transition_address("org", Noun::Closure, "c-1");
        let addr_c2 = transition_address("org", Noun::Closure, "c-2");

        let rules_1 = vec![Rule::EmpiricalRecommendation];
        let ev1 = JournalEvent::new(
            1,
            Vec::new(),
            vec![addr_c1],
            JournalEventKind::RoundReduce {
                round_id: 10,
                attempt_id: 100,
                consequence: Consequence::Continue,
                rules: rules_1.clone(),
            },
            [0x01; 32],
            2,
            1_000,
        )
        .expect("ev1");
        journal.append(ev1).expect("append 1");

        let rules_2 = vec![Rule::EmpiricalRecommendation, Rule::ExactDisqualifier];
        let ev2 = JournalEvent::new(
            2,
            Vec::new(),
            vec![addr_c2.clone()],
            JournalEventKind::RoundReduce {
                round_id: 11,
                attempt_id: 101,
                consequence: Consequence::Deny,
                rules: rules_2.clone(),
            },
            [0x02; 32],
            3,
            2_000,
        )
        .expect("ev2");
        journal.append(ev2).expect("append 2");

        let stored = vec![
            StoredClosureRecord {
                round_id: 10,
                attempt_id: 100,
                control_seq: 1,
                consequence: Consequence::Continue,
                rules: rules_1,
                closure_digest: StoredClosureRecord::compute_digest(
                    10,
                    100,
                    1,
                    Consequence::Continue,
                    &[Rule::EmpiricalRecommendation],
                ),
            },
            StoredClosureRecord {
                round_id: 11,
                attempt_id: 101,
                control_seq: 2,
                consequence: Consequence::Deny,
                rules: rules_2,
                closure_digest: StoredClosureRecord::compute_digest(
                    11,
                    101,
                    2,
                    Consequence::Deny,
                    &[Rule::EmpiricalRecommendation, Rule::ExactDisqualifier],
                ),
            },
        ];

        // Replay matches stored closures identically.
        let replayed = decision_replay_from_journal(&journal, &stored)
            .expect("replay from journal equals stored closures");
        assert_eq!(replayed, stored);

        // Tampering with a stored closure digest is caught as IntegrityFailure.
        let mut tampered = stored.clone();
        tampered[1].closure_digest[0] ^= 0xFF;
        assert!(matches!(
            decision_replay_from_journal(&journal, &tampered),
            Err(JournalError::IntegrityFailure { round_id: 11, .. })
        ));

        // Injecting a gap (appending control_seq = 5 after 2) is reported by fa_tail and fails replay.
        let ev5 = JournalEvent::new(
            5,
            Vec::new(),
            vec![addr_c2],
            JournalEventKind::Proposal { attempt_id: 102 },
            [0x05; 32],
            1,
            5_000,
        )
        .expect("ev5");
        journal.append(ev5).expect("append 5 with gap 3..=4");

        let tail = journal.fa_tail(0, 10);
        assert_eq!(tail.events.len(), 3);
        assert_eq!(
            tail.gaps,
            vec![(
                JournalGapRange {
                    missing_from_seq: 3,
                    missing_through_seq: 4,
                },
                UnknownReason::Incomplete
            )]
        );
        assert!(matches!(
            decision_replay_from_journal(&journal, &stored),
            Err(JournalError::IntegrityFailure { round_id: 0, .. })
        ));
    }
}
