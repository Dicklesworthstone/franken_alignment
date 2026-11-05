//! Structured projection of the reference Tower response. Nested observations
//! pass through the same complete Knowledge encoder as the outer result.

use std::fmt::Write as _;
use std::marker::PhantomData;

use super::{ResponseStatus, TowerEnvelope, TowerLimits, TowerResponse, TowerValue};
use crate::action::consequence::Consequence;
use crate::address_resolver::{Address, RelatedEdge, RelationKind, ResolvedObject, ResolverPlane};
use crate::event_journal::{SourceFrontierEntry, TailBatch, rule_tag};
use crate::explain_affordances::{
    Affordance, MAX_PREDICATE_TREE_DEPTH, MAX_PREDICATE_TREE_NODES, PredicateNode,
    PredicateNodeKind, PredicateNodeSummary, PredicateTree, WhyHeldReport,
};
use crate::knowledge_surface::{SafetyRelevant, SerializerRefusal, encode_knowledge_json};
use crate::observation_algebra::{Knowledge, MAX_OPERATOR_FRONTIERS};
use crate::situation_rehearsal::{SituationItem, SituationReport};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TowerEncodingError {
    InvalidKnowledge,
    OutputLimit,
}

impl TowerResponse {
    /// Encode the exact sixteen-field envelope and complete typed result.
    /// Refusal never truncates an observation or turns missing data into Known.
    pub fn to_json(&self) -> Result<String, TowerEncodingError> {
        let encoder = Encoder {
            max_bytes: self.envelope.limits.max_response_bytes,
        };
        let envelope = encoder.envelope(&self.envelope)?;
        let result = encoder
            .knowledge::<_, TowerResult>(&self.result, |value| encoder.tower_value(value))?;
        encoder.bounded(object_json(&[("envelope", envelope), ("result", result)]))
    }
}

// These types name the existing fields; none implements SafetyRelevant for a
// bare primitive. PredicateTruth is available only inside this wire adapter.
trait KnowledgeKind {
    const NAME: &'static str;
}

struct TowerResult;
impl KnowledgeKind for TowerResult {
    const NAME: &'static str = "TowerValue";
}

struct ObjectIdentity;
impl KnowledgeKind for ObjectIdentity {
    const NAME: &'static str = "ResolvedObject";
}

struct SourceFrontiers;
impl KnowledgeKind for SourceFrontiers {
    const NAME: &'static str = "SourceFrontiers";
}

struct SituationEntry;
impl KnowledgeKind for SituationEntry {
    const NAME: &'static str = "SituationItem";
}

struct TypedAffordances;
impl KnowledgeKind for TypedAffordances {
    const NAME: &'static str = "Affordances";
}

struct SemanticGeneration;
impl KnowledgeKind for SemanticGeneration {
    const NAME: &'static str = "SemanticGeneration";
}

struct ControlSequence;
impl KnowledgeKind for ControlSequence {
    const NAME: &'static str = "ControlSequence";
}

struct RemainingReadUnits;
impl KnowledgeKind for RemainingReadUnits {
    const NAME: &'static str = "RemainingReadUnits";
}

struct RealizedReadUnits;
impl KnowledgeKind for RealizedReadUnits {
    const NAME: &'static str = "RealizedReadUnits";
}

struct PredicateTruth;
impl KnowledgeKind for PredicateTruth {
    const NAME: &'static str = "PredicateTruth";
}

struct EncodedPayload<K> {
    json: String,
    kind: PhantomData<K>,
}

impl<K: KnowledgeKind> SafetyRelevant for EncodedPayload<K> {
    fn safety_type_name() -> &'static str {
        K::NAME
    }

    fn canonical_payload_string(&self) -> String {
        self.json.clone()
    }

    fn canonical_payload_json(&self) -> String {
        self.json.clone()
    }
}

struct Encoder {
    max_bytes: usize,
}

impl Encoder {
    fn bounded(&self, encoded: String) -> Result<String, TowerEncodingError> {
        if encoded.len() > self.max_bytes {
            return Err(TowerEncodingError::OutputLimit);
        }
        Ok(encoded)
    }

