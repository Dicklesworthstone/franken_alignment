//! Closed bounded operator input, not a new journal, checkpoint or authenticator.
use crate::config::{Config, debug, read_regular};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileRecoveryFloor, anchored::{FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES}};
use fa_reference::strict_json::{self, Json, Limits};
use std::{collections::BTreeMap, fs::{self, File, OpenOptions}, io::{self, Cursor}, path::{Path, PathBuf}};
use std::os::unix::fs::OpenOptionsExt;

mod topology;
mod creation;
pub(crate) use creation::CreationOutput;

const PROFILE_BYTES: usize = 16 * 1024;

pub(crate) struct Profile {
    minimum: FileRecoveryFloor,
    input: PathBuf,
    output: Option<PathBuf>,
    topology: Option<topology::Selection>,
}
impl Profile {
    pub(crate) fn read(path: &Path) -> Result<Self, String> {
        Self::decode(&read_regular(path, PROFILE_BYTES)?)
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, String> {
        Self::decode_schema(bytes, "fa.guarded-stop/1", "unsupported guarded stop profile")
    }

    fn decode_schema(bytes: &[u8], schema: &str, unsupported: &str) -> Result<Self, String> {
        let json = strict_json::parse(bytes, Limits { max_bytes: PROFILE_BYTES, max_depth: 4,
            max_items: 32, max_string_bytes: 4096 }).map_err(debug)?;
        let mut root = object(json)?;
        let version = take(&mut root, "schema_version")?;
        // Only the stop reader may select v2. Receipt and new-work profiles
        // retain their exact original schema; no other mode ignores topology.
        let mediated = schema == "fa.guarded-stop/1"
            && version.as_str() == Some("fa.guarded-stop/2");
        if !mediated && version.as_str() != Some(schema) {
            return Err(unsupported.into());
        }
        let mut floor = object(take(&mut root, "minimum")?)?;
        let minimum = FileRecoveryFloor { journal_revision: number(&mut floor, "journal_revision")?,
            control_sequence: number(&mut floor, "control_sequence")?,
            authority_epoch: number(&mut floor, "authority_epoch")? };
        end(&floor)?;
        let input = absolute(take(&mut root, "history_anchor")?)?;
        let output = root.remove("retain_history_anchor").map(absolute).transpose()?;
        let topology = if mediated {
            Some(topology::Selection::decode(take(&mut root, "topology")?)?)
        } else { None };
        end(&root)?;
        Ok(Self { minimum, input, output, topology })
    }

    /// Reuse the exact independent input custody for read-only investigation.
    /// Such a read acknowledges no new cut and therefore cannot export one.
    /// Reject output requests before touching either the store or anchor paths.
    pub(crate) fn admit_investigation(&self, config: &Config)
        -> Result<(FileHistoryAnchor, FileRecoveryFloor), String>
    {
        if self.topology.is_some() {
            return Err("mediated stop profiles are not read-only investigation profiles".into());
        }
        if self.output.is_some() {
            return Err("read-only proposal explanation cannot retain a history anchor".into());
        }
        let custody = self.admit(config)?;
        Ok((custody.anchor, self.minimum))
    }

    pub(super) fn minimum(&self) -> FileRecoveryFloor { self.minimum }

    /// Load only independently provisioned recovery requirements. This neither
    /// reads a live source nor certifies the declared topology for new work.
    pub(super) fn admit_topology(&self, config: &Config,
        checkpoint: &mut impl FnMut() -> Result<(), String>)
        -> Result<Option<fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::FileTopologyRequirement>, String>
    {
        self.topology.as_ref().map(|selection| selection.admit(config, checkpoint)).transpose()
    }

    pub(super) fn admit(&self, config: &Config) -> Result<Custody, String> {
        // Recovery must already have a store. Neither path admission nor an
        // absent anchor creates a replacement store or derives a new anchor.
        let store = fs::canonicalize(&config.store).map_err(debug)?;
        let input = fs::canonicalize(&self.input).map_err(debug)?;
        outside(&input, &store)?;
        let bytes = read_regular(&self.input, MAX_HISTORY_ANCHOR_BYTES)?;
        let anchor = FileHistoryAnchor::read_trusted(&mut Cursor::new(bytes), MAX_HISTORY_ANCHOR_BYTES)
            .map_err(debug)?;
        let output = self.output.as_ref().map(|path| {
            let parent = fs::canonicalize(path.parent().ok_or("anchor output needs a parent")?).map_err(debug)?;
            if !fs::metadata(&parent).map_err(debug)?.is_dir() { return Err("anchor parent is not a directory".into()); }
            let path = parent.join(path.file_name().ok_or("anchor output needs a filename")?);
            outside(&path, &store)?;
            match fs::symlink_metadata(&path) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(path),
                Err(error) => Err(debug(error)),
                Ok(_) => Err("anchor output must be a new file; the previous archive is never replaced".into()),
            }
        }).transpose()?;
        Ok(Custody { anchor, output })
    }
}

