//! Finite actor and separately configured operator Tower document exchanges.
//! Replies remain observations. This client does not infer execution, rewrite
//! proposals, select another key, reconnect, resend or grant a capability.

use super::super::operator::Settings as OperatorSettings;
use super::super::{Duration, Instant, Profile, debug, pause};
use crate::config::read_regular;
use fa_reference::action::consequence::oversight::actor_peer::PeerPolicy;
use fa_reference::action::consequence::oversight::actor_wire::client::{
    CLIENT_IO_CHUNK, MAX_CLIENT_IO_BYTES, MAX_CLIENT_IO_CALLS,
};
use fa_reference::strict_json::{self, Json, Limits};
use fa_reference::tower::publication::MAX_PUBLICATION_RESPONSE_BYTES;
use fa_reference::tower::{
    ENVELOPE_FIELDS, MAX_REQUEST_BYTES, MAX_REQUEST_STRING_BYTES, RESPONSE_SCHEMA, TowerRequest,
    Verb, decode_request, encode_request,
};
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;

#[path = "client/policy_replay.rs"]
mod policy_replay;
#[path = "client/situation.rs"]
mod situation;

const MAX_DOCUMENTS: usize = 64;
const MAX_REPLY_BYTES: usize = MAX_PUBLICATION_RESPONSE_BYTES;
// Complete policy reports repeat canonical decimal strings and trace objects.
// Their densest repeated arrays require at least four wire bytes per item;
// this allowance covers the original 1 MiB response without unbounded parsing.
// Other operations retain the smaller existing parser-work allowance.
const MAX_POLICY_REPLAY_REPLY_ITEMS: usize = MAX_REPLY_BYTES / 4;
const USAGE: &str = "tower-exchange ACTOR_PROFILE TOWER_REQUEST_JSON [TOWER_REQUEST_JSON ...] (at most 64 documents; no automatic poll or resend)";
const OPERATOR_USAGE: &str = "tower-operator-exchange OPERATOR_PROFILE TOWER_REQUEST_JSON [TOWER_REQUEST_JSON ...] (at most 64 documents; no automatic page following or resend)";

struct Prepared {
    request: TowerRequest,
    frame: Vec<u8>,
}

fn request_paths<'a>(
    args: &'a [String],
    command: &str,
    usage: &str,
) -> Result<Vec<&'a Path>, String> {
    if !(3..=MAX_DOCUMENTS + 2).contains(&args.len())
        || args[0] != command
        || args[1..]
            .iter()
            .any(|value| value.is_empty() || value.starts_with("--"))
    {
        return Err(usage.into());
    }
    Ok(args[2..].iter().map(Path::new).collect())
}

pub(crate) fn command(args: &[String], output: &mut impl Write) -> Result<(), String> {
    let paths = request_paths(args, "tower-exchange", USAGE)?;
    let started = Instant::now();
    // This public connection profile is the only configuration read here. No
    // supervisor, evidence source, journal, helper or reviewer file is opened.
    let profile = Profile::read(Path::new(&args[1]))?;
    exchange(&profile, &paths, output, started)
}

/// The operator connection profile supplies only endpoint identity and bounds.
/// Neither its disclosure declarations nor this command create a server grant.
/// Each explicit document is forwarded unchanged in meaning, with no automatic
/// continuation, policy query, proposal, retry or interpretation of permission.
pub(crate) fn operator_command(args: &[String], output: &mut impl Write) -> Result<(), String> {
    let paths = request_paths(args, "tower-operator-exchange", OPERATOR_USAGE)?;
    let started = Instant::now();
    let settings = OperatorSettings::read(Path::new(&args[1]))?;
    exchange_endpoint(
        Endpoint::operator(&settings),
        &paths,
        output,
        started,
        || settings.check_directory(),
    )
}

/// Enumerate one native historical situation capture within the caller's
/// explicit page allowance and the original transport and server grants.
pub(crate) fn situation_command(args: &[String], output: &mut impl Write) -> Result<(), String> {
    situation::command(args, output)
}

pub(crate) fn policy_replay_command(
    args: &[String],
    output: &mut impl Write,
) -> Result<(), String> {
    policy_replay::command(args, output)
}

/// Only shared transport parameters. An operator profile is never converted
/// into an actor profile, given an invented request key or used as TowerAccess.
#[derive(Clone, Copy)]
struct Endpoint<'a> {
    socket: &'a Path,
    supervisor: PeerPolicy,
    exchanges: u64,
    runtime_ms: u64,
    poll_ms: u64,
}

impl<'a> Endpoint<'a> {
    fn actor(profile: &'a Profile) -> Self {
        Self {
            socket: &profile.socket,
            supervisor: profile.supervisor,
            exchanges: profile.exchanges,
            runtime_ms: profile.runtime_ms,
            poll_ms: profile.poll_ms,
        }
    }

    fn operator(settings: &'a OperatorSettings) -> Self {
        Self {
            socket: &settings.socket,
            supervisor: settings.supervisor,
            exchanges: settings.exchange_limit,
            runtime_ms: settings.runtime_ms,
            poll_ms: settings.poll_ms,
        }
    }
}

