//! Explicit trusted offline governor workflow, separate from policy replay and
//! human effect approval. The sealed native owner is the only policy writer.
use super::{Config, Duration, Instant, Loaded, anchors, checked, debug, fields, recipe, recovery};
use crate::{config::read_regular, policy_replay::{Request as Candidate, encode}};
use fa_reference::action::consequence::delivery::persistent::{
    governance::PolicyUpdate,
    observed::decoder::learned::{FileLearnedPolicyPromotionHistory,
        FileLearnedPolicyPromotionRequest, FileLearnedPolicyPromotionReview},
};
use fa_reference::strict_json::{self, Json, Limits};
use std::{collections::BTreeSet, fs, io::Write, path::{Path, PathBuf}};

const USAGE: &str = "create-learned-generated CONFIG LEARNED_RECIPE --promote-policy GOVERNOR_REQUEST --history-anchor INPUT --retain-history-anchor NEW_OUTPUT";
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

fn arguments(args: &[String]) -> Result<(), String> {
    if args.len() != 9 || args.first().map(String::as_str) != Some("create-learned-generated")
        || args[3] != "--promote-policy" { return Err(USAGE.into()); }
    for at in [1, 2, 4] { fields::path(&args[at])?; }
    let mut seen = BTreeSet::new();
    for at in [5, 7] {
        if !matches!(args[at].as_str(), "--history-anchor" | "--retain-history-anchor")
            || !seen.insert(args[at].as_str()) { return Err(USAGE.into()); }
        fields::path(&args[at + 1])?;
    }
    Ok(())
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
    arguments(args)?;
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
    check_selection(&loaded)?;
    checkpoint(wall, elapsed)?;
    run(&config, &loaded, &request, candidate, &reviewed, output, elapsed)
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
        || loaded.generation.required_owned_pre_output_forecast().is_some() || loaded.topology.is_some() {
        return Err("offline policy promotion requires a plain nonpredictive learned history".into());
    }
    Ok(())
}

pub(super) fn run(config: &Config, loaded: &Loaded, request: &Request, candidate: Candidate,
    reviewed: &[u8], output: &mut impl Write, elapsed: &mut impl FnMut() -> Duration)
    -> Result<(), String>
{
    checked::check_selection(config, loaded)?;
    check_selection(loaded)?;
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
    let history = run.finish_promoted_policy(&original, request.accept_newly_reviewable)
        .map_err(|error| format!("policy promotion is unconfirmed: {error:?}; retain the exact governor request, reviewed report and old history anchor"))?;
    // An acknowledged transition cannot be retracted by a slow or failing
    // archive/output. Attempt both independently; exact retries append nothing.
    let mut failures = Vec::new();
    let retained = anchors::export_acknowledged(history.history_anchor(),
        loaded.anchor_output.as_deref().ok_or(USAGE)?);
    if let Err(error) = &retained { failures.push(format!("history retention failed: {error}")); }
    let late = elapsed() >= wall;
    if late { failures.push("original policy promotion deadline reached after acknowledgment".into()); }
    if let Err(error) = report(&history, retained.is_ok(), late, output) { failures.push(error); }
    if !late && elapsed() >= wall { failures.push("original policy promotion deadline reached during acknowledged output".into()); }
    if failures.is_empty() { Ok(()) } else { Err(format!(
        "policy promotion acknowledged at revision {}: {}; retry the exact request/report with the independently selected current policy and a new anchor output",
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
    output: &mut impl Write) -> Result<(), String>
{
    let cut = history.snapshot();
    let change = history.receipt().change();
    let mut bytes = format!(concat!(
        "{{\"schema\":\"fa.learned-policy-promotion-result/1\",\"outcome\":\"{}\",",
        "\"operation\":\"{}\",\"reviewed_revision\":\"{}\",\"revision\":\"{}\",",
        "\"appended_events\":\"{}\",\"promoted_policy_generation\":\"{}\",",
        "\"current_policy_generation\":\"{}\",\"control_sequence\":\"{}\",",
        "\"authority_epoch\":\"{}\",\"executions\":\"{}\",\"charged\":\"{}\",",
        "\"reserved\":\"{}\",\"accepted_newly_reviewable\":{},\"cancelled_attempts\":["),
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
    bytes.push_str(&format!(concat!(
        "],\"history_anchor_retained\":{},\"deadline_reached\":{},",
        "\"effect_authority\":\"not_issued\",\"helpers_rerun\":false}}\n"), retained, late));
    output.write_all(bytes.as_bytes()).and_then(|()| output.flush()).map_err(debug)
}
