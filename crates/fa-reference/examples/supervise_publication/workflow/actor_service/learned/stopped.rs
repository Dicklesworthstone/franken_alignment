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
    let started = Instant::now();
    let (recovered, retained) = if let Some(path) = loaded.anchor_output.as_deref() {
        let history = recovery::stop_selected_with_anchor_with_elapsed(config, loaded, operation, clock,
            &mut || started.elapsed()).map_err(unconfirmed)?;
        let (recovered, anchor) = history.into_parts();
        // Export exactly the acknowledged stopped cut, before rendering. Never
        // reopen through a live owner or read possibly newer canonical bytes.
        let retained = anchors::export_acknowledged(&anchor, path);
        (recovered, retained)
    } else {
        let recovered = recovery::stop_selected_with_elapsed(config, loaded, operation, clock,
            &mut || started.elapsed()).map_err(unconfirmed)?;
        (recovered, Ok(()))
    };
    let retained = retained.map_err(|error| format!(
        "native stop acknowledged at revision {}, but history anchor retention failed: {error}; no effect retry was sent; keep the previous archive and select a new output path",
        recovered.snapshot().revision));
    // A failed export must not hide the native stop or its remaining liabilities.
    // Conversely a failed/pending report must not hide failed archive retention.
    let reported = crate::recover_stop::report(&recovered, out);
    match (reported, retained) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(report), Err(retention)) => Err(format!("{report}; {retention}")),
    }
}

fn unconfirmed(error: String) -> String {
    format!("learned native stop completion is unconfirmed: {error}; no effect was resent")
}
