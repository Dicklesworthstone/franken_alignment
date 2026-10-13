//! Privileged local recovery when the live supervisor/stop socket is unavailable.
//! Only original durable stop/fence/settlement runs; no source or helper is read.
pub(crate) mod guarded;
mod cooperative;
pub(crate) mod admission;

use super::config::{Config, debug};
use fa_reference::action::ElapsedTick;
#[cfg(test)]
use fa_reference::action::consequence::delivery::StopRequest;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::shutdown::recovery::FileStoppedRecovery;
use std::io::Write;

pub fn run<F>(config: &Config, operation: u64, clock: F, out: &mut impl Write) -> Result<(), String>
where F: FnOnce() -> ElapsedTick {
    let started = std::time::Instant::now();
    cooperative::run_with_elapsed(config, operation, clock, &mut || started.elapsed(), out)
}

fn ids(values: &[u64]) -> String {
    values.iter().map(|id| format!("\"{id}\"")).collect::<Vec<_>>().join(",")
}
pub(crate) fn report(recovered: &FileStoppedRecovery, out: &mut impl Write) -> Result<(), String> {
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
#[path = "recover_stop/tests.rs"]
mod tests;
