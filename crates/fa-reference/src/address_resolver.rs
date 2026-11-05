//! Canonical address scheme `fa://<tenant>/<kind>/<id>[@<generation>]`, noun
//! registry binding (`registry/vocabulary.json`), and unified authorization-filtered
//! resolver (`fa get`, `fa related`) across all tower layers (`FA-134`, plan §6.7,
//! §17.2, §17.6, §17.11).

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use crate::{
    knowledge_surface::SafetyRelevant,
    observation_algebra::{AuthorityKind, Basis, Knowledge, UnknownReason},
};

/// The 26 registered nouns in `registry/vocabulary.json` (`fa.vocabulary/0.3`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Noun {
    Run,
    Attempt,
    Effect,
    Permit,
    Closure,
    Receipt,
    Round,
    Consequence,
    Helper,
    Evidence,
    View,
    Witness,
    Frontier,
    Observation,
    Passport,
    Policy,
    Profile,
    Grade,
    Fence,
    Branch,
    Checkpoint,
    Incident,
    Finding,
    Ledger,
    Annotation,
    Handoff,
}

/// Which backing store plane owns a given [`Noun`] (`2fkx.2`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResolverPlane {
    /// L4/L5 control and effect ledger (`run`, `attempt`, `effect`, `permit`, `closure`, `receipt`, `fence`, `checkpoint`).
    ControlLedger,
    /// L1/L2 observation and evidence store (`evidence`, `view`, `witness`, `frontier`, `observation`, `annotation`, `handoff`).
    EvidenceStore,
    /// L3/L8 derived judgment and governance projections (`round`, `consequence`, `helper`, `passport`, `policy`, `profile`, `grade`, `incident`, `finding`, `ledger`).
    DerivedProjections,
    /// L7 counterfactual and rehearsal store (`branch`).
    ExperimentStore,
}