fn exchange(
    profile: &Profile,
    paths: &[&Path],
    output: &mut impl Write,
    started: Instant,
) -> Result<(), String> {
    exchange_endpoint(Endpoint::actor(profile), paths, output, started, || {
        profile.check_directory()
    })
}

fn exchange_endpoint(
    endpoint: Endpoint<'_>,
    paths: &[&Path],
    output: &mut impl Write,
    started: Instant,
    check_directory: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    if paths.is_empty() || paths.len() > MAX_DOCUMENTS || paths.len() as u64 > endpoint.exchanges {
        return Err("Tower document sequence exceeds its original exchange allowance".into());
    }
    let mut budget = Budget::new(&endpoint, started);
    let mut documents = Vec::with_capacity(paths.len());
    // Validate and freeze the WHOLE sequence before connecting. A malformed
    // later file cannot follow a successfully submitted earlier effect.
    for path in paths {
        budget.check()?;
        let request = decode_request(&read_regular(path, MAX_REQUEST_BYTES)?)
            .map_err(|_| "invalid bounded Tower request document".to_owned())?;
        let frame = request_frame(&request)?;
        documents.push(Prepared { request, frame });
    }
    let mut socket = connect_endpoint(endpoint, &budget, check_directory)?;
    for document in documents {
        budget.write_frame(&mut socket, &document.frame)?;
        let response = budget.read_frame(&mut socket)?;
        validate_response(&response, &document.request)?;
        emit_response(&response, output, &budget)?;
    }
    Ok(())
}

fn request_frame(request: &TowerRequest) -> Result<Vec<u8>, String> {
    let mut frame = encode_request(request)
        .map_err(|_| "Tower request could not be canonically framed".to_owned())?
        .into_bytes();
    // Canonicalization changes only JSON spelling, never request identity,
    // proposal bytes or integers. A pretty-printed file becomes one frame.
    frame.push(b'\n');
    Ok(frame)
}

fn connect_endpoint(
    endpoint: Endpoint<'_>,
    budget: &Budget,
    check_directory: impl FnOnce() -> Result<(), String>,
) -> Result<UnixStream, String> {
    budget.check()?;
    check_directory()?;
    let socket = UnixStream::connect(endpoint.socket)
        .map_err(|error| format!("Tower connection failed: {error}; no request was sent"))?;
    // Check this connected server before writing or reading ANY protocol data.
    endpoint.supervisor.verify(&socket).map_err(debug)?;
    socket.set_nonblocking(true).map_err(debug)?;
    budget.check()?;
    Ok(socket)
}

fn emit_response(response: &[u8], output: &mut impl Write, budget: &Budget) -> Result<(), String> {
    budget.check()?;
    // Emit each original complete reply without interpreting Unknown,
    // Pending, denial or a successful intake as execution or permission.
    output
        .write_all(response)
        .and_then(|()| output.write_all(b"\n"))
        .and_then(|()| output.flush())
        .map_err(|error| format!(
            "Tower response received, output failed: {error}; no later request or retry was sent"
        ))?;
    budget.check()
}

struct Budget {
    started: Instant,
    wall: Duration,
    poll_ms: u64,
    calls: u64,
    written: u64,
    read: u64,
}

impl Budget {
    fn new(endpoint: &Endpoint<'_>, started: Instant) -> Self {
        Self {
            started,
            wall: Duration::from_millis(endpoint.runtime_ms),
            poll_ms: endpoint.poll_ms,
            calls: 0,
            written: 0,
            read: 0,
        }
    }

    fn check(&self) -> Result<(), String> {
        if self.started.elapsed() >= self.wall {
            return Err("Tower exchange deadline exceeded; retain original request identity; no reconnect or resend".into());
        }
        Ok(())
    }

    fn call(&mut self) -> Result<(), String> {
        self.check()?;
        if self.calls == MAX_CLIENT_IO_CALLS {
            return Err("Tower exchange I/O allowance exhausted; no reconnect or resend".into());
        }
        self.calls += 1;
        Ok(())
    }

    fn blocked(&self) -> Result<(), String> {
        self.check()?;
        pause(self.poll_ms);
        self.check()
    }

    fn write_frame(&mut self, socket: &mut impl Write, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() as u64 > MAX_CLIENT_IO_BYTES - self.written {
            return Err("Tower exchange write allowance exhausted; no reconnect or resend".into());
        }
        let mut sent = 0;
        while sent != bytes.len() {
            self.call()?;
            let end = sent.saturating_add(CLIENT_IO_CHUNK).min(bytes.len());
            match socket.write(&bytes[sent..end]) {
                Ok(0) => {
                    return Err(
                        "Tower request write stopped; request may have reached peer; no resend"
                            .into(),
                    );
                }
                Ok(count) if count <= end - sent => {
                    sent += count;
                    self.written += count as u64;
                }
                Ok(_) => return Err("Tower writer violated its byte bound; no resend".into()),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => self.blocked()?,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => {
                    return Err(format!(
                        "Tower request write failed: {error}; request may have reached peer; no resend"
                    ));
                }
            }
        }
        loop {
            self.call()?;
            match socket.flush() {
                Ok(()) => return self.check(),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => self.blocked()?,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => {
                    return Err(format!(
                        "Tower request flush failed: {error}; request may have reached peer; no resend"
                    ));
                }
            }
        }
    }

