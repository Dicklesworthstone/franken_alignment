//! Offline historical audit: no owner recovery, live helper, reviewer or effect.
use super::{Config, FileOversight, Loaded, Path, anchors, debug, recipe};
use fa_reference::action::consequence::delivery::persistent::observed::{
    guarded::anchored::FileHistoryAnchor,
    replay::{FileReviewReplay, FileReviewReplayCursor, FileReviewReplayProgress, FileReviewReplayStatus},
};
use fa_reference::action::consequence::gate::containment::session::policy::Predicate;
use std::{fmt::{self, Write as _}, fs, io::Write, time::{Duration, Instant}};

const USAGE: &str = "create-learned-generated CONFIG LEARNED_RECIPE --audit-review ROUND HISTORY_ANCHOR";
const MAX_REPORT_BYTES: usize = 64 * 1024;

pub(super) fn command(args: &[String], output: &mut impl Write) -> Result<(), String> {
    if args.len() != 6 || args[0] != "create-learned-generated" || args[3] != "--audit-review" {
        return Err(USAGE.into());
    }
    let round = args[4].parse::<u64>().map_err(debug)?;
    if round == 0 { return Err("audit round must be nonzero".into()); }
    let started = Instant::now();
    let config = Config::read(Path::new(&args[1]))?;
    let allowance = Duration::from_millis(config.timing.runtime_ms);
    let mut checkpoint = || {
        if started.elapsed() >= allowance { Err("historical audit deadline elapsed; no report or mutation".into()) }
        else { Ok(()) }
    };
    checkpoint()?;
    // The archive is required, external operator custody, never today's journal
    // promoted into a supposedly independent expectation. No output-anchor mode.
    let anchor = load_anchor(&config, Path::new(&args[5]))?;
    checkpoint()?;
    // Same complete asset/fit/model/probe admission, but NO live native roster.
    let loaded = recipe::load(Path::new(&args[2]), &config, false)?;
    checkpoint()?;
    run(&config, &loaded, round, &anchor, output, &mut checkpoint)
}

pub(super) fn load_anchor(config: &Config, path: &Path) -> Result<FileHistoryAnchor, String> {
    let store = fs::canonicalize(&config.store).map_err(debug)?;
    let identity = fs::canonicalize(path).map_err(debug)?;
    if identity.starts_with(&store) {
        return Err("audit anchor must be independently retained outside the authority store".into());
    }
    anchors::load(path)
}

// The original reader binds every independently loaded numerical/predictor byte.
// A recipe-owned raw forecast uses its original generation binder, never a raw
// supplied-capture map. No read failure is retried through unanchored recovery.
pub(super) fn run(config: &Config, loaded: &Loaded, round: u64, anchor: &FileHistoryAnchor,
    output: &mut impl Write, checkpoint: &mut impl FnMut() -> Result<(), String>) -> Result<(), String>
{
    checkpoint()?;
    let cursor = match loaded.generation.required_owned_pre_output_forecast() {
        Some(predictor) => FileOversight::begin_read_review_replay_anchored_with_owned_learned_consistency(
            &config.store, &config.profile, &loaded.generation, predictor, round, anchor),
        None => FileOversight::begin_read_review_replay_anchored_with_learned_generation(
            &config.store, &config.profile, &loaded.generation, round, anchor),
    }.map_err(debug)?;
    let (replay, progress) = complete(cursor, checkpoint)?;
    // Recheck using the original independent decision verifier, not display
    // heuristics. Its expected pre-vote material is reconstructed from the exact
    // externally anchored history, not independently authenticated as a source.
    let decision = replay.archive().verify(replay.anchor()).map_err(debug)?;
    if let Ok(receipt) = replay.application() {
        replay.archive().verify_receipt(replay.anchor(), receipt).map_err(debug)?;
    }
    checkpoint()?;
    let report = render(&replay, &decision, anchor.revision(), progress)
        .map_err(|_| "audit summary exceeds its bounded output allowance".to_owned())?;
    checkpoint()?;
    // Nothing reaches stdout until replay, suffix verification and rendering
    // finish. Output errors never trigger a replay, recovery, send or retry.
    output.write_all(report.as_bytes()).and_then(|()| output.flush())
        .map_err(|error| format!("audit output failed: {error}; no effect or recovery was attempted"))
}

fn complete(mut cursor: FileReviewReplayCursor, checkpoint: &mut impl FnMut() -> Result<(), String>)
    -> Result<(FileReviewReplay, FileReviewReplayProgress), String>
{
    while cursor.progress().status == FileReviewReplayStatus::Replaying {
        checkpoint()?;
        cursor.advance(cursor.progress().replayed_events, 1).map_err(debug)?;
    }
    checkpoint()?;
    let progress = cursor.progress();
    Ok((cursor.finish().map_err(debug)?, progress))
}

struct Report(String);
impl fmt::Write for Report {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let size = self.0.len().checked_add(value.len()).ok_or(fmt::Error)?;
        if size > MAX_REPORT_BYTES { return Err(fmt::Error); }
        self.0.try_reserve(value.len()).map_err(|_| fmt::Error)?;
        self.0.push_str(value);
        Ok(())
    }
}

