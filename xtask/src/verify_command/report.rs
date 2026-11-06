//! Deterministic JSON for the bounded profile. No complete Tower envelope or
//! global clock is synthesized from a native FARJ head.

use std::fmt::Write as _;

use xtask::independent_verify::{Accounting, AssuranceStatus, ExpectedContext, Head};

use super::{Failure, Verified, context};

const SCHEMA: &str = "fa.rights_verification_result/1";

fn quoted(value: &str) -> String {
    let mut output = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character <= '\u{001f}' => {
                write!(&mut output, "\\u{:04x}", character as u32)
                    .expect("writing to a String cannot fail");
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

fn object<const N: usize>(entries: [(&str, String); N]) -> String {
    let mut output = String::from("{");
    for (index, (key, value)) in entries.into_iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str(&quoted(key));
        output.push(':');
        output.push_str(&value);
    }
    output.push('}');
    output
}

fn known(type_name: &str, value: String, basis: String) -> String {
    object([
        ("type", quoted(type_name)),
        ("state", quoted("Known")),
        ("value", value),
        ("basis", basis),
    ])
}

fn unknown(type_name: &str, reason: &str) -> String {
    object([
        ("type", quoted(type_name)),
        ("state", quoted("Unknown")),
        ("reason", quoted(reason)),
    ])
}

fn predicate(scope: &str) -> String {
    known(
        "ScopedVerificationPredicate",
        String::from("true"),
        object([
            ("claim_class", quoted("BoundedModel")),
            ("scope", quoted(scope)),
            ("checker", quoted("xtask::independent_verify")),
        ]),
    )
}

fn basis() -> String {
    object([
        ("claim_class", quoted("BoundedModel")),
        (
            "scope",
            quoted(
                "entire supplied FARJ byte image and supplied exact prefix under the explicitly selected rights-accounting profile",
            ),
        ),
        ("checker", quoted("xtask::independent_verify")),
        (
            "assumptions",
            quoted(
                "caller selects meaningful expected context and supplies stable files during this read",
            ),
        ),
        (
            "invalidation_conditions",
            quoted(
                "different input bytes, selected context or accounting semantics require a new verification",
            ),
        ),
        (
            "authentic_origin",
            unknown(
                "ScopedVerificationPredicate",
                "FARJ CRCs authenticate neither the producer nor the supplied anchor",
            ),
        ),
        (
            "complete_for_contract",
            predicate(
                "all supplied bytes were checked; completeness of unseen history is not established",
            ),
        ),
        (
            "semantically_valid",
            predicate(
                "the supplied image satisfies the explicitly selected bounded FARJ accounting law",
            ),
        ),
        (
            "available_for_replay",
            unknown(
                "ScopedVerificationPredicate",
                "this read acquires no retention lease",
            ),
        ),
        (
            "anchor_custody",
            unknown(
                "ScopedVerificationPredicate",
                "distinct files do not establish independent custody or freshness",
            ),
        ),
        (
            "global_generation",
            unknown("TowerGeneration", "global Tower state was not observed"),
        ),
        (
            "global_control_sequence",
            unknown(
                "TowerControlSequence",
                "native FARJ sequences and epochs are not global Tower clocks",
            ),
        ),
    ])
}

fn hex(identity: &[u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in identity {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn selected_context(expected: ExpectedContext) -> String {
    object([
        ("schema_version", quoted(context::SCHEMA)),
        ("run", quoted(&hex(&expected.run))),
        ("policy", quoted(&hex(&expected.policy))),
        ("total_units", expected.total_units.to_string()),
        (
            "limits",
            object([
                (
                    "max_frame_bytes",
                    expected.limits.max_frame_bytes.to_string(),
                ),
                ("max_records", expected.limits.max_records.to_string()),
                (
                    "max_total_bytes",
                    expected.limits.max_total_bytes.to_string(),
                ),
            ]),
        ),
        (
            "recovery",
            match expected.recovery {
                None => String::from("null"),
                Some(reserve) => object([
                    ("records", reserve.records.to_string()),
                    ("bytes", reserve.bytes.to_string()),
                ]),
            },
        ),
    ])
}

fn head(head: Head, bytes: u64) -> String {
    object([
        (
            "head",
            object([
                ("sequence", head.sequence.to_string()),
                ("checksum", head.checksum.to_string()),
            ]),
        ),
        ("bytes", bytes.to_string()),
    ])
}

fn accounting(value: Accounting) -> String {
    object([
        ("total_units", value.total_units.to_string()),
        ("available_units", value.available_units.to_string()),
        ("spent_units", value.spent_units.to_string()),
        ("reserved_units", value.reserved_units.to_string()),
        ("dispatched_units", value.dispatched_units.to_string()),
        ("unknown_units", value.unknown_units.to_string()),
        ("epoch", value.epoch.to_string()),
        ("incident_count", value.incident_count.to_string()),
        ("reservations", value.reservations.to_string()),
        ("reserved_count", value.reserved_count.to_string()),
        ("dispatched_count", value.dispatched_count.to_string()),
        ("unknown_count", value.unknown_count.to_string()),
        ("committed_count", value.committed_count.to_string()),
        ("aborted_count", value.aborted_count.to_string()),
    ])
}

pub(super) fn success(verified: &Verified) -> String {
    let report = &verified.report;
    let assurances = report
        .assurances()
        .iter()
        .map(|assurance| {
            let knowledge = match assurance.status {
                AssuranceStatus::Known => {
                    known("VerificationProperty", String::from("true"), basis())
                }
                AssuranceStatus::Unknown => unknown("VerificationProperty", assurance.explanation),
            };
            object([
                ("property", quoted(assurance.property)),
                ("knowledge", knowledge),
                ("explanation", quoted(assurance.explanation)),
            ])
        })
        .collect::<Vec<_>>()
        .join(",");
    object([
        ("schema_version", quoted(SCHEMA)),
        ("profile", quoted("rights-journal")),
        ("status", quoted("verified")),
        ("reason_code", quoted("bounded_profile_verified")),
        (
            "context",
            known(
                "RightsVerificationContext",
                selected_context(verified.expected),
                object([
                    ("claim_class", quoted("Invariant")),
                    (
                        "scope",
                        quoted(
                            "explicit parsed operator parameters; meaningful selection and provenance are not established",
                        ),
                    ),
                    (
                        "authentic_origin",
                        unknown(
                            "ScopedVerificationPredicate",
                            "operator input is not authenticated",
                        ),
                    ),
                ]),
            ),
        ),
        (
            "result",
            known(
                "RightsVerificationReport",
                object([
                    ("format_version", report.format_version.to_string()),
                    ("prefix", head(report.prefix_head, report.prefix_bytes)),
                    ("final", head(report.final_head, report.final_bytes)),
                    ("accounting", accounting(report.accounting)),
                ]),
                basis(),
            ),
        ),
        (
            "assurances",
            known("VerificationAssurances", format!("[{assurances}]"), basis()),
        ),
        ("error", String::from("null")),
    ])
}

pub(super) fn failure(error: &Failure) -> String {
    object([
        ("schema_version", quoted(SCHEMA)),
        ("profile", quoted("rights-journal")),
        ("status", quoted("rejected")),
        ("reason_code", quoted(error.reason_code)),
        (
            "context",
            unknown(
                "RightsVerificationContext",
                "no successful verification report was produced",
            ),
        ),
        (
            "result",
            unknown(
                "RightsVerificationReport",
                "verification did not complete; no partial accounting is exported",
            ),
        ),
        (
            "assurances",
            unknown("VerificationAssurances", "verification did not complete"),
        ),
        (
            "error",
            object([
                ("input", quoted(error.input)),
                (
                    "field",
                    error.field.map_or_else(|| String::from("null"), quoted),
                ),
                (
                    "byte_offset",
                    error
                        .byte_offset
                        .map_or_else(|| String::from("null"), |value| value.to_string()),
                ),
                (
                    "record_sequence",
                    error
                        .sequence
                        .map_or_else(|| String::from("null"), |value| value.to_string()),
                ),
                (
                    "io_kind",
                    error
                        .io_kind
                        .map_or_else(|| String::from("null"), |kind| quoted(&format!("{kind:?}"))),
                ),
                ("detail", quoted(&error.detail)),
            ]),
        ),
    ])
}
