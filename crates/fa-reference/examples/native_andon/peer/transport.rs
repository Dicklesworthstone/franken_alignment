//! Finite peer I/O under the caller's ONE invocation budget. Socket progress
//! never renews that budget; native journal execution is outside this module.
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;

pub(super) const MAX_MESSAGE_BYTES: usize = 64 * 1024;

pub(super) fn read(
    stream: &mut UnixStream,
    maximum: usize,
    checkpoint: &mut impl FnMut() -> Result<(), String>,
    wait: &mut impl FnMut(),
) -> Result<Vec<u8>, String> {
    if maximum == 0 || maximum > MAX_MESSAGE_BYTES {
        return Err("invalid peer read bound".into());
    }
    checkpoint()?;
    stream
        .set_nonblocking(true)
        .map_err(|error| format!("peer nonblocking setup: {error}"))?;
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        checkpoint()?;
        // Read at most ONE byte beyond the declared bound to distinguish an
        // exact-sized document from an overlong one. EOF alone seals a frame.
        let allowance = chunk.len().min(maximum + 1 - bytes.len());
        let result = stream.read(&mut chunk[..allowance]);
        checkpoint()?;
        match result {
            Ok(0) if bytes.is_empty() => return Err("empty peer document".into()),
            Ok(0) => return Ok(bytes),
            Ok(count) => {
                if bytes.len() + count > maximum {
                    return Err("peer document exceeds byte limit".into());
                }
                bytes
                    .try_reserve(count)
                    .map_err(|_| "peer document allocation limit")?;
                bytes.extend_from_slice(&chunk[..count]);
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                wait()
            }
            Err(error) => return Err(format!("peer read failed: {error}")),
        }
    }
}

pub(super) fn write(
    stream: &mut UnixStream,
    bytes: &[u8],
    checkpoint: &mut impl FnMut() -> Result<(), String>,
    wait: &mut impl FnMut(),
) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > MAX_MESSAGE_BYTES {
        return Err("invalid peer write bound".into());
    }
    checkpoint()?;
    stream
        .set_nonblocking(true)
        .map_err(|error| format!("peer nonblocking setup: {error}"))?;
    let mut written = 0;
    while written < bytes.len() {
        checkpoint()?;
        let end = (written + 4096).min(bytes.len());
        let result = stream.write(&bytes[written..end]);
        // Even the last successful syscall can cross the deadline. Its bytes
        // may be visible, but that does not acknowledge peer receipt or effects.
        checkpoint()?;
        match result {
            Ok(0) => return Err("peer write made no progress".into()),
            Ok(count) => written += count,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                wait()
            }
            Err(error) => return Err(format!("peer write failed: {error}")),
        }
    }
    Ok(())
}

/// Bound native report capture BEFORE entering an untrusted socket. A failed
/// output after a journal write is not a rollback or permission to retry it.
#[derive(Default)]
pub(super) struct Output(pub(super) Vec<u8>);
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_MESSAGE_BYTES.saturating_sub(self.0.len()) {
            return Err(io::ErrorKind::InvalidData.into());
        }
        self.0
            .try_reserve(bytes.len())
            .map_err(|_| io::ErrorKind::OutOfMemory)?;
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[path = "transport/tests.rs"]
mod tests;
