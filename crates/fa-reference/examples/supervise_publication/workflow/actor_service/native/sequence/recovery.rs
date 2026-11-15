//! Admit an explicitly selected remaining suffix on the original recovered
//! owner. These historical checks run after its recovery Fence but before any
//! live source, new numerical work, review listener or actor reference output.
use super::super::{ElapsedTick, Error, FileOversight, JournalError, continuation, debug, finish};
use super::{Entry, Release};

pub(super) fn preflight(
    host: &FileOversight,
    entries: &[Entry],
    after: u64,
    deadline: ElapsedTick,
) -> Result<(), String> {
    if after == 0 || entries.is_empty() {
        return Err(
            "native suffix recovery requires an explicit original Message predecessor".into(),
        );
    }
    for entry in entries {
        // Scan the WHOLE admitted suffix first. A collision in a later entry
        // cannot be discovered only after an earlier new effect has published.
        match host.request_status(entry.actor.request) {
            Ok(_) => {
                return Err(format!(
                    "native suffix request {} is already recorded; recovery requires new requests",
                    entry.actor.request
                ));
            }
            Err(JournalError::Contract(Error::Missing)) => {}
            Err(error) => return Err(debug(error)),
        }
        if matches!(entry.release, Release::Message) {
            match host.decoder_generation_progress(entry.inputs.generation) {
                Ok(_) => {
                    return Err(format!(
                        "native suffix generation {} already exists; a pending or completed intent cannot become a new Message",
                        entry.inputs.generation
                    ));
                }
                Err(JournalError::Contract(Error::Missing)) => {}
                Err(error) => return Err(debug(error)),
            }
        }
    }
    let stream = host.stream_snapshot().map_err(debug)?;
    let messages = entries
        .iter()
        .filter(|entry| matches!(entry.release, Release::Message))
        .count();
    if stream
        .confirmed
        .message_count()
        .checked_add(messages)
        .is_none_or(|count| count > stream.confirmed.profile().max_messages())
    {
        return Err("native suffix exceeds the original remaining stream message allowance".into());
    }
    let first = &entries[0];
    match first.release {
        Release::Message => {
            // The original builder requires the exact latest confirmed receipt,
            // full stream prefix and current numerical predecessor. It refuses
            // pending/unknown effects, later inference and unresolved Stop.
            continuation::prepare(host, first.actor.request, &first.inputs, after)?;
        }
        Release::Finish => {
            // A singleton Finish may close the already confirmed prefix, but
            // it must retain that actual Message's complete original recipe.
            // It does not create or advance another numerical generation.
            if finish::select(host, first.actor.request, &first.inputs, after, deadline)?.is_some()
            {
                return Err(
                    "native suffix Finish must be a new independently reviewed request".into(),
                );
            }
        }
    }
    Ok(())
}
