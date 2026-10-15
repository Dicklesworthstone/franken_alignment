//! Create once, publish through the existing review workflow, retain first custody.
//! This explicit creation route never opens an existing store or recovers keys.
use super::{Config, Continuation, Deadline, ElapsedTick, FileOversight, PeerProfile,
    PublicationProfile, RecoveryReserve, RunResult, combine_failure, debug, from_recovered,
    from_recovered_with_topology, plus};
use crate::recover_stop::guarded::CreationOutput;
use fa_reference::action::consequence::oversight::actor_wire::{Command, MAX_FRAME_BYTES, decode_command};
use std::{io::Write, path::Path, time::{Duration, Instant}};

mod topology;

#[cfg(test)]
mod topology_tests;
#[cfg(test)]
mod checked_topology_tests;

pub(crate) struct Creation<'a> {
    pub document: &'a [u8],
    pub output: &'a Path,
    pub peers: Option<&'a PeerProfile>,
    pub publication: Option<&'a PublicationProfile>,
}

pub(crate) fn run<F>(config: Config, creation: Creation<'_>, time: F) -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick {
    run_selected(config, creation, None, time)
}

fn run_selected<F>(config: Config, creation: Creation<'_>, selection: Option<topology::Selection>,
    mut time: F) -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick {
    let started = Instant::now();
    let Creation { document, output, peers, publication } = creation;
    let Command::Submit { request, proposal } = decode_command(document).map_err(debug)? else {
        return Err("creation requires an original submit document".into());
    };
    if config.profile.delivery.clock_domain != crate::config::CLOCK_DOMAIN {
        return Err("creation requires the configured Unix-millisecond clock domain".into());
    }
    // Admit the complete checked selection BEFORE topology I/O or storage.
    // The native mediated bootstrap supports plain witnesses, not the stronger
    // feed/snapshot/joint profiles. Never project those to their limits alone.
    let topology_publication = if selection.is_some() {
        publication.map(PublicationProfile::simple_limits).transpose()?
    } else { None };
    if let Some(peers) = peers { peers.check_host(&config)?; }
    // No store, source, helper or reviewer side effect precedes custody admission.
    let output = CreationOutput::admit(&config, output)?;
    let start = time();
    let runtime_deadline = plus(start, config.timing.runtime_ms)?;
    let deadline = Deadline { logical: proposal.deadline.min(runtime_deadline), started,
        wall: Duration::from_millis(config.timing.runtime_ms) };
    deadline.check(start)?;
    if let Some(selection) = selection {
        let prepared = selection.prepare(&config, &mut || deadline.check(time()))?;
        let (host, reviewer, topology) = prepared.create(&config, topology_publication)?;
        return from_recovered_with_topology(config, host, reviewer, Continuation {
            document, request, proposal, peers, publication, credibility: None, runtime_deadline, started,
        }, Some(topology), time, |host| output.retain(host));
    }
    let (mut host, reviewer) = match publication {
        Some(profile) => profile.create(&config.store, config.profile.clone()),
        None => FileOversight::create(&config.store, config.profile.clone()),
    }.map_err(debug)?;
    // Match original creation order and native boundaries. An error can leave
    // an acknowledged bootstrap prefix; retain only a healthy owner's actual cut.
    let bootstrap = (|| {
        host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).map_err(debug)?;
        host.enable_file_source(host.revision(), config.source_policy).map_err(debug)
    })();
    if let Err(error) = bootstrap {
        return Err(combine_failure(Some(error), output.retain(&host).err()).expect("bootstrap failure"));
    }
    // No ordinary recovery fence: the new owner has the original creation epoch.
    // Both source-backed and checked creation use the existing source -> helper
    // congress -> human approval -> publication body, including its stop/cleanup.
    from_recovered(config, host, reviewer, Continuation {
        document, request, proposal, peers, publication, credibility: None, runtime_deadline, started,
    }, time, |host| output.retain(host))
}

const USAGE: &str = "usage: supervise_publication create CONFIG SUBMIT_JSON --retain-history-anchor OUTPUT [--reviewer-profile REVIEWER_JSON] [--topology-profile TOPOLOGY_PROFILE]\n       supervise_publication create-checked CONFIG SUBMIT_JSON WITNESS_PROFILE --retain-history-anchor OUTPUT [--reviewer-profile REVIEWER_JSON] [--topology-profile TOPOLOGY_PROFILE]";

// Dispatched before general option stripping. Stop/resume/new-work recovery
// intent and credibility activation cannot be smuggled into initial creation.
pub(crate) fn command(args: &[String], out: &mut impl Write) -> Result<(), String> {
    let (output, peers, checked, topology) = arguments(args)?;
    let config = Config::read(Path::new(&args[1]))?;
    let document = crate::config::read_regular(Path::new(&args[2]), MAX_FRAME_BYTES)?;
    let publication = checked.then(|| PublicationProfile::read(Path::new(&args[3]))).transpose()?;
    let peers = peers.map(PeerProfile::read).transpose()?;
    let selection = topology.map(topology::Selection::read).transpose()?;
    let creation = Creation { document: &document, output,
        peers: peers.as_ref(), publication: publication.as_ref() };
    let result = match selection {
        Some(selection) => run_selected(config, creation, Some(selection), crate::workflow::clock)?,
        None => run(config, creation, crate::workflow::clock)?,
    };
    // Preserve export/workflow errors alongside write/flush failure. The original
    // actor JSON has no history bytes, custody paths or manufactured success.
    super::guarded::emit(result, out)
}

fn arguments(args: &[String]) -> Result<(&Path, Option<&Path>, bool, Option<&Path>), String> {
    let checked = match args.first().map(String::as_str) {
        Some("create") => false,
        Some("create-checked") => true,
        _ => return Err(USAGE.into()),
    };
    let offset = if checked { 4 } else { 3 };
    if ![offset + 2, offset + 4, offset + 6].contains(&args.len()) { return Err(USAGE.into()); }
    let mut output = None;
    let mut peers = None;
    let mut topology = None;
    for pair in args[offset..].chunks_exact(2) {
        let slot = match pair[0].as_str() {
            "--retain-history-anchor" => &mut output,
            "--reviewer-profile" => &mut peers,
            "--topology-profile" => &mut topology,
            _ => return Err(USAGE.into()),
        };
        if slot.is_some() || pair[1].is_empty() || pair[1].starts_with("--") { return Err(USAGE.into()); }
        *slot = Some(Path::new(&pair[1]));
    }
    Ok((output.ok_or(USAGE)?, peers, checked, topology))
}