    fn read_frame(&mut self, socket: &mut impl Read) -> Result<Vec<u8>, String> {
        let mut response = Vec::new();
        let mut buffer = [0_u8; CLIENT_IO_CHUNK];
        loop {
            self.call()?;
            if self.read == MAX_CLIENT_IO_BYTES {
                return Err(
                    "Tower exchange read allowance exhausted; no reconnect or resend".into(),
                );
            }
            let allowed = usize::try_from(MAX_CLIENT_IO_BYTES - self.read)
                .unwrap_or(usize::MAX)
                .min(buffer.len());
            match socket.read(&mut buffer[..allowed]) {
                Ok(0) => {
                    return Err(
                        "Tower peer closed before a complete response; no reconnect or resend"
                            .into(),
                    );
                }
                Ok(count) if count <= allowed => {
                    self.read += count as u64;
                    self.check()?;
                    let bytes = &buffer[..count];
                    let newline = bytes.iter().position(|byte| *byte == b'\n');
                    let payload = newline.unwrap_or(count);
                    if response.len().saturating_add(payload) > MAX_REPLY_BYTES {
                        return Err(
                            "Tower response exceeds its byte bound; no reconnect or resend".into(),
                        );
                    }
                    response.extend_from_slice(&bytes[..payload]);
                    if let Some(index) = newline {
                        if index + 1 != count {
                            return Err("Tower peer supplied an unsolicited response suffix; no later request was sent".into());
                        }
                        return Ok(response);
                    }
                }
                Ok(_) => return Err("Tower reader violated its byte bound; no resend".into()),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => self.blocked()?,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => {
                    return Err(format!(
                        "Tower response read failed: {error}; no reconnect or resend"
                    ));
                }
            }
        }
    }
}

// Correlation and bounded syntax only. We deliberately do not reconstruct a
// public Knowledge, native ticket, signed profile or authority from this data.
fn validate_response(bytes: &[u8], request: &TowerRequest) -> Result<(), String> {
    parse_response(bytes, request).map(|_| ())
}

fn parse_response(bytes: &[u8], request: &TowerRequest) -> Result<Json, String> {
    let invalid =
        || "invalid or unbound Tower response; no later request or retry was sent".to_owned();
    let parsed = strict_json::parse(
        bytes,
        Limits {
            max_bytes: MAX_REPLY_BYTES,
            max_depth: 16,
            max_items: if request.verb == Verb::PolicyReplay {
                MAX_POLICY_REPLAY_REPLY_ITEMS
            } else {
                4096
            },
            max_string_bytes: MAX_REQUEST_STRING_BYTES,
        },
    )
    .map_err(|_| invalid())?;
    let root = parsed.as_object().ok_or_else(invalid)?;
    if root.len() != 2 || !root.contains_key("envelope") || !root.contains_key("result") {
        return Err(invalid());
    }
    let envelope = root
        .get("envelope")
        .and_then(Json::as_object)
        .ok_or_else(invalid)?;
    if envelope.len() != ENVELOPE_FIELDS.len()
        || ENVELOPE_FIELDS
            .iter()
            .any(|name| !envelope.contains_key(*name))
        || envelope.get("schema_version").and_then(Json::as_str) != Some(RESPONSE_SCHEMA)
        || envelope.get("operation_id").and_then(Json::as_str)
            != Some(request.operation_id.as_str())
        || envelope.get("address").and_then(Json::as_str)
            != Some(request.address.canonical_string().as_str())
        || !matches!(
            envelope.get("status").and_then(Json::as_str),
            Some("ok" | "error")
        )
    {
        return Err(invalid());
    }
    match (envelope.get("idempotency_key"), &request.idempotency_key) {
        (Some(Json::Null), None) => {}
        (Some(Json::String(actual)), Some(expected)) if actual == expected => {}
        _ => return Err(invalid()),
    }
    let result = root
        .get("result")
        .and_then(Json::as_object)
        .ok_or_else(invalid)?;
    if result.len() != 2 || !result.contains_key("value") {
        return Err(invalid());
    }
    match (result.get("kind").and_then(Json::as_str), request.verb) {
        (Some("refused"), _) => {}
        (Some("publication-request"), Verb::Propose | Verb::Get) => {}
        (Some("precheck"), Verb::Precheck) => {}
        (Some("capabilities"), Verb::Capabilities) => {}
        (Some("situation"), Verb::Situation) => {}
        (Some("explanation"), Verb::Explain) => {}
        (Some("policy-replay"), Verb::PolicyReplay) => {}
        _ => return Err(invalid()),
    }
    Ok(parsed)
}

#[cfg(test)]
#[path = "client/tests.rs"]
mod tests;
