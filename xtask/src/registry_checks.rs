//! Fail-closed structural checks for the owned registry core (roadmap FA-003).
//!
//! This module establishes repository-internal consistency only.  It does not
//! promote a roadmap packet, prove a production invariant, or replace the
//! dedicated founding-concordance checker.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Component, Path};

use crate::json::{Json, Limits, parse};

const INVARIANTS: &str = "registry/invariants.json";
const ROADMAP: &str = "registry/roadmap.json";
const CLAIMS: &str = "registry/claims.json";
const SOURCES: &str = "registry/sources.json";
const FOUNDING: &str = "registry/founding_concordance.json";
const MAX_ROADMAP_NODES: usize = 1_000;
const MAX_DEPENDENCIES_PER_PACKET: usize = 1_000;
const MAX_REFERENCE_SOURCE_BYTES: usize = 1_048_576;

/// One causal registry refusal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    /// Repository-relative registry input that caused the refusal.
    pub file: &'static str,
    /// Object identity, when it could be read without inventing one.
    pub id: Option<String>,
    /// Stable machine-readable refusal cause.
    pub code: &'static str,
    /// Field-level location inside `file`.
    pub location: String,
    /// Actionable explanation of the observed defect.
    pub detail: String,
}

impl Finding {
    fn new(
        file: &'static str,
        id: Option<&str>,
        code: &'static str,
        location: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            file,
            id: id.map(str::to_string),
            code,
            location: location.into(),
            detail: detail.into(),
        }
    }
}

/// The complete result of one registry-core pass.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Every detected structural refusal in deterministic order.
    pub findings: Vec<Finding>,
}

impl Report {
    /// A registry pass is clean only when it found no structural refusal.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }
}

struct Documents {
    invariants: Json,
    roadmap: Json,
    claims: Json,
    sources: Json,
    founding: Json,
}

/// Raw bytes for the five registry inputs covered by the FA-003 core checker.
///
/// This is intentionally a byte-level input: independent tests can mutate one
/// document while keeping the other four valid, and the strict JSON parser
/// remains the single decoding boundary.
#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub struct RegistryInputs<'a> {
    pub invariants: &'a [u8],
    pub roadmap: &'a [u8],
    pub claims: &'a [u8],
    pub sources: &'a [u8],
    pub founding: &'a [u8],
}

/// Read the current registry inputs and validate the FA-003 core boundaries.
///
/// I/O and JSON parse failures are returned as findings rather than becoming
/// an implicit skipped pass.  Callers must treat a non-clean [`Report`] as a
/// failed gate.
#[must_use]
pub fn check_repository(root: &Path) -> Report {
    let mut findings = Vec::new();
    let invariants = read_json(root, INVARIANTS, &mut findings);
    let roadmap = read_json(root, ROADMAP, &mut findings);
    let claims = read_json(root, CLAIMS, &mut findings);
    let sources = read_json(root, SOURCES, &mut findings);
    let founding = read_json(root, FOUNDING, &mut findings);

    match (invariants, roadmap, claims, sources, founding) {
        (Some(invariants), Some(roadmap), Some(claims), Some(sources), Some(founding)) => {
            check_parsed_documents(
                root,
                Documents {
                    invariants,
                    roadmap,
                    claims,
                    sources,
                    founding,
                },
                findings,
            )
        }
        _ => finish(findings),
    }
}

/// Validate supplied registry bytes using the same strict parser and semantic
/// checks as [`check_repository`].
#[cfg(test)]
#[must_use]
pub fn check_inputs(root: &Path, inputs: RegistryInputs<'_>) -> Report {
    let mut findings = Vec::new();
    let invariants = parse_document(inputs.invariants, INVARIANTS, &mut findings);
    let roadmap = parse_document(inputs.roadmap, ROADMAP, &mut findings);
    let claims = parse_document(inputs.claims, CLAIMS, &mut findings);
    let sources = parse_document(inputs.sources, SOURCES, &mut findings);
    let founding = parse_document(inputs.founding, FOUNDING, &mut findings);
    match (invariants, roadmap, claims, sources, founding) {
        (Some(invariants), Some(roadmap), Some(claims), Some(sources), Some(founding)) => {
            check_parsed_documents(
                root,
                Documents {
                    invariants,
                    roadmap,
                    claims,
                    sources,
                    founding,
                },
                findings,
            )
        }
        _ => finish(findings),
    }
}

fn read_json(root: &Path, file: &'static str, findings: &mut Vec<Finding>) -> Option<Json> {
    let path = root.join(file);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            findings.push(Finding::new(
                file,
                None,
                "read_failed",
                "$",
                format!("cannot read {}: {error}", path.display()),
            ));
            return None;
        }
    };
    parse_document(&bytes, file, findings)
}

