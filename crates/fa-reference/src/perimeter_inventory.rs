//! Bounded loading for a complete declared effect-perimeter inventory.
//!
//! [`LoadedPerimeterInventory`] is a checked in-memory configuration object.
//! It does not authenticate an operator, prevent an effect, broker a credential,
//! or make the existing [`PerimeterInventory`] classification model a complete
//! production perimeter. `load_path` rejects non-regular inputs before reading,
//! but filesystem replacement after metadata inspection and operator-input
//! authenticity remain explicit outer-boundary concerns.

use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Read,
    path::{Component, Path},
};

use crate::{
    Error,
    perimeter::{
        BypassDisposition, CredentialHolder, EffectFamilyRecord, MAX_FAMILIES,
        MAX_FAMILY_TEXT_BYTES, MAX_RESIDUAL_NONCLAIMS, MAX_ROUTES_PER_FAMILY, MAX_TEXT_BYTES,
        Mediation, PerimeterInventory, PerimeterScope, RouteRecord, ThreatClass, TrustDomain,
    },
    strict_json::{self, Json, Limits},
};

/// Maximum bytes admitted from a single operator inventory before JSON parsing.
pub const MAX_INVENTORY_BYTES: usize = 1024 * 1024;
/// Maximum explicit route residual nonclaims.
pub const MAX_ROUTE_NONCLAIMS: usize = 8;
/// Maximum named trust domains in one declared route path.
pub const MAX_ROUTE_TRUST_DOMAINS: usize = 4;
const MAX_INVENTORY_JSON_ITEMS: usize = 131_072;
const INVENTORY_SCHEMA_VERSION: u64 = 1;

/// Closed categories for every effect route named by plan §4.4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectKind {
    HumanVisibleMessage,
    Download,
    Image,
    NetworkRequest,
    FileWrite,
    CodeExecution,
    DatabaseCommit,
    ProcessCreation,
    ActuatorCommand,
    InterAgentMessage,
    ResourceAllocation,
}

/// An exact declared profile identifier and generation for a route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteProfile {
    id: String,
    generation: u64,
}

impl RouteProfile {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }
}

/// The actor-side credential disposition declared for one route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActorCredentialDisposition {
    /// This route declares that no actor-held credential is part of its profile.
    NoneDeclared,
    /// This route is mediated by a declared family broker credential.
    BrokerMediated,
    /// An actor directly holds this named declared credential.
    ActorDirect { credential: String },
}

/// Mandatory metadata retained alongside every classified route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteMetadata {
    effect: EffectKind,
    profile: RouteProfile,
    trust_path: Vec<TrustDomain>,
    threat: ThreatClass,
    actor_credential: ActorCredentialDisposition,
    residual_nonclaims: Vec<String>,
}

impl RouteMetadata {
    #[must_use]
    pub const fn effect(&self) -> EffectKind {
        self.effect
    }

    #[must_use]
    pub fn profile(&self) -> &RouteProfile {
        &self.profile
    }

    #[must_use]
    pub fn trust_path(&self) -> &[TrustDomain] {
        &self.trust_path
    }

    #[must_use]
    pub const fn threat(&self) -> ThreatClass {
        self.threat
    }

    #[must_use]
    pub fn actor_credential(&self) -> &ActorCredentialDisposition {
        &self.actor_credential
    }

    #[must_use]
    pub fn residual_nonclaims(&self) -> &[String] {
        &self.residual_nonclaims
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RouteKey {
    scope: PerimeterScope,
    family: String,
    route: String,
}

/// An immutable route lookup that keeps classification and mandatory metadata
/// together. It is a declaration for a future policy consumer, never a permit.
#[derive(Clone, Copy, Debug)]
pub struct LoadedRoute<'a> {
    record: &'a RouteRecord,
    metadata: &'a RouteMetadata,
}

impl<'a> LoadedRoute<'a> {
    #[must_use]
    pub const fn record(&self) -> &'a RouteRecord {
        self.record
    }

    #[must_use]
    pub const fn metadata(&self) -> &'a RouteMetadata {
        self.metadata
    }
}

/// A fully populated strict-JSON inventory. Fields are private so a caller
/// cannot construct a loaded inventory with an unpaired route metadata record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedPerimeterInventory {
    inventory: PerimeterInventory,
    metadata: BTreeMap<RouteKey, RouteMetadata>,
}