/// Schema-separated receipt recovery. A stop profile is never a live-recovery
/// request, even though the same bounded floors and private custody are needed.
pub(crate) struct ResumeProfile(Profile);
impl ResumeProfile {
    pub(crate) fn read(path: &Path) -> Result<Self, String> {
        Self::decode(&read_regular(path, PROFILE_BYTES)?)
    }
    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, String> {
        Profile::decode_schema(bytes, "fa.guarded-resume/1", "unsupported guarded resume profile")
            .map(Self)
    }
    pub(crate) fn minimum(&self) -> FileRecoveryFloor { self.0.minimum() }
    pub(crate) fn admit(&self, config: &Config) -> Result<Custody, String> { self.0.admit(config) }
}

/// Explicit new-work recovery intent, never inferred from a stop/receipt profile.
pub(crate) struct SubmitProfile(Profile);
impl SubmitProfile {
    pub(crate) fn read(path: &Path) -> Result<Self, String> {
        Self::decode(&read_regular(path, PROFILE_BYTES)?)
    }
    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, String> {
        Profile::decode_schema(bytes, "fa.guarded-submit/1", "unsupported guarded submit profile").map(Self)
    }
    pub(crate) fn minimum(&self) -> FileRecoveryFloor { self.0.minimum() }
    pub(crate) fn admit(&self, config: &Config) -> Result<Custody, String> { self.0.admit(config) }
}

pub(crate) struct Custody {
    anchor: FileHistoryAnchor,
    output: Option<PathBuf>,
}
impl Custody {
    pub(crate) fn retention_requested(&self) -> bool { self.output.is_some() }
    pub(crate) fn anchor(&self) -> &FileHistoryAnchor { &self.anchor }

    pub(crate) fn retain(&self, anchor: &FileHistoryAnchor) -> Result<(), String> {
        let Some(path) = self.output.as_deref() else { return Ok(()); };
        let parent = File::open(path.parent().ok_or("anchor output needs a parent")?).map_err(debug)?;
        // Repeat exclusive creation at the real write boundary, not only at
        // admission. A partial failed export stays unacknowledged and untouched.
        let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600).open(path).map_err(debug)?;
        anchor.write_to(&mut file, MAX_HISTORY_ANCHOR_BYTES).map_err(debug)?;
        file.sync_all().map_err(debug)?;
        parent.sync_all().map_err(debug)
    }
}

fn outside(path: &Path, store: &Path) -> Result<(), String> {
    if path.starts_with(store) { Err("history anchors require custody outside the authority store".into()) }
    else { Ok(()) }
}
fn object(value: Json) -> Result<BTreeMap<String, Json>, String> {
    match value { Json::Object(fields) => Ok(fields), _ => Err("guarded stop profile requires an object".into()) }
}
fn take(fields: &mut BTreeMap<String, Json>, key: &str) -> Result<Json, String> {
    fields.remove(key).ok_or_else(|| format!("guarded stop profile missing {key}"))
}
fn number(fields: &mut BTreeMap<String, Json>, key: &str) -> Result<u64, String> {
    take(fields, key)?.as_u64().ok_or_else(|| format!("guarded stop {key} must be an unsigned integer"))
}
fn absolute(value: Json) -> Result<PathBuf, String> {
    let path = PathBuf::from(value.as_str().ok_or("anchor path must be a string")?);
    if !path.is_absolute() { return Err("anchor paths must be absolute".into()); }
    Ok(path)
}
fn end(fields: &BTreeMap<String, Json>) -> Result<(), String> {
    if fields.is_empty() { Ok(()) } else { Err("unknown guarded stop profile fields".into()) }
}