fn parse_document(bytes: &[u8], file: &'static str, findings: &mut Vec<Finding>) -> Option<Json> {
    match parse(bytes, Limits::default()) {
        Ok(value) => Some(value),
        Err(error) => {
            findings.push(Finding::new(
                file,
                None,
                "invalid_json",
                "$",
                format!("strict JSON parse failed: {error}"),
            ));
            None
        }
    }
}

fn check_parsed_documents(root: &Path, docs: Documents, mut findings: Vec<Finding>) -> Report {
    let invariant_rows = rows(&docs.invariants, INVARIANTS, "invariants", &mut findings);
    let roadmap_rows = rows(&docs.roadmap, ROADMAP, "packets", &mut findings);
    let claim_rows = rows(&docs.claims, CLAIMS, "claims", &mut findings);
    let source_rows = rows(&docs.sources, SOURCES, "sources", &mut findings);
    let founding_rows = rows(&docs.founding, FOUNDING, "founding_ideas", &mut findings);

    let invariant_ids = ids(invariant_rows, INVARIANTS, "FA-INV-", &mut findings);
    let roadmap_ids = ids(roadmap_rows, ROADMAP, "FA-", &mut findings);
    drop(ids(claim_rows, CLAIMS, "H", &mut findings));
    let source_ids = ids(source_rows, SOURCES, "SOURCE", &mut findings);
    let founding_ids = ids(founding_rows, FOUNDING, "FOUNDING", &mut findings);

    if let Some(rows) = roadmap_rows {
        check_roadmap(rows, &roadmap_ids, &invariant_ids, &mut findings);
        check_artifact_references(rows, ROADMAP, "result_artifacts", root, &mut findings);
    }
    if let Some(rows) = invariant_rows {
        check_string_references(
            rows,
            INVARIANTS,
            "founding_ideas",
            &founding_ids,
            "unknown_founding_idea",
            &mut findings,
        );
        check_artifact_references(rows, INVARIANTS, "evidence_artifacts", root, &mut findings);
        check_reference_checks(rows, root, &mut findings);
    }
    if let Some(rows) = claim_rows {
        check_string_references(
            rows,
            CLAIMS,
            "source_refs",
            &source_ids,
            "unknown_source",
            &mut findings,
        );
        check_string_references(
            rows,
            CLAIMS,
            "founding_ideas",
            &founding_ids,
            "unknown_founding_idea",
            &mut findings,
        );
        check_file_references(rows, CLAIMS, "experiment_card", root, &mut findings);
        check_artifact_references(rows, CLAIMS, "result_artifacts", root, &mut findings);
    }
    if let Some(rows) = founding_rows {
        check_single_string_references(
            rows,
            FOUNDING,
            "source",
            &source_ids,
            "unknown_source",
            &mut findings,
        );
    }
    if let Some(source_document) = docs.sources.as_object()
        && let Some(value) = source_document.get("revision_0_2_source_inventory")
    {
        match value.as_str() {
            Some(path) => check_existing_file(
                SOURCES,
                None,
                "revision_0_2_source_inventory",
                path,
                root,
                &mut findings,
            ),
            None => findings.push(type_finding(
                SOURCES,
                None,
                "expected_string",
                "$.revision_0_2_source_inventory",
                "string",
                value,
            )),
        }
    }

    finish(findings)
}

fn finish(mut findings: Vec<Finding>) -> Report {
    findings.sort_by(|left, right| {
        left.file
            .cmp(right.file)
            .then_with(|| left.id.cmp(&right.id))
            .then_with(|| left.code.cmp(right.code))
            .then_with(|| left.location.cmp(&right.location))
    });
    Report { findings }
}

fn rows<'a>(
    document: &'a Json,
    file: &'static str,
    key: &'static str,
    findings: &mut Vec<Finding>,
) -> Option<&'a [Json]> {
    let object = match document.as_object() {
        Some(object) => object,
        None => {
            findings.push(type_finding(
                file,
                None,
                "expected_object",
                "$",
                "object",
                document,
            ));
            return None;
        }
    };
    let value = match object.get(key) {
        Some(value) => value,
        None => {
            findings.push(Finding::new(
                file,
                None,
                "missing_field",
                "$",
                format!("required top-level field `{key}` is absent"),
            ));
            return None;
        }
    };
    match value.as_array() {
        Some(rows) => Some(rows),
        None => {
            findings.push(type_finding(
                file,
                None,
                "expected_array",
                format!("$.{key}"),
                "array",
                value,
            ));
            None
        }
    }
}

