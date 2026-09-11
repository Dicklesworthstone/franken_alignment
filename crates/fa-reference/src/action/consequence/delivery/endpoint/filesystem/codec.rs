//! Strict replay format for the filesystem endpoint, not a portable permit.
//! Only the surviving recovery key can rebind records to a live broker. Read-only
//! imports use a new private brand and export payload data, never receipt tokens.

use super::*;
use crate::action::{Purpose, VERSION};

const DOMAIN: &[u8; 8] = b"FAPUBFS\x01";
const MAX_INITIAL_BYTES: usize = MAX_PAYLOAD_BYTES + 128;
const MAX_EVENT_BYTES: usize = MAX_PAYLOAD_BYTES + 256;

pub(super) struct Decoded {
    pub(super) endpoint: PublicationEndpoint,
    pub(super) initial: Vec<u8>,
    pub(super) events: Vec<Vec<u8>>,
    pub(super) limits: FilePublicationLimits,
}

pub(super) fn encode_initial(endpoint: &PublicationEndpoint) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    put_target(&mut bytes, endpoint.resource);
    put64(&mut bytes, endpoint.retention_ticks);
    put32(&mut bytes, endpoint.max_deliveries)?;
    match &endpoint.stream {
        None => bytes.push(0),
        Some(view) => {
            if view.message_count() != 0 || view.finished() || !endpoint.payload.is_empty() {
                return Err(Error::WrongState);
            }
            bytes.push(1);
            let profile = view.profile();
            put64(&mut bytes, profile.id());
            put64(&mut bytes, profile.generation());
            put32(&mut bytes, profile.max_messages())?;
            put32(&mut bytes, profile.max_message_bytes())?;
            put32(&mut bytes, profile.max_stream_bytes())?;
        }
    }
    put_bytes(&mut bytes, &endpoint.payload)?;
    Ok(bytes)
}

fn decode_initial(bytes: &[u8], binding: Rc<()>) -> Result<PublicationEndpoint, Error> {
    if bytes.len() > MAX_INITIAL_BYTES { return Err(Error::Limit); }
    let mut reader = Reader::new(bytes);
    let target = reader.target()?;
    let retention = reader.u64()?;
    let max_deliveries = reader.length()?;
    let profile = match reader.byte()? {
        0 => None,
        1 => Some(StreamProfile::new(reader.u64()?, reader.u64()?, reader.length()?,
            reader.length()?, reader.length()?)?),
        _ => return Err(Error::InvalidInput),
    };
    let payload = reader.vector(MAX_PAYLOAD_BYTES)?;
    reader.finish()?;
    let mut endpoint = match profile {
        Some(profile) => {
            if !payload.is_empty() { return Err(Error::Binding); }
            PublicationEndpoint::new_stream(target, profile, retention, max_deliveries)?
        }
        None => PublicationEndpoint::new(target, payload, retention, max_deliveries)?,
    };
    endpoint.binding = binding;
    if encode_initial(&endpoint)? != bytes { return Err(Error::InvalidInput); }
    Ok(endpoint)
}

pub(super) fn encode_file(
    initial: &[u8], events: &[Vec<u8>], limits: FilePublicationLimits,
) -> Result<Vec<u8>, Error> {
    encode_history(initial, events, None, limits)
}

pub(super) fn encode_appended_file(
    initial: &[u8], events: &[Vec<u8>], next: &[u8], limits: FilePublicationLimits,
) -> Result<Vec<u8>, Error> {
    encode_history(initial, events, Some(next), limits)
}

fn encode_history(
    initial: &[u8], events: &[Vec<u8>], next: Option<&[u8]>, limits: FilePublicationLimits,
) -> Result<Vec<u8>, Error> {
    limits.validate()?;
    if initial.len() > MAX_INITIAL_BYTES { return Err(Error::Limit); }
    let count = events.len().checked_add(usize::from(next.is_some())).ok_or(Error::Overflow)?;
    if count > limits.mutations { return Err(Error::Limit); }
    let mut length = 32_usize.checked_add(initial.len()).ok_or(Error::Overflow)?;
    for event in events.iter().map(Vec::as_slice).chain(next) {
        if event.is_empty() || event.len() > MAX_EVENT_BYTES { return Err(Error::Limit); }
        length = length.checked_add(4).and_then(|n| n.checked_add(event.len())).ok_or(Error::Overflow)?;
    }
    if length > limits.bytes { return Err(Error::Limit); }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|_| Error::Limit)?;
    bytes.extend_from_slice(DOMAIN);
    put32(&mut bytes, limits.mutations)?;
    put32(&mut bytes, limits.bytes)?;
    put64(&mut bytes, count as u64);
    put_bytes(&mut bytes, initial)?;
    put32(&mut bytes, count)?;
    for event in events.iter().map(Vec::as_slice).chain(next) { put_bytes(&mut bytes, event)?; }
    if bytes.len() != length { return Err(Error::Binding); }
    Ok(bytes)
}

