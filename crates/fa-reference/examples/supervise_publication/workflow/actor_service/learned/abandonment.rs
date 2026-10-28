//! Explicit disposition of an original learned Unknown liability. Complete
//! numerical replay and independently selected guards remain mandatory; this
//! workflow cannot obtain a live owner, clock, source observer or sendable action.
use super::{
    Config, Duration, Instant, Loaded, anchors, checked, credential, fields, recipe, recovery,
    topology,
};
use crate::config::read_regular;
use crate::recover_stop::guarded::abandonment::decode_request;
use fa_reference::action::ActionState;
use fa_reference::action::consequence::delivery::persistent::{
    abandonment::FileAbandonRequest, observed::guarded::recovery::FileAbandonedHistory,
};
use std::{
    collections::BTreeSet,
    io::Write,
    path::{Path, PathBuf},
};

#[path = "abandonment/custody.rs"]
mod custody;

const USAGE: &str = "create-learned-generated CONFIG LEARNED_RECIPE --abandon-unknown REQUEST_JSON --history-anchor INPUT --retain-history-anchor NEW_OUTPUT [--topology-profile PATH --retain-topology-checkpoint NEW_OUTPUT]";
const REQUEST_BYTES: usize = 4096;

pub(super) fn command(args: &[String], credibility: Option<&Path>) -> Result<(), String> {
    let started = Instant::now();
    command_with(
        args,
        credibility,
        &mut || started.elapsed(),
        &mut std::io::stdout().lock(),
    )
}

/// Check the entire original grammar before removing anything. An option may
/// never fill a neighboring missing value or disappear into another workflow.
fn arguments(args: &[String]) -> Result<(Vec<String>, Option<PathBuf>), String> {
    if args.len() < 9
        || args.first().map(String::as_str) != Some("create-learned-generated")
        || args.get(3).map(String::as_str) != Some("--abandon-unknown")
        || args[1..3]
            .iter()
            .any(|value| value.is_empty() || value.starts_with("--"))
    {
        return Err(USAGE.into());
    }
    fields::path(&args[4])?;
    let mut seen = BTreeSet::new();
    let mut remaining = args[..5].to_vec();
    let mut source = None;
    let mut at = 5;
    while at < args.len() {
        let flag = args[at].as_str();
        if !matches!(
            flag,
            "--history-anchor"
                | "--retain-history-anchor"
                | "--topology-profile"
                | "--retain-topology-checkpoint"
        ) || !seen.insert(flag)
        {
            return Err(USAGE.into());
        }
        let value = args.get(at + 1).ok_or(USAGE)?;
        if value.is_empty() || value.starts_with("--") {
            return Err(format!("{flag} requires an immediate non-option path"));
        }
        let path = fields::path(value)?;
        if flag == "--retain-topology-checkpoint" {
            source = Some(path);
        } else {
            remaining.extend([args[at].clone(), value.clone()]);
        }
        at += 2;
    }
    if !seen.contains("--history-anchor")
        || !seen.contains("--retain-history-anchor")
        || seen.contains("--topology-profile") != source.is_some()
    {
        return Err(USAGE.into());
    }
    Ok((remaining, source))
}

/// Production and deterministic tests share the same admission, complete replay
/// and post-acknowledgment custody boundary. Elapsed time is never effect time.
pub(super) fn command_with(
    args: &[String],
    credibility: Option<&Path>,
    elapsed: &mut impl FnMut() -> Duration,
    out: &mut impl Write,
) -> Result<(), String> {
    if credibility.is_some() {
        return Err("learned abandonment cannot activate credibility".into());
    }
    let (args, source_output) = arguments(args)?;
    let (args, options) = anchors::take_options(&args)?;
    let (args, topology_path) = topology::take_option(&args)?;
    if args.len() != 5 {
        return Err(USAGE.into());
    }
    let config = Config::read(Path::new(&args[1]))?;
    let wall = Duration::from_millis(config.timing.runtime_ms);
    check_elapsed(elapsed, wall)?;
    let request = decode_request(&read_regular(Path::new(&args[4]), REQUEST_BYTES)?)?;
    check_elapsed(elapsed, wall)?;
    // Original models, tokenizer, monitor, predictor, identity, credential and
    // joint-policy contracts are admitted. Historical mode reads no live helper
    // roster, provider secret, source inventory or current evidence file.
    let mut loaded = recipe::load(Path::new(&args[2]), &config, false)?;
    credential::check_selection(&loaded)?;
    checked::check_selection(&config, &loaded)?;
    check_elapsed(elapsed, wall)?;
    options.admit(&config, &mut loaded, true)?;
    check_elapsed(elapsed, wall)?;
    topology::admit(
        &config,
        &mut loaded,
        topology_path.as_deref(),
        topology::Mode::Historical,
    )?;
    check_elapsed(elapsed, wall)?;
    let outputs = custody::admit(&config, &loaded, source_output.as_deref())?;
    check_elapsed(elapsed, wall)?;
    run(&config, &loaded, request, &outputs, elapsed, out)
}

