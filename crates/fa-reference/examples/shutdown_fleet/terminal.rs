//! Explicit terminal-clock execution over the original roster and custody path.
use super::*;
use fa_reference::action::consequence::delivery::persistent::{JournalError,
    observed::shutdown::coordinator::{FileShutdownClockBinding, FileShutdownLearnedPass}};
use std::cell::{Cell, RefCell};

pub(super) fn take_flag(args: &[String]) -> Result<Option<Vec<String>>, String> {
    let positions: Vec<_> = args.iter().enumerate().skip(2)
        .filter_map(|(index, arg)| (arg == "--terminal-clocks").then_some(index)).collect();
    match positions.as_slice() {
        [] => Ok(None),
        [index] => { let mut clean = args.to_vec(); clean.remove(*index); Ok(Some(clean)) }
        _ => Err("duplicate --terminal-clocks option".into()),
    }
}

// The executable and real-file tests use this same entry point. A caller cannot
// supply timestamps to the roster: only the bound native terminal callback can
// acquire one. Legacy no-flag invocation retains its separate supplied-time API.
pub(super) fn command_with(args: &[String], clock: impl FnMut() -> Result<ElapsedTick, String>,
    elapsed: &mut impl FnMut() -> Duration, out: &mut impl Write) -> Result<(), String>
{
    command_for_phase(args, clock, elapsed, out, None)
}

pub(super) fn command_for_phase(args: &[String], clock: impl FnMut() -> Result<ElapsedTick, String>,
    elapsed: &mut impl FnMut() -> Duration, out: &mut impl Write, phase: Option<phases::Phase>) -> Result<(), String>
{
    command_for_selection(args, clock, elapsed, out, phase, None)
}

pub(super) fn command_for_selection(args: &[String], clock: impl FnMut() -> Result<ElapsedTick, String>,
    elapsed: &mut impl FnMut() -> Duration, out: &mut impl Write,
    phase: Option<phases::Phase>, selected: Option<&[u64]>) -> Result<(), String>
{
    command_for_selection_with_learned(args, clock, elapsed, out, phase, selected, None)
}

