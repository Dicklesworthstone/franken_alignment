//! Explicit topology-supervised socket service selection.
//! Uses the existing service, mediated bootstrap and driver source; no second executor.
use super::{Config, ElapsedTick, PeerProfile, Profile, PublicationProfile, serve_selected};
use super::super::continuation::creation::topology::Selection;
use std::path::Path;

const USAGE: &str = "usage: supervise_publication serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE --topology-profile TOPOLOGY_PROFILE | serve-create-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE --topology-profile TOPOLOGY_PROFILE";

pub(super) fn command(args: &[String], credibility: Option<&Path>) -> Result<(), String> {
    command_with_clock(args, credibility, super::super::clock)
}

fn command_with_clock<F>(args: &[String], credibility: Option<&Path>, time: F) -> Result<(), String>
where F: FnMut() -> ElapsedTick {
    if credibility.is_some() { return Err(USAGE.into()); }
    let (publication, topology) = arguments(args)?;
    let config = Config::read(Path::new(&args[1]))?;
    let actor = Profile::read(Path::new(&args[2]))?;
    let reviewer = PeerProfile::read(Path::new(&args[3]))?;
    // A malformed/missing witness profile fails before socket or Store creation.
    // Never retry the plain command when the explicitly checked profile fails.
    let publication = publication.map(PublicationProfile::read).transpose()?;
    let topology = Selection::read(topology)?;
    serve_selected(config, &actor, &reviewer, publication.as_ref(), false, None,
        Some(topology), time)
}

fn arguments(args: &[String]) -> Result<(Option<&Path>, &Path), String> {
    let flag = match args.first().map(String::as_str) {
        Some("serve-create") if args.len() == 6 => 4,
        Some("serve-create-checked") if args.len() == 7 => 5,
        _ => return Err(USAGE.into()),
    };
    // Own the complete grammar before any file read. An option is not a path;
    // extra modes, sequences, peers and recovery cannot downgrade this endpoint.
    if args[flag] != "--topology-profile"
        || args[1..flag].iter().chain(args[flag + 1..].iter())
            .any(|arg| arg.is_empty() || arg.starts_with("--")) {
        return Err(USAGE.into());
    }
    Ok(((flag == 5).then(|| Path::new(&args[4])), Path::new(&args[flag + 1])))
}

#[cfg(test)]
#[path = "topology/tests.rs"] mod tests;