fn ids(
    rows: Option<&[Json]>,
    file: &'static str,
    prefix: &str,
    findings: &mut Vec<Finding>,
) -> BTreeSet<String> {
    let mut known = BTreeSet::new();
    let Some(rows) = rows else {
        return known;
    };
    for (index, row) in rows.iter().enumerate() {
        let location = format!("$[{index}]");
        let Some(object) = row.as_object() else {
            findings.push(type_finding(
                file,
                None,
                "expected_object",
                location,
                "object",
                row,
            ));
            continue;
        };
        let Some(value) = object.get("id") else {
            findings.push(Finding::new(
                file,
                None,
                "missing_field",
                format!("{location}.id"),
                "registry rows require a string `id`",
            ));
            continue;
        };
        let Some(id) = value.as_str() else {
            findings.push(type_finding(
                file,
                None,
                "expected_string",
                format!("{location}.id"),
                "string",
                value,
            ));
            continue;
        };
        if !well_formed_id(id, prefix) {
            findings.push(Finding::new(
                file,
                Some(id),
                "malformed_id",
                format!("{location}.id"),
                format!("`{id}` is not a well-formed identifier for this registry"),
            ));
        }
        if !known.insert(id.to_string()) {
            findings.push(Finding::new(
                file,
                Some(id),
                "duplicate_id",
                format!("{location}.id"),
                format!("identifier `{id}` occurs more than once"),
            ));
        }
    }
    known
}

fn well_formed_id(id: &str, prefix: &str) -> bool {
    if prefix == "SOURCE" {
        return id.contains('-')
            && id.split('-').all(|segment| {
                !segment.is_empty()
                    && segment
                        .bytes()
                        .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
            });
    }
    if prefix == "FOUNDING" {
        if let Some(number) = id.strip_prefix("FS-") {
            return number.len() == 2 && number.bytes().all(|byte| byte.is_ascii_digit());
        }
        let Some(suffix) = id.strip_prefix("FI-") else {
            return false;
        };
        let bytes = suffix.as_bytes();
        return bytes.len() == 3
            && matches!(bytes[0], b'A' | b'I' | b'S')
            && bytes[1..].iter().all(|byte| byte.is_ascii_digit());
    }
    let Some(suffix) = id.strip_prefix(prefix) else {
        return false;
    };
    match prefix {
        "FA-INV-" => suffix.len() == 3 && suffix.bytes().all(|byte| byte.is_ascii_digit()),
        "FA-" => suffix.len() == 3 && suffix.bytes().all(|byte| byte.is_ascii_digit()),
        "H" => !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit()),
        _ => false,
    }
}

fn check_single_string_references(
    rows: &[Json],
    file: &'static str,
    key: &'static str,
    known: &BTreeSet<String>,
    code: &'static str,
    findings: &mut Vec<Finding>,
) {
    for (index, row) in rows.iter().enumerate() {
        let Some(object) = row.as_object() else {
            continue;
        };
        if !object.contains_key(key) {
            continue;
        }
        let id = object.get("id").and_then(Json::as_str);
        let base = format!("$[{index}]");
        let Some(value) = object.get(key) else {
            findings.push(Finding::new(
                file,
                id,
                "missing_field",
                format!("{base}.{key}"),
                format!("required field `{key}` is absent"),
            ));
            continue;
        };
        let Some(value) = value.as_str() else {
            findings.push(type_finding(
                file,
                id,
                "expected_string",
                format!("{base}.{key}"),
                "string",
                value,
            ));
            continue;
        };
        if !known.contains(value) {
            findings.push(Finding::new(
                file,
                id,
                code,
                format!("{base}.{key}"),
                format!("`{value}` is not present in its referenced registry"),
            ));
        }
    }
}

