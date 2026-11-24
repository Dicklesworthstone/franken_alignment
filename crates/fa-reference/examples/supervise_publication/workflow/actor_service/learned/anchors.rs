//! Private operator custody of exact original history prefixes.
//! A pathname is not authentication: the selected input must be independently
//! retained, and the old archive remains required until a new export succeeds.
use super::{Config, FileOversight, FileSupervisedDriver, Loaded, USAGE, debug, fields, topology};
use crate::config::read_regular;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::anchored::{
    FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES,
};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Cursor},
    path::{Path, PathBuf},
};

#[derive(Default)]
pub(super) struct Options {
    input: Option<PathBuf>,
    output: Option<PathBuf>,
}

/// Remove only these explicit operator options. The original mode parsers still
/// reject every other unsupported combination and own all message/finish IDs.
pub(super) fn take_options(args: &[String]) -> Result<(Vec<String>, Options), String> {
    let mut options = Options::default();
    let mut remaining = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let selected = match args[index].as_str() {
            "--history-anchor" => Some(&mut options.input),
            "--retain-history-anchor" => Some(&mut options.output),
            _ => None,
        };
        if let Some(selected) = selected {
            if index < 4 || selected.is_some() {
                return Err(USAGE.into());
            }
            let value = args.get(index + 1).ok_or(USAGE)?;
            *selected = Some(fields::path(value)?);
            index += 2;
        } else {
            remaining.push(args[index].clone());
            index += 1;
        }
    }
    Ok((remaining, options))
}

impl Options {
    pub(super) fn has_input(&self) -> bool { self.input.is_some() }

    pub(super) fn admit(
        self,
        config: &Config,
        loaded: &mut Loaded,
        recovering: bool,
    ) -> Result<(), String> {
        if self.input.is_some() && !recovering {
            return Err(
                "an existing history anchor is only valid for recovery of its original store"
                    .into(),
            );
        }
        if self.input.is_none() && self.output.is_none() {
            return Ok(());
        }
        // Resolve an existing store or its original parent without creating it.
        // The archive must not be a child of the mutable authority directory.
        let store = match fs::canonicalize(&config.store) {
            Ok(path) => path,
            Err(error) if error.kind() == io::ErrorKind::NotFound => resolved_new(&config.store)?,
            Err(error) => return Err(debug(error)),
        };
        let input = self
            .input
            .map(|path| {
                let canonical = fs::canonicalize(&path).map_err(debug)?;
                outside_store(&canonical, &store)?;
                // Read the independently selected path with the original regular-file
                // admission. Never replace a missing/bad archive with today's journal.
                load(&path)
            })
            .transpose()?;
        let output = self
            .output
            .map(|path| {
                let canonical = resolved_new(&path)?;
                outside_store(&canonical, &store)?;
                require_new(&canonical)?;
                Ok::<_, String>(canonical)
            })
            .transpose()?;
        loaded.recovery_anchor = input;
        loaded.anchor_output = output;
        Ok(())
    }
}

/// A new archive must leave the names of independent inputs available. A
/// missing parent of a future witness or source must remain a directory slot.
pub(super) fn check_checkpoint_outputs(config: &Config, loaded: &Loaded) -> Result<(), String> {
    let mut inputs = vec![config.source.path()];
    let topology_output = match &loaded.topology {
        Some(topology::Selection::Create(selection)) => {
            inputs.extend(selection.input_paths());
            selection.acknowledged_output()
        }
        Some(topology::Selection::Recover { selection, .. }) => {
            inputs.extend(selection.input_paths());
            selection.acknowledged_output()
        }
        None => None,
    };
    let outputs = loaded
        .anchor_output
        .as_deref()
        .into_iter()
        .chain(topology_output)
        .map(fields::resolve_root)
        .collect::<Result<Vec<_>, _>>()?;
    if outputs.is_empty() {
        return Ok(());
    }
    if let Some(publication) = &loaded.publication {
        inputs.extend(publication.input_paths().iter().map(PathBuf::as_path));
    }
    if let Some(credential) = &loaded.credential {
        inputs.extend(credential.input_paths());
    }
    if let Some(identity) = &loaded.identity {
        inputs.push(&identity.observed_manifest);
    }
    for input in inputs {
        // Original source and credential readers require regular files. Compare
        // their directory entries without opening or reading the selected file.
        let parent = input.parent().ok_or("checkpoint input has no parent")?;
        let name = input.file_name().ok_or("checkpoint input has no filename")?;
        let input = fields::resolve_root(parent)?.join(name);
        if outputs.iter().any(|output| input.starts_with(output)) {
            return Err("checkpoint archive output conflicts with an independently selected input".into());
        }
    }
    Ok(())
}