    /// Map a borrowed epistemic value without losing its basis or converting a
    /// non-Known variant into a value. Payload encoding runs only for Known.
    fn knowledge<T, K: KnowledgeKind>(
        &self,
        original: &Knowledge<T>,
        encode: impl FnOnce(&T) -> Result<String, TowerEncodingError>,
    ) -> Result<String, TowerEncodingError> {
        let mapped: Knowledge<EncodedPayload<K>> = match original {
            Knowledge::Known { value, basis } => Knowledge::Known {
                value: EncodedPayload {
                    json: self.bounded(encode(value)?)?,
                    kind: PhantomData,
                },
                basis: *basis,
            },
            Knowledge::Pending {
                frontiers,
                expected_cost,
            } => {
                if frontiers.len() > MAX_OPERATOR_FRONTIERS {
                    return Err(TowerEncodingError::OutputLimit);
                }
                Knowledge::Pending {
                    frontiers: frontiers.clone(),
                    expected_cost: *expected_cost,
                }
            }
            Knowledge::Unknown { reason } => Knowledge::Unknown { reason: *reason },
            Knowledge::Withheld { authority_required } => Knowledge::Withheld {
                authority_required: *authority_required,
            },
            Knowledge::Stale {
                basis,
                current_generation,
            } => Knowledge::Stale {
                basis: *basis,
                current_generation: *current_generation,
            },
            Knowledge::Absent { domain } => Knowledge::Absent {
                domain: domain.clone(),
            },
        };
        let encoded = encode_knowledge_json(&mapped).map_err(|error| match error {
            SerializerRefusal::EncodingLimitExceeded { .. } => TowerEncodingError::OutputLimit,
            SerializerRefusal::InvalidPayloadJson { .. }
            | SerializerRefusal::BareScalarRejected { .. } => TowerEncodingError::InvalidKnowledge,
        })?;
        self.bounded(encoded)
    }

    fn envelope(&self, envelope: &TowerEnvelope) -> Result<String, TowerEncodingError> {
        let status = match envelope.status {
            ResponseStatus::Ok => "ok",
            ResponseStatus::Error => "error",
        };
        self.bounded(object_json(&[
            ("schema_version", quote(envelope.schema_version)),
            ("operation_id", quote(&envelope.operation_id)),
            ("status", quote(status)),
            (
                "reason_code",
                envelope
                    .reason_code
                    .map_or_else(|| "null".to_owned(), |reason| quote(reason.as_str())),
            ),
            ("address", address_json(&envelope.address)),
            (
                "generation",
                self.knowledge::<_, SemanticGeneration>(&envelope.generation, |value| {
                    Ok(value.to_string())
                })?,
            ),
            (
                "control_seq",
                self.knowledge::<_, ControlSequence>(&envelope.control_seq, |value| {
                    Ok(value.to_string())
                })?,
            ),
            (
                "source_frontiers",
                self.knowledge::<_, SourceFrontiers>(&envelope.source_frontiers, |value| {
                    Ok(frontiers_json(value))
                })?,
            ),
            ("missing", addresses_json(&envelope.missing)),
            ("limits", limits_json(envelope.limits)),
            (
                "budgets_remaining",
                self.knowledge::<_, RemainingReadUnits>(&envelope.budgets_remaining, |value| {
                    Ok(value.to_string())
                })?,
            ),
            (
                "realized_cost",
                self.knowledge::<_, RealizedReadUnits>(&envelope.realized_cost, |value| {
                    Ok(value.to_string())
                })?,
            ),
            (
                "affordances",
                self.knowledge::<_, TypedAffordances>(&envelope.affordances, |value| {
                    Ok(affordances_json(value))
                })?,
            ),
            ("retry_safe", envelope.retry_safe.to_string()),
            (
                "idempotency_key",
                envelope
                    .idempotency_key
                    .as_deref()
                    .map_or_else(|| "null".to_owned(), quote),
            ),
            (
                "would_change_this",
                addresses_json(&envelope.would_change_this),
            ),
        ]))
    }