/// Failures at the operator-input boundary. `Unreadable` intentionally does
/// not assert why a path failed or that a path was absent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InventoryLoadError {
    Unreadable,
    Parse(strict_json::Error),
    Schema(Error),
}

impl LoadedPerimeterInventory {
    /// Reads a bounded, regular operator file and decodes the exact schema.
    ///
    /// No recursive directory traversal is performed. Parent-directory
    /// components, directories, symlinks, FIFOs, devices, and other nonregular
    /// inputs are refused rather than opened as a potentially blocking stream.
    pub fn load_path(path: &Path) -> Result<Self, InventoryLoadError> {
        if path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(InventoryLoadError::Unreadable);
        }
        let metadata = fs::symlink_metadata(path).map_err(|_| InventoryLoadError::Unreadable)?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err(InventoryLoadError::Unreadable);
        }
        let file = File::open(path).map_err(|_| InventoryLoadError::Unreadable)?;
        if !file
            .metadata()
            .map_err(|_| InventoryLoadError::Unreadable)?
            .file_type()
            .is_file()
        {
            return Err(InventoryLoadError::Unreadable);
        }
        let mut bytes = Vec::new();
        file.take((MAX_INVENTORY_BYTES as u64) + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| InventoryLoadError::Unreadable)?;
        if bytes.len() > MAX_INVENTORY_BYTES {
            return Err(InventoryLoadError::Schema(Error::Limit));
        }
        Self::from_json_bytes(&bytes)
    }

    /// Parses already obtained operator bytes under the same bounded schema.
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, InventoryLoadError> {
        if bytes.len() > MAX_INVENTORY_BYTES {
            return Err(InventoryLoadError::Schema(Error::Limit));
        }
        let parsed = strict_json::parse(
            bytes,
            Limits {
                max_bytes: MAX_INVENTORY_BYTES,
                max_depth: 8,
                max_items: MAX_INVENTORY_JSON_ITEMS,
                max_string_bytes: MAX_TEXT_BYTES,
            },
        )
        .map_err(InventoryLoadError::Parse)?;
        Self::from_json(parsed)
    }

    /// Performs the exact existing scope/family/route classification lookup
    /// and returns its required loaded metadata with it.
    pub fn route_for(
        &self,
        scope: PerimeterScope,
        family: &str,
        route: &str,
    ) -> Result<LoadedRoute<'_>, Error> {
        let record = self.inventory.route_for(scope, family, route)?;
        let key = RouteKey {
            scope,
            family: family.to_owned(),
            route: route.to_owned(),
        };
        let metadata = self.metadata.get(&key).ok_or(Error::Binding)?;
        Ok(LoadedRoute { record, metadata })
    }

    /// Count declared families without exposing a metadata-free lookup path.
    #[must_use]
    pub fn family_count(&self) -> usize {
        self.inventory.records().len()
    }

    fn from_json(value: Json) -> Result<Self, InventoryLoadError> {
        let root = object(&value)?;
        exact_fields(root, &["version", "families"])?;
        if unsigned(required(root, "version")?)? != INVENTORY_SCHEMA_VERSION {
            return Err(schema(Error::InvalidInput));
        }
        let families = array(required(root, "families")?)?;
        if families.is_empty() {
            return Err(schema(Error::InvalidInput));
        }
        if families.len() > MAX_FAMILIES {
            return Err(schema(Error::Limit));
        }

        let mut records = Vec::with_capacity(families.len());
        let mut metadata = BTreeMap::new();
        for family in families {
            let (record, route_metadata) = parse_family(family)?;
            for (route, metadata_record) in record.routes.iter().zip(route_metadata) {
                let key = RouteKey {
                    scope: record.scope,
                    family: record.family.clone(),
                    route: route.route.clone(),
                };
                if metadata.insert(key, metadata_record).is_some() {
                    return Err(schema(Error::Duplicate));
                }
            }
            records.push(record);
        }
        let inventory = PerimeterInventory::new(records).map_err(InventoryLoadError::Schema)?;
        let expected_routes = inventory
            .records()
            .iter()
            .map(|record| record.routes.len())
            .sum::<usize>();
        if metadata.len() != expected_routes {
            return Err(schema(Error::Binding));
        }
        Ok(Self {
            inventory,
            metadata,
        })
    }
}