pub(super) fn decode_file(bytes: &[u8], binding: Rc<()>) -> Result<Decoded, Error> {
    if bytes.len() > MAX_PUBLICATION_FILE_BYTES { return Err(Error::Limit); }
    let mut reader = Reader::new(bytes);
    if reader.take(8)? != DOMAIN { return Err(Error::InvalidInput); }
    let limits = FilePublicationLimits { mutations: reader.length()?, bytes: reader.length()? };
    limits.validate()?;
    if bytes.len() > limits.bytes { return Err(Error::Limit); }
    let revision = reader.u64()?;
    let initial = reader.vector(MAX_INITIAL_BYTES)?;
    let mut endpoint = decode_initial(&initial, binding)?;
    let count = reader.length()?;
    if count > limits.mutations { return Err(Error::Limit); }
    if revision != count as u64 { return Err(Error::Binding); }
    let mut events = Vec::new();
    events.try_reserve_exact(count).map_err(|_| Error::Limit)?;
    for _ in 0..count {
        let event = reader.vector(MAX_EVENT_BYTES)?;
        let operation = decode_operation(&event, Rc::clone(&endpoint.binding))?;
        // Every admitted non-idempotent endpoint operation changes this stamp.
        // Do not clone all prior payloads for each replayed clock/fence event.
        let before = (endpoint.scope, endpoint.epoch, endpoint.elapsed, endpoint.receipts.len(), endpoint.executions);
        operation.apply(&mut endpoint)?;
        let after = (endpoint.scope, endpoint.epoch, endpoint.elapsed, endpoint.receipts.len(), endpoint.executions);
        if before == after { return Err(Error::InvalidInput); }
        events.push(event);
    }
    reader.finish()?;
    Ok(Decoded { endpoint, initial, events, limits })
}

pub(super) fn encode_operation(operation: &Operation) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    match operation {
        Operation::Attach(scope) => { bytes.push(0); put_scope(&mut bytes, *scope); }
        Operation::ObserveTime(tick) => { bytes.push(1); put64(&mut bytes, tick.0); }
        Operation::Fence(request) => { bytes.push(2); put64(&mut bytes, request.epoch); }
        Operation::Deliver(message) => { bytes.push(3); put_message(&mut bytes, message)?; }
        Operation::Seal(query) => { bytes.push(4); put_message(&mut bytes, &query.0)?; }
        Operation::ResolveExpired(query) => { bytes.push(5); put_message(&mut bytes, &query.0)?; }
    }
    if bytes.len() > MAX_EVENT_BYTES { return Err(Error::Limit); }
    Ok(bytes)
}

fn decode_operation(bytes: &[u8], binding: Rc<()>) -> Result<Operation, Error> {
    let mut reader = Reader::new(bytes);
    let operation = match reader.byte()? {
        0 => Operation::Attach(reader.scope()?),
        1 => Operation::ObserveTime(ElapsedTick(reader.u64()?)),
        2 => Operation::Fence(FenceRequest { binding, epoch: reader.u64()? }),
        3 => Operation::Deliver(reader.message(binding)?),
        4 => Operation::Seal(StatusQuery(reader.message(binding)?)),
        5 => Operation::ResolveExpired(StatusQuery(reader.message(binding)?)),
        _ => return Err(Error::InvalidInput),
    };
    reader.finish()?;
    if encode_operation(&operation)? != bytes { return Err(Error::InvalidInput); }
    Ok(operation)
}

