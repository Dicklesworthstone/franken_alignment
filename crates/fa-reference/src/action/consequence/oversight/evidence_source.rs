//! Bounded operator-file observations for the existing policy/congress path.
//!
//! A complete parsed file is not proof of a complete external world. The producer
//! declares the Snapshot domain and publishes immutable versions by atomic rename.
//! Scope, generation and exact input bytes are checked; authentication is not.

mod policy_stream;
pub use policy_stream::{PolicyFileSource, PolicyFileStatus};

use super::{CommitteeContract, CommitteeInput, MAX_COMMITTEE_BYTES, action_frame};
use crate::action::{FrozenAction, Purpose, Scope};
use crate::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use crate::full_input::{ActualHelperInput, ByteSpan, PartKind, SubmittedPart};
use crate::reducer::{MAX_IDENTIFIER_BYTES, MAX_VOTES};
use crate::strict_json::{self, Json, Limits};
use crate::{Error, ReadWitness, Snapshot};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub const MAX_EVIDENCE_FILE_BYTES: usize = 4 * 1_048_576;
pub const MAX_EVIDENCE_VALUES: usize = 256;
pub const MAX_EVIDENCE_VALUE_BYTES: usize = 65_536;
pub const MAX_EVIDENCE_POLICY_BYTES: usize = 524_288;
pub const MAX_HELPER_CONTEXT_BYTES: usize = 65_536;
pub const MAX_HELPER_CONTEXT_TOTAL: usize = 524_288;
const CONTEXT_DOMAIN: &[u8] = b"fa/file-helper-context/v1\0";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvidenceIdentity {
    pub source: u64,
    pub generation: u64,
    pub scope: Scope,
}

/// Immutable observation data, never an authorization or authenticated receipt.
/// Private policy values are NOT automatically copied into helper contexts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvidenceSnapshot {
    identity: EvidenceIdentity,
    snapshot: Snapshot,
    contexts: BTreeMap<String, Vec<u8>>,
}

impl EvidenceSnapshot {
    pub fn new(identity: EvidenceIdentity, snapshot: Snapshot, contexts: BTreeMap<String, Vec<u8>>) -> Result<Self, Error> {
        validate_scope(identity.scope)?;
        if identity.source == 0 || identity.generation == 0 || contexts.is_empty() { return Err(Error::InvalidInput); }
        if snapshot.values.len() > MAX_EVIDENCE_VALUES || contexts.len() > MAX_VOTES { return Err(Error::Limit); }
        let mut policy_bytes = 0;
        for value in snapshot.values.values() {
            if value.len() > MAX_EVIDENCE_VALUE_BYTES { return Err(Error::Limit); }
            charge(&mut policy_bytes, value.len(), MAX_EVIDENCE_POLICY_BYTES)?;
        }
        let mut context_bytes = 0;
        for (member, value) in &contexts {
            if member.is_empty() { return Err(Error::InvalidInput); }
            if member.len() > MAX_IDENTIFIER_BYTES || value.len() > MAX_HELPER_CONTEXT_BYTES { return Err(Error::Limit); }
            charge(&mut context_bytes, value.len(), MAX_HELPER_CONTEXT_TOTAL)?;
        }
        Ok(Self { identity, snapshot, contexts })
    }

    pub fn identity(&self) -> EvidenceIdentity { self.identity }
    pub fn snapshot(&self) -> &Snapshot { &self.snapshot }
    pub fn contexts(&self) -> &BTreeMap<String, Vec<u8>> { &self.contexts }

    /// A scoped reference-model identifier, NOT a hash or content commitment.
    /// The action/session separately binds full scope, policy and actual bytes.
    pub fn reference_root(&self) -> [u8; 32] {
        let mut root = [0; 32];
        for (chunk, value) in root.chunks_exact_mut(8).zip([
            self.identity.source, self.identity.generation,
            self.identity.scope.tenant, self.identity.scope.authority,
        ]) { chunk.copy_from_slice(&value.to_be_bytes()); }
        root
    }