impl Noun {
    pub const ALL: [Self; 26] = [
        Self::Run,
        Self::Attempt,
        Self::Effect,
        Self::Permit,
        Self::Closure,
        Self::Receipt,
        Self::Round,
        Self::Consequence,
        Self::Helper,
        Self::Evidence,
        Self::View,
        Self::Witness,
        Self::Frontier,
        Self::Observation,
        Self::Passport,
        Self::Policy,
        Self::Profile,
        Self::Grade,
        Self::Fence,
        Self::Branch,
        Self::Checkpoint,
        Self::Incident,
        Self::Finding,
        Self::Ledger,
        Self::Annotation,
        Self::Handoff,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Run => "run",
            Self::Attempt => "attempt",
            Self::Effect => "effect",
            Self::Permit => "permit",
            Self::Closure => "closure",
            Self::Receipt => "receipt",
            Self::Round => "round",
            Self::Consequence => "consequence",
            Self::Helper => "helper",
            Self::Evidence => "evidence",
            Self::View => "view",
            Self::Witness => "witness",
            Self::Frontier => "frontier",
            Self::Observation => "observation",
            Self::Passport => "passport",
            Self::Policy => "policy",
            Self::Profile => "profile",
            Self::Grade => "grade",
            Self::Fence => "fence",
            Self::Branch => "branch",
            Self::Checkpoint => "checkpoint",
            Self::Incident => "incident",
            Self::Finding => "finding",
            Self::Ledger => "ledger",
            Self::Annotation => "annotation",
            Self::Handoff => "handoff",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, AddressError> {
        match raw {
            "run" => Ok(Self::Run),
            "attempt" => Ok(Self::Attempt),
            "effect" => Ok(Self::Effect),
            "permit" => Ok(Self::Permit),
            "closure" => Ok(Self::Closure),
            "receipt" => Ok(Self::Receipt),
            "round" => Ok(Self::Round),
            "consequence" => Ok(Self::Consequence),
            "helper" => Ok(Self::Helper),
            "evidence" => Ok(Self::Evidence),
            "view" => Ok(Self::View),
            "witness" => Ok(Self::Witness),
            "frontier" => Ok(Self::Frontier),
            "observation" => Ok(Self::Observation),
            "passport" => Ok(Self::Passport),
            "policy" => Ok(Self::Policy),
            "profile" => Ok(Self::Profile),
            "grade" => Ok(Self::Grade),
            "fence" => Ok(Self::Fence),
            "branch" => Ok(Self::Branch),
            "checkpoint" => Ok(Self::Checkpoint),
            "incident" => Ok(Self::Incident),
            "finding" => Ok(Self::Finding),
            "ledger" => Ok(Self::Ledger),
            "annotation" => Ok(Self::Annotation),
            "handoff" => Ok(Self::Handoff),
            other => Err(AddressError::UnknownNounKind {
                kind: other.to_owned(),
                would_change_this: "use one of the 26 registered nouns in registry/vocabulary.json"
                    .to_owned(),
            }),
        }
    }

    pub const fn plane(self) -> ResolverPlane {
        match self {
            Self::Run
            | Self::Attempt
            | Self::Effect
            | Self::Permit
            | Self::Closure
            | Self::Receipt
            | Self::Fence
            | Self::Checkpoint => ResolverPlane::ControlLedger,
            Self::Evidence
            | Self::View
            | Self::Witness
            | Self::Frontier
            | Self::Observation
            | Self::Annotation
            | Self::Handoff => ResolverPlane::EvidenceStore,
            Self::Round
            | Self::Consequence
            | Self::Helper
            | Self::Passport
            | Self::Policy
            | Self::Profile
            | Self::Grade
            | Self::Incident
            | Self::Finding
            | Self::Ledger => ResolverPlane::DerivedProjections,
            Self::Branch => ResolverPlane::ExperimentStore,
        }
    }

    pub const fn required_authority(self) -> AuthorityKind {
        match self.plane() {
            ResolverPlane::ControlLedger => AuthorityKind::Verify,
            ResolverPlane::EvidenceStore => AuthorityKind::ExactRead,
            ResolverPlane::DerivedProjections => AuthorityKind::Judge,
            ResolverPlane::ExperimentStore => AuthorityKind::Probe,
        }
    }
}

/// Errors returned when parsing or validating an [`Address`] (`2fkx.1`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AddressError {
    MissingScheme {
        raw: String,
        would_change_this: String,
    },
    WrongSegmentCount {
        raw: String,
        would_change_this: String,
    },
    EmptyComponent {
        component: &'static str,
        would_change_this: String,
    },
    InvalidCharacters {
        component: &'static str,
        would_change_this: String,
    },
    UnknownNounKind {
        kind: String,
        would_change_this: String,
    },
    InvalidGeneration {
        raw_generation: String,
        would_change_this: String,
    },
    DisplayAliasForbiddenInAuthorityPath {
        alias: String,
        would_change_this: String,
    },
}

fn is_valid_segment_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.'
}

/// Canonical object identity `fa://<tenant>/<kind>/<id>[@<generation>]` (`2fkx.1`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Address {
    pub tenant: String,
    pub kind: Noun,
    pub id: String,
    pub generation: Option<u64>,
}

impl Address {
    pub fn new(
        tenant: impl Into<String>,
        kind: Noun,
        id: impl Into<String>,
        generation: Option<u64>,
    ) -> Result<Self, AddressError> {
        let tenant = tenant.into();
        let id = id.into();
        if tenant.is_empty() {
            return Err(AddressError::EmptyComponent {
                component: "tenant",
                would_change_this: "supply non-empty tenant segment in fa://<tenant>/<kind>/<id>"
                    .to_owned(),
            });
        }
        if id.is_empty() {
            return Err(AddressError::EmptyComponent {
                component: "id",
                would_change_this: "supply non-empty id segment in fa://<tenant>/<kind>/<id>"
                    .to_owned(),
            });
        }
        if !tenant.chars().all(is_valid_segment_char) {
            return Err(AddressError::InvalidCharacters {
                component: "tenant",
                would_change_this: "use only ASCII alphanumeric, '-', '_', or '.' in tenant"
                    .to_owned(),
            });
        }
        if !id.chars().all(is_valid_segment_char) {
            return Err(AddressError::InvalidCharacters {
                component: "id",
                would_change_this: "use only ASCII alphanumeric, '-', '_', or '.' in id".to_owned(),
            });
        }
        Ok(Self {
            tenant,
            kind,
            id,
            generation,
        })
    }

