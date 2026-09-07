//! Strict contract for the closed two-package FA-053 admission inventory.
//!
//! This is deliberately not an external-admission path. It binds the two
//! reviewed local rows to the policy's source-snapshot descriptor and refuses
//! any permissive external-admission state.

use std::collections::{BTreeMap, BTreeSet};

use crate::json::Json;

const SNAPSHOT_ID: &str = "initial-reviewed-workspace";
const SNAPSHOT_SCHEMA: &str = "fa.source_snapshot/1";
const SNAPSHOT_PATH: &str = "registry/source_snapshot.json";
const SOURCE_URL: &str = "https://github.com/Dicklesworthstone/franken_alignment.git";
const IDENTITY_SCOPE: &str = "reviewed_snapshot_bytes";
const CONSTITUTION: &str = "docs/DEPENDENCY_CONSTITUTION.md";
const TARGETS: [&str; 2] = ["aarch64-apple-darwin", "x86_64-unknown-linux-gnu"];

const LOCAL_ROWS: [(&str, &str); 2] = [("fa-reference", "0.2.0"), ("xtask", "0.2.0")];
const BUILD_FIELDS: [&str; 5] = [
    "dependencies",
    "build_dependencies",
    "build_scripts",
    "proc_macros",
    "native_links",
];
const IDENTITY_FIELDS: [&str; 5] = ["name", "version", "source", "revision", "checksum"];
const SCOPE_FIELDS: [&str; 3] = ["target", "features", "default_features"];
const CLOSURE_FIELDS: [&str; 6] = [
    "build_dependency_closure",
    "proc_macro_closure",
    "runtime_closure",
    "native_link_declarations",
    "build_script_effects",
    "downloaded_artifacts",
];
const GOVERNANCE_FIELDS: [&str; 5] = [
    "direct_reason",
    "owner",
    "decision",
    "decision_date",
    "evidence",
];

/// The portion of a checked source-snapshot identity a caller may consume.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotBinding {
    pub manifest_path: String,
    pub sha256: String,
}

/// Validate the fixed initial local-admission contract.
///
/// The returned binding identifies an operator-reviewed manifest. It does not
/// authenticate the snapshot file or approve any external package; the caller
/// must separately verify the manifest bytes against the workspace.
pub fn check(document: &Json) -> Result<SnapshotBinding, String> {
    let root = object(document, "$")?;
    exact_text(root, "schema_version", "fa.dependencies/0.3", "$")?;
    exact_empty_array(root, "external_exceptions", "$")?;
    exact_empty_array(root, "foundation_admissions", "$")?;
    exact_text(root, "transitive_feature_audits", "pending", "$")?;
    let admission = object(field(root, "admission", "$")?, "$.admission")?;
    let binding = snapshot_binding(admission)?;
    check_external(admission)?;

    let profiles = target_profiles(admission)?;
    let rows = array(
        field(admission, "local_packages", "$.admission")?,
        "$.admission.local_packages",
    )?;
    if rows.len() != LOCAL_ROWS.len() {
        return Err(format!(
            "$.admission.local_packages must contain exactly {} local rows, found {}",
            LOCAL_ROWS.len(),
            rows.len()
        ));
    }

    let mut seen = BTreeSet::new();
    for (index, row) in rows.iter().enumerate() {
        let path = format!("$.admission.local_packages[{index}]");
        let row = object(row, &path)?;
        check_local_row(row, &path, &profiles)?;
        let name = text(field(row, "name", &path)?, &format!("{path}.name"))?;
        let version = text(field(row, "version", &path)?, &format!("{path}.version"))?;
        if !LOCAL_ROWS.contains(&(name, version)) {
            return Err(format!(
                "{path} names unsupported local package `{name} {version}`"
            ));
        }
        if !seen.insert((name.to_string(), version.to_string())) {
            return Err(format!(
                "{path} duplicates local package `{name} {version}`"
            ));
        }
    }
    for (name, version) in LOCAL_ROWS {
        if !seen.contains(&(name.to_string(), version.to_string())) {
            return Err(format!(
                "$.admission.local_packages is missing required local package `{name} {version}`"
            ));
        }
    }
    Ok(binding)
}