    /// Build each helper's ACTUAL input under the existing full-input contract.
    /// Policy snapshot keys stay private; only the explicitly selected context
    /// for that member is included. No other member's context is disclosed.
    pub fn inputs_for(&self, action: &FrozenAction, contracts: &CommitteeContract) -> Result<CommitteeInput, Error> {
        if action.spec().scope != self.identity.scope || !self.contexts.keys().eq(contracts.members().keys()) {
            return Err(Error::Binding);
        }
        let frame = action_frame(action);
        let mut bytes = action.spec().payload.len();
        for witness in &action.spec().required_witnesses {
            if let ReadWitness::Exact { value: Some(value), .. } = witness { charge(&mut bytes, value.len(), MAX_COMMITTEE_BYTES)?; }
        }
        for (member, helper) in contracts.members() {
            let header = CONTEXT_DOMAIN.len() + 8 * 9 + member.len();
            for count in [member.len(), helper.profile_at(action.spec().policy_epoch).profile_bytes.len(),
                frame.len(), helper.question().len(), header, self.contexts[member].len()]
            { charge(&mut bytes, count, MAX_COMMITTEE_BYTES)?; }
        }
        let mut views = BTreeMap::new();
        for (member, helper) in contracts.members() {
            let context = &self.contexts[member];
            let mut submitted = frame.clone();
            let action_end = submitted.len();
            submitted.extend_from_slice(helper.question());
            let question_end = submitted.len();
            submitted.extend_from_slice(CONTEXT_DOMAIN);
            let s = self.identity.scope;
            for value in [self.identity.source, self.identity.generation, s.tenant, s.principal,
                s.run, s.branch, s.authority, member.len() as u64, context.len() as u64]
            { submitted.extend_from_slice(&value.to_be_bytes()); }
            submitted.extend_from_slice(member.as_bytes());
            submitted.extend_from_slice(context);
            let end = submitted.len();
            let input = ActualHelperInput::new(submitted, helper.profile_at(action.spec().policy_epoch), vec![
                SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: action_end } },
                SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: action_end, end: question_end } },
                SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: question_end, end } },
            ], Vec::new())?;
            views.insert(member.clone(), EvidenceViewManifest::new(input, AuthorizationProjection {
                projection_id: helper.projection_id(), policy_epoch: action.spec().policy_epoch,
                projected_originals: Vec::new(),
            }, Vec::new())?);
        }
        CommitteeInput::capture(action, contracts, views)
    }

    /// Producer interchange: exact decimal-string IDs and lowercase hex bytes.
    /// The producer must replace the destination atomically, never edit it live.
    pub fn encode(&self) -> Vec<u8> {
        let s = self.identity.scope;
        let values = self.snapshot.values.iter().map(|(key, value)| format!("\"{key}\":\"{}\"", hex(value))).collect::<Vec<_>>().join(",");
        let contexts = self.contexts.iter().map(|(member, value)| format!("{}:\"{}\"", quote(member), hex(value))).collect::<Vec<_>>().join(",");
        format!(concat!("{{\"version\":1,\"source\":\"{}\",\"generation\":\"{}\",",
            "\"scope\":{{\"tenant\":\"{}\",\"principal\":\"{}\",\"run\":\"{}\",\"branch\":\"{}\",\"authority\":\"{}\",\"purpose\":\"effect\"}},",
            "\"semantic_epoch\":\"{}\",\"complete\":{},\"values\":{{{}}},\"contexts\":{{{}}}}}"),
            self.identity.source, self.identity.generation, s.tenant, s.principal, s.run, s.branch,
            s.authority, self.snapshot.semantic_epoch, self.snapshot.complete, values, contexts).into_bytes()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        let json = strict_json::parse(bytes, Limits {
            max_bytes: MAX_EVIDENCE_FILE_BYTES, max_depth: 4, max_items: 2_048,
            max_string_bytes: 2 * MAX_EVIDENCE_VALUE_BYTES,
        }).map_err(|_| Error::InvalidInput)?;
        let root = object(&json)?;
        fields(root, &["version", "source", "generation", "scope", "semantic_epoch", "complete", "values", "contexts"])?;
        if root["version"].as_u64() != Some(1) { return Err(Error::InvalidInput); }
        let scope = object(&root["scope"])?;
        fields(scope, &["tenant", "principal", "run", "branch", "authority", "purpose"])?;
        if scope["purpose"].as_str() != Some("effect") { return Err(Error::Binding); }
        let scope = Scope { tenant: number(&scope["tenant"])?, principal: number(&scope["principal"])?,
            run: number(&scope["run"])?, branch: number(&scope["branch"])?, authority: number(&scope["authority"])?, purpose: Purpose::Effect };
        let identity = EvidenceIdentity { source: number(&root["source"])?, generation: number(&root["generation"])?, scope };
        let raw_values = object(&root["values"])?;
        let raw_contexts = object(&root["contexts"])?;
        if raw_values.len() > MAX_EVIDENCE_VALUES || raw_contexts.len() > MAX_VOTES { return Err(Error::Limit); }
        let mut values = BTreeMap::new();
        let mut policy_bytes = 0;
        for (key, value) in raw_values {
            let value = unhex(value, MAX_EVIDENCE_VALUE_BYTES)?;
            charge(&mut policy_bytes, value.len(), MAX_EVIDENCE_POLICY_BYTES)?;
            if values.insert(decimal(key)?, value).is_some() { return Err(Error::Duplicate); }
        }
        let mut contexts = BTreeMap::new();
        let mut context_bytes = 0;
        for (member, value) in raw_contexts {
            let value = unhex(value, MAX_HELPER_CONTEXT_BYTES)?;
            charge(&mut context_bytes, value.len(), MAX_HELPER_CONTEXT_TOTAL)?;
            contexts.insert(member.clone(), value);
        }
        Self::new(identity, Snapshot { semantic_epoch: number(&root["semantic_epoch"])?,
            complete: root["complete"].as_bool().ok_or(Error::InvalidInput)?, values }, contexts)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvidenceError { Data(Error), Io(io::ErrorKind) }
impl From<Error> for EvidenceError { fn from(error: Error) -> Self { Self::Data(error) } }
impl std::fmt::Display for EvidenceError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(out, "evidence source: {self:?}") }
}
impl std::error::Error for EvidenceError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvidenceSourceStatus {
    pub retained_generation: Option<u64>,
    pub available: bool,
    pub read_attempts: u64,
    pub failure: Option<EvidenceError>,
}

