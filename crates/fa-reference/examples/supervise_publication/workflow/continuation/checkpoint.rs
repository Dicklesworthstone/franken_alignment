//! Private create-new custody for the native source-work/history checkpoint.
//! Export is separate from publication acknowledgment and never retries an effect.
use crate::config::{Config, debug, read_regular};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight,
    driver::topology::{FileDriverTopology, checkpoint::{TopologyCheckpoint, MAX_TOPOLOGY_CHECKPOINT_BYTES}}};
use std::{fs::{self, File, OpenOptions}, io::{self, Cursor}, path::{Component, Path, PathBuf}};
use std::os::unix::fs::OpenOptionsExt;

pub(crate) struct Output {
    path: PathBuf,
    topology: Vec<u8>,
    inventory: Vec<u8>,
}
impl Output {
    pub(crate) fn admit(config: &Config, path: &Path, topology: Vec<u8>, inventory: Vec<u8>)
        -> Result<Self, String>
    {
        validate_path(path)?;
        let parent = fs::canonicalize(path.parent().ok_or("checkpoint output needs a parent")?).map_err(debug)?;
        if !fs::metadata(&parent).map_err(debug)?.is_dir() { return Err("checkpoint parent is not a directory".into()); }
        let path = parent.join(path.file_name().ok_or("checkpoint output needs a filename")?);
        let store = match fs::canonicalize(&config.store) {
            Ok(store) => store,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let parent = fs::canonicalize(config.store.parent().ok_or("store needs a parent")?).map_err(debug)?;
                parent.join(config.store.file_name().ok_or("store needs a filename")?)
            }
            Err(error) => return Err(debug(error)),
        };
        if path.starts_with(&store) { return Err("topology checkpoints require custody outside the authority store".into()); }
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self { path, topology, inventory }),
            Err(error) => Err(debug(error)),
            Ok(_) => Err("checkpoint output must be a new file; previous custody is never replaced".into()),
        }
    }

    pub(crate) fn retain(&self, host: &FileOversight, topology: Option<&FileDriverTopology>)
        -> Result<(), String>
    {
        let result = (|| {
            let topology = topology.ok_or("no retained topology-source custody to checkpoint")?;
            // Use retained expectations, NOT newly read live files. Native capture
            // pairs actual expenditure with this SAME acknowledged owner's history.
            let checkpoint = topology.checkpoint(host, &self.topology, &self.inventory).map_err(debug)?;
            let parent = File::open(self.path.parent().ok_or("checkpoint output needs a parent")?).map_err(debug)?;
            let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600)
                .open(&self.path).map_err(debug)?;
            checkpoint.write_to(&mut file, MAX_TOPOLOGY_CHECKPOINT_BYTES).map_err(debug)?;
            file.sync_all().map_err(debug)?;
            parent.sync_all().map_err(debug)
        })();
        result.map_err(|error: String| format!("topology checkpoint retention failed: {error}; acknowledged work is not undone; keep previous custody and do not resend an effect"))
    }
}

pub(super) fn read(config: &Config, path: &Path,
    checkpoint: &mut impl FnMut() -> Result<(), String>) -> Result<TopologyCheckpoint, String>
{
    checkpoint()?;
    validate_path(path)?;
    let store = fs::canonicalize(&config.store).map_err(debug)?;
    if fs::canonicalize(path).map_err(debug)?.starts_with(&store) {
        return Err("topology checkpoints require custody outside the authority store".into());
    }
    let bytes = read_regular(path, MAX_TOPOLOGY_CHECKPOINT_BYTES)?;
    checkpoint()?;
    let saved = TopologyCheckpoint::read_trusted(&mut Cursor::new(bytes), MAX_TOPOLOGY_CHECKPOINT_BYTES).map_err(debug)?;
    if !saved.expected().graph().binds(config.profile.delivery.scope, config.profile.delivery.target) {
        return Err("source checkpoint does not bind the configured scope and resource".into());
    }
    checkpoint()?;
    Ok(saved)
}

// Attempt both independent outputs and preserve both errors. Neither successful
// archive is permission to report the other as retained or to repeat publication.
pub(super) fn combine(history: Result<(), String>, source: Result<(), String>) -> Result<(), String> {
    match (history, source) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(history), Err(source)) => Err(format!("{history}; {source}")),
    }
}
fn validate_path(path: &Path) -> Result<(), String> {
    if !path.is_absolute() || path.file_name().is_none()
        || path.components().any(|part| matches!(part, Component::ParentDir)) {
        return Err("checkpoint paths must be absolute without parent traversal".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "checkpoint/tests.rs"] mod tests;
