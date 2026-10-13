//! Private create-new custody for terminal history anchors.
use super::{Config, debug};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::anchored::{
    FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES};
use std::{fs::{self, File, OpenOptions}, io, path::{Path, PathBuf}};
use std::os::unix::fs::OpenOptionsExt;

pub(super) struct AnchorOutput { path: PathBuf }

impl AnchorOutput {
    pub(super) fn admit(config: &Config, output: &Path) -> Result<Self, String> {
        if !output.is_absolute() { return Err("anchor output must be absolute".into()); }
        let store = fs::canonicalize(&config.store).map_err(debug)?;
        let parent = fs::canonicalize(output.parent().ok_or("anchor output needs a parent")?).map_err(debug)?;
        if !fs::metadata(&parent).map_err(debug)?.is_dir() { return Err("anchor parent is not a directory".into()); }
        let path = parent.join(output.file_name().ok_or("anchor output needs a filename")?);
        if path.starts_with(&store) { return Err("history anchors require custody outside the authority store".into()); }
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self { path }),
            Err(error) => Err(debug(error)),
            Ok(_) => Err("anchor output must be a new file; retained history is never overwritten".into()),
        }
    }

    pub(super) fn retain(&self, anchor: &FileHistoryAnchor) -> Result<(), String> {
        let parent = File::open(self.path.parent().ok_or("anchor output needs a parent")?).map_err(debug)?;
        let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600).open(&self.path).map_err(debug)?;
        anchor.write_to(&mut file, MAX_HISTORY_ANCHOR_BYTES).map_err(debug)?;
        file.sync_all().map_err(debug)?;
        parent.sync_all().map_err(debug)
    }
}
