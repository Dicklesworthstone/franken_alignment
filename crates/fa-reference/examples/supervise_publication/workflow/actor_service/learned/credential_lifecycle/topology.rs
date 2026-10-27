//! Paired custody for credential maintenance on an exact topology source cut.
//! The native checkpoint successor owns suffix validation; this adapter only
//! admits private output paths and exports its already acknowledged result.
use super::{Config, Loaded, debug, owner::Host, topology};
use crate::workflow::continuation::checkpoint::AcknowledgedOutput;
use fa_reference::action::consequence::delivery::persistent::observed::{
    driver::topology::checkpoint::TopologyCheckpoint, guarded::anchored::FileHistoryAnchor,
};
use std::path::{Path, PathBuf};

const FLAG: &str = "--retain-topology-checkpoint";
const REQUIRED: &str = "credential topology custody requires an explicit topology profile and a paired new source checkpoint output";

pub(super) struct Output {
    archive: AcknowledgedOutput,
}

pub(super) fn take_options(
    args: &[String],
) -> Result<(Vec<String>, Option<PathBuf>, bool), String> {
    let mut remaining = Vec::new();
    let mut output = None;
    let mut recover_checkpoint = false;
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--recover-terminal-checkpoint" {
            return Err(
                "credential topology custody cannot use terminal checkpoint recovery".into(),
            );
        }
        if args[index] == "--recover-credential-checkpoint" {
            if index < 4 || recover_checkpoint {
                return Err("credential checkpoint recovery must be selected once in credential lifecycle mode".into());
            }
            recover_checkpoint = true;
            index += 1;
        } else if args[index] == FLAG {
            if index < 4 || output.is_some() {
                return Err(REQUIRED.into());
            }
            output = Some(super::super::fields::path(
                args.get(index + 1).ok_or(REQUIRED)?,
            )?);
            index += 2;
        } else {
            remaining.push(args[index].clone());
            index += 1;
        }
    }
    Ok((remaining, output, recover_checkpoint))
}

/// Reject incomplete topology custody before configuration intake. The two
/// original parsers still own duplicate flags, paths and remaining mode grammar.
pub(super) fn check_pair(
    profile: Option<&Path>,
    output: Option<&Path>,
    recover_checkpoint: bool,
) -> Result<(), String> {
    if profile.is_some() != output.is_some() || (recover_checkpoint && profile.is_none()) {
        return Err(REQUIRED.into());
    }
    Ok(())
}

pub(super) fn admit(
    config: &Config,
    loaded: &Loaded,
    path: Option<&Path>,
) -> Result<Option<Output>, String> {
    let history = topology::history(loaded)?;
    match (history, path) {
        (None, None) => Ok(None),
        (Some(history), Some(path)) => {
            if history.checkpoint().is_none() {
                return Err("credential topology custody requires an exact v2 source checkpoint; manual work minima cannot establish complete custody".into());
            }
            let anchor = loaded.anchor_output.as_deref().ok_or(REQUIRED)?;
            let archive = AcknowledgedOutput::admit(config, path)?;
            if archive.path() == anchor {
                return Err(
                    "history anchor and topology checkpoint outputs must be distinct new files"
                        .into(),
                );
            }
            Ok(Some(Output { archive }))
        }
        _ => Err(REQUIRED.into()),
    }
}

/// Capture from the same healthy original owner as the independent anchor.
/// No current source files are consulted and no source work is replenished.
pub(super) fn capture(
    loaded: &Loaded,
    host: &impl Host,
    anchor: &FileHistoryAnchor,
) -> Result<Option<TopologyCheckpoint>, String> {
    let Some(history) = topology::history(loaded)? else {
        return Ok(None);
    };
    let saved = history.checkpoint().ok_or(REQUIRED)?;
    let next = host.checkpoint(saved).map_err(debug)?;
    if next.history_anchor() != anchor {
        return Err(
            "credential source checkpoint and acknowledged history describe different cuts".into(),
        );
    }
    Ok(Some(next))
}

/// A source export is attempted independently of the matching history export.
/// Missing capture never produces a file or a successful retention claim.
pub(super) fn retain(
    output: Option<&Output>,
    checkpoint: Option<&TopologyCheckpoint>,
) -> Result<Option<u64>, String> {
    match (output, checkpoint) {
        (None, None) => Ok(None),
        (Some(output), Some(checkpoint)) => {
            output.archive.retain(checkpoint)?;
            Ok(Some(checkpoint.history_anchor().revision()))
        }
        _ => Err(
            "credential topology checkpoint retention has no matching acknowledged source cut"
                .into(),
        ),
    }
}
