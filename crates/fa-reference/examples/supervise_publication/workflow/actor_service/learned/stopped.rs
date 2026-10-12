//! Offline terminal recovery through the original independently guarded cursor.
//! This command returns a native stop report, never live actor/reviewer custody.
use super::{Config, Loaded, ElapsedTick, Instant, USAGE, anchors, clock, recipe, recovery};
use std::{io::Write, path::Path};

pub(super) fn command(args: &[String], options: anchors::Options) -> Result<(), String> {
    if args.len() != 5 || args[0] != "create-learned-generated" || args[3] != "--recover-stop" {
        return Err(USAGE.into());
    }
    let text = &args[4];
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) { return Err(USAGE.into()); }
    let operation = text.parse::<u64>().map_err(|_| USAGE.to_owned())?;
    if operation == 0 { return Err(USAGE.into()); }
    options.check_stopped()?;
    let config = Config::read(Path::new(&args[1]))?;
    // Recovery binds primary model and predictor history without loading native
    // helper models, opening a reviewer/stop socket or reading current evidence.
    let mut loaded = recipe::load(Path::new(&args[2]), &config, false)?;
    options.admit(&config, &mut loaded, true)?;
    run(&config, &loaded, operation, clock, &mut std::io::stdout().lock())
}

pub(super) fn run<F>(config: &Config, loaded: &Loaded, operation: u64,
    clock: F, out: &mut impl Write) -> Result<(), String>
where F: FnOnce() -> ElapsedTick {
    if operation == 0 { return Err("stop operation must be nonzero".into()); }
    if loaded.anchor_output.is_some() {
        return Err("learned stop recovery cannot retain a successor history anchor".into());
    }
    let started = Instant::now();
    let recovered = recovery::stop_selected_with_elapsed(config, loaded, operation, clock,
        &mut || started.elapsed()).map_err(|error|
            format!("learned native stop completion is unconfirmed: {error}; no effect was resent"))?;
    crate::recover_stop::report(&recovered, out)
}