fn parse_family(
    value: &Json,
) -> Result<(EffectFamilyRecord, Vec<RouteMetadata>), InventoryLoadError> {
    let value = object(value)?;
    exact_fields(
        value,
        &[
            "scope",
            "family",
            "trust_domains",
            "credentials",
            "routes",
            "residual_nonclaims",
        ],
    )?;
    let scope = parse_scope(required(value, "scope")?)?;
    let family = text(required(value, "family")?)?.to_owned();
    let trust_domains = parse_trust_domains(required(value, "trust_domains")?, true)?;
    let credentials = parse_credentials(required(value, "credentials")?)?;
    let residual_nonclaims = parse_texts(
        required(value, "residual_nonclaims")?,
        1,
        MAX_RESIDUAL_NONCLAIMS,
    )?;
    let routes = array(required(value, "routes")?)?;
    if routes.is_empty() {
        return Err(schema(Error::Incomplete));
    }
    if routes.len() > MAX_ROUTES_PER_FAMILY {
        return Err(schema(Error::Limit));
    }
    let mut records = Vec::with_capacity(routes.len());
    let mut metadata = Vec::with_capacity(routes.len());
    let mut total_text = family.len();
    for credential in &credentials {
        total_text = add_text(total_text, credential.credential.len())?;
    }
    for nonclaim in &residual_nonclaims {
        total_text = add_text(total_text, nonclaim.len())?;
    }
    for route in routes {
        let (record, route_metadata) = parse_route(route, &trust_domains, &credentials)?;
        total_text = add_text(total_text, record.route.len())?;
        total_text = add_text(total_text, route_metadata.profile.id.len())?;
        if let ActorCredentialDisposition::ActorDirect { credential } =
            &route_metadata.actor_credential
        {
            total_text = add_text(total_text, credential.len())?;
        }
        for nonclaim in &route_metadata.residual_nonclaims {
            total_text = add_text(total_text, nonclaim.len())?;
        }
        records.push(record);
        metadata.push(route_metadata);
    }
    Ok((
        EffectFamilyRecord {
            scope,
            family,
            trust_domains,
            credentials,
            routes: records,
            residual_nonclaims,
        },
        metadata,
    ))
}

fn parse_scope(value: &Json) -> Result<PerimeterScope, InventoryLoadError> {
    let value = object(value)?;
    exact_fields(value, &["tenant", "principal", "purpose"])?;
    Ok(PerimeterScope {
        tenant: unsigned(required(value, "tenant")?)?,
        principal: unsigned(required(value, "principal")?)?,
        purpose: unsigned(required(value, "purpose")?)?,
    })
}

fn parse_credentials(
    value: &Json,
) -> Result<Vec<crate::perimeter::CredentialExposure>, InventoryLoadError> {
    let values = array(value)?;
    if values.len() > crate::perimeter::MAX_CREDENTIALS_PER_FAMILY {
        return Err(schema(Error::Limit));
    }
    values
        .iter()
        .map(|value| {
            let value = object(value)?;
            exact_fields(value, &["credential", "holder"])?;
            let credential = text(required(value, "credential")?)?.to_owned();
            let holder = match text(required(value, "holder")?)? {
                "broker" => CredentialHolder::Broker,
                "actor_direct" => CredentialHolder::ActorDirect,
                _ => return Err(schema(Error::InvalidInput)),
            };
            Ok(crate::perimeter::CredentialExposure { credential, holder })
        })
        .collect()
}

fn parse_route(
    value: &Json,
    family_domains: &[TrustDomain],
    credentials: &[crate::perimeter::CredentialExposure],
) -> Result<(RouteRecord, RouteMetadata), InventoryLoadError> {
    let value = object(value)?;
    exact_fields(
        value,
        &[
            "route",
            "effect",
            "profile",
            "trust_path",
            "threat",
            "actor_credential",
            "mediation",
            "bypass",
            "residual_nonclaims",
        ],
    )?;
    let route = text(required(value, "route")?)?.to_owned();
    let threat = parse_threat(text(required(value, "threat")?)?)?;
    let mediation = parse_mediation(text(required(value, "mediation")?)?)?;
    let record = RouteRecord {
        route,
        threat: Some(threat),
        mediation,
        bypass: parse_bypass(text(required(value, "bypass")?)?)?,
    };
    let metadata = RouteMetadata {
        effect: parse_effect(text(required(value, "effect")?)?)?,
        profile: parse_profile(required(value, "profile")?)?,
        trust_path: parse_trust_domains(required(value, "trust_path")?, false)?,
        threat,
        actor_credential: parse_actor_credential(required(value, "actor_credential")?)?,
        residual_nonclaims: parse_texts(
            required(value, "residual_nonclaims")?,
            1,
            MAX_ROUTE_NONCLAIMS,
        )?,
    };
    validate_metadata(&record, &metadata, family_domains, credentials)?;
    Ok((record, metadata))
}

