//! Completed disclosure is historical evidence, never a policy application.
//! Keep its versioned report separate from the existing applied-review schema.
use super::{FileReviewReplayProgress, Predicate, Report};
use fa_reference::action::consequence::{
    activation::tensor::kv::experiment::KvSide,
    delivery::persistent::observed::replay::{
        FileRefinedReviewReplay, ReviewEffectAudit,
    },
};
use std::fmt::{self, Write as _};

pub(super) fn render(
    replay: &FileRefinedReviewReplay,
    decision: &fa_reference::action::consequence::gate::containment::session::policy::controller::ReplayedDecision,
    effect: &ReviewEffectAudit,
    anchored_through: u64,
    progress: FileReviewReplayProgress,
) -> Result<String, fmt::Error> {
    let mut out = Report(String::new());
    let anchor = replay.anchor();
    let archive = replay.archive();
    let spec = anchor.inputs.action().spec();
    let completed = replay.completed_snapshot();
    let journal = replay.journal_snapshot();
    write!(
        out,
        "{{\"schema\":\"fa.refinement-audit/1\",\"status\":\"complete\",\"knowledge\":{{\"kind\":\"known\",\"basis\":{{\"class\":\"bounded_model\",\"scope\":\"historical_reference_replay\",\"authentic_origin\":\"unknown\",\"source_truth\":\"not_established\",\"independent_history_through\":\"{anchored_through}\",\"verified_journal_revision\":\"{}\",\"unanchored_suffix_events\":\"{}\"}},\"value\":{{",
        journal.revision,
        journal.revision.saturating_sub(anchored_through)
    )?;
    write!(
        out,
        "\"round\":\"{}\",\"attempt\":\"{}\",\"tenant\":\"{}\",\"run\":\"{}\",\"begin_revision\":\"{}\",\"completed_revision\":\"{}\",\"consequence\":\"{:?}\",\"policy_truth\":\"{:?}\",\"input_revision\":\"{}\",\"input_bytes\":\"{}\",\"payload_bytes\":\"{}\",",
        anchor.policy.round,
        anchor.policy.attempt,
        spec.scope.tenant,
        spec.scope.run,
        replay.begin_revision(),
        completed.revision,
        decision.decision().consequence,
        decision.evaluation().result(),
        archive.input_revision,
        archive.inputs.logical_bytes(),
        spec.payload.len()
    )?;
    write!(
        out,
        "\"commit_count\":\"{}\",\"reveal_count\":\"{}\",\"started_at\":\"{}\",\"completed_at\":\"{}\",\"application\":",
        archive.commit_times.len(),
        archive.reveal_times.len(),
        archive.started_at.0,
        archive.completed_at.0
    )?;
    out.write_str("{\"status\":\"not_attempted\",\"error\":null}")?;
    let group = replay.group();
    let side = match group.row.side {
        KvSide::Key => "key",
        KvSide::Value => "value",
    };
    write!(
        out,
        ",\"application_inputs\":\"not_applicable\",\"refinement\":{{\"input_revision\":\"{}\",\"group\":{{\"layer\":\"{}\",\"side\":\"{side}\",\"position\":\"{}\",\"head\":\"{}\"}},\"requires_fresh_review\":true}},\"policy_trace\":[",
        replay.input_revision(),
        group.row.layer,
        group.row.position,
        group.head
    )?;
    for (index, step) in decision.evaluation().trace().iter().enumerate() {
        if index != 0 {
            out.write_str(",")?;
        }
        let node = &anchor.policy.policy.nodes()[step.node];
        let (kind, children): (&str, &[usize]) = match node {
            Predicate::TargetIs(_) => ("TargetIs", &[]),
            Predicate::PayloadIs(_) => ("PayloadIs", &[]),
            Predicate::PayloadAtMost(_) => ("PayloadAtMost", &[]),
            Predicate::UnitsAtMost(_) => ("UnitsAtMost", &[]),
            Predicate::ExactValue { .. } => ("ExactValue", &[]),
            Predicate::Absent { .. } => ("Absent", &[]),
            Predicate::EmptyRange { .. } => ("EmptyRange", &[]),
            Predicate::All(children) => ("All", children),
            Predicate::Any(children) => ("Any", children),
            Predicate::Not(child) => ("Not", std::slice::from_ref(child)),
        };
        write!(
            out,
            "{{\"node\":\"{}\",\"predicate\":\"{kind}\",\"truth\":\"{:?}\",\"children\":[",
            step.node, step.result
        )?;
        for (position, child) in children.iter().enumerate() {
            if position != 0 {
                out.write_str(",")?;
            }
            write!(out, "\"{child}\"")?;
        }
        out.write_str("],\"witness\":")?;
        match step.witness {
            Some(index) => write!(out, "\"{index}\"")?,
            None => out.write_str("null")?,
        }
        out.write_str("}")?;
    }
    out.write_str("],\"members\":[")?;
    for (index, (name, member)) in anchor.policy.congress.members.iter().enumerate() {
        if index != 0 {
            out.write_str(",")?;
        }
        out.write_str("{\"member_utf8_hex\":\"")?;
        for byte in name.as_bytes() {
            write!(out, "{byte:02x}")?;
        }
        out.write_str("\",\"cohort_utf8_hex\":\"")?;
        for byte in member.cohort.as_bytes() {
            write!(out, "{byte:02x}")?;
        }
        write!(
            out,
            "\",\"configured_weight\":\"{}\",\"missing\":{},\"abstained\":{},\"admitted_weight\":",
            member.weight,
            decision.missing().contains(name),
            decision.abstained().contains(name)
        )?;
        match decision.tally().admitted_weights.get(name) {
            Some(weight) => write!(out, "\"{weight}\"")?,
            None => out.write_str("null")?,
        }
        out.write_str("}")?;
    }
    write!(
        out,
        "],\"historical_effects\":{{\"executions_at_review\":\"{}\",\"executions_at_journal_cut\":\"{}\",\"charged_at_journal_cut\":\"{}\",\"reserved_at_journal_cut\":\"{}\"}},\"attempt_effects\":{{\"stage_at_review\":\"{:?}\",\"stage_at_journal_cut\":\"{:?}\",\"selected_review_dispatched\":{},\"dispatch\":",
        completed.executions,
        journal.executions,
        journal.control.ledger.charged,
        journal.control.ledger.reserved,
        effect.stage_at_review(),
        effect.stage_at_end(),
        effect.selected_review_dispatched()
    )?;
    match effect.dispatch() {
        Some(dispatch) => {
            let authorization = dispatch.authorization();
            write!(
                out,
                "{{\"revision\":\"{}\",\"authorization_round\":\"{}\",\"review_revision\":\"{}\",\"authorization_revision\":\"{}\",\"input_revision\":\"{}\"}}",
                dispatch.revision(),
                authorization.round(),
                authorization.review_revision(),
                authorization.revision(),
                authorization.input_revision()
            )?;
        }
        None => out.write_str("null")?,
    }
    // These are original event observations for the SAME attempt. A later
    // round's dispatch never becomes an application by the selected refinement.
    write!(
        out,
        ",\"terminal_outcome_recorded\":{},\"reconciliation_recorded\":{}}},\"replay_work\":{{\"verified_events\":\"{}\",\"attempted_events\":\"{}\"}}}}}},\"disclosure\":\"summary_only_no_payload_or_helper_input_bytes\",\"authority\":\"none\",\"external_receipt\":\"not_verified\"}}\n",
        effect.outcome().is_some(),
        effect.reconciliation().is_some(),
        progress.replayed_events,
        progress.attempted_events
    )?;
    Ok(out.0)
}
