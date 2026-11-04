//! Exact checkpoint capture, prompt containment and stream-reset initiation.
//! The sealed native finalizer returns only acknowledged history and no roles.
use super::{Config, Deadline, Duration, ElapsedTick, Instant, Loaded, anchors,
    checked, debug, fields, plus, recipe, recovery, stream_reset};
use crate::config::read_regular;
use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::{
    FileLearnedStreamCheckpointRequest, FileLearnedStreamNarrowRequest, FileLearnedStreamControlHistory,
    FileLearnedStreamControlReceipt,
};
use std::{collections::BTreeSet, fs, io::Write, path::Path};

#[path = "stream_checkpoint/request.rs"]
mod request;
#[path = "stream_checkpoint/narrow.rs"]
mod narrow;

const USAGE: &str = "create-learned-generated CONFIG LEARNED_RECIPE (--capture-stream-checkpoint CHECKPOINT_REQUEST | --narrow-stream-checkpoint NARROW_REQUEST | --begin-stream-reset RESET_REQUEST) --history-anchor INPUT --retain-history-anchor NEW_OUTPUT";
const REQUEST_BYTES: usize = 262_144;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Request {
    Capture(FileLearnedStreamCheckpointRequest),
    Narrow(FileLearnedStreamNarrowRequest),
    Begin(stream_reset::Request),
}

pub(super) fn decode_capture(bytes: &[u8]) -> Result<FileLearnedStreamCheckpointRequest, String> {
    request::decode(bytes)
}

pub(super) fn decode_narrow(bytes: &[u8]) -> Result<FileLearnedStreamNarrowRequest, String> {
    narrow::decode(bytes)
}

fn arguments(args: &[String]) -> Result<(), String> {
    if args.len() != 9 || args.first().map(String::as_str) != Some("create-learned-generated")
        || !matches!(args.get(3).map(String::as_str),
            Some("--capture-stream-checkpoint" | "--narrow-stream-checkpoint" | "--begin-stream-reset")) {
        return Err(USAGE.into());
    }
    for index in [1, 2, 4] { fields::path(&args[index])?; }
    let mut seen = BTreeSet::new();
    for index in [5, 7] {
        if !matches!(args[index].as_str(), "--history-anchor" | "--retain-history-anchor")
            || !seen.insert(args[index].as_str()) { return Err(USAGE.into()); }
        fields::path(&args[index + 1])?;
    }
    Ok(())
}

pub(super) fn command<F>(args: &[String], credibility: Option<&Path>, mut time: F,
    output: &mut impl Write) -> Result<(), String>
where F: FnMut() -> ElapsedTick {
    let started = Instant::now();
    if credibility.is_some() { return Err("stream checkpoint control cannot activate credibility".into()); }
    arguments(args)?;
    let (args, options) = anchors::take_options(args)?;
    let start = time();
    let config = Config::read_learned_stream_reset(Path::new(&args[1]))?;
    let deadline = Deadline { logical: plus(start, config.timing.runtime_ms)?,
        started, wall: Duration::from_millis(config.timing.runtime_ms) };
    deadline.check(time())?;
    let path = Path::new(&args[4]);
    if fs::canonicalize(path).map_err(debug)?.starts_with(fs::canonicalize(&config.store).map_err(debug)?) {
        return Err("stream checkpoint/reset expectation must be retained outside the authority store".into());
    }
    let bytes = read_regular(path, REQUEST_BYTES)?;
    let request = match args[3].as_str() {
        "--capture-stream-checkpoint" => Request::Capture(decode_capture(&bytes)?),
        "--narrow-stream-checkpoint" => Request::Narrow(decode_narrow(&bytes)?),
        "--begin-stream-reset" => Request::Begin(stream_reset::decode(&bytes)?),
        _ => return Err(USAGE.into()),
    };
    deadline.check(time())?;
    // Historical selection pins all original native contracts without reading
    // current helper/source files or constructing a publication service.
    let mut loaded = recipe::load(Path::new(&args[2]), &config, false)?;
    checked::check_selection(&config, &loaded)?;
    options.admit(&config, &mut loaded, true)?;
    check_request_selection(&loaded, &request)?;
    deadline.check(time())?;
    run(&config, &loaded, &request, &deadline, &mut time, output)
}

pub(super) fn check_selection(loaded: &Loaded) -> Result<(), String> {
    if loaded.generation.text_stream_profile().is_none() {
        return Err("stream checkpoint control requires the original learned text stream".into());
    }
    if loaded.generation.required_pre_output_forecast().is_some()
        || (loaded.generation.required_owned_pre_output_forecast().is_some()
            && !loaded.generation.requires_owned_forecast_stream_restart_audit()) {
        return Err("predictive stream checkpoint control requires the explicit owned prompt-audit reset recipe".into());
    }
    if loaded.topology.is_some() {
        return Err("stream checkpoint control requires a plain history; topology custody is unsupported".into());
    }
    if loaded.recovery_anchor.is_none() || loaded.anchor_output.is_none() {
        return Err("stream checkpoint control requires an independent input history anchor and a new retained output".into());
    }
    Ok(())
}