fn parse_profile(value: &Json) -> Result<RouteProfile, InventoryLoadError> {
    let value = object(value)?;
    exact_fields(value, &["id", "generation"])?;
    Ok(RouteProfile {
        id: text(required(value, "id")?)?.to_owned(),
        generation: unsigned(required(value, "generation")?)?,
    })
}

fn parse_actor_credential(value: &Json) -> Result<ActorCredentialDisposition, InventoryLoadError> {
    let value = object(value)?;
    let kind = text(required(value, "kind")?)?;
    match kind {
        "none_declared" => {
            exact_fields(value, &["kind"])?;
            Ok(ActorCredentialDisposition::NoneDeclared)
        }
        "broker_mediated" => {
            exact_fields(value, &["kind"])?;
            Ok(ActorCredentialDisposition::BrokerMediated)
        }
        "actor_direct" => {
            exact_fields(value, &["kind", "credential"])?;
            Ok(ActorCredentialDisposition::ActorDirect {
                credential: text(required(value, "credential")?)?.to_owned(),
            })
        }
        _ => Err(schema(Error::InvalidInput)),
    }
}

fn parse_trust_domains(
    value: &Json,
    complete_family: bool,
) -> Result<Vec<TrustDomain>, InventoryLoadError> {
    let values = array(value)?;
    let minimum = if complete_family { 4 } else { 1 };
    let maximum = if complete_family {
        4
    } else {
        MAX_ROUTE_TRUST_DOMAINS
    };
    if values.len() < minimum {
        return Err(schema(Error::Incomplete));
    }
    if values.len() > maximum {
        return Err(schema(Error::Limit));
    }
    let mut domains = Vec::with_capacity(values.len());
    for value in values {
        let domain = match text(value)? {
            "actor" => TrustDomain::Actor,
            "observation_and_analysis" => TrustDomain::ObservationAndAnalysis,
            "enforcement" => TrustDomain::Enforcement,
            "governance_and_investigation" => TrustDomain::GovernanceAndInvestigation,
            _ => return Err(schema(Error::InvalidInput)),
        };
        if domains.contains(&domain) {
            return Err(schema(Error::Duplicate));
        }
        domains.push(domain);
    }
    Ok(domains)
}

fn parse_texts(
    value: &Json,
    minimum: usize,
    maximum: usize,
) -> Result<Vec<String>, InventoryLoadError> {
    let values = array(value)?;
    if values.len() < minimum {
        return Err(schema(Error::Incomplete));
    }
    if values.len() > maximum {
        return Err(schema(Error::Limit));
    }
    values
        .iter()
        .map(|value| Ok(text(value)?.to_owned()))
        .collect()
}

fn parse_effect(value: &str) -> Result<EffectKind, InventoryLoadError> {
    match value {
        "human_visible_message" => Ok(EffectKind::HumanVisibleMessage),
        "download" => Ok(EffectKind::Download),
        "image" => Ok(EffectKind::Image),
        "network_request" => Ok(EffectKind::NetworkRequest),
        "file_write" => Ok(EffectKind::FileWrite),
        "code_execution" => Ok(EffectKind::CodeExecution),
        "database_commit" => Ok(EffectKind::DatabaseCommit),
        "process_creation" => Ok(EffectKind::ProcessCreation),
        "actuator_command" => Ok(EffectKind::ActuatorCommand),
        "inter_agent_message" => Ok(EffectKind::InterAgentMessage),
        "resource_allocation" => Ok(EffectKind::ResourceAllocation),
        _ => Err(schema(Error::InvalidInput)),
    }
}

fn parse_threat(value: &str) -> Result<ThreatClass, InventoryLoadError> {
    match value {
        "guardrail_removal" => Ok(ThreatClass::GuardrailRemoval),
        "prompt_injection" => Ok(ThreatClass::PromptInjection),
        "payload_substitution" => Ok(ThreatClass::PayloadSubstitution),
        "direct_credential_or_egress" => Ok(ThreatClass::DirectCredentialOrEgress),
        "unknown_remote_outcome" => Ok(ThreatClass::UnknownRemoteOutcome),
        _ => Err(schema(Error::InvalidInput)),
    }
}

