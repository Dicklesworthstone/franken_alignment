//! Independent successor custody for the same finite mediated submission.
//! Archive selection is admitted before recovery; acknowledgment is not export.
use super::{Config, Custody, FileOversight, PeerProfile, Selection, debug, retain};
use crate::workflow::continuation::checkpoint::{AcknowledgedOutput, combine};
use fa_reference::action::consequence::delivery::persistent::observed::driver::topology::checkpoint::TopologyCheckpoint;
use std::{fs, path::{Path, PathBuf}};

pub(super) fn admit(
    config: &Config,
    custody: &Custody,
    topology: &Selection,
    request: u64,
    peers: Option<&PeerProfile>,
) -> Result<Option<AcknowledgedOutput>, String> {
    let source = topology
        .acknowledged_output()
        .map(|path| AcknowledgedOutput::admit(config, path))
        .transpose()?;
    if let Some(source) = &source {
        if custody.output_path() == Some(source.path()) {
            return Err(
                "history anchor and topology checkpoint outputs must be distinct new files".into(),
            );
        }
    }
    let outputs: Vec<_> = [
        custody.output_path(),
        source.as_ref().map(AcknowledgedOutput::path),
    ]
    .into_iter()
    .flatten()
    .collect();
    if outputs.is_empty() {
        return Ok(source);
    }
    // A path that will become a human/Stop endpoint cannot also receive an
    // archive after that listener is retired. Resolve parents before fencing.
    let reviewer = peers.map_or_else(|| config.socket(request), |peer| peer.socket(request));
    let reviewer = destination(&reviewer)?;
    if outputs.iter().any(|output| *output == reviewer.as_path()) {
        return Err("recovery archives and reviewer sockets must be distinct".into());
    }
    #[cfg(target_os = "linux")]
    if let Some(peers) = peers {
        let stop = destination(&crate::workflow::control::socket_path(peers, request))?;
        if outputs.iter().any(|output| *output == stop.as_path()) {
            return Err("recovery archives and Stop sockets must be distinct".into());
        }
    }
    Ok(source)
}

fn destination(path: &Path) -> Result<PathBuf, String> {
    let parent =
        fs::canonicalize(path.parent().ok_or("review endpoint needs a parent")?).map_err(debug)?;
    Ok(parent.join(path.file_name().ok_or("review endpoint needs a filename")?))
}

pub(super) fn retain_recovered(
    custody: &Custody,
    source: Option<&AcknowledgedOutput>,
    saved: Option<&TopologyCheckpoint>,
    owner: &FileOversight,
) -> Result<(), String> {
    // Evaluate both exports independently. A failed destination never suppresses
    // its companion and cannot repeat recovery, source checking or publication.
    combine(
        retain(custody, owner),
        source.map_or(Ok(()), |output| retain_unchanged(output, saved, owner)),
    )
}

pub(super) fn retain_unchanged(
    output: &AcknowledgedOutput,
    saved: Option<&TopologyCheckpoint>,
    owner: &FileOversight,
) -> Result<(), String> {
    let saved =
        saved.ok_or("source checkpoint retention requires the original exact recovered cut")?;
    // The sealed native finalizer captured source custody while no source could
    // progress. This helper is only used before binding or on source-free reads.
    // A later event or poisoned owner must not relabel that capture as current.
    if &owner.history_anchor().map_err(debug)? != saved.history_anchor() {
        return Err("source checkpoint retention refused: the original owner advanced beyond its recovered source cut; preserve the old pair and the new history".into());
    }
    output.retain(saved)
}