fn check_elapsed(elapsed: &mut impl FnMut() -> Duration, wall: Duration) -> Result<(), String> {
    if elapsed() >= wall {
        Err("learned abandonment deadline reached before acknowledgment".into())
    } else {
        Ok(())
    }
}

fn run(
    config: &Config,
    loaded: &Loaded,
    request: FileAbandonRequest,
    outputs: &custody::Outputs,
    elapsed: &mut impl FnMut() -> Duration,
    out: &mut impl Write,
) -> Result<(), String> {
    let wall = Duration::from_millis(config.timing.runtime_ms);
    let history = recovery::abandon_selected_with_checkpoints(config, loaded, request,
        &mut |_| check_elapsed(elapsed, wall))
        .map_err(|error| format!("native learned abandonment is unconfirmed: {error}; inspect canonical history before choosing a new request"))?;
    let mut late = check_elapsed(elapsed, wall).is_err();
    // Both independent exports precede the final report, even after a late
    // deadline or an export failure. Nothing after acknowledgment undoes it.
    let retained = outputs.retain(config, loaded, &history);
    late |= check_elapsed(elapsed, wall).is_err();
    let reported = report(&history, &retained, out);
    late |= check_elapsed(elapsed, wall).is_err();
    let mut failures = retained.failures();
    if late {
        failures.push("deadline reached after native acknowledgment".into());
    }
    if let Err(error) = reported {
        failures.push(format!("report output failed: {error}"));
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "native learned abandonment acknowledged at revision {}: {}; the liability remains charged; retain the input archives and retry only the exact original request with new output paths",
            history.snapshot().revision,
            failures.join("; ")
        ))
    }
}

fn report(
    history: &FileAbandonedHistory,
    retained: &custody::Retention,
    out: &mut impl Write,
) -> Result<(), String> {
    let receipt = history.receipt();
    let request = receipt.request();
    let state = history.snapshot();
    let ids = |irrecoverable_only: bool| {
        state
            .control
            .ledger
            .stages
            .iter()
            .filter_map(|(id, stage)| {
                (if irrecoverable_only {
                    *stage == ActionState::IrrecoverablyUnknown
                } else {
                    matches!(
                        stage,
                        ActionState::Dispatching
                            | ActionState::Unknown
                            | ActionState::IrrecoverablyUnknown
                    )
                })
                .then(|| format!("\"{id}\""))
            })
            .collect::<Vec<_>>()
            .join(",")
    };
    // Fixed labels and bounded numeric data only: no action payload, source
    // documents, credential material, model tensors or archive bytes on stdout.
    let text = format!(
        concat!(
            "{{\"schema\":\"fa.learned-abandon-unknown-result/1\",\"knowledge\":{{\"kind\":\"known\",",
            "\"basis\":{{\"class\":\"bounded_model\",\"scope\":\"acknowledged_reference_disposition\",",
            "\"authentic_origin\":\"unknown\",\"source_truth\":\"not_established\",",
            "\"verified_journal_revision\":\"{}\"}},\"value\":{{",
            "\"status\":\"irrecoverably_unknown\",\"disposition_recorded\":true,",
            "\"effect_outcome\":{{\"kind\":\"unknown\",\"reason\":\"OutcomeUnknown\"}},",
            "\"operation\":\"{}\",\"attempt\":\"{}\",\"revision\":\"{}\",",
            "\"control_sequence\":\"{}\",\"authority_epoch\":\"{}\",\"charged\":\"{}\",",
            "\"abandoned_units\":\"{}\",\"executions\":\"{}\",\"history_anchor_retained\":{},",
            "\"topology_checkpoint_retained\":{},\"unresolved\":[{}],\"irrecoverable\":[{}]}}}},\"authority\":\"none\"}}\n"
        ),
        state.revision,
        request.operation,
        request.attempt,
        state.revision,
        state.control.sequence,
        state.control.ledger.epoch,
        state.control.ledger.charged,
        receipt.charged_units(),
        state.executions,
        retained.history.is_ok(),
        retained
            .source
            .as_ref()
            .map_or("null", |result| if result.is_ok() {
                "true"
            } else {
                "false"
            }),
        ids(false),
        ids(true)
    );
    out.write_all(text.as_bytes())
        .and_then(|_| out.flush())
        .map_err(super::debug)
}
