//! Bounded JSON rendering; no inferred permissions or refreshed observations.
use crate::config::debug;
use fa_reference::action::consequence::{
    Consequence,
    delivery::persistent::FileDeliverySnapshot,
    gate::containment::session::policy::{Evaluation, Policy, Predicate, Truth},
    policy_campaign::{PolicyDelta, PolicyReplayReport, ReplayCaseId, ReplayLimits},
};
use fa_reference::action::{ActionSpec, Purpose, ResolvedTarget};
use std::io::{self, Write};

pub(super) fn encode(
    cut: &FileDeliverySnapshot,
    report: &PolicyReplayReport,
    limits: ReplayLimits,
    maximum: usize,
) -> Result<Vec<u8>, String> {
    let mut out = Bounded {
        bytes: Vec::new(),
        maximum,
    };
    document(&mut out, cut, report, limits, maximum).map_err(debug)?;
    Ok(out.bytes)
}

fn document(
    out: &mut impl Write,
    cut: &FileDeliverySnapshot,
    report: &PolicyReplayReport,
    limits: ReplayLimits,
    maximum: usize,
) -> io::Result<()> {
    out.write_all(concat!(
        "{\"schema_version\":\"fa.policy-replay/1\",\"knowledge\":{\"kind\":\"known\",",
        "\"basis\":{\"class\":\"bounded_model\",\"scope\":\"retained_exact_policy_corpus\",",
        "\"provenance\":\"operator_trusted_local_journal\",\"cut\":\"single_canonical_image\"},",
        "\"value\":{\"operation\":\"policy-replay\",\"historical\":true,",
        "\"replay_grade\":\"decision_replay\",\"authority\":\"not_issued\",",
        "\"policy_changed\":false,\"helpers_rerun\":false,\"live_evidence_read\":false,"
    ).as_bytes())?;
    write!(
        out,
        "\"cut\":{{\"journal_revision\":{},\"control_sequence\":{},\"authority_epoch\":{},\"executions\":{},\"charged\":{},\"reserved\":{}}},",
        cut.revision,
        cut.control.sequence,
        cut.control.ledger.epoch,
        cut.executions,
        cut.control.ledger.charged,
        cut.control.ledger.reserved
    )?;
    write!(
        out,
        "\"limits\":{{\"cases\":{},\"input_bytes\":{},\"output_bytes\":{}}},",
        limits.cases, limits.input_bytes, maximum
    )?;
    out.write_all(b"\"previous_policy\":")?;
    policy(out, report.previous_policy())?;
    out.write_all(b",\"candidate_policy\":")?;
    policy(out, report.candidate_policy())?;
    write!(
        out,
        ",\"case_count\":{},\"input_bytes\":{},\"requires_shadow\":{},\"newly_reviewable\":",
        report.cases().len(),
        report.input_bytes(),
        report.requires_shadow()
    )?;
    ids(out, &report.newly_reviewable())?;
    out.write_all(b",\"newly_blocked\":")?;
    ids(out, &report.newly_blocked())?;
    out.write_all(b",\"cases\":[")?;
    for (index, case) in report.cases().iter().enumerate() {
        comma(out, index)?;
        out.write_all(b"{\"id\":")?;
        id(out, case.id())?;
        write!(
            out,
            ",\"delta\":\"{}\",\"snapshot_semantic_epoch\":{},\"action\":",
            delta(case.delta()),
            case.snapshot_semantic_epoch()
        )?;
        action(out, case.action().spec())?;
        out.write_all(b",\"original\":")?;
        evaluation(out, case.original())?;
        out.write_all(b",\"original_consequence\":")?;
        match case.original_consequence() {
            Some(value) => write!(out, "\"{}\"", consequence(value))?,
            None => out.write_all(b"null")?,
        }
        out.write_all(b",\"candidate\":")?;
        match case.candidate() {
            Some(value) => evaluation(out, value)?,
            None => {
                out.write_all(b"{\"kind\":\"unknown\",\"reason_code\":\"MissingRequiredObservation\",\"missing_nodes\":")?;
                indices(out, case.missing_nodes())?;
                out.write_all(b"}")?;
            }
        }
        out.write_all(b"}")?;
    }
    out.write_all(b"]}}}\n")
}