    pub fn parse(raw: &str) -> Result<Self, AddressError> {
        if raw.contains('#') && !raw.starts_with("fa://") {
            return Err(AddressError::DisplayAliasForbiddenInAuthorityPath {
                alias: raw.to_owned(),
                would_change_this:
                    "expand display alias through DisplayAliasTable before passing to authority or resolver paths"
                        .to_owned(),
            });
        }
        let Some(rest) = raw.strip_prefix("fa://") else {
            return Err(AddressError::MissingScheme {
                raw: raw.to_owned(),
                would_change_this:
                    "prefix address with 'fa://' as in fa://<tenant>/<kind>/<id>[@<generation>]"
                        .to_owned(),
            });
        };
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() != 3 {
            return Err(AddressError::WrongSegmentCount {
                raw: raw.to_owned(),
                would_change_this:
                    "format address with exactly 3 slash-separated segments: fa://<tenant>/<kind>/<id>[@<generation>]"
                        .to_owned(),
            });
        }
        let tenant = parts[0];
        let kind_str = parts[1];
        let id_and_gen = parts[2];
        let kind = Noun::parse(kind_str)?;

        let (id_part, generation) = match id_and_gen.split_once('@') {
            Some((id_str, gen_str)) => {
                if gen_str.is_empty() || !gen_str.chars().all(|c| c.is_ascii_digit()) {
                    return Err(AddressError::InvalidGeneration {
                        raw_generation: gen_str.to_owned(),
                        would_change_this: "supply a non-negative integer generation after '@'"
                            .to_owned(),
                    });
                }
                let parsed_gen =
                    gen_str
                        .parse::<u64>()
                        .map_err(|_| AddressError::InvalidGeneration {
                            raw_generation: gen_str.to_owned(),
                            would_change_this: "supply a u64 generation after '@'".to_owned(),
                        })?;
                (id_str, Some(parsed_gen))
            }
            None => (id_and_gen, None),
        };

        Self::new(tenant, kind, id_part, generation)
    }

    pub fn canonical_string(&self) -> String {
        match self.generation {
            Some(gen_val) => format!(
                "fa://{}/{}/{}@{}",
                self.tenant,
                self.kind.as_str(),
                self.id,
                gen_val
            ),
            None => format!("fa://{}/{}/{}", self.tenant, self.kind.as_str(), self.id),
        }
    }

    // Public fields allow construction without `new` or `parse`; resolver
    // admission must check the canonical components again.
    fn has_canonical_components(&self) -> bool {
        !self.tenant.is_empty()
            && !self.id.is_empty()
            && self.tenant.chars().all(is_valid_segment_char)
            && self.id.chars().all(is_valid_segment_char)
    }

    pub fn without_generation(&self) -> Self {
        Self {
            tenant: self.tenant.clone(),
            kind: self.kind,
            id: self.id.clone(),
            generation: None,
        }
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.canonical_string())
    }
}

/// Display alias table mapping short human/UI aliases (e.g. `run#1`) to full
/// canonical [`Address`] values. Authority paths accept only full [`Address`]es (`2fkx.1`).
#[derive(Clone, Debug, Default)]
pub struct DisplayAliasTable {
    alias_to_address: BTreeMap<String, Address>,
}

impl DisplayAliasTable {
    pub fn register_alias(&mut self, alias: impl Into<String>, address: Address) {
        self.alias_to_address.insert(alias.into(), address);
    }

    pub fn expand_for_display_ui(&self, alias: &str) -> Option<&Address> {
        self.alias_to_address.get(alias)
    }

    /// Authority paths must reject short display aliases outright (`2fkx.1`).
    pub fn parse_authority_address(raw: &str) -> Result<Address, AddressError> {
        Address::parse(raw)
    }
}

/// Resolved object returned by `fa get` wrapped inside `Knowledge<ResolvedObject>` (`2fkx.2`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedObject {
    pub address: Address,
    pub plane: ResolverPlane,
    pub summary_digest: u64,
    pub generation: u64,
}

