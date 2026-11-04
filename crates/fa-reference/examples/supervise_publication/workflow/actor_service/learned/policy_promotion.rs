//! Explicit trusted offline governor workflow, separate from policy replay and
//! human effect approval. The sealed native owner is the only policy writer.
use super::{Config, Duration, Instant, Loaded, anchors, checked, debug, fields, recipe, recovery, topology};
use crate::{config::read_regular, policy_replay::{Request as Candidate, encode}};
use crate::workflow::continuation::checkpoint::AcknowledgedOutput;
use fa_reference::action::consequence::delivery::persistent::{
    governance::PolicyUpdate,
    observed::decoder::learned::{FileLearnedPolicyPromotionHistory,
        FileLearnedPolicyPromotionRequest, FileLearnedPolicyPromotionReview},
    observed::driver::topology::checkpoint::TopologyCheckpoint,
};
use fa_reference::strict_json::{self, Json, Limits};
use std::{collections::BTreeSet, fs, io::Write, path::{Path, PathBuf}};

const USAGE: &str = "create-learned-generated CONFIG LEARNED_RECIPE --promote-policy GOVERNOR_REQUEST --history-anchor INPUT --retain-history-anchor NEW_OUTPUT [--topology-profile PATH --retain-topology-checkpoint NEW_OUTPUT]";
const REQUEST_BYTES: usize = 16_384;
const DEADLINE: &str = "learned policy promotion deadline reached before acknowledgment";

pub(super) struct Request {
    pub operation: u64,
    pub revision: u64,
    pub sequence: u64,
    pub epoch: u64,
    pub candidate: PathBuf,
    pub reviewed_report: PathBuf,
    pub accept_newly_reviewable: bool,
}

pub(super) fn decode(bytes: &[u8]) -> Result<Request, String> {
    let json = strict_json::parse(bytes, Limits { max_bytes: REQUEST_BYTES,
        max_depth: 2, max_items: 32, max_string_bytes: 4096 }).map_err(debug)?;
    let mut fields = fields::Fields::new(json)?;
    if fields.text("schema")? != "fa.learned-policy-promotion/1" {
        return Err("unsupported learned policy promotion schema".into());
    }
    let operation = number(&mut fields, "operation")?;
    let revision = number(&mut fields, "expected_journal_revision")?;
    let sequence = number(&mut fields, "expected_control_sequence")?;
    let epoch = number(&mut fields, "expected_authority_epoch")?;
    let candidate = fields.path("candidate_request")?;
    let reviewed_report = fields.path("reviewed_report")?;
    let Json::Bool(accept_newly_reviewable) = fields.take("accept_newly_reviewable")? else {
        return Err("accept_newly_reviewable must be an explicit Boolean".into());
    };
    fields.end()?;
    if operation == 0 || revision == 0 { return Err("policy operation and revision must be nonzero".into()); }
    Ok(Request { operation, revision, sequence, epoch, candidate, reviewed_report,
        accept_newly_reviewable })
}

fn number(fields: &mut fields::Fields, name: &str) -> Result<u64, String> {
    let value = fields.text(name)?;
    if value.is_empty() || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("{name} must be a canonical unsigned decimal string"));
    }
    value.parse().map_err(|_| format!("{name} exceeds the unsigned 64-bit range"))
}

struct Arguments { topology: Option<PathBuf>, source_output: Option<PathBuf> }

fn arguments(args: &[String]) -> Result<Arguments, String> {
    if args.len() < 9 || args.first().map(String::as_str) != Some("create-learned-generated")
        || args[3] != "--promote-policy" { return Err(USAGE.into()); }
    for at in [1, 2, 4] { fields::path(&args[at])?; }
    let mut seen = BTreeSet::new();
    let mut selected = Arguments { topology: None, source_output: None };
    let mut at = 5;
    while at < args.len() {
        let flag = args[at].as_str();
        if !matches!(flag, "--history-anchor" | "--retain-history-anchor"
            | "--topology-profile" | "--retain-topology-checkpoint") || !seen.insert(flag) {
            return Err(USAGE.into());
        }
        let path = fields::path(args.get(at + 1).ok_or(USAGE)?).map_err(|_| USAGE.to_owned())?;
        match flag {
            "--topology-profile" => selected.topology = Some(path),
            "--retain-topology-checkpoint" => selected.source_output = Some(path),
            _ => {}
        }
        at += 2;
    }
    if !seen.contains("--history-anchor") || !seen.contains("--retain-history-anchor")
        || selected.topology.is_some() != selected.source_output.is_some() {
        return Err(USAGE.into());
    }
    Ok(selected)
}

pub(super) fn command(args: &[String], credibility: Option<&Path>, output: &mut impl Write)
    -> Result<(), String>
{
    let started = Instant::now();
    command_with_elapsed(args, credibility, output, &mut || started.elapsed())
}

