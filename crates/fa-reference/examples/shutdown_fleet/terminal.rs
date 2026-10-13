//! Explicit terminal-clock execution over the original roster and custody path.
use super::*;
use fa_reference::action::consequence::delivery::persistent::{JournalError,
    observed::shutdown::coordinator::FileShutdownClockBinding};
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
pub(super) fn command_with(args: &[String], mut clock: impl FnMut() -> Result<ElapsedTick, String>,
    elapsed: &mut impl FnMut() -> Duration, out: &mut impl Write) -> Result<(), String>
{
    let (mode, prepared, custody) = prepare_command(args, elapsed)?;
    let Prepared { spec, plan, .. } = prepared;
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
        match mode {
            Mode::Create | Mode::Recover => owner.recover_registered_pass_with_clocks(
                revision, &bindings, spec.replay_events, &mut acquire, &mut poll),
            Mode::Resume => owner.resume_registered_pass_with_clocks(
                revision, &bindings, spec.replay_events, &mut acquire, &mut poll),
        }
    }.map_err(|error| format!("fleet pass admission refused at coordinator revision {revision}: {error}; no member visit started"))?;
    report_pass(&owner, &custody, &spec,
        (timed_out.get(), pass.final_revision(), pass.all_observed_drained()),
        || render::encode_clocked(mode.label(), timed_out.get(), &pass), elapsed, out)
}

#[cfg(test)]
#[path = "terminal_tests.rs"] mod tests;
