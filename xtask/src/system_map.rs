//! Bounded structural conformance checks for the system-map and vocabulary.
//!
//! This is an enabler for the prospective public surface, not an execution
//! claim: a planned layer suite is never treated as a passing suite here.

use std::collections::{BTreeMap, BTreeSet};

use crate::concordance::{self, closes_fence, opening_fence};
use crate::json::{Json, Limits, parse};

const SYSTEM_MAP_FILE: &str = "registry/system_map.json";
const VOCABULARY_FILE: &str = "registry/vocabulary.json";
const INVARIANTS_FILE: &str = "registry/invariants.json";
const ROADMAP_FILE: &str = "registry/roadmap.json";
const README_FILE: &str = "README.md";
const GUIDE_FILE: &str = "docs/AGENT_GUIDE.md";
const SYSTEM_MAP_DOC_FILE: &str = "docs/SYSTEM_MAP.md";
const BEADS_FILE: &str = ".beads/issues.jsonl";
const PLAN_FILE: &str = "COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENALIGNMENT.md";

const MAX_TEXT_BYTES: usize = 8 * 1024 * 1024;
const MAX_ROWS: usize = 4_096;
const MAX_FINDINGS: usize = 8_192;

/// All checked inputs are injected by the gate owner.  The checker performs no
/// filesystem reads, making causal fixture mutation possible without hidden
/// repository state.
pub struct Inputs<'a> {
    pub system_map: &'a Json,
    pub vocabulary: &'a Json,
    pub invariants: &'a Json,
    pub roadmap: &'a Json,
    pub readme: &'a str,
    pub agent_guide: &'a str,
    pub system_map_doc: &'a str,
    pub beads_issues: &'a str,
    pub plan: &'a str,
}

/// One deterministic structural refusal.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Finding {
    pub file: String,
    pub id: Option<String>,
    pub code: String,
    pub detail: String,
}

/// Counts describe inspected declarations, not execution or assurance.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Counts {
    pub layers: usize,
    pub verbs: usize,
    pub packets: usize,
    pub guide_commands: usize,
    pub readme_commands: usize,
}

/// Deterministic result of one structural pass.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Report {
    pub findings: Vec<Finding>,
    pub counts: Counts,
}

impl Report {
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }

    /// Render the gate-facing structural report.  The category names are the
    /// zn70.2 contract; individual finding codes remain implementation detail.
    #[must_use]
    pub fn render_json(&self) -> String {
        let mut output = String::from("{\"missing\":[");
        render_findings(
            &mut output,
            self.findings.iter().filter(|finding| {
                finding.code != "retired_verb" && finding.code != "unregistered_verb"
            }),
        );
        output.push_str("],\"retired_in_use\":[");
        render_findings(
            &mut output,
            self.findings
                .iter()
                .filter(|finding| finding.code == "retired_verb"),
        );
        output.push_str("],\"unregistered_verbs\":[");
        render_findings(
            &mut output,
            self.findings
                .iter()
                .filter(|finding| finding.code == "unregistered_verb"),
        );
        output.push_str("],\"counts\":{\"layers\":");
        output.push_str(&self.counts.layers.to_string());
        output.push_str(",\"verbs\":");
        output.push_str(&self.counts.verbs.to_string());
        output.push_str(",\"packets\":");
        output.push_str(&self.counts.packets.to_string());
        output.push_str(",\"guide_commands\":");
        output.push_str(&self.counts.guide_commands.to_string());
        output.push_str(",\"readme_commands\":");
        output.push_str(&self.counts.readme_commands.to_string());
        output.push_str("}}");
        output
    }
}