pub(super) fn command_with_elapsed(args: &[String], credibility: Option<&Path>,
    output: &mut impl Write, elapsed: &mut impl FnMut() -> Duration) -> Result<(), String>
{
    if credibility.is_some() { return Err("policy governance cannot activate helper credibility".into()); }
    let arguments = arguments(args)?;
    let config = Config::read_learned_recovery(Path::new(&args[1]))?;
    let wall = Duration::from_millis(config.timing.runtime_ms);
    checkpoint(wall, elapsed)?;
    let request_path = Path::new(&args[4]);
    outside(&config, request_path)?;
    let request = decode(&read_regular(request_path, REQUEST_BYTES)?)?;
    outside(&config, &request.candidate)?;
    outside(&config, &request.reviewed_report)?;
    let candidate = Candidate::read(&request.candidate)?;
    let reviewed = read_regular(&request.reviewed_report, candidate.output_bytes)?;
    checkpoint(wall, elapsed)?;
    let mut loaded = recipe::load(Path::new(&args[2]), &config, false)?;
    checked::check_selection(&config, &loaded)?;
    let (_, custody) = anchors::take_options(args)?;
    custody.admit(&config, &mut loaded, true)?;
    if let Some(path) = &arguments.topology { outside(&config, path)?; }
    topology::admit(&config, &mut loaded, arguments.topology.as_deref(), topology::Mode::Historical)?;
    check_selection(&loaded)?;
    let source_output = arguments.source_output.as_deref().map(|path| {
        let output = AcknowledgedOutput::admit(&config, path)?;
        if loaded.anchor_output.as_deref() == Some(output.path()) {
            return Err("policy promotion history and topology outputs must be distinct new files".into());
        }
        Ok::<_, String>(output)
    }).transpose()?;
    checkpoint(wall, elapsed)?;
    run_selected(&config, &loaded, &request, candidate, &reviewed,
        (output, source_output.as_ref()), elapsed)
}

fn outside(config: &Config, path: &Path) -> Result<(), String> {
    if fs::canonicalize(path).map_err(debug)?.starts_with(fields::resolve_root(&config.store)?) {
        return Err("policy governance inputs must be retained outside the authority store".into());
    }
    Ok(())
}

