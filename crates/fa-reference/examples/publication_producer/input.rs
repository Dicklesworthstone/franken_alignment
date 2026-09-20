//! Bounded acquisition of operator-selected immutable observation documents.
//! Explicit producer observation time is never replaced by this process's clock.
use fa_reference::action::{ElapsedTick, Purpose, Scope};
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::{FilePublicationInputs, MAX_PUBLICATION_PACKET_BYTES};
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::producer::PublicationProducerProfile;
use fa_reference::strict_json::{self, Json, Limits};
use std::collections::BTreeMap;
use std::fs::{self, File, Metadata};
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

const PROFILE_BYTES: usize = 65_536;
const OBSERVATION_BYTES: usize = 2 * MAX_PUBLICATION_PACKET_BYTES + PROFILE_BYTES;
/// The existing supervised-publication profile uses the same explicit label.
const CLOCK_DOMAIN: u64 = u64::from_be_bytes(*b"FAUNIXMS");

pub(super) struct Profile {
    pub(super) directory: PathBuf,
    pub(super) identity: PublicationProducerProfile,
    pub(super) minimum_generation: u64,
}
impl Profile {
    pub(super) fn read(path: &Path) -> Result<Self, String> {
        Self::decode(&read_regular(path, PROFILE_BYTES)?)
    }
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut f = Fields::parse(bytes, PROFILE_BYTES, 4096)?;
        if f.text("schema")? != "fa.publication-producer/1" { return Err("unsupported producer profile".into()); }
        let directory = PathBuf::from(f.text("directory")?);
        if !directory.is_absolute() || directory.file_name().is_none() {
            return Err("producer directory must be an absolute named path".into());
        }
        let source = f.number("source")?; let feed = f.number("feed")?; let after = f.number("after")?;
        if f.text("clock")? != "unix_milliseconds" { return Err("producer clock must be unix_milliseconds".into()); }
        let mut s = Fields::object(f.take("scope")?)?;
        let scope = Scope { tenant: s.number("tenant")?, principal: s.number("principal")?,
            run: s.number("run")?, branch: s.number("branch")?, authority: s.number("authority")?, purpose: Purpose::Effect };
        s.end()?;
        let minimum_generation = f.number("minimum_generation")?;
        if minimum_generation == 0 { return Err("minimum producer generation must be nonzero".into()); }
        f.end()?;
        let identity = PublicationProducerProfile { source, scope, feed, clock_domain: CLOCK_DOMAIN, after };
        identity.check().map_err(debug)?;
        Ok(Self { directory, identity, minimum_generation })
    }
}

pub(super) struct Observation {
    pub(super) expected_generation: u64,
    pub(super) observed_at: ElapsedTick,
    pub(super) inputs: FilePublicationInputs,
}
impl Observation {
    pub(super) fn read(path: &Path) -> Result<Self, String> {
        Self::decode(&read_regular(path, OBSERVATION_BYTES)?)
    }
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut f = Fields::parse(bytes, OBSERVATION_BYTES, 2 * MAX_PUBLICATION_PACKET_BYTES)?;
        if f.text("schema")? != "fa.publication-observation/1" { return Err("unsupported producer observation".into()); }
        let expected_generation = f.number("expected_generation")?;
        expected_generation.checked_add(1).ok_or("producer generation overflow")?;
        let observed_at = ElapsedTick(f.number("observed_at_unix_ms")?);
        let encoded = f.text("inputs_hex")?;
        f.end()?;
        let bytes = unhex(&encoded, MAX_PUBLICATION_PACKET_BYTES)?;
        let inputs = FilePublicationInputs::from_bytes(&bytes).map_err(debug)?;
        Ok(Self { expected_generation, observed_at, inputs })
    }
    pub(super) fn encode(&self) -> Result<Vec<u8>, String> {
        self.expected_generation.checked_add(1).ok_or("producer generation overflow")?;
        let bytes = self.inputs.to_bytes().map_err(debug)?;
        let encoded = hex(&bytes)?;
        Ok(format!("{{\"schema\":\"fa.publication-observation/1\",\"expected_generation\":{},\"observed_at_unix_ms\":{},\"inputs_hex\":\"{}\"}}\n",
            self.expected_generation, self.observed_at.0, encoded).into_bytes())
    }
}