/// Validate the bounded system-map/vocabulary contract.
#[must_use]
pub fn check(inputs: Inputs<'_>) -> Report {
    let mut checker = Checker::default();
    if !checker.check_text_bounds(&inputs) {
        return checker.finish();
    }

    let map = checker.object(inputs.system_map, SYSTEM_MAP_FILE, "$", None);
    let vocabulary = checker.object(inputs.vocabulary, VOCABULARY_FILE, "$", None);
    let invariants = checker.object(inputs.invariants, INVARIANTS_FILE, "$", None);
    let roadmap = checker.object(inputs.roadmap, ROADMAP_FILE, "$", None);

    let invariant_ids = invariants
        .and_then(|root| checker.id_set(root, INVARIANTS_FILE, "invariants", "FA-INV-"))
        .unwrap_or_default();
    let roadmap_ids = roadmap
        .and_then(|root| checker.id_set(root, ROADMAP_FILE, "packets", "FA-"))
        .unwrap_or_default();
    checker.report.counts.packets = roadmap_ids.len();

    let vocabulary_data = vocabulary.map(|root| checker.vocabulary(root));
    if let Some(map) = map {
        checker.system_map(
            map,
            &invariant_ids,
            &roadmap_ids,
            vocabulary_data.as_ref(),
            inputs.plan,
        );
    }
    if let Some(vocabulary) = vocabulary_data.as_ref() {
        checker.commands(README_FILE, inputs.readme, vocabulary);
        checker.commands(GUIDE_FILE, inputs.agent_guide, vocabulary);
        checker.addresses(README_FILE, inputs.readme, vocabulary);
        checker.addresses(GUIDE_FILE, inputs.agent_guide, vocabulary);
        checker.addresses(SYSTEM_MAP_DOC_FILE, inputs.system_map_doc, vocabulary);
    }
    if let (Some(map), Some(vocabulary)) = (map, vocabulary_data.as_ref()) {
        checker.knowledge_registry(map, vocabulary);
        checker.knowledge(vocabulary, README_FILE, inputs.readme);
        checker.knowledge(vocabulary, GUIDE_FILE, inputs.agent_guide);
        checker.knowledge(vocabulary, SYSTEM_MAP_DOC_FILE, inputs.system_map_doc);
        checker.reason_codes(vocabulary, inputs.plan);
        checker.bead_packet(map, inputs.beads_issues);
    }

    checker.finish()
}

#[derive(Default)]
struct Checker {
    report: Report,
    limit_reported: bool,
}

struct Vocabulary {
    verbs: BTreeSet<String>,
    retired: BTreeSet<String>,
    nouns: BTreeSet<String>,
    knowledge: BTreeSet<String>,
    reason_codes: BTreeSet<String>,
}

impl Checker {
    fn finish(&mut self) -> Report {
        self.report.findings.sort();
        self.report.clone()
    }

    fn finding(&mut self, file: &str, id: Option<&str>, code: &str, detail: impl Into<String>) {
        if self.report.findings.len() < MAX_FINDINGS {
            self.report.findings.push(Finding {
                file: file.to_owned(),
                id: id.map(str::to_owned),
                code: code.to_owned(),
                detail: detail.into(),
            });
        } else if !self.limit_reported {
            self.limit_reported = true;
            self.report.findings.push(Finding {
                file: "system_map_checker".to_owned(),
                id: None,
                code: "finding_limit".to_owned(),
                detail: format!("more than {MAX_FINDINGS} structural findings"),
            });
        }
    }

    fn check_text_bounds(&mut self, inputs: &Inputs<'_>) -> bool {
        let mut bounded = true;
        for (file, text) in [
            (README_FILE, inputs.readme),
            (GUIDE_FILE, inputs.agent_guide),
            (SYSTEM_MAP_DOC_FILE, inputs.system_map_doc),
            (BEADS_FILE, inputs.beads_issues),
            (PLAN_FILE, inputs.plan),
        ] {
            if text.len() > MAX_TEXT_BYTES {
                bounded = false;
                self.finding(
                    file,
                    None,
                    "input_too_large",
                    format!("input exceeds {MAX_TEXT_BYTES} byte bound"),
                );
            }
        }
        bounded
    }

