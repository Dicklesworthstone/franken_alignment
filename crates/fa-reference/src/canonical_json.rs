//! Canonical JSON compatibility profile for the three retained draft schemas.
//!
//! This is a bounded reference-format contract. It neither authenticates a
//! document nor authorizes an action, persists a record, or activates a
//! production transport.

use std::collections::{BTreeMap, BTreeSet};

use crate::strict_json::{ErrorKind, Json, Limits, parse};

pub const MAX_DOCUMENT_BYTES: usize = 512 * 1024;
const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemaVariant {
    ActionProposalV01,
    CapabilityManifestV01,
    EvidenceClaimV01,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DraftDocument {
    ActionProposal(ActionProposalV01),
    CapabilityManifest(CapabilityManifestV01),
    EvidenceClaim(EvidenceClaimV01),
}

impl DraftDocument {
    pub fn schema_variant(&self) -> SchemaVariant {
        match self {
            Self::ActionProposal(_) => SchemaVariant::ActionProposalV01,
            Self::CapabilityManifest(_) => SchemaVariant::CapabilityManifestV01,
            Self::EvidenceClaim(_) => SchemaVariant::EvidenceClaimV01,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// A validated action proposal. Its fields are deliberately opaque: construct
/// it through `validate_json` or `decode_canonical`, then retain/compare/encode
/// the resulting typed document.
///
/// ```compile_fail
/// use fa_reference::canonical_json::{ActionProposalV01, Branch};
/// let _ = ActionProposalV01 {
///     action_id: "act-1".into(), tenant_id: "tenant-1".into(), run_id: "run-1".into(),
///     branch: Branch::Production, effect_kind: "publish".into(),
///     adapter_id: "adapter-1".into(), target_binding: "target-1".into(),
///     payload_sha256: "a".repeat(64), payload_bytes: 0, units: 1,
///     policy_epoch: 1, required_observations: vec![],
/// };
/// ```
///
/// ```compile_fail
/// use fa_reference::canonical_json::ActionProposalV01;
/// fn corrupt(mut action: ActionProposalV01) {
/// action.units = 0;
/// }
/// ```
pub struct ActionProposalV01 {
    action_id: String,
    tenant_id: String,
    run_id: String,
    branch: Branch,
    effect_kind: String,
    adapter_id: String,
    target_binding: String,
    payload_sha256: String,
    payload_bytes: u64,
    units: u64,
    policy_epoch: u64,
    required_observations: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Branch {
    Production,
    Sandbox,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapabilityManifestV01 {
    adapter_id: String,
    status: ManifestStatus,
    mediation_mode: ManifestMediation,
    activation_capture: bool,
    restart_grades: Vec<RestartGrade>,
    effect_families: Vec<String>,
    limitations: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManifestStatus {
    ExampleOnly,
    TestedProfile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManifestMediation {
    ObserveOnly,
    CooperativeGate,
    BrokeredEffects,
    AttestedProfile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestartGrade {
    Unavailable,
    DecisionReplay,
    TraceReplay,
    FunctionalRestart,
    ExactRestartProfile,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvidenceClaimV01 {
    claim_id: String,
    claim_class: ClaimClass,
    status: ClaimStatus,
    assertion: String,
    scope: String,
    assumptions: Vec<String>,
    limitations: Vec<String>,
    source_refs: Vec<String>,
    evidence_refs: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimClass {
    Invariant,
    Proof,
    BoundedModel,
    Statistical,
    Benchmark,
    Slo,
    Hypothesis,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimStatus {
    Proposed,
    ObservedInDeclaredScope,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CanonicalError {
    Syntax,
    Truncated,
    DuplicateKey(String),
    InputLimit,
    MissingField(&'static str),
    UnknownCriticalField(String),
    WrongType(&'static str),
    WrongSchemaVersion(String),
    InvalidValue(&'static str),
    IntegerOverflow(&'static str),
    IntegerOutOfRange(&'static str),
    DuplicateArrayValue(&'static str),
    NonCanonical,
}

/// Validate an already strictly parsed syntax tree against one exact draft
/// schema. This function deliberately does not parse bytes or normalize input.
pub fn validate_json(value: &Json) -> Result<DraftDocument, CanonicalError> {
    let object = object(value)?;
    let version = string_field(object, "schema_version")?;
    match version {
        "fa.action/0.1" => validate_action(object).map(DraftDocument::ActionProposal),
        "fa.capabilities/0.1" => validate_manifest(object).map(DraftDocument::CapabilityManifest),
        "fa.claim/0.1" => validate_claim(object).map(DraftDocument::EvidenceClaim),
        other => Err(CanonicalError::WrongSchemaVersion(other.to_owned())),
    }
}

/// Return exact canonical UTF-8 JSON bytes for a previously validated draft.
#[must_use]
pub fn encode(document: &DraftDocument) -> Vec<u8> {
    let mut output = String::new();
    match document {
        DraftDocument::ActionProposal(value) => encode_action(&mut output, value),
        DraftDocument::CapabilityManifest(value) => encode_manifest(&mut output, value),
        DraftDocument::EvidenceClaim(value) => encode_claim(&mut output, value),
    }
    output.into_bytes()
}

/// Parse, validate, and require byte-for-byte canonical JSON.
pub fn decode_canonical(bytes: &[u8]) -> Result<DraftDocument, CanonicalError> {
    let value = parse(bytes, limits()).map_err(map_syntax_error)?;
    let document = validate_json(&value)?;
    (encode(&document).as_slice() == bytes)
        .then_some(document)
        .ok_or(CanonicalError::NonCanonical)
}

fn limits() -> Limits {
    Limits {
        max_bytes: MAX_DOCUMENT_BYTES,
        max_depth: 8,
        max_items: 1_024,
        max_string_bytes: 8_192,
    }
}

fn validate_action(object: &BTreeMap<String, Json>) -> Result<ActionProposalV01, CanonicalError> {
    fields(object, ACTION_FIELDS)?;
    let branch = branch(string_field(object, "branch")?)?;
    let required_observations = identifier_array(object, "required_observations", 16)?;
    Ok(ActionProposalV01 {
        action_id: identifier_field(object, "action_id")?,
        tenant_id: identifier_field(object, "tenant_id")?,
        run_id: identifier_field(object, "run_id")?,
        branch,
        effect_kind: identifier_field(object, "effect_kind")?,
        adapter_id: identifier_field(object, "adapter_id")?,
        target_binding: identifier_field(object, "target_binding")?,
        payload_sha256: digest_field(object, "payload_sha256")?,
        payload_bytes: integer_field(object, "payload_bytes", 0, 65_536)?,
        units: integer_field(object, "units", 1, MAX_SAFE_JSON_INTEGER)?,
        policy_epoch: integer_field(object, "policy_epoch", 1, MAX_SAFE_JSON_INTEGER)?,
        required_observations,
    })
}

fn validate_manifest(
    object: &BTreeMap<String, Json>,
) -> Result<CapabilityManifestV01, CanonicalError> {
    fields(object, MANIFEST_FIELDS)?;
    Ok(CapabilityManifestV01 {
        adapter_id: identifier_field(object, "adapter_id")?,
        status: manifest_status(string_field(object, "status")?)?,
        mediation_mode: manifest_mediation(string_field(object, "mediation_mode")?)?,
        activation_capture: bool_field(object, "activation_capture")?,
        restart_grades: restart_grades(object)?,
        effect_families: identifier_array(object, "effect_families", 128)?,
        limitations: text_array(object, "limitations", 1, 32, 1_024, false)?,
    })
}

fn validate_claim(object: &BTreeMap<String, Json>) -> Result<EvidenceClaimV01, CanonicalError> {
    fields(object, CLAIM_FIELDS)?;
    Ok(EvidenceClaimV01 {
        claim_id: identifier_field(object, "claim_id")?,
        claim_class: claim_class(string_field(object, "claim_class")?)?,
        status: claim_status(string_field(object, "status")?)?,
        assertion: text_field(object, "assertion", 2_048)?,
        scope: text_field(object, "scope", 2_048)?,
        assumptions: text_array(object, "assumptions", 1, 32, 1_024, false)?,
        limitations: text_array(object, "limitations", 1, 32, 1_024, false)?,
        source_refs: identifier_array(object, "source_refs", 64)?,
        evidence_refs: digest_array(object, "evidence_refs", 64)?,
    })
}

const ACTION_FIELDS: &[&str] = &[
    "schema_version",
    "action_id",
    "tenant_id",
    "run_id",
    "branch",
    "effect_kind",
    "adapter_id",
    "target_binding",
    "payload_sha256",
    "payload_bytes",
    "units",
    "policy_epoch",
    "required_observations",
];
const MANIFEST_FIELDS: &[&str] = &[
    "schema_version",
    "adapter_id",
    "status",
    "mediation_mode",
    "activation_capture",
    "restart_grades",
    "effect_families",
    "limitations",
];
const CLAIM_FIELDS: &[&str] = &[
    "schema_version",
    "claim_id",
    "claim_class",
    "status",
    "assertion",
    "scope",
    "assumptions",
    "limitations",
    "source_refs",
    "evidence_refs",
];

fn object(value: &Json) -> Result<&BTreeMap<String, Json>, CanonicalError> {
    value.as_object().ok_or(CanonicalError::WrongType("root"))
}

fn fields(
    object: &BTreeMap<String, Json>,
    expected: &[&'static str],
) -> Result<(), CanonicalError> {
    for key in object.keys() {
        if !expected.contains(&key.as_str()) {
            return Err(CanonicalError::UnknownCriticalField(key.clone()));
        }
    }
    for key in expected {
        if !object.contains_key(*key) {
            return Err(CanonicalError::MissingField(key));
        }
    }
    Ok(())
}

fn string_field<'a>(
    object: &'a BTreeMap<String, Json>,
    key: &'static str,
) -> Result<&'a str, CanonicalError> {
    object
        .get(key)
        .ok_or(CanonicalError::MissingField(key))?
        .as_str()
        .ok_or(CanonicalError::WrongType(key))
}

fn bool_field(object: &BTreeMap<String, Json>, key: &'static str) -> Result<bool, CanonicalError> {
    object
        .get(key)
        .and_then(Json::as_bool)
        .ok_or(CanonicalError::WrongType(key))
}

fn text_field(
    object: &BTreeMap<String, Json>,
    key: &'static str,
    maximum: usize,
) -> Result<String, CanonicalError> {
    let value = string_field(object, key)?;
    valid_text(value, maximum)
        .then_some(value.to_owned())
        .ok_or(CanonicalError::InvalidValue(key))
}

fn identifier_field(
    object: &BTreeMap<String, Json>,
    key: &'static str,
) -> Result<String, CanonicalError> {
    let value = string_field(object, key)?;
    valid_identifier(value)
        .then_some(value.to_owned())
        .ok_or(CanonicalError::InvalidValue(key))
}

fn digest_field(
    object: &BTreeMap<String, Json>,
    key: &'static str,
) -> Result<String, CanonicalError> {
    let value = string_field(object, key)?;
    valid_digest(value)
        .then_some(value.to_owned())
        .ok_or(CanonicalError::InvalidValue(key))
}

fn integer_field(
    object: &BTreeMap<String, Json>,
    key: &'static str,
    minimum: u64,
    maximum: u64,
) -> Result<u64, CanonicalError> {
    let value = object.get(key).ok_or(CanonicalError::MissingField(key))?;
    let Json::Number(number) = value else {
        return Err(CanonicalError::WrongType(key));
    };
    let lexeme = number.lexeme();
    if !canonical_uint(lexeme) {
        return Err(CanonicalError::InvalidValue(key));
    }
    let integer = lexeme
        .parse::<u64>()
        .map_err(|_| CanonicalError::IntegerOverflow(key))?;
    (minimum..=maximum)
        .contains(&integer)
        .then_some(integer)
        .ok_or(CanonicalError::IntegerOutOfRange(key))
}

fn identifier_array(
    object: &BTreeMap<String, Json>,
    key: &'static str,
    maximum: usize,
) -> Result<Vec<String>, CanonicalError> {
    array_strings(object, key, 0, maximum, valid_identifier, true)
}

fn digest_array(
    object: &BTreeMap<String, Json>,
    key: &'static str,
    maximum: usize,
) -> Result<Vec<String>, CanonicalError> {
    array_strings(object, key, 0, maximum, valid_digest, true)
}

fn text_array(
    object: &BTreeMap<String, Json>,
    key: &'static str,
    minimum: usize,
    maximum: usize,
    text_maximum: usize,
    unique: bool,
) -> Result<Vec<String>, CanonicalError> {
    array_strings(
        object,
        key,
        minimum,
        maximum,
        |value| valid_text(value, text_maximum),
        unique,
    )
}

fn array_strings<F>(
    object: &BTreeMap<String, Json>,
    key: &'static str,
    minimum: usize,
    maximum: usize,
    validate: F,
    unique: bool,
) -> Result<Vec<String>, CanonicalError>
where
    F: Fn(&str) -> bool,
{
    let values = object
        .get(key)
        .and_then(Json::as_array)
        .ok_or(CanonicalError::WrongType(key))?;
    if !(minimum..=maximum).contains(&values.len()) {
        return Err(CanonicalError::IntegerOutOfRange(key));
    }
    let mut result = Vec::with_capacity(values.len());
    let mut seen = BTreeSet::new();
    for value in values {
        let value = value.as_str().ok_or(CanonicalError::WrongType(key))?;
        if !validate(value) {
            return Err(CanonicalError::InvalidValue(key));
        }
        if unique && !seen.insert(value) {
            return Err(CanonicalError::DuplicateArrayValue(key));
        }
        result.push(value.to_owned());
    }
    Ok(result)
}

fn restart_grades(object: &BTreeMap<String, Json>) -> Result<Vec<RestartGrade>, CanonicalError> {
    let values = object
        .get("restart_grades")
        .and_then(Json::as_array)
        .ok_or(CanonicalError::WrongType("restart_grades"))?;
    if !(1..=5).contains(&values.len()) {
        return Err(CanonicalError::IntegerOutOfRange("restart_grades"));
    }
    let mut result = Vec::with_capacity(values.len());
    let mut seen = BTreeSet::new();
    for value in values {
        let value = value
            .as_str()
            .ok_or(CanonicalError::WrongType("restart_grades"))?;
        if !seen.insert(value) {
            return Err(CanonicalError::DuplicateArrayValue("restart_grades"));
        }
        result.push(restart_grade(value)?);
    }
    Ok(result)
}

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.chars().count() <= maximum
}
fn valid_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    matches!(characters.next(), Some(character) if character.is_ascii_alphanumeric())
        && value.chars().count() <= 128
        && characters.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '.' | ':' | '/' | '-')
        })
}
fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}
fn canonical_uint(value: &str) -> bool {
    value == "0" || (!value.starts_with('0') && value.bytes().all(|byte| byte.is_ascii_digit()))
}

fn branch(value: &str) -> Result<Branch, CanonicalError> {
    match value {
        "production" => Ok(Branch::Production),
        "sandbox" => Ok(Branch::Sandbox),
        _ => Err(CanonicalError::InvalidValue("branch")),
    }
}
fn manifest_status(value: &str) -> Result<ManifestStatus, CanonicalError> {
    match value {
        "example_only" => Ok(ManifestStatus::ExampleOnly),
        "tested_profile" => Ok(ManifestStatus::TestedProfile),
        _ => Err(CanonicalError::InvalidValue("status")),
    }
}
fn manifest_mediation(value: &str) -> Result<ManifestMediation, CanonicalError> {
    match value {
        "observe_only" => Ok(ManifestMediation::ObserveOnly),
        "cooperative_gate" => Ok(ManifestMediation::CooperativeGate),
        "brokered_effects" => Ok(ManifestMediation::BrokeredEffects),
        "attested_profile" => Ok(ManifestMediation::AttestedProfile),
        _ => Err(CanonicalError::InvalidValue("mediation_mode")),
    }
}
fn restart_grade(value: &str) -> Result<RestartGrade, CanonicalError> {
    match value {
        "unavailable" => Ok(RestartGrade::Unavailable),
        "decision_replay" => Ok(RestartGrade::DecisionReplay),
        "trace_replay" => Ok(RestartGrade::TraceReplay),
        "functional_restart" => Ok(RestartGrade::FunctionalRestart),
        "exact_restart_profile" => Ok(RestartGrade::ExactRestartProfile),
        _ => Err(CanonicalError::InvalidValue("restart_grades")),
    }
}
fn claim_class(value: &str) -> Result<ClaimClass, CanonicalError> {
    match value {
        "invariant" => Ok(ClaimClass::Invariant),
        "proof" => Ok(ClaimClass::Proof),
        "bounded_model" => Ok(ClaimClass::BoundedModel),
        "statistical" => Ok(ClaimClass::Statistical),
        "benchmark" => Ok(ClaimClass::Benchmark),
        "slo" => Ok(ClaimClass::Slo),
        "hypothesis" => Ok(ClaimClass::Hypothesis),
        _ => Err(CanonicalError::InvalidValue("claim_class")),
    }
}
fn claim_status(value: &str) -> Result<ClaimStatus, CanonicalError> {
    match value {
        "proposed" => Ok(ClaimStatus::Proposed),
        "observed_in_declared_scope" => Ok(ClaimStatus::ObservedInDeclaredScope),
        _ => Err(CanonicalError::InvalidValue("status")),
    }
}

fn encode_action(output: &mut String, value: &ActionProposalV01) {
    object_start(output);
    field_string(output, "action_id", &value.action_id);
    comma(output);
    field_string(output, "adapter_id", &value.adapter_id);
    comma(output);
    field_string(
        output,
        "branch",
        match value.branch {
            Branch::Production => "production",
            Branch::Sandbox => "sandbox",
        },
    );
    comma(output);
    field_string(output, "effect_kind", &value.effect_kind);
    comma(output);
    field_uint(output, "payload_bytes", value.payload_bytes);
    comma(output);
    field_string(output, "payload_sha256", &value.payload_sha256);
    comma(output);
    field_uint(output, "policy_epoch", value.policy_epoch);
    comma(output);
    field_strings(
        output,
        "required_observations",
        &value.required_observations,
    );
    comma(output);
    field_string(output, "run_id", &value.run_id);
    comma(output);
    field_string(output, "schema_version", "fa.action/0.1");
    comma(output);
    field_string(output, "target_binding", &value.target_binding);
    comma(output);
    field_string(output, "tenant_id", &value.tenant_id);
    comma(output);
    field_uint(output, "units", value.units);
    object_end(output);
}

fn encode_manifest(output: &mut String, value: &CapabilityManifestV01) {
    object_start(output);
    field_bool(output, "activation_capture", value.activation_capture);
    comma(output);
    field_string(output, "adapter_id", &value.adapter_id);
    comma(output);
    field_strings(output, "effect_families", &value.effect_families);
    comma(output);
    field_strings(output, "limitations", &value.limitations);
    comma(output);
    field_string(
        output,
        "mediation_mode",
        manifest_mediation_name(value.mediation_mode),
    );
    comma(output);
    field_restart_grades(output, "restart_grades", &value.restart_grades);
    comma(output);
    field_string(output, "schema_version", "fa.capabilities/0.1");
    comma(output);
    field_string(output, "status", manifest_status_name(value.status));
    object_end(output);
}

fn encode_claim(output: &mut String, value: &EvidenceClaimV01) {
    object_start(output);
    field_string(output, "assertion", &value.assertion);
    comma(output);
    field_strings(output, "assumptions", &value.assumptions);
    comma(output);
    field_string(output, "claim_class", claim_class_name(value.claim_class));
    comma(output);
    field_string(output, "claim_id", &value.claim_id);
    comma(output);
    field_strings(output, "evidence_refs", &value.evidence_refs);
    comma(output);
    field_strings(output, "limitations", &value.limitations);
    comma(output);
    field_string(output, "schema_version", "fa.claim/0.1");
    comma(output);
    field_string(output, "scope", &value.scope);
    comma(output);
    field_strings(output, "source_refs", &value.source_refs);
    comma(output);
    field_string(output, "status", claim_status_name(value.status));
    object_end(output);
}

fn object_start(output: &mut String) {
    output.push('{');
}
fn object_end(output: &mut String) {
    output.push('}');
}
fn comma(output: &mut String) {
    output.push(',');
}
fn field_name(output: &mut String, name: &str) {
    json_string(output, name);
    output.push(':');
}
fn field_string(output: &mut String, name: &str, value: &str) {
    field_name(output, name);
    json_string(output, value);
}
fn field_uint(output: &mut String, name: &str, value: u64) {
    field_name(output, name);
    output.push_str(&value.to_string());
}
fn field_bool(output: &mut String, name: &str, value: bool) {
    field_name(output, name);
    output.push_str(if value { "true" } else { "false" });
}
fn field_strings(output: &mut String, name: &str, values: &[String]) {
    field_name(output, name);
    output.push('[');
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            comma(output);
        }
        json_string(output, value);
    }
    output.push(']');
}
fn field_restart_grades(output: &mut String, name: &str, values: &[RestartGrade]) {
    field_name(output, name);
    output.push('[');
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            comma(output);
        }
        json_string(output, restart_grade_name(*value));
    }
    output.push(']');
}

fn json_string(output: &mut String, value: &str) {
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{0008}' => output.push_str("\\b"),
            '\u{000C}' => output.push_str("\\f"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character <= '\u{001F}' => {
                use std::fmt::Write;
                let _ = write!(output, "\\u{:04x}", character as u32);
            }
            character => output.push(character),
        }
    }
    output.push('"');
}

fn manifest_status_name(value: ManifestStatus) -> &'static str {
    match value {
        ManifestStatus::ExampleOnly => "example_only",
        ManifestStatus::TestedProfile => "tested_profile",
    }
}
fn manifest_mediation_name(value: ManifestMediation) -> &'static str {
    match value {
        ManifestMediation::ObserveOnly => "observe_only",
        ManifestMediation::CooperativeGate => "cooperative_gate",
        ManifestMediation::BrokeredEffects => "brokered_effects",
        ManifestMediation::AttestedProfile => "attested_profile",
    }
}
fn restart_grade_name(value: RestartGrade) -> &'static str {
    match value {
        RestartGrade::Unavailable => "unavailable",
        RestartGrade::DecisionReplay => "decision_replay",
        RestartGrade::TraceReplay => "trace_replay",
        RestartGrade::FunctionalRestart => "functional_restart",
        RestartGrade::ExactRestartProfile => "exact_restart_profile",
    }
}
fn claim_class_name(value: ClaimClass) -> &'static str {
    match value {
        ClaimClass::Invariant => "invariant",
        ClaimClass::Proof => "proof",
        ClaimClass::BoundedModel => "bounded_model",
        ClaimClass::Statistical => "statistical",
        ClaimClass::Benchmark => "benchmark",
        ClaimClass::Slo => "slo",
        ClaimClass::Hypothesis => "hypothesis",
    }
}
fn claim_status_name(value: ClaimStatus) -> &'static str {
    match value {
        ClaimStatus::Proposed => "proposed",
        ClaimStatus::ObservedInDeclaredScope => "observed_in_declared_scope",
    }
}

fn map_syntax_error(error: crate::strict_json::Error) -> CanonicalError {
    match error.kind {
        ErrorKind::UnexpectedEof => CanonicalError::Truncated,
        ErrorKind::DuplicateKey(key) => CanonicalError::DuplicateKey(key),
        ErrorKind::DepthLimit
        | ErrorKind::SizeLimit
        | ErrorKind::ItemLimit
        | ErrorKind::StringLimit => CanonicalError::InputLimit,
        _ => CanonicalError::Syntax,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_valid(bytes: &[u8]) -> Json {
        parse(bytes, limits()).unwrap()
    }

    const ACTION: &str = r#"{"action_id":"act-1","adapter_id":"adapter-1","branch":"production","effect_kind":"publish","payload_bytes":0,"payload_sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","policy_epoch":1,"required_observations":["obs-1"],"run_id":"run-1","schema_version":"fa.action/0.1","target_binding":"target-1","tenant_id":"tenant-1","units":1}"#;
    const MANIFEST: &str = r#"{"activation_capture":false,"adapter_id":"adapter-1","effect_families":["publish"],"limitations":["évidence is scoped"],"mediation_mode":"observe_only","restart_grades":["unavailable"],"schema_version":"fa.capabilities/0.1","status":"example_only"}"#;
    const CLAIM: &str = r#"{"assertion":"line\n ","assumptions":["bounded"],"claim_class":"bounded_model","claim_id":"claim-1","evidence_refs":["0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"],"limitations":["no authority"],"schema_version":"fa.claim/0.1","scope":"tenant-1","source_refs":["source-1"],"status":"proposed"}"#;

    #[test]
    fn all_three_fixed_canonical_vectors_round_trip_without_normalization() {
        for bytes in [ACTION.as_bytes(), MANIFEST.as_bytes(), CLAIM.as_bytes()] {
            let document = decode_canonical(bytes).unwrap();
            assert_eq!(encode(&document), bytes);
            assert_eq!(validate_json(&parse_valid(bytes)).unwrap(), document);
        }
    }

    #[test]
    fn duplicate_unknown_noncanonical_truncated_and_overflow_inputs_refuse_causally() {
        assert!(matches!(
            decode_canonical(
                br#"{"schema_version":"fa.action/0.1","schema_version":"fa.action/0.1"}"#
            ),
            Err(CanonicalError::DuplicateKey(_))
        ));
        let unknown = ACTION.replace('}', ",\"unknown\":1}");
        assert_eq!(
            decode_canonical(unknown.as_bytes()),
            Err(CanonicalError::UnknownCriticalField("unknown".to_owned()))
        );
        assert_eq!(
            decode_canonical(b"{\"schema_version\":"),
            Err(CanonicalError::Truncated)
        );
        let overflow = ACTION.replace("\"units\":1", "\"units\":18446744073709551616");
        assert_eq!(
            decode_canonical(overflow.as_bytes()),
            Err(CanonicalError::IntegerOverflow("units"))
        );
        let reordered = ACTION.replace("{\"action_id\"", "{ \"action_id\"");
        assert_eq!(
            decode_canonical(reordered.as_bytes()),
            Err(CanonicalError::NonCanonical)
        );
    }

    #[test]
    fn schema_permitted_identifier_arrays_may_be_empty() {
        let action = ACTION.replace("[\"obs-1\"]", "[]");
        let manifest = MANIFEST.replace("[\"publish\"]", "[]");
        let claim = CLAIM.replace("[\"source-1\"]", "[]");
        for bytes in [action.as_bytes(), manifest.as_bytes(), claim.as_bytes()] {
            assert!(decode_canonical(bytes).is_ok());
        }
    }

    #[test]
    fn unicode_scalar_length_is_not_raw_byte_length_and_identifier_stays_ascii() {
        let scalar_boundary = CLAIM.replace("line\\n ", &"é".repeat(2_048));
        assert!(decode_canonical(scalar_boundary.as_bytes()).is_ok());
        let non_ascii_identifier =
            scalar_boundary.replace("\"claim_id\":\"claim-1\"", "\"claim_id\":\"é\"");
        assert_eq!(
            validate_json(&parse_valid(non_ascii_identifier.as_bytes())),
            Err(CanonicalError::InvalidValue("claim_id"))
        );
    }
}
