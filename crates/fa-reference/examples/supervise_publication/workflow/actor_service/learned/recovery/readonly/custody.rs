//! Independent archives for one acknowledged receipt-settlement transaction.
//! A returned outcome is not proof that either separately requested file exists.
use super::{Config, Loaded, anchors, topology};
use crate::config::debug;
use crate::workflow::continuation::checkpoint::AcknowledgedOutput;
use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::FileReconciledLearnedReceipt;
use std::{
    fs,
    path::{Path, PathBuf},
};

const FLAG: &str = "--retain-topology-checkpoint";
const REQUIRED: &str = "receipt custody requires an independent input history anchor and new retained history output; topology custody additionally requires an explicit v2 profile and paired new source output";

pub(super) struct Outputs {
    history: PathBuf,
    source: Option<AcknowledgedOutput>,
    identity: PathBuf,
}

pub(super) fn take_option(args: &[String]) -> Result<(Vec<String>, Option<PathBuf>), String> {
    let mut remaining = Vec::new();
    let mut source = None;
    let mut index = 0;
    while index < args.len() {
        if args[index] == FLAG {
            if index < 4 || source.is_some() {
                return Err(REQUIRED.into());
            }
            source = Some(super::super::super::fields::path(
                args.get(index + 1).ok_or(REQUIRED)?,
            )?);
            index += 2;
        } else {
            remaining.push(args[index].clone());
            index += 1;
        }
    }
    Ok((remaining, source))
}

/// Pure option checks precede configuration, archive reads and original replay.
pub(super) fn check_options(
    args: &[String],
    topology: Option<&Path>,
    source: Option<&Path>,
) -> Result<(), String> {
    let input = args.iter().any(|arg| arg == "--history-anchor");
    let history = args.iter().any(|arg| arg == "--retain-history-anchor");
    if (history && !input)
        || (source.is_some() && (!history || topology.is_none()))
        || (history && topology.is_some() && source.is_none())
    {
        return Err(REQUIRED.into());
    }
    Ok(())
}

/// Existing archive primitives own path normalization, outside-store custody,
/// private create-new admission and rechecks at the actual export boundary.
pub(super) fn admit(
    config: &Config,
    loaded: &Loaded,
    source: Option<&Path>,
) -> Result<Option<Outputs>, String> {
    let Some(history) = loaded.anchor_output.as_ref() else {
        return if source.is_none() {
            Ok(None)
        } else {
            Err(REQUIRED.into())
        };
    };
    let anchor = loaded.recovery_anchor.as_ref().ok_or(REQUIRED)?;
    let selected = topology::history(loaded)?;
    let source = match (selected, source) {
        (None, None) => None,
        (Some(selected), Some(path)) => {
            let saved = selected.checkpoint().ok_or(
                "receipt topology custody requires an exact v2 source checkpoint; manual source minima cannot establish complete custody")?;
            if saved.history_anchor() != anchor {
                return Err(
                    "receipt source checkpoint and input history anchor describe different cuts"
                        .into(),
                );
            }
            let output = AcknowledgedOutput::admit(config, path)?;
            if output.path() == history {
                return Err("receipt history anchor and source checkpoint outputs must be distinct new files".into());
            }
            Some(output)
        }
        _ => return Err(REQUIRED.into()),
    };
    let identity = fs::canonicalize(&config.store).map_err(debug)?;
    Ok(Some(Outputs {
        history: history.clone(),
        source,
        identity,
    }))
}

impl Outputs {
    /// Retain only the immutable result from the SAME acknowledged transaction.
    /// Both exports are attempted even if capture or either write fails.
    pub(super) fn retain(
        &self,
        config: &Config,
        loaded: &Loaded,
        result: &FileReconciledLearnedReceipt,
    ) -> Result<(), String> {
        let captured = self.source.as_ref().map(|_| {
            let selected = topology::history(loaded)?.ok_or(REQUIRED)?;
            let saved = selected.checkpoint().ok_or(REQUIRED)?;
            let next = saved
                .after_reconciled_receipt(&config.profile, &self.identity, result)
                .map_err(debug)?;
            if next.history_anchor() != result.history_anchor() {
                return Err(
                    "receipt checkpoint and acknowledged history describe different cuts".into(),
                );
            }
            Ok::<_, String>(next)
        });
        let history = anchors::export_acknowledged(result.history_anchor(), &self.history)
            .map_err(|error| format!("receipt history anchor retention failed: {error}; keep the previous archive and select a new output path"));
        let source = match (&self.source, captured) {
            (None, None) => Ok(()),
            (Some(output), Some(Ok(saved))) => output.retain(&saved),
            (Some(_), Some(Err(error))) => Err(format!(
                "receipt topology checkpoint capture failed: {error}; keep previous custody"
            )),
            _ => Err(REQUIRED.into()),
        };
        match (history, source) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
            (Err(history), Err(source)) => Err(format!("{history}; {source}")),
        }
    }
}
