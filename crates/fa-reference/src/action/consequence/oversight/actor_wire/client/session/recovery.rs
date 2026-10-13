//! Bounded restart data for the ORIGINAL client. No connection or live ticket is
//! restored. Integrity, anti-rollback and exclusive custody belong to the owner
//! of the saved bytes; decoding a client archive never authenticates a server.
mod errors;
use super::*;
use super::super::super::{decode_command, encode_command, decode_response, MAX_FRAME_BYTES, MAX_RESPONSE_BYTES};

const MAGIC: &[u8; 8] = b"FACREQ\0\x01";
const MAX_BINDING: usize = 4096;
fn invalid() -> ClientError { ClientError::Response(ResponseError::Binding) }

impl ActorClientState {
    /// Maximum complete recovery image, including all original request payloads,
    /// latest observations, independently retained terminal tombstones and errors.
    pub const MAX_RECOVERY_BYTES: usize = 2 * MAX_ACTOR_BYTES
        + MAX_ACTOR_REQUESTS * (2 * MAX_RESPONSE_BYTES + 2048) + MAX_FRAME_BYTES + 8192;

    /// Snapshot a disconnected client for its independently provisioned server
    /// domain. The binding must identify the server domain AND transport profile,
    /// not merely a socket path reused by unrelated servers. It is exact opaque
    /// bootstrap data, not authentication or a self-describing authority grant.
    ///
    /// Saves original proposals, terminal history, ambiguous commands, failure
    /// details and spent I/O allowances. It does not turn a missing response into
    /// nonexecution. Never save only AFTER sending when crash recovery is needed.
    pub fn encode_recovery(&self, binding: &[u8]) -> Result<Vec<u8>, ClientError> {
        encode(self, binding, self.interrupted.as_ref(), self.failure)
    }

    /// Validate the entire bounded image against an independently retained domain,
    /// exact limits and lower bounds. Restored requests have NO visible tickets;
    /// reconnect and explicitly retry the SAME submission to reacquire each one.
    /// No I/O, automatic retry, clock change, endpoint operation or budget refill.
    ///
    /// A floor detects older images, not equal-counter forks. Storage integrity
    /// and ownership remain assumptions. Client history is not a server receipt.
    pub fn decode_recovery(bytes: &[u8], binding: &[u8], limits: ClientSessionLimits,
        minimum_work: ClientIoWork, minimum_requests: usize) -> Result<Self, ClientError>
    {
        if bytes.len() > Self::MAX_RECOVERY_BYTES { return Err(ClientError::Limit); }
        check_binding(binding)?;
        let mut state = Self::new(limits)?;
        let mut r = Reader { bytes, offset: 0 };
        if r.take(MAGIC.len())? != MAGIC || r.blob(MAX_BINDING)? != binding { return Err(invalid()); }
        if read_limits(&mut r)? != limits { return Err(invalid()); }
        let work = read_work(&mut r)?;
        check_work(work, limits.io, minimum_work)?;
        let count = r.count(limits.requests)?;
        if count < minimum_requests || count as u64 > work.exchanges { return Err(invalid()); }
        let mut previous = 0;
        for _ in 0..count {
            let command = read_command(&mut r)?;
            let Command::Submit { request, proposal } = command else { return Err(invalid()); };
            if request <= previous { return Err(invalid()); } previous = request;
            state.payload_bytes = state.payload_bytes.checked_add(proposal.payload.len()).ok_or(ClientError::Limit)?;
            if state.payload_bytes > limits.retained_payload_bytes { return Err(ClientError::Limit); }
            let response = read_response(&mut r, request)?;
            let terminal = match read_response(&mut r, request)? {
                None => None,
                Some(WireResponse { result: Ok(Knowledge::Known { value, basis }), .. }) => Some((value, basis)),
                Some(_) => return Err(invalid()),
            };
            // The original live client updates its terminal tombstone on EVERY
            // Known response, while later Unknown/Withheld cannot erase it.
            match response.as_ref().map(|value| &value.result) {
                Some(Ok(Knowledge::Known { value, basis })) if terminal != Some((*value, *basis)) => return Err(invalid()),
                Some(Ok(Knowledge::Pending { .. })) if terminal.is_some() => return Err(invalid()),
                None if terminal.is_some() => return Err(invalid()),
                _ => {}
            }
            if response.is_some() && (work.calls == 0 || work.read_bytes == 0) { return Err(invalid()); }
            state.requests.insert(request, Request { proposal, visible: false, response, terminal });
        }
        state.interrupted = if r.flag()? {
            let command = read_command(&mut r)?;
            let entry = state.requests.get(&command.request()).ok_or_else(invalid)?;
            if let Command::Submit { proposal, .. } = &command {
                if proposal != &entry.proposal { return Err(invalid()); }
            }
            Some(command)
        } else { None };
        state.failure = if r.flag()? {
            let error = errors::read(&mut r)?;
            let request = r.u64()?;
            let written_bytes = r.count(MAX_FRAME_BYTES + 1)?;
            let request_may_have_reached_peer = r.flag()?;
            if !state.requests.contains_key(&request)
                || (written_bytes != 0 && !request_may_have_reached_peer)
                || (request_may_have_reached_peer && work.calls == 0)
            { return Err(invalid()); }
            Some(ClientFailure { error, request, written_bytes, request_may_have_reached_peer })
        } else { None };
        if r.offset != bytes.len() { return Err(invalid()); }
        if count == 0 && work != ClientIoWork::default() { return Err(invalid()); }
        state.budget.work = work;
        Ok(state)
    }
}

