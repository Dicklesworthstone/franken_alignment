//! Explicit finite-message continuation; no additional prompt or budget input.
use super::{Config, FileSupervisedDriver, FileHumanReviewer, Control, Deadline,
    ElapsedTick, debug, refresh, USAGE};
use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::FileLearnedTextContinuation;

pub(super) fn check_ids(request: u64, after: u64) -> Result<(), String> {
    if request == 0 || after == 0 || request == after {
        return Err("learned message continuation requires a new request and its distinct original predecessor".into());
    }
    Ok(())
}

pub(super) fn take_option(args: &[String]) -> Result<(&[String], Option<u64>), String> {
    let Some(index) = args.iter().position(|value| value == "--after-message") else {
        return Ok((args, None));
    };
    if args.len() != 6 || index != 4 || args[0] != "create-learned-generated"
        || args[3].starts_with("--") || args[5].is_empty()
        || !args[5].bytes().all(|value| value.is_ascii_digit()) { return Err(USAGE.into()); }
    let previous = args[5].parse::<u64>().map_err(|_| USAGE.to_owned())?;
    if previous == 0 { return Err(USAGE.into()); }
    Ok((&args[..4], Some(previous)))
}

/// Resume and record the original finite-message intent after fresh source and
/// cooperative stop checks. Shared by the executable and loaded-owner tests;
/// generation, native review and human approval remain in their original paths.
pub(super) fn prepare<F>(driver: &mut FileSupervisedDriver, reviewer: &FileHumanReviewer,
    config: &mut Config, command: FileLearnedTextContinuation, control: &mut Control,
    deadline: &Deadline, time: &mut F) -> Result<bool, String>
where F: FnMut() -> ElapsedTick {
    if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(false); }
    refresh(driver, config, deadline, time)?;
    if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(false); }
    deadline.check(time())?;
    let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
    let current = host.learned_generation_inspection().map_err(debug)?;
    if current.paused {
        let revision = host.revision();
        host.resume_learned_generation(revision, current.numerical.actor_revision,
            current.numerical.position).map_err(debug)?;
    }
    let revision = host.revision();
    host.begin_learned_text_stream_continuation(revision, command).map_err(debug)?;
    Ok(true)
}