impl SafetyRelevant for ResolvedObject {
    fn safety_type_name() -> &'static str {
        "ResolvedObject"
    }

    fn canonical_payload_string(&self) -> String {
        format!(
            "ResolvedObject(addr={},plane={:?},digest={},gen={})",
            self.address, self.plane, self.summary_digest, self.generation
        )
    }
}

/// Caller authorization scope for `fa get` and `fa related` (`2fkx.2`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallerScope {
    pub caller_tenant: String,
    pub allowed_planes: BTreeSet<ResolverPlane>,
    pub granted_authorities: BTreeSet<AuthorityKind>,
}

impl CallerScope {
    pub fn full_tenant_access(tenant: impl Into<String>) -> Self {
        let mut allowed_planes = BTreeSet::new();
        allowed_planes.insert(ResolverPlane::ControlLedger);
        allowed_planes.insert(ResolverPlane::EvidenceStore);
        allowed_planes.insert(ResolverPlane::DerivedProjections);
        allowed_planes.insert(ResolverPlane::ExperimentStore);

        let mut granted_authorities = BTreeSet::new();
        granted_authorities.insert(AuthorityKind::Verify);
        granted_authorities.insert(AuthorityKind::ExactRead);
        granted_authorities.insert(AuthorityKind::Judge);
        granted_authorities.insert(AuthorityKind::Probe);

        Self {
            caller_tenant: tenant.into(),
            allowed_planes,
            granted_authorities,
        }
    }

    pub fn is_authorized_for(&self, addr: &Address) -> Result<(), AuthorityKind> {
        let req = addr.kind.required_authority();
        if self.caller_tenant != addr.tenant
            || !self.allowed_planes.contains(&addr.kind.plane())
            || !self.granted_authorities.contains(&req)
        {
            return Err(req);
        }
        Ok(())
    }
}

/// Typed edge kinds returned by `fa related` across planes (`2fkx.3`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RelationKind {
    ClosureStrongRef,
    ClosureWeakRef,
    Provenance,
    ThoughtGraphEdge,
    SupportSetMember,
    AnnotationTarget,
}

/// Directed typed edge returned by `fa related` (`2fkx.3`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelatedEdge {
    pub from: Address,
    pub to: Address,
    pub relation: RelationKind,
    pub target_resolution: Knowledge<ResolvedObject>,
}

impl SafetyRelevant for Vec<RelatedEdge> {
    fn safety_type_name() -> &'static str {
        "Vec<RelatedEdge>"
    }

    fn canonical_payload_string(&self) -> String {
        format!("RelatedEdges(count={})", self.len())
    }
}

/// Structured audit log entry for every `fa get` / `fa related` call (`2fkx.2`, `2fkx.5`).
/// Never logs object content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolutionLogEntry {
    pub verb: &'static str,
    pub caller_tenant: String,
    pub requested_address: String,
    pub returned_variant: &'static str,
    pub authorization_decision: &'static str,
}

#[derive(Clone, Debug)]
struct StoredEntry {
    object: ResolvedObject,
    basis: Basis,
}

#[derive(Clone, Debug)]
struct StoredEdge {
    source_generation: Option<u64>,
    relation: RelationKind,
    target: Address,
}

/// Single address resolver (`fa get` and `fa related`) dispatching by [`Noun`]
/// across the four planes (`ControlLedger`, `EvidenceStore`, `DerivedProjections`, `ExperimentStore`).
#[derive(Clone, Debug, Default)]
pub struct AddressResolver {
    control_ledger: BTreeMap<(String, Noun, String), StoredEntry>,
    evidence_store: BTreeMap<(String, Noun, String), StoredEntry>,
    derived_projections: BTreeMap<(String, Noun, String), StoredEntry>,
    experiment_store: BTreeMap<(String, Noun, String), StoredEntry>,
    edges: BTreeMap<(String, Noun, String), Vec<StoredEdge>>,
    resolution_logs: Vec<ResolutionLogEntry>,
}

impl AddressResolver {
    pub fn new() -> Self {
        Self::default()
    }

