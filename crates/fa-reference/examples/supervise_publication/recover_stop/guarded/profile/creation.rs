//! First independent custody for a newly created original publication owner.
//! Admission does not create files. Capture never opens or recovers a live store.
use super::{Config, Custody, Path, PathBuf, debug, fs, io, outside};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;

pub(crate) struct CreationOutput {
    path: PathBuf,
}
impl CreationOutput {
    /// Resolve the new store via its original parent, without creating it.
    /// An existing store is still refused by the original create-only owner API.
    pub(crate) fn admit(config: &Config, output: &Path) -> Result<Self, String> {
        if !output.is_absolute() { return Err("anchor paths must be absolute".into()); }
        let store = match fs::canonicalize(&config.store) {
            Ok(path) => path,
            Err(error) if error.kind() == io::ErrorKind::NotFound => resolved_new(&config.store)?,
            Err(error) => return Err(debug(error)),
        };
        let path = resolved_new(output)?;
        outside(&path, &store)?;
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self { path }),
            Err(error) => Err(debug(error)),
            Ok(_) => Err("anchor output must be a new file; the previous archive is never replaced".into()),
        }
    }

    /// The same original acknowledged-owner capture and bounded create-new
    /// writer used by guarded recovery. No re-encoding of a later disk image,
    /// implicit floor, permit, or assertion of successful publication is added.
    pub(crate) fn retain(&self, host: &FileOversight) -> Result<(), String> {
        let result = (|| {
            let anchor = host.history_anchor().map_err(debug)?;
            let custody = Custody { anchor, output: Some(self.path.clone()) };
            custody.retain(custody.anchor())
        })();
        result.map_err(|error| format!(
            "history anchor retention failed: {error}; the original store may already contain acknowledged work; do not repeat creation or resend an effect"))
    }
}

fn resolved_new(path: &Path) -> Result<PathBuf, String> {
    let parent = fs::canonicalize(path.parent().ok_or("anchor output needs a parent")?).map_err(debug)?;
    if !fs::metadata(&parent).map_err(debug)?.is_dir() { return Err("anchor parent is not a directory".into()); }
    Ok(parent.join(path.file_name().ok_or("anchor output needs a filename")?))
}
