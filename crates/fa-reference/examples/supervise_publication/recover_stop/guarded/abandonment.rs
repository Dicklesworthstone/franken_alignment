//! Explicit offline unknown-liability disposition with independent history custody.
//! This command never selects a weaker recovery path or returns a live owner.
use super::profile::AbandonProfile;
use super::{Config, FileGuardedRecoveryStatus, debug, prepare_recovery};
use crate::config::read_regular;
use fa_reference::action::ActionState;
use fa_reference::action::consequence::delivery::persistent::{
    abandonment::FileAbandonRequest, observed::guarded::recovery::FileAbandonedHistory,
};
use fa_reference::strict_json::{self, Json, Limits};
use std::{
    collections::BTreeMap,
    io::Write,
    path::Path,
    time::{Duration, Instant},
};

const REQUEST_BYTES: usize = 4096;
const USAGE: &str =
    "usage: supervise_publication abandon-unknown CONFIG REQUEST_JSON --guarded RECOVERY_PROFILE";

pub(crate) fn command(args: &[String], out: &mut impl Write) -> Result<(), String> {
    let started = Instant::now();
    if args.len() != 5 || args[0] != "abandon-unknown" || args[3] != "--guarded" {
        return Err(USAGE.into());
    }
    // Every input is parsed before authority custody is opened. The one elapsed
    // budget starts before these reads and is checked against CONFIG afterward.
    let config = Config::read(Path::new(&args[1]))?;
    let request = decode_request(&read_regular(Path::new(&args[2]), REQUEST_BYTES)?)?;
    let profile = AbandonProfile::read(Path::new(&args[4]))?;
    run_with_elapsed(&config, &profile, request, &mut || started.elapsed(), out)
}

fn decode_request(bytes: &[u8]) -> Result<FileAbandonRequest, String> {
    let json = strict_json::parse(
        bytes,
        Limits {
            max_bytes: REQUEST_BYTES,
            max_depth: 2,
            max_items: 16,
            max_string_bytes: 128,
        },
    )
    .map_err(debug)?;
    let Json::Object(mut fields) = json else {
        return Err("abandonment request requires an object".into());
    };
    if take(&mut fields, "schema_version")?.as_str() != Some("fa.abandon-unknown/1") {
        return Err("unsupported abandonment request".into());
    }
    let request = FileAbandonRequest {
        operation: number(&mut fields, "operation")?,
        attempt: number(&mut fields, "attempt")?,
        expected_control_sequence: number(&mut fields, "expected_control_sequence")?,
        expected_authority_epoch: number(&mut fields, "expected_authority_epoch")?,
    };
    if !fields.is_empty() {
        return Err("unknown abandonment request fields".into());
    }
    if request.operation == 0 || request.attempt == 0 {
        return Err("abandonment operation and attempt must be nonzero".into());
    }
    Ok(request)
}

fn take(fields: &mut BTreeMap<String, Json>, name: &str) -> Result<Json, String> {
    fields
        .remove(name)
        .ok_or_else(|| format!("abandonment request missing {name}"))
}
fn number(fields: &mut BTreeMap<String, Json>, name: &str) -> Result<u64, String> {
    take(fields, name)?
        .as_u64()
        .ok_or_else(|| format!("abandonment {name} must be an unsigned integer"))
}

