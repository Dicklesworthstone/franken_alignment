//! Explicit topology-supervised socket service selection.
//! Uses the existing service, mediated bootstrap and driver source; no second executor.
use super::{Config, PeerProfile, Profile, serve_selected};
use super::super::continuation::creation::topology::Selection;
use std::path::Path;

const USAGE: &str = "usage: supervise_publication serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE --topology-profile TOPOLOGY_PROFILE";

pub(super) fn command(args: &[String], credibility: Option<&Path>) -> Result<(), String> {
    if credibility.is_some() || args.len() != 6 || args[0] != "serve-create"
        || args[4] != "--topology-profile" { return Err(USAGE.into()); }
    let config = Config::read(Path::new(&args[1]))?;
    let actor = Profile::read(Path::new(&args[2]))?;
    let reviewer = PeerProfile::read(Path::new(&args[3]))?;
    let topology = Selection::read(Path::new(&args[5]))?;
    serve_selected(config, &actor, &reviewer, None, false, None, Some(topology),
        super::super::clock)
}