fn check_roadmap(
    rows: &[Json],
    roadmap_ids: &BTreeSet<String>,
    invariant_ids: &BTreeSet<String>,
    findings: &mut Vec<Finding>,
) {
    if rows.len() > MAX_ROADMAP_NODES {
        findings.push(Finding::new(
            ROADMAP,
            None,
            "graph_node_limit_exceeded",
            "$.packets",
            format!(
                "roadmap contains {} packet rows; the graph limit is {MAX_ROADMAP_NODES}",
                rows.len()
            ),
        ));
        return;
    }
    let mut edges = BTreeMap::new();
    for (index, row) in rows.iter().enumerate() {
        let Some(object) = row.as_object() else {
            continue;
        };
        let Some(id) = object.get("id").and_then(Json::as_str) else {
            continue;
        };
        let base = format!("$.packets[{index}]");
        if !well_formed_id(id, "FA-") {
            // `ids` has retained the causal malformed-id finding.  Invalid
            // identifiers never become graph vertices, even if another row
            // names the same malformed string as a dependency.
            continue;
        }
        let dependencies = string_array(object, ROADMAP, id, &base, "depends_on", findings);
        let invariants = string_array(object, ROADMAP, id, &base, "invariants", findings);
        if let Some(dependencies) = dependencies {
            if dependencies.len() > MAX_DEPENDENCIES_PER_PACKET {
                findings.push(Finding::new(
                    ROADMAP,
                    Some(id),
                    "dependency_limit_exceeded",
                    format!("{base}.depends_on"),
                    format!(
                        "packet `{id}` has {} dependencies; the per-packet limit is {MAX_DEPENDENCIES_PER_PACKET}",
                        dependencies.len()
                    ),
                ));
                continue;
            }
            for dependency in &dependencies {
                if !well_formed_id(dependency, "FA-") || !roadmap_ids.contains(dependency) {
                    findings.push(Finding::new(
                        ROADMAP,
                        Some(id),
                        "unknown_dependency",
                        format!("{base}.depends_on"),
                        format!("packet `{id}` depends on missing packet `{dependency}`"),
                    ));
                }
            }
            edges.insert(id.to_string(), dependencies);
        }
        if let Some(invariants) = invariants {
            for invariant in invariants {
                if !invariant_ids.contains(&invariant) {
                    findings.push(Finding::new(
                        ROADMAP,
                        Some(id),
                        "unknown_invariant",
                        format!("{base}.invariants"),
                        format!("packet `{id}` names missing invariant `{invariant}`"),
                    ));
                }
            }
        }
    }
    detect_cycles(&edges, findings);
}

fn detect_cycles(edges: &BTreeMap<String, Vec<String>>, findings: &mut Vec<Finding>) {
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut stack = Vec::new();
    for node in edges.keys() {
        visit(
            node,
            edges,
            &mut visiting,
            &mut visited,
            &mut stack,
            findings,
        );
    }
}

fn visit(
    node: &str,
    edges: &BTreeMap<String, Vec<String>>,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    stack: &mut Vec<String>,
    findings: &mut Vec<Finding>,
) {
    if visited.contains(node) {
        return;
    }
    if !visiting.insert(node.to_string()) {
        let start = stack.iter().position(|item| item == node).unwrap_or(0);
        let mut cycle = stack[start..].to_vec();
        cycle.push(node.to_string());
        findings.push(Finding::new(
            ROADMAP,
            Some(node),
            "dependency_cycle",
            "$.packets.depends_on",
            format!("roadmap dependency cycle: {}", cycle.join(" -> ")),
        ));
        return;
    }
    stack.push(node.to_string());
    if let Some(dependencies) = edges.get(node) {
        for dependency in dependencies {
            if edges.contains_key(dependency) {
                visit(dependency, edges, visiting, visited, stack, findings);
            }
        }
    }
    stack.pop();
    visiting.remove(node);
    visited.insert(node.to_string());
}

fn check_string_references(
    rows: &[Json],
    file: &'static str,
    key: &'static str,
    known: &BTreeSet<String>,
    code: &'static str,
    findings: &mut Vec<Finding>,
) {
    for (index, row) in rows.iter().enumerate() {
        let Some(object) = row.as_object() else {
            continue;
        };
        if !object.contains_key(key) {
            continue;
        }
        let id = object.get("id").and_then(Json::as_str);
        let base = format!("$[{index}]");
        let Some(values) = string_array(
            object,
            file,
            id.unwrap_or("<missing>"),
            &base,
            key,
            findings,
        ) else {
            continue;
        };
        for value in values {
            if !known.contains(&value) {
                findings.push(Finding::new(
                    file,
                    id,
                    code,
                    format!("{base}.{key}"),
                    format!("`{value}` is not present in its referenced registry"),
                ));
            }
        }
    }
}

fn check_artifact_references(
    rows: &[Json],
    file: &'static str,
    key: &'static str,
    root: &Path,
    findings: &mut Vec<Finding>,
) {
    for (index, row) in rows.iter().enumerate() {
        let Some(object) = row.as_object() else {
            continue;
        };
        if !object.contains_key(key) {
            continue;
        }
        let id = object.get("id").and_then(Json::as_str);
        let base = format!("$[{index}]");
        let Some(paths) = string_array(
            object,
            file,
            id.unwrap_or("<missing>"),
            &base,
            key,
            findings,
        ) else {
            continue;
        };
        for path in paths {
            check_existing_file(file, id, &format!("{base}.{key}"), &path, root, findings);
        }
    }
}

