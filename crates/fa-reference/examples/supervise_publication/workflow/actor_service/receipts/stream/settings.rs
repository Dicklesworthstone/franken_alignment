//! Independent stream identity, complete baseline guard choice and history floor.
use super::Config;
use crate::config::{debug, read_regular};
use fa_reference::action::consequence::delivery::stream::StreamProfile;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileRecoveryFloor, anchored::{FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES},
};
use fa_reference::strict_json::{self, Json, Limits};
use std::{collections::BTreeMap, fs, io::Cursor, path::{Component, Path, PathBuf}};

pub(super) struct Settings {
    pub stream: StreamProfile,
    pub registered_source: bool,
    pub minimum: FileRecoveryFloor,
    pub history: PathBuf,
}
impl Settings {
    pub fn read(path: &Path) -> Result<Self, String> { Self::decode(&read_regular(path, 16384)?) }
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let json = strict_json::parse(bytes, Limits { max_bytes: 16384, max_depth: 3,
            max_items: 64, max_string_bytes: 4096 }).map_err(debug)?;
        let mut f = Fields::new(json)?;
        if f.text("schema")? != "fa.stream-receipt-recovery/1" { return Err("unsupported stream receipt schema".into()); }
        let mut s = Fields::new(f.take("stream")?)?;
        let stream = StreamProfile::new(s.number("id")?, s.number("generation")?, s.usize("max_messages")?,
            s.usize("max_message_bytes")?, s.usize("max_stream_bytes")?).map_err(debug)?;
        s.end()?;
        let registered_source = match f.take("registered_source")? {
            Json::Bool(v) => v, _ => return Err("registered_source must explicitly be true or false".into()),
        };
        let mut m = Fields::new(f.take("minimum")?)?;
        let minimum = FileRecoveryFloor { journal_revision: m.number("journal_revision")?,
            control_sequence: m.number("control_sequence")?, authority_epoch: m.number("authority_epoch")? };
        m.end()?;
        let history = PathBuf::from(f.text("history_anchor")?);
        if !history.is_absolute() || history.file_name().is_none()
            || history.components().any(|p| !matches!(p, Component::RootDir | Component::Normal(_)))
            || history.as_os_str().as_encoded_bytes().contains(&0)
        { return Err("history_anchor must be an absolute normalized file path".into()); }
        f.end()?;
        Ok(Self { stream, registered_source, minimum, history })
    }
    pub fn anchor(&self, config: &Config) -> Result<FileHistoryAnchor, String> {
        // An anchor must be independently retained, not read from the authority
        // directory under another textual path. This checks location, not trust.
        let domain = fs::canonicalize(&config.store).map_err(debug)?;
        let history = fs::canonicalize(&self.history).map_err(debug)?;
        if history.starts_with(domain) { return Err("history anchor must be outside the authority store".into()); }
        let bytes = read_regular(&self.history, MAX_HISTORY_ANCHOR_BYTES)?;
        FileHistoryAnchor::read_trusted(&mut Cursor::new(bytes), MAX_HISTORY_ANCHOR_BYTES).map_err(debug)
    }
}
struct Fields(BTreeMap<String, Json>);
impl Fields {
    fn new(value: Json) -> Result<Self, String> { match value { Json::Object(v) => Ok(Self(v)), _ => Err("expected stream recovery object".into()) } }
    fn take(&mut self, name: &str) -> Result<Json, String> { self.0.remove(name).ok_or_else(|| format!("missing stream recovery field {name}")) }
    fn text(&mut self, name: &str) -> Result<String, String> { self.take(name)?.as_str().map(str::to_owned).ok_or_else(|| format!("{name} must be text")) }
    fn number(&mut self, name: &str) -> Result<u64, String> { self.take(name)?.as_u64().ok_or_else(|| format!("{name} must be unsigned")) }
    fn usize(&mut self, name: &str) -> Result<usize, String> { usize::try_from(self.number(name)?).map_err(debug) }
    fn end(self) -> Result<(), String> { if self.0.is_empty() { Ok(()) } else { Err("unknown stream recovery field".into()) } }
}
