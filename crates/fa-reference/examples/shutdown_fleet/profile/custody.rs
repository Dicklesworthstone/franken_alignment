//! Explicit independent coordinator custody for the existing fleet operator.
use super::super::{Prepared, Mode};
use super::{path, charged, err};
use fa_reference::action::consequence::delivery::persistent::observed::shutdown::coordinator::{
    FileShutdownCoordinator, FileShutdownCoordinatorAnchor, MAX_COORDINATOR_ANCHOR_BYTES};
use std::{fs::{self, File, OpenOptions}, io::Write, os::unix::fs::OpenOptionsExt, path::{Path, PathBuf}};

#[derive(Default)]
pub(in super::super) struct Options { input: Option<PathBuf>, output: Option<PathBuf> }
pub(in super::super) struct Custody {
    pub previous: Option<FileShutdownCoordinatorAnchor>,
    output: Option<PathBuf>,
    excluded: Vec<PathBuf>,
}
impl Options {
    pub fn parse(mode: Mode, args: &[String]) -> Result<Self, String> {
        if args.len() > 4 || args.len() % 2 != 0 { return Err("invalid coordinator custody options".into()); }
        let mut result = Self::default();
        for pair in args.chunks_exact(2) {
            let slot = match pair[0].as_str() {
                "--coordinator-anchor" => &mut result.input,
                "--retain-coordinator-anchor" => &mut result.output,
                _ => return Err("unknown coordinator custody option".into()),
            };
            if slot.is_some() { return Err("duplicate coordinator custody option".into()); }
            *slot = Some(path(pair[1].clone())?);
        }
        if mode == Mode::Create && result.input.is_some() {
            return Err("create cannot consume a previous coordinator anchor".into());
        }
        Ok(result)
    }

    pub fn admit(self, prepared: &mut Prepared, check: &mut impl FnMut() -> Result<(), String>)
        -> Result<Custody, String>
    {
        if self.input.is_none() && self.output.is_none() {
            return Ok(Custody { previous: None, output: None, excluded: Vec::new() });
        }
        check()?;
        let mut excluded = vec![prepared.spec.coordinator.clone()];
        excluded.extend(prepared.plan.domains().iter().map(|domain| domain.directory().to_path_buf()));
        let previous = if let Some(input) = self.input {
            let resolved = fs::canonicalize(&input).map_err(err)?;
            outside(&resolved, &excluded)?;
            let bytes = charged(&input, MAX_COORDINATOR_ANCHOR_BYTES, &mut prepared.remaining_input_bytes)?;
            let anchor = FileShutdownCoordinatorAnchor::from_trusted_bytes(&bytes).map_err(err)?;
            check()?;
            Some(anchor)
        } else { None };
        if let Some(output) = &self.output { destination(output, &excluded)?; }
        check()?;
        Ok(Custody { previous, output: self.output, excluded })
    }
}
impl Custody {
    /// Export only a healthy acknowledged image, before reporting it to stdout.
    /// New-file creation preserves the old anchor even on late export failure.
    /// A failed write can leave an incomplete file; it is never reported retained.
    pub fn retain(&self, owner: &FileShutdownCoordinator,
        check: &mut impl FnMut() -> Result<(), String>) -> Result<(), String>
    {
        let Some(output) = &self.output else { return Ok(()); };
        check()?;
        let anchor = match &self.previous {
            Some(previous) => owner.history_anchor_after(previous),
            None => owner.history_anchor(),
        }.map_err(err)?;
        let bytes = anchor.to_bytes().map_err(err)?;
        check()?;
        destination(output, &self.excluded)?;
        let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600).open(output).map_err(err)?;
        file.write_all(&bytes).and_then(|()| file.sync_all()).map_err(err)?;
        File::open(output.parent().ok_or("missing output parent")?).and_then(|parent| parent.sync_all()).map_err(err)?;
        Ok(())
    }
}
fn outside(path: &Path, excluded: &[PathBuf]) -> Result<(), String> {
    if excluded.iter().any(|store| path.starts_with(store)) {
        Err("coordinator custody must be outside all authority stores".into())
    } else { Ok(()) }
}
fn destination(output: &Path, excluded: &[PathBuf]) -> Result<(), String> {
    let parent = output.parent().ok_or("missing output parent")?;
    if fs::canonicalize(parent).map_err(err)? != parent {
        return Err("coordinator anchor output parent must be canonical".into());
    }
    outside(output, excluded)?;
    match fs::symlink_metadata(output) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(err(error)),
        Ok(_) => Err("coordinator anchor output must be a new file".into()),
    }
}