    fn plane_map_mut(
        &mut self,
        plane: ResolverPlane,
    ) -> &mut BTreeMap<(String, Noun, String), StoredEntry> {
        match plane {
            ResolverPlane::ControlLedger => &mut self.control_ledger,
            ResolverPlane::EvidenceStore => &mut self.evidence_store,
            ResolverPlane::DerivedProjections => &mut self.derived_projections,
            ResolverPlane::ExperimentStore => &mut self.experiment_store,
        }
    }

    fn plane_map(&self, plane: ResolverPlane) -> &BTreeMap<(String, Noun, String), StoredEntry> {
        match plane {
            ResolverPlane::ControlLedger => &self.control_ledger,
            ResolverPlane::EvidenceStore => &self.evidence_store,
            ResolverPlane::DerivedProjections => &self.derived_projections,
            ResolverPlane::ExperimentStore => &self.experiment_store,
        }
    }

    /// Insert a resolved object into the plane corresponding to `object.address.kind`.
    pub fn insert_object(&mut self, mut object: ResolvedObject, basis: Basis) {
        let plane = object.address.kind.plane();
        object.plane = plane;
        let key = (
            object.address.tenant.clone(),
            object.address.kind,
            object.address.id.clone(),
        );
        self.plane_map_mut(plane)
            .insert(key, StoredEntry { object, basis });
    }

    /// Register a typed cross-plane or intra-plane edge for `fa related` (`2fkx.3`).
    ///
    /// An explicit source generation remains exact, including when its object arrives later.
    /// An unqualified source binds only to the current object at insertion. Without that object
    /// the edge remains unbound and cannot become visible merely because an object later appears.
    pub fn insert_edge(&mut self, from: Address, relation: RelationKind, to: Address) {
        let key = (from.tenant, from.kind, from.id);
        let source_generation = from.generation.or_else(|| {
            self.plane_map(from.kind.plane())
                .get(&key)
                .map(|entry| entry.object.generation)
        });
        self.edges.entry(key).or_default().push(StoredEdge {
            source_generation,
            relation,
            target: to,
        });
    }

    pub fn logs(&self) -> &[ResolutionLogEntry] {
        &self.resolution_logs
    }

    /// Resolve an already authorized address without emitting another audit entry.
    /// Both read verbs use this admission path before inspecting object contents
    /// or expanding graph edges.
    fn resolve_authorized(&self, address: &Address) -> Knowledge<ResolvedObject> {
        if !address.has_canonical_components() {
            return Knowledge::unknown(UnknownReason::MissingExactDomain);
        }
        let key = (address.tenant.clone(), address.kind, address.id.clone());
        match self.plane_map(address.kind.plane()).get(&key) {
            Some(entry) => match address.generation {
                Some(requested_gen) if requested_gen < entry.object.generation => {
                    Knowledge::stale(entry.basis, entry.object.generation)
                }
                Some(requested_gen) if requested_gen > entry.object.generation => {
                    Knowledge::unknown(UnknownReason::IncompatibleGeneration)
                }
                _ => Knowledge::known(entry.object.clone(), entry.basis),
            },
            None => Knowledge::unknown(UnknownReason::MissingExactDomain),
        }
    }

    /// `fa get`: Resolve any [`Address`] to `Knowledge<ResolvedObject>` with
    /// strict authorization filtering (`2fkx.2`).
    ///
    /// Cross-tenant or out-of-scope requests return `Knowledge::Withheld` with an
    /// identical shape whether or not the target object exists (`FA-INV-008`, plan §17.6).
    pub fn fa_get(&mut self, caller: &CallerScope, address: &Address) -> Knowledge<ResolvedObject> {
        if let Err(required_auth) = caller.is_authorized_for(address) {
            let result = Knowledge::withheld(required_auth);
            self.resolution_logs.push(ResolutionLogEntry {
                verb: "get",
                caller_tenant: caller.caller_tenant.clone(),
                requested_address: address.canonical_string(),
                returned_variant: result.variant_name(),
                authorization_decision: "deny_withheld",
            });
            return result;
        }

        let result = self.resolve_authorized(address);

        self.resolution_logs.push(ResolutionLogEntry {
            verb: "get",
            caller_tenant: caller.caller_tenant.clone(),
            requested_address: address.canonical_string(),
            returned_variant: result.variant_name(),
            authorization_decision: "allow",
        });
        result
    }