fn check_file_references(
    rows: &[Json],
    file: &'static str,
    key: &'static str,
    root: &Path,
    findings: &mut Vec<Finding>,
) {
    for (index, row) in rows.iter().enumerate() {
        let Some(object) = row.as_object() else {
            continue;
        };
        if !object.contains_key(key) {
            continue;
        }
        let id = object.get("id").and_then(Json::as_str);
        let base = format!("$[{index}]");
        let Some(value) = object.get(key) else {
            findings.push(Finding::new(
                file,
                id,
                "missing_field",
                format!("{base}.{key}"),
                format!("required field `{key}` is absent"),
            ));
            continue;
        };
        match value.as_str() {
            Some(path) => {
                check_existing_file(file, id, &format!("{base}.{key}"), path, root, findings)
            }
            None => findings.push(type_finding(
                file,
                id,
                "expected_string",
                format!("{base}.{key}"),
                "string",
                value,
            )),
        }
    }
}

fn check_reference_checks(rows: &[Json], root: &Path, findings: &mut Vec<Finding>) {
    for (index, row) in rows.iter().enumerate() {
        let Some(object) = row.as_object() else {
            continue;
        };
        if !object.contains_key("reference_checks") {
            continue;
        }
        let id = object.get("id").and_then(Json::as_str);
        let base = format!("$[{index}]");
        let Some(symbols) = string_array(
            object,
            INVARIANTS,
            id.unwrap_or("<missing>"),
            &base,
            "reference_checks",
            findings,
        ) else {
            continue;
        };
        for symbol in symbols {
            let Some((path, name)) = symbol.split_once("::tests::") else {
                findings.push(Finding::new(
                    INVARIANTS,
                    id,
                    "malformed_reference_check",
                    format!("{base}.reference_checks"),
                    format!("`{symbol}` must be `path::tests::test_name`"),
                ));
                continue;
            };
            if path.is_empty()
                || Path::new(path)
                    .extension()
                    .and_then(|extension| extension.to_str())
                    != Some("rs")
                || !rust_identifier(name)
            {
                findings.push(Finding::new(
                    INVARIANTS,
                    id,
                    "malformed_reference_check",
                    format!("{base}.reference_checks"),
                    format!("`{symbol}` must name a Rust source file and Rust test identifier"),
                ));
                continue;
            }
            let Some(source_path) = safe_relative_path(path) else {
                findings.push(Finding::new(
                    INVARIANTS,
                    id,
                    "unsafe_reference_path",
                    format!("{base}.reference_checks"),
                    format!("`{symbol}` does not name a safe repository-relative source path"),
                ));
                continue;
            };
            let source_path = match canonical_repository_file(root, source_path) {
                Ok(path) => path,
                Err(RepositoryFileError::Outside { root, candidate }) => {
                    findings.push(Finding::new(
                        INVARIANTS,
                        id,
                        "unsafe_reference_path",
                        format!("{base}.reference_checks"),
                        format!(
                            "`{symbol}` resolves outside {}: {}",
                            root.display(),
                            candidate.display()
                        ),
                    ));
                    continue;
                }
                Err(RepositoryFileError::Unavailable(error)) => {
                    findings.push(Finding::new(
                        INVARIANTS,
                        id,
                        "reference_source_missing",
                        format!("{base}.reference_checks"),
                        format!("cannot resolve `{symbol}` inside the repository: {error}"),
                    ));
                    continue;
                }
            };
            let source = match read_reference_source(&source_path) {
                Ok(source) => source,
                Err(ReferenceSourceError::Limit) => {
                    findings.push(Finding::new(
                        INVARIANTS,
                        id,
                        "reference_source_limit",
                        format!("{base}.reference_checks"),
                        format!(
                            "{} exceeds the {MAX_REFERENCE_SOURCE_BYTES}-byte reference-source limit",
                            source_path.display()
                        ),
                    ));
                    continue;
                }
                Err(ReferenceSourceError::Io(error)) => {
                    findings.push(Finding::new(
                        INVARIANTS,
                        id,
                        "reference_source_missing",
                        format!("{base}.reference_checks"),
                        format!("cannot read {}: {error}", source_path.display()),
                    ));
                    continue;
                }
            };
            if !declares_test(&source, name) {
                findings.push(Finding::new(
                    INVARIANTS,
                    id,
                    "reference_test_missing",
                    format!("{base}.reference_checks"),
                    format!("`{symbol}` does not resolve to a #[test] function"),
                ));
            }
        }
    }
}

fn declares_test(source: &str, name: &str) -> bool {
    let tokens = rust_tokens(source);
    let mut scopes = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        if let Some(next) = skip_macro_body(&tokens, index) {
            index = next;
            continue;
        }
        if scope_is_direct_tests(&scopes) && test_declaration_at(&tokens, index, name) {
            return true;
        }
        match tokens[index] {
            RustToken::Punctuation('{') => {
                scopes.push(module_opening_at(&tokens, index));
            }
            RustToken::Punctuation('}') if scopes.pop().is_none() => return false,
            _ => {}
        }
        index += 1;
    }
    false
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RustToken<'a> {
    Word(&'a str),
    Punctuation(char),
}