/// One operator-selected file and one surviving source-version floor. The path
/// and its ancestors must be outside actor write authority. Not a watcher,
/// authenticated provider, durable anti-rollback key or arbitrary file reader.
pub struct FileEvidenceSource {
    path: PathBuf,
    source: u64,
    scope: Scope,
    max_bytes: usize,
    retained: Option<Rc<EvidenceSnapshot>>,
    status: EvidenceSourceStatus,
}

impl std::fmt::Debug for FileEvidenceSource {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("FileEvidenceSource").field("status", &self.status).finish_non_exhaustive()
    }
}

impl FileEvidenceSource {
    pub fn new(path: impl AsRef<Path>, source: u64, scope: Scope, max_bytes: usize) -> Result<Self, Error> {
        validate_scope(scope)?;
        if !path.as_ref().is_absolute() || source == 0 || max_bytes == 0 { return Err(Error::InvalidInput); }
        if max_bytes > MAX_EVIDENCE_FILE_BYTES { return Err(Error::Limit); }
        Ok(Self { path: path.as_ref().to_owned(), source, scope, max_bytes, retained: None,
            status: EvidenceSourceStatus { retained_generation: None, available: false, read_attempts: 0, failure: None } })
    }
    pub fn scope(&self) -> Scope { self.scope }
    pub fn status(&self) -> EvidenceSourceStatus { self.status }
    pub fn current(&self) -> Option<&EvidenceSnapshot> {
        if self.status.available { self.retained.as_deref() } else { None }
    }

    /// Opens and reads anew on EVERY call. Failure withdraws current eligibility,
    /// never returns the previous snapshot, and retains the generation floor.
    /// Same-generation content substitution and semantic rollback both refuse.
    pub fn read(&mut self) -> Result<Rc<EvidenceSnapshot>, EvidenceError> {
        self.status.available = false;
        let result = self.read_inner();
        self.status.failure = result.as_ref().err().copied();
        if result.is_ok() { self.status.available = true; }
        result
    }

