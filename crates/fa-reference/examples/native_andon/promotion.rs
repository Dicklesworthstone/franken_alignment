//! Original deterministic promotion only; never caller-supplied weights or truth.
use super::{FileOversight, FileCredibilityUpdate, ElapsedTick, debug};
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::Error;
use std::io::Write;

pub(super) fn apply(host: &mut FileOversight, request: &FileCredibilityUpdate,
    checkpoint: &mut impl FnMut() -> Result<(), String>,
    clock: &mut impl FnMut() -> Result<ElapsedTick, String>) -> Result<bool, String>
{
    if host.joint_credibility_policy().map_err(debug)?.is_none() {
        return Err("joint promotion policy is required".into());
    }
    match host.credibility_promotion(request.operation) {
        Ok(_) => {
            // The ORIGINAL operation identity check decides equality. Do not
            // substitute today's control counters for the retained request.
            host.promote_credibility(host.revision(), request).map_err(debug)?;
            return Ok(false);
        }
        Err(JournalError::Contract(Error::Missing)) => {}
        Err(error) => return Err(debug(error)),
    }
    let control = host.inspect().control;
    if control.sequence != request.expected_control_sequence
        || control.ledger.epoch != request.expected_authority_epoch {
        return Err("promotion does not bind the recovered control predecessor".into());
    }
    if control.suspended || host.inspect().stop.is_some() {
        return Err("stopped or suspended native owner cannot promote".into());
    }
    let preview = host.joint_credibility_report(request.expected_evaluation_revision).map_err(debug)?;
    if !preview.qualified() {
        return Err(format!("native joint replay refused promotion: {:?}", preview.failures));
    }
    checkpoint()?;
    let tick = clock()?;
    checkpoint()?;
    host.observe_time(host.revision(), tick).map_err(debug)?;
    checkpoint()?;
    // The native operation rechecks the candidate and ALL retained evidence.
    // Time and promotion remain distinct original durable transactions.
    host.promote_credibility(host.revision(), request).map_err(debug)?;
    Ok(true)
}

// Return bounded numerical metadata from the actual acknowledged reports only.
// No helper identifier, input, evidence bytes, payload or arbitrary text is emitted.
pub(super) fn report(host: &FileOversight, operation: u64, out: &mut impl Write) -> Result<(), String> {
    let promotion = host.credibility_promotion(operation).map_err(debug)?;
    let joint = host.joint_credibility_promotion(operation).map_err(debug)?;
    write!(out, ",\"promotion\":{{\"operation\":\"{operation}\",\"previous_congress_generation\":\"{}\",\"current_congress_generation\":\"{}\",\"evaluation_revision\":\"{}\",\"joint_qualified\":{}}}",
        promotion.change.previous.generation, promotion.change.current.generation,
        joint.evaluation_revision, joint.qualified()).map_err(debug)
}

#[cfg(test)]
#[path = "promotion_tests.rs"]
mod tests;
