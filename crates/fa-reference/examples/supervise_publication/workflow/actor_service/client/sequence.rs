//! Bounded actor-side composition over the ORIGINAL authenticated connection.
//! Later documents never repair earlier uncertainty or renew transport work.
use super::{ClientIoBudget, ClientIoLimits, Command, Duration, Instant, Knowledge,
    MAX_FRAME_BYTES, MAX_SEQUENCE_DOCUMENTS, Path, Profile, Write,
    connect, debug, decode_command, exchange_on_connection, read_regular, write_response};
use fa_reference::action::consequence::oversight::actor::ActorOutcome;
use std::collections::BTreeSet;

// Includes original JSON framing/whitespace, not merely decoded payloads.
const MAX_SEQUENCE_INPUT_BYTES: usize = 2 * 1024 * 1024;

/// Fully preflight the selected original documents before connecting or sending
/// the first request. This grants nothing: the independently configured service
/// still checks its schedule, current target, epoch, budget and both approvals.
fn read_documents(profile: &Profile, documents: &[&Path]) -> Result<Vec<Command>, String> {
    if !(2..=MAX_SEQUENCE_DOCUMENTS).contains(&documents.len()) {
        return Err("actor sequence requires 2..=64 original Submit documents".into());
    }
    let mut commands = Vec::new();
    commands.try_reserve_exact(documents.len()).map_err(debug)?;
    let mut keys = BTreeSet::new();
    let mut bytes_read = 0_usize;
    for path in documents {
        let remaining = MAX_SEQUENCE_INPUT_BYTES.checked_sub(bytes_read)
            .ok_or("actor sequence input allowance exhausted")?;
        if remaining == 0 { return Err("actor sequence input allowance exhausted".into()); }
        let bytes = read_regular(path, MAX_FRAME_BYTES.min(remaining))?;
        bytes_read = bytes_read.checked_add(bytes.len()).ok_or("actor input size overflow")?;
        let command = decode_command(&bytes).map_err(debug)?;
        let Command::Submit { request, .. } = &command else {
            return Err("every actor sequence document must be an original Submit".into());
        };
        if (commands.is_empty() && *request != profile.request) || !keys.insert(*request) {
            return Err("actor sequence must begin with the profile request and have distinct keys".into());
        }
        commands.push(command);
    }
    Ok(commands)
}

pub(super) fn submit(profile: &Profile, documents: &[&Path], out: &mut impl Write)
    -> Result<(), String>
{
    let started = Instant::now();
    let commands = read_documents(profile, documents)?;
    let mut budget = ClientIoBudget::new(ClientIoLimits {
        exchanges: profile.exchanges, ..ClientIoLimits::default()
    }).map_err(debug)?;
    if started.elapsed() >= Duration::from_millis(profile.runtime_ms) {
        return Err("actor sequence preflight exceeded its lifetime; nothing sent".into());
    }
    let mut socket = Some(connect(profile)?);
    for command in commands {
        let request = command.request();
        let (reusable, response) = exchange_on_connection(
            socket.take().ok_or("completed actor sequence lost its connection; no reconnect")?,
            profile, command, &mut budget, started,
        )?;
        // Output failure may follow real execution. Never send later work before
        // the caller receives and flushes this ORIGINAL response successfully.
        write_response(&response, out)?;
        if !matches!(&response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })) {
            return Err(format!("actor sequence stopped at request {request}: execution not confirmed; later documents unsent"));
        }
        socket = reusable;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