fn put64(bytes: &mut Vec<u8>, value: u64) { bytes.extend_from_slice(&value.to_be_bytes()); }
fn put32(bytes: &mut Vec<u8>, value: usize) -> Result<(), Error> {
    bytes.extend_from_slice(&u32::try_from(value).map_err(|_| Error::Limit)?.to_be_bytes());
    Ok(())
}
fn put_bytes(bytes: &mut Vec<u8>, value: &[u8]) -> Result<(), Error> {
    put32(bytes, value.len())?;
    bytes.extend_from_slice(value);
    Ok(())
}
fn put_target(bytes: &mut Vec<u8>, target: ResolvedTarget) {
    for value in [target.adapter, target.object, target.contract_version, target.expected_version, target.generation] {
        put64(bytes, value);
    }
}
fn put_scope(bytes: &mut Vec<u8>, scope: Scope) {
    for value in [scope.tenant, scope.principal, scope.run, scope.branch, scope.authority] { put64(bytes, value); }
    bytes.push(match scope.purpose { Purpose::Effect => 0, Purpose::Experiment => 1 });
}
fn put_message(bytes: &mut Vec<u8>, message: &DispatchEnvelope) -> Result<(), Error> {
    for value in [message.epoch, message.attempt, message.retained_until.0] { put64(bytes, value); }
    bytes.extend_from_slice(&message.request.version.to_be_bytes());
    put_scope(bytes, message.request.scope);
    put_target(bytes, message.request.target);
    for value in [message.request.policy_epoch, message.request.deadline.0, message.request.units] { put64(bytes, value); }
    put_bytes(bytes, &message.request.payload)?;
    match message.request.approval {
        None => bytes.push(0),
        Some(approval) => {
            bytes.push(1);
            for value in [approval.request(), approval.reviewer(), approval.issued_at().0, approval.expires_at().0] {
                put64(bytes, value);
            }
        }
    }
    Ok(())
}

struct Reader<'a> { bytes: &'a [u8], position: usize }
impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self { Self { bytes, position: 0 } }
    fn take(&mut self, count: usize) -> Result<&'a [u8], Error> {
        let end = self.position.checked_add(count).ok_or(Error::Overflow)?;
        let bytes = self.bytes.get(self.position..end).ok_or(Error::Incomplete)?;
        self.position = end;
        Ok(bytes)
    }
    fn byte(&mut self) -> Result<u8, Error> { Ok(self.take(1)?[0]) }
    fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().map_err(|_| Error::Incomplete)?))
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().map_err(|_| Error::Incomplete)?))
    }
    fn length(&mut self) -> Result<usize, Error> { usize::try_from(self.u32()?).map_err(|_| Error::Limit) }
    fn vector(&mut self, limit: usize) -> Result<Vec<u8>, Error> {
        let length = self.length()?;
        if length > limit { return Err(Error::Limit); }
        Ok(self.take(length)?.to_vec())
    }
    fn finish(&self) -> Result<(), Error> {
        if self.position != self.bytes.len() { return Err(Error::InvalidInput); }
        Ok(())
    }
    fn scope(&mut self) -> Result<Scope, Error> {
        let scope = Scope { tenant: self.u64()?, principal: self.u64()?, run: self.u64()?,
            branch: self.u64()?, authority: self.u64()?, purpose: Purpose::Effect };
        if self.byte()? != 0 || [scope.tenant, scope.principal, scope.run, scope.branch, scope.authority].contains(&0) {
            return Err(Error::Binding);
        }
        Ok(scope)
    }
    fn target(&mut self) -> Result<ResolvedTarget, Error> {
        let target = ResolvedTarget { adapter: self.u64()?, object: self.u64()?, contract_version: self.u64()?,
            expected_version: self.u64()?, generation: self.u64()? };
        if [target.adapter, target.object, target.contract_version, target.expected_version, target.generation].contains(&0) {
            return Err(Error::InvalidInput);
        }
        Ok(target)
    }
    fn message(&mut self, binding: Rc<()>) -> Result<DispatchEnvelope, Error> {
        let epoch = self.u64()?;
        let attempt = self.u64()?;
        let retained_until = ElapsedTick(self.u64()?);
        let version = self.u32()?;
        if attempt == 0 || version != VERSION { return Err(Error::InvalidInput); }
        let scope = self.scope()?;
        let target = self.target()?;
        let policy_epoch = self.u64()?;
        let deadline = ElapsedTick(self.u64()?);
        let units = self.u64()?;
        let payload = self.vector(MAX_PAYLOAD_BYTES)?;
        let action = FrozenAction::freeze(ActionSpec { version, scope, target: Some(target), payload,
            required_witnesses: Vec::new(), policy_epoch, deadline, units })?;
        let approval = match self.byte()? {
            0 => None,
            1 => {
                let approval = DispatchApproval::new(self.u64()?, self.u64()?, ElapsedTick(self.u64()?), ElapsedTick(self.u64()?))?;
                if approval.expires_at() > deadline { return Err(Error::Binding); }
                Some(approval)
            }
            _ => return Err(Error::InvalidInput),
        };
        Ok(DispatchEnvelope { binding, epoch, attempt, retained_until,
            request: PublicationRequest { approval, ..PublicationRequest::from_action(&action) } })
    }
}