fn scope_is_direct_tests(scopes: &[Option<&str>]) -> bool {
    matches!(scopes, [Some("tests")])
}

fn module_opening_at<'a>(tokens: &[RustToken<'a>], brace: usize) -> Option<&'a str> {
    match (
        tokens.get(brace.checked_sub(2)?),
        tokens.get(brace.checked_sub(1)?),
    ) {
        (Some(RustToken::Word(keyword)), Some(RustToken::Word(name))) if *keyword == "mod" => {
            Some(*name)
        }
        _ => None,
    }
}

fn test_declaration_at(tokens: &[RustToken<'_>], index: usize, name: &str) -> bool {
    matches!(
        tokens.get(index..index.saturating_add(7)),
        Some([
            RustToken::Punctuation('#'),
            RustToken::Punctuation('['),
            RustToken::Word(test),
            RustToken::Punctuation(']'),
            RustToken::Word(function),
            RustToken::Word(actual_name),
            RustToken::Punctuation('('),
        ]) if *test == "test" && *function == "fn" && *actual_name == name
    )
}

fn skip_macro_body(tokens: &[RustToken<'_>], bang: usize) -> Option<usize> {
    if !matches!(tokens.get(bang), Some(RustToken::Punctuation('!'))) {
        return None;
    }
    let group = match tokens.get(bang + 1) {
        Some(RustToken::Punctuation(open @ ('{' | '[' | '('))) => Some((bang + 1, *open)),
        Some(RustToken::Word(_)) => match tokens.get(bang + 2) {
            Some(RustToken::Punctuation(open @ ('{' | '[' | '('))) => Some((bang + 2, *open)),
            _ => None,
        },
        _ => None,
    }?;
    let close = match group.1 {
        '{' => '}',
        '[' => ']',
        '(' => ')',
        _ => return None,
    };
    let mut depth = 1_usize;
    let mut index = group.0 + 1;
    while index < tokens.len() {
        if matches!(tokens[index], RustToken::Punctuation(punctuation) if punctuation == group.1) {
            depth += 1;
        } else if matches!(tokens[index], RustToken::Punctuation(punctuation) if punctuation == close)
        {
            depth -= 1;
            if depth == 0 {
                return Some(index + 1);
            }
        }
        index += 1;
    }
    Some(tokens.len())
}

fn rust_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(byte) if byte == b'_' || byte.is_ascii_alphabetic())
        && bytes.all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
}

fn rust_tokens(source: &str) -> Vec<RustToken<'_>> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_whitespace() {
            index += 1;
        } else if bytes[index..].starts_with(b"//") {
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
        } else if bytes[index..].starts_with(b"/*") {
            index = skip_block_comment(bytes, index);
        } else if let Some(next) = raw_string_end(bytes, index) {
            index = next;
        } else if bytes[index] == b'"' {
            index = skip_quoted(bytes, index, b'"');
        } else if bytes[index] == b'\'' {
            if let Some(next) = char_literal_end(source, index) {
                index = next;
            } else {
                tokens.push(RustToken::Punctuation('\''));
                index += 1;
            }
        } else if bytes[index].is_ascii_alphabetic() || bytes[index] == b'_' {
            let start = index;
            index += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
            {
                index += 1;
            }
            // This slice begins and ends at ASCII identifier boundaries.
            tokens.push(RustToken::Word(&source[start..index]));
        } else {
            tokens.push(RustToken::Punctuation(char::from(bytes[index])));
            index += 1;
        }
    }
    tokens
}

fn skip_block_comment(bytes: &[u8], mut index: usize) -> usize {
    let mut depth = 1_u32;
    index += 2;
    while index < bytes.len() && depth > 0 {
        if bytes[index..].starts_with(b"/*") {
            depth = depth.saturating_add(1);
            index += 2;
        } else if bytes[index..].starts_with(b"*/") {
            depth -= 1;
            index += 2;
        } else {
            index += 1;
        }
    }
    index
}

fn raw_string_end(bytes: &[u8], index: usize) -> Option<usize> {
    let mut marker = index;
    if matches!(bytes.get(marker), Some(b'b' | b'c')) {
        marker += 1;
    }
    if bytes.get(marker) != Some(&b'r') {
        return None;
    }
    marker += 1;
    let hashes_start = marker;
    while bytes.get(marker) == Some(&b'#') {
        marker += 1;
    }
    if bytes.get(marker) != Some(&b'"') {
        return None;
    }
    let hash_count = marker - hashes_start;
    marker += 1;
    while marker < bytes.len() {
        if bytes[marker] == b'"'
            && bytes
                .get(marker + 1..marker + 1 + hash_count)
                .is_some_and(|closing| closing.iter().all(|byte| *byte == b'#'))
        {
            return Some(marker + 1 + hash_count);
        }
        marker += 1;
    }
    Some(bytes.len())
}