fn snapshot_binding(admission: &BTreeMap<String, Json>) -> Result<SnapshotBinding, String> {
    let path = "$.admission.source_snapshot";
    let descriptor = object(field(admission, "source_snapshot", "$.admission")?, path)?;
    exact_keys(
        descriptor,
        path,
        &[
            "id",
            "schema",
            "manifest_path",
            "sha256",
            "source_url",
            "identity_scope",
        ],
    )?;
    exact_text(descriptor, "id", SNAPSHOT_ID, path)?;
    exact_text(descriptor, "schema", SNAPSHOT_SCHEMA, path)?;
    exact_text(descriptor, "manifest_path", SNAPSHOT_PATH, path)?;
    exact_text(descriptor, "source_url", SOURCE_URL, path)?;
    exact_text(descriptor, "identity_scope", IDENTITY_SCOPE, path)?;
    let sha256 = text(
        field(descriptor, "sha256", path)?,
        &format!("{path}.sha256"),
    )?;
    if !lower_sha256(sha256) {
        return Err(format!(
            "{path}.sha256 must be exactly 64 lowercase hexadecimal digits"
        ));
    }
    Ok(SnapshotBinding {
        manifest_path: SNAPSHOT_PATH.to_string(),
        sha256: sha256.to_string(),
    })
}

fn check_external(admission: &BTreeMap<String, Json>) -> Result<(), String> {
    let path = "$.admission.external_admission";
    let external = object(field(admission, "external_admission", "$.admission")?, path)?;
    exact_bool(external, "supported", false, path)?;
    exact_bool(external, "approved", false, path)?;
    exact_bool(external, "external_rows_must_remain_empty", true, path)?;

    let rows = array(
        field(admission, "external_rows", "$.admission")?,
        "$.admission.external_rows",
    )?;
    if !rows.is_empty() {
        return Err(format!(
            "$.admission.external_rows has {} row(s); external admission remains impossible",
            rows.len()
        ));
    }

    let required_path = format!("{path}.required_fields_when_implemented");
    let required = object(
        field(external, "required_fields_when_implemented", path)?,
        &required_path,
    )?;
    exact_string_set(required, "identity", &IDENTITY_FIELDS, &required_path)?;
    exact_string_set(required, "scope", &SCOPE_FIELDS, &required_path)?;
    exact_string_set(required, "closure", &CLOSURE_FIELDS, &required_path)?;
    exact_string_set(required, "governance", &GOVERNANCE_FIELDS, &required_path)?;
    Ok(())
}

fn target_profiles(admission: &BTreeMap<String, Json>) -> Result<BTreeSet<String>, String> {
    let path = "$.admission.target_profiles";
    let profiles = array(field(admission, "target_profiles", "$.admission")?, path)?;
    if profiles.is_empty() {
        return Err(format!("{path} must not be empty"));
    }
    let mut targets = BTreeSet::new();
    for (index, profile) in profiles.iter().enumerate() {
        let profile_path = format!("{path}[{index}]");
        let profile = object(profile, &profile_path)?;
        let target = nonblank(
            field(profile, "target", &profile_path)?,
            &format!("{profile_path}.target"),
        )?;
        if !targets.insert(target.to_string()) {
            return Err(format!("{profile_path}.target duplicates `{target}`"));
        }
        let features_path = format!("{profile_path}.expected_features");
        let features = object(
            field(profile, "expected_features", &profile_path)?,
            &features_path,
        )?;
        exact_keys(features, &features_path, &["fa-reference", "xtask"])?;
        for name in ["fa-reference", "xtask"] {
            string_set(features, name, &format!("{features_path}.{name}"))?;
        }
    }
    let expected: BTreeSet<String> = TARGETS.iter().map(|target| (*target).to_string()).collect();
    if targets != expected {
        return Err(format!(
            "{path} must contain exactly the qualified targets {:?}, found {:?}",
            expected, targets
        ));
    }
    Ok(targets)
}