fn outside_store(path: &Path, store: &Path) -> Result<(), String> {
    if path.starts_with(store) {
        return Err(
            "history anchors require separately selected custody outside the authority store"
                .into(),
        );
    }
    Ok(())
}

fn resolved_new(path: &Path) -> Result<PathBuf, String> {
    let parent = fs::canonicalize(
        path.parent()
            .ok_or("history anchor needs a parent directory")?,
    )
    .map_err(debug)?;
    if !fs::metadata(&parent).map_err(debug)?.is_dir() {
        return Err("history anchor parent is not a directory".into());
    }
    Ok(parent.join(path.file_name().ok_or("history anchor needs a filename")?))
}

fn require_new(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(debug(error)),
        Ok(_) => Err(
            "retained history anchor output must be a new file; the old archive is never replaced"
                .into(),
        ),
    }
}

pub(in super::super) fn load(path: &Path) -> Result<FileHistoryAnchor, String> {
    let bytes = read_regular(path, MAX_HISTORY_ANCHOR_BYTES)?;
    FileHistoryAnchor::read_trusted(&mut Cursor::new(bytes), MAX_HISTORY_ANCHOR_BYTES)
        .map_err(debug)
}

fn export(
    host: &FileOversight,
    previous: Option<&FileHistoryAnchor>,
    path: &Path,
) -> Result<(), String> {
    // Capture only the acknowledged healthy owner. Even an owner opened by a
    // weaker entry point cannot advance a divergent independently held prefix.
    let anchor = match previous {
        Some(previous) => host.history_anchor_after(previous),
        None => host.history_anchor(),
    }
    .map_err(debug)?;
    export_acknowledged(&anchor, path)
}

/// Persist an already acknowledged native anchor to a separately admitted NEW
/// private file. This never reads the store, recreates an owner or captures a
/// newer revision. The caller retains stop/effect acknowledgment on failure.
pub(in super::super) fn export_acknowledged(anchor: &FileHistoryAnchor, path: &Path) -> Result<(), String> {
    let parent = File::open(
        path.parent()
            .ok_or("history anchor needs a parent directory")?,
    )
    .map_err(debug)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    // create_new is repeated at the actual write boundary: a path appearing
    // after admission is never overwritten. A failed partial archive is not an
    // acknowledged anchor; retain the old one and select a new output to retry.
    let mut file = options.open(path).map_err(debug)?;
    anchor
        .write_to(&mut file, MAX_HISTORY_ANCHOR_BYTES)
        .map_err(debug)?;
    file.sync_all().map_err(debug)?;
    parent.sync_all().map_err(debug)?;
    Ok(())
}

/// Called exactly once while the original driver still owns its acknowledged
/// host, before cleanup and actor response rendering. Export failure changes the
/// command result, never the original effect or its response, and sends no retry.
pub(super) fn retain(
    loaded: &Loaded,
    driver: &FileSupervisedDriver,
    failure: Option<String>,
) -> Option<String> {
    let Some(path) = loaded.anchor_output.as_deref() else {
        return failure;
    };
    let result = (|| {
        let host = driver.supervisor().host().map_err(debug)?;
        export(&host, loaded.recovery_anchor.as_ref(), path)
    })();
    match result {
        Ok(()) => failure,
        Err(error) => {
            let error =
                format!("history anchor retention failed: {error}; no effect retry was sent");
            Some(match failure {
                Some(original) => format!("{original}; {error}"),
                None => error,
            })
        }
    }
}
