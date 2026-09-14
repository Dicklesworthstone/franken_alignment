//! Reviewer-only configuration and Linux credential admission for the runnable
//! command. Contains no helper executable, environment, source path or ledger key.
use super::config::{Config, CLOCK_DOMAIN, debug, read_regular};
use fa_reference::action::{Purpose, Scope};
use fa_reference::action::consequence::delivery::persistent::observed::{FileHumanRequest, FileHumanReviewer, FileOversight};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::{ReviewerConnection};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::{ReviewerClient, ReviewerExpectation};
use fa_reference::strict_json::{self, Json, Limits};
#[cfg(target_os = "linux")]
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::{
    PeerPolicy, ReviewerPeerAdmission, ReviewerPeerError, VerifiedReviewerSocket,
};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Component, Path, PathBuf};

const MAX_PROFILE_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy, Debug)]
struct Identity { uid: u32, gid: u32, pid: Option<u32> }
impl Identity {
    #[cfg(target_os = "linux")]
    fn policy(self) -> Result<PeerPolicy, String> { PeerPolicy::new(self.uid, self.gid, self.pid).map_err(debug) }
}

/// Public to the independent reviewer but never supplied by an actor or offer.
/// Logical audience and kernel identities are separate, independently checked.
#[derive(Clone, Debug)]
pub struct PeerProfile {
    pub expected: ReviewerExpectation,
    pub runtime_ms: u64,
    pub poll_ms: u64,
    directory: PathBuf,
    supervisor: Identity,
    reviewer: Identity,
    candidates: u32,
}
impl PeerProfile {
    pub fn read(path: &Path) -> Result<Self, String> {
        supported()?;
        Self::decode(&read_regular(path, MAX_PROFILE_BYTES)?)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        supported()?;
        let json = strict_json::parse(bytes, Limits { max_bytes: MAX_PROFILE_BYTES,
            max_depth: 4, max_items: 64, max_string_bytes: 4096 }).map_err(debug)?;
        let root = object(&json, &["version", "clock", "scope", "reviewer_id", "socket_directory",
            "supervisor", "reviewer", "candidate_limit", "runtime_ms", "poll_ms"])?;
        if number(root, "version")? != 1 || root["clock"].as_str() != Some("unix_milliseconds") {
            return Err("unsupported peer profile version or clock".into());
        }
        let fields = object(&root["scope"], &["tenant", "principal", "run", "branch", "authority"])?;
        let scope = Scope { tenant: number(fields, "tenant")?, principal: number(fields, "principal")?,
            run: number(fields, "run")?, branch: number(fields, "branch")?, authority: number(fields, "authority")?, purpose: Purpose::Effect };
        let expected = ReviewerExpectation { scope, reviewer: number(root, "reviewer_id")?, clock_domain: CLOCK_DOMAIN };
        if [scope.tenant, scope.principal, scope.run, scope.branch, scope.authority, expected.reviewer].contains(&0) {
            return Err("peer profile audience identifiers must be nonzero".into());
        }
        let directory = PathBuf::from(root["socket_directory"].as_str().ok_or("socket_directory must be text")?);
        if !directory.is_absolute() || directory.components().any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
            || directory.as_os_str().as_encoded_bytes().contains(&0) {
            return Err("socket_directory must be an absolute normalized path without NUL".into());
        }
        let supervisor = identity(&root["supervisor"])?;
        let reviewer = identity(&root["reviewer"])?;
        let candidates = u32::try_from(number(root, "candidate_limit")?).map_err(debug)?;
        #[cfg(target_os = "linux")]
        {
            supervisor.policy()?;
            ReviewerPeerAdmission::new(reviewer.policy()?, candidates).map_err(debug)?;
        }
        let runtime_ms = number(root, "runtime_ms")?;
        let poll_ms = number(root, "poll_ms")?;
        if runtime_ms == 0 || runtime_ms > 3_600_000 || poll_ms == 0 || poll_ms > 1000 || poll_ms > runtime_ms {
            return Err("peer timing must be finite: runtime 1..=3600000 ms and poll 1..=1000 ms within runtime".into());
        }
        Ok(Self { expected, runtime_ms, poll_ms, directory, supervisor, reviewer, candidates })
    }

    pub fn check_host(&self, config: &Config) -> Result<(), String> {
        supported()?;
        if self.expected.reviewer != config.profile.human.reviewer_id
            || self.expected.scope != config.profile.delivery.scope
            || self.expected.clock_domain != config.profile.delivery.clock_domain {
            return Err("reviewer profile does not match the independently configured authority audience".into());
        }
        self.check_directory()
    }

    /// The directory must be preprovisioned, owned by the declared supervisor,
    /// and not writable by group or others. Group traversal is an operator choice
    /// enabling a separate reviewer UID without exposing the 0700 ledger directory.
    pub fn check_directory(&self) -> Result<(), String> {
        let meta = fs::symlink_metadata(&self.directory).map_err(debug)?;
        if !meta.is_dir() || meta.file_type().is_symlink() || meta.mode() & 0o022 != 0
            || meta.uid() != self.supervisor.uid || meta.gid() != self.supervisor.gid {
            return Err("reviewer socket directory must be supervisor-owned with the declared group and no group/other write access".into());
        }
        Ok(())
    }
    pub fn socket(&self, request: u64) -> PathBuf { self.directory.join(format!("review-{request}.sock")) }