fn parse_mediation(value: &str) -> Result<Mediation, InventoryLoadError> {
    match value {
        "observe_only" => Ok(Mediation::ObserveOnly),
        "cooperative_gate" => Ok(Mediation::CooperativeGate),
        "brokered_effects" => Ok(Mediation::BrokeredEffects),
        _ => Err(schema(Error::InvalidInput)),
    }
}

fn parse_bypass(value: &str) -> Result<BypassDisposition, InventoryLoadError> {
    match value {
        "blocked" => Ok(BypassDisposition::Blocked),
        "residual_uncovered" => Ok(BypassDisposition::ResidualUncovered),
        "unmodeled" => Ok(BypassDisposition::Unmodeled),
        _ => Err(schema(Error::InvalidInput)),
    }
}

fn validate_metadata(
    record: &RouteRecord,
    metadata: &RouteMetadata,
    family_domains: &[TrustDomain],
    credentials: &[crate::perimeter::CredentialExposure],
) -> Result<(), InventoryLoadError> {
    if record.threat != Some(metadata.threat)
        || metadata.trust_path.is_empty()
        || metadata.residual_nonclaims.is_empty()
        || metadata
            .trust_path
            .iter()
            .any(|domain| !family_domains.contains(domain))
    {
        return Err(schema(Error::Binding));
    }
    match &metadata.actor_credential {
        ActorCredentialDisposition::ActorDirect { credential } => {
            if !credentials.iter().any(|declared| {
                declared.credential == *credential
                    && declared.holder == CredentialHolder::ActorDirect
            }) {
                return Err(schema(Error::Binding));
            }
        }
        ActorCredentialDisposition::BrokerMediated => {
            if !credentials
                .iter()
                .any(|declared| declared.holder == CredentialHolder::Broker)
            {
                return Err(schema(Error::Incomplete));
            }
        }
        ActorCredentialDisposition::NoneDeclared => {}
    }
    if record.mediation == Mediation::BrokeredEffects
        && !matches!(
            metadata.actor_credential,
            ActorCredentialDisposition::BrokerMediated
        )
    {
        return Err(schema(Error::Binding));
    }
    Ok(())
}

fn object(value: &Json) -> Result<&std::collections::BTreeMap<String, Json>, InventoryLoadError> {
    value.as_object().ok_or_else(|| schema(Error::InvalidInput))
}

fn array(value: &Json) -> Result<&[Json], InventoryLoadError> {
    value.as_array().ok_or_else(|| schema(Error::InvalidInput))
}

fn required<'a>(
    object: &'a std::collections::BTreeMap<String, Json>,
    field: &str,
) -> Result<&'a Json, InventoryLoadError> {
    object.get(field).ok_or_else(|| schema(Error::InvalidInput))
}

fn exact_fields(
    object: &std::collections::BTreeMap<String, Json>,
    fields: &[&str],
) -> Result<(), InventoryLoadError> {
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(schema(Error::InvalidInput));
    }
    Ok(())
}

fn text(value: &Json) -> Result<&str, InventoryLoadError> {
    let value = value.as_str().ok_or_else(|| schema(Error::InvalidInput))?;
    if value.is_empty() {
        return Err(schema(Error::InvalidInput));
    }
    if value.len() > MAX_TEXT_BYTES {
        return Err(schema(Error::Limit));
    }
    Ok(value)
}

fn unsigned(value: &Json) -> Result<u64, InventoryLoadError> {
    value.as_u64().ok_or_else(|| schema(Error::InvalidInput))
}

fn add_text(total: usize, added: usize) -> Result<usize, InventoryLoadError> {
    let total = total
        .checked_add(added)
        .ok_or_else(|| schema(Error::Overflow))?;
    if total > MAX_FAMILY_TEXT_BYTES {
        Err(schema(Error::Limit))
    } else {
        Ok(total)
    }
}

