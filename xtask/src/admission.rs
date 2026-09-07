//! Dependency admission compiler (roadmap FA-053 schema, FA-054 checker).
//!
//! Plan §19.8 states the obligation this module starts to discharge: "A future
//! full dependency admission compiler must parse Cargo metadata's resolved graph
//! for each target and feature profile, enumerate build/proc-macro/runtime
//! closures and independently verify source manifests."
//!
//! Scope boundary, stated once so it is not overclaimed elsewhere:
//!
//! * This module reads `cargo metadata --format-version 1 --locked --offline
//!   --filter-platform <target>` output together with the admission block of
//!   `registry/dependency_policy.json`, and decides admission for the **closed
//!   initial inventory**: exactly the local path packages the policy names, for
//!   exactly one declared target profile, with exactly the feature set that
//!   profile freezes.
//! * **No external package is admissible by this checker at all.** Cargo
//!   metadata cannot establish a checksum, a build-script effect, or what bytes
//!   a loader would fetch at run time, so there is no boolean, policy row, or
//!   attestation field here that can turn an external package into an admitted
//!   one. Admitting a foundation requires the full source/feature/runtime
//!   inspection that FA-053 defers; until that exists the honest answer is
//!   refusal, not a disabled acceptance path.
//! * Absence is never evidence. A missing array, a missing field, a malformed
//!   entry, an unresolvable id, or a graph that is not an exact bijection is a
//!   refusal, never an empty set that passes.
//! * This module does not replace `check_lock`. The exact lockfile guard in
//!   `main.rs` remains the operative gate; this runs alongside it.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Component, Path};

use crate::json::{Json, Limits, parse};

/// The phase a finding was produced in.
///
/// Findings are causal: a negative test asserts the phase and code, not merely
/// that admission failed, so a checker that refuses for the wrong reason does
/// not pass its own suite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Phase {
    /// Shape of the metadata document itself.
    Metadata,
    /// Structure of the resolve graph and its bijection with the package set.
    Graph,
    /// Package inventory: presence, absence, count.
    Inventory,
    /// Where a package comes from, and where its manifest lives.
    Source,
    /// Build scripts, proc macros, native `links`, build dependencies.
    Build,
    /// Feature activation against the frozen profile.
    Features,
    /// Target profile binding and target-scoped activation.
    Target,
    /// Policy document shape and self-consistency.
    Policy,
}

impl Phase {
    /// Stable lowercase name used in reports.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Metadata => "metadata",
            Phase::Graph => "graph",
            Phase::Inventory => "inventory",
            Phase::Source => "source",
            Phase::Build => "build",
            Phase::Features => "features",
            Phase::Target => "target",
            Phase::Policy => "policy",
        }
    }
}

/// One reason admission was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// Which phase produced it.
    pub phase: Phase,
    /// Stable machine-readable code.
    pub code: &'static str,
    /// Where in the input, as a JSON path or package identity.
    pub location: String,
    /// What was expected versus observed.
    pub detail: String,
}

impl Finding {
    fn new(
        phase: Phase,
        code: &'static str,
        location: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Finding {
            phase,
            code,
            location: location.into(),
            detail: detail.into(),
        }
    }
}

/// What the compiler observed about one resolved package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageFacts {
    /// Package name as declared in metadata.
    pub name: String,
    /// Package version as declared in metadata.
    pub version: String,
    /// Opaque package id, retained verbatim for graph comparison only.
    pub id: String,
    /// `manifest_path` made relative to `workspace_root` by path components.
    pub manifest_path: String,
    /// `true` when `source` was JSON `null`, i.e. a local path package.
    pub is_local: bool,
    /// The exact opaque `source` string when present, compared verbatim.
    pub source: Option<String>,
    /// Sorted, de-duplicated target kinds and crate types.
    pub target_kinds: Vec<String>,
    /// The `links` value when present.
    pub links: Option<String>,
    /// Features activated for this package in the resolve graph, sorted.
    pub activated_features: Vec<String>,
}

/// One admitted local package row from the policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalRow {
    /// Expected package name. Never inferred from a directory name.
    pub name: String,
    /// Expected exact version.
    pub version: String,
    /// Expected workspace-relative manifest path.
    pub manifest_path: String,
    /// Target triples this row admits. A triple absent here is not admitted.
    pub targets: BTreeSet<String>,
    /// Who owns this admission decision.
    ///
    /// Required by the admission procedure in
    /// `docs/DEPENDENCY_CONSTITUTION.md`. An unowned row is not an admission,
    /// so a missing or empty owner is a policy load error.
    pub owner: String,
}

/// How Cargo's `dep_kinds[].kind` spells an ordinary dependency: as JSON null.
///
/// Recorded under this name so a normal edge stays visible in the evidence
/// while still being distinguishable from a build or dev edge.
const NORMAL_DEP_KIND: &str = "normal";

/// The only decision value that admits.
///
/// Anything else — including a pending or provisional value — refuses. This is
/// what stops a row from being pre-staged in the registry and quietly taking
/// effect later.
pub const DECISION_ADMITTED: &str = "admitted";

/// A frozen per-target admission profile.
///
/// A feature set is a property of a target, not of a workspace, so the exact
/// expected feature set is frozen per profile and compared for set equality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetProfile {
    /// The exact target triple this profile claims for.
    pub target: String,
    /// Exact expected feature set per package name.
    pub expected_features: BTreeMap<String, Vec<String>>,
}

/// Structural prohibitions the policy asserts.
///
/// Every field is required to be explicitly present and explicitly `false` in
/// the initial scope. A policy cannot switch a constitutional prohibition off,
/// and a defaulted value is not an assertion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rules {
    /// Required `cargo metadata` format version.
    pub metadata_version: u64,
}

/// The admission policy, as parsed from the registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    /// Admission block schema identity.
    pub schema: String,
    /// Admitted local path packages.
    pub local: Vec<LocalRow>,
    /// Frozen per-target profiles.
    pub profiles: Vec<TargetProfile>,
    /// Structural rules.
    pub rules: Rules,
}

/// The result of one admission run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The target profile this run claims for.
    pub target: String,
    /// Every refusal reason, in phase order.
    pub findings: Vec<Finding>,
    /// The exact inventory the compiler observed.
    pub inventory: Vec<PackageFacts>,
    /// Statements that bound the claim. Always non-empty on a pass.
    pub notes: Vec<String>,
}

impl Report {
    /// `true` only when nothing was refused.
    ///
    /// There is no "skipped" disposition: a check that could not run is a
    /// finding, never a pass.
    #[must_use]
    pub fn is_admitted(&self) -> bool {
        self.findings.is_empty()
    }

    /// Render the report for the gate log.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "ADMISSION target profile: {}", self.target);
        out.push_str("ADMISSION inventory:\n");
        for facts in &self.inventory {
            let origin = match &facts.source {
                None => "local".to_string(),
                Some(source) => format!("external source={source}"),
            };
            let _ = writeln!(
                out,
                "  {} {} {} at {} [{}{}] kinds={} links={} features={}",
                facts.name,
                facts.version,
                facts.id,
                facts.manifest_path,
                if facts.is_local { "workspace " } else { "" },
                origin,
                facts.target_kinds.join(","),
                facts.links.as_deref().unwrap_or("none"),
                if facts.activated_features.is_empty() {
                    "none".to_string()
                } else {
                    facts.activated_features.join(",")
                }
            );
        }
        for note in &self.notes {
            let _ = writeln!(out, "ADMISSION note: {note}");
        }
        if self.findings.is_empty() {
            let _ = writeln!(
                out,
                "PASS admission_closed_inventory[{}]: exact local package set, exact resolve \
                 bijection, no external source, build script, proc macro, native link, build or \
                 target-scoped dependency, and the frozen feature set. Not a runtime or \
                 downloaded-artifact closure claim.",
                self.target
            );
        } else {
            for finding in &self.findings {
                let _ = writeln!(
                    out,
                    "FAIL admission[{}] {}: {} -- {}",
                    finding.phase.as_str(),
                    finding.code,
                    finding.location,
                    finding.detail
                );
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Strict accessors. Every one of these refuses rather than defaulting, so an
// absent or malformed field can never be laundered into an empty passing set.
// ---------------------------------------------------------------------------

fn type_error(
    phase: Phase,
    code: &'static str,
    path: &str,
    expected: &str,
    found: &Json,
) -> Finding {
    Finding::new(
        phase,
        code,
        path.to_string(),
        format!("expected {expected}, found {}", found.kind()),
    )
}

fn missing(phase: Phase, path: &str, key: &str) -> Finding {
    Finding::new(
        phase,
        "missing_field",
        format!("{path}.{key}"),
        format!("required field `{key}` is absent; an absent field is not an empty value"),
    )
}

fn req_object<'a>(
    value: &'a Json,
    phase: Phase,
    path: &str,
    findings: &mut Vec<Finding>,
) -> Option<&'a BTreeMap<String, Json>> {
    match value.as_object() {
        Some(map) => Some(map),
        None => {
            findings.push(type_error(phase, "expected_object", path, "object", value));
            None
        }
    }
}

fn req_array<'a>(
    value: &'a Json,
    phase: Phase,
    path: &str,
    findings: &mut Vec<Finding>,
) -> Option<&'a [Json]> {
    match value.as_array() {
        Some(items) => Some(items),
        None => {
            findings.push(type_error(phase, "expected_array", path, "array", value));
            None
        }
    }
}

fn req_str<'a>(
    value: &'a Json,
    phase: Phase,
    path: &str,
    findings: &mut Vec<Finding>,
) -> Option<&'a str> {
    match value.as_str() {
        Some(text) => Some(text),
        None => {
            findings.push(type_error(phase, "expected_string", path, "string", value));
            None
        }
    }
}

/// Fetch a required field, recording a finding when it is absent.
fn req_field<'a>(
    map: &'a BTreeMap<String, Json>,
    key: &'static str,
    phase: Phase,
    path: &str,
    findings: &mut Vec<Finding>,
) -> Option<&'a Json> {
    match map.get(key) {
        Some(value) => Some(value),
        None => {
            findings.push(missing(phase, path, key));
            None
        }
    }
}

/// A required array whose every element must be a string.
///
/// A malformed element is a finding; it is never filtered out, because
/// discarding malformed evidence is how a prohibition silently stops applying.
fn req_string_array(
    map: &BTreeMap<String, Json>,
    key: &'static str,
    phase: Phase,
    path: &str,
    findings: &mut Vec<Finding>,
) -> Option<Vec<String>> {
    let value = req_field(map, key, phase, path, findings)?;
    let items = req_array(value, phase, &format!("{path}.{key}"), findings)?;
    let mut out = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let item_path = format!("{path}.{key}[{index}]");
        let text = req_str(item, phase, &item_path, findings)?;
        out.push(text.to_string());
    }
    Some(out)
}

