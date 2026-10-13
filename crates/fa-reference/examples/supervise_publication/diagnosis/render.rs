//! Deterministic predicate-tree output, never a prose-derived explanation.
//! The operator may see policy literals; actor/helper APIs do not expose this.
use fa_reference::action::consequence::delivery::persistent::observed::guarded::investigation::FileProposalDiagnosis;
use fa_reference::action::consequence::experiment::NextRequirement;
use fa_reference::action::consequence::gate::containment::session::policy::{Predicate, Truth};
use std::io::{self, Write};

pub(super) fn encode(diagnosis: &FileProposalDiagnosis) -> Result<Vec<u8>, String> {
    encode_inner(diagnosis).map_err(|error| format!("encode proposal explanation: {error}"))
}
fn encode_inner(diagnosis: &FileProposalDiagnosis) -> io::Result<Vec<u8>> {
    let origin = diagnosis.origin();
    let proposal = origin.proposal();
    let result = diagnosis.result();
    let captured = origin.journal_snapshot();
    let invalid = || io::Error::new(io::ErrorKind::InvalidData, "incomplete original proposal diagnosis");
    let request = origin.external_request().ok_or_else(invalid)?;
    let ending = captured.control.ledger.stages.get(&proposal.attempt).ok_or_else(invalid)?;
    let nodes = proposal.policy.nodes();
    let trace = proposal.evaluation.trace();
    if nodes.is_empty() || trace.len() != nodes.len() || result.trace().len() != nodes.len()
        || result.attempt() != proposal.attempt { return Err(invalid()); }
    let mut out = Vec::new();
    write!(out, concat!("{{\"schema_version\":\"fa.proposal-diagnosis/1\",",
        "\"purpose\":\"experiment\",\"permission\":\"not_issued\",",
        "\"historical\":true,\"source\":\"captured_journal_image\",",
        "\"request\":\"{}\",\"attempt\":\"{}\",",
        "\"admission_revision\":\"{}\",\"journal_revision\":\"{}\",",
        "\"admission_state\":\"{:?}\",\"ending_state\":\"{:?}\",",
        "\"policy_generation\":\"{}\",\"semantic_epoch\":\"{}\",",
        "\"witness_count\":{},\"intervention\":\"{}\",",
        "\"baseline\":\"{}\",\"counterfactual\":\"{}\",\"next_requirement\":\"{}\",",
        "\"executions_at_capture\":\"{}\",\"charged_units\":\"{}\",\"reserved_units\":\"{}\",",
        "\"predicate_tree\":{{\"root\":{},\"nodes\":["),
        request, proposal.attempt, origin.admission_snapshot().revision, captured.revision,
        proposal.state, ending, proposal.policy.generation(), proposal.snapshot_semantic_epoch,
        proposal.evaluation.witnesses().len(), if result.interventions().is_empty() { "none" } else { "payload" },
        truth(result.baseline()), truth(result.counterfactual()), next(result.next_requirement()),
        captured.executions, captured.control.ledger.charged, captured.control.ledger.reserved, nodes.len() - 1)?;
    for (index, predicate) in nodes.iter().enumerate() {
        if trace[index].node != index { return Err(invalid()); }
        if index != 0 { out.write_all(b",")?; }
        write!(out, "{{\"node\":{index},\"predicate\":")?;
        node(&mut out, predicate)?;
        write!(out, ",\"baseline\":\"{}\",\"counterfactual\":\"{}\",\"witness\":",
            truth(trace[index].result), truth(result.trace()[index]))?;
        match trace[index].witness { Some(witness) => write!(out, "{witness}")?, None => out.write_all(b"null")? }
        out.write_all(b"}")?;
    }
    out.write_all(b"]}}\n")?;
    Ok(out)
}
fn truth(value: Truth) -> &'static str {
    match value { Truth::Satisfied => "satisfied", Truth::Violated => "violated", Truth::Unknown => "unknown" }
}
fn next(value: NextRequirement) -> &'static str {
    match value { NextRequirement::FreshIndependentReview => "fresh_independent_review",
        NextRequirement::ExactPolicyViolation => "exact_policy_violation", NextRequirement::MoreEvidence => "more_evidence" }
}
fn hex(out: &mut impl Write, bytes: &[u8]) -> io::Result<()> {
    for byte in bytes { write!(out, "{byte:02x}")?; }
    Ok(())
}
fn node(out: &mut impl Write, predicate: &Predicate) -> io::Result<()> {
    match predicate {
        Predicate::TargetIs(target) => write!(out, concat!("{{\"op\":\"target_is\",\"adapter\":\"{}\",",
            "\"object\":\"{}\",\"contract_version\":\"{}\",\"expected_version\":\"{}\",\"generation\":\"{}\"}}"),
            target.adapter, target.object, target.contract_version, target.expected_version, target.generation),
        Predicate::PayloadIs(bytes) => {
            out.write_all(b"{\"op\":\"payload_is\",\"value_hex\":\"")?;
            hex(out, bytes)?; out.write_all(b"\"}")
        }
        Predicate::PayloadAtMost(maximum) => write!(out, "{{\"op\":\"payload_at_most\",\"maximum\":{maximum}}}"),
        Predicate::UnitsAtMost(maximum) => write!(out, "{{\"op\":\"units_at_most\",\"maximum\":\"{maximum}\"}}"),
        Predicate::ExactValue { key, value } => {
            write!(out, "{{\"op\":\"exact_value\",\"key\":\"{key}\",\"value_hex\":\"")?;
            hex(out, value)?; out.write_all(b"\"}")
        }
        Predicate::Absent { key } => write!(out, "{{\"op\":\"absent\",\"key\":\"{key}\"}}"),
        Predicate::EmptyRange { start, end } => write!(out, "{{\"op\":\"empty_range\",\"start\":\"{start}\",\"end\":\"{end}\"}}"),
        Predicate::All(children) | Predicate::Any(children) => {
            let op = if matches!(predicate, Predicate::All(_)) { "all" } else { "any" };
            write!(out, "{{\"op\":\"{op}\",\"children\":[")?;
            for (index, child) in children.iter().enumerate() {
                if index != 0 { out.write_all(b",")?; }
                write!(out, "{child}")?;
            }
            out.write_all(b"]}")
        }
        Predicate::Not(child) => write!(out, "{{\"op\":\"not\",\"child\":{child}}}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fa_reference::action::ResolvedTarget;
    use fa_reference::strict_json::{self, Json, Limits};
    fn encoded(predicate: &Predicate) -> Json {
        let mut bytes = Vec::new(); node(&mut bytes, predicate).unwrap();
        assert!(bytes.is_ascii()); assert!(!bytes.contains(&0x1b));
        strict_json::parse(&bytes, Limits::default()).unwrap()
    }
    #[test]
    fn every_native_predicate_preserves_its_operator_and_structural_arguments() {
        let target = ResolvedTarget { adapter: 1, object: u64::MAX, contract_version: 3, expected_version: 4, generation: 5 };
        let cases = [
            (Predicate::TargetIs(target), "target_is"),
            (Predicate::PayloadIs(vec![0, 0x1b, b'"', b'\\', 0xff]), "payload_is"),
            (Predicate::PayloadAtMost(4), "payload_at_most"), (Predicate::UnitsAtMost(u64::MAX), "units_at_most"),
            (Predicate::ExactValue { key: u64::MAX, value: vec![0xff] }, "exact_value"),
            (Predicate::Absent { key: 7 }, "absent"), (Predicate::EmptyRange { start: 8, end: 10 }, "empty_range"),
            (Predicate::All(vec![0, 1, 2]), "all"), (Predicate::Any(vec![0, 2]), "any"), (Predicate::Not(1), "not"),
        ];
        for (predicate, op) in &cases { assert_eq!(encoded(predicate).get("op").unwrap().as_str(), Some(*op)); }
        assert_eq!(encoded(&cases[0].0).get("object").unwrap().as_str(), Some("18446744073709551615"));
        assert_eq!(encoded(&cases[1].0).get("value_hex").unwrap().as_str(), Some("001b225cff"));
        assert_eq!(encoded(&cases[4].0).get("key").unwrap().as_str(), Some("18446744073709551615"));
        assert_eq!(encoded(&cases[9].0).get("child").unwrap().as_u64(), Some(1));
        let value = encoded(&cases[7].0);
        let Json::Array(children) = value.get("children").unwrap() else { panic!("native edges") };
        assert_eq!(children.iter().map(Json::as_u64).collect::<Vec<_>>(), vec![Some(0), Some(1), Some(2)]);
    }
    #[test]
    fn no_truth_or_next_requirement_is_rendered_as_permission() {
        assert_eq!(truth(Truth::Satisfied), "satisfied");
        assert_eq!(truth(Truth::Violated), "violated");
        assert_eq!(truth(Truth::Unknown), "unknown");
        assert_eq!(next(NextRequirement::FreshIndependentReview), "fresh_independent_review");
        assert_eq!(next(NextRequirement::ExactPolicyViolation), "exact_policy_violation");
        assert_eq!(next(NextRequirement::MoreEvidence), "more_evidence");
    }
}
