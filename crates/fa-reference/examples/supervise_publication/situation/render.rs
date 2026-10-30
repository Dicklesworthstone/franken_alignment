//! A bounded rendering of native historical Knowledge; no policy/outcome reducer.
use fa_reference::action::{ActionState, Purpose};
use fa_reference::action::consequence::delivery::persistent::observed::replay::situation::{
    FileSituationBasis, FileSituationOrigin, FileSituationPage, MAX_SITUATION_PAGE_ATTEMPTS,
};

pub(super) const MAX_PAGE_BYTES: usize = 65_536;

pub(super) fn page(page: &FileSituationPage, basis: FileSituationBasis) -> Result<Vec<u8>, String> {
    if page.attempts.len() > MAX_SITUATION_PAGE_ATTEMPTS { return Err("situation page exceeds native entry bound".into()); }
    let summary = page.summary;
    let scope = basis.scope;
    let origin = match basis.origin {
        FileSituationOrigin::AcknowledgedOwner => "acknowledged_owner",
        FileSituationOrigin::CanonicalImage => "canonical_image",
    };
    let purpose = match scope.purpose { Purpose::Effect => "effect", Purpose::Experiment => "experiment" };
    let optional = |value: Option<u64>| value.map_or_else(|| "null".to_owned(), |value| format!("\"{value}\""));
    let mut rows = Vec::with_capacity(page.attempts.len());
    for row in &page.attempts {
        let target = row.target;
        rows.push(format!(concat!(
            "{{\"address\":\"{}\",\"attempt\":\"{}\",\"stage\":\"{}\",\"target\":{{",
            "\"adapter\":\"{}\",\"object\":\"{}\",\"contract_version\":\"{}\",",
            "\"expected_version\":\"{}\",\"generation\":\"{}\"}},",
            "\"units\":\"{}\",\"payload_bytes\":\"{}\",\"proposal_policy_epoch\":\"{}\",\"deadline\":\"{}\"}}"),
            row.address, row.attempt, stage(row.stage), target.adapter, target.object, target.contract_version,
            target.expected_version, target.generation, row.units, row.payload_bytes,
            row.proposal_policy_epoch, row.deadline.0));
    }
    // Every dynamic string above/below is a decimal integer or compiled enum.
    // No attacker bytes, paths, payloads, helper material or Debug text enter JSON.
    let bytes = format!(concat!(
        "{{\"schema\":\"fa.publication-situation/1\",\"operation\":\"situation\",",
        "\"historical\":true,\"current_eligibility\":\"not_observed\",",
        "\"knowledge\":{{\"state\":\"known\",\"basis\":{{",
        "\"scope\":{{\"tenant\":\"{}\",\"principal\":\"{}\",\"run\":\"{}\",\"branch\":\"{}\",\"authority\":\"{}\",\"purpose\":\"{}\"}},",
        "\"journal_revision\":\"{}\",\"control_sequence\":\"{}\",\"recorded_at\":{},",
        "\"origin\":\"{}\",\"anchored_through\":{}}},\"value\":{{\"summary\":{{",
        "\"total_units\":\"{}\",\"available_units\":\"{}\",\"reserved_units\":\"{}\",\"charged_units\":\"{}\",",
        "\"policy_epoch\":\"{}\",\"dispatcher_epoch\":\"{}\",\"suspended\":{},\"stop_recorded\":{},\"executions\":\"{}\",",
        "\"attempts\":\"{}\",\"retained_requests\":\"{}\",\"pending_attempts\":\"{}\",\"unresolved_attempts\":\"{}\",",
        "\"irrecoverably_unknown_attempts\":\"{}\",\"terminal_attempts\":\"{}\"}},",
        "\"offset\":\"{}\",\"enumeration_complete\":{},\"attempts\":[{}]}}}}}}\n"),
        scope.tenant, scope.principal, scope.run, scope.branch, scope.authority, purpose,
        basis.journal_revision, basis.control_sequence, optional(basis.recorded_at.map(|tick| tick.0)),
        origin, optional(basis.anchored_through),
        summary.total_units, summary.available_units, summary.reserved_units, summary.charged_units,
        summary.policy_epoch, summary.dispatcher_epoch, summary.suspended, summary.stop_recorded, summary.executions,
        summary.attempts, summary.retained_requests, summary.pending_attempts, summary.unresolved_attempts,
        summary.irrecoverably_unknown_attempts, summary.terminal_attempts,
        page.offset, page.next.is_none(), rows.join(",")).into_bytes();
    if bytes.len() > MAX_PAGE_BYTES { return Err("situation encoded page exceeds byte bound".into()); }
    Ok(bytes)
}

fn stage(value: ActionState) -> &'static str {
    match value {
        ActionState::Proposed => "proposed", ActionState::Prepared => "prepared",
        ActionState::Reviewing => "reviewing", ActionState::Authorized => "authorized",
        ActionState::Dispatching => "dispatching", ActionState::Confirmed => "confirmed",
        ActionState::Denied => "denied", ActionState::Cancelled => "cancelled",
        ActionState::Unknown => "unknown", ActionState::ConfirmedNotExecuted => "confirmed_not_executed",
        ActionState::IrrecoverablyUnknown => "irrecoverably_unknown",
    }
}