fn skip_quoted(bytes: &[u8], mut index: usize, quote: u8) -> usize {
    index += 1;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index = index.saturating_add(2);
        } else if bytes[index] == quote {
            return index + 1;
        } else {
            index += 1;
        }
    }
    bytes.len()
}

fn char_literal_end(source: &str, index: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let literal = source.get(index + 1..)?;
    let mut characters = literal.chars();
    let first = characters.next()?;
    let mut end = index + 1 + first.len_utf8();
    if first == '\\' {
        let escaped = characters.next()?;
        end += escaped.len_utf8();
        if escaped == 'x' {
            let digits = bytes.get(end..end + 2)?;
            if !digits.iter().all(|byte| byte.is_ascii_hexdigit()) {
                return None;
            }
            end += 2;
        } else if escaped == 'u' && bytes.get(end) == Some(&b'{') {
            end += 1;
            let digits_start = end;
            while matches!(bytes.get(end), Some(byte) if byte.is_ascii_hexdigit() || *byte == b'_')
            {
                end += 1;
            }
            if end == digits_start || bytes.get(end) != Some(&b'}') {
                return None;
            }
            end += 1;
        }
    }
    (bytes.get(end) == Some(&b'\'')).then_some(end + 1)
}

fn string_array(
    object: &BTreeMap<String, Json>,
    file: &'static str,
    id: &str,
    base: &str,
    key: &'static str,
    findings: &mut Vec<Finding>,
) -> Option<Vec<String>> {
    let value = match object.get(key) {
        Some(value) => value,
        None => {
            findings.push(Finding::new(
                file,
                Some(id),
                "missing_field",
                format!("{base}.{key}"),
                format!("required field `{key}` is absent"),
            ));
            return None;
        }
    };
    let Some(values) = value.as_array() else {
        findings.push(type_finding(
            file,
            Some(id),
            "expected_array",
            format!("{base}.{key}"),
            "array",
            value,
        ));
        return None;
    };
    let mut strings = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        match value.as_str() {
            Some(value) => strings.push(value.to_string()),
            None => {
                findings.push(type_finding(
                    file,
                    Some(id),
                    "expected_string",
                    format!("{base}.{key}[{index}]"),
                    "string",
                    value,
                ));
                return None;
            }
        }
    }
    Some(strings)
}

fn check_existing_file(
    file: &'static str,
    id: Option<&str>,
    location: &str,
    referenced: &str,
    root: &Path,
    findings: &mut Vec<Finding>,
) {
    let Some(path) = safe_relative_path(referenced) else {
        findings.push(Finding::new(
            file,
            id,
            "unsafe_file_reference",
            location,
            format!("`{referenced}` is not a safe repository-relative path"),
        ));
        return;
    };
    match canonical_repository_file(root, path) {
        Ok(_) => {}
        Err(RepositoryFileError::Outside { root, candidate }) => findings.push(Finding::new(
            file,
            id,
            "unsafe_file_reference",
            location,
            format!(
                "retained reference `{referenced}` resolves outside {}: {}",
                root.display(),
                candidate.display()
            ),
        )),
        Err(RepositoryFileError::Unavailable(error)) => findings.push(Finding::new(
            file,
            id,
            "referenced_file_missing",
            location,
            format!("retained reference `{referenced}` is unavailable: {error}"),
        )),
    }
}

enum ReferenceSourceError {
    Io(std::io::Error),
    Limit,
}

fn read_reference_source(path: &Path) -> Result<String, ReferenceSourceError> {
    let file = fs::File::open(path).map_err(ReferenceSourceError::Io)?;
    let mut source = String::new();
    file.take((MAX_REFERENCE_SOURCE_BYTES + 1) as u64)
        .read_to_string(&mut source)
        .map_err(ReferenceSourceError::Io)?;
    if source.len() > MAX_REFERENCE_SOURCE_BYTES {
        return Err(ReferenceSourceError::Limit);
    }
    Ok(source)
}

enum RepositoryFileError {
    Outside {
        root: std::path::PathBuf,
        candidate: std::path::PathBuf,
    },
    Unavailable(std::io::Error),
}