    fn tower_value(&self, value: &TowerValue) -> Result<String, TowerEncodingError> {
        let (kind, body) = match value {
            TowerValue::Object(value) => ("object", resolved_object_json(value)),
            TowerValue::Related(edges) => ("related", self.related(edges)?),
            TowerValue::Tail(batch) => ("tail", tail_json(batch)),
            TowerValue::Situation(report) => ("situation", self.situation(report)?),
            TowerValue::Explain(tree) => ("explain", self.tree(tree)?),
            TowerValue::WhyHeld(report) => ("why-held", self.why_held(report)?),
            TowerValue::Next(affordances) => ("next", affordances_json(affordances)),
        };
        self.bounded(object_json(&[("kind", quote(kind)), ("value", body)]))
    }

    fn related(&self, edges: &[RelatedEdge]) -> Result<String, TowerEncodingError> {
        let mut encoded = Vec::with_capacity(edges.len());
        for edge in edges {
            encoded.push(object_json(&[
                ("from", address_json(&edge.from)),
                ("to", address_json(&edge.to)),
                ("relation", quote(relation_tag(edge.relation))),
                (
                    "target_resolution",
                    self.knowledge::<_, ObjectIdentity>(&edge.target_resolution, |value| {
                        Ok(resolved_object_json(value))
                    })?,
                ),
            ]));
        }
        self.bounded(array_json(encoded))
    }

    fn situation(&self, report: &SituationReport) -> Result<String, TowerEncodingError> {
        let mut items = Vec::with_capacity(report.items.len());
        for item in &report.items {
            items.push(
                self.knowledge::<_, SituationEntry>(item, |value| Ok(situation_item_json(value)))?,
            );
        }
        self.bounded(object_json(&[
            ("tenant", quote(&report.tenant)),
            ("domain_id", quote(&report.domain_id)),
            (
                "exact_at_control_seq",
                report.exact_at_control_seq.to_string(),
            ),
            ("open_frontiers", frontiers_json(&report.open_frontiers)),
            (
                "pagination_cursor",
                report
                    .pagination_cursor
                    .as_ref()
                    .map_or_else(|| "null".to_owned(), address_json),
            ),
            ("items", array_json(items)),
        ]))
    }

    fn tree(&self, tree: &PredicateTree) -> Result<String, TowerEncodingError> {
        let mut count = 0;
        let mut depth = 0;
        let root = self.node(&tree.root, 1, &mut count, &mut depth)?;
        if count != tree.node_count || depth != tree.max_depth {
            return Err(TowerEncodingError::InvalidKnowledge);
        }
        self.bounded(object_json(&[
            ("subject_address", address_json(&tree.subject_address)),
            ("root", root),
            ("node_count", tree.node_count.to_string()),
            ("max_depth", tree.max_depth.to_string()),
            ("replay_digest", quote(&hex(&tree.replay_digest))),
        ]))
    }

    fn node(
        &self,
        node: &PredicateNode,
        depth: usize,
        count: &mut usize,
        max_depth: &mut usize,
    ) -> Result<String, TowerEncodingError> {
        if depth > MAX_PREDICATE_TREE_DEPTH || *count >= MAX_PREDICATE_TREE_NODES {
            return Err(TowerEncodingError::OutputLimit);
        }
        *count += 1;
        *max_depth = (*max_depth).max(depth);
        let mut children = Vec::with_capacity(node.children.len());
        for child in &node.children {
            children.push(self.node(child, depth + 1, count, max_depth)?);
        }
        self.bounded(object_json(&[
            ("node_id", quote(&node.node_id)),
            ("kind", predicate_kind_json(&node.kind)),
            ("claim_class", quote(node.claim_class.tag())),
            (
                "status",
                self.knowledge::<_, PredicateTruth>(&node.status, |value| Ok(value.to_string()))?,
            ),
            (
                "evidence_addresses",
                addresses_json(&node.evidence_addresses),
            ),
            ("children", array_json(children)),
        ]))
    }

