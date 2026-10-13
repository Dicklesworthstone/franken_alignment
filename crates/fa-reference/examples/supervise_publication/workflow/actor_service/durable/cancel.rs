//! Cancellation of an acknowledged durable request, never creation-to-cancel.
//! The original server still owns cancellation, dispatch and terminal evidence.
use super::{FilePeerConnection, Profile, Settings, Duration, WireResponse, Knowledge,
    UnknownReason, check_time, receive, debug};
use fa_reference::action::consequence::oversight::actor::ActorOutcome;

const UNACKNOWLEDGED: &str = "cancellation requires a retained server acknowledgment; an unanswered request needs independent supervisor resolution, not a new Submit";

// These projections are emitted only after the original durable request exists.
// ControllerUnavailable, Withheld, Stale, Absent and errors do not establish it.
// This relies on the selected server domain retaining its original request book;
// it is not protection against server rollback, hostile replies or file tampering.
pub(super) fn check_record(retained: bool, response: Option<&WireResponse>, request: u64) -> Result<(), String> {
    if retained && response.is_some_and(|response| response.request == Some(request)
        && matches!(&response.result, Ok(Knowledge::Known { .. } | Knowledge::Pending { .. }
            | Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown }))) {
        Ok(())
    } else { Err(UNACKNOWLEDGED.into()) }
}

pub(super) fn exchange(client: &mut FilePeerConnection, actor: &Profile, settings: &Settings,
    elapsed: &mut impl FnMut() -> Duration, wait: &mut impl FnMut(u64)) -> Result<WireResponse, String>
{
    check_record(client.state().map_err(debug)?.original_proposal(actor.request).is_some(),
        client.state().map_err(debug)?.last_response(actor.request), actor.request)?;
    check_time(actor, elapsed)?;
    // Recover visibility of the ALREADY acknowledged original request. The
    // immutable durable request book makes this a retry, not a new admission.
    client.retry_submission(client.revision(), actor.request).map_err(debug)?;
    let response = receive(client, actor, settings, elapsed, wait)?;
    match &response.result {
        Ok(Knowledge::Pending { .. } | Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown }) => {
            check_time(actor, elapsed)?;
            client.cancel(client.revision(), actor.request).map_err(debug)?;
            // One explicit cancellation, not a polling loop or automatic retry.
            // Unknown after dispatch returns immediately as Unknown, not success.
            receive(client, actor, settings, elapsed, wait)
        }
        _ => Ok(response), // terminal or unavailable: send no Cancel
    }
}

pub(super) fn confirmed(response: &WireResponse) -> Result<(), String> {
    match &response.result {
        Ok(Knowledge::Known { value: ActorOutcome::NotAdmitted | ActorOutcome::Denied
            | ActorOutcome::CancelledBeforeDispatch | ActorOutcome::ConfirmedNotExecuted, .. }) => Ok(()),
        Ok(Knowledge::Known { value: ActorOutcome::Executed, .. }) =>
            Err("the original request executed; cancellation cannot undo it".into()),
        _ => Err("cancellation did not establish nonexecution; retain the original request and use supervisor reconciliation".into()),
    }
}