/// A field that must be exactly JSON `null` or a JSON string, never anything
/// else and never absent.
fn req_null_or_string(
    map: &BTreeMap<String, Json>,
    key: &'static str,
    phase: Phase,
    path: &str,
    findings: &mut Vec<Finding>,
) -> Option<Option<String>> {
    let value = req_field(map, key, phase, path, findings)?;
    if value.is_null() {
        return Some(None);
    }
    match value.as_str() {
        Some(text) => Some(Some(text.to_string())),
        None => {
            findings.push(type_error(
                phase,
                "expected_null_or_string",
                &format!("{path}.{key}"),
                "null or string",
                value,
            ));
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Policy
// ---------------------------------------------------------------------------

/// Required admission block schema identity.
pub const ADMISSION_SCHEMA: &str = "fa.admission/0.1";

/// The only `cargo metadata` format version this checker implements.
///
/// The policy states the version it expects, but it may not state a version
/// this code cannot read: accepting an arbitrary number would let a policy
/// declare `0` and admit a document that also says `0`, on a shape nothing
/// here has ever parsed.
pub const SUPPORTED_METADATA_VERSION: u64 = 1;

/// Prohibition flags that a policy may state but may never relax.
const PROHIBITIONS: [&str; 3] = ["allow_build_scripts", "allow_proc_macro", "allow_links"];

/// Parse the admission block of the dependency policy.
///
/// # Errors
///
/// Returns a message when the document cannot be parsed, when the admission
/// block is structurally unusable, or when the policy attempts to relax a
/// constitutional prohibition. A missing or permissive policy is a hard
/// failure: an admission check that cannot load sound rules has not passed.
pub fn parse_policy(bytes: &[u8]) -> Result<Policy, String> {
    let document = parse(bytes, Limits::default())
        .map_err(|error| format!("registry/dependency_policy.json: {error}"))?;
    let root = document
        .as_object()
        .ok_or_else(|| "dependency policy root is not an object".to_string())?;
    let admission = root.get("admission").ok_or_else(|| {
        "dependency policy has no `admission` block (FA-053 not integrated)".to_string()
    })?;
    let admission = admission
        .as_object()
        .ok_or_else(|| "$.admission is not an object".to_string())?;

    let schema = admission
        .get("schema")
        .and_then(Json::as_str)
        .ok_or_else(|| "$.admission.schema must be a string".to_string())?;
    if schema != ADMISSION_SCHEMA {
        return Err(format!(
            "$.admission.schema is `{schema}`, this checker implements `{ADMISSION_SCHEMA}`"
        ));
    }

    // An external row cannot be honored by this checker at all, so a policy
    // that carries one is refused rather than silently ignored.
    match admission.get("external_rows") {
        None => return Err("$.admission.external_rows must be present and empty".to_string()),
        Some(value) => {
            let rows = value
                .as_array()
                .ok_or_else(|| "$.admission.external_rows is not an array".to_string())?;
            if !rows.is_empty() {
                return Err(format!(
                    "$.admission.external_rows has {} row(s); this checker admits no external \
                     package under any row, because Cargo metadata cannot establish a checksum, \
                     a build-script effect, or a downloaded-artifact closure. Remove the rows or \
                     implement the full source/feature/runtime inspection first.",
                    rows.len()
                ));
            }
        }
    }

    let rules_value = admission.get("rules").ok_or_else(|| {
        "$.admission.rules is absent; structural rules must be explicit".to_string()
    })?;
    let rules_map = rules_value
        .as_object()
        .ok_or_else(|| "$.admission.rules is not an object".to_string())?;

    // Each prohibition must be explicitly present and explicitly false. A
    // defaulted value is not an assertion, and `true` is a policy attempting to
    // disable the constitution.
    for flag in PROHIBITIONS {
        match rules_map.get(flag) {
            None => {
                return Err(format!(
                    "$.admission.rules.{flag} must be present and explicitly false; a defaulted \
                     prohibition is not an assertion"
                ));
            }
            Some(value) => match value.as_bool() {
                Some(false) => {}
                Some(true) => {
                    return Err(format!(
                        "$.admission.rules.{flag} is true; a policy file may not switch off a \
                         constitutional prohibition (docs/DEPENDENCY_CONSTITUTION.md)"
                    ));
                }
                None => {
                    return Err(format!(
                        "$.admission.rules.{flag} must be a boolean, found {}",
                        value.kind()
                    ));
                }
            },
        }
    }
    match rules_map
        .get("require_resolve_section")
        .and_then(Json::as_bool)
    {
        Some(true) => {}
        _ => {
            return Err(
                "$.admission.rules.require_resolve_section must be present and explicitly true"
                    .to_string(),
            );
        }
    }
    let metadata_version = rules_map
        .get("metadata_version")
        .and_then(Json::as_u64)
        .ok_or_else(|| "$.admission.rules.metadata_version must be an integer".to_string())?;
    if metadata_version != SUPPORTED_METADATA_VERSION {
        return Err(format!(
            "$.admission.rules.metadata_version is {metadata_version}; this checker implements \
             cargo metadata format {SUPPORTED_METADATA_VERSION} only. Accepting another number \
             would let a policy declare a version and admit a document that agrees with it, on a \
             shape nothing here has ever parsed"
        ));
    }

    let mut findings = Vec::new();

    let rows_value = admission
        .get("local_packages")
        .ok_or_else(|| "$.admission.local_packages is absent".to_string())?;
    let rows = rows_value
        .as_array()
        .ok_or_else(|| "$.admission.local_packages is not an array".to_string())?;
    if rows.is_empty() {
        return Err(
            "$.admission.local_packages is empty; an empty inventory admits nothing and \
                    would make every workspace fail for the wrong reason"
                .to_string(),
        );
    }
    let mut local = Vec::new();
    let mut seen_names = BTreeSet::new();
    for (index, row) in rows.iter().enumerate() {
        let path = format!("$.admission.local_packages[{index}]");
        let Some(row) = req_object(row, Phase::Policy, &path, &mut findings) else {
            continue;
        };
        let name = req_field(row, "name", Phase::Policy, &path, &mut findings)
            .and_then(|v| req_str(v, Phase::Policy, &path, &mut findings));
        let version = req_field(row, "version", Phase::Policy, &path, &mut findings)
            .and_then(|v| req_str(v, Phase::Policy, &path, &mut findings));
        let manifest = req_field(row, "manifest_path", Phase::Policy, &path, &mut findings)
            .and_then(|v| req_str(v, Phase::Policy, &path, &mut findings));
        let owner = req_field(row, "owner", Phase::Policy, &path, &mut findings)
            .and_then(|v| req_str(v, Phase::Policy, &path, &mut findings));
        let decision = req_field(row, "decision", Phase::Policy, &path, &mut findings)
            .and_then(|v| req_str(v, Phase::Policy, &path, &mut findings));

        // An unowned or undecided row is not an admission. Only the exact
        // decision value admits; a pending or provisional row must refuse
        // rather than sit in the registry waiting to take effect.
        if let Some(owner) = owner
            && owner.trim().is_empty()
        {
            findings.push(Finding::new(
                Phase::Policy,
                "row_owner_empty",
                path.clone(),
                "`owner` is empty; an unowned row is not an admission",
            ));
        }
        if let Some(decision) = decision
            && decision != DECISION_ADMITTED
        {
            findings.push(Finding::new(
                Phase::Policy,
                "row_not_admitted",
                path.clone(),
                format!(
                    "`decision` is `{decision}`; only the exact value `{DECISION_ADMITTED}` \
                     admits, so this row refuses"
                ),
            ));
        }

        // Target scope is required data, not decoration: a row that names no
        // target admits nothing, and must not read as admitting everywhere.
        let mut row_targets: BTreeSet<String> = BTreeSet::new();
        match req_field(row, "targets", Phase::Policy, &path, &mut findings)
            .and_then(|value| req_array(value, Phase::Policy, &path, &mut findings))
        {
            None => {}
            Some([]) => findings.push(Finding::new(
                Phase::Policy,
                "row_admits_no_target",
                path.clone(),
                "`targets` is empty; a row that names no target admits nothing",
            )),
            Some(items) => {
                for item in items {
                    if let Some(text) = req_str(item, Phase::Policy, &path, &mut findings) {
                        row_targets.insert(text.to_string());
                    }
                }
            }
        }

        if let (Some(name), Some(version), Some(manifest), Some(owner), Some(_)) =
            (name, version, manifest, owner, decision)
        {
            if !seen_names.insert(name.to_string()) {
                findings.push(Finding::new(
                    Phase::Policy,
                    "duplicate_local_row",
                    path.clone(),
                    format!("package `{name}` is listed more than once"),
                ));
            }
            local.push(LocalRow {
                name: name.to_string(),
                version: version.to_string(),
                manifest_path: manifest.to_string(),
                targets: row_targets,
                owner: owner.to_string(),
            });
        }
    }

    let profiles_value = admission.get("target_profiles").ok_or_else(|| {
        "$.admission.target_profiles is absent; a per-target admission claim \
                        cannot be made without a frozen target profile"
            .to_string()
    })?;
    let profile_rows = profiles_value
        .as_array()
        .ok_or_else(|| "$.admission.target_profiles is not an array".to_string())?;
    if profile_rows.is_empty() {
        return Err("$.admission.target_profiles is empty; no target could be claimed".to_string());
    }
    let mut profiles = Vec::new();
    let mut seen_targets = BTreeSet::new();
    for (index, row) in profile_rows.iter().enumerate() {
        let path = format!("$.admission.target_profiles[{index}]");
        let Some(row) = req_object(row, Phase::Policy, &path, &mut findings) else {
            continue;
        };
        let Some(target) = req_field(row, "target", Phase::Policy, &path, &mut findings)
            .and_then(|v| req_str(v, Phase::Policy, &path, &mut findings))
        else {
            continue;
        };
        if !seen_targets.insert(target.to_string()) {
            findings.push(Finding::new(
                Phase::Policy,
                "duplicate_target_profile",
                path.clone(),
                format!("target `{target}` is listed more than once"),
            ));
        }
        let Some(expected) = req_field(
            row,
            "expected_features",
            Phase::Policy,
            &path,
            &mut findings,
        )
        .and_then(|v| {
            req_object(
                v,
                Phase::Policy,
                &format!("{path}.expected_features"),
                &mut findings,
            )
        }) else {
            continue;
        };
        let mut expected_features = BTreeMap::new();
        for (package, value) in expected {
            let feature_path = format!("{path}.expected_features.{package}");
            let Some(items) = req_array(value, Phase::Policy, &feature_path, &mut findings) else {
                continue;
            };
            let mut features = Vec::new();
            let mut ok = true;
            for (feature_index, item) in items.iter().enumerate() {
                match req_str(
                    item,
                    Phase::Policy,
                    &format!("{feature_path}[{feature_index}]"),
                    &mut findings,
                ) {
                    Some(text) => features.push(text.to_string()),
                    None => ok = false,
                }
            }
            if ok {
                features.sort();
                expected_features.insert(package.clone(), features);
            }
        }
        // Every admitted package must have a frozen feature set in every
        // profile, or the profile is silent about it rather than covering it.
        for local_row in &local {
            if !expected_features.contains_key(&local_row.name) {
                findings.push(Finding::new(
                    Phase::Policy,
                    "profile_silent_on_package",
                    format!("{path}.expected_features"),
                    format!(
                        "target `{target}` freezes no feature set for admitted package `{}`; a \
                         profile that omits a package is silent on it, never implicitly covering it",
                        local_row.name
                    ),
                ));
            }
        }
        profiles.push(TargetProfile {
            target: target.to_string(),
            expected_features,
        });
    }

    if !findings.is_empty() {
        let rendered: Vec<String> = findings
            .iter()
            .map(|f| {
                format!(
                    "[{}] {} at {}: {}",
                    f.phase.as_str(),
                    f.code,
                    f.location,
                    f.detail
                )
            })
            .collect();
        return Err(format!(
            "dependency policy is malformed: {}",
            rendered.join("; ")
        ));
    }

    Ok(Policy {
        schema: schema.to_string(),
        local,
        profiles,
        rules: Rules { metadata_version },
    })
}

// ---------------------------------------------------------------------------
// Path containment
// ---------------------------------------------------------------------------

/// Make `manifest_path` relative to `workspace_root` by **path components**.
///
/// A byte-prefix strip would accept a sibling directory: root `/workspace` and
/// manifest `/workspacextask/Cargo.toml` share a string prefix but not a path
/// prefix. `Path::strip_prefix` matches component-wise, which closes that.
/// Both paths must be absolute and free of `..`, and the result must be a
/// non-empty sequence of normal components.
fn relative_manifest(manifest_path: &str, workspace_root: &str) -> Result<String, String> {
    let manifest = Path::new(manifest_path);
    let root = Path::new(workspace_root);
    if !manifest.is_absolute() {
        return Err("manifest_path is not absolute".to_string());
    }
    if !root.is_absolute() {
        return Err("workspace_root is not absolute".to_string());
    }
    for (label, path) in [("manifest_path", manifest), ("workspace_root", root)] {
        if path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
        {
            return Err(format!("{label} contains a `.` or `..` component"));
        }
    }
    let rest = manifest
        .strip_prefix(root)
        .map_err(|_| "manifest_path is not contained in workspace_root".to_string())?;
    let mut parts = Vec::new();
    for component in rest.components() {
        match component {
            Component::Normal(part) => match part.to_str() {
                Some(text) => parts.push(text.to_string()),
                None => return Err("manifest_path has a non-UTF-8 component".to_string()),
            },
            _ => return Err("manifest_path has an unexpected path component".to_string()),
        }
    }
    if parts.is_empty() {
        return Err("manifest_path equals workspace_root".to_string());
    }
    Ok(parts.join("/"))
}

// ---------------------------------------------------------------------------
// Check
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// The admission evaluator.
//
// This is a general, pure decision procedure over reviewed rows and observed
// packages. It has no knowledge that this repository's inventory happens to be
// closed, and no knowledge that external packages are currently forbidden:
// those are properties of the policy wrapper (`parse_policy`), which sits
// outside and admits no external row. Keeping the two apart is what lets the
// algorithm be exercised on both sides of a decision without anything being
// admitted here.
// ---------------------------------------------------------------------------

/// Where a package's bytes come from. Compared verbatim, never parsed.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceId {
    /// Cargo's JSON null: a path package, identified by its workspace-relative
    /// manifest location.
    WorkspacePath { manifest_path: String },
    /// Any non-null `source` string, retained exactly as Cargo emitted it.
    Exact(String),
}

/// Exact package identity.
///
/// No component is a key on its own. Every crates.io package shares the one
/// registry index URL, and a single git repository can contain several packages
/// and several versions, so a source alone identifies nothing. A name alone is
/// the `ft-api` / `frankentorch-api` confusion the constitution names.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageId {
    /// Where the bytes come from.
    pub source: SourceId,
    /// Package name as Cargo reports it.
    pub name: String,
    /// Exact version.
    pub version: String,
}

/// Cargo's dependency kinds. A null `kind` is `Normal`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DepKind {
    /// An ordinary dependency; Cargo spells this as null.
    Normal,
    /// A build-dependency.
    Build,
    /// A dev-dependency.
    Dev,
}

impl DepKind {
    /// Stable lowercase name used in reports.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            DepKind::Normal => "normal",
            DepKind::Build => "build",
            DepKind::Dev => "dev",
        }
    }
}

/// One dependency edge, naming its exact destination package.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct EdgeId {
    /// The destination package's full identity, never a bare name or source.
    pub to: PackageId,
    /// Which kind of dependency this edge is.
    pub kind: DepKind,
}

/// What one reviewed row permits on one target triple.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TargetScope {
    /// Exactly the features that may be activated.
    pub features: BTreeSet<String>,
    /// Exactly the edges that may be present.
    pub edges: BTreeSet<EdgeId>,
}

/// One reviewed admission row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewedRow {
    /// The package this row admits.
    pub id: PackageId,
    /// Per-target scope. A target absent from this map is **not** admitted:
    /// absence is never scope.
    pub scopes: BTreeMap<String, TargetScope>,
}

/// The reviewed set the evaluator decides against.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AdmissionSet {
    rows: BTreeMap<PackageId, ReviewedRow>,
}

/// Why a reviewed set could not be constructed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdmissionSetError {
    /// Two rows carried the same full identity.
    DuplicateIdentity(PackageId),
}

impl AdmissionSet {
    /// Build a reviewed set, refusing a duplicate full identity.
    ///
    /// # Errors
    ///
    /// Returns [`AdmissionSetError::DuplicateIdentity`] rather than silently
    /// keeping one of two rows that claim the same package: a set that quietly
    /// dropped a row would admit under a scope nobody reviewed.
    pub fn new(rows: Vec<ReviewedRow>) -> Result<AdmissionSet, AdmissionSetError> {
        let mut map: BTreeMap<PackageId, ReviewedRow> = BTreeMap::new();
        for row in rows {
            if map.contains_key(&row.id) {
                return Err(AdmissionSetError::DuplicateIdentity(row.id));
            }
            map.insert(row.id.clone(), row);
        }
        Ok(AdmissionSet { rows: map })
    }
}

/// One resolved package as observed on one target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservedPackage {
    /// The package's full identity.
    pub id: PackageId,
    /// Features activated on this target.
    pub features: BTreeSet<String>,
    /// Edges present on this target.
    pub edges: BTreeSet<EdgeId>,
}