    fn summary(&self, node: &PredicateNodeSummary) -> Result<String, TowerEncodingError> {
        self.bounded(object_json(&[
            ("node_id", quote(&node.node_id)),
            ("kind", predicate_kind_json(&node.kind)),
            ("claim_class", quote(node.claim_class.tag())),
            (
                "status",
                self.knowledge::<_, PredicateTruth>(&node.status, |value| Ok(value.to_string()))?,
            ),
            (
                "evidence_addresses",
                addresses_json(&node.evidence_addresses),
            ),
        ]))
    }

    fn why_held(&self, report: &WhyHeldReport) -> Result<String, TowerEncodingError> {
        let mut items = Vec::with_capacity(report.items.len());
        for item in &report.items {
            items.push(object_json(&[
                ("node", self.summary(&item.node)?),
                ("affordances", affordances_json(&item.affordances)),
            ]));
        }
        self.bounded(object_json(&[
            ("subject_address", address_json(&report.subject_address)),
            (
                "exact_at_control_seq",
                report.exact_at_control_seq.to_string(),
            ),
            ("items", array_json(items)),
        ]))
    }
}

fn resolved_object_json(object: &ResolvedObject) -> String {
    let plane = match object.plane {
        ResolverPlane::ControlLedger => "control_ledger",
        ResolverPlane::EvidenceStore => "evidence_store",
        ResolverPlane::DerivedProjections => "derived_projections",
        ResolverPlane::ExperimentStore => "experiment_store",
    };
    object_json(&[
        ("address", address_json(&object.address)),
        ("plane", quote(plane)),
        ("summary_digest", object.summary_digest.to_string()),
        ("generation", object.generation.to_string()),
    ])
}

fn tail_json(batch: &TailBatch) -> String {
    object_json(&[
        ("domain_id", quote(&batch.domain_id)),
        ("since_seq", batch.since_seq.to_string()),
        (
            "events",
            array_json(
                batch
                    .events
                    .iter()
                    .map(|event| event.encode_wire_canonical()),
            ),
        ),
        (
            "gaps",
            array_json(batch.gaps.iter().map(|(gap, reason)| {
                object_json(&[
                    ("missing_from_seq", gap.missing_from_seq.to_string()),
                    ("missing_through_seq", gap.missing_through_seq.to_string()),
                    ("reason", quote(&format!("{reason:?}"))),
                ])
            })),
        ),
        ("next_cursor_seq", batch.next_cursor_seq.to_string()),
    ])
}

fn situation_item_json(item: &SituationItem) -> String {
    object_json(&[
        ("address", address_json(&item.address)),
        ("section", quote(item.section.tag())),
        ("summary", quote(&item.summary)),
        (
            "decision_relevance_voi",
            item.decision_relevance_voi.to_string(),
        ),
        ("is_hard_floor", item.is_hard_floor.to_string()),
    ])
}

fn predicate_kind_json(kind: &PredicateNodeKind) -> String {
    let mut fields = vec![("kind", quote(kind.kind_tag()))];
    match kind {
        PredicateNodeKind::Conjunct => {}
        PredicateNodeKind::PolicyPredicate { predicate_id } => {
            fields.push(("predicate_id", quote(predicate_id)));
        }
        PredicateNodeKind::HelperVerdict {
            helper_id,
            recommendation,
        } => {
            fields.push(("helper_id", helper_id.to_string()));
            fields.push(("recommendation", quote(consequence_tag(*recommendation))));
        }
        PredicateNodeKind::ReducerRule { rule } => {
            fields.push(("rule", quote(rule_tag(*rule))));
        }
        PredicateNodeKind::Consequence { consequence } => {
            fields.push(("consequence", quote(consequence_tag(*consequence))));
        }
        PredicateNodeKind::EvidenceRef { domain_id } => {
            fields.push(("domain_id", domain_id.to_string()));
        }
    }
    object_json(&fields)
}

