//! Evaluate actual sampled continuations; no live deployment or effects.
#![forbid(unsafe_code)]
use fa_reference::action::consequence::activation::monitor::decoder::sampled::evaluation::files::evaluate_checkpoint_files;
use std::io;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).take(6).collect();
    if args.len() != 5 {
        eprintln!("evaluate_sampled_monitor_from_checkpoint CONFIG_JSON WEIGHTS_SAFETENSORS MONITOR_JSON ROLLOUT_JSON NEW_MONITOR_JSON");
        return ExitCode::FAILURE;
    }
    match evaluate_checkpoint_files(Path::new(&args[0]), Path::new(&args[1]), Path::new(&args[2]),
        Path::new(&args[3]), Path::new(&args[4]), &mut io::stdout().lock())
    {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(2),
        Err(error) => { eprintln!("{error}"); ExitCode::FAILURE }
    }
}