    /// `fa related`: Return typed edges from `source` across closure DAG strong/weak
    /// references, provenance, thought-graph edges, support sets, and annotations (`2fkx.3`).
    /// Source admission matches `fa get`; unauthorized or malformed targets do not
    /// enter the result or its target-resolution log (plan §17.6).
    pub fn fa_related(
        &mut self,
        caller: &CallerScope,
        source: &Address,
        relation_filter: Option<RelationKind>,
    ) -> Knowledge<Vec<RelatedEdge>> {
        if let Err(required_auth) = caller.is_authorized_for(source) {
            let result = Knowledge::withheld(required_auth);
            self.resolution_logs.push(ResolutionLogEntry {
                verb: "related",
                caller_tenant: caller.caller_tenant.clone(),
                requested_address: source.canonical_string(),
                returned_variant: result.variant_name(),
                authorization_decision: "deny_withheld",
            });
            return result;
        }

        // A historical or future source must not acquire current graph edges
        // under the requested generation's label.
        let source_resolution = self.resolve_authorized(source);
        let result = source_resolution.map(|source_object| {
            let key = (source.tenant.clone(), source.kind, source.id.clone());
            let mut resolved_source = source.clone();
            resolved_source.generation = Some(source_object.generation);
            let visible_edges: Vec<_> = self
                .edges
                .get(&key)
                .into_iter()
                .flatten()
                .filter(|edge| {
                    edge.source_generation == Some(source_object.generation)
                        && relation_filter.is_none_or(|filter| filter == edge.relation)
                        && caller.is_authorized_for(&edge.target).is_ok()
                        && edge.target.has_canonical_components()
                })
                .cloned()
                .collect();
            let mut resolved_edges = Vec::new();
            for edge in visible_edges {
                // Address, relation and result count are protected metadata too.
                // Hidden targets never reach resolution or its audit log.
                let target_resolution = self.fa_get(caller, &edge.target);
                resolved_edges.push(RelatedEdge {
                    from: resolved_source.clone(),
                    to: edge.target,
                    relation: edge.relation,
                    target_resolution,
                });
            }
            resolved_edges
        });
        self.resolution_logs.push(ResolutionLogEntry {
            verb: "related",
            caller_tenant: caller.caller_tenant.clone(),
            requested_address: source.canonical_string(),
            returned_variant: result.variant_name(),
            authorization_decision: if matches!(
                &result,
                Knowledge::Unknown {
                    reason: UnknownReason::MissingExactDomain
                }
            ) {
                "allow_missing"
            } else {
                "allow"
            },
        });
        result
    }
}

/// Addressed event log record using [`Address`] as the sole join key (`2fkx.4`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddressedEventRecord {
    pub event_address: Address,
    pub subject_address: Address,
    pub verb: String,
    pub would_change_this: Option<String>,
}

