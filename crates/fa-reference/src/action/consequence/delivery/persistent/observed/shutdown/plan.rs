//! Portable registrations without opening domains or reconstructing live roles.
use super::*;
use super::super::super::codec::shared::{Reader, Writer};
use std::os::unix::ffi::OsStrExt;

const DOMAIN: &[u8; 8] = b"FASHPLN\x01";
const MAX_PATH_BYTES: usize = 4096;
pub const MAX_SHUTDOWN_PLAN_BYTES: usize = MAX_SHUTDOWN_HEAD_BYTES
    + MAX_SHUTDOWN_DOMAINS * (MAX_PATH_BYTES + 32) + 64;

/// Independently supplied operator configuration for ONE registered domain.
/// A missing/offline domain still needs its explicit entry. Profiles and paths
/// are not inferred from untrusted archive bytes or read from current storage.
#[derive(Clone, Debug)]
pub struct FileShutdownMember {
    pub id: u64,
    pub directory: PathBuf,
    pub profile: FileOversightProfile,
}

impl FileShutdownDomain {
    pub fn member(&self) -> FileShutdownMember {
        FileShutdownMember { id: self.id, directory: self.directory.clone(),
            profile: self.profile.as_ref().clone() }
    }
}

impl FileShutdownPlan {
    /// Save exact registrations, operation and budgets before a process exits.
    /// This is operator configuration, NOT an authenticated receipt or a saved
    /// campaign. It contains no roles, permits or successful-stop observations.
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        let mut w = Writer::new(MAX_SHUTDOWN_PLAN_BYTES);
        w.raw(DOMAIN)?; w.u64(self.operation)?;
        w.count(self.max_attempts)?; w.count(self.max_head_bytes)?;
        w.count(self.domains.len())?;
        for domain in self.domains.iter() {
            let path = domain.directory.as_os_str().as_bytes();
            if path.len() > MAX_PATH_BYTES { return Err(Error::Limit); }
            w.u64(domain.id)?; w.blob(path)?; w.count(domain.revision)?;
            w.blob(&domain.anchor)?;
        }
        Ok(w.finish())
    }

    /// Restore the complete roster even when every owner is offline. The
    /// ORIGINAL journal decoder checks each anchor against its independently
    /// supplied bootstrap and exact path. No filesystem read, inference, helper
    /// call, cleanup, recovery fence or role issuance occurs here. Semantic
    /// validation remains the original campaign's owner/canonical inspection.
    ///
    /// Archive authenticity must be established by the operator. Starting the
    /// decoded plan starts a NEW observation session; saved bytes never imply
    /// that a registered domain is currently stopped or drained.
    pub fn decode(bytes: &[u8], mut members: Vec<FileShutdownMember>) -> Result<Self, Error> {
        if bytes.len() > MAX_SHUTDOWN_PLAN_BYTES || members.len() > MAX_SHUTDOWN_DOMAINS {
            return Err(Error::Limit);
        }
        if members.is_empty() { return Err(Error::InvalidInput); }
        members.sort_by_key(|member| member.id);
        for (index, member) in members.iter().enumerate() {
            if member.id == 0 || !member.directory.is_absolute() { return Err(Error::InvalidInput); }
            if member.directory.as_os_str().as_bytes().len() > MAX_PATH_BYTES { return Err(Error::Limit); }
            if members[..index].iter().any(|other| other.id == member.id
                || other.directory == member.directory || other.profile.delivery.scope == member.profile.delivery.scope)
            { return Err(Error::Duplicate); }
        }
        let mut r = Reader::new(bytes);
        if r.take(DOMAIN.len())? != DOMAIN { return Err(Error::Binding); }
        let operation = r.u64()?;
        let attempts = r.count(MAX_SHUTDOWN_ATTEMPTS)?;
        let limit = r.count(MAX_SHUTDOWN_HEAD_BYTES)?;
        let count = r.count(MAX_SHUTDOWN_DOMAINS)?;
        if count != members.len() { return Err(Error::Binding); }
        let mut domains = Vec::new();
        domains.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        let mut retained = 0_usize;
        for member in members {
            if r.u64()? != member.id || r.blob(MAX_PATH_BYTES)? != member.directory.as_os_str().as_bytes() {
                return Err(Error::Binding);
            }
            let revision = r.count(member.profile.delivery.limits.events)?;
            let anchor = r.blob(MAX_SHUTDOWN_HEAD_BYTES)?;
            retained = retained.checked_add(anchor.len()).ok_or(Error::Limit)?;
            if retained > limit { return Err(Error::Limit); }
            let events = journal::decode(&member.profile, &member.directory, anchor)?;
            if events.len() != revision { return Err(Error::Binding); }
            domains.push(FileShutdownDomain { id: member.id, directory: member.directory,
                profile: Rc::new(member.profile), anchor: Rc::from(anchor), revision });
        }
        r.end()?;
        let plan = Self::new(operation, domains, attempts, limit)?;
        if plan.encode()?.as_slice() != bytes { return Err(Error::Binding); }
        Ok(plan)
    }
}