fn check_binding(binding: &[u8]) -> Result<(), ClientError> {
    if binding.is_empty() { return Err(invalid()); }
    if binding.len() > MAX_BINDING { return Err(ClientError::Limit); }
    Ok(())
}
fn check_work(work: ClientIoWork, limits: ClientIoLimits, minimum: ClientIoWork) -> Result<(), ClientError> {
    for (actual, maximum, floor) in [
        (work.exchanges, limits.exchanges, minimum.exchanges), (work.calls, limits.calls, minimum.calls),
        (work.written_bytes, limits.written_bytes, minimum.written_bytes), (work.read_bytes, limits.read_bytes, minimum.read_bytes),
    ] { if actual > maximum || actual < floor { return Err(invalid()); } }
    if work.calls == 0 && (work.written_bytes != 0 || work.read_bytes != 0) { return Err(invalid()); }
    Ok(())
}
fn encode(state: &ActorClientState, binding: &[u8], interrupted: Option<&Command>,
    failure: Option<ClientFailure>) -> Result<Vec<u8>, ClientError>
{
    check_binding(binding)?;
    let mut w = Writer(Vec::new()); w.raw(MAGIC)?; w.blob(binding)?;
    write_limits(&mut w, state.limits)?; write_work(&mut w, state.budget.work)?;
    w.count(state.requests.len())?;
    for (&request, entry) in &state.requests {
        w.blob(&encode_command(&Command::Submit { request, proposal: entry.proposal.clone() }).map_err(ClientError::Command)?)?;
        write_response(&mut w, entry.response.as_ref())?;
        let terminal = entry.terminal.map(|(value, basis)| WireResponse {
            request: Some(request), result: Ok(Knowledge::Known { value, basis }) });
        write_response(&mut w, terminal.as_ref())?;
    }
    w.byte(u8::from(interrupted.is_some()))?;
    if let Some(command) = interrupted { w.blob(&encode_command(command).map_err(ClientError::Command)?)?; }
    w.byte(u8::from(failure.is_some()))?;
    if let Some(failure) = failure {
        errors::write(&mut w, failure.error)?; w.u64(failure.request)?;
        w.count(failure.written_bytes)?; w.byte(u8::from(failure.request_may_have_reached_peer))?;
    }
    Ok(w.0)
}
fn write_limits(w: &mut Writer, limits: ClientSessionLimits) -> Result<(), ClientError> {
    w.count(limits.requests)?; w.count(limits.retained_payload_bytes)?;
    write_work(w, ClientIoWork { exchanges: limits.io.exchanges, calls: limits.io.calls,
        written_bytes: limits.io.written_bytes, read_bytes: limits.io.read_bytes })
}
fn read_limits(r: &mut Reader<'_>) -> Result<ClientSessionLimits, ClientError> {
    let requests = r.count(MAX_ACTOR_REQUESTS)?; let retained_payload_bytes = r.count(MAX_ACTOR_BYTES)?;
    let work = read_work(r)?;
    Ok(ClientSessionLimits { requests, retained_payload_bytes, io: ClientIoLimits {
        exchanges: work.exchanges, calls: work.calls, written_bytes: work.written_bytes, read_bytes: work.read_bytes } })
}
fn write_work(w: &mut Writer, work: ClientIoWork) -> Result<(), ClientError> {
    for value in [work.exchanges, work.calls, work.written_bytes, work.read_bytes] { w.u64(value)?; } Ok(())
}
fn read_work(r: &mut Reader<'_>) -> Result<ClientIoWork, ClientError> {
    Ok(ClientIoWork { exchanges: r.u64()?, calls: r.u64()?, written_bytes: r.u64()?, read_bytes: r.u64()? })
}
fn read_command(r: &mut Reader<'_>) -> Result<Command, ClientError> {
    let bytes = r.blob(MAX_FRAME_BYTES)?;
    let command = decode_command(bytes).map_err(ClientError::Command)?;
    if encode_command(&command).map_err(ClientError::Command)?.as_slice() != bytes { return Err(invalid()); }
    Ok(command)
}
fn write_response(w: &mut Writer, response: Option<&WireResponse>) -> Result<(), ClientError> {
    w.byte(u8::from(response.is_some()))?;
    if let Some(response) = response { w.blob(&response.encode())?; } Ok(())
}
fn read_response(r: &mut Reader<'_>, request: u64) -> Result<Option<WireResponse>, ClientError> {
    if !r.flag()? { return Ok(None); }
    let bytes = r.blob(MAX_RESPONSE_BYTES)?;
    let response = decode_response(bytes).map_err(ClientError::Response)?;
    if response.request != Some(request) || response.encode().as_slice() != bytes { return Err(invalid()); }
    Ok(Some(response))
}