    fn object<'a>(
        &mut self,
        value: &'a Json,
        file: &str,
        location: &str,
        id: Option<&str>,
    ) -> Option<&'a BTreeMap<String, Json>> {
        value.as_object().or_else(|| {
            self.finding(
                file,
                id,
                "invalid_root",
                format!("{location} must be a JSON object, found {}", value.kind()),
            );
            None
        })
    }

    fn array<'a>(
        &mut self,
        object: &'a BTreeMap<String, Json>,
        file: &str,
        key: &str,
        id: Option<&str>,
    ) -> Option<&'a [Json]> {
        match object.get(key) {
            Some(value) => value.as_array().or_else(|| {
                self.finding(
                    file,
                    id,
                    "invalid_type",
                    format!("$.{key} must be an array, found {}", value.kind()),
                );
                None
            }),
            None => {
                self.finding(file, id, "missing_field", format!("$.{key} is required"));
                None
            }
        }
    }

    fn string<'a>(
        &mut self,
        object: &'a BTreeMap<String, Json>,
        file: &str,
        key: &str,
        id: Option<&str>,
    ) -> Option<&'a str> {
        match object.get(key) {
            Some(value) => match value.as_str() {
                Some(value) if !value.trim().is_empty() => Some(value),
                Some(_) => {
                    self.finding(
                        file,
                        id,
                        "missing_field",
                        format!("$.{key} must not be empty"),
                    );
                    None
                }
                None => {
                    self.finding(
                        file,
                        id,
                        "invalid_type",
                        format!("$.{key} must be a string, found {}", value.kind()),
                    );
                    None
                }
            },
            None => {
                self.finding(file, id, "missing_field", format!("$.{key} is required"));
                None
            }
        }
    }

    fn id_set(
        &mut self,
        root: &BTreeMap<String, Json>,
        file: &str,
        key: &str,
        prefix: &str,
    ) -> Option<BTreeSet<String>> {
        let rows = self.array(root, file, key, None)?;
        if rows.len() > MAX_ROWS {
            self.finding(
                file,
                None,
                "input_too_large",
                format!("$.{key} has more than {MAX_ROWS} rows"),
            );
            return None;
        }
        let mut ids = BTreeSet::new();
        for (index, row) in rows.iter().enumerate() {
            let Some(row) = self.object(row, file, &format!("$.{key}[{index}]"), None) else {
                continue;
            };
            let Some(id) = self.string(row, file, "id", None) else {
                continue;
            };
            if !id.starts_with(prefix) {
                self.finding(
                    file,
                    Some(id),
                    "invalid_id",
                    format!("$.{key}[{index}].id must start with {prefix}"),
                );
            }
            if !ids.insert(id.to_owned()) {
                self.finding(
                    file,
                    Some(id),
                    "duplicate_id",
                    format!("$.{key} repeats id {id}"),
                );
            }
        }
        Some(ids)
    }

    fn vocabulary(&mut self, root: &BTreeMap<String, Json>) -> Vocabulary {
        let mut vocabulary = Vocabulary {
            verbs: BTreeSet::new(),
            retired: self.string_set(root, VOCABULARY_FILE, "retired_verbs"),
            nouns: self.string_set(root, VOCABULARY_FILE, "nouns"),
            knowledge: self.string_set(root, VOCABULARY_FILE, "knowledge_variants"),
            reason_codes: self.string_set(root, VOCABULARY_FILE, "reason_codes"),
        };
        let Some(rows) = self.array(root, VOCABULARY_FILE, "verbs", None) else {
            return vocabulary;
        };
        if rows.len() > MAX_ROWS {
            self.finding(
                VOCABULARY_FILE,
                None,
                "input_too_large",
                "$.verbs exceeds row bound",
            );
            return vocabulary;
        }
        self.report.counts.verbs = rows.len();
        for (index, row) in rows.iter().enumerate() {
            let Some(row) = self.object(row, VOCABULARY_FILE, &format!("$.verbs[{index}]"), None)
            else {
                continue;
            };
            let Some(verb) = self.string(row, VOCABULARY_FILE, "verb", None) else {
                continue;
            };
            if !vocabulary.verbs.insert(verb.to_owned()) {
                self.finding(
                    VOCABULARY_FILE,
                    Some(verb),
                    "duplicate_id",
                    "$.verbs repeats verb",
                );
            }
            for field in [
                "layer",
                "authority",
                "class",
                "idempotency",
                "cost_class",
                "response",
            ] {
                if self
                    .string(row, VOCABULARY_FILE, field, Some(verb))
                    .is_none()
                {
                    self.finding(
                        VOCABULARY_FILE,
                        Some(verb),
                        "missing_verb_contract",
                        format!("$.verbs[{index}].{field} is required for a registered verb"),
                    );
                }
            }
            if let Some(class) = self.string(row, VOCABULARY_FILE, "class", Some(verb))
                && !matches!(class, "read" | "rehearsal" | "mutation")
            {
                self.finding(
                    VOCABULARY_FILE,
                    Some(verb),
                    "invalid_verb_class",
                    format!("$.verbs[{index}].class must be read, rehearsal, or mutation"),
                );
            }
            if let Some(layer) = self.string(row, VOCABULARY_FILE, "layer", Some(verb))
                && !valid_layer_descriptor(layer)
            {
                self.finding(
                    VOCABULARY_FILE,
                    Some(verb),
                    "invalid_layer_descriptor",
                    format!(
                        "$.verbs[{index}].layer must use L0-L8 ranges or slash-separated lists"
                    ),
                );
            }
        }
        vocabulary
    }

    fn string_set(
        &mut self,
        root: &BTreeMap<String, Json>,
        file: &str,
        key: &str,
    ) -> BTreeSet<String> {
        let Some(values) = self.array(root, file, key, None) else {
            return BTreeSet::new();
        };
        if values.len() > MAX_ROWS {
            self.finding(
                file,
                None,
                "input_too_large",
                format!("$.{key} exceeds row bound"),
            );
            return BTreeSet::new();
        }
        let mut result = BTreeSet::new();
        for (index, value) in values.iter().enumerate() {
            match value.as_str() {
                Some(value) if !value.trim().is_empty() => {
                    if !result.insert(value.to_owned()) {
                        self.finding(
                            file,
                            Some(value),
                            "duplicate_id",
                            format!("$.{key}[{index}] repeats value"),
                        );
                    }
                }
                Some(_) => self.finding(
                    file,
                    None,
                    "missing_field",
                    format!("$.{key}[{index}] must not be empty"),
                ),
                None => self.finding(
                    file,
                    None,
                    "invalid_type",
                    format!("$.{key}[{index}] must be a string"),
                ),
            }
        }
        result
    }

    fn system_map(
        &mut self,
        map: &BTreeMap<String, Json>,
        invariant_ids: &BTreeSet<String>,
        roadmap_ids: &BTreeSet<String>,
        vocabulary: Option<&Vocabulary>,
        plan: &str,
    ) {
        let plan_keys = match concordance::plan_heading_keys(plan) {
            Ok(keys) => keys,
            Err(report) => {
                for finding in report.missing.iter().chain(&report.dangling) {
                    self.finding(
                        PLAN_FILE,
                        Some(&finding.id),
                        "invalid_plan_heading",
                        format!("{}: {}", finding.code, finding.detail),
                    );
                }
                BTreeSet::new()
            }
        };
        let checker_packet = self.string(map, SYSTEM_MAP_FILE, "checker_packet", None);
        if let Some(packet) = checker_packet
            && !roadmap_ids.contains(packet)
        {
            self.finding(
                SYSTEM_MAP_FILE,
                Some(packet),
                "unknown_reference",
                "$.checker_packet is not in roadmap packets",
            );
        }
        if let Some(sections) = self.array(map, SYSTEM_MAP_FILE, "normative_sections", None) {
            if sections.is_empty() {
                self.finding(
                    SYSTEM_MAP_FILE,
                    None,
                    "missing_field",
                    "$.normative_sections must not be empty",
                );
            }
            if sections.len() > MAX_ROWS {
                self.finding(
                    SYSTEM_MAP_FILE,
                    None,
                    "input_too_large",
                    "$.normative_sections exceeds row bound",
                );
            } else {
                for section in sections {
                    match section.as_str() {
                        Some(section) if plan_keys.contains(section) => {}
                        Some(section) => self.finding(
                            SYSTEM_MAP_FILE,
                            Some(section),
                            "unknown_reference",
                            "normative section is not a canonical plan heading",
                        ),
                        None => self.finding(
                            SYSTEM_MAP_FILE,
                            None,
                            "invalid_type",
                            "$.normative_sections must contain strings",
                        ),
                    }
                }
            }
        }
        let Some(layers) = self.array(map, SYSTEM_MAP_FILE, "layers", None) else {
            return;
        };
        if layers.len() > MAX_ROWS {
            self.finding(
                SYSTEM_MAP_FILE,
                None,
                "input_too_large",
                "$.layers exceeds row bound",
            );
            return;
        }
        self.report.counts.layers = layers.len();
        let mut ids = BTreeSet::new();
        let mut object_layers = BTreeMap::new();
        for (index, layer) in layers.iter().enumerate() {
            let Some(layer) =
                self.object(layer, SYSTEM_MAP_FILE, &format!("$.layers[{index}]"), None)
            else {
                continue;
            };
            let Some(id) = self.string(layer, SYSTEM_MAP_FILE, "id", None) else {
                continue;
            };
            if !matches!(
                id,
                "L0" | "L1" | "L2" | "L3" | "L4" | "L5" | "L6" | "L7" | "L8"
            ) {
                self.finding(
                    SYSTEM_MAP_FILE,
                    Some(id),
                    "invalid_id",
                    "layer id must be one of L0 through L8",
                );
            }
            if !ids.insert(id.to_owned()) {
                self.finding(
                    SYSTEM_MAP_FILE,
                    Some(id),
                    "duplicate_id",
                    "$.layers repeats layer id",
                );
            }
            for field in ["name", "question", "conformance_suite"] {
                self.string(layer, SYSTEM_MAP_FILE, field, Some(id));
            }
            for field in [
                "objects",
                "verbs",
                "plan_sections",
                "boundary_invariants",
                "packets",
            ] {
                let Some(values) = self.array(layer, SYSTEM_MAP_FILE, field, Some(id)) else {
                    continue;
                };
                if values.is_empty() {
                    self.finding(
                        SYSTEM_MAP_FILE,
                        Some(id),
                        "missing_field",
                        format!("$.layers[{index}].{field} must not be empty"),
                    );
                    continue;
                }
                if values.len() > MAX_ROWS {
                    self.finding(
                        SYSTEM_MAP_FILE,
                        Some(id),
                        "input_too_large",
                        format!("$.layers[{index}].{field} exceeds row bound"),
                    );
                    continue;
                }
                for value in values {
                    let Some(value) = value.as_str() else {
                        self.finding(
                            SYSTEM_MAP_FILE,
                            Some(id),
                            "invalid_type",
                            format!("$.layers[{index}].{field} must contain strings"),
                        );
                        continue;
                    };
                    if value.trim().is_empty() {
                        self.finding(
                            SYSTEM_MAP_FILE,
                            Some(id),
                            "missing_field",
                            format!("$.layers[{index}].{field} contains an empty string"),
                        );
                        continue;
                    }
                    if field == "objects"
                        && let Some(previous) =
                            object_layers.insert(value.to_owned(), id.to_owned())
                    {
                        self.finding(
                            SYSTEM_MAP_FILE,
                            Some(value),
                            "duplicate_object",
                            format!("object is listed in both {previous} and {id}"),
                        );
                    }
                    match field {
                        "plan_sections" if !plan_keys.contains(value) => self.finding(
                            SYSTEM_MAP_FILE,
                            Some(value),
                            "unknown_reference",
                            format!("layer {id} references unknown plan section"),
                        ),
                        "boundary_invariants" if !invariant_ids.contains(value) => self.finding(
                            SYSTEM_MAP_FILE,
                            Some(value),
                            "unknown_reference",
                            format!("layer {id} references unknown invariant"),
                        ),
                        "packets" if !roadmap_ids.contains(value) => self.finding(
                            SYSTEM_MAP_FILE,
                            Some(value),
                            "unknown_reference",
                            format!("layer {id} references unknown roadmap packet"),
                        ),
                        // Map membership is cross-layer access.  It is deliberately not compared
                        // to vocabulary ownership/range declarations.
                        "verbs"
                            if vocabulary
                                .is_some_and(|vocabulary| !vocabulary.verbs.contains(value)) =>
                        {
                            self.finding(
                                SYSTEM_MAP_FILE,
                                Some(value),
                                "unregistered_verb",
                                format!("layer {id} cross-access verb is not registered"),
                            )
                        }
                        _ => {}
                    }
                }
            }
        }
        for required in ["L0", "L1", "L2", "L3", "L4", "L5", "L6", "L7", "L8"] {
            if !ids.contains(required) {
                self.finding(
                    SYSTEM_MAP_FILE,
                    Some(required),
                    "missing_field",
                    "required layer is absent",
                );
            }
        }
    }

    fn commands(&mut self, file: &str, text: &str, vocabulary: &Vocabulary) {
        let commands = command_tokens(text);
        if file == README_FILE {
            self.report.counts.readme_commands = commands.len();
        } else {
            self.report.counts.guide_commands = commands.len();
        }
        for command in commands {
            if vocabulary.retired.contains(&command) {
                self.finding(
                    file,
                    Some(&command),
                    "retired_verb",
                    "command is retired in vocabulary.json",
                );
            } else if !vocabulary.verbs.contains(&command) {
                self.finding(
                    file,
                    Some(&command),
                    "unregistered_verb",
                    "command is not registered in vocabulary.json",
                );
            }
        }
    }

    fn addresses(&mut self, file: &str, text: &str, vocabulary: &Vocabulary) {
        for address in address_tokens(text) {
            let mut parts = address.split('/');
            let Some(tenant) = parts.next() else { continue };
            let Some(kind) = parts.next() else { continue };
            let Some(id) = parts.next() else { continue };
            if tenant.is_empty()
                || kind.is_empty()
                || id.is_empty()
                || address.contains('<')
                || address.contains('…')
            {
                continue;
            }
            if !vocabulary.nouns.contains(kind) {
                self.finding(
                    file,
                    Some(kind),
                    "unknown_noun",
                    format!("address {address} uses an unregistered kind"),
                );
            }
        }
    }

    fn knowledge_registry(&mut self, map: &BTreeMap<String, Json>, vocabulary: &Vocabulary) {
        let map_variants = map
            .get("epistemic_type")
            .and_then(Json::as_object)
            .map(|epistemic| self.string_set(epistemic, SYSTEM_MAP_FILE, "variants"))
            .unwrap_or_default();
        let required = constitutional_variants();
        if map_variants != required {
            self.finding(
                SYSTEM_MAP_FILE,
                None,
                "invalid_knowledge_variants",
                "epistemic_type.variants must be exactly the six constitutional variants",
            );
        }
        if vocabulary.knowledge != required {
            self.finding(
                VOCABULARY_FILE,
                None,
                "invalid_knowledge_variants",
                "knowledge_variants must be exactly the six constitutional variants",
            );
        }
        if map_variants != vocabulary.knowledge {
            self.finding(
                SYSTEM_MAP_FILE,
                None,
                "unknown_reference",
                "epistemic_type.variants must equal vocabulary knowledge_variants",
            );
        }
    }

    fn knowledge(&mut self, vocabulary: &Vocabulary, file: &str, text: &str) {
        for variant in knowledge_tokens(text) {
            if !vocabulary.knowledge.contains(&variant) {
                self.finding(
                    file,
                    Some(&variant),
                    "unknown_knowledge_variant",
                    "named Knowledge variant is not registered",
                );
            }
        }
    }

    fn reason_codes(&mut self, vocabulary: &Vocabulary, plan: &str) {
        let visible = concordance::unfenced_markdown_lines(plan);
        let starts: Vec<_> = visible
            .iter()
            .enumerate()
            .filter_map(|(index, line)| {
                concordance::markdown_line(line)
                    .is_some_and(|line| line.starts_with("### 17.3 "))
                    .then_some(index)
            })
            .collect();
        if starts.len() != 1 {
            self.finding(
                PLAN_FILE,
                Some("§17.3"),
                "unknown_reference",
                format!(
                    "expected one unfenced level-three §17.3 heading, found {}",
                    starts.len()
                ),
            );
            return;
        }
        let start = starts[0] + 1;
        let end = visible[start..]
            .iter()
            .position(|line| {
                concordance::markdown_line(line).is_some_and(|line| line.starts_with("### "))
            })
            .map_or(visible.len(), |offset| start + offset);
        let section = visible[start..end].join("\n");
        let Some(errors) = section.find("Errors are the same envelope") else {
            self.finding(
                PLAN_FILE,
                Some("§17.3"),
                "missing_field",
                "reason-code paragraph is absent",
            );
            return;
        };
        let paragraph = section[errors..].split("\n\n").next().unwrap_or_default();
        let codes: Vec<_> = inline_code_tokens(paragraph)
            .into_iter()
            .filter(|token| is_pascal_token(token))
            .collect();
        let mut unique = BTreeSet::new();
        if codes.is_empty() {
            self.finding(
                PLAN_FILE,
                Some("§17.3"),
                "missing_reason_code",
                "reason-code list is empty",
            );
        }
        for code in codes {
            if !unique.insert(code.clone()) {
                self.finding(
                    PLAN_FILE,
                    Some(&code),
                    "duplicate_id",
                    "§17.3 repeats a reason code",
                );
            }
            if !vocabulary.reason_codes.contains(&code) {
                self.finding(
                    PLAN_FILE,
                    Some(&code),
                    "missing_reason_code",
                    "§17.3 reason code is absent from vocabulary.json",
                );
            }
        }
    }

    fn bead_packet(&mut self, map: &BTreeMap<String, Json>, issues: &str) {
        let Some(packet) = self.string(map, SYSTEM_MAP_FILE, "checker_packet", None) else {
            return;
        };
        let expected = format!("- Roadmap packet: {packet}");
        let mut matches = 0usize;
        for (line_number, line) in issues.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let value = match parse(line.as_bytes(), Limits::default()) {
                Ok(value) => value,
                Err(error) => {
                    self.finding(
                        BEADS_FILE,
                        None,
                        "invalid_json",
                        format!("line {}: {error}", line_number + 1),
                    );
                    continue;
                }
            };
            let Some(issue) = self.object(
                &value,
                BEADS_FILE,
                &format!("line {}", line_number + 1),
                None,
            ) else {
                continue;
            };
            if is_tombstone(issue) {
                continue;
            }
            let Some(description) = issue.get("description").and_then(Json::as_str) else {
                continue;
            };
            // This verifies only the FA-132 checker packet link.  It does not
            // claim comprehensive Bead/roadmap coverage.
            matches += description
                .lines()
                .filter(|line| line.trim() == expected)
                .count();
        }
        match matches {
            0 => self.finding(
                BEADS_FILE,
                Some(packet),
                "missing_bead_packet_reference",
                format!("no live issue description contains exactly {expected:?}"),
            ),
            1 => {}
            _ => self.finding(
                BEADS_FILE,
                Some(packet),
                "duplicate_bead_packet_reference",
                format!("{matches} live issue description lines equal {expected:?}"),
            ),
        }
    }
}

