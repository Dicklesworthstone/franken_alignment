//! Privileged local recovery when the live supervisor/stop socket is unavailable.
//! Only original durable stop/fence/settlement runs; no source or helper is read.
use super::config::{Config, debug};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::StopRequest;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::shutdown::recovery::FileStoppedRecovery;
use std::io::Write;

pub fn run<F>(config: &Config, operation: u64, clock: F, out: &mut impl Write) -> Result<(), String>
where F: FnOnce() -> ElapsedTick {
    if operation == 0 { return Err("stop operation must be nonzero".into()); }
    // This is read-only selection, not recovery by opening a live owner. The
    // native call reacquires under its exclusive lock and checks this EXACT
    // predecessor. A racing policy/stop change refuses rather than being patched.
    let state = FileOversight::read_publication(&config.store, &config.profile).map_err(debug)?;
    let request = match state.stop {
        Some(receipt) => {
            if receipt.request().operation != operation {
                return Err("the retained stop belongs to a different operation".into());
            }
            receipt.request()
        }
        None => StopRequest { operation, expected_control_sequence: state.control.sequence,
            expected_authority_epoch: state.control.ledger.epoch },
    };
    let recovered = FileOversight::recover_stopped(&config.store, config.profile.clone(), request, clock)
        .map_err(|error| format!("native recovery completion is unconfirmed: {error:?}; no effect was resent"))?;
    report(&recovered, out)
}

fn ids(values: &[u64]) -> String {
    values.iter().map(|id| format!("\"{id}\"")).collect::<Vec<_>>().join(",")
}
fn report(recovered: &FileStoppedRecovery, out: &mut impl Write) -> Result<(), String> {
    let progress = recovered.progress(); let state = recovered.snapshot();
    let kind = match recovered { FileStoppedRecovery::AlreadyDrained { .. } => "already_drained",
        FileStoppedRecovery::Advanced { .. } => "advanced" };
    let drained = progress.drained();
    let status = if drained { "stopped_drained" } else { "stopped_pending" };
    // Only fixed labels, booleans and bounded native numeric data are rendered.
    // No payload, helper view or fallible user-controlled text enters the report.
    let document = format!(concat!(
        "{{\"status\":\"{}\",\"recovery\":\"{}\",\"operation\":\"{}\",",
        "\"stop_acknowledged\":true,\"drained\":{},\"endpoint_fenced\":{},",
        "\"revision\":\"{}\",\"control_sequence\":\"{}\",\"authority_epoch\":\"{}\",",
        "\"revocation_floor\":\"{}\",\"dispatcher_epoch\":\"{}\",",
        "\"executions\":\"{}\",\"reserved\":\"{}\",\"charged\":\"{}\",",
        "\"unresolved\":[{}],\"irrecoverable\":[{}]}}\n"),
        status, kind, progress.receipt.request().operation, drained, progress.endpoint_fenced,
        state.revision, state.control.sequence, state.control.ledger.epoch,
        progress.receipt.revocation_floor(), progress.dispatcher_epoch, state.executions,
        progress.reserved_units, progress.charged_units, ids(&progress.unresolved), ids(&progress.irrecoverable));
    out.write_all(document.as_bytes()).and_then(|_| out.flush()).map_err(|error| {
        format!("native stop acknowledged at revision {} (drained={drained}), but output failed: {error:?}; do not resend an effect", state.revision)
    })?;
    if !drained {
        return Err(format!("native stop acknowledged, but {} unresolved obligations remain ({} irrecoverable); charges are retained",
            progress.unresolved.len(), progress.irrecoverable.len()));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
