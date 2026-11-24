//! Explicit bounded evaluation of original learned history after a restart.
//! Replay verifies numerical provenance once. This path never creates an actor
//! listener, resumes generation, reads live evidence, offers review or dispatches.
use super::{
    Config, Control, Deadline, ElapsedTick, FileSupervisedDriver, Loaded, Options,
    PeerProfile, Profile, Selection, USAGE, debug, evaluator, ingress,
};
use super::super::{anchors, checked, cleanup, finish, plus, recipe, startup_custody};
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub(in super::super) fn command<F>(
    args: &[String],
    options: Options,
    time: F,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    if !options.evaluate_only || options.history_input.is_none() {
        return Err(USAGE.into());
    }
    // The complete opted-in grammar was checked before this path was selected.
    // The actor profile identifies the original audience only; no actor binds.
    let (args, actor_path) = ingress::take_option(args)?;
    let actor_path = actor_path.ok_or(USAGE)?;
    let (args, anchor_options) = anchors::take_options(&args)?;
    let (args, finish_after) = finish::take_option(&args)?;
    if args.len() != 4 || args[0] != "create-learned-generated" {
        return Err(USAGE.into());
    }
    let config = Config::read_learned_recovery(Path::new(&args[1]))?;
    let reviewer = PeerProfile::read(Path::new(&args[3]))?;
    let actor = Profile::read(actor_path)?;
    // Historical loading binds the original model/codec/recipe while leaving
    // present policy evidence, native helper weights and credentials unopened.
    let mut loaded = recipe::load(Path::new(&args[2]), &config, false)?;
    anchor_options.admit(&config, &mut loaded, true)?;
    let selected = options.read()?;
    selected.check_inputs(
        &loaded,
        &[
            Path::new(&args[1]),
            Path::new(&args[2]),
            Path::new(&args[3]),
            actor_path,
        ],
    )?;
    run(config, loaded, &reviewer, &actor, &selected, finish_after, time)
}

fn run<F>(
    config: Config,
    loaded: Loaded,
    reviewer: &PeerProfile,
    actor: &Profile,
    selected: &Selection,
    finish_after: Option<u64>,
    mut time: F,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    reviewer.check_host(&config)?;
    ingress::check(&config, &loaded, reviewer, actor)?;
    checked::check_selection(&config, &loaded)?;
    if let Some(after) = finish_after {
        finish::check_ids(loaded.request, after)?;
    }
    selected.check(
        &config,
        &loaded,
        Some(actor),
        reviewer,
        (None, None),
        true,
        finish_after,
        None,
        None,
    )?;
    let start = time();
    // This is a fresh finite assessment transport, never a renewed action
    // deadline or numerical allowance. Replay spends this same remaining lease.
    let lifetime = loaded.ttl_ms.min(config.timing.runtime_ms).min(actor.runtime_ms);
    let deadline = Deadline {
        logical: plus(start, lifetime)?,
        started: Instant::now(),
        wall: Duration::from_millis(lifetime),
    };
    deadline.check(start)?;
    // Original independent Stop is admitted before replay and remains serviced
    // through the final evaluator quantum. No human approval session is opened.
    let mut control = Control::new(&config, loaded.request, Some(reviewer))?;
    let ((host, human, _observer, _identity), role) = selected.open_history(
        &config, &loaded, &mut control, &deadline, &mut time, finish_after,
    )?;
    let custody = startup_custody::Captured::capture(&loaded, true, &host, None, None)?;
    let retain_startup = |error| custody.retain_error(error);
    let (unused_actor, supervisor) = host
        .into_learned_text_stream_actor_gateway()
        .map_err(debug)
        .map_err(retain_startup)?;
    drop(unused_actor);
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut service = evaluator::Service::start(&selected.peer, actor, role, &driver, &deadline)
        .map_err(retain_startup)?;
    let mut failure = None;
    super::complete(
        &mut service,
        &mut driver,
        &human,
        &mut control,
        &deadline,
        &mut time,
        &mut None,
        &mut None,
        config.timing.poll_ms,
        &mut failure,
    );
    // No retained output can predate a successful assessment or acknowledged
    // Stop. Attempt custody retention even when the service reports a failure.
    let failure = anchors::retain(&loaded, &driver, failure);
    let pending = cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms);
    if let Some(error) = failure {
        return Err(error);
    }
    if pending != 0 {
        return Err("historical learned evaluation cleanup remains pending".into());
    }
    Ok(())
}
