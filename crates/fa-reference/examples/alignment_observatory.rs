//! Read-only local observatory for an independently configured publication history.
#![forbid(unsafe_code)]

#[cfg(unix)]
#[allow(dead_code)] // Reuse the original complete config parser; never launch programs.
#[path = "supervise_publication/config.rs"]
mod config;
#[cfg(unix)]
#[path = "alignment_observatory/model.rs"]
mod model;

#[cfg(unix)]
fn main() -> std::process::ExitCode {
    use std::io::Write;
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 2 || args[0] != "--snapshot" {
        eprintln!("usage: alignment_observatory --snapshot CONFIG.json");
        return std::process::ExitCode::from(2);
    }
    let result = (|| {
        let source = model::Source::read_config(std::path::Path::new(&args[1]), None)?;
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .map_err(config::debug)?.as_millis();
        let value = source.sample();
        let failed = value.is_err();
        if let Err(error) = &value { eprintln!("alignment_observatory: {error}"); }
        let document = model::envelope("live", 1, u64::try_from(now).map_err(config::debug)?, value);
        let mut out = std::io::stdout().lock();
        writeln!(out, "{}", model::encode(&document)).map_err(config::debug)?;
        out.flush().map_err(config::debug)?;
        Ok::<_, String>(failed)
    })();
    match result {
        Ok(false) => std::process::ExitCode::SUCCESS,
        Ok(true) => std::process::ExitCode::FAILURE,
        Err(error) => { eprintln!("alignment_observatory: {error}"); std::process::ExitCode::from(2) }
    }
}
#[cfg(not(unix))]
fn main() { eprintln!("alignment_observatory requires the Unix publication profile"); std::process::exit(2); }