fn schema(error: Error) -> InventoryLoadError {
    InventoryLoadError::Schema(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_json() -> Vec<u8> {
        br#"{
            "version":1,
            "families":[{
                "scope":{"tenant":1,"principal":2,"purpose":3},
                "family":"egress",
                "trust_domains":["actor","observation_and_analysis","enforcement","governance_and_investigation"],
                "credentials":[{"credential":"broker-token","holder":"broker"}],
                "routes":[{
                    "route":"adapter:disposable",
                    "effect":"network_request",
                    "profile":{"id":"reference-v1","generation":1},
                    "trust_path":["actor","enforcement"],
                    "threat":"direct_credential_or_egress",
                    "actor_credential":{"kind":"broker_mediated"},
                    "mediation":"brokered_effects",
                    "bypass":"blocked",
                    "residual_nonclaims":["no endpoint proof"]
                }],
                "residual_nonclaims":["no production perimeter claim"]
            }]
        }"#
        .to_vec()
    }

    #[test]
    fn loads_complete_metadata_with_exact_classification_lookup() {
        let loaded = LoadedPerimeterInventory::from_json_bytes(&valid_json()).unwrap();
        let route = loaded
            .route_for(
                PerimeterScope {
                    tenant: 1,
                    principal: 2,
                    purpose: 3,
                },
                "egress",
                "adapter:disposable",
            )
            .unwrap();
        assert_eq!(route.record().mediation, Mediation::BrokeredEffects);
        assert_eq!(
            route.record().threat,
            Some(ThreatClass::DirectCredentialOrEgress)
        );
        assert_eq!(route.metadata().effect(), EffectKind::NetworkRequest);
        assert_eq!(route.metadata().profile().id(), "reference-v1");
        assert_eq!(route.metadata().profile().generation(), 1);
        assert_eq!(
            route.metadata().trust_path(),
            &[TrustDomain::Actor, TrustDomain::Enforcement]
        );
        assert!(matches!(
            route.metadata().actor_credential(),
            ActorCredentialDisposition::BrokerMediated
        ));
    }

    #[test]
    fn malformed_unknown_and_unmodeled_inputs_refuse_next_to_valid_control() {
        assert!(LoadedPerimeterInventory::from_json_bytes(&valid_json()).is_ok());
        assert_eq!(
            LoadedPerimeterInventory::from_json_bytes(&vec![b' '; MAX_INVENTORY_BYTES + 1]),
            Err(InventoryLoadError::Schema(Error::Limit))
        );
        assert!(matches!(
            LoadedPerimeterInventory::from_json_bytes(br"{"),
            Err(InventoryLoadError::Parse(_))
        ));

        let unknown = String::from_utf8(valid_json()).unwrap().replacen(
            "\"version\":1",
            "\"version\":1,\"extra\":0",
            1,
        );
        assert_eq!(
            LoadedPerimeterInventory::from_json_bytes(unknown.as_bytes()),
            Err(InventoryLoadError::Schema(Error::InvalidInput))
        );

        let unmodeled = String::from_utf8(valid_json()).unwrap().replacen(
            "\"bypass\":\"blocked\"",
            "\"bypass\":\"unmodeled\"",
            1,
        );
        assert_eq!(
            LoadedPerimeterInventory::from_json_bytes(unmodeled.as_bytes()),
            Err(InventoryLoadError::Schema(Error::Binding))
        );
    }

    #[test]
    fn direct_credential_and_route_nonclaim_overflow_refuse() {
        let direct = String::from_utf8(valid_json())
            .unwrap()
            .replace(
                "\"broker-token\",\"holder\":\"broker\"",
                "\"actor-token\",\"holder\":\"actor_direct\"",
            )
            .replace(
                "\"kind\":\"broker_mediated\"",
                "\"kind\":\"actor_direct\",\"credential\":\"actor-token\"",
            );
        assert_eq!(
            LoadedPerimeterInventory::from_json_bytes(direct.as_bytes()),
            Err(InventoryLoadError::Schema(Error::Binding))
        );

        let nonclaims = (0..=MAX_ROUTE_NONCLAIMS)
            .map(|index| format!("\"n{index}\""))
            .collect::<Vec<_>>()
            .join(",");
        let too_many_nonclaims = String::from_utf8(valid_json()).unwrap().replacen(
            "[\"no endpoint proof\"]",
            &format!("[{nonclaims}]"),
            1,
        );
        assert_eq!(
            LoadedPerimeterInventory::from_json_bytes(too_many_nonclaims.as_bytes()),
            Err(InventoryLoadError::Schema(Error::Limit))
        );
    }

    #[test]
    fn directory_or_traversing_path_is_unreadable_without_parsing() {
        assert_eq!(
            LoadedPerimeterInventory::load_path(Path::new(".")),
            Err(InventoryLoadError::Unreadable)
        );
        assert_eq!(
            LoadedPerimeterInventory::load_path(Path::new("../not-an-inventory.json")),
            Err(InventoryLoadError::Unreadable)
        );
    }
}