pub(super) fn check_selection(loaded: &Loaded) -> Result<(), String> {
    if loaded.expected_campaigns.is_none() {
        return Err("policy promotion requires an independently selected existing campaign guard".into());
    }
    if loaded.recovery_anchor.is_none() || loaded.anchor_output.is_none() {
        return Err("policy promotion requires an independent input history anchor and a new retained output".into());
    }
    if loaded.generation.required_pre_output_forecast().is_some()
        || loaded.generation.required_owned_pre_output_forecast().is_some() {
        return Err("offline policy promotion requires a nonpredictive learned history".into());
    }
    if let Some(history) = topology::history(loaded)? {
        let saved = history.checkpoint().ok_or("policy promotion requires an exact v2 topology-source checkpoint")?;
        if Some(saved.history_anchor()) != loaded.recovery_anchor.as_ref() {
            return Err("policy promotion source checkpoint and history anchor describe different cuts".into());
        }
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn run(config: &Config, loaded: &Loaded, request: &Request, candidate: Candidate,
    reviewed: &[u8], output: &mut impl Write, elapsed: &mut impl FnMut() -> Duration)
    -> Result<(), String>
{
    run_selected(config, loaded, request, candidate, reviewed, (output, None), elapsed)
}

fn run_selected(config: &Config, loaded: &Loaded, request: &Request, candidate: Candidate,
    reviewed: &[u8], outputs: (&mut impl Write, Option<&AcknowledgedOutput>),
    elapsed: &mut impl FnMut() -> Duration) -> Result<(), String>
{
    let (output, source_output) = outputs;
    checked::check_selection(config, loaded)?;
    check_selection(loaded)?;
    if topology::history(loaded)?.is_some() != source_output.is_some() {
        return Err("policy promotion requires paired history and topology checkpoint retention".into());
    }
    if loaded.expected_campaigns.ok_or(USAGE)?.limits != candidate.limits {
        return Err("reviewed report limits must equal the installed campaign guard exactly".into());
    }
    let update = PolicyUpdate::new(request.operation, request.sequence, request.epoch,
        candidate.candidate).map_err(debug)?;
    let selected = FileLearnedPolicyPromotionRequest::new(request.revision, update).map_err(debug)?;
    let wall = Duration::from_millis(config.timing.runtime_ms);
    checkpoint(wall, elapsed)?;
    let run = recovery::policy_promotion::ready(config, loaded, selected,
        &mut |_| checkpoint(wall, elapsed))?;
    let original = run.policy_promotion_review().map_err(debug)?.clone();
    compare(&original, candidate.expected_policy_generation, candidate.output_bytes, reviewed)?;
    checkpoint(wall, elapsed)?;
    let (history, source) = run.finish_promoted_policy(&original, request.accept_newly_reviewable)
        .map_err(|error| format!("policy promotion is unconfirmed: {error:?}; retain the exact governor request, reviewed report and old history anchor"))?;
    // An acknowledged transition cannot be retracted by a slow or failing
    // archive/output. Attempt both independently; exact retries append nothing.
    let mut failures = Vec::new();
    let late = elapsed() >= wall;
    if late { failures.push("original policy promotion deadline reached after acknowledgment".into()); }
    let retained = anchors::export_acknowledged(history.history_anchor(),
        loaded.anchor_output.as_deref().ok_or(USAGE)?);
    if let Err(error) = &retained { failures.push(format!("history retention failed: {error}")); }
    let source_retained = match (source_output, source.as_ref()) {
        (Some(destination), Some(saved)) => Some(destination.retain(saved)),
        (None, None) => None,
        _ => Some(Err("policy promotion has no matching acknowledged topology checkpoint".into())),
    };
    if let Some(Err(error)) = &source_retained { failures.push(error.clone()); }
    let source_report = source.as_ref().map(|saved|
        (saved, source_retained.as_ref().is_some_and(Result::is_ok)));
    if let Err(error) = report(&history, retained.is_ok(), late, source_report, output) { failures.push(error); }
    if !late && elapsed() >= wall { failures.push("original policy promotion deadline reached during acknowledged output".into()); }
    if failures.is_empty() { Ok(()) } else { Err(format!(
        "policy promotion acknowledged at revision {}: {}; retry the exact request/report with the independently selected current policy and new paired outputs when topology is selected",
        history.snapshot().revision, failures.join("; "))) }
}

fn compare(original: &FileLearnedPolicyPromotionReview, previous_generation: u64,
    output_bytes: usize, reviewed: &[u8]) -> Result<(), String>
{
    if original.report().previous_policy().generation() != previous_generation {
        return Err("governor selection differs from the original previous policy generation".into());
    }
    let expected = encode(original.snapshot(), original.report(), original.limits(), output_bytes)?;
    if expected != reviewed {
        return Err("complete original policy report differs from the independently reviewed report; no promotion was acknowledged".into());
    }
    Ok(())
}

fn checkpoint(wall: Duration, elapsed: &mut impl FnMut() -> Duration) -> Result<(), String> {
    if elapsed() >= wall { Err(DEADLINE.into()) } else { Ok(()) }
}

fn report(history: &FileLearnedPolicyPromotionHistory, retained: bool, late: bool,
    source: Option<(&TopologyCheckpoint, bool)>,
    output: &mut impl Write) -> Result<(), String>
{
    let cut = history.snapshot();
    let change = history.receipt().change();
    let mut bytes = format!(concat!(
        "{{\"schema\":\"{}\",\"outcome\":\"{}\",",
        "\"operation\":\"{}\",\"reviewed_revision\":\"{}\",\"revision\":\"{}\",",
        "\"appended_events\":\"{}\",\"promoted_policy_generation\":\"{}\",",
        "\"current_policy_generation\":\"{}\",\"control_sequence\":\"{}\",",
        "\"authority_epoch\":\"{}\",\"executions\":\"{}\",\"charged\":\"{}\",",
        "\"reserved\":\"{}\",\"accepted_newly_reviewable\":{},\"cancelled_attempts\":["),
        if source.is_some() { "fa.learned-policy-promotion-result/2" } else { "fa.learned-policy-promotion-result/1" },
        if history.appended_events() == 0 { "already_promoted" } else { "promoted" },
        history.receipt().update().operation(), history.review().request().revision(),
        cut.revision, history.appended_events(), history.receipt().update().policy().generation(),
        history.current_policy().generation(), cut.control.sequence, cut.control.ledger.epoch,
        cut.executions, cut.control.ledger.charged, cut.control.ledger.reserved,
        history.accepted_newly_reviewable());
    for (index, attempt) in change.cancelled.iter().enumerate() {
        if index != 0 { bytes.push(','); }
        bytes.push_str(&format!("\"{attempt}\""));
    }
    bytes.push_str(&format!("],\"history_anchor_retained\":{},\"deadline_reached\":{}", retained, late));
    if let Some((saved, retained)) = source {
        bytes.push_str(&format!(concat!(
            ",\"topology_checkpoint_retained\":{},\"topology_checkpoint_revision\":\"{}\",",
            "\"topology_source_checks\":\"{}\",\"topology_source_read_bytes\":\"{}\",",
            "\"topology_last_operation\":\"{}\""), retained, saved.history_anchor().revision(),
            saved.work().checks, saved.work().reserved_read_bytes, saved.last_operation()));
    }
    bytes.push_str(",\"effect_authority\":\"not_issued\",\"helpers_rerun\":false}\n");
    output.write_all(bytes.as_bytes()).and_then(|()| output.flush()).map_err(debug)
}