fn is_tombstone(issue: &BTreeMap<String, Json>) -> bool {
    issue
        .get("tombstone")
        .and_then(Json::as_bool)
        .unwrap_or(false)
        || issue
            .get("deleted")
            .and_then(Json::as_bool)
            .unwrap_or(false)
        || issue.get("op").and_then(Json::as_str) == Some("delete")
}

fn render_findings<'a>(output: &mut String, findings: impl Iterator<Item = &'a Finding>) {
    for (index, finding) in findings.enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"file\":");
        json_string(output, &finding.file);
        output.push_str(",\"id\":");
        match &finding.id {
            Some(id) => json_string(output, id),
            None => output.push_str("null"),
        }
        output.push_str(",\"code\":");
        json_string(output, &finding.code);
        output.push_str(",\"detail\":");
        json_string(output, &finding.detail);
        output.push('}');
    }
}

fn json_string(output: &mut String, value: &str) {
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                use std::fmt::Write;
                let _ = write!(output, "\\u{:04x}", character as u32);
            }
            character => output.push(character),
        }
    }
    output.push('"');
}

fn command_tokens(text: &str) -> Vec<String> {
    let mut commands = Vec::new();
    let mut fence = None;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if let Some(opening) = fence {
            if closes_fence(trimmed, opening) {
                fence = None;
            } else {
                commands.extend(commands_in(trimmed));
            }
            continue;
        }
        if let Some(opening) = opening_fence(trimmed) {
            fence = Some(opening);
            continue;
        }
        if trimmed.starts_with('|') {
            commands.extend(commands_in(trimmed));
        } else {
            commands.extend(
                inline_code_tokens(trimmed)
                    .into_iter()
                    .flat_map(|span| commands_in(&span)),
            );
        }
    }
    commands
}