impl AddressedEventRecord {
    pub fn to_agent_json(&self) -> String {
        match &self.would_change_this {
            Some(hint) => format!(
                "{{\"event\":\"{}\",\"subject\":\"{}\",\"verb\":\"{}\",\"would_change_this\":\"{}\"}}",
                self.event_address, self.subject_address, self.verb, hint
            ),
            None => format!(
                "{{\"event\":\"{}\",\"subject\":\"{}\",\"verb\":\"{}\"}}",
                self.event_address, self.subject_address, self.verb
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawIdLintViolation {
    pub forbidden_token: String,
    pub would_change_this: String,
}

const FORBIDDEN_RAW_ID_KEYS: [&str; 12] = [
    "\"run_id\"",
    "\"attempt_id\"",
    "\"effect_id\"",
    "\"permit_id\"",
    "\"closure_id\"",
    "\"receipt_id\"",
    "\"round_id\"",
    "\"helper_id\"",
    "\"finding_id\"",
    "\"incident_id\"",
    "\"branch_id\"",
    "\"checkpoint_id\"",
];

/// Lint that rejects agent-facing logs, error envelopes, receipts, and reports
/// containing raw ad-hoc ID fields instead of canonical `fa://` addresses (`2fkx.4`).
pub fn lint_no_raw_ids_in_agent_output(agent_output: &str) -> Result<(), RawIdLintViolation> {
    for forbidden in FORBIDDEN_RAW_ID_KEYS {
        if agent_output.contains(forbidden) {
            return Err(RawIdLintViolation {
                forbidden_token: forbidden.to_owned(),
                would_change_this: format!(
                    "replace raw identifier field {forbidden} with a canonical 'fa://<tenant>/<kind>/<id>' address"
                ),
            });
        }
    }
    if !agent_output.contains("fa://") {
        return Err(RawIdLintViolation {
            forbidden_token: "missing_fa_address".to_owned(),
            would_change_this:
                "include at least one canonical 'fa://<tenant>/<kind>/<id>' subject address in agent-facing output"
                    .to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        knowledge_surface::AgentSurfaceSerializer,
        observation_algebra::{BasisIdentity, ClaimClass, ProofStates, ScopedProof},
    };

    fn sample_basis(generation: u64) -> Basis {
        let declared = ScopedProof::Declared {
            scope_id: 1,
            generation,
        };
        Basis::new(
            BasisIdentity {
                subject_id: 1,
                question_id: 1,
                profile_id: 1,
                semantic_generation: generation,
                policy_epoch: 1,
                model_epoch: 1,
                tokenizer_epoch: 1,
                codec_epoch: 1,
                control_seq: 1,
            },
            ClaimClass::Invariant,
            ProofStates {
                authentic_origin: declared,
                complete_for_contract: declared,
                semantically_valid: declared,
                available_for_replay: declared,
            },
        )
    }

    #[test]
    fn parser_printer_round_trip_for_all_26_nouns_and_rejects_unknown_kinds() {
        assert_eq!(Noun::ALL.len(), 26);
        for (idx, noun) in Noun::ALL.iter().copied().enumerate() {
            let without_gen = format!("fa://tenant-a/{}/obj-{idx}", noun.as_str());
            let parsed = Address::parse(&without_gen).expect("valid noun address should parse");
            assert_eq!(parsed.kind, noun);
            assert_eq!(parsed.generation, None);
            assert_eq!(parsed.canonical_string(), without_gen);

            let with_gen = format!("fa://tenant-a/{}/obj-{idx}@{}", noun.as_str(), idx + 1);
            let parsed_gen =
                Address::parse(&with_gen).expect("valid generation address should parse");
            assert_eq!(parsed_gen.kind, noun);
            assert_eq!(parsed_gen.generation, Some((idx + 1) as u64));
            assert_eq!(parsed_gen.canonical_string(), with_gen);
        }

        // Unknown noun kind is refused.
        assert!(matches!(
            Address::parse("fa://tenant-a/unknown_widget/obj-1"),
            Err(AddressError::UnknownNounKind { .. })
        ));

        // Display alias is refused in authority path.
        let mut aliases = DisplayAliasTable::default();
        let canonical = Address::parse("fa://tenant-a/run/r-99@2").unwrap();
        aliases.register_alias("run#99", canonical.clone());
        assert_eq!(aliases.expand_for_display_ui("run#99"), Some(&canonical));
        assert!(matches!(
            DisplayAliasTable::parse_authority_address("run#99"),
            Err(AddressError::DisplayAliasForbiddenInAuthorityPath { .. })
        ));
    }

    #[test]
    fn fa_get_resolves_all_26_noun_kinds_and_withheld_shape_never_leaks_existence() {
        let mut resolver = AddressResolver::new();
        let basis = sample_basis(3);

        for (idx, noun) in Noun::ALL.iter().copied().enumerate() {
            let addr = Address::new("tenant-alpha", noun, format!("item-{idx}"), Some(3)).unwrap();
            resolver.insert_object(
                ResolvedObject {
                    address: addr,
                    plane: noun.plane(),
                    summary_digest: 1000 + idx as u64,
                    generation: 3,
                },
                basis,
            );
        }

        let caller_alpha = CallerScope::full_tenant_access("tenant-alpha");
        let caller_beta = CallerScope::full_tenant_access("tenant-beta");

        for (idx, noun) in Noun::ALL.iter().copied().enumerate() {
            let existing_addr =
                Address::new("tenant-alpha", noun, format!("item-{idx}"), Some(3)).unwrap();
            let missing_addr =
                Address::new("tenant-alpha", noun, format!("nonexistent-{idx}"), Some(3)).unwrap();

            // Authorized caller resolves all 26 kinds to Known.
            let known_res = resolver.fa_get(&caller_alpha, &existing_addr);
            assert_eq!(known_res.variant_name(), "Known");

            // Cross-tenant caller receives Withheld for BOTH existing and nonexistent objects,
            // and the serialized response is byte-for-byte identical (no existence leak).
            let withheld_existing = resolver.fa_get(&caller_beta, &existing_addr);
            let withheld_missing = resolver.fa_get(&caller_beta, &missing_addr);
            assert_eq!(withheld_existing, withheld_missing);
            let ser_existing =
                AgentSurfaceSerializer::serialize_safety_field("object", &withheld_existing);
            let ser_missing =
                AgentSurfaceSerializer::serialize_safety_field("object", &withheld_missing);
            assert_eq!(ser_existing.canonical_json, ser_missing.canonical_json);
        }

        // Verify structured logs recorded every resolution without content leakage.
        assert_eq!(resolver.logs().len(), 26 * 3);
        assert!(
            resolver
                .logs()
                .iter()
                .any(|entry| entry.authorization_decision == "deny_withheld"
                    && entry.returned_variant == "Withheld")
        );
    }

    #[test]
    fn fa_related_traverses_closure_dag_and_thought_graph_edges_and_lints_raw_ids() {
        let mut resolver = AddressResolver::new();
        let basis = sample_basis(1);

        let closure_addr = Address::parse("fa://tenant-a/closure/cl-1@1").unwrap();
        let evidence_addr = Address::parse("fa://tenant-a/evidence/ev-1@1").unwrap();
        let finding_addr = Address::parse("fa://tenant-a/finding/fd-1@1").unwrap();
        let cross_tenant_secret = Address::parse("fa://tenant-b/witness/wt-secret@1").unwrap();

        for addr in [
            &closure_addr,
            &evidence_addr,
            &finding_addr,
            &cross_tenant_secret,
        ] {
            resolver.insert_object(
                ResolvedObject {
                    address: addr.clone(),
                    plane: addr.kind.plane(),
                    summary_digest: 42,
                    generation: 1,
                },
                basis,
            );
        }

        resolver.insert_edge(
            closure_addr.clone(),
            RelationKind::ClosureStrongRef,
            evidence_addr.clone(),
        );
        resolver.insert_edge(
            closure_addr.clone(),
            RelationKind::ThoughtGraphEdge,
            finding_addr.clone(),
        );
        resolver.insert_edge(
            closure_addr.clone(),
            RelationKind::SupportSetMember,
            cross_tenant_secret.clone(),
        );

        let caller_a = CallerScope::full_tenant_access("tenant-a");
        let related = resolver.fa_related(&caller_a, &closure_addr, None);
        let (edges, _) = related
            .as_known(ClaimClass::Invariant, 1)
            .expect("authorized source should return Known edge list");
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].target_resolution.variant_name(), "Known");
        assert_eq!(edges[1].target_resolution.variant_name(), "Known");
        // The cross-tenant edge's address and existence are private too.
        assert!(edges.iter().all(|edge| edge.to != cross_tenant_secret));
        assert!(resolver.logs().iter().all(|entry| {
            entry.requested_address != cross_tenant_secret.canonical_string()
        }));

        // Lint test: addressed event record passes; raw ID JSON fails.
        let addressed_event = AddressedEventRecord {
            event_address: Address::parse("fa://tenant-a/receipt/rc-1@1").unwrap(),
            subject_address: closure_addr,
            verb: "verify".to_owned(),
            would_change_this: None,
        };
        assert!(lint_no_raw_ids_in_agent_output(&addressed_event.to_agent_json()).is_ok());
        assert!(
            lint_no_raw_ids_in_agent_output("{\"run_id\":\"raw-123\",\"status\":\"ok\"}").is_err()
        );
    }
}
