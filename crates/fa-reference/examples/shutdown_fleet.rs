//! Operator-only fleet shutdown; uses the original registered-domain coordinator.
#![forbid(unsafe_code)]
#[cfg(unix)]
#[allow(dead_code)]
#[path = "supervise_publication/config.rs"] mod config;
#[cfg(unix)]
#[path = "shutdown_fleet/operator.rs"] mod operator;

#[cfg(unix)]
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if let Err(error) = operator::command(&args, &mut std::io::stdout().lock()) {
        eprintln!("{error}");
        std::process::exit(2);
    }
}
#[cfg(not(unix))]
fn main() {
    eprintln!("shutdown_fleet requires the original Unix file-owner profile");
    std::process::exit(2);
}