fn check_local_row(
    row: &BTreeMap<String, Json>,
    path: &str,
    profile_targets: &BTreeSet<String>,
) -> Result<(), String> {
    exact_keys(
        row,
        path,
        &[
            "name",
            "version",
            "manifest_path",
            "direct_reason",
            "owner",
            "decision",
            "decision_date",
            "source_snapshot",
            "targets",
            "build_closure",
            "runtime_closure",
            "evidence",
        ],
    )?;
    for name in [
        "name",
        "version",
        "manifest_path",
        "direct_reason",
        "owner",
        "decision_date",
    ] {
        nonblank(field(row, name, path)?, &format!("{path}.{name}"))?;
    }
    exact_text(row, "decision", "admitted", path)?;
    exact_text(row, "source_snapshot", SNAPSHOT_ID, path)?;

    let targets_path = format!("{path}.targets");
    let targets = string_set(row, "targets", &targets_path)?;
    if &targets != profile_targets {
        return Err(format!(
            "{targets_path} must exactly match target_profiles; row has {:?}, profiles have {:?}",
            targets, profile_targets
        ));
    }

    let evidence_path = format!("{path}.evidence");
    let evidence = string_set(row, "evidence", &evidence_path)?;
    for required in [SNAPSHOT_PATH, CONSTITUTION] {
        if !evidence.contains(required) {
            return Err(format!("{evidence_path} is missing required `{required}`"));
        }
    }

    let build_path = format!("{path}.build_closure");
    let build = object(field(row, "build_closure", path)?, &build_path)?;
    exact_keys(build, &build_path, &BUILD_FIELDS)?;
    for name in BUILD_FIELDS {
        let values = array(
            field(build, name, &build_path)?,
            &format!("{build_path}.{name}"),
        )?;
        if !values.is_empty() {
            return Err(format!(
                "{build_path}.{name} must be an empty array in the closed inventory"
            ));
        }
    }

    let name = text(field(row, "name", path)?, &format!("{path}.name"))?;
    let runtime_path = format!("{path}.runtime_closure");
    let runtime = object(field(row, "runtime_closure", path)?, &runtime_path)?;
    check_runtime(runtime, &runtime_path, name)
}

fn check_runtime(
    runtime: &BTreeMap<String, Json>,
    path: &str,
    package: &str,
) -> Result<(), String> {
    exact_keys(
        runtime,
        path,
        &[
            "scope",
            "direct_network",
            "dynamic_loading",
            "downloaded_artifacts",
            "operator_tools",
            "filesystem",
            "test_filesystem",
            "trust_boundary",
        ],
    )?;
    exact_text(runtime, "scope", "reviewed_local_rust_source", path)?;
    exact_bool(runtime, "direct_network", false, path)?;
    exact_bool(runtime, "dynamic_loading", false, path)?;
    exact_string_set(runtime, "downloaded_artifacts", &[], path)?;

    match package {
        "fa-reference" => {
            exact_string_set(runtime, "operator_tools", &[], path)?;
            exact_text(runtime, "filesystem", "in_memory_only", path)?;
            exact_text(runtime, "test_filesystem", "in_memory_only", path)?;
            exact_string_set(runtime, "trust_boundary", &["std", "os", "toolchain"], path)?;
        }
        "xtask" => {
            exact_string_set(
                runtime,
                "operator_tools",
                &["cargo", "rustc", "sha256sum", "shasum"],
                path,
            )?;
            exact_text(
                runtime,
                "filesystem",
                "read_workspace_and_run_operator_tools",
                path,
            )?;
            exact_text(
                runtime,
                "test_filesystem",
                "owned_temporary_sandboxes",
                path,
            )?;
            exact_string_set(
                runtime,
                "trust_boundary",
                &["std", "os", "toolchain", "operator_tools"],
                path,
            )?;
        }
        _ => return Err(format!("{path} belongs to unsupported package `{package}`")),
    }
    Ok(())
}

fn field<'a>(
    object: &'a BTreeMap<String, Json>,
    name: &str,
    path: &str,
) -> Result<&'a Json, String> {
    object
        .get(name)
        .ok_or_else(|| format!("{path}.{name} is required"))
}

fn object<'a>(value: &'a Json, path: &str) -> Result<&'a BTreeMap<String, Json>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("{path} must be an object, found {}", value.kind()))
}

fn array<'a>(value: &'a Json, path: &str) -> Result<&'a [Json], String> {
    value
        .as_array()
        .ok_or_else(|| format!("{path} must be an array, found {}", value.kind()))
}

fn text<'a>(value: &'a Json, path: &str) -> Result<&'a str, String> {
    value
        .as_str()
        .ok_or_else(|| format!("{path} must be a string, found {}", value.kind()))
}

fn nonblank<'a>(value: &'a Json, path: &str) -> Result<&'a str, String> {
    let value = text(value, path)?;
    if value.trim().is_empty() {
        return Err(format!("{path} must not be blank"));
    }
    Ok(value)
}

