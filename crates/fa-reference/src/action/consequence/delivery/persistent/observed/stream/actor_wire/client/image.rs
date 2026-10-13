//! Wrap the original recovery image in the existing bounded storage primitives.
use super::*;
use crate::action::consequence::delivery::persistent::codec::shared::{Reader, Writer};
use std::os::unix::ffi::OsStrExt;

const MAGIC: &[u8; 8] = b"FACDISK\x01";
const MAX_PATH_BYTES: usize = 4096;

pub(super) fn encode(path: &Path, profile: &FileClientProfile, revision: u64,
    archive: &[u8], reserved: Option<ClientIoWork>) -> Result<Vec<u8>, FileClientError>
{
    if path.as_os_str().as_bytes().len() > MAX_PATH_BYTES { return Err(Error::Limit.into()); }
    let mut w = Writer::new(profile.image_bytes);
    w.raw(MAGIC)?; w.blob(path.as_os_str().as_bytes())?; w.count(profile.image_bytes)?;
    w.u64(revision)?; w.blob(archive)?; w.u8(u8::from(reserved.is_some()))?;
    if let Some(work) = reserved {
        for n in [work.exchanges, work.calls, work.written_bytes, work.read_bytes] { w.u64(n)?; }
    }
    Ok(w.finish())
}
pub(super) fn decode(path: &Path, profile: &FileClientProfile, minimum_revision: u64,
    bytes: &[u8]) -> Result<FileClientInspection, FileClientError>
{
    if bytes.len() > profile.image_bytes { return Err(Error::Limit.into()); }
    let mut r = Reader::new(bytes);
    if r.take(MAGIC.len())? != MAGIC || r.blob(MAX_PATH_BYTES)? != path.as_os_str().as_bytes()
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
    r.end()?;
    if let Some(ceiling) = reserved { state.check_recovery_reservation(ceiling)?; }
    if revision == 0 && (state.request_count() != 0 || state.work() != ClientIoWork::default() || reserved.is_some()) {
        return Err(Error::Binding.into());
    }
    Ok(FileClientInspection { revision, state, reserved })
}