// Only representation helpers: the ORIGINAL command/response codecs do all
// protocol parsing. Check every aggregate bound before extending or allocating.
struct Writer(Vec<u8>);
impl Writer {
    fn raw(&mut self, bytes: &[u8]) -> Result<(), ClientError> {
        if self.0.len().checked_add(bytes.len()).ok_or(ClientError::Limit)? > ActorClientState::MAX_RECOVERY_BYTES { return Err(ClientError::Limit); }
        self.0.try_reserve(bytes.len()).map_err(|_| ClientError::Limit)?;
        self.0.extend_from_slice(bytes); Ok(())
    }
    fn byte(&mut self, n: u8) -> Result<(), ClientError> { self.raw(&[n]) }
    fn u64(&mut self, n: u64) -> Result<(), ClientError> { self.raw(&n.to_be_bytes()) }
    fn count(&mut self, n: usize) -> Result<(), ClientError> { self.u64(u64::try_from(n).map_err(|_| ClientError::Limit)?) }
    fn blob(&mut self, bytes: &[u8]) -> Result<(), ClientError> { self.count(bytes.len())?; self.raw(bytes) }
}
struct Reader<'a> { bytes: &'a [u8], offset: usize }
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], ClientError> {
        let end = self.offset.checked_add(n).ok_or(ClientError::Limit)?;
        let value = self.bytes.get(self.offset..end).ok_or_else(invalid)?; self.offset = end; Ok(value)
    }
    fn byte(&mut self) -> Result<u8, ClientError> { Ok(self.take(1)?[0]) }
    fn flag(&mut self) -> Result<bool, ClientError> {
        match self.byte()? { 0 => Ok(false), 1 => Ok(true), _ => Err(invalid()) }
    }
    fn u64(&mut self) -> Result<u64, ClientError> { Ok(u64::from_be_bytes(self.take(8)?.try_into().map_err(|_| invalid())?)) }
    fn count(&mut self, maximum: usize) -> Result<usize, ClientError> {
        let n = usize::try_from(self.u64()?).map_err(|_| ClientError::Limit)?;
        if n > maximum { return Err(ClientError::Limit); } Ok(n)
    }
    fn blob(&mut self, maximum: usize) -> Result<&'a [u8], ClientError> { let n = self.count(maximum)?; self.take(n) }
}

#[cfg(test)]
mod tests;