/// One way an observation departed from the reviewed set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Violation {
    /// No reviewed row carries this exact identity.
    NoReviewedRow { id: PackageId },
    /// A row exists, but says nothing about this target.
    TargetNotAdmitted {
        /// The observed package.
        id: PackageId,
        /// The targets the row does admit, sorted.
        admitted: Vec<String>,
    },
    /// An edge is present that this target's scope does not permit.
    EdgeNotAdmitted {
        /// The target evaluated.
        target: String,
        /// The offending edge.
        edge: EdgeId,
    },
    /// The scope requires an edge that is absent.
    EdgeMissing {
        /// The target evaluated.
        target: String,
        /// The absent edge.
        edge: EdgeId,
    },
    /// An admitted edge points at a package with no reviewed row on this
    /// target, so the edge is admitted into somewhere nobody reviewed.
    EdgeDestinationNotReviewed {
        /// The target evaluated.
        target: String,
        /// The dangling edge.
        edge: EdgeId,
    },
    /// The activated feature set is not exactly the reviewed set.
    FeatureSetMismatch {
        /// Reviewed features, sorted.
        expected: Vec<String>,
        /// Observed features, sorted.
        observed: Vec<String>,
    },
}

/// Project a parsed `cargo metadata` document into per-package observations.
///
/// This is the production projection: `check` uses exactly this function, so a
/// test that feeds real Cargo-shaped JSON exercises the same code the gate
/// runs. Features come from `resolve.nodes[].features`; edges come from
/// `resolve.nodes[].deps[]`, each resolved through `deps[].pkg` to the
/// destination's exact `(source, name, version)`.
///
/// Nothing degrades to an empty observation. A package with no resolve node, a
/// dependency with no `pkg`, and a `pkg` that names no package are each a
/// finding and cause that package to be **omitted** from the result rather than
/// observed as empty, so an unverifiable package can never be evaluated as if
/// it were clean.
///
/// Causal diagnostics for malformed dependency kinds are emitted separately by
/// the caller's structural pass; this function does not swallow them.
#[must_use]
pub fn project_observed_packages(
    document: &Json,
    findings: &mut Vec<Finding>,
) -> Vec<ObservedPackage> {
    let mut observations = Vec::new();
    // These two early exits are the public API's own boundary. `check` happens
    // to diagnose a malformed document separately, but a direct caller of this
    // function must never receive an empty result with no reason: an empty vec
    // with no finding would read as "nothing to observe" rather than "the
    // document could not be read".
    let Some(root) = req_object(document, Phase::Metadata, "$", findings) else {
        return observations;
    };
    let workspace_root = root.get("workspace_root").and_then(Json::as_str);
    let Some(packages) = req_field(root, "packages", Phase::Inventory, "$", findings)
        .and_then(|value| req_array(value, Phase::Inventory, "$.packages", findings))
    else {
        return observations;
    };
    if packages.is_empty() {
        findings.push(Finding::new(
            Phase::Inventory,
            "empty_package_inventory",
            "$.packages",
            "an empty package inventory cannot establish the required workspace observation",
        ));
        return observations;
    }

    // metadata id -> exact identity, so an edge can name its destination.
    let mut identities: BTreeMap<String, PackageId> = BTreeMap::new();
    for (index, package) in packages.iter().enumerate() {
        let (Some(id), Some(name), Some(version)) = (
            package.get("id").and_then(Json::as_str),
            package.get("name").and_then(Json::as_str),
            package.get("version").and_then(Json::as_str),
        ) else {
            findings.push(Finding::new(
                Phase::Source,
                "package_identity_unreadable",
                format!("$.packages[{index}]"),
                "package id, name and version must all be readable strings",
            ));
            continue;
        };
        let source = match package.get("source") {
            Some(value) if value.is_null() => {
                // A path package's identity includes where it lives, so an
                // unreadable manifest path yields no identity at all rather
                // than an empty one that could never match a reviewed row for
                // the wrong reason.
                let Some(manifest) = package.get("manifest_path").and_then(Json::as_str) else {
                    findings.push(Finding::new(
                        Phase::Source,
                        "manifest_path_unreadable",
                        id.to_string(),
                        format!(
                            "path package `{name} {version}` has no readable `manifest_path`, so \
                             it has no identity and is not observed"
                        ),
                    ));
                    continue;
                };
                let Some(relative) =
                    workspace_root.and_then(|root| relative_manifest(manifest, root).ok())
                else {
                    findings.push(Finding::new(
                        Phase::Source,
                        "manifest_path_unreadable",
                        id.to_string(),
                        format!(
                            "path package `{name} {version}` has manifest `{manifest}`, which is \
                             not resolvable inside the workspace root, so it has no identity"
                        ),
                    ));
                    continue;
                };
                SourceId::WorkspacePath {
                    manifest_path: relative,
                }
            }
            Some(value) => match value.as_str() {
                Some(text) => SourceId::Exact(text.to_string()),
                None => {
                    findings.push(Finding::new(
                        Phase::Source,
                        "package_source_unreadable",
                        format!("$.packages[{index}].source"),
                        "source must be an explicit null or a string",
                    ));
                    continue;
                }
            },
            None => {
                findings.push(missing(
                    Phase::Source,
                    &format!("$.packages[{index}]"),
                    "source",
                ));
                continue;
            }
        };
        identities.insert(
            id.to_string(),
            PackageId {
                source,
                name: name.to_string(),
                version: version.to_string(),
            },
        );
    }

    // Resolve nodes, by metadata id. A node whose `features` or `deps` cannot
    // be read exactly is marked unfaithful rather than defaulted to empty: an
    // absent or malformed array is unknown, and unknown is not "none".
    let mut nodes: BTreeMap<String, NodeObservation> = BTreeMap::new();
    if let Some(resolve) = root.get("resolve").filter(|value| !value.is_null())
        && let Some(list) = resolve.get("nodes").and_then(Json::as_array)
    {
        for (index, node) in list.iter().enumerate() {
            let node_path = format!("$.resolve.nodes[{index}]");
            let Some(node) = req_object(node, Phase::Graph, &node_path, findings) else {
                continue;
            };
            let Some(id) = node.get("id").and_then(Json::as_str) else {
                continue;
            };
            let mut faithful = true;
            let mut features = BTreeSet::new();
            match req_field(node, "features", Phase::Features, &node_path, findings).and_then(
                |value| {
                    req_array(
                        value,
                        Phase::Features,
                        &format!("{node_path}.features"),
                        findings,
                    )
                },
            ) {
                None => faithful = false,
                Some(items) => {
                    for (item_index, item) in items.iter().enumerate() {
                        match req_str(
                            item,
                            Phase::Features,
                            &format!("{node_path}.features[{item_index}]"),
                            findings,
                        ) {
                            Some(text) => {
                                features.insert(text.to_string());
                            }
                            // A malformed feature entry is never skipped: it
                            // would silently shrink the activated set.
                            None => faithful = false,
                        }
                    }
                }
            }
            let mut deps = Vec::new();
            match req_field(node, "deps", Phase::Graph, &node_path, findings).and_then(|value| {
                req_array(value, Phase::Graph, &format!("{node_path}.deps"), findings)
            }) {
                None => faithful = false,
                Some(list) => {
                    for (dep_index, dep) in list.iter().enumerate() {
                        deps.push(record_dep_edge(
                            dep,
                            &format!("{node_path}.deps[{dep_index}]"),
                        ));
                    }
                }
            }
            nodes.insert(
                id.to_string(),
                NodeObservation {
                    features,
                    deps,
                    faithful,
                },
            );
        }
    }

    for (metadata_id, id) in &identities {
        let Some(NodeObservation {
            features,
            deps,
            faithful: node_faithful,
        }) = nodes.get(metadata_id)
        else {
            findings.push(Finding::new(
                Phase::Graph,
                "package_missing_from_resolve",
                metadata_id.clone(),
                format!(
                    "package `{} {}` has no resolve node, so its activated features and edges are \
                     unknown; unknown is not an empty observation and it is not evaluated",
                    id.name, id.version
                ),
            ));
            continue;
        };
        let mut edges = BTreeSet::new();
        // An unreadable `features` or `deps` array already produced its own
        // finding above; carry that forward so the package is omitted rather
        // than observed with a set nobody could read.
        let mut faithful = *node_faithful;
        for dep in deps {
            let Some(destination) = dep
                .dest_metadata_id
                .as_ref()
                .and_then(|pkg| identities.get(pkg))
            else {
                findings.push(Finding::new(
                    Phase::Graph,
                    "edge_destination_unresolved",
                    dep.path.clone(),
                    format!(
                        "dependency edge from `{} {}` names `{}`, which matches no package in \
                         $.packages; the edge cannot be given a destination identity",
                        id.name,
                        id.version,
                        dep.dest_metadata_id.as_deref().unwrap_or("<no pkg field>")
                    ),
                ));
                faithful = false;
                continue;
            };
            if dep.kinds.is_empty() {
                // Every kind on this edge was missing or malformed. The causal
                // marker is already reported; the edge has no representable
                // identity, so the observation cannot be complete.
                faithful = false;
                findings.push(Finding::new(
                    Phase::Metadata,
                    "dependency_kind_unreadable",
                    dep.path.clone(),
                    "no readable dependency kind; the package cannot be projected faithfully",
                ));
                continue;
            }
            for kind in &dep.kinds {
                edges.insert(EdgeId {
                    to: destination.clone(),
                    kind: *kind,
                });
            }
        }
        if !faithful {
            continue;
        }
        observations.push(ObservedPackage {
            id: id.clone(),
            features: features.clone(),
            edges,
        });
    }
    observations
}

/// Decide whether `observed` is admitted on `target` under `set`.
///
/// Pure: no I/O, no JSON, no policy, no notion of a closed inventory. An empty
/// result means admitted. Ordering is deterministic because every collection is
/// a `BTreeSet` or `BTreeMap`.
#[must_use]
pub fn evaluate(set: &AdmissionSet, target: &str, observed: &ObservedPackage) -> Vec<Violation> {
    let Some(row) = set.rows.get(&observed.id) else {
        return vec![Violation::NoReviewedRow {
            id: observed.id.clone(),
        }];
    };
    let Some(scope) = row.scopes.get(target) else {
        return vec![Violation::TargetNotAdmitted {
            id: observed.id.clone(),
            admitted: row.scopes.keys().cloned().collect(),
        }];
    };

    let mut violations = Vec::new();
    if scope.features != observed.features {
        violations.push(Violation::FeatureSetMismatch {
            expected: scope.features.iter().cloned().collect(),
            observed: observed.features.iter().cloned().collect(),
        });
    }
    for edge in observed.edges.difference(&scope.edges) {
        violations.push(Violation::EdgeNotAdmitted {
            target: target.to_string(),
            edge: edge.clone(),
        });
    }
    for edge in scope.edges.difference(&observed.edges) {
        violations.push(Violation::EdgeMissing {
            target: target.to_string(),
            edge: edge.clone(),
        });
    }
    // An admitted edge must land on a reviewed row that is itself scoped to
    // this target. Without this, a scope could admit an edge into a package
    // nobody reviewed and the graph would still report clean.
    for edge in observed.edges.intersection(&scope.edges) {
        let reviewed_destination = set
            .rows
            .get(&edge.to)
            .is_some_and(|destination| destination.scopes.contains_key(target));
        if !reviewed_destination {
            violations.push(Violation::EdgeDestinationNotReviewed {
                target: target.to_string(),
                edge: edge.clone(),
            });
        }
    }
    violations
}

/// A resolve node as the structural pass reads it.
///
/// Edges are not carried here: the projector re-reads them into
/// [`NodeObservation`], which is the form the evaluator consumes.
struct Node {
    features: Vec<String>,
}

/// One resolve node, with whether it could be read exactly.
struct NodeObservation {
    /// Activated features, complete only when `faithful`.
    features: BTreeSet<String>,
    /// Declared edges, complete only when `faithful`.
    deps: Vec<DepRecord>,
    /// `false` when `features` or `deps` was absent, not an array, or held an
    /// entry of the wrong type. The sets above are then incomplete and the
    /// package must not be observed from them.
    faithful: bool,
}

/// One `resolve.nodes[].deps[]` entry, reduced to what identity needs.
///
/// The causal diagnostic for this edge is emitted separately by
/// [`report_dep_edge`]; this record exists so the same edge can also be given
/// an exact destination identity and flow through the evaluator.
struct DepRecord {
    /// `deps[].pkg`: the destination's opaque metadata id.
    dest_metadata_id: Option<String>,
    /// Readable dependency kinds. An unreadable entry contributes none.
    kinds: BTreeSet<DepKind>,
    /// JSON path, for findings.
    path: String,
}

/// Extract the identity half of a dependency edge.
///
/// Deliberately silent: every refusal for this edge is already produced by
/// [`report_dep_edge`], which keeps the missing/malformed markers and the
/// Build/Target/Features/Metadata precedence.
fn record_dep_edge(dep: &Json, path: &str) -> DepRecord {
    let mut kinds = BTreeSet::new();
    if let Some(entries) = dep.get("dep_kinds").and_then(Json::as_array) {
        for entry in entries {
            let Some(entry) = entry.as_object() else {
                continue;
            };
            match entry.get("kind") {
                // Cargo spells a normal dependency as an explicit null.
                Some(value) if value.is_null() => {
                    kinds.insert(DepKind::Normal);
                }
                Some(value) => match value.as_str() {
                    Some("build") => {
                        kinds.insert(DepKind::Build);
                    }
                    Some("dev") => {
                        kinds.insert(DepKind::Dev);
                    }
                    // An unknown or malformed kind has no representable
                    // identity; it stays refused by `report_dep_edge`.
                    _ => {}
                },
                None => {}
            }
        }
    }
    DepRecord {
        dest_metadata_id: dep.get("pkg").and_then(Json::as_str).map(str::to_string),
        kinds,
        path: path.to_string(),
    }
}

