//! Native producer lifecycle for the same coupled bundle consumed by checked
//! supervision. This command owns no actor, helper, reviewer or effect authority.
#![forbid(unsafe_code)]
#[cfg(unix)]
#[path = "publication_producer/command.rs"]
mod command;

#[cfg(unix)]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(error) = command::run(&args, &mut std::io::stdout().lock()) {
        eprintln!("publication_producer: {error}");
        std::process::exit(1);
    }
}
#[cfg(not(unix))]
fn main() {
    eprintln!("publication_producer requires the Unix reference profile");
    std::process::exit(1);
}
