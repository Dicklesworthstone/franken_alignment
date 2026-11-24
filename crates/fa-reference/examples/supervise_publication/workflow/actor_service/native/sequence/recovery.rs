//! Admit an explicitly selected remaining suffix on the original recovered
//! owner. These historical checks run after its recovery Fence but before any
//! live source, new numerical work, review listener or actor reference output.
use super::super::{
    ElapsedTick, Error, FileOversight, JournalError, continuation, debug, finish, recovery,
};
use super::{Entry, Mode, Release};

pub(super) fn preflight(
    host: &FileOversight,
    entries: &[Entry],
    mode: Mode,
    deadline: ElapsedTick,
) -> Result<(), String> {
    if !mode.is_recovery() || mode.after() == Some(0) || entries.is_empty() {
        return Err(
            "native suffix recovery requires an explicit original generation or Message predecessor".into(),
        );
    }
    for (index, entry) in entries.iter().enumerate() {
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
        if matches!(entry.release, Release::Message)
            && !(index == 0 && mode == Mode::ContinueGeneration)
        {
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
    match (mode, first.release) {
        (Mode::ContinueGeneration, Release::Message) => {
            // Only this first recipe may name an existing intent. The SAME
            // single-request selector binds its raw command, current pending
            // generation or completed unsubmitted result, and numerical state.
            // No command is reconstructed from the current decoder position.
            if recovery::select(host, first.actor.request, &first.inputs)?.is_some() {
                return Err(
                    "native sequence continuation requires an unsubmitted original text intent"
                        .into(),
                );
            }
        }
        (Mode::AfterMessage(after), Release::Message) => {
            // The original builder requires the exact latest confirmed receipt,
            // full stream prefix and current numerical predecessor. It refuses
            // pending/unknown effects, later inference and unresolved Stop.
            continuation::prepare(host, first.actor.request, &first.inputs, after)?;
        }
        (Mode::AfterMessage(after), Release::Finish) => {
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
        (Mode::ContinueGeneration, Release::Finish) | (Mode::Create, _) => {
            return Err(
                "native sequence continuation must begin with its original Message generation"
                    .into(),
            );
        }
    }
    Ok(())
}
