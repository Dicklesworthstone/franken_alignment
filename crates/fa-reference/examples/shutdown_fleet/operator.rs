//! One explicitly selected fixed-roster pass. Never auto-retry an ambiguous write.
#[path = "profile.rs"] mod profile;
#[path = "render.rs"] mod render;
#[cfg(test)]
#[path = "tests.rs"] mod tests;
use profile::{Prepared, Spec, custody::{Options, Custody}};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::shutdown::coordinator::{
    FileShutdownClock, FileShutdownCoordinator};
use std::{io::Write, path::Path, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};

const USAGE: &str = "usage: shutdown_fleet create|recover|resume FLEET_PROFILE [--coordinator-anchor FILE] [--retain-coordinator-anchor NEW_FILE]";
const DEADLINE: &str = "fleet recovery deadline reached; not all registered domains are confirmed drained";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode { Create, Recover, Resume }
impl Mode {
    fn parse(args: &[String]) -> Result<Self, String> {
        if args.len() != 2 { return Err(USAGE.into()); }
        match args[0].as_str() {
            "create" => Ok(Self::Create), "recover" => Ok(Self::Recover), "resume" => Ok(Self::Resume),
            _ => Err(USAGE.into()),
        }
    }
    fn label(self) -> &'static str {
        match self { Self::Create => "create", Self::Recover => "recover", Self::Resume => "resume" }
    }
}

pub(crate) fn command(args: &[String], out: &mut impl Write) -> Result<(), String> {
    let started = Instant::now();
    command_with(args, || {
        let duration = SystemTime::now().duration_since(UNIX_EPOCH)
            .map_err(|error| format!("Unix clock unavailable: {error}"))?;
        let tick = u64::try_from(duration.as_millis()).map_err(|_| "Unix clock overflow".to_owned())?;
        Ok(ElapsedTick(tick))
    }, &mut || started.elapsed(), out)
}

// Both the executable and deterministic boundary tests use the same implementation.
fn command_with(args: &[String], clock: impl FnOnce() -> Result<ElapsedTick, String>,
    elapsed: &mut impl FnMut() -> Duration, out: &mut impl Write) -> Result<(), String>
{
    if args.len() < 2 { return Err(USAGE.into()); }
    let mode = Mode::parse(&args[..2])?;
    let options = Options::parse(mode, &args[2..])?;
    let spec = Spec::read(Path::new(&args[1]))?;
    let wall = spec.wall;
    checkpoint(wall, elapsed)?;
    if mode == Mode::Create && spec.minimum_revision != 0 {
        return Err("create requires minimum_revision zero; it never opens an existing coordinator".into());
    }
    let mut prepared = spec.prepare(Path::new(&args[1]), &mut || checkpoint(wall, elapsed))?;
    checkpoint(wall, elapsed)?;
    let custody = options.admit(&mut prepared, &mut || checkpoint(wall, elapsed))?;
    // CONFIG fixes the Unix-millisecond clock domain for every member. One new
    // observation supplies this pass; native resolution never reuses it as a
    // saved visit's clock and exact drained reads append no time event.
    let tick = clock()?;
    checkpoint(wall, elapsed)?;
    run(prepared, mode, tick, custody, elapsed, out)
}

fn checkpoint(wall: Duration, elapsed: &mut impl FnMut() -> Duration) -> Result<(), String> {
    if elapsed() >= wall { Err(DEADLINE.into()) } else { Ok(()) }
}

fn run(prepared: Prepared, mode: Mode, tick: ElapsedTick, custody: Custody,
    elapsed: &mut impl FnMut() -> Duration, out: &mut impl Write) -> Result<(), String>
{
    let Prepared { spec, plan, .. } = prepared;
    // Every member and anchor was admitted before either store is mutated.
    let clocks: Vec<_> = plan.domains().iter().map(|domain| FileShutdownClock {
        domain: domain.id(), clock_domain: domain.clock_domain(), at: tick,
    }).collect();
    checkpoint(spec.wall, elapsed)?;
    let mut owner = match mode {
        Mode::Create => FileShutdownCoordinator::create(&spec.coordinator, &plan, spec.coordinator_bytes),
        Mode::Recover | Mode::Resume => match &custody.previous {
            Some(anchor) => FileShutdownCoordinator::open_anchored(&spec.coordinator, &plan,
                spec.coordinator_bytes, spec.minimum_revision, anchor),
            None => FileShutdownCoordinator::open(&spec.coordinator, &plan,
                spec.coordinator_bytes, spec.minimum_revision),
        },
    }.map_err(|error| format!("coordinator acquisition unconfirmed: {error}; no domain command was started"))?;
    let mut timed_out = false;
    let mut poll = |_| {
        if elapsed() >= spec.wall { timed_out = true; false } else { true }
    };
    let revision = owner.revision();
    let pass = match mode {
        Mode::Create | Mode::Recover => owner.recover_registered_pass_with_checkpoints(
            revision, &clocks, spec.replay_events, &mut poll),
        Mode::Resume => owner.resume_registered_pass_with_checkpoints(
            revision, &clocks, spec.replay_events, &mut poll),
    }.map_err(|error| format!("fleet pass admission refused at coordinator revision {revision}: {error}; no member visit started"))?;
    // A pass may have changed some domains. Never hide those outcomes merely
    // because a later member failed or the elapsed budget ended.
    let retained = custody.retain(&owner, &mut || checkpoint(spec.wall, elapsed)).map_err(|error| format!(
        "coordinator anchor retention failed after acknowledged revision {}: {error}; keep the previous archive; do not resend publications",
        owner.revision()));
    let bytes = render::encode(mode.label(), timed_out, &pass);
    let reported = out.write_all(bytes.as_bytes()).and_then(|_| out.flush()).map_err(|error| format!(
        "fleet output failed after coordinator revision {}: {error}; domain effects may already be settled; use explicit resume, never resend publications",
        pass.final_revision()));
    let mut failures = Vec::new();
    if let Err(error) = reported { failures.push(error); }
    if let Err(error) = retained { failures.push(error); }
    if timed_out { failures.push(DEADLINE.to_owned()); }
    else if !pass.all_observed_drained() {
        failures.push(format!("fleet recovery incomplete at coordinator revision {}; inspect every member outcome; charges and unvisited members remain explicit",
            pass.final_revision()));
    }
    if failures.is_empty() { Ok(()) } else { Err(failures.join("; ")) }
}

#[cfg(test)]
#[path = "custody_tests.rs"] mod custody_tests;