/// Run the admission check over `cargo metadata` output for one declared target.
///
/// `target` names the target triple the metadata was collected for. It must
/// match a frozen profile in the policy; an unknown target is refused rather
/// than checked against a default.
///
/// # Errors
///
/// Returns a message only when the metadata bytes cannot be parsed at all.
/// Every other refusal is a [`Finding`] in the returned [`Report`], so the
/// caller sees all of them rather than only the first.
pub fn check(metadata_bytes: &[u8], policy: &Policy, target: &str) -> Result<Report, String> {
    let document = parse(metadata_bytes, Limits::default())
        .map_err(|error| format!("cargo metadata output: {error}"))?;
    let mut findings = Vec::new();
    let mut inventory = Vec::new();
    let mut notes = vec![
        format!(
            "claim is bound to target profile `{target}`; cargo metadata does not record which \
             --filter-platform produced it, so the target binding rests on the caller collecting \
             this document with `--filter-platform {target}`, not on the document itself"
        ),
        "metadata establishes identity, structure and feature activation only; it does not \
         establish a runtime or downloaded-artifact closure"
            .to_string(),
    ];

    // Defensive: a `Policy` can also be built by hand, so re-check the schema
    // identity rather than trusting that it came from `parse_policy`.
    if policy.schema != ADMISSION_SCHEMA {
        return Err(format!(
            "policy schema is `{}`, this checker implements `{ADMISSION_SCHEMA}`",
            policy.schema
        ));
    }

    let profile = policy.profiles.iter().find(|p| p.target == target);
    let Some(profile) = profile else {
        findings.push(Finding::new(
            Phase::Target,
            "unknown_target_profile",
            format!("target `{target}`"),
            format!(
                "no frozen profile for this target; policy freezes {}",
                policy
                    .profiles
                    .iter()
                    .map(|p| p.target.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ));
        return Ok(Report {
            target: target.to_string(),
            findings,
            inventory,
            notes,
        });
    };

    // Project the reviewed rows into the evaluator's general form. Only local
    // rows exist, because `parse_policy` refuses every external row; the
    // evaluator itself imposes no such restriction.
    //
    // Per-target feature sets stay with the frozen profile rather than being
    // duplicated into the scope, so there remains one source of truth for them.
    let mut reviewed_rows = Vec::with_capacity(policy.local.len());
    for row in &policy.local {
        // Real per-target scope. The frozen profile must actually declare a
        // feature set for this package on this triple: `parse_policy` enforces
        // that for a loaded policy, but a `Policy` built by hand must not be
        // able to default an absent scope to "no features", which would admit
        // under a scope nobody reviewed.
        let mut scopes = BTreeMap::new();
        for triple in &row.targets {
            let Some(profile) = policy.profiles.iter().find(|p| &p.target == triple) else {
                return Err(format!(
                    "dependency policy admits `{} {}` on `{triple}` but freezes no target profile \
                     for that triple; an absent profile is not an empty scope",
                    row.name, row.version
                ));
            };
            let Some(features) = profile.expected_features.get(&row.name) else {
                return Err(format!(
                    "target profile `{triple}` freezes no feature set for admitted package `{}`; \
                     an absent scope must refuse, never default to none",
                    row.name
                ));
            };
            scopes.insert(
                triple.clone(),
                TargetScope {
                    features: features.iter().cloned().collect(),
                    // The policy admits no edges today: the schema refuses
                    // every external row and the workspace has no
                    // intra-workspace dependency. This is the real admitted
                    // set, not a placeholder.
                    edges: BTreeSet::new(),
                },
            );
        }
        reviewed_rows.push(ReviewedRow {
            id: PackageId {
                source: SourceId::WorkspacePath {
                    manifest_path: row.manifest_path.clone(),
                },
                name: row.name.clone(),
                version: row.version.clone(),
            },
            scopes,
        });
    }
    let reviewed =
        AdmissionSet::new(reviewed_rows).map_err(|AdmissionSetError::DuplicateIdentity(id)| {
            format!(
                "dependency policy admits `{} {}` from the same source twice; a duplicate \
                 identity would let one row silently replace another",
                id.name, id.version
            )
        })?;

    let Some(root) = req_object(&document, Phase::Metadata, "$", &mut findings) else {
        return Ok(Report {
            target: target.to_string(),
            findings,
            inventory,
            notes,
        });
    };

    // -- Phase Metadata -----------------------------------------------------
    match root.get("version").and_then(Json::as_u64) {
        Some(version) if version == policy.rules.metadata_version => {}
        Some(version) => findings.push(Finding::new(
            Phase::Metadata,
            "metadata_version_mismatch",
            "$.version",
            format!(
                "policy requires format version {}, found {version}",
                policy.rules.metadata_version
            ),
        )),
        None => findings.push(Finding::new(
            Phase::Metadata,
            "metadata_version_missing",
            "$.version",
            "required integer field `version` is absent or not a number",
        )),
    }

    let workspace_root = match root.get("workspace_root") {
        Some(value) => {
            req_str(value, Phase::Metadata, "$.workspace_root", &mut findings).map(str::to_string)
        }
        None => {
            findings.push(missing(Phase::Metadata, "$", "workspace_root"));
            None
        }
    };

    // -- Phase Graph: resolve must exist, be well formed, and have unique ids -
    let mut nodes: BTreeMap<String, Node> = BTreeMap::new();
    let mut resolve_usable = false;
    match root.get("resolve") {
        None => findings.push(Finding::new(
            Phase::Metadata,
            "resolve_absent",
            "$.resolve",
            "no `resolve` section; metadata was not produced with a resolved graph",
        )),
        Some(value) if value.is_null() => findings.push(Finding::new(
            Phase::Metadata,
            "resolve_null_no_deps",
            "$.resolve",
            "`resolve` is null, which Cargo emits for --no-deps; --no-deps is not a transitive \
             audit and must never be read as an empty passing graph",
        )),
        Some(value) => {
            if let Some(resolve) = req_object(value, Phase::Graph, "$.resolve", &mut findings) {
                match resolve.get("nodes") {
                    None => findings.push(missing(Phase::Graph, "$.resolve", "nodes")),
                    Some(nodes_value) => {
                        if let Some(items) =
                            req_array(nodes_value, Phase::Graph, "$.resolve.nodes", &mut findings)
                        {
                            resolve_usable = true;
                            for (index, node) in items.iter().enumerate() {
                                let path = format!("$.resolve.nodes[{index}]");
                                let Some(node) =
                                    req_object(node, Phase::Graph, &path, &mut findings)
                                else {
                                    resolve_usable = false;
                                    continue;
                                };
                                let id = req_field(node, "id", Phase::Graph, &path, &mut findings)
                                    .and_then(|v| req_str(v, Phase::Graph, &path, &mut findings));
                                let features = req_string_array(
                                    node,
                                    "features",
                                    Phase::Features,
                                    &path,
                                    &mut findings,
                                );
                                // `dependencies` and `deps` are both required.
                                // Their absence must not read as "no edges".
                                let dependencies = req_field(
                                    node,
                                    "dependencies",
                                    Phase::Graph,
                                    &path,
                                    &mut findings,
                                )
                                .and_then(|v| {
                                    req_array(
                                        v,
                                        Phase::Graph,
                                        &format!("{path}.dependencies"),
                                        &mut findings,
                                    )
                                });
                                let deps =
                                    req_field(node, "deps", Phase::Graph, &path, &mut findings)
                                        .and_then(|v| {
                                            req_array(
                                                v,
                                                Phase::Graph,
                                                &format!("{path}.deps"),
                                                &mut findings,
                                            )
                                        });

                                if let Some(dependencies) = dependencies
                                    && !dependencies.is_empty()
                                {
                                    findings.push(Finding::new(
                                        Phase::Graph,
                                        "resolved_dependency_present",
                                        format!("{path}.dependencies"),
                                        format!(
                                            "{} resolved dependency id(s); the closed inventory \
                                             has none",
                                            dependencies.len()
                                        ),
                                    ));
                                }
                                if let Some(deps) = deps {
                                    for (dep_index, dep) in deps.iter().enumerate() {
                                        let dep_path = format!("{path}.deps[{dep_index}]");
                                        report_dep_edge(dep, &dep_path, &mut findings);
                                    }
                                }

                                if let (Some(id), Some(mut features)) = (id, features) {
                                    features.sort();
                                    if nodes.insert(id.to_string(), Node { features }).is_some() {
                                        findings.push(Finding::new(
                                            Phase::Graph,
                                            "duplicate_resolve_node",
                                            path.clone(),
                                            format!(
                                                "package id `{id}` appears more than once in \
                                                 resolve.nodes; a later node would otherwise \
                                                 overwrite an earlier one"
                                            ),
                                        ));
                                        resolve_usable = false;
                                    }
                                } else {
                                    resolve_usable = false;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // -- Phase Inventory / Source / Build / Features -------------------------
    let Some(packages_value) = root.get("packages") else {
        findings.push(missing(Phase::Inventory, "$", "packages"));
        return Ok(Report {
            target: target.to_string(),
            findings,
            inventory,
            notes,
        });
    };
    let Some(packages) = req_array(
        packages_value,
        Phase::Inventory,
        "$.packages",
        &mut findings,
    ) else {
        return Ok(Report {
            target: target.to_string(),
            findings,
            inventory,
            notes,
        });
    };

    let mut package_ids: BTreeSet<String> = BTreeSet::new();
    let mut matched_rows: BTreeSet<String> = BTreeSet::new();

    for (index, package) in packages.iter().enumerate() {
        let path = format!("$.packages[{index}]");
        let Some(package) = req_object(package, Phase::Inventory, &path, &mut findings) else {
            continue;
        };

        let name = req_field(package, "name", Phase::Inventory, &path, &mut findings)
            .and_then(|v| req_str(v, Phase::Inventory, &path, &mut findings));
        let version = req_field(package, "version", Phase::Inventory, &path, &mut findings)
            .and_then(|v| req_str(v, Phase::Inventory, &path, &mut findings));
        let id = req_field(package, "id", Phase::Graph, &path, &mut findings)
            .and_then(|v| req_str(v, Phase::Graph, &path, &mut findings));
        let manifest_path = req_field(
            package,
            "manifest_path",
            Phase::Source,
            &path,
            &mut findings,
        )
        .and_then(|v| req_str(v, Phase::Source, &path, &mut findings));
        let source = req_null_or_string(package, "source", Phase::Source, &path, &mut findings);
        let links = req_null_or_string(package, "links", Phase::Build, &path, &mut findings);

        // Package-level declared dependencies must be present and empty.
        if let Some(value) = req_field(package, "dependencies", Phase::Build, &path, &mut findings)
            && let Some(items) = req_array(
                value,
                Phase::Build,
                &format!("{path}.dependencies"),
                &mut findings,
            )
        {
            for (dep_index, dep) in items.iter().enumerate() {
                let dep_path = format!("{path}.dependencies[{dep_index}]");
                report_manifest_dependency(dep, &dep_path, &mut findings);
            }
        }

        let target_kinds =
            collect_target_kinds(package, &path, workspace_root.as_deref(), &mut findings);

        let (Some(name), Some(version), Some(id), Some(manifest_path), Some(source), Some(links)) =
            (name, version, id, manifest_path, source, links)
        else {
            continue;
        };

        package_ids.insert(id.to_string());

        let relative = match workspace_root
            .as_deref()
            .map(|root| relative_manifest(manifest_path, root))
        {
            Some(Ok(relative)) => relative,
            Some(Err(reason)) => {
                findings.push(Finding::new(
                    Phase::Source,
                    "manifest_path_not_contained",
                    format!("{path}.manifest_path"),
                    format!("`{manifest_path}` rejected: {reason}"),
                ));
                manifest_path.to_string()
            }
            None => manifest_path.to_string(),
        };

        // -- Build prohibitions. These are unconditional: no policy flag can
        //    reach them, because a policy may not relax a prohibition.
        if target_kinds.iter().any(|kind| kind == "custom-build") {
            findings.push(Finding::new(
                Phase::Build,
                "build_script_present",
                format!("{path}.targets"),
                format!(
                    "package `{name}` carries a build script (`custom-build` target); a build \
                     script executes arbitrary code at build time"
                ),
            ));
        }
        if target_kinds.iter().any(|kind| kind == "proc-macro") {
            findings.push(Finding::new(
                Phase::Build,
                "proc_macro_present",
                format!("{path}.targets"),
                format!("package `{name}` is a proc macro; proc macros execute at compile time"),
            ));
        }
        if let Some(links) = links.as_deref() {
            findings.push(Finding::new(
                Phase::Build,
                "native_link_declared",
                format!("{path}.links"),
                format!("package `{name}` declares native `links = \"{links}\"`"),
            ));
        }

        // -- Source. External is refused unconditionally.
        let is_local = source.is_none();
        if let Some(source) = source.as_deref() {
            findings.push(Finding::new(
                Phase::Source,
                "unsupported_external_admission",
                path.clone(),
                format!(
                    "package `{name} {version}` resolves from external source `{source}`. This \
                     checker admits no external package under any policy row: Cargo metadata \
                     cannot establish a checksum, a build-script effect, or a downloaded-artifact \
                     closure, so approving one here would be admission on absent evidence"
                ),
            ));
        } else {
            match policy
                .local
                .iter()
                .find(|row| row.name == name && row.version == version)
            {
                None => findings.push(Finding::new(
                    Phase::Inventory,
                    "local_package_not_admitted",
                    path.clone(),
                    format!(
                        "local package `{name} {version}` at `{relative}` has no admitted row; \
                         enlarging the workspace is itself an admission event"
                    ),
                )),
                Some(row) => {
                    matched_rows.insert(format!("{name} {version}"));
                    if row.manifest_path != relative {
                        findings.push(Finding::new(
                            Phase::Source,
                            "manifest_path_mismatch",
                            format!("{path}.manifest_path"),
                            format!(
                                "admitted row expects `{}`, metadata resolves `{relative}`",
                                row.manifest_path
                            ),
                        ));
                    }
                }
            }
        }

        // -- Features, against the frozen profile for this target.
        let activated = if resolve_usable {
            match nodes.get(id) {
                Some(node) => node.features.clone(),
                None => {
                    findings.push(Finding::new(
                        Phase::Graph,
                        "package_missing_from_resolve",
                        path.clone(),
                        format!(
                            "package id `{id}` has no node in resolve.nodes; its feature \
                             activation is unknown and unknown is not empty"
                        ),
                    ));
                    Vec::new()
                }
            }
        } else {
            findings.push(Finding::new(
                Phase::Features,
                "feature_set_unverifiable",
                path.clone(),
                format!(
                    "resolve graph is unusable, so the activated feature set of `{name}` could \
                     not be established; this is a refusal, not an empty feature set"
                ),
            ));
            Vec::new()
        };

        // The frozen profile must be explicit about every package it covers.
        // Feature comparison itself is the evaluator's, below.
        if !profile.expected_features.contains_key(name) {
            findings.push(Finding::new(
                Phase::Target,
                "package_absent_from_profile",
                path.clone(),
                format!(
                    "target profile `{target}` freezes no feature set for `{name}`; a profile \
                     that omits a package is silent on it, never implicitly covering it"
                ),
            ));
        }

        inventory.push(PackageFacts {
            name: name.to_string(),
            version: version.to_string(),
            id: id.to_string(),
            manifest_path: relative,
            is_local,
            source,
            target_kinds,
            links,
            activated_features: activated,
        });
    }

    // -- Admission, decided by the shared evaluator --------------------------
    //
    // Runs only now, once every package identity and the resolve graph are
    // available, over observations carrying the real activated features and
    // every real dependency edge resolved to its exact destination identity.
    // A package the projection could not observe faithfully is absent here and
    // already carries its own refusal, so nothing is evaluated as empty.
    for observed in project_observed_packages(&document, &mut findings) {
        let location = format!("{} {}", observed.id.name, observed.id.version);
        for violation in evaluate(&reviewed, target, &observed) {
            match violation {
                // Keep every evaluator refusal. Package-level diagnostics are
                // more specific, but can never substitute for this decision.
                Violation::NoReviewedRow { id } => findings.push(Finding::new(
                    Phase::Source,
                    "identity_not_reviewed",
                    location.clone(),
                    format!(
                        "no reviewed row for exact source {:?}, name `{}`, version `{}`",
                        id.source, id.name, id.version
                    ),
                )),
                Violation::TargetNotAdmitted { admitted, .. } => findings.push(Finding::new(
                    Phase::Target,
                    "package_not_admitted_for_target",
                    location.clone(),
                    format!(
                        "`{location}` is reviewed, but its row admits only [{}]; this collection \
                         is for `{target}`",
                        admitted.join(", ")
                    ),
                )),
                Violation::FeatureSetMismatch { expected, observed } => {
                    findings.push(Finding::new(
                        Phase::Features,
                        "feature_set_mismatch",
                        location.clone(),
                        format!(
                            "target `{target}` freezes features [{}] for `{location}`, metadata \
                             activates [{}]",
                            expected.join(", "),
                            observed.join(", ")
                        ),
                    ));
                }
                Violation::EdgeNotAdmitted { target, edge } => findings.push(Finding::new(
                    Phase::Target,
                    "edge_outside_target_scope",
                    location.clone(),
                    format!(
                        "edge to `{} {}` as a `{}` dependency is not in the reviewed scope for \
                         `{target}`",
                        edge.to.name,
                        edge.to.version,
                        edge.kind.as_str()
                    ),
                )),
                Violation::EdgeMissing { target, edge } => findings.push(Finding::new(
                    Phase::Graph,
                    "admitted_edge_absent",
                    location.clone(),
                    format!(
                        "the reviewed scope for `{target}` requires an edge to `{} {}` as a `{}` \
                         dependency, and it is absent",
                        edge.to.name,
                        edge.to.version,
                        edge.kind.as_str()
                    ),
                )),
                Violation::EdgeDestinationNotReviewed { target, edge } => {
                    findings.push(Finding::new(
                        Phase::Graph,
                        "edge_destination_not_reviewed",
                        location.clone(),
                        format!(
                            "an admitted edge points at `{} {}`, which has no reviewed row scoped \
                             to `{target}`",
                            edge.to.name, edge.to.version
                        ),
                    ));
                }
            }
        }
    }

    // -- Phase Graph: exact bijection packages <-> resolve.nodes <-> members --
    if resolve_usable {
        let node_ids: BTreeSet<String> = nodes.keys().cloned().collect();
        for extra in node_ids.difference(&package_ids) {
            findings.push(Finding::new(
                Phase::Graph,
                "resolve_node_without_package",
                "$.resolve.nodes",
                format!("resolve node `{extra}` has no corresponding entry in $.packages"),
            ));
        }
        for missing_id in package_ids.difference(&node_ids) {
            findings.push(Finding::new(
                Phase::Graph,
                "package_missing_from_resolve",
                "$.packages",
                format!("package `{missing_id}` has no node in $.resolve.nodes"),
            ));
        }
    }

    match root.get("workspace_members") {
        None => findings.push(missing(Phase::Graph, "$", "workspace_members")),
        Some(value) => {
            if let Some(items) =
                req_array(value, Phase::Graph, "$.workspace_members", &mut findings)
            {
                let mut members = BTreeSet::new();
                for (index, item) in items.iter().enumerate() {
                    match req_str(
                        item,
                        Phase::Graph,
                        &format!("$.workspace_members[{index}]"),
                        &mut findings,
                    ) {
                        Some(text) => {
                            if !members.insert(text.to_string()) {
                                findings.push(Finding::new(
                                    Phase::Graph,
                                    "duplicate_workspace_member",
                                    "$.workspace_members",
                                    format!("`{text}` is listed more than once"),
                                ));
                            }
                        }
                        None => continue,
                    }
                }
                for extra in members.difference(&package_ids) {
                    findings.push(Finding::new(
                        Phase::Graph,
                        "workspace_member_without_package",
                        "$.workspace_members",
                        format!("workspace member `{extra}` has no entry in $.packages"),
                    ));
                }
                for extra in package_ids.difference(&members) {
                    findings.push(Finding::new(
                        Phase::Graph,
                        "package_outside_workspace",
                        "$.packages",
                        format!(
                            "package `{extra}` is not a workspace member; in the closed inventory \
                             every resolved package is a local workspace member"
                        ),
                    ));
                }
            }
        }
    }

    // -- Phase Inventory: every admitted row must actually be present ---------
    for row in &policy.local {
        let key = format!("{} {}", row.name, row.version);
        if !matched_rows.contains(&key) {
            findings.push(Finding::new(
                Phase::Inventory,
                "admitted_row_absent",
                format!("$.admission.local_packages `{key}`"),
                format!(
                    "policy admits this package (owner `{}`) but metadata does not resolve it",
                    row.owner
                ),
            ));
        }
    }

    if inventory.len() != policy.local.len() {
        findings.push(Finding::new(
            Phase::Inventory,
            "inventory_size_mismatch",
            "$.packages",
            format!(
                "policy admits {} package(s), metadata resolved {}",
                policy.local.len(),
                inventory.len()
            ),
        ));
    }

    if !findings.is_empty() {
        notes.push("no admission was granted; the findings below are the reasons".to_string());
    }

    findings.sort_by(|a, b| {
        a.phase
            .cmp(&b.phase)
            .then_with(|| a.code.cmp(b.code))
            .then_with(|| a.location.cmp(&b.location))
    });

    Ok(Report {
        target: target.to_string(),
        findings,
        inventory,
        notes,
    })
}

/// Record a resolved dependency edge, naming its kind and target so the refusal
/// is causal rather than a generic "graph changed".
fn report_dep_edge(dep: &Json, path: &str, findings: &mut Vec<Finding>) {
    let name = dep
        .get("name")
        .and_then(Json::as_str)
        .or_else(|| dep.get("pkg").and_then(Json::as_str))
        .unwrap_or("<unnamed>");
    // Every declared kind and target is collected before anything is decided,
    // so the classification cannot depend on which `dep_kinds` entry happens to
    // come last. Reversing a mixed build/target list previously changed both
    // the phase and the detail, which made the diagnostic an artifact of
    // Cargo's emission order rather than of the dependency.
    // Only an explicit JSON null is Cargo's spelling of a normal dependency.
    // A missing key and a wrong-typed value are neither normal nor a fact
    // about the edge, so each is kept as its own marker instead of being
    // folded into `normal`.
    let mut real_kinds: BTreeSet<String> = BTreeSet::new();
    let mut real_targets: BTreeSet<String> = BTreeSet::new();
    let mut anomalies: BTreeSet<String> = BTreeSet::new();
    let mut explicit_normal = false;

    if let Some(entries) = dep.get("dep_kinds").and_then(Json::as_array) {
        for entry in entries {
            let Some(entry) = entry.as_object() else {
                anomalies.insert(format!("<entry:malformed:{}>", entry.kind()));
                continue;
            };
            match entry.get("kind") {
                None => {
                    anomalies.insert("<kind:missing>".to_string());
                }
                Some(value) if value.is_null() => explicit_normal = true,
                Some(value) => match value.as_str() {
                    // Cargo spells a normal dependency as null, so any string
                    // here is a non-normal kind such as `build` or `dev`.
                    Some(kind) => {
                        real_kinds.insert(kind.to_string());
                    }
                    None => {
                        anomalies.insert(format!("<kind:malformed:{}>", value.kind()));
                    }
                },
            }
            match entry.get("target") {
                None => {
                    anomalies.insert("<target:missing>".to_string());
                }
                Some(value) if value.is_null() => {}
                Some(value) => match value.as_str() {
                    Some(target) => {
                        real_targets.insert(target.to_string());
                    }
                    None => {
                        anomalies.insert(format!("<target:malformed:{}>", value.kind()));
                    }
                },
            }
        }
    } else if dep.get("dep_kinds").is_some() {
        anomalies.insert("<dep_kinds:malformed>".to_string());
    } else {
        anomalies.insert("<dep_kinds:missing>".to_string());
    }

    // Exact precedence, evaluated over the whole set rather than the last
    // entry seen. Real facts dominate anomalies, because a genuine
    // target-scoped or build activation is more specific than an unreadable
    // field; an anomaly only decides the phase when nothing real was declared:
    //
    //   1. Target   any real target string is present
    //   2. Build    else any real kind string is present (Cargo uses null for
    //               normal, so a string is always non-normal)
    //   3. Metadata else any missing or malformed marker is present
    //   4. Features else the edge is an ordinary dependency
    //
    // The refusal is unconditional in every case: this chooses how the edge is
    // described, never whether it is admitted.
    let phase = if !real_targets.is_empty() {
        Phase::Target
    } else if !real_kinds.is_empty() {
        Phase::Build
    } else if !anomalies.is_empty() {
        Phase::Metadata
    } else {
        Phase::Features
    };

    let mut declared_kinds = real_kinds;
    if explicit_normal {
        declared_kinds.insert(NORMAL_DEP_KIND.to_string());
    }
    let render = |set: &BTreeSet<String>, empty: &str| {
        if set.is_empty() {
            empty.to_string()
        } else {
            set.iter().cloned().collect::<Vec<_>>().join(", ")
        }
    };

    findings.push(Finding::new(
        phase,
        "unadmitted_dependency_edge",
        path.to_string(),
        format!(
            "resolved dependency edge on `{name}` is not in the admitted inventory; declared \
             kinds [{}], targets [{}], anomalies [{}]",
            render(&declared_kinds, "none declared"),
            render(&real_targets, "none"),
            render(&anomalies, "none")
        ),
    ));
}

/// Record a manifest-declared dependency, naming its kind and target.
fn report_manifest_dependency(dep: &Json, path: &str, findings: &mut Vec<Finding>) {
    let name = dep
        .get("name")
        .and_then(Json::as_str)
        .unwrap_or("<unnamed>");
    let kind = dep.get("kind").and_then(Json::as_str);
    let target = dep.get("target").and_then(Json::as_str);
    let (phase, detail) = match (kind, target) {
        (_, Some(target)) => (
            Phase::Target,
            format!(
                "manifest declares dependency `{name}` for target `{target}` only; a \
                 target-scoped dependency is still an admission event"
            ),
        ),
        (Some(kind), None) => (
            Phase::Build,
            format!("manifest declares `{name}` as a `{kind}` dependency"),
        ),
        (None, None) => (
            Phase::Build,
            format!("manifest declares a normal dependency on `{name}`"),
        ),
    };
    findings.push(Finding::new(
        phase,
        "unadmitted_manifest_dependency",
        path.to_string(),
        detail,
    ));
}

/// Collect target kinds and crate types.
///
/// `targets`, and each target's `kind` and `crate_types`, are required arrays of
/// strings, and an empty one is refused: an empty kind list would silently
/// exempt a package from the build-script and proc-macro prohibitions.
fn collect_target_kinds(
    package: &BTreeMap<String, Json>,
    path: &str,
    workspace_root: Option<&str>,
    findings: &mut Vec<Finding>,
) -> Vec<String> {
    let mut kinds: BTreeSet<String> = BTreeSet::new();
    let Some(value) = req_field(package, "targets", Phase::Build, path, findings) else {
        return Vec::new();
    };
    let Some(targets) = req_array(value, Phase::Build, &format!("{path}.targets"), findings) else {
        return Vec::new();
    };
    if targets.is_empty() {
        findings.push(Finding::new(
            Phase::Build,
            "empty_target_list",
            format!("{path}.targets"),
            "package declares no targets; an empty target list would exempt it from the \
             build-script and proc-macro checks",
        ));
        return Vec::new();
    }
    for (index, target) in targets.iter().enumerate() {
        let target_path = format!("{path}.targets[{index}]");
        let Some(target) = req_object(target, Phase::Build, &target_path, findings) else {
            continue;
        };

        // The manifest location being admitted says nothing about where the
        // target's source actually lives: a manifest inside the workspace can
        // point a target at `../../outside/foreign.rs`, which Cargo resolves to
        // an absolute path outside the workspace. Admitting that would report a
        // closed local inventory while compiling foreign source.
        if let Some(src_path) = req_field(target, "src_path", Phase::Source, &target_path, findings)
            .and_then(|value| {
                req_str(
                    value,
                    Phase::Source,
                    &format!("{target_path}.src_path"),
                    findings,
                )
            })
        {
            if let Some(root) = workspace_root {
                if let Err(reason) = relative_manifest(src_path, root) {
                    findings.push(Finding::new(
                        Phase::Source,
                        "target_src_path_not_contained",
                        format!("{target_path}.src_path"),
                        format!(
                            "`{src_path}` rejected: {reason}. A target source outside the \
                             workspace is not covered by the admitted manifest"
                        ),
                    ));
                }
            } else {
                findings.push(Finding::new(
                    Phase::Source,
                    "target_src_path_uncheckable",
                    format!("{target_path}.src_path"),
                    "workspace_root is unavailable, so target source containment could not be \
                     established; this is a refusal, not a pass",
                ));
            }
        }

        for key in ["kind", "crate_types"] {
            match req_string_array(target, key, Phase::Build, &target_path, findings) {
                Some(items) if items.is_empty() => findings.push(Finding::new(
                    Phase::Build,
                    "empty_target_kind",
                    format!("{target_path}.{key}"),
                    format!("`{key}` is empty; an empty kind list is not a kind"),
                )),
                Some(items) => kinds.extend(items),
                None => {}
            }
        }
    }
    kinds.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real captured output of
    /// `cargo metadata --format-version 1 --locked --offline --filter-platform
    /// aarch64-apple-darwin`, exit 0, collected on the macOS operator host on
    /// 2026-09-07 and stored verbatim. `Cargo.lock` was hashed before and after
    /// the collection and was unchanged.
    ///
    /// This is the only fixture that can support a per-target admission,
    /// because it is the only one whose collection actually named a target.
    const FILTERED_METADATA: &str =
        include_str!("../tests/fixtures/admission/metadata-filtered-aarch64-apple-darwin.json");

    /// The admission block proposed for `registry/dependency_policy.json`
    /// (FA-053). Root owns integrating it; this copy is what tests compile
    /// against.
    const POLICY: &str = include_str!("../tests/fixtures/admission/policy-closed-inventory.json");

    const TARGET: &str = "aarch64-apple-darwin";

    fn policy() -> Policy {
        parse_policy(POLICY.as_bytes()).expect("proposed admission policy parses")
    }

    /// Check `metadata` for the admitted target profile.
    fn check_filtered(metadata: &str) -> Report {
        check(metadata.as_bytes(), &policy(), TARGET).expect("metadata parses")
    }

    fn codes(report: &Report) -> Vec<(Phase, &'static str)> {
        report.findings.iter().map(|f| (f.phase, f.code)).collect()
    }

    fn assert_refused(report: &Report, phase: Phase, code: &'static str) {
        assert!(
            codes(report).contains(&(phase, code)),
            "expected refusal [{}] {code}; got {:?}",
            phase.as_str(),
            report.findings
        );
        assert!(!report.is_admitted(), "a refusal finding cannot admit");
    }

    /// Replace the first occurrence of `from` with `to`, asserting it existed,
    /// so fixture drift turns into a test failure rather than a silent no-op
    /// that leaves the negative testing nothing.
    fn mutate(source: &str, from: &str, to: &str) -> String {
        assert!(source.contains(from), "fixture no longer contains {from:?}");
        source.replacen(from, to, 1)
    }

    // ---- positive ----

    #[test]
    fn filtered_metadata_is_admitted_under_the_closed_inventory() {
        let report = check_filtered(FILTERED_METADATA);
        assert!(
            report.is_admitted(),
            "the real filtered capture must be admitted; findings: {:?}",
            report.findings
        );
    }

    #[test]
    fn positive_inventory_is_exact_and_relative() {
        let report = check_filtered(FILTERED_METADATA);
        assert_eq!(report.inventory.len(), 2);
        let names: Vec<&str> = report.inventory.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["fa-reference", "xtask"]);
        let paths: Vec<&str> = report
            .inventory
            .iter()
            .map(|p| p.manifest_path.as_str())
            .collect();
        assert_eq!(
            paths,
            vec!["crates/fa-reference/Cargo.toml", "xtask/Cargo.toml"]
        );
        assert!(report.inventory.iter().all(|p| p.is_local));
        assert!(report.inventory.iter().all(|p| p.links.is_none()));
        assert!(
            report
                .inventory
                .iter()
                .all(|p| p.activated_features.is_empty())
        );
    }

    #[test]
    fn positive_report_states_its_claim_boundary_and_target() {
        let report = check_filtered(FILTERED_METADATA);
        let rendered = report.render();
        assert!(rendered.contains("PASS admission_closed_inventory[aarch64-apple-darwin]"));
        assert!(
            rendered.contains("Not a runtime or downloaded-artifact closure claim"),
            "a passing report must not read as a full closure audit"
        );
        assert!(
            rendered.contains("--filter-platform"),
            "a passing report must say the target binding rests on the invocation"
        );
    }

    // ---- target source containment ----

    #[test]
    fn target_src_path_outside_the_workspace_is_refused() {
        // A manifest inside the workspace can point a target at
        // `../../outside/foreign.rs`, which Cargo resolves to an absolute path
        // outside the workspace. `manifest_path` containment does not cover it.
        let mutated = mutate(
            FILTERED_METADATA,
            "\"src_path\":\"/Users/jemanuel/projects/franken_alignment/xtask/src/main.rs\"",
            "\"src_path\":\"/outside/foreign.rs\"",
        );
        let report = check_filtered(&mutated);
        assert_refused(&report, Phase::Source, "target_src_path_not_contained");
    }

    #[test]
    fn target_src_path_in_a_sibling_of_the_workspace_is_refused() {
        let mutated = mutate(
            FILTERED_METADATA,
            "\"src_path\":\"/Users/jemanuel/projects/franken_alignment/xtask/src/main.rs\"",
            "\"src_path\":\"/Users/jemanuel/projects/franken_alignment_evil/xtask/src/main.rs\"",
        );
        let report = check_filtered(&mutated);
        assert_refused(&report, Phase::Source, "target_src_path_not_contained");
    }

    #[test]
    fn absent_target_src_path_is_refused() {
        let mutated = mutate(
            FILTERED_METADATA,
            ",\"src_path\":\"/Users/jemanuel/projects/franken_alignment/xtask/src/main.rs\"",
            "",
        );
        let report = check_filtered(&mutated);
        assert_refused(&report, Phase::Source, "missing_field");
    }

    // ---- target profile must be bound ----

    #[test]
    fn unknown_target_profile_is_refused() {
        let report = check(
            FILTERED_METADATA.as_bytes(),
            &policy(),
            "riscv64gc-unknown-linux-gnu",
        )
        .expect("parses");
        assert_refused(&report, Phase::Target, "unknown_target_profile");
    }

    // ---- external is never admissible ----

    #[test]
    fn external_source_is_refused_unconditionally() {
        let mutated = mutate(
            FILTERED_METADATA,
            "\"license_file\":\"../LICENSE\",\"description\":null,\"source\":null",
            "\"license_file\":\"../LICENSE\",\"description\":null,\"source\":\"registry+https://github.com/rust-lang/crates.io-index\"",
        );
        let report = check_filtered(&mutated);
        assert_refused(&report, Phase::Source, "unsupported_external_admission");
    }

    #[test]
    fn policy_carrying_any_external_row_is_a_hard_error() {
        let permissive = br#"{"admission":{"schema":"fa.admission/0.1","local_packages":[
            {"name":"xtask","version":"0.2.0","manifest_path":"xtask/Cargo.toml",
             "owner":"systems","decision":"admitted"}],
            "external_rows":[{"name":"serde","version":"1.0.0","source":"registry+x",
            "runtime_closure_inspected":true}],
            "target_profiles":[{"target":"t","expected_features":{"xtask":[]}}],
            "rules":{"metadata_version":1,"require_resolve_section":true,
            "allow_build_scripts":false,"allow_proc_macro":false,"allow_links":false}}}"#;
        let error = parse_policy(permissive).expect_err("must refuse");
        assert!(error.contains("external_rows"), "{error}");
    }

    // ---- a policy may not relax a prohibition ----

    #[test]
    fn policy_enabling_build_scripts_is_a_hard_error() {
        let relaxed = POLICY.replace(
            "\"allow_build_scripts\": false",
            "\"allow_build_scripts\": true",
        );
        let error = parse_policy(relaxed.as_bytes()).expect_err("must refuse");
        assert!(error.contains("allow_build_scripts"), "{error}");
    }

    #[test]
    fn policy_omitting_a_prohibition_is_a_hard_error() {
        // `allow_proc_macro` rather than `allow_links` because it is not the
        // last key in `rules`: removing a trailing-comma-bearing entry leaves
        // valid JSON, so the parse reaches the prohibition check instead of
        // failing earlier for an unrelated syntax reason.
        let needle = "\"allow_proc_macro\": false,";
        assert!(
            POLICY.contains(needle),
            "fixture no longer contains {needle:?}"
        );
        let stripped = POLICY.replace(needle, "");
        let error = parse_policy(stripped.as_bytes()).expect_err("must refuse");
        assert!(error.contains("allow_proc_macro"), "{error}");
    }

    #[test]
    fn build_script_is_refused_even_though_no_flag_can_reach_it() {
        let mutated = mutate(
            FILTERED_METADATA,
            "\"kind\":[\"bin\"]",
            "\"kind\":[\"custom-build\"]",
        );
        let report = check_filtered(&mutated);
        assert_refused(&report, Phase::Build, "build_script_present");
    }

    #[test]
    fn proc_macro_is_refused() {
        let mutated = mutate(
            FILTERED_METADATA,
            "\"crate_types\":[\"lib\"]",
            "\"crate_types\":[\"proc-macro\"]",
        );
        let report = check_filtered(&mutated);
        assert_refused(&report, Phase::Build, "proc_macro_present");
    }

    #[test]
    fn native_links_is_refused() {
        let mutated = mutate(FILTERED_METADATA, "\"links\":null", "\"links\":\"sqlite3\"");
        let report = check_filtered(&mutated);
        assert_refused(&report, Phase::Build, "native_link_declared");
    }

    // ---- path containment, not byte prefix ----

    #[test]
    fn sibling_prefix_is_not_containment() {
        assert_eq!(
            relative_manifest("/w/root/xtask/Cargo.toml", "/w/root").as_deref(),
            Ok("xtask/Cargo.toml")
        );
        // The bug this closes: a byte-prefix strip would turn this into
        // "xtask/Cargo.toml" and admit a sibling directory.
        assert!(relative_manifest("/w/rootxtask/Cargo.toml", "/w/root").is_err());
        assert!(relative_manifest("/elsewhere/xtask/Cargo.toml", "/w/root").is_err());
        assert!(relative_manifest("/w/root/../evil/Cargo.toml", "/w/root").is_err());
        assert!(relative_manifest("relative/Cargo.toml", "/w/root").is_err());
        assert!(relative_manifest("/w/root", "/w/root").is_err());
    }

    #[test]
    fn manifest_outside_workspace_root_is_refused() {
        let mutated = mutate(
            FILTERED_METADATA,
            "\"manifest_path\":\"/Users/jemanuel/projects/franken_alignment/xtask/Cargo.toml\"",
            "\"manifest_path\":\"/Users/jemanuel/projects/franken_alignment_evil/xtask/Cargo.toml\"",
        );
        let report = check_filtered(&mutated);
        assert_refused(&report, Phase::Source, "manifest_path_not_contained");
    }

    #[test]
    fn moved_manifest_inside_the_workspace_is_refused() {
        let mutated = mutate(
            FILTERED_METADATA,
            "/xtask/Cargo.toml",
            "/tools/xtask/Cargo.toml",
        );
        let report = check_filtered(&mutated);
        assert_refused(&report, Phase::Source, "manifest_path_mismatch");
    }

    // ---- absence and malformation never launder into a pass ----

    #[test]
    fn no_deps_metadata_is_refused_not_treated_as_empty() {
        let mutated = mutate(
            FILTERED_METADATA,
            "\"resolve\":{\"nodes\":",
            "\"resolve_disabled\":{\"nodes\":",
        );
        let mutated = mutate(
            &mutated,
            "\"target_directory\"",
            "\"resolve\":null,\"target_directory\"",
        );
        let report = check_filtered(&mutated);
        assert_refused(&report, Phase::Metadata, "resolve_null_no_deps");
    }

    #[test]
    fn absent_resolve_nodes_is_refused() {
        let mutated = mutate(
            FILTERED_METADATA,
            "\"resolve\":{\"nodes\":[",
            "\"resolve\":{\"other\":[",
        );
        let report = check_filtered(&mutated);
        assert_refused(&report, Phase::Graph, "missing_field");
    }

    #[test]
    fn absent_node_features_is_refused_not_read_as_no_features() {
        let mutated = mutate(
            FILTERED_METADATA,
            "\"dependencies\":[],\"deps\":[],\"features\":[]}",
            "\"dependencies\":[],\"deps\":[]}",
        );
        let report = check_filtered(&mutated);
        assert_refused(&report, Phase::Features, "missing_field");
    }

    #[test]
    fn absent_node_deps_is_refused_not_read_as_no_edges() {
        let mutated = mutate(
            FILTERED_METADATA,
            "\"dependencies\":[],\"deps\":[],\"features\":[]}",
            "\"dependencies\":[],\"features\":[]}",
        );
        let report = check_filtered(&mutated);
        assert_refused(&report, Phase::Graph, "missing_field");
    }

    #[test]
    fn duplicate_resolve_node_is_refused_not_overwritten() {
        let node = "{\"id\":\"path+file:///Users/jemanuel/projects/franken_alignment/xtask#0.2.0\",\"dependencies\":[],\"deps\":[],\"features\":[]}";
        let mutated = mutate(FILTERED_METADATA, node, &format!("{node},{node}"));
        let report = check_filtered(&mutated);
        assert_refused(&report, Phase::Graph, "duplicate_resolve_node");
    }

    #[test]
    fn package_absent_from_resolve_is_refused() {
        let node = ",{\"id\":\"path+file:///Users/jemanuel/projects/franken_alignment/xtask#0.2.0\",\"dependencies\":[],\"deps\":[],\"features\":[]}";
        let mutated = mutate(FILTERED_METADATA, node, "");
        let report = check_filtered(&mutated);
        assert_refused(&report, Phase::Graph, "package_missing_from_resolve");
    }

    #[test]
    fn workspace_member_without_package_is_refused() {
        let mutated = mutate(
            FILTERED_METADATA,
            "\"workspace_members\":[",
            "\"workspace_members\":[\"path+file:///ghost#1.0.0\",",
        );
        let report = check_filtered(&mutated);
        assert_refused(&report, Phase::Graph, "workspace_member_without_package");
    }

    #[test]
    fn empty_target_kind_is_refused() {
        let mutated = mutate(FILTERED_METADATA, "\"kind\":[\"bin\"]", "\"kind\":[]");
        let report = check_filtered(&mutated);
        assert_refused(&report, Phase::Build, "empty_target_kind");
    }

    #[test]
    fn non_string_source_is_refused() {
        let mutated = mutate(
            FILTERED_METADATA,
            "\"description\":null,\"source\":null",
            "\"description\":null,\"source\":[]",
        );
        let report = check_filtered(&mutated);
        assert_refused(&report, Phase::Source, "expected_null_or_string");
    }

    #[test]
    fn absent_links_field_is_refused() {
        let mutated = mutate(FILTERED_METADATA, ",\"links\":null", "");
        let report = check_filtered(&mutated);
        assert_refused(&report, Phase::Build, "missing_field");
    }

    // ---- dependency edges, causal by kind and target ----

    #[test]
    fn feature_only_injection_is_refused_by_features_phase() {
        let mutated = mutate(
            FILTERED_METADATA,
            "\"dependencies\":[],\"deps\":[],\"features\":[]}",
            "\"dependencies\":[],\"deps\":[],\"features\":[\"native-zstd\"]}",
        );
        let report = check_filtered(&mutated);
        assert_refused(&report, Phase::Features, "feature_set_mismatch");
    }

    #[test]
    fn target_scoped_dependency_is_refused_by_target_phase() {
        let mutated = mutate(
            FILTERED_METADATA,
            "\"dependencies\":[],\"deps\":[],\"features\":[]}",
            "\"dependencies\":[],\"deps\":[{\"name\":\"libc\",\"pkg\":\"registry+x#libc@0.2.0\",\
             \"dep_kinds\":[{\"kind\":null,\"target\":\"cfg(unix)\"}]}],\"features\":[]}",
        );
        let report = check_filtered(&mutated);
        assert_refused(&report, Phase::Target, "unadmitted_dependency_edge");
    }

    #[test]
    fn build_dependency_edge_is_refused_by_build_phase() {
        let mutated = mutate(
            FILTERED_METADATA,
            "\"dependencies\":[],\"deps\":[],\"features\":[]}",
            "\"dependencies\":[],\"deps\":[{\"name\":\"cc\",\"pkg\":\"registry+x#cc@1.0.0\",\
             \"dep_kinds\":[{\"kind\":\"build\",\"target\":null}]}],\"features\":[]}",
        );
        let report = check_filtered(&mutated);
        assert_refused(&report, Phase::Build, "unadmitted_dependency_edge");
    }

    /// The unmutated `deps: []` slot in the first resolve node.
    const DEPS_ANCHOR: &str = "\"dependencies\":[],\"deps\":[],\"features\":[]}";

    fn edge_finding(report: &Report) -> Finding {
        report
            .findings
            .iter()
            .find(|finding| finding.code == "unadmitted_dependency_edge")
            .cloned()
            .expect("an unadmitted dependency edge must be reported")
    }

    #[test]
    fn dependency_edge_classification_is_order_independent() {
        // Same two dep_kinds entries, emitted in both orders. Before the fix
        // the last entry won, so reversing them changed both the phase and the
        // detail: the diagnostic described Cargo's emission order rather than
        // the dependency.
        let forward = "\"dependencies\":[],\"deps\":[{\"name\":\"libc\",\
                       \"pkg\":\"registry+x#libc@0.2.0\",\"dep_kinds\":[\
                       {\"kind\":\"build\",\"target\":null},\
                       {\"kind\":null,\"target\":\"cfg(unix)\"}]}],\"features\":[]}";
        let reversed = "\"dependencies\":[],\"deps\":[{\"name\":\"libc\",\
                        \"pkg\":\"registry+x#libc@0.2.0\",\"dep_kinds\":[\
                        {\"kind\":null,\"target\":\"cfg(unix)\"},\
                        {\"kind\":\"build\",\"target\":null}]}],\"features\":[]}";

        let first = edge_finding(&check_filtered(&mutate(
            FILTERED_METADATA,
            DEPS_ANCHOR,
            forward,
        )));
        let second = edge_finding(&check_filtered(&mutate(
            FILTERED_METADATA,
            DEPS_ANCHOR,
            reversed,
        )));

        assert_eq!(first.phase, second.phase, "phase must not depend on order");
        assert_eq!(first.code, second.code, "code must not depend on order");
        assert_eq!(
            first.detail, second.detail,
            "detail must not depend on order"
        );

        // A declared target dominates the whole set, whichever entry is last.
        assert_eq!(first.phase, Phase::Target);
        // Neither declared kind is lost from the evidence.
        assert!(
            first.detail.contains("build") && first.detail.contains(NORMAL_DEP_KIND),
            "both declared kinds must survive: {}",
            first.detail
        );
        assert!(
            first.detail.contains("cfg(unix)"),
            "the declared target must survive: {}",
            first.detail
        );
    }

    #[test]
    fn normal_dependency_edge_is_classified_as_features() {
        // A single ordinary edge: no target, and a null kind, which Cargo uses
        // for a normal dependency. It is still an unconditional refusal.
        let normal = "\"dependencies\":[],\"deps\":[{\"name\":\"serde\",\
                      \"pkg\":\"registry+x#serde@1.0.0\",\"dep_kinds\":[\
                      {\"kind\":null,\"target\":null}]}],\"features\":[]}";
        let report = check_filtered(&mutate(FILTERED_METADATA, DEPS_ANCHOR, normal));
        assert_refused(&report, Phase::Features, "unadmitted_dependency_edge");

        let finding = edge_finding(&report);
        assert_eq!(finding.phase, Phase::Features);
        assert!(
            finding.detail.contains(NORMAL_DEP_KIND),
            "a null kind must be recorded as `{NORMAL_DEP_KIND}`, not dropped: {}",
            finding.detail
        );
        assert!(
            finding.detail.contains("targets [none]"),
            "an edge with no target must say so explicitly: {}",
            finding.detail
        );
    }

    // ---- evaluator compiler tests -------------------------------------------
    //
    // These exercise the production algorithm over hypothetical reviewed rows.
    // They are compiler tests: nothing here admits a package, no row reaches
    // `registry/`, and none of this is donor or runtime evidence.

    const REGISTRY: &str = "registry+https://github.com/rust-lang/crates.io-index";
    const OWNED_GIT: &str = "git+https://github.com/Dicklesworthstone/frankentorch?rev=abc#abc";

    fn pkg(source: &str, name: &str, version: &str) -> PackageId {
        PackageId {
            source: SourceId::Exact(source.to_string()),
            name: name.to_string(),
            version: version.to_string(),
        }
    }

    fn row(id: PackageId, scopes: Vec<(&str, TargetScope)>) -> ReviewedRow {
        ReviewedRow {
            id,
            scopes: scopes
                .into_iter()
                .map(|(target, scope)| (target.to_string(), scope))
                .collect(),
        }
    }

    fn observed(id: PackageId, edges: Vec<EdgeId>) -> ObservedPackage {
        ObservedPackage {
            id,
            features: BTreeSet::new(),
            edges: edges.into_iter().collect(),
        }
    }

    /// The real captured graph with one feature and one build edge injected.
    fn injected_metadata() -> String {
        mutate(
            FILTERED_METADATA,
            "\"dependencies\":[],\"deps\":[],\"features\":[]}",
            "\"dependencies\":[],\"deps\":[{\"name\":\"xtask\",\
             \"pkg\":\"path+file:///Users/jemanuel/projects/franken_alignment/xtask#0.2.0\",\
             \"dep_kinds\":[{\"kind\":\"build\",\"target\":null}]}],\
             \"features\":[\"native-zstd\"]}",
        )
    }

    #[test]
    fn projection_carries_real_features_and_edges_from_cargo_shaped_input() {
        // This is the regression that a hard-coded empty observation fails.
        // Both the feature and the edge are injected into the real captured
        // metadata, and the production projection must carry both.
        let document = parse(injected_metadata().as_bytes(), Limits::default()).expect("parses");
        let mut findings = Vec::new();
        let observed = project_observed_packages(&document, &mut findings);

        let fa = observed
            .iter()
            .find(|package| package.id.name == "fa-reference")
            .expect("fa-reference must be observed");
        assert!(
            fa.features.contains("native-zstd"),
            "projection must carry the real activated features, got {:?}",
            fa.features
        );
        assert_eq!(
            fa.edges.len(),
            1,
            "projection must carry the real dependency edges, got {:?}",
            fa.edges
        );
        let edge = fa.edges.iter().next().expect("one edge");
        assert_eq!(edge.kind, DepKind::Build, "the real dep kind is preserved");
        assert_eq!(edge.to.name, "xtask");
        assert_eq!(edge.to.version, "0.2.0");
        assert!(
            matches!(edge.to.source, SourceId::WorkspacePath { .. }),
            "the edge names its exact destination package, not a bare name"
        );
    }

    #[test]
    fn injected_edge_and_feature_reach_the_live_evaluator() {
        // The same injected graph, through the whole gate: the evaluator, not
        // a bespoke branch, is what refuses them.
        let report = check_filtered(&injected_metadata());
        assert_refused(&report, Phase::Target, "edge_outside_target_scope");
        assert_refused(&report, Phase::Features, "feature_set_mismatch");
    }

    fn project(metadata: &str) -> (Vec<ObservedPackage>, Vec<Finding>) {
        let document = parse(metadata.as_bytes(), Limits::default()).expect("parses");
        let mut findings = Vec::new();
        let observed = project_observed_packages(&document, &mut findings);
        (observed, findings)
    }

    fn observes_fa_reference(observed: &[ObservedPackage]) -> bool {
        observed.iter().any(|p| p.id.name == "fa-reference")
    }

    #[test]
    fn the_projector_never_returns_an_unexplained_empty_result() {
        // A direct caller of the public projector must be able to tell
        // "nothing to observe" from "the document could not be read". `check`
        // diagnoses these separately, but the public API stands on its own.
        for document in [
            "[]",                  // root is not an object
            "{\"version\":1}",     // no `packages` at all
            "{\"packages\":{}}",   // `packages` is not an array
            "{\"packages\":[]}",   // required inventory cannot be empty
            "{\"packages\":[{}]}", // unreadable identity cannot silently disappear
            "{\"packages\":[{\"id\":\"x\",\"name\":\"x\",\"version\":\"1\",\"source\":42}]}",
            "{\"packages\":[{\"id\":\"x\",\"name\":\"x\",\"version\":\"1\"}]}",
        ] {
            let parsed = parse(document.as_bytes(), Limits::default()).expect("parses");
            let mut findings = Vec::new();
            let observed = project_observed_packages(&parsed, &mut findings);
            assert!(observed.is_empty(), "{document}");
            assert!(
                !findings.is_empty(),
                "an empty projection must carry its reason: {document}"
            );
        }
    }

    #[test]
    fn a_malformed_feature_entry_is_refused_not_silently_dropped() {
        // Filtering this out would silently shrink the activated set, which is
        // exactly how an injected feature could pass unnoticed.
        let mutated = mutate(FILTERED_METADATA, "\"features\":[]}", "\"features\":[42]}");
        let (observed, findings) = project(&mutated);
        assert!(
            findings
                .iter()
                .any(|f| f.phase == Phase::Features && f.code == "expected_string"),
            "a non-string feature must be reported: {findings:?}"
        );
        assert!(
            !observes_fa_reference(&observed),
            "a package whose feature set could not be read exactly must not be observed"
        );
    }

    #[test]
    fn an_absent_features_array_is_unknown_not_empty() {
        let mutated = mutate(
            FILTERED_METADATA,
            "\"dependencies\":[],\"deps\":[],\"features\":[]}",
            "\"dependencies\":[],\"deps\":[]}",
        );
        let (observed, findings) = project(&mutated);
        assert!(
            findings
                .iter()
                .any(|f| f.phase == Phase::Features && f.code == "missing_field"),
            "an absent features array must be reported: {findings:?}"
        );
        assert!(!observes_fa_reference(&observed));
    }

    #[test]
    fn an_absent_deps_array_is_unknown_not_no_edges() {
        let mutated = mutate(
            FILTERED_METADATA,
            "\"dependencies\":[],\"deps\":[],\"features\":[]}",
            "\"dependencies\":[],\"features\":[]}",
        );
        let (observed, findings) = project(&mutated);
        assert!(
            findings
                .iter()
                .any(|f| f.phase == Phase::Graph && f.code == "missing_field"),
            "an absent deps array must be reported: {findings:?}"
        );
        assert!(
            !observes_fa_reference(&observed),
            "unknown edges must never be observed as no edges"
        );
    }

    #[test]
    fn a_hand_built_policy_with_an_absent_scope_refuses() {
        // `parse_policy` guarantees a profile entry for every admitted row, but
        // a `Policy` assembled in code must not be able to default an absent
        // scope to "no features".
        let mut incomplete_policy = policy();
        incomplete_policy.profiles[0]
            .expected_features
            .remove("fa-reference");
        let error = check(FILTERED_METADATA.as_bytes(), &incomplete_policy, TARGET)
            .expect_err("an absent reviewed scope must refuse");
        assert!(
            error.contains("freezes no feature set"),
            "the refusal must name the absent scope: {error}"
        );

        let mut orphan = policy();
        orphan.local[0]
            .targets
            .insert("riscv64gc-unknown-linux-gnu".to_string());
        let error = check(FILTERED_METADATA.as_bytes(), &orphan, TARGET)
            .expect_err("a target with no frozen profile must refuse");
        assert!(error.contains("freezes no target profile"), "{error}");
    }

    #[test]
    fn an_unresolvable_edge_destination_is_refused_not_observed_empty() {
        let mutated = mutate(
            FILTERED_METADATA,
            "\"dependencies\":[],\"deps\":[],\"features\":[]}",
            "\"dependencies\":[],\"deps\":[{\"name\":\"ghost\",\
             \"pkg\":\"path+file:///nowhere/ghost#9.9.9\",\
             \"dep_kinds\":[{\"kind\":null,\"target\":null}]}],\"features\":[]}",
        );
        let document = parse(mutated.as_bytes(), Limits::default()).expect("parses");
        let mut findings = Vec::new();
        let observed = project_observed_packages(&document, &mut findings);
        assert!(
            findings
                .iter()
                .any(|finding| finding.code == "edge_destination_unresolved"),
            "an edge naming no known package must be refused: {findings:?}"
        );
        assert!(
            !observed.iter().any(|p| p.id.name == "fa-reference"),
            "a package whose edges could not be resolved must not be observed at all"
        );
    }

    #[test]
    fn source_alone_is_not_an_identity() {
        // Every crates.io package shares one index URL, so two distinct
        // packages under it must coexist in one set.
        let set = AdmissionSet::new(vec![
            row(
                pkg(REGISTRY, "alpha", "1.0.0"),
                vec![("A", TargetScope::default())],
            ),
            row(
                pkg(REGISTRY, "beta", "1.0.0"),
                vec![("A", TargetScope::default())],
            ),
            row(
                pkg(REGISTRY, "alpha", "2.0.0"),
                vec![("A", TargetScope::default())],
            ),
        ])
        .expect("distinct identities under one source must coexist");
        for (name, version) in [("alpha", "1.0.0"), ("beta", "1.0.0"), ("alpha", "2.0.0")] {
            assert_eq!(
                evaluate(&set, "A", &observed(pkg(REGISTRY, name, version), vec![])),
                vec![]
            );
        }
    }

    #[test]
    fn duplicate_full_identity_is_refused_not_overwritten() {
        let error = AdmissionSet::new(vec![
            row(
                pkg(REGISTRY, "alpha", "1.0.0"),
                vec![("A", TargetScope::default())],
            ),
            row(
                pkg(REGISTRY, "alpha", "1.0.0"),
                vec![("B", TargetScope::default())],
            ),
        ])
        .expect_err("a duplicate identity must not silently replace a row");
        assert_eq!(
            error,
            AdmissionSetError::DuplicateIdentity(pkg(REGISTRY, "alpha", "1.0.0"))
        );
    }

    #[test]
    fn registry_impostor_is_refused_with_name_and_version_held_equal() {
        // Only `source` differs. Name and version are identical, so the
        // refusal is causal on origin and cannot be a name check.
        let reviewed_id = pkg(OWNED_GIT, "frankentorch-api", "0.1.0");
        let impostor = pkg(REGISTRY, "frankentorch-api", "0.1.0");
        assert_eq!(reviewed_id.name, impostor.name);
        assert_eq!(reviewed_id.version, impostor.version);

        let set = AdmissionSet::new(vec![row(
            reviewed_id.clone(),
            vec![("A", TargetScope::default())],
        )])
        .expect("set");
        assert_eq!(evaluate(&set, "A", &observed(reviewed_id, vec![])), vec![]);
        assert_eq!(
            evaluate(&set, "A", &observed(impostor.clone(), vec![])),
            vec![Violation::NoReviewedRow { id: impostor }]
        );
    }

    #[test]
    fn identical_edge_discriminates_between_admitted_and_unadmitted_target() {
        let host = pkg(OWNED_GIT, "host", "1.0.0");
        let dest = pkg(OWNED_GIT, "dest", "1.0.0");
        let edge = EdgeId {
            to: dest.clone(),
            kind: DepKind::Normal,
        };
        let scope_with = TargetScope {
            features: BTreeSet::new(),
            edges: [edge.clone()].into_iter().collect(),
        };
        // The destination is reviewed on both targets, so the only difference
        // between the two evaluations is whether the edge is in scope.
        let set = AdmissionSet::new(vec![
            row(
                host.clone(),
                vec![("A", scope_with), ("B", TargetScope::default())],
            ),
            row(
                dest,
                vec![("A", TargetScope::default()), ("B", TargetScope::default())],
            ),
        ])
        .expect("set");

        let seen = observed(host, vec![edge.clone()]);
        assert_eq!(evaluate(&set, "A", &seen), vec![], "admitted on A");
        assert_eq!(
            evaluate(&set, "B", &seen),
            vec![Violation::EdgeNotAdmitted {
                target: "B".to_string(),
                edge
            }],
            "the identical edge is refused on B"
        );
    }

    #[test]
    fn an_admitted_edge_into_an_unreviewed_destination_does_not_pass() {
        let host = pkg(OWNED_GIT, "host", "1.0.0");
        let dangling = pkg(REGISTRY, "ghost", "9.9.9");
        let edge = EdgeId {
            to: dangling,
            kind: DepKind::Build,
        };
        let set = AdmissionSet::new(vec![row(
            host.clone(),
            vec![(
                "A",
                TargetScope {
                    features: BTreeSet::new(),
                    edges: [edge.clone()].into_iter().collect(),
                },
            )],
        )])
        .expect("set");
        assert_eq!(
            evaluate(&set, "A", &observed(host, vec![edge.clone()])),
            vec![Violation::EdgeDestinationNotReviewed {
                target: "A".to_string(),
                edge
            }]
        );
    }

    #[test]
    fn a_target_absent_from_the_row_is_not_admitted() {
        let id = pkg(OWNED_GIT, "host", "1.0.0");
        let set = AdmissionSet::new(vec![row(id.clone(), vec![("A", TargetScope::default())])])
            .expect("set");
        assert_eq!(evaluate(&set, "A", &observed(id.clone(), vec![])), vec![]);
        assert_eq!(
            evaluate(&set, "B", &observed(id.clone(), vec![])),
            vec![Violation::TargetNotAdmitted {
                id,
                admitted: vec!["A".to_string()]
            }],
            "absence is never scope"
        );
    }

    #[test]
    fn missing_kind_key_is_a_marker_not_normal() {
        // An absent `kind` is not Cargo's null-means-normal spelling; it is a
        // document that did not say. It must not be laundered into `normal`.
        let absent = "\"dependencies\":[],\"deps\":[{\"name\":\"ghost\",\
                      \"pkg\":\"registry+x#ghost@0.1.0\",\"dep_kinds\":[\
                      {\"target\":null}]}],\"features\":[]}";
        let report = check_filtered(&mutate(FILTERED_METADATA, DEPS_ANCHOR, absent));
        let finding = edge_finding(&report);
        assert_eq!(
            finding.phase,
            Phase::Metadata,
            "an unreadable kind classifies as metadata, never as a normal edge"
        );
        assert!(
            finding.detail.contains("<kind:missing>"),
            "the marker must survive: {}",
            finding.detail
        );
        assert!(
            !finding.detail.contains("kinds [normal]"),
            "a missing kind must not be reported as normal: {}",
            finding.detail
        );
        assert!(!report.is_admitted());
    }

    #[test]
    fn malformed_kind_value_is_a_marker_not_normal() {
        let malformed = "\"dependencies\":[],\"deps\":[{\"name\":\"ghost\",\
                         \"pkg\":\"registry+x#ghost@0.1.0\",\"dep_kinds\":[\
                         {\"kind\":42,\"target\":null}]}],\"features\":[]}";
        let report = check_filtered(&mutate(FILTERED_METADATA, DEPS_ANCHOR, malformed));
        let finding = edge_finding(&report);
        assert_eq!(finding.phase, Phase::Metadata);
        assert!(
            finding.detail.contains("<kind:malformed:number>"),
            "the marker must name the offending type: {}",
            finding.detail
        );
        assert!(!report.is_admitted());
    }

    #[test]
    fn a_real_target_dominates_an_anomaly() {
        // Precedence step 1 beats step 3: one entry is unreadable, another
        // declares a genuine target. The real fact decides the phase, and the
        // anomaly is still reported rather than dropped.
        let mixed = "\"dependencies\":[],\"deps\":[{\"name\":\"ghost\",\
                     \"pkg\":\"registry+x#ghost@0.1.0\",\"dep_kinds\":[\
                     {\"target\":null},\
                     {\"kind\":null,\"target\":\"cfg(unix)\"}]}],\"features\":[]}";
        let report = check_filtered(&mutate(FILTERED_METADATA, DEPS_ANCHOR, mixed));
        let finding = edge_finding(&report);
        assert_eq!(finding.phase, Phase::Target);
        assert!(finding.detail.contains("cfg(unix)"));
        assert!(
            finding.detail.contains("<kind:missing>"),
            "the anomaly must still be reported: {}",
            finding.detail
        );
    }

    #[test]
    fn manifest_declared_dependency_is_refused() {
        let mutated = mutate(
            FILTERED_METADATA,
            "\"source\":null,\"dependencies\":[]",
            "\"source\":null,\"dependencies\":[{\"name\":\"serde\",\"kind\":null,\"target\":null}]",
        );
        let report = check_filtered(&mutated);
        assert_refused(&report, Phase::Build, "unadmitted_manifest_dependency");
    }

    // ---- inventory ----

    #[test]
    fn unlisted_local_package_is_refused() {
        let mutated = mutate(
            FILTERED_METADATA,
            "\"name\":\"xtask\",\"version\":\"0.2.0\"",
            "\"name\":\"serde\",\"version\":\"1.0.0\"",
        );
        let report = check_filtered(&mutated);
        assert_refused(&report, Phase::Inventory, "local_package_not_admitted");
    }

    #[test]
    fn admitted_row_with_no_package_is_refused() {
        let mutated = mutate(
            FILTERED_METADATA,
            "\"name\":\"xtask\",\"version\":\"0.2.0\"",
            "\"name\":\"xtask-renamed\",\"version\":\"0.2.0\"",
        );
        let report = check_filtered(&mutated);
        assert_refused(&report, Phase::Inventory, "admitted_row_absent");
    }

    #[test]
    fn name_identity_confusion_is_refused_on_source_not_name() {
        // `ft-api` must not silently resolve to an unrelated registry package
        // when the intended owned package is `frankentorch-api`. The refusal is
        // keyed on the source, never on a directory name.
        let mutated = mutate(
            FILTERED_METADATA,
            "\"name\":\"fa-reference\",\"version\":\"0.2.0\",\"id\":\"path+file://",
            "\"name\":\"ft-api\",\"version\":\"0.2.0\",\"id\":\"path+file://",
        );
        let mutated = mutate(
            &mutated,
            "\"license_file\":\"../../LICENSE\",\"description\":null,\"source\":null",
            "\"license_file\":\"../../LICENSE\",\"description\":null,\"source\":\"registry+https://github.com/rust-lang/crates.io-index\"",
        );
        let report = check_filtered(&mutated);
        assert_refused(&report, Phase::Source, "unsupported_external_admission");
    }

    #[test]
    fn metadata_version_mismatch_is_refused() {
        let mutated = mutate(
            FILTERED_METADATA,
            "\"version\":1,\"workspace_root\"",
            "\"version\":2,\"workspace_root\"",
        );
        let report = check_filtered(&mutated);
        assert_refused(&report, Phase::Metadata, "metadata_version_mismatch");
    }

    // ---- policy loading is never skippable ----

    #[test]
    fn policy_without_admission_block_is_a_hard_error() {
        let error =
            parse_policy(br#"{"schema_version":"fa.dependencies/0.2"}"#).expect_err("must fail");
        assert!(error.contains("admission"), "{error}");
    }

    #[test]
    fn policy_with_wrong_schema_is_a_hard_error() {
        let relaxed = POLICY.replace("fa.admission/0.1", "fa.admission/9.9");
        let error = parse_policy(relaxed.as_bytes()).expect_err("must fail");
        assert!(error.contains("schema"), "{error}");
    }

    #[test]
    fn policy_declaring_an_unsupported_metadata_version_is_a_hard_error() {
        // Control: version 1 is the supported case and parses (see `policy()`).
        assert_eq!(policy().rules.metadata_version, SUPPORTED_METADATA_VERSION);
        for unsupported in ["0", "2"] {
            let staged = POLICY.replace(
                "\"metadata_version\": 1",
                &format!("\"metadata_version\": {unsupported}"),
            );
            let error = parse_policy(staged.as_bytes())
                .expect_err(&format!("metadata_version {unsupported} is unsupported"));
            assert!(
                error.contains("metadata_version"),
                "version {unsupported}: {error}"
            );
        }
    }

    #[test]
    fn policy_without_target_profiles_is_a_hard_error() {
        let stripped = br#"{"admission":{"schema":"fa.admission/0.1","local_packages":[
            {"name":"xtask","version":"0.2.0","manifest_path":"xtask/Cargo.toml",
             "owner":"systems","decision":"admitted"}],
            "external_rows":[],
            "rules":{"metadata_version":1,"require_resolve_section":true,
            "allow_build_scripts":false,"allow_proc_macro":false,"allow_links":false}}}"#;
        let error = parse_policy(stripped).expect_err("must fail");
        assert!(error.contains("target_profiles"), "{error}");
    }

    #[test]
    fn profile_silent_on_an_admitted_package_is_a_hard_error() {
        let stripped = br#"{"admission":{"schema":"fa.admission/0.1","local_packages":[
            {"name":"xtask","version":"0.2.0","manifest_path":"xtask/Cargo.toml",
             "owner":"systems","decision":"admitted"}],
            "external_rows":[],
            "target_profiles":[{"target":"t","expected_features":{}}],
            "rules":{"metadata_version":1,"require_resolve_section":true,
            "allow_build_scripts":false,"allow_proc_macro":false,"allow_links":false}}}"#;
        let error = parse_policy(stripped).expect_err("must fail");
        assert!(error.contains("profile_silent_on_package"), "{error}");
    }

    #[test]
    fn policy_with_empty_local_packages_is_a_hard_error() {
        let stripped = br#"{"admission":{"schema":"fa.admission/0.1","local_packages":[],
            "external_rows":[],
            "target_profiles":[{"target":"t","expected_features":{}}],
            "rules":{"metadata_version":1,"require_resolve_section":true,
            "allow_build_scripts":false,"allow_proc_macro":false,"allow_links":false}}}"#;
        let error = parse_policy(stripped).expect_err("must fail");
        assert!(error.contains("empty"), "{error}");
    }

    // ---- owner and decision: an unowned or undecided row is not an admission

    #[test]
    fn row_without_an_owner_is_a_hard_error() {
        let stripped = POLICY.replace("\"owner\": \"systems\",", "");
        let error = parse_policy(stripped.as_bytes()).expect_err("must fail");
        assert!(error.contains("owner"), "{error}");
    }

    #[test]
    fn row_without_a_decision_is_a_hard_error() {
        let stripped = POLICY.replace("\"decision\": \"admitted\",", "");
        let error = parse_policy(stripped.as_bytes()).expect_err("must fail");
        assert!(error.contains("decision"), "{error}");
    }

    #[test]
    fn row_with_an_empty_owner_is_a_hard_error() {
        let blanked = POLICY.replace("\"owner\": \"systems\"", "\"owner\": \"   \"");
        let error = parse_policy(blanked.as_bytes()).expect_err("must fail");
        assert!(error.contains("row_owner_empty"), "{error}");
    }

    #[test]
    fn a_pending_decision_does_not_admit() {
        // The case this closes: a row pre-staged in the registry with a
        // provisional decision must refuse, not wait to take effect.
        for value in ["pending", "provisional", "Admitted", "approved", ""] {
            let staged = POLICY.replace(
                "\"decision\": \"admitted\"",
                &format!("\"decision\": \"{value}\""),
            );
            let error = parse_policy(staged.as_bytes())
                .expect_err(&format!("decision `{value}` must not admit"));
            assert!(
                error.contains("row_not_admitted"),
                "decision `{value}`: {error}"
            );
        }
    }
}