fn exact_text(
    object: &BTreeMap<String, Json>,
    name: &str,
    expected: &str,
    path: &str,
) -> Result<(), String> {
    let value = text(field(object, name, path)?, &format!("{path}.{name}"))?;
    if value != expected {
        return Err(format!(
            "{path}.{name} must be `{expected}`, found `{value}`"
        ));
    }
    Ok(())
}

fn exact_bool(
    object: &BTreeMap<String, Json>,
    name: &str,
    expected: bool,
    path: &str,
) -> Result<(), String> {
    let value = field(object, name, path)?;
    match value.as_bool() {
        Some(value) if value == expected => Ok(()),
        Some(value) => Err(format!("{path}.{name} must be {expected}, found {value}")),
        None => Err(format!(
            "{path}.{name} must be a boolean, found {}",
            value.kind()
        )),
    }
}

fn exact_keys(
    object: &BTreeMap<String, Json>,
    path: &str,
    expected: &[&str],
) -> Result<(), String> {
    let actual: BTreeSet<&str> = object.keys().map(String::as_str).collect();
    let expected: BTreeSet<&str> = expected.iter().copied().collect();
    if actual != expected {
        let missing: Vec<&str> = expected.difference(&actual).copied().collect();
        let extra: Vec<&str> = actual.difference(&expected).copied().collect();
        return Err(format!(
            "{path} has missing fields [{}] and unsupported fields [{}]",
            missing.join(", "),
            extra.join(", ")
        ));
    }
    Ok(())
}

fn exact_empty_array(
    object: &BTreeMap<String, Json>,
    name: &str,
    path: &str,
) -> Result<(), String> {
    let values = array(field(object, name, path)?, &format!("{path}.{name}"))?;
    if !values.is_empty() {
        return Err(format!(
            "{path}.{name} must be empty in the closed inventory"
        ));
    }
    Ok(())
}

fn string_set(
    object: &BTreeMap<String, Json>,
    name: &str,
    path: &str,
) -> Result<BTreeSet<String>, String> {
    let values = array(field(object, name, path)?, path)?;
    let mut result = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        let value_path = format!("{path}[{index}]");
        let value = nonblank(value, &value_path)?;
        if !result.insert(value.to_string()) {
            return Err(format!("{value_path} duplicates `{value}`"));
        }
    }
    Ok(result)
}

fn exact_string_set(
    object: &BTreeMap<String, Json>,
    name: &str,
    expected: &[&str],
    path: &str,
) -> Result<(), String> {
    let actual = string_set(object, name, &format!("{path}.{name}"))?;
    let expected: BTreeSet<String> = expected.iter().map(|value| (*value).to_string()).collect();
    if actual != expected {
        return Err(format!(
            "{path}.{name} does not contain the exact required values"
        ));
    }
    Ok(())
}