fn commands_in(text: &str) -> Vec<String> {
    let mut commands = Vec::new();
    let mut rest = text;
    while let Some(position) = rest.find("fa ") {
        if position != 0
            && rest[..position]
                .chars()
                .next_back()
                .is_some_and(is_command_identifier)
        {
            rest = &rest[position + 2..];
            continue;
        }
        rest = &rest[position + 3..];
        let token: String = rest
            .chars()
            .take_while(|character| {
                character.is_ascii_alphanumeric() || *character == '-' || *character == '|'
            })
            .collect();
        // `fa <verb>` is the literal grammar template.  `fa verb` is an
        // actual command spelling and must be checked like every other name.
        if token.is_empty() {
            continue;
        }
        commands.extend(
            token
                .split('|')
                .filter(|part| !part.is_empty())
                .map(str::to_owned),
        );
    }
    commands
}

fn is_command_identifier(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
}

fn inline_code_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('`') {
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('`') else {
            break;
        };
        tokens.push(after_start[..end].to_owned());
        rest = &after_start[end + 1..];
    }
    tokens
}

fn address_tokens(text: &str) -> Vec<&str> {
    let mut addresses = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("fa://") {
        let candidate = &rest[start + 5..];
        let length = candidate
            .chars()
            .take_while(|character| {
                !character.is_whitespace()
                    && !matches!(character, '`' | ')' | ']' | '}' | ',' | ';' | '"')
            })
            .map(char::len_utf8)
            .sum();
        addresses.push(&candidate[..length]);
        rest = &candidate[length..];
    }
    addresses
}

