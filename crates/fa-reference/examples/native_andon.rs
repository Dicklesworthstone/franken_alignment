//! Native evaluation/accounting operator over the original guarded journal.
//! This executable does not run helpers, publish effects or authenticate labels.
#![forbid(unsafe_code)]

// Reuse the original bounded operator parser and independent anchor custody.
// Their other commands are intentionally not linked into this executable.
#[cfg(unix)]
#[allow(dead_code)]
#[path = "supervise_publication/config.rs"]
mod config;
#[cfg(unix)]
#[allow(dead_code)]
#[path = "supervise_publication/recover_stop/guarded/profile.rs"]
mod profile;
#[cfg(unix)]
#[path = "native_andon/operator.rs"]
mod operator;

#[cfg(unix)]
fn main() {
    if let Err(error) = operator::command(&std::env::args().skip(1).collect::<Vec<_>>(),
        &mut std::io::stdout().lock())
    {
        eprintln!("native_andon: {error}");
        std::process::exit(1);
    }
}
#[cfg(not(unix))]
fn main() {
    eprintln!("native_andon requires the Unix reference profile");
    std::process::exit(1);
}
