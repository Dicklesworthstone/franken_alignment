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
const REPAIR: &str = "--recover-receipt-checkpoint";
const REPAIR_REQUIRED: &str = "receipt checkpoint recovery requires explicit --resume --reconcile-receipts with an independent input anchor, v2 topology profile and both new custody outputs";
const REQUIRED: &str = "receipt custody requires an independent input history anchor and new retained history output; topology custody additionally requires an explicit v2 profile and paired new source output";

pub(super) struct Outputs {
    history: PathBuf,
    source: Option<AcknowledgedOutput>,
    identity: PathBuf,
    repair: bool,
}

/// Validate original immediate values before removing any options. Neither a
/// boolean nor another path option may fill a neighboring missing value when
/// stripped. The original parsers still own the complete path/numeric laws.
pub(super) fn take_repair_option(args: &[String]) -> Result<(Vec<String>, bool), String> {
    for (index, arg) in args.iter().enumerate() {
        if matches!(arg.as_str(), "--history-anchor" | "--retain-history-anchor"
            | "--topology-profile" | "--retain-topology-checkpoint"
            | "--serve-receipts" | "--finish-after-message")
        {
            let value = args.get(index + 1)
                .ok_or_else(|| format!("{arg} requires an immediate value"))?;
            if value.is_empty() || value.starts_with("--") {
                return Err(format!("{arg} requires an immediate non-option value"));
            }
        }
    }
    let mut remaining = Vec::new();
    let mut repair = false;
    for (index, arg) in args.iter().enumerate() {
        if arg == REPAIR {
            if index < 4 || repair || args.get(index.wrapping_sub(1)).is_some_and(|prior|
                matches!(prior.as_str(), "--history-anchor" | "--retain-history-anchor"
                    | "--topology-profile" | "--retain-topology-checkpoint"
                    | "--serve-receipts" | "--finish-after-message"))
            {
                return Err(REPAIR_REQUIRED.into());
            }
            repair = true;
        } else {
            remaining.push(arg.clone());
        }
    }
    Ok((remaining, repair))
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

/// Repair is an explicit historical receipt consumer, never a mode upgrade.
/// These shape checks run before configuration or archive intake in both clients.
pub(super) fn check_repair_options(
    args: &[String], topology: Option<&Path>, source: Option<&Path>, repair: bool,
) -> Result<(), String> {
    check_options(args, topology, source)?;
    if repair && (topology.is_none() || source.is_none()
        || !args.iter().any(|arg| arg == "--history-anchor")
        || !args.iter().any(|arg| arg == "--retain-history-anchor")
        || args.get(3).map(String::as_str) != Some("--resume")
        || args.iter().filter(|arg| arg.as_str() == "--reconcile-receipts").count() != 1
        || args.iter().any(|arg| arg == "--receipts-only"))
    {
        return Err(REPAIR_REQUIRED.into());
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
    admit_repair(config, loaded, source, false)
}

pub(super) fn admit_repair(
    config: &Config, loaded: &Loaded, source: Option<&Path>, repair: bool,
) -> Result<Option<Outputs>, String> {
    if repair && (source.is_none() || topology::history(loaded)?.is_none()) {
        return Err(REPAIR_REQUIRED.into());
    }
    let Some(history) = loaded.anchor_output.as_ref() else {
        return if source.is_none() && !repair {
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
        repair,
    }))
}

impl Outputs {
    pub(super) fn repair_requested(&self) -> bool { self.repair }

    /// Binding the service endpoint must not occupy either newly admitted
    /// archive destination. Resolve its parent exactly as archive admission does
    /// so a symlinked ancestor cannot hide this deterministic collision.
    pub(super) fn check_service_socket(&self, socket: &Path) -> Result<(), String> {
        let parent = fs::canonicalize(socket.parent().ok_or("receipt service socket needs a parent directory")?)
            .map_err(debug)?;
        if !fs::metadata(&parent).map_err(debug)?.is_dir() {
            return Err("receipt service socket parent is not a directory".into());
        }
        let socket = parent.join(socket.file_name().ok_or("receipt service socket needs a filename")?);
        if self.history == socket || self.source.as_ref().is_some_and(|source| source.path() == socket.as_path()) {
            return Err("receipt custody outputs must be distinct from the service socket".into());
        }
        Ok(())
    }

    /// Retain only the immutable result from the SAME acknowledged/confirmed cut.
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