// Deterministic deadline seam; production supplies one monotonic Instant.
fn run_with_elapsed(
    config: &Config,
    profile: &AbandonProfile,
    request: FileAbandonRequest,
    elapsed: &mut impl FnMut() -> Duration,
    out: &mut impl Write,
) -> Result<(), String> {
    let wall = Duration::from_millis(config.timing.runtime_ms);
    let mut checkpoint = || {
        if elapsed() >= wall {
            Err("guarded abandonment deadline reached before acknowledgment".to_owned())
        } else {
            Ok(())
        }
    };
    let (mut recovery, custody) = prepare_recovery(config, profile.profile(), &mut checkpoint)?;
    loop {
        checkpoint()?;
        let current = recovery.progress();
        if current.status == FileGuardedRecoveryStatus::Ready {
            break;
        }
        recovery
            .advance(current.replayed_events, 1)
            .map_err(unconfirmed)?;
    }
    // No Fence, new clock or source observation can change the independently
    // selected predecessor between this check and the consuming finalizer.
    let history = recovery
        .finish_abandoned_with_anchor(request)
        .map_err(unconfirmed)?;
    let mut late = checkpoint().is_err();
    // Once acknowledged, always attempt independent retention even after the
    // deadline. Export/report failures must not hide or undo the native change.
    let retained = custody.retain(history.history_anchor());
    late |= checkpoint().is_err();
    let reported = report(&history, retained.is_ok(), out);
    late |= checkpoint().is_err();
    let mut failures = Vec::new();
    if late {
        failures.push("deadline reached after native acknowledgment".to_owned());
    }
    if let Err(error) = retained {
        failures.push(format!(
            "history anchor retention failed: {error}; select a new output file"
        ));
    }
    if let Err(error) = reported {
        failures.push(format!("report output failed: {error}"));
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "native abandonment acknowledged at revision {}: {}; the liability remains charged; retry only the exact original request",
            history.snapshot().revision,
            failures.join("; ")
        ))
    }
}

fn unconfirmed(
    error: fa_reference::action::consequence::delivery::persistent::JournalError,
) -> String {
    format!(
        "native abandonment is unconfirmed: {}; no effect was resent; inspect the canonical history before choosing a new request",
        debug(error)
    )
}

fn report(
    history: &FileAbandonedHistory,
    retained: bool,
    out: &mut impl Write,
) -> Result<(), String> {
    let receipt = history.receipt();
    let request = receipt.request();
    let state = history.snapshot();
    let unresolved = state
        .control
        .ledger
        .stages
        .iter()
        .filter_map(|(id, stage)| {
            matches!(
                stage,
                ActionState::Dispatching | ActionState::Unknown | ActionState::IrrecoverablyUnknown
            )
            .then_some(*id)
        })
        .collect::<Vec<_>>();
    let irrecoverable = state
        .control
        .ledger
        .stages
        .iter()
        .filter_map(|(id, stage)| (*stage == ActionState::IrrecoverablyUnknown).then_some(*id))
        .collect::<Vec<_>>();
    let ids = |values: &[u64]| {
        values
            .iter()
            .map(|id| format!("\"{id}\""))
            .collect::<Vec<_>>()
            .join(",")
    };
    // Fixed labels and bounded numeric data only. The private action payload,
    // source bytes, helper views and independent anchor never enter stdout.
    let text = format!(
        concat!(
            "{{\"schema\":\"fa.abandon-unknown-result/1\",\"knowledge\":{{\"kind\":\"known\",",
            "\"basis\":{{\"class\":\"bounded_model\",\"scope\":\"acknowledged_reference_disposition\",",
            "\"authentic_origin\":\"unknown\",\"source_truth\":\"not_established\",",
            "\"verified_journal_revision\":\"{}\"}},\"value\":{{",
            "\"status\":\"irrecoverably_unknown\",\"disposition_recorded\":true,",
            "\"effect_outcome\":{{\"kind\":\"unknown\",\"reason\":\"OutcomeUnknown\"}},",
            "\"operation\":\"{}\",\"attempt\":\"{}\",",
            "\"revision\":\"{}\",\"control_sequence\":\"{}\",\"authority_epoch\":\"{}\",",
            "\"charged\":\"{}\",\"abandoned_units\":\"{}\",\"executions\":\"{}\",",
            "\"history_anchor_retained\":{},\"unresolved\":[{}],\"irrecoverable\":[{}]}}}}}}\n"
        ),
        state.revision,
        request.operation,
        request.attempt,
        state.revision,
        state.control.sequence,
        state.control.ledger.epoch,
        state.control.ledger.charged,
        receipt.charged_units(),
        state.executions,
        retained,
        ids(&unresolved),
        ids(&irrecoverable)
    );
    out.write_all(text.as_bytes())
        .and_then(|_| out.flush())
        .map_err(debug)
}

#[cfg(test)]
pub(in crate::recover_stop) mod tests;
