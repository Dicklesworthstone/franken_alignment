//! Wrap the original recovery image in the existing bounded storage primitives.
use super::*;
use crate::action::consequence::delivery::persistent::codec::shared::{Reader, Writer};
use std::os::unix::ffi::OsStrExt;

const MAGIC: &[u8; 8] = b"FACDISK\x01";
const DRIVE_MAGIC: &[u8; 8] = b"FACDISK\x02";
const MAX_PATH_BYTES: usize = 4096;

pub(super) fn encode(path: &Path, profile: &FileClientProfile, revision: u64,
    archive: &[u8], reserved: Option<ClientIoWork>) -> Result<Vec<u8>, FileClientError>
{
    encode_inner(path, profile, revision, archive, reserved, None)
}
pub(super) fn encode_drive(path: &Path, profile: &FileClientProfile, revision: u64,
    archive: &[u8], ceiling: ClientIoWork, budget: ClientDriveBudget) -> Result<Vec<u8>, FileClientError>
{
    if budget.steps() < 2 { return Err(Error::InvalidInput.into()); }
    encode_inner(path, profile, revision, archive, Some(ceiling), Some(budget))
}
fn encode_inner(path: &Path, profile: &FileClientProfile, revision: u64,
    archive: &[u8], reserved: Option<ClientIoWork>, drive: Option<ClientDriveBudget>) -> Result<Vec<u8>, FileClientError>
{
    if path.as_os_str().as_bytes().len() > MAX_PATH_BYTES { return Err(Error::Limit.into()); }
    let mut w = Writer::new(profile.image_bytes);
    w.raw(if drive.is_some() { DRIVE_MAGIC } else { MAGIC })?; w.blob(path.as_os_str().as_bytes())?; w.count(profile.image_bytes)?;
    w.u64(revision)?; w.blob(archive)?; w.u8(u8::from(reserved.is_some()))?;
    if let Some(work) = reserved {
        for n in [work.exchanges, work.calls, work.written_bytes, work.read_bytes] { w.u64(n)?; }
    }
    if let Some(budget) = drive { w.u64(budget.steps())?; }
    Ok(w.finish())
}
pub(super) fn decode(path: &Path, profile: &FileClientProfile, minimum_revision: u64,
    bytes: &[u8]) -> Result<FileClientInspection, FileClientError>
{
    if bytes.len() > profile.image_bytes { return Err(Error::Limit.into()); }
    let mut r = Reader::new(bytes);
    let magic = r.take(MAGIC.len())?;
    let batched = magic == DRIVE_MAGIC;
    if (!batched && magic != MAGIC) || r.blob(MAX_PATH_BYTES)? != path.as_os_str().as_bytes()
        || r.count(MAX_JOURNAL_BYTES)? != profile.image_bytes { return Err(Error::Binding.into()); }
    let revision = r.u64()?;
    if revision < minimum_revision { return Err(Error::Stale.into()); }
    let state = ActorClientState::decode_recovery(r.blob(ActorClientState::MAX_RECOVERY_BYTES)?,
        &profile.binding, profile.limits, ClientIoWork::default(), 0)?;
    let reserved = match r.u8()? {
        0 => None,
        1 => Some(ClientIoWork { exchanges: r.u64()?, calls: r.u64()?, written_bytes: r.u64()?, read_bytes: r.u64()? }),
        _ => return Err(Error::InvalidInput.into()),
    };
    let reserved_drive = if batched {
        let budget = ClientDriveBudget::new(r.u64()?)?;
        if budget.steps() < 2 || reserved.is_none() { return Err(Error::Binding.into()); }
        Some(budget)
    } else { None };
    r.end()?;
    if let Some(ceiling) = reserved {
        match reserved_drive {
            None => state.check_recovery_reservation(ceiling)?,
            Some(budget) => state.check_recovery_drive_reservation(ceiling, budget)?,
        }
    }
    if revision == 0 && (state.request_count() != 0 || state.work() != ClientIoWork::default() || reserved.is_some()) {
        return Err(Error::Binding.into());
    }
    Ok(FileClientInspection { revision, state, reserved, reserved_drive })
}
