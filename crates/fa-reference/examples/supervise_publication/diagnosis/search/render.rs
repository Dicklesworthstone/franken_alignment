//! A bounded projection of the native search report, including failed cases.
//! Candidate byte strings occur once; cases retain indexes rather than copies.
use super::*;
use fa_reference::action::consequence::experiment::{Minimality, NextRequirement};
use fa_reference::action::consequence::gate::containment::session::policy::Truth;
use std::io::{self, Write};

// Separate operator encoding bound, NOT the native retained-edit-byte budget.
// At most 256 cases x 128 truth/changed nodes and one 64-KiB menu fit this bound.
const OUTPUT_BYTES: usize = 4 * 1024 * 1024;
struct Output(Vec<u8>);
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.0.len().checked_add(bytes.len()).is_none_or(|n| n > OUTPUT_BYTES) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "search JSON exceeds its output bound"));
        }
        self.0.extend_from_slice(bytes); Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

pub(super) fn encode(report: &FileProposalRepairReport, menu: &Menu) -> Result<Vec<u8>, String> {
    encode_inner(report, menu).map_err(|error| format!("encode proposal search: {error}"))
}
fn encode_inner(report: &FileProposalRepairReport, menu: &Menu) -> io::Result<Vec<u8>> {
    let origin = report.origin(); let search = report.search(); let proposal = origin.proposal();
    let captured = origin.journal_snapshot();
    let invalid = || io::Error::new(io::ErrorKind::InvalidData, "incomplete native search report");
    let request = origin.external_request().ok_or_else(invalid)?;
    let ending = captured.control.ledger.stages.get(&proposal.attempt).ok_or_else(invalid)?;
    let missing: Vec<_> = search.unreported_masks().collect();
    if search.cases().len() + missing.len() != search.work().planned_cases
        || origin.scope() != &menu.scope || search.candidates() != menu.candidates.as_slice() {
        return Err(invalid());
    }
    let mut out = Output(Vec::new());
    let status = status(search.status());
    write!(out, concat!("{{\"schema_version\":\"fa.proposal-search-result/1\",",
        "\"purpose\":\"experiment\",\"permission\":\"not_issued\",\"historical\":true,",
        "\"source\":\"captured_journal_image\",\"request\":\"{}\",\"attempt\":\"{}\",\"scope\":\"{}\",",
        "\"admission_revision\":\"{}\",\"journal_revision\":\"{}\",",
        "\"admission_state\":\"{:?}\",\"ending_state\":\"{:?}\",",
        "\"policy_generation\":\"{}\",\"semantic_epoch\":\"{}\",\"witness_count\":{},",
        "\"executions_at_capture\":\"{}\",\"charged_units\":\"{}\",\"reserved_units\":\"{}\",",
        "\"status\":\"{}\",\"budget\":"), request, proposal.attempt, menu.id,
        origin.admission_snapshot().revision, captured.revision, proposal.state, ending,
        proposal.policy.generation(), proposal.snapshot_semantic_epoch, proposal.evaluation.witnesses().len(),
        captured.executions, captured.control.ledger.charged, captured.control.ledger.reserved, status)?;
    write!(out, "{{\"cases\":{},\"retained_edit_bytes\":{}}},\"stop_after_cases\":{},\"work\":",
        menu.budget.cases, menu.budget.retained_edit_bytes, menu.stop_after_cases)?;
    let work = search.work();
    write!(out, "{{\"planned_cases\":{},\"entered_cases\":{},\"completed_cases\":{},\"planned_edit_bytes\":{},\"retained_edit_bytes\":{}}}",
        work.planned_cases, work.entered_cases, work.completed_cases, work.planned_edit_bytes, work.retained_edit_bytes)?;
    out.write_all(b",\"candidates\":[")?;
    for (index, candidate) in search.candidates().iter().enumerate() {
        if index != 0 { out.write_all(b",")?; }
        write!(out, "{{\"index\":{index},\"edit\":")?; edit(&mut out, candidate)?; out.write_all(b"}")?;
    }
    out.write_all(b"],\"cases\":[")?;
    for (index, case) in search.cases().iter().enumerate() {
        if index != 0 { out.write_all(b",")?; }
        write!(out, "{{\"mask\":{},\"selected\":", case.mask)?;
        selection(&mut out, case.mask, search.candidates().len())?;
        match &case.outcome {
            Err(error) => write!(out, ",\"result\":{{\"kind\":\"refused\",\"error\":\"{error:?}\"}}")?,
            Ok(result) => {
                write!(out, ",\"result\":{{\"kind\":\"evaluated\",\"baseline\":\"{}\",\"counterfactual\":\"{}\",\"next_requirement\":\"{}\",\"trace\":[",
                    truth(result.baseline()), truth(result.counterfactual()), next(result.next_requirement()))?;
                for (node, value) in result.trace().iter().enumerate() {
                    if node != 0 { out.write_all(b",")?; }
                    write!(out, "\"{}\"", truth(*value))?;
                }
                out.write_all(b"],\"changes\":[")?;
                for (change_index, change) in result.changes().iter().enumerate() {
                    if change_index != 0 { out.write_all(b",")?; }
                    write!(out, "{{\"node\":{},\"baseline\":\"{}\",\"counterfactual\":\"{}\"}}",
                        change.node, truth(change.baseline), truth(change.counterfactual))?;
                }
                out.write_all(b"]}")?;
            }
        }
        out.write_all(b"}")?;
    }
    out.write_all(b"],\"repairs\":[")?;
    for (index, repair) in search.sufficient_repairs().iter().enumerate() {
        if index != 0 { out.write_all(b",")?; }
        write!(out, "{{\"mask\":{},\"untouched\":{},\"minimality\":\"{}\",\"selected\":",
            repair.mask, repair.mask == 0, minimality(repair.minimality))?;
        selection(&mut out, repair.mask, search.candidates().len())?;
        out.write_all(b",\"next_requirement\":\"fresh_independent_review\"}")?;
    }
    out.write_all(b"],\"unreported_masks\":[")?;
    for (index, mask) in missing.iter().enumerate() {
        if index != 0 { out.write_all(b",")?; }
        write!(out, "{mask}")?;
    }
    out.write_all(b"]}\n")?;
    Ok(out.0)
}
fn selection(out: &mut impl Write, mask: u16, count: usize) -> io::Result<()> {
    out.write_all(b"[")?;
    let mut first = true;
    for index in 0..count {
        if mask & (1_u16 << index) != 0 {
            if !first { out.write_all(b",")?; }
            first = false; write!(out, "{index}")?;
        }
    }
    out.write_all(b"]")
}
fn hex(out: &mut impl Write, bytes: &[u8]) -> io::Result<()> {
    out.write_all(b"\"")?; for byte in bytes { write!(out, "{byte:02x}")?; } out.write_all(b"\"")
}
fn optional_hex(out: &mut impl Write, bytes: &Option<Vec<u8>>) -> io::Result<()> {
    match bytes { Some(bytes) => hex(out, bytes), None => out.write_all(b"null") }
}
fn edit(out: &mut impl Write, value: &Intervention) -> io::Result<()> {
    match value {
        Intervention::Payload(bytes) => {
            out.write_all(b"{\"kind\":\"payload\",\"bytes_hex\":")?; hex(out, bytes)?; out.write_all(b"}")
        }
        Intervention::Units(value) => write!(out, "{{\"kind\":\"units\",\"value\":\"{value}\"}}"),
        Intervention::Target(t) => write!(out, concat!("{{\"kind\":\"target\",\"adapter\":\"{}\",\"object\":\"{}\",",
            "\"contract_version\":\"{}\",\"expected_version\":\"{}\",\"generation\":\"{}\"}}"),
            t.adapter, t.object, t.contract_version, t.expected_version, t.generation),
        Intervention::Key { key, expected, replacement } => {
            write!(out, "{{\"kind\":\"key\",\"key\":\"{key}\",\"expected_hex\":")?;
            optional_hex(out, expected)?; out.write_all(b",\"replacement_hex\":")?;
            optional_hex(out, replacement)?; out.write_all(b"}")
        }
    }
}
fn truth(value: Truth) -> &'static str {
    match value { Truth::Satisfied => "satisfied", Truth::Violated => "violated", Truth::Unknown => "unknown" }
}
fn next(value: NextRequirement) -> &'static str {
    match value { NextRequirement::FreshIndependentReview => "fresh_independent_review",
        NextRequirement::ExactPolicyViolation => "exact_policy_violation", NextRequirement::MoreEvidence => "more_evidence" }
}
fn minimality(value: Minimality) -> &'static str {
    match value { Minimality::EstablishedWithinMenu => "established_within_menu",
        Minimality::NotMinimal => "not_minimal", Minimality::Undetermined => "undetermined" }
}
fn status(value: ProposalSearchStatus) -> &'static str {
    match value { ProposalSearchStatus::Running => "running", ProposalSearchStatus::Complete => "complete",
        ProposalSearchStatus::Cancelled => "cancelled", ProposalSearchStatus::Interrupted => "interrupted" }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encoding_bound_accepts_exact_bytes_and_refuses_before_growing_past_it() {
        let mut out = Output(vec![b' '; OUTPUT_BYTES - 1]);
        assert_eq!(out.write(b"x").unwrap(), 1);
        assert_eq!(out.write(b"y").unwrap_err().kind(), io::ErrorKind::InvalidData);
        assert_eq!(out.0.len(), OUTPUT_BYTES);
        assert!(out.write(b"").is_ok());
    }
}