    fn read_inner(&mut self) -> Result<Rc<EvidenceSnapshot>, EvidenceError> {
        self.status.read_attempts = self.status.read_attempts.checked_add(1).ok_or(Error::Overflow)?;
        let meta = fs::symlink_metadata(&self.path).map_err(|e| EvidenceError::Io(e.kind()))?;
        if !meta.is_file() || meta.file_type().is_symlink() { return Err(Error::Binding.into()); }
        let mut file = File::open(&self.path).map_err(|e| EvidenceError::Io(e.kind()))?;
        let meta = file.metadata().map_err(|e| EvidenceError::Io(e.kind()))?;
        if !meta.is_file() { return Err(Error::Binding.into()); }
        if meta.len() > self.max_bytes as u64 { return Err(Error::Limit.into()); }
        let mut bytes = Vec::new();
        (&mut file).take(self.max_bytes as u64 + 1).read_to_end(&mut bytes).map_err(|e| EvidenceError::Io(e.kind()))?;
        if bytes.len() > self.max_bytes { return Err(Error::Limit.into()); }
        let next = EvidenceSnapshot::decode(&bytes)?;
        if next.identity.source != self.source || next.identity.scope != self.scope { return Err(Error::Binding.into()); }
        if let Some(previous) = &self.retained {
            if next.identity.generation < previous.identity.generation
                || next.snapshot.semantic_epoch < previous.snapshot.semantic_epoch { return Err(Error::Stale.into()); }
            if next.identity.generation == previous.identity.generation {
                if &next != previous.as_ref() { return Err(Error::Binding.into()); }
                return Ok(Rc::clone(previous));
            }
        }
        self.status.retained_generation = Some(next.identity.generation);
        let next = Rc::new(next);
        self.retained = Some(Rc::clone(&next));
        Ok(next)
    }
}

fn validate_scope(scope: Scope) -> Result<(), Error> {
    if scope.purpose != Purpose::Effect || [scope.tenant, scope.principal, scope.run, scope.branch, scope.authority].contains(&0) {
        return Err(Error::Binding);
    }
    Ok(())
}
fn charge(total: &mut usize, count: usize, maximum: usize) -> Result<(), Error> {
    *total = total.checked_add(count).ok_or(Error::Overflow)?;
    if *total > maximum { return Err(Error::Limit); }
    Ok(())
}
fn object(json: &Json) -> Result<&BTreeMap<String, Json>, Error> { json.as_object().ok_or(Error::InvalidInput) }
fn fields(object: &BTreeMap<String, Json>, expected: &[&str]) -> Result<(), Error> {
    if object.len() != expected.len() || expected.iter().any(|name| !object.contains_key(*name)) { return Err(Error::InvalidInput); }
    Ok(())
}
fn decimal(text: &str) -> Result<u64, Error> {
    if text.is_empty() || text.len() > 20 || !text.bytes().all(|b| b.is_ascii_digit())
        || (text.len() > 1 && text.starts_with('0')) { return Err(Error::InvalidInput); }
    text.parse().map_err(|_| Error::InvalidInput)
}
fn number(json: &Json) -> Result<u64, Error> { decimal(json.as_str().ok_or(Error::InvalidInput)?) }
fn unhex(json: &Json, max: usize) -> Result<Vec<u8>, Error> {
    let text = json.as_str().ok_or(Error::InvalidInput)?;
    if text.len() > max * 2 { return Err(Error::Limit); }
    if !text.len().is_multiple_of(2) { return Err(Error::InvalidInput); }
    let digit = |b| match b { b'0'..=b'9' => Ok(b - b'0'), b'a'..=b'f' => Ok(b - b'a' + 10), _ => Err(Error::InvalidInput) };
    text.as_bytes().chunks_exact(2).map(|p| Ok((digit(p[0])? << 4) | digit(p[1])?)).collect()
}
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes { text.push(char::from(DIGITS[(byte >> 4) as usize])); text.push(char::from(DIGITS[(byte & 15) as usize])); }
    text
}
fn quote(text: &str) -> String {
    let mut quoted = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => quoted.push_str("\\\""), '\\' => quoted.push_str("\\\\"),
            c if c <= '\u{1f}' => quoted.push_str(&format!("\\u{:04x}", c as u32)),
            c => quoted.push(c),
        }
    }
    quoted.push('"'); quoted
}

mod sealed { pub trait Sealed {} }

/// Only the concrete bounded file readers implement this interface. The driver
/// can use either direct observations or the adapter registered in its live gate
/// without accepting a third-party implementation that silently returns a cache.
pub trait EvidenceFile: sealed::Sealed {
    fn read_evidence(&mut self) -> Result<Rc<EvidenceSnapshot>, EvidenceError>;
}
impl sealed::Sealed for FileEvidenceSource {}
impl EvidenceFile for FileEvidenceSource {
    fn read_evidence(&mut self) -> Result<Rc<EvidenceSnapshot>, EvidenceError> { self.read() }
}
