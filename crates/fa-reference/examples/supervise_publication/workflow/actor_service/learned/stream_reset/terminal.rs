//! Recover paired source custody for an already recorded stream reset and Stop.
//! The sealed native terminal cursor can only confirm the selected existing Stop.
use super::{
    AcknowledgedOutput, Config, Deadline, Duration, ElapsedTick, Instant, Loaded, Request, SourceRetention,
    anchors, check_selection, checked, fields, plus, read_request, recipe,
    recovery, topology,
};
use std::{collections::BTreeSet, io::Write, path::Path};

const USAGE: &str = "create-learned-generated CONFIG LEARNED_RECIPE --recover-stopped-stream-reset RESET_REQUEST STOP_OPERATION --history-anchor INPUT --retain-history-anchor NEW_OUTPUT --topology-profile TOPOLOGY_PROFILE";

fn arguments(args: &[String]) -> Result<u64, String> {
    if args.len() != 12
        || args.first().map(String::as_str) != Some("create-learned-generated")
        || args.get(3).map(String::as_str) != Some("--recover-stopped-stream-reset")
    {
        return Err(USAGE.into());
    }
    for index in [1, 2, 4] {
        fields::path(&args[index])?;
    }
    let operation = &args[5];
    if operation.is_empty()
        || operation.starts_with('0')
        || !operation.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err("stopped stream-reset recovery requires a nonzero canonical Stop operation".into());
    }
    let operation = operation.parse::<u64>().map_err(|_| USAGE.to_owned())?;
    let mut seen = BTreeSet::new();
    for pair in args[6..].chunks_exact(2) {
        let flag = pair[0].as_str();
        if !matches!(flag, "--history-anchor" | "--retain-history-anchor" | "--topology-profile")
            || !seen.insert(flag)
        {
            return Err(USAGE.into());
        }
        fields::path(&pair[1])?;
    }
    if seen.len() != 3 { return Err(USAGE.into()); }
    Ok(operation)
}

pub(in super::super) fn command<F>(
    args: &[String],
    credibility: Option<&Path>,
    mut time: F,
    output: &mut impl Write,
) -> Result<(), String>
where F: FnMut() -> ElapsedTick {
    let started = Instant::now();
    if credibility.is_some() {
        return Err("stopped stream-reset custody recovery cannot activate credibility".into());
    }
    let operation = arguments(args)?;
    let (args, topology_profile) = topology::take_option(args)?;
    let (args, options) = anchors::take_options(&args)?;
    let start = time();
    let config = Config::read_learned_recovery(Path::new(&args[1]))?;
    let deadline = Deadline {
        logical: plus(start, config.timing.runtime_ms)?,
        started,
        wall: Duration::from_millis(config.timing.runtime_ms),
    };
    deadline.check(time())?;
    let request = read_request(&config, Path::new(&args[4]))?;
    deadline.check(time())?;
    let mut loaded = recipe::load(Path::new(&args[2]), &config, false)?;
    checked::check_selection(&config, &loaded)?;
    deadline.check(time())?;
    options.admit(&config, &mut loaded, true)?;
    // Live here admits the two new archive destinations; it does not read next
    // source files, create a live owner, or certify a fresh topology cut.
    topology::admit(&config, &mut loaded, topology_profile.as_deref(), topology::Mode::Live)?;
    deadline.check(time())?;
    run(&config, &loaded, &request, operation, &deadline, &mut time, output)
}

pub(in super::super) fn run<F>(
    config: &Config,
    loaded: &Loaded,
    request: &Request,
    operation: u64,
    deadline: &Deadline,
    time: &mut F,
    output: &mut impl Write,
) -> Result<(), String>
where F: FnMut() -> ElapsedTick {
    checked::check_selection(config, loaded)?;
    check_selection(loaded)?;
    if operation == 0 {
        return Err("stopped stream-reset recovery requires a nonzero Stop operation".into());
    }
    let history_output = AcknowledgedOutput::admit(config, loaded.anchor_output.as_deref()
        .ok_or("stopped stream-reset custody recovery requires a new history output")?)?;
    let source = SourceRetention::admit(config, loaded)?
        .ok_or("stopped stream-reset custody recovery requires an exact paired v2 source checkpoint")?;
    if history_output.path() == source.archive.path() {
        return Err("history anchor and topology checkpoint outputs must be distinct new files".into());
    }
    let stopped = recovery::stream_reset::recover_stopped(
        config, loaded, request, &source, operation, deadline, time,
    ).map_err(|error| format!(
        "stopped stream-reset custody recovery is unconfirmed: {error}; preserve the original paired archives and canonical stopped history"
    ))?;

    // Both exports describe this same already acknowledged terminal cut.
    // A late deadline or failed output cannot suppress the other export or
    // cause a reset, Stop, Fence, settlement, or live-owner retry.
    let retained = source.retain_stop(config, loaded, &stopped, request);
    let mut failures = Vec::new();
    if let Err(error) = retained { failures.push(error); }
    let late = deadline.check(time()).is_err();
    if late { failures.push("original custody recovery deadline reached after confirmation".into()); }
    if let Err(error) = crate::recover_stop::report_admission(&stopped, output) {
        failures.push(error);
    }
    if deadline.check(time()).is_err() && !late {
        failures.push("original custody recovery deadline reached during acknowledged output".into());
    }
    if failures.is_empty() { Ok(()) } else {
        Err(format!(
            "native Stop {} confirmed at revision {}: {}; preserve the original reset expectation and paired custody; select new output paths for an exact custody retry",
            operation, stopped.snapshot().revision, failures.join("; "),
        ))
    }
}