fn policy(out: &mut impl Write, policy: &Policy) -> io::Result<()> {
    write!(out, "{{\"generation\":{},\"nodes\":[", policy.generation())?;
    for (index, node) in policy.nodes().iter().enumerate() {
        comma(out, index)?;
        match node {
            Predicate::TargetIs(value) => {
                out.write_all(b"{\"op\":\"target_is\",\"target\":")?;
                target(out, *value)?;
                out.write_all(b"}")?;
            }
            Predicate::PayloadIs(value) => {
                out.write_all(b"{\"op\":\"payload_is\",\"value_hex\":")?;
                hex(out, value)?;
                out.write_all(b"}")?;
            }
            Predicate::PayloadAtMost(maximum) => {
                write!(out, "{{\"op\":\"payload_at_most\",\"maximum\":{maximum}}}")?
            }
            Predicate::UnitsAtMost(maximum) => {
                write!(out, "{{\"op\":\"units_at_most\",\"maximum\":{maximum}}}")?
            }
            Predicate::ExactValue { key, value } => {
                write!(out, "{{\"op\":\"exact_value\",\"key\":{key},\"value_hex\":")?;
                hex(out, value)?;
                out.write_all(b"}")?;
            }
            Predicate::Absent { key } => write!(out, "{{\"op\":\"absent\",\"key\":{key}}}")?,
            Predicate::EmptyRange { start, end } => write!(
                out,
                "{{\"op\":\"empty_range\",\"start\":{start},\"end\":{end}}}"
            )?,
            Predicate::All(children) | Predicate::Any(children) => {
                let op = if matches!(node, Predicate::All(_)) {
                    "all"
                } else {
                    "any"
                };
                write!(out, "{{\"op\":\"{op}\",\"children\":")?;
                indices(out, children)?;
                out.write_all(b"}")?;
            }
            Predicate::Not(child) => write!(out, "{{\"op\":\"not\",\"child\":{child}}}")?,
        }
    }
    out.write_all(b"]}")
}
fn action(out: &mut impl Write, action: &ActionSpec) -> io::Result<()> {
    let scope = action.scope;
    let purpose = match scope.purpose {
        Purpose::Effect => "effect",
        Purpose::Experiment => "experiment",
    };
    write!(
        out,
        "{{\"version\":{},\"scope\":{{\"tenant\":{},\"principal\":{},\"run\":{},\"branch\":{},\"authority\":{},\"purpose\":\"{purpose}\"}},\"target\":",
        action.version, scope.tenant, scope.principal, scope.run, scope.branch, scope.authority
    )?;
    match action.target {
        Some(value) => target(out, value)?,
        None => out.write_all(b"null")?,
    }
    out.write_all(b",\"payload_hex\":")?;
    hex(out, &action.payload)?;
    write!(
        out,
        ",\"units\":{},\"policy_epoch\":{},\"deadline\":{}}}",
        action.units, action.policy_epoch, action.deadline.0
    )
}
fn target(out: &mut impl Write, target: ResolvedTarget) -> io::Result<()> {
    write!(
        out,
        "{{\"adapter\":{},\"object\":{},\"contract_version\":{},\"expected_version\":{},\"generation\":{}}}",
        target.adapter,
        target.object,
        target.contract_version,
        target.expected_version,
        target.generation
    )
}
fn evaluation(out: &mut impl Write, value: &Evaluation) -> io::Result<()> {
    write!(
        out,
        "{{\"kind\":\"known\",\"basis\":\"retained_original_observations\",\"result\":\"{}\",\"trace\":[",
        truth(value.result())
    )?;
    for (index, step) in value.trace().iter().enumerate() {
        comma(out, index)?;
        write!(
            out,
            "{{\"node\":{},\"result\":\"{}\"}}",
            step.node,
            truth(step.result)
        )?;
    }
    out.write_all(b"]}")
}
fn ids(out: &mut impl Write, values: &[ReplayCaseId]) -> io::Result<()> {
    out.write_all(b"[")?;
    for (index, value) in values.iter().enumerate() {
        comma(out, index)?;
        id(out, *value)?;
    }
    out.write_all(b"]")
}
fn id(out: &mut impl Write, value: ReplayCaseId) -> io::Result<()> {
    match value {
        ReplayCaseId::Proposal(attempt) => {
            write!(out, "{{\"kind\":\"proposal\",\"attempt\":{attempt}}}")
        }
        ReplayCaseId::Review {
            attempt,
            round,
            control_sequence,
        } => write!(
            out,
            "{{\"kind\":\"review\",\"attempt\":{attempt},\"round\":{round},\"control_sequence\":{control_sequence}}}"
        ),
    }
}
fn indices(out: &mut impl Write, values: &[usize]) -> io::Result<()> {
    out.write_all(b"[")?;
    for (index, value) in values.iter().enumerate() {
        comma(out, index)?;
        write!(out, "{value}")?;
    }
    out.write_all(b"]")
}
fn hex(out: &mut impl Write, bytes: &[u8]) -> io::Result<()> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    out.write_all(b"\"")?;
    for byte in bytes {
        out.write_all(&[HEX[(byte >> 4) as usize], HEX[(byte & 15) as usize]])?;
    }
    out.write_all(b"\"")
}
fn comma(out: &mut impl Write, index: usize) -> io::Result<()> {
    if index > 0 {
        out.write_all(b",")
    } else {
        Ok(())
    }
}
fn truth(value: Truth) -> &'static str {
    match value {
        Truth::Satisfied => "satisfied",
        Truth::Violated => "violated",
        Truth::Unknown => "unknown",
    }
}
fn delta(value: PolicyDelta) -> &'static str {
    match value {
        PolicyDelta::Unchanged => "unchanged",
        PolicyDelta::NewlyBlocked => "newly_blocked",
        PolicyDelta::NewlyReviewable => "newly_reviewable",
        PolicyDelta::RequiresShadow => "requires_shadow",
    }
}
fn consequence(value: Consequence) -> &'static str {
    match value {
        Consequence::Continue => "continue",
        Consequence::HoldEffect => "hold_effect",
        Consequence::Deny => "deny",
        Consequence::NarrowAuthority => "narrow_authority",
        Consequence::SuspendRun => "suspend_run",
        Consequence::ResetToCheckpoint => "reset_to_checkpoint",
    }
}

struct Bounded {
    bytes: Vec<u8>,
    maximum: usize,
}
impl Write for Bounded {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.maximum.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other(
                "policy replay output_bytes exhausted; no report was emitted",
            ));
        }
        self.bytes
            .try_reserve(bytes.len())
            .map_err(|_| io::Error::other("policy replay output allocation failed"))?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