fn consequence_tag(value: Consequence) -> &'static str {
    match value {
        Consequence::Continue => "Continue",
        Consequence::HoldEffect => "HoldEffect",
        Consequence::Deny => "Deny",
        Consequence::NarrowAuthority => "NarrowAuthority",
        Consequence::SuspendRun => "SuspendRun",
        Consequence::ResetToCheckpoint => "ResetToCheckpoint",
    }
}

fn relation_tag(relation: RelationKind) -> &'static str {
    match relation {
        RelationKind::ClosureStrongRef => "closure_strong_ref",
        RelationKind::ClosureWeakRef => "closure_weak_ref",
        RelationKind::Provenance => "provenance",
        RelationKind::ThoughtGraphEdge => "thought_graph_edge",
        RelationKind::SupportSetMember => "support_set_member",
        RelationKind::AnnotationTarget => "annotation_target",
    }
}

fn affordances_json(affordances: &[Affordance]) -> String {
    array_json(affordances.iter().map(|affordance| {
        object_json(&[
            ("affordance_id", quote(&affordance.affordance_id)),
            ("verb", quote(affordance.verb)),
            ("target", address_json(&affordance.target)),
            (
                "authority_required",
                quote(affordance.authority_required.as_str()),
            ),
            ("cost_estimate", affordance.cost_estimate.to_string()),
            (
                "expected_information_gain",
                affordance.expected_information_gain.to_string(),
            ),
            ("would_change", addresses_json(&affordance.would_change)),
            (
                "unsatisfied_node_id",
                quote(&affordance.unsatisfied_node_id),
            ),
            ("is_hard_floor", affordance.is_hard_floor.to_string()),
            ("rehearsable", affordance.rehearsable.to_string()),
        ])
    }))
}

fn limits_json(limits: TowerLimits) -> String {
    object_json(&[
        ("max_objects", limits.max_objects.to_string()),
        ("max_edges", limits.max_edges.to_string()),
        ("max_journal_events", limits.max_journal_events.to_string()),
        ("max_explanations", limits.max_explanations.to_string()),
        ("max_page_events", limits.max_page_events.to_string()),
        ("max_response_bytes", limits.max_response_bytes.to_string()),
        ("max_read_units", limits.max_read_units.to_string()),
    ])
}

fn frontiers_json(frontiers: &[SourceFrontierEntry]) -> String {
    array_json(frontiers.iter().map(|frontier| {
        object_json(&[
            ("source_id", frontier.source_id.to_string()),
            ("frontier_seq", frontier.frontier_seq.to_string()),
        ])
    }))
}

fn address_json(address: &Address) -> String {
    quote(&address.canonical_string())
}

fn addresses_json(addresses: &[Address]) -> String {
    array_json(addresses.iter().map(address_json))
}

fn quote(value: &str) -> String {
    let mut encoded = String::new();
    encoded.push('"');
    for ch in value.chars() {
        match ch {
            '"' => encoded.push_str("\\\""),
            '\\' => encoded.push_str("\\\\"),
            '\n' => encoded.push_str("\\n"),
            '\r' => encoded.push_str("\\r"),
            '\t' => encoded.push_str("\\t"),
            '\u{08}' => encoded.push_str("\\b"),
            '\u{0c}' => encoded.push_str("\\f"),
            ch if ch <= '\u{1f}' => {
                let _ = write!(encoded, "\\u{:04x}", ch as u32);
            }
            ch => encoded.push(ch),
        }
    }
    encoded.push('"');
    encoded
}

fn object_json(fields: &[(&str, String)]) -> String {
    format!(
        "{{{}}}",
        fields
            .iter()
            .map(|(key, value)| format!("{}:{value}", quote(key)))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn array_json(values: impl IntoIterator<Item = String>) -> String {
    format!("[{}]", values.into_iter().collect::<Vec<_>>().join(","))
}

fn hex(bytes: &[u8]) -> String {
    let mut encoded = String::new();
    for byte in bytes {
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}