fn knowledge_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("Knowledge::") {
        let candidate = &rest[start + "Knowledge::".len()..];
        let token: String = candidate
            .chars()
            .take_while(|character| character.is_ascii_alphanumeric())
            .collect();
        let token_len = token.len();
        if is_pascal_token(&token) {
            tokens.push(token);
        }
        rest = &candidate[token_len..];
    }
    let bytes = text.as_bytes();
    for index in 0..bytes.len() {
        if bytes[index] != b'{' || index == 0 {
            continue;
        }
        let before = &text[..index];
        let token = before
            .rsplit(|character: char| !character.is_ascii_alphanumeric())
            .next()
            .unwrap_or_default();
        if is_pascal_token(token) && is_bare_knowledge_constructor(text, index, token) {
            tokens.push(token.to_owned());
        }
    }
    for line in text.lines() {
        let code_tokens: Vec<_> = inline_code_tokens(line)
            .into_iter()
            .filter(|token| is_pascal_token(token))
            .collect();
        // A code span is a variant only in an explicit variant list.  This
        // accepts ordinary payload/domain words and still exposes a mutated
        // member of a list that otherwise contains registered variants.
        if line.to_ascii_lowercase().contains("knowledge variants:") {
            tokens.extend(code_tokens);
        }
    }
    tokens
}