    /// Called only on the just-bound socket in the protected directory. No
    /// existing socket is unlinked or chmodded to make a conflicting launch work.
    pub fn secure_socket(&self, path: &Path) -> Result<(), String> {
        let meta = fs::symlink_metadata(path).map_err(debug)?;
        if path.parent() != Some(self.directory.as_path()) || !meta.file_type().is_socket()
            || meta.uid() != self.supervisor.uid || meta.gid() != self.supervisor.gid {
            return Err("new reviewer socket has unexpected ownership or type".into());
        }
        fs::set_permissions(path, fs::Permissions::from_mode(0o660)).map_err(debug)
    }

    pub fn connect_client(&self, request: u64) -> Result<ReviewerClient<UnixStream>, String> {
        supported()?;
        if request == 0 { return Err("review request must be nonzero".into()); }
        self.check_directory()?;
        #[cfg(target_os = "linux")]
        {
            let stream = UnixStream::connect(self.socket(request)).map_err(debug)?;
            let verified = VerifiedReviewerSocket::verify(stream, self.supervisor.policy()?).map_err(debug)?;
            eprintln!("Verified supervisor connection: {:?}", verified.peer());
            verified.into_client(self.expected).map_err(debug)
        }
        #[cfg(not(target_os = "linux"))]
        { Err("Linux peer credential verification is required".into()) }
    }
}

/// An accepted raw legacy stream or the opaque checked socket. The checked
/// variant is never extracted into raw transport or retried after a refusal.
pub enum ReviewStream {
    Legacy(UnixStream),
    #[cfg(target_os = "linux")]
    Checked(VerifiedReviewerSocket),
}
impl ReviewStream {
    pub fn into_connection(self, host: &FileOversight, reviewer: &FileHumanReviewer,
        request: FileHumanRequest, nonce: [u8; 32]) -> Result<ReviewerConnection<UnixStream>, String>
    {
        match self {
            Self::Legacy(stream) => ReviewerConnection::from_unix(host, reviewer, request, stream, nonce).map_err(debug),
            #[cfg(target_os = "linux")]
            Self::Checked(socket) => socket.into_connection(host, reviewer, request, nonce).map_err(debug),
        }
    }
}

pub struct Admission {
    #[cfg(target_os = "linux")]
    gate: Option<ReviewerPeerAdmission>,
}
impl Admission {
    pub fn new(profile: Option<&PeerProfile>) -> Result<Self, String> {
        if profile.is_some() { supported()?; }
        #[cfg(target_os = "linux")]
        { Ok(Self { gate: profile.map(|p| ReviewerPeerAdmission::new(p.reviewer.policy()?, p.candidates).map_err(debug)).transpose()? }) }
        #[cfg(not(target_os = "linux"))]
        { Ok(Self {}) }
    }
    /// None means this candidate was rejected without any evidence or decision
    /// processing. The host keeps accepting under its ORIGINAL deadline. The last
    /// rejection consumes the quota and fails instead of weakening the rule.
    pub fn admit(&mut self, stream: UnixStream) -> Result<Option<ReviewStream>, String> {
        #[cfg(target_os = "linux")]
        if let Some(gate) = &mut self.gate {
            return match gate.admit(stream) {
                Ok(socket) => {
                    eprintln!("Verified reviewer connection: {:?}", socket.peer());
                    Ok(Some(ReviewStream::Checked(socket)))
                }
                Err(ReviewerPeerError::Credentials(std::io::ErrorKind::PermissionDenied)) => {
                    eprintln!("Rejected reviewer process credentials; attempted={}", gate.status().attempted);
                    if gate.exhausted() { Err("reviewer candidate quota exhausted; no review decision accepted".into()) }
                    else { Ok(None) }
                }
                Err(error) => Err(debug(error)),
            };
        }
        Ok(Some(ReviewStream::Legacy(stream)))
    }
}
fn supported() -> Result<(), String> {
    if cfg!(target_os = "linux") { Ok(()) } else { Err("reviewer peer profile requires Linux; no unchecked fallback".into()) }
}
fn object<'a>(value: &'a Json, fields: &[&str]) -> Result<&'a BTreeMap<String, Json>, String> {
    let Json::Object(object) = value else { return Err("peer profile expects an object".into()); };
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err("peer profile has missing or unsupported fields".into());
    }
    Ok(object)
}
fn number(object: &BTreeMap<String, Json>, field: &str) -> Result<u64, String> {
    object[field].as_u64().ok_or_else(|| format!("{field} must be an unsigned integer"))
}
fn identity(value: &Json) -> Result<Identity, String> {
    let fields = object(value, &["uid", "gid", "pid"])?;
    let uid = u32::try_from(number(fields, "uid")?).map_err(debug)?;
    let gid = u32::try_from(number(fields, "gid")?).map_err(debug)?;
    // Explicit null is required to choose UID/GID-only matching. Missing PID is
    // not silently interpreted as a wildcard.
    let pid = match &fields["pid"] {
        Json::Null => None,
        number => Some(u32::try_from(number.as_u64().ok_or("pid must be null or an unsigned integer")?).map_err(debug)?),
    };
    Ok(Identity { uid, gid, pid })
}