fn hex(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() > MAX_PUBLICATION_PACKET_BYTES { return Err("input packet is too large".into()); }
    let mut encoded = String::new();
    encoded.try_reserve_exact(bytes.len() * 2).map_err(debug)?;
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    for &byte in bytes { encoded.push(DIGITS[(byte >> 4) as usize] as char); encoded.push(DIGITS[(byte & 15) as usize] as char); }
    Ok(encoded)
}
pub(super) fn unhex(text: &str, limit: usize) -> Result<Vec<u8>, String> {
    if text.len() / 2 > limit || !text.len().is_multiple_of(2) {
        return Err("invalid input packet hex length".into());
    }
    let nibble = |b| match b { b'0'..=b'9' => Ok(b - b'0'), b'a'..=b'f' => Ok(b - b'a' + 10),
        _ => Err("input packet must use canonical lower-case hex".to_owned()) };
    let mut bytes = Vec::new(); bytes.try_reserve_exact(text.len() / 2).map_err(debug)?;
    for pair in text.as_bytes().chunks_exact(2) { bytes.push((nibble(pair[0])? << 4) | nibble(pair[1])?); }
    Ok(bytes)
}

/// Reject symlinks/special files, bound growth, and detect replacement/in-place
/// mutation during acquisition. Equal metadata is not hostile-host authentication.
pub(super) fn read_regular(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let before = regular(path, limit)?;
    let file = File::open(path).map_err(debug)?;
    let opened = file.metadata().map_err(debug)?;
    if !opened.is_file() || stamp(&before) != stamp(&opened) { return Err("input changed before acquisition".into()); }
    let mut bytes = Vec::new(); bytes.try_reserve_exact(opened.len() as usize).map_err(debug)?;
    (&file).take(limit as u64 + 1).read_to_end(&mut bytes).map_err(debug)?;
    if bytes.len() > limit { return Err("input exceeded its byte limit".into()); }
    if stamp(&opened) != stamp(&file.metadata().map_err(debug)?) || stamp(&opened) != stamp(&regular(path, limit)?) {
        return Err("input changed during acquisition".into());
    }
    Ok(bytes)
}
fn regular(path: &Path, limit: usize) -> Result<Metadata, String> {
    let metadata = fs::symlink_metadata(path).map_err(debug)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() { return Err("input must be a regular non-symlink file".into()); }
    if metadata.len() > limit as u64 { return Err("input exceeded its byte limit".into()); }
    Ok(metadata)
}
fn stamp(m: &Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (m.dev(), m.ino(), m.len(), m.mtime(), m.mtime_nsec(), m.ctime(), m.ctime_nsec())
}
pub(super) fn debug(error: impl std::fmt::Debug) -> String { format!("{error:?}") }
pub(super) struct Fields(BTreeMap<String, Json>);
impl Fields {
    fn parse(bytes: &[u8], limit: usize, string_limit: usize) -> Result<Self, String> {
        Self::object(strict_json::parse(bytes, Limits { max_bytes: limit, max_depth: 4,
            max_items: 64, max_string_bytes: string_limit }).map_err(debug)?)
    }
    pub(super) fn object(value: Json) -> Result<Self, String> {
        match value { Json::Object(fields) => Ok(Self(fields)), _ => Err("expected a JSON object".into()) }
    }
    pub(super) fn take(&mut self, key: &str) -> Result<Json, String> { self.0.remove(key).ok_or_else(|| format!("missing field {key}")) }
    pub(super) fn text(&mut self, key: &str) -> Result<String, String> {
        self.take(key)?.as_str().map(str::to_owned).ok_or_else(|| format!("{key} must be text"))
    }
    pub(super) fn number(&mut self, key: &str) -> Result<u64, String> {
        self.take(key)?.as_u64().ok_or_else(|| format!("{key} must be an unsigned integer"))
    }
    pub(super) fn array(&mut self, key: &str, max: usize) -> Result<Vec<Json>, String> {
        match self.take(key)? {
            Json::Array(values) if values.len() <= max => Ok(values),
            Json::Array(_) => Err(format!("{key} exceeds its item limit")),
            _ => Err(format!("{key} must be an array")),
        }
    }
    pub(super) fn end(self) -> Result<(), String> { if self.0.is_empty() { Ok(()) } else { Err("unknown producer field".into()) } }
}