fn canonical_repository_file(
    root: &Path,
    relative: &Path,
) -> Result<std::path::PathBuf, RepositoryFileError> {
    let root = fs::canonicalize(root).map_err(RepositoryFileError::Unavailable)?;
    let candidate =
        fs::canonicalize(root.join(relative)).map_err(RepositoryFileError::Unavailable)?;
    if !candidate.starts_with(&root) {
        return Err(RepositoryFileError::Outside { root, candidate });
    }
    if !candidate.is_file() {
        return Err(RepositoryFileError::Unavailable(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "resolved path is not a file",
        )));
    }
    Ok(candidate)
}

fn safe_relative_path(value: &str) -> Option<&Path> {
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return None;
    }
    Some(path)
}

fn type_finding(
    file: &'static str,
    id: Option<&str>,
    code: &'static str,
    location: impl Into<String>,
    expected: &str,
    found: &Json,
) -> Finding {
    Finding::new(
        file,
        id,
        code,
        location,
        format!("expected {expected}, found {}", found.kind()),
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    const DUPLICATE_ROADMAP: &str =
        include_str!("../tests/fixtures/registry/duplicate-roadmap-id.json");
    const DEPENDENCY_CYCLE: &str =
        include_str!("../tests/fixtures/registry/dependency-cycle-roadmap.json");
    const MISSING_INVARIANT: &str =
        include_str!("../tests/fixtures/registry/missing-invariant-roadmap.json");
    const MISSING_REFERENCE_SYMBOL: &str =
        include_str!("../tests/fixtures/registry/missing-reference-symbol-invariants.json");
    const MISSING_EVIDENCE_ARTIFACT: &str =
        include_str!("../tests/fixtures/registry/missing-evidence-artifact-invariants.json");

    fn repository_root() -> PathBuf {
        crate::workspace_root().expect("actual invocation workspace")
    }

    fn fixture_documents(bytes: &str) -> Documents {
        let fixture = parse(bytes.as_bytes(), Limits::default()).expect("fixture parses");
        let object = fixture.as_object().expect("fixture root is an object");
        let document = |key: &str| {
            object
                .get(key)
                .unwrap_or_else(|| panic!("fixture lacks `{key}`"))
                .clone()
        };
        Documents {
            invariants: document("invariants"),
            roadmap: document("roadmap"),
            claims: document("claims"),
            sources: document("sources"),
            founding: document("founding"),
        }
    }

    fn fixture_report(bytes: &str) -> Report {
        check_parsed_documents(&repository_root(), fixture_documents(bytes), Vec::new())
    }

    fn assert_cause(report: &Report, file: &str, code: &str) {
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.file == file && finding.code == code),
            "expected causal refusal {file}:{code}; got {:?}",
            report.findings
        );
        assert!(
            !report.is_clean(),
            "a refusal cannot be a clean registry pass"
        );
    }

    #[test]
    fn reference_source_read_refuses_more_than_the_declared_byte_limit() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "franken-alignment-reference-source-limit-{}-{nonce}.rs",
            std::process::id()
        ));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .expect("atomically claim temporary over-limit source");
        file.write_all(&vec![b'x'; MAX_REFERENCE_SOURCE_BYTES + 1])
            .expect("temporary over-limit source is writable");
        drop(file);
        let result = read_reference_source(&path);
        let _ = fs::remove_file(&path);
        assert!(
            matches!(result, Err(ReferenceSourceError::Limit)),
            "over-limit reference source must fail with its causal limit error"
        );
    }

    #[test]
    fn current_owned_registry_inputs_pass_the_core_checks() {
        let report = check_repository(&repository_root());
        assert!(
            report.is_clean(),
            "current registry inputs must satisfy the owned FA-003 core checks: {:?}",
            report.findings
        );
    }

    #[test]
    fn duplicate_roadmap_id_is_refused_as_an_identity_collision() {
        assert_cause(&fixture_report(DUPLICATE_ROADMAP), ROADMAP, "duplicate_id");
    }

    #[test]
    fn dependency_cycle_is_refused_as_a_dag_violation() {
        assert_cause(
            &fixture_report(DEPENDENCY_CYCLE),
            ROADMAP,
            "dependency_cycle",
        );
    }

    #[test]
    fn unknown_roadmap_invariant_is_refused_at_the_link() {
        assert_cause(
            &fixture_report(MISSING_INVARIANT),
            ROADMAP,
            "unknown_invariant",
        );
    }

    #[test]
    fn missing_reference_test_symbol_is_not_a_textual_near_match() {
        assert_cause(
            &fixture_report(MISSING_REFERENCE_SYMBOL),
            INVARIANTS,
            "reference_test_missing",
        );
    }

    #[test]
    fn missing_retained_evidence_artifact_is_refused() {
        assert_cause(
            &fixture_report(MISSING_EVIDENCE_ARTIFACT),
            INVARIANTS,
            "referenced_file_missing",
        );
    }
}