pub(super) fn check_request_selection(loaded: &Loaded, request: &Request) -> Result<(), String> {
    check_selection(loaded)?;
    if matches!(request, Request::Narrow(_))
        && (loaded.generation.required_owned_pre_output_forecast().is_none()
            || !loaded.generation.requires_owned_forecast_stream_restart_audit()) {
        return Err("stream checkpoint narrowing requires the explicit owned prompt-audit reset recipe".into());
    }
    Ok(())
}

pub(super) fn run<F>(config: &Config, loaded: &Loaded, request: &Request,
    deadline: &Deadline, time: &mut F, output: &mut impl Write) -> Result<(), String>
where F: FnMut() -> ElapsedTick {
    checked::check_selection(config, loaded)?;
    check_request_selection(loaded, request)?;
    let acknowledged = recovery::stream_checkpoint::execute(config, loaded, request,
        &mut |_| deadline.check(time())).map_err(|error| format!(
            "native stream checkpoint control is unconfirmed: {error}; preserve the original independent expectation and input history anchor"))?;
    // A late deadline, failed archive or broken output cannot retract native
    // acknowledgment. Attempt retention and reporting independently, once.
    let mut failures = Vec::new();
    let retained = anchors::export_acknowledged(acknowledged.history_anchor(),
        loaded.anchor_output.as_deref().ok_or(USAGE)?);
    if let Err(error) = &retained { failures.push(format!("history anchor retention failed: {error}")); }
    let late = deadline.check(time()).is_err();
    if late { failures.push("original control deadline reached after acknowledgment".into()); }
    if let Err(error) = report(&acknowledged, retained.is_ok(), late, output) { failures.push(error); }
    if !late && deadline.check(time()).is_err() {
        failures.push("original control deadline reached during acknowledged output".into());
    }
    if failures.is_empty() { Ok(()) } else { Err(format!(
        "native stream checkpoint control acknowledged at revision {}: {}; preserve the exact expectation and old archive; retry with a new output path without ordinary recovery fencing the reset intent",
        acknowledged.snapshot().revision, failures.join("; "))) }
}

fn report(history: &FileLearnedStreamControlHistory, retained: bool, late: bool,
    output: &mut impl Write) -> Result<(), String>
{
    let (outcome, checkpoint, operation, first) = match history.receipt() {
        FileLearnedStreamControlReceipt::Captured(info) => (
            if history.appended_events() == 0 { "already_captured" } else { "captured" },
            info.checkpoint, "null".to_owned(), "null".to_owned()),
        FileLearnedStreamControlReceipt::Narrowed { request, receipt } => (
            if history.appended_events() == 0 { "already_narrowed" } else { "narrowed" },
            request.checkpoint().checkpoint, format!("\"{}\"", request.control().operation),
            // A later exact retry may observe a newer current control cut. The
            // original receipt, never that newer cut, supplies this frontier.
            format!("\"{}\"", receipt.sequence)),
        FileLearnedStreamControlReceipt::ResetBegun { intent, first_incident_sequence, already_completed } => (
            if *already_completed { "already_completed" } else if history.appended_events() == 0 {
                "already_pending"
            } else { "pending" }, intent.checkpoint(), format!("\"{}\"", intent.control().operation),
            format!("\"{first_incident_sequence}\"")),
    };
    let state = history.snapshot();
    let numerical = history.numerical();
    let usage = history.recovery_usage();
    let text = format!(concat!(
        "{{\"schema\":\"fa.learned-stream-control-result/1\",\"outcome\":\"{}\",",
        "\"checkpoint\":\"{}\",\"operation\":{},\"first_incident_sequence\":{},",
        "\"revision\":\"{}\",\"appended_events\":\"{}\",\"control_sequence\":\"{}\",",
        "\"authority_epoch\":\"{}\",\"actor_revision\":\"{}\",\"position\":\"{}\",",
        "\"sampled_draws\":\"{}\",\"charged\":\"{}\",\"reserved\":\"{}\",",
        "\"executions\":\"{}\",\"checkpoints\":\"{}\",\"restart_attempts\":\"{}\",",
        "\"history_anchor_retained\":{},\"deadline_reached\":{},\"authority\":\"none\"}}\n"),
        outcome, checkpoint, operation, first, state.revision, history.appended_events(),
        state.control.sequence, state.control.ledger.epoch, numerical.actor_revision,
        numerical.position, numerical.sampled_draws, state.control.ledger.charged,
        state.control.ledger.reserved, state.executions, usage.checkpoints, usage.restart_attempts,
        retained, late);
    output.write_all(text.as_bytes()).and_then(|()| output.flush()).map_err(debug)
}