fn lower_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;

    use crate::json::{Json, Limits, parse};

    use super::{SNAPSHOT_PATH, check};

    fn policy() -> Json {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let root = manifest_dir
            .parent()
            .expect("xtask has a workspace parent")
            .to_path_buf();
        let bytes = fs::read(root.join("registry/dependency_policy.json"))
            .expect("read current operator policy at test runtime");
        parse(&bytes, Limits::default()).expect("current policy parses")
    }

    fn object_mut(value: &mut Json) -> &mut BTreeMap<String, Json> {
        match value {
            Json::Object(object) => object,
            _ => panic!("fixture path must be an object"),
        }
    }

    fn admission_mut(policy: &mut Json) -> &mut BTreeMap<String, Json> {
        let root = object_mut(policy);
        object_mut(root.get_mut("admission").expect("admission exists"))
    }

    fn row_mut(policy: &mut Json, index: usize) -> &mut BTreeMap<String, Json> {
        let rows = admission_mut(policy)
            .get_mut("local_packages")
            .expect("rows exist");
        match rows {
            Json::Array(rows) => object_mut(&mut rows[index]),
            _ => panic!("rows must be an array"),
        }
    }

    fn assert_refusal(policy: &Json, cause: &str) {
        let error = check(policy).expect_err("one planted contract defect must refuse");
        assert!(error.contains(cause), "expected {cause:?} in {error:?}");
    }

    #[test]
    fn current_runtime_policy_binds_the_reviewed_snapshot() {
        let binding = check(&policy()).expect("current policy must satisfy the contract");
        assert_eq!(binding.manifest_path, SNAPSHOT_PATH);
        assert_eq!(binding.sha256.len(), 64);
    }

    #[test]
    fn top_level_schema_version_missing_or_unknown_refuse() {
        let mut missing = policy();
        object_mut(&mut missing).remove("schema_version");
        assert_refusal(&missing, "schema_version");

        let mut unknown = policy();
        object_mut(&mut unknown).insert(
            "schema_version".to_string(),
            Json::String("fa.dependencies/9.9".to_string()),
        );
        assert_refusal(&unknown, "schema_version");
    }

    #[test]
    fn missing_blank_and_wrong_typed_local_fields_refuse() {
        let mut missing = policy();
        row_mut(&mut missing, 0).remove("direct_reason");
        assert_refusal(&missing, "direct_reason");

        let mut blank = policy();
        row_mut(&mut blank, 0).insert("owner".to_string(), Json::String("  ".to_string()));
        assert_refusal(&blank, "owner");

        let mut wrong_type = policy();
        row_mut(&mut wrong_type, 0).insert("targets".to_string(), Json::Bool(false));
        assert_refusal(&wrong_type, "targets");
    }

    #[test]
    fn changed_snapshot_ref_and_missing_target_refuse() {
        let mut changed_ref = policy();
        row_mut(&mut changed_ref, 0).insert(
            "source_snapshot".to_string(),
            Json::String("another-snapshot".to_string()),
        );
        assert_refusal(&changed_ref, "source_snapshot");

        let mut missing_target = policy();
        let targets = row_mut(&mut missing_target, 0)
            .get_mut("targets")
            .expect("targets exist");
        match targets {
            Json::Array(targets) => {
                targets.pop();
            }
            _ => panic!("targets must be an array"),
        }
        assert_refusal(&missing_target, "targets");
    }

    #[test]
    fn source_descriptor_url_and_shape_are_causal() {
        let mut wrong_url = policy();
        let descriptor = admission_mut(&mut wrong_url)
            .get_mut("source_snapshot")
            .expect("source descriptor exists");
        object_mut(descriptor).insert(
            "source_url".to_string(),
            Json::String("https://example.invalid/not-franken-alignment.git".to_string()),
        );
        assert_refusal(&wrong_url, "source_url");

        let mut malformed_sha = policy();
        let descriptor = admission_mut(&mut malformed_sha)
            .get_mut("source_snapshot")
            .expect("source descriptor exists");
        object_mut(descriptor).insert("sha256".to_string(), Json::Bool(false));
        assert_refusal(&malformed_sha, "sha256");
    }

    #[test]
    fn target_profiles_are_fixed_even_when_rows_follow_the_mutation() {
        let removed_target = "x86_64-unknown-linux-gnu";
        let mut jointly_removed = policy();
        let profiles = admission_mut(&mut jointly_removed)
            .get_mut("target_profiles")
            .expect("profiles exist");
        match profiles {
            Json::Array(profiles) => profiles.retain(|profile| {
                profile.get("target").and_then(Json::as_str) != Some(removed_target)
            }),
            _ => panic!("profiles must be an array"),
        }
        for index in 0..2 {
            match row_mut(&mut jointly_removed, index)
                .get_mut("targets")
                .expect("row targets exist")
            {
                Json::Array(targets) => {
                    targets.retain(|target| target.as_str() != Some(removed_target));
                }
                _ => panic!("row targets must be an array"),
            }
        }
        assert_refusal(&jointly_removed, "target_profiles");

        let mut unknown_target = policy();
        let cloned_profile = match admission_mut(&mut unknown_target)
            .get_mut("target_profiles")
            .expect("profiles exist")
        {
            Json::Array(profiles) => profiles[0].clone(),
            _ => panic!("profiles must be an array"),
        };
        let mut cloned_profile = cloned_profile;
        object_mut(&mut cloned_profile).insert(
            "target".to_string(),
            Json::String("unknown-qualification-target".to_string()),
        );
        match admission_mut(&mut unknown_target)
            .get_mut("target_profiles")
            .expect("profiles exist")
        {
            Json::Array(profiles) => profiles.push(cloned_profile),
            _ => panic!("profiles must be an array"),
        }
        for index in 0..2 {
            match row_mut(&mut unknown_target, index)
                .get_mut("targets")
                .expect("row targets exist")
            {
                Json::Array(targets) => {
                    targets.push(Json::String("unknown-qualification-target".to_string()));
                }
                _ => panic!("row targets must be an array"),
            }
        }
        assert_refusal(&unknown_target, "target_profiles");
    }

    #[test]
    fn permissive_external_flags_and_nonempty_rows_refuse() {
        for flag in ["supported", "approved"] {
            let mut permissive = policy();
            let external = admission_mut(&mut permissive)
                .get_mut("external_admission")
                .expect("external policy exists");
            object_mut(external).insert(flag.to_string(), Json::Bool(true));
            assert_refusal(&permissive, flag);
        }

        let mut no_empty_rule = policy();
        let external = admission_mut(&mut no_empty_rule)
            .get_mut("external_admission")
            .expect("external policy exists");
        object_mut(external).insert(
            "external_rows_must_remain_empty".to_string(),
            Json::Bool(false),
        );
        assert_refusal(&no_empty_rule, "external_rows_must_remain_empty");

        let mut row = policy();
        match admission_mut(&mut row)
            .get_mut("external_rows")
            .expect("external rows exist")
        {
            Json::Array(rows) => rows.push(Json::Object(BTreeMap::new())),
            _ => panic!("external rows must be an array"),
        }
        assert_refusal(&row, "external_rows");
    }

    #[test]
    fn future_field_build_shape_and_xtask_tools_are_causal() {
        let mut missing_future = policy();
        let external = admission_mut(&mut missing_future)
            .get_mut("external_admission")
            .expect("external policy exists");
        let required = object_mut(external)
            .get_mut("required_fields_when_implemented")
            .expect("future requirements exist");
        match object_mut(required)
            .get_mut("closure")
            .expect("closure exists")
        {
            Json::Array(values) => {
                values.retain(|value| value.as_str() != Some("downloaded_artifacts"));
            }
            _ => panic!("closure must be an array"),
        }
        assert_refusal(&missing_future, "closure");

        let mut omitted_build = policy();
        let build = row_mut(&mut omitted_build, 0)
            .get_mut("build_closure")
            .expect("build closure exists");
        object_mut(build).remove("build_scripts");
        assert_refusal(&omitted_build, "build_closure");

        let mut empty_xtask_tools = policy();
        let runtime = row_mut(&mut empty_xtask_tools, 1)
            .get_mut("runtime_closure")
            .expect("runtime closure exists");
        object_mut(runtime).insert("operator_tools".to_string(), Json::Array(Vec::new()));
        assert_refusal(&empty_xtask_tools, "operator_tools");
    }

    #[test]
    fn malformed_and_duplicate_future_rule_entries_refuse() {
        let mut duplicate = policy();
        let external = admission_mut(&mut duplicate)
            .get_mut("external_admission")
            .expect("external policy exists");
        let required = object_mut(external)
            .get_mut("required_fields_when_implemented")
            .expect("future requirements exist");
        match object_mut(required)
            .get_mut("identity")
            .expect("identity exists")
        {
            Json::Array(values) => values.push(Json::String("name".to_string())),
            _ => panic!("identity must be an array"),
        }
        assert_refusal(&duplicate, "identity");

        let mut malformed = policy();
        let external = admission_mut(&mut malformed)
            .get_mut("external_admission")
            .expect("external policy exists");
        let required = object_mut(external)
            .get_mut("required_fields_when_implemented")
            .expect("future requirements exist");
        object_mut(required).insert("scope".to_string(), Json::Bool(false));
        assert_refusal(&malformed, "scope");
    }

    #[test]
    fn top_level_closed_universe_controls_are_consumed() {
        for field in ["external_exceptions", "foundation_admissions"] {
            let mut expanded = policy();
            match object_mut(&mut expanded)
                .get_mut(field)
                .expect("array exists")
            {
                Json::Array(values) => values.push(Json::String("planted".to_string())),
                _ => panic!("closed-universe field must be an array"),
            }
            assert_refusal(&expanded, field);
        }

        let mut audited = policy();
        object_mut(&mut audited).insert(
            "transitive_feature_audits".to_string(),
            Json::String("complete".to_string()),
        );
        assert_refusal(&audited, "transitive_feature_audits");
    }
}