fn render(replay: &FileReviewReplay,
    decision: &fa_reference::action::consequence::gate::containment::session::policy::controller::ReplayedDecision,
    anchored_through: u64, progress: FileReviewReplayProgress) -> Result<String, fmt::Error>
{
    let mut out = Report(String::new());
    let anchor = replay.anchor();
    let archive = replay.archive();
    let spec = anchor.inputs.action().spec();
    let completed = replay.completed_snapshot();
    let journal = replay.journal_snapshot();
    write!(out, "{{\"schema\":\"fa.review-audit/1\",\"status\":\"complete\",\"knowledge\":{{\"kind\":\"known\",\"basis\":{{\"class\":\"bounded_model\",\"scope\":\"historical_reference_replay\",\"authentic_origin\":\"unknown\",\"source_truth\":\"not_established\",\"independent_history_through\":\"{anchored_through}\",\"verified_journal_revision\":\"{}\",\"unanchored_suffix_events\":\"{}\"}},\"value\":{{",
        journal.revision, journal.revision.saturating_sub(anchored_through))?;
    write!(out, "\"round\":\"{}\",\"attempt\":\"{}\",\"tenant\":\"{}\",\"run\":\"{}\",\"begin_revision\":\"{}\",\"completed_revision\":\"{}\",\"consequence\":\"{:?}\",\"policy_truth\":\"{:?}\",\"input_revision\":\"{}\",\"input_bytes\":\"{}\",\"payload_bytes\":\"{}\",",
        anchor.policy.round, anchor.policy.attempt, spec.scope.tenant, spec.scope.run,
        replay.begin_revision(), completed.revision, decision.decision().consequence,
        decision.evaluation().result(), archive.input_revision, archive.inputs.logical_bytes(), spec.payload.len())?;
    write!(out, "\"commit_count\":\"{}\",\"reveal_count\":\"{}\",\"started_at\":\"{}\",\"completed_at\":\"{}\",\"application\":",
        archive.commit_times.len(), archive.reveal_times.len(), archive.started_at.0, archive.completed_at.0)?;
    match replay.application() {
        Ok(_) => out.write_str("{\"status\":\"applied\",\"error\":null}")?,
        Err(error) => write!(out, "{{\"status\":\"refused\",\"error\":\"{error:?}\"}}")?,
    }
    let inputs = match replay.application_inputs() {
        None => "missing", Some(current) if current == archive.inputs.as_ref() => "equal_to_reviewed",
        Some(_) => "changed",
    };
    write!(out, ",\"application_inputs\":\"{inputs}\",\"policy_trace\":[")?;
    for (index, step) in decision.evaluation().trace().iter().enumerate() {
        if index != 0 { out.write_str(",")?; }
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
        write!(out, "{{\"node\":\"{}\",\"predicate\":\"{kind}\",\"truth\":\"{:?}\",\"children\":[", step.node, step.result)?;
        for (position, child) in children.iter().enumerate() {
            if position != 0 { out.write_str(",")?; }
            write!(out, "\"{child}\"")?;
        }
        out.write_str("],\"witness\":")?;
        match step.witness { Some(index) => write!(out, "\"{index}\"")?, None => out.write_str("null")? }
        out.write_str("}")?;
    }
    out.write_str("],\"members\":[")?;
    for (index, (name, member)) in anchor.policy.congress.members.iter().enumerate() {
        if index != 0 { out.write_str(",")?; }
        out.write_str("{\"member_utf8_hex\":\"")?;
        for byte in name.as_bytes() { write!(out, "{byte:02x}")?; }
        out.write_str("\",\"cohort_utf8_hex\":\"")?;
        for byte in member.cohort.as_bytes() { write!(out, "{byte:02x}")?; }
        write!(out, "\",\"configured_weight\":\"{}\",\"missing\":{},\"abstained\":{},\"admitted_weight\":",
            member.weight, decision.missing().contains(name), decision.abstained().contains(name))?;
        match decision.tally().admitted_weights.get(name) {
            Some(weight) => write!(out, "\"{weight}\"")?, None => out.write_str("null")?,
        }
        out.write_str("}")?;
    }
    write!(out, "],\"historical_effects\":{{\"executions_at_review\":\"{}\",\"executions_at_journal_cut\":\"{}\",\"charged_at_journal_cut\":\"{}\",\"reserved_at_journal_cut\":\"{}\"}},\"replay_work\":{{\"verified_events\":\"{}\",\"attempted_events\":\"{}\"}}}}}},\"disclosure\":\"summary_only_no_payload_or_helper_input_bytes\",\"authority\":\"none\",\"external_receipt\":\"not_verified\"}}\n",
        completed.executions, journal.executions, journal.control.ledger.charged,
        journal.control.ledger.reserved, progress.replayed_events, progress.attempted_events)?;
    Ok(out.0)
}

#[cfg(test)]
mod boundary_tests {
    use super::*;
    #[test]
    fn report_capacity_is_checked_before_appending_and_does_not_truncate() {
        let mut report = Report(String::new());
        report.write_str(&"x".repeat(MAX_REPORT_BYTES)).unwrap();
        assert!(report.write_str("y").is_err());
        assert_eq!(report.0.len(), MAX_REPORT_BYTES);
        assert!(report.0.bytes().all(|byte| byte == b'x'));
    }
}

#[path = "audit/situation.rs"]
mod situation;