fn is_bare_knowledge_constructor(text: &str, brace_index: usize, token: &str) -> bool {
    if constitutional_variants().contains(token) {
        return true;
    }
    let line_start = text[..brace_index].rfind('\n').map_or(0, |index| index + 1);
    let line_end = text[brace_index..]
        .find('\n')
        .map_or(text.len(), |offset| brace_index + offset);
    text[line_start..line_end]
        .to_ascii_lowercase()
        .contains("knowledge variant")
}

fn is_pascal_token(token: &str) -> bool {
    token.len() > 1
        && token
            .as_bytes()
            .first()
            .is_some_and(|byte| byte.is_ascii_uppercase())
        && token.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn valid_layer_descriptor(value: &str) -> bool {
    !value.is_empty()
        && value
            .split('/')
            .all(|segment| match segment.split_once('-') {
                Some((start, end)) => layer_number(start)
                    .zip(layer_number(end))
                    .is_some_and(|(start, end)| start <= end),
                None => layer_number(segment).is_some(),
            })
}

fn layer_number(value: &str) -> Option<u8> {
    match value.as_bytes() {
        [b'L', digit @ b'0'..=b'8'] => Some(digit - b'0'),
        _ => None,
    }
}

fn constitutional_variants() -> BTreeSet<String> {
    ["Known", "Pending", "Unknown", "Withheld", "Stale", "Absent"]
        .into_iter()
        .map(str::to_owned)
        .collect()
}
