//! Exact finite tags for existing client failures. Unlisted I/O kinds
//! refuse serialization rather than being silently rewritten to Other/success.
use super::{ClientError, Reader, Writer, WireError, ResponseError, invalid};
use std::io::ErrorKind;

const WIRE: [WireError; 6] = [WireError::MalformedRequest, WireError::UnsupportedVersion,
    WireError::IdempotencyConflict, WireError::Capacity, WireError::Unavailable, WireError::Withheld];
const RESPONSE: [ResponseError; 4] = [ResponseError::Malformed, ResponseError::UnsupportedVersion,
    ResponseError::Capacity, ResponseError::Binding];
const IO: [ErrorKind; 21] = [ErrorKind::NotFound, ErrorKind::PermissionDenied,
    ErrorKind::ConnectionRefused, ErrorKind::ConnectionReset, ErrorKind::ConnectionAborted,
    ErrorKind::NotConnected, ErrorKind::AddrInUse, ErrorKind::AddrNotAvailable, ErrorKind::BrokenPipe,
    ErrorKind::AlreadyExists, ErrorKind::WouldBlock, ErrorKind::InvalidInput, ErrorKind::InvalidData,
    ErrorKind::TimedOut, ErrorKind::WriteZero, ErrorKind::Interrupted, ErrorKind::Unsupported,
    ErrorKind::UnexpectedEof, ErrorKind::OutOfMemory, ErrorKind::Other, ErrorKind::NotADirectory];
fn tag<T: PartialEq>(values: &[T], value: T) -> Result<u8, ClientError> {
    values.iter().position(|item| item == &value).map(|n| n as u8)
        .ok_or(ClientError::Response(ResponseError::UnsupportedVersion))
}
pub(super) fn write(w: &mut Writer, error: ClientError) -> Result<(), ClientError> {
    match error {
        ClientError::Command(error) => { w.byte(0)?; w.byte(tag(&WIRE, error)?) }
        ClientError::Response(error) => { w.byte(1)?; w.byte(tag(&RESPONSE, error)?) }
        ClientError::Io(error) => { w.byte(2)?; w.byte(tag(&IO, error)?) }
        ClientError::Limit => w.byte(3), ClientError::Busy => w.byte(4),
        ClientError::Disconnected => w.byte(5), ClientError::TicketUnavailable => w.byte(6),
        ClientError::InterruptedOperation => w.byte(7),
    }
}
pub(super) fn read(r: &mut Reader<'_>) -> Result<ClientError, ClientError> {
    Ok(match r.byte()? {
        0 => ClientError::Command(*WIRE.get(usize::from(r.byte()?)).ok_or_else(invalid)?),
        1 => ClientError::Response(*RESPONSE.get(usize::from(r.byte()?)).ok_or_else(invalid)?),
        2 => ClientError::Io(*IO.get(usize::from(r.byte()?)).ok_or_else(invalid)?),
        3 => ClientError::Limit, 4 => ClientError::Busy, 5 => ClientError::Disconnected,
        6 => ClientError::TicketUnavailable, 7 => ClientError::InterruptedOperation,
        _ => return Err(invalid()),
    })
}