pub(super) fn command_for_selection_with_learned(args: &[String],
    mut clock: impl FnMut() -> Result<ElapsedTick, String>, elapsed: &mut impl FnMut() -> Duration,
    out: &mut impl Write, phase: Option<phases::Phase>, selected: Option<&[u64]>,
    loader: Option<&mut LearnedLoader<'_>>) -> Result<(), String>
{
    if selected.is_some() && args.first().map(String::as_str) != Some("recover") {
        return Err("domain selection requires explicit recover, not create or resume".into());
    }
    let numerical = loader.is_some();
    let (mode, prepared, custody) = prepare_command_with_learned(args, elapsed, loader)?;
    let Prepared { spec, plan, learned, .. } = prepared;
    if let Some(domains) = selected { selected::check_plan(&plan, domains)?; }
    let bindings: Vec<_> = plan.domains().iter().map(|domain| FileShutdownClockBinding {
        domain: domain.id(), clock_domain: domain.clock_domain(),
    }).collect();
    checkpoint(spec.wall, elapsed)?;
    let mut owner = acquire_coordinator(&spec, &plan, &custody, mode)?;
    let revision = owner.revision();
    let timed_out = Cell::new(false);
    let pass = {
        // Both callbacks share ONE elapsed source/budget, not a new stopwatch per
        // member. Borrow guards end before invoking the independently supplied
        // clock; no mutable borrow of elapsed crosses an external clock call.
        let shared = RefCell::new(&mut *elapsed);
        let within_budget = || {
            let expired = (**shared.borrow_mut())() >= spec.wall;
            if expired { timed_out.set(true); }
            !expired
        };
        let mut acquire = |_binding| {
            if !within_budget() { return Err(JournalError::Unavailable); }
            let tick = clock().map_err(|_| JournalError::Unavailable)?;
            // A slow clock must not acquire eligibility past the original budget.
            if !within_budget() { return Err(JournalError::Unavailable); }
            Ok(tick)
        };
        let mut poll = |_| within_budget();
        if numerical {
            owner.run_registered_learned_pass(revision, &bindings, &learned,
                learned_selection(mode, phase, selected), spec.replay_events, &mut acquire, &mut poll)
        } else if let Some(domains) = selected {
            match phase {
                None => owner.recover_selected_pass_with_clocks(revision, &bindings, domains,
                    spec.replay_events, &mut acquire, &mut poll),
                Some(phases::Phase::Admission) => owner.stop_selected_admission(revision, &bindings,
                    domains, spec.replay_events, &mut poll),
                Some(phases::Phase::Drain) => owner.drain_selected_stopped_with_clocks(revision,
                    &bindings, domains, spec.replay_events, &mut acquire, &mut poll),
            }
        } else {
            match (phase, mode) {
                (None, Mode::Create | Mode::Recover) => owner.recover_registered_pass_with_clocks(
                    revision, &bindings, spec.replay_events, &mut acquire, &mut poll),
                (None, Mode::Resume) => owner.resume_registered_pass_with_clocks(
                    revision, &bindings, spec.replay_events, &mut acquire, &mut poll),
                (Some(phases::Phase::Admission), Mode::Create | Mode::Recover) => owner.stop_registered_admission(
                    revision, &bindings, spec.replay_events, &mut poll),
                (Some(phases::Phase::Admission), Mode::Resume) => owner.resume_registered_admission(
                    revision, &bindings, spec.replay_events, &mut poll),
                (Some(phases::Phase::Drain), Mode::Create | Mode::Recover) => owner.drain_registered_stopped_with_clocks(
                    revision, &bindings, spec.replay_events, &mut acquire, &mut poll),
                (Some(phases::Phase::Drain), Mode::Resume) => owner.resume_registered_stopped_with_clocks(
                    revision, &bindings, spec.replay_events, &mut acquire, &mut poll),
            }
        }
    }.map_err(|error| format!("fleet pass admission refused at coordinator revision {revision}: {error}; no member visit started"))?;
    let complete = if phase == Some(phases::Phase::Admission) { pass.all_observed_stopped() }
        else { pass.all_observed_drained() };
    report_pass(&owner, &custody, &spec,
        (timed_out.get(), pass.final_revision(), complete),
        || match selected {
            Some(domains) => render::encode_selected(mode.label(), phase, domains, timed_out.get(), &pass),
            None => match phase {
                Some(phase) => render::encode_phased(mode.label(), phase, timed_out.get(), &pass),
                None => render::encode_clocked(mode.label(), timed_out.get(), &pass),
            },
        }, elapsed, out)
}

fn learned_selection<'a>(mode: Mode, phase: Option<phases::Phase>, selected: Option<&'a [u64]>)
    -> FileShutdownLearnedPass<'a>
{
    use FileShutdownLearnedPass as Pass;
    match (selected, phase, mode) {
        (Some(domains), None, _) => Pass::RecoverSelected(domains),
        (Some(domains), Some(phases::Phase::Admission), _) => Pass::StopSelected(domains),
        (Some(domains), Some(phases::Phase::Drain), _) => Pass::DrainSelected(domains),
        (None, None, Mode::Resume) => Pass::Resume,
        (None, None, _) => Pass::Recover,
        (None, Some(phases::Phase::Admission), Mode::Resume) => Pass::ResumeAdmission,
        (None, Some(phases::Phase::Admission), _) => Pass::StopAdmission,
        (None, Some(phases::Phase::Drain), Mode::Resume) => Pass::ResumeStopped,
        (None, Some(phases::Phase::Drain), _) => Pass::DrainStopped,
    }
}

#[cfg(test)]
#[path = "terminal_tests.rs"] mod tests;
