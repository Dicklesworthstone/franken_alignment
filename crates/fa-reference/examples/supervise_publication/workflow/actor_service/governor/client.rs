//! One explicit independently authenticated governance exchange. The client
//! validates correlation and complete report binding before emitting a reply.
//! It neither approves a returned report nor follows a promotion action.
use super::{Settings, protocol};
use crate::config::{debug, policy_from_json, read_regular};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::FileCampaignRequirement;
use fa_reference::action::consequence::delivery::persistent::requests::actor::MAX_POLICY_PROMOTION_REPLAY_EVENTS;
use fa_reference::action::consequence::oversight::actor_wire::MAX_CHANNEL_EXCHANGES;
use fa_reference::action::consequence::oversight::policy_governance::MAX_POLICY_CAMPAIGNS;
use fa_reference::action::consequence::policy_campaign::{
    MAX_REPLAY_CASES, MAX_REPLAY_INPUT_BYTES, ReplayCaseId, ReplayLimits,
};
use fa_reference::action::{Scope, VERSION};
use fa_reference::strict_json::{self, Json, Limits};
use std::collections::BTreeSet;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

#[cfg(test)]
#[path = "client/tests.rs"]
mod tests;

const USAGE: &str = "policy-governor-exchange GOVERNOR_PROFILE REQUEST_JSON (one explicit inspection, review or approval; no automatic retry)";
const IO_CALLS: usize = 65_536;
const IO_CHUNK: usize = 16_384;

pub(crate) fn command(args: &[String], output: &mut impl Write) -> Result<(), String> {
    if args.len() != 3
        || args[0] != "policy-governor-exchange"
        || args[1..]
            .iter()
            .any(|arg| arg.is_empty() || arg.starts_with("--"))
    {
        return Err(USAGE.into());
    }
    let started = Instant::now();
    let settings = Settings::read(Path::new(&args[1]))?;
    let request = protocol::decode(
        &read_regular(Path::new(&args[2]), protocol::MAX_REQUEST_BYTES)?,
        settings.report_bytes,
    )?;
    if request.scope != settings.scope {
        return Err("governor request audience differs from the explicit connection profile; no request sent".into());
    }
    let mut budget = Budget {
        started,
        wall: Duration::from_millis(settings.runtime_ms),
        poll: Duration::from_millis(settings.poll_ms),
        calls: 0,
    };
    budget.check()?;
    settings.check_directory()?;
    let mut socket = UnixStream::connect(&settings.socket)
        .map_err(|error| format!("governor connection failed: {error}; no request sent"))?;
    settings
        .supervisor
        .verify(&socket)
        .map_err(|_| "governor server credentials rejected; no request sent".to_owned())?;
    socket.set_nonblocking(true).map_err(debug)?;
    let length = u32::try_from(request.canonical.len()).map_err(debug)?;
    budget.write(&mut socket, &length.to_be_bytes())?;
    budget.write(&mut socket, &request.canonical)?;
    let mut header = [0; 4];
    budget.read(&mut socket, &mut header)?;
    let length = usize::try_from(u32::from_be_bytes(header)).map_err(debug)?;
    if length == 0 || length > protocol::MAX_RESPONSE_BYTES {
        return Err(
            "governor response exceeds its bounded frame; request may have completed; no retry"
                .into(),
        );
    }
    let mut response = vec![0; length];
    budget.read(&mut socket, &mut response)?;
    validate_selected_response(&request, &response, &settings)?;
    budget.check()?;
    output
        .write_all(&response)
        .and_then(|()| output.write_all(b"\n"))
        .and_then(|()| output.flush())
        .map_err(|error| {
            format!("governor reply validated but output failed: {error}; no retry")
        })?;
    budget.check()
}

struct Budget {
    started: Instant,
    wall: Duration,
    poll: Duration,
    calls: usize,
}
impl Budget {
    fn check(&self) -> Result<(), String> {
        if self.started.elapsed() >= self.wall {
            return Err(
                "governor exchange deadline reached; request may have completed; no retry".into(),
            );
        }
        Ok(())
    }
    fn call(&mut self) -> Result<(), String> {
        self.check()?;
        if self.calls == IO_CALLS {
            return Err(
                "governor exchange I/O allowance exhausted; request may have completed; no retry"
                    .into(),
            );
        }
        self.calls += 1;
        Ok(())
    }
    fn blocked(&self) -> Result<(), String> {
        self.check()?;
        std::thread::sleep(
            self.poll
                .min(self.wall.saturating_sub(self.started.elapsed())),
        );
        self.check()
    }
    fn write(&mut self, socket: &mut impl Write, bytes: &[u8]) -> Result<(), String> {
        let mut sent = 0;
        while sent < bytes.len() {
            self.call()?;
            let end = (sent + IO_CHUNK).min(bytes.len());
            match socket.write(&bytes[sent..end]) {
                Ok(0) => {
                    return Err(
                        "governor request write stopped; request may have completed; no retry"
                            .into(),
                    );
                }
                Ok(count) if count <= end - sent => sent += count,
                Ok(_) => return Err("governor writer exceeded its byte bound; no retry".into()),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => self.blocked()?,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => {
                    return Err(format!(
                        "governor request write failed: {error}; request may have completed; no retry"
                    ));
                }
            }
        }
        self.check()
    }
    fn read(&mut self, socket: &mut impl Read, bytes: &mut [u8]) -> Result<(), String> {
        let mut received = 0;
        while received < bytes.len() {
            self.call()?;
            let end = (received + IO_CHUNK).min(bytes.len());
            match socket.read(&mut bytes[received..end]) {
                Ok(0) => return Err("governor peer closed before a complete reply; request may have completed; no retry".into()),
                Ok(count) if count <= end - received => received += count,
                Ok(_) => return Err("governor reader exceeded its byte bound; no retry".into()),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => self.blocked()?,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(format!("governor reply read failed: {error}; request may have completed; no retry")),
            }
        }
        self.check()
    }
}

fn parse(bytes: &[u8], maximum: usize, strings: usize, items: usize) -> Result<Json, String> {
    strict_json::parse(
        bytes,
        Limits {
            max_bytes: maximum,
            max_depth: 24,
            max_items: items,
            max_string_bytes: strings,
        },
    )
    .map_err(|error| format!("invalid bounded governor reply: {error:?}; no retry"))
}

fn object<'a>(
    value: &'a Json,
    names: &[&str],
) -> Result<&'a std::collections::BTreeMap<String, Json>, String> {
    let fields = value
        .as_object()
        .ok_or("governor reply requires an object")?;
    if fields.len() != names.len() || names.iter().any(|name| !fields.contains_key(*name)) {
        return Err("governor reply has missing or unknown fields".into());
    }
    Ok(fields)
}

fn decimal(value: &Json) -> Result<u64, String> {
    let text = value
        .as_str()
        .ok_or("governor counter must be a decimal string")?;
    if text.is_empty()
        || text.len() > 20
        || (text.len() > 1 && text.starts_with('0'))
        || !text.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err("governor counter is not canonical".into());
    }
    text.parse().map_err(debug)
}

enum ValidatedResponse {
    Inspection(protocol::Inspection),
    Review,
    Approval,
    Refusal,
}

fn validate_selected_response(
    request: &protocol::Request,
    bytes: &[u8],
    settings: &Settings,
) -> Result<(), String> {
    if let ValidatedResponse::Inspection(inspection) =
        validate_response(request, bytes, settings.report_bytes)?
        && (inspection.campaigns != settings.campaign_requirement()
            || inspection.campaign_limit != settings.campaign_limit
            || inspection.report_bytes != settings.report_bytes
            || inspection
                .numerical_replay
                .map(|numerical| numerical.selected)
                != settings.numerical_replay)
    {
        return Err(
            "governor inspection changed the independently selected campaign bounds".into(),
        );
    }
    Ok(())
}

fn validate_response(
    request: &protocol::Request,
    bytes: &[u8],
    report_bytes: usize,
) -> Result<ValidatedResponse, String> {
    let reply = parse(
        bytes,
        protocol::MAX_RESPONSE_BYTES,
        report_bytes.max(128),
        128,
    )?;
    let result_field = match (reply.get("knowledge"), reply.get("error")) {
        (Some(_), None) => "knowledge",
        (None, Some(_)) => "error",
        _ => return Err("governor reply requires exactly one result".into()),
    };
    let root = object(
        &reply,
        &[
            "schema",
            "scope",
            "verb",
            "phase",
            request.correlation().field(),
            result_field,
        ],
    )?;
    let scope = parse(protocol::scope_json(request.scope).as_bytes(), 512, 128, 32)?;
    if root["schema"].as_str() != Some("fa.policy-governor-response/1")
        || root["scope"] != scope
        || root["verb"].as_str() != Some(request.verb())
        || root["phase"].as_str() != Some(request.phase())
        || decimal(&root[request.correlation().field()])? != request.correlation().id()
    {
        return Err(
            "governor reply does not match the original audience, identity and command".into(),
        );
    }
    if result_field == "error" {
        let fields = object(&root["error"], &["code"])?;
        if !fields["code"]
            .as_str()
            .is_some_and(protocol::is_refusal_reason)
        {
            return Err("unregistered governor refusal reason".into());
        }
        return Ok(ValidatedResponse::Refusal);
    }
    let fields = object(&root["knowledge"], &["state", "value", "basis"])?;
    if fields["state"].as_str() != Some("known") {
        return Err("unsupported governor knowledge state".into());
    }
    let basis = object(
        &fields["basis"],
        &[
            "origin",
            "scope",
            request.correlation().field(),
            "journal_revision",
        ],
    )?;
    if basis["origin"].as_str() != Some("acknowledged_owner")
        || basis["scope"] != scope
        || decimal(&basis[request.correlation().field()])? != request.correlation().id()
    {
        return Err("governor acknowledgment changed its original owner evidence basis".into());
    }
    let basis_revision = decimal(&basis["journal_revision"])?;
    match &request.command {
        protocol::Command::Inspect { .. } => {
            let mut expected_fields = vec![
                "journal_revision",
                "control_sequence",
                "authority_epoch",
                "policy_generation",
                "campaigns",
                "campaign_limit",
                "report_bytes",
                "stopped",
                "source_interrupted",
            ];
            if fields["value"].get("numerical_replay").is_some() {
                expected_fields.push("numerical_replay");
            }
            let value = object(&fields["value"], &expected_fields)?;
            let cut = protocol::Expected {
                journal_revision: decimal(&value["journal_revision"])?,
                control_sequence: decimal(&value["control_sequence"])?,
                authority_epoch: decimal(&value["authority_epoch"])?,
                policy_generation: decimal(&value["policy_generation"])?,
            };
            if cut.journal_revision == 0
                || cut.journal_revision != basis_revision
                || cut.policy_generation == 0
            {
                return Err("governor inspection changed its acknowledged native cut".into());
            }
            let campaigns = object(
                &value["campaigns"],
                &["cases", "input_bytes", "max_campaigns"],
            )?;
            let bounded = |value: &Json, maximum: usize| -> Result<usize, String> {
                let number = usize::try_from(decimal(value)?).map_err(debug)?;
                if number == 0 || number > maximum {
                    return Err("governor inspection exceeds its finite selected bounds".into());
                }
                Ok(number)
            };
            let campaigns = FileCampaignRequirement {
                limits: ReplayLimits {
                    cases: bounded(&campaigns["cases"], MAX_REPLAY_CASES)?,
                    input_bytes: bounded(&campaigns["input_bytes"], MAX_REPLAY_INPUT_BYTES)?,
                },
                max_campaigns: bounded(&campaigns["max_campaigns"], MAX_POLICY_CAMPAIGNS)?,
            };
            let campaign_limit = bounded(
                &value["campaign_limit"],
                super::profile::MAX_CAMPAIGNS.min(campaigns.max_campaigns),
            )?;
            let stopped = value["stopped"]
                .as_bool()
                .ok_or("governor inspection Stop status must be Boolean")?;
            let source_interrupted = match &value["source_interrupted"] {
                Json::Null => None,
                Json::Bool(interrupted) => Some(*interrupted),
                _ => return Err("governor inspection source status must be Boolean or null".into()),
            };
            let numerical_replay = match value.get("numerical_replay") {
                None => None,
                Some(value) => {
                    let numerical =
                        object(value, &["max_replay_events", "events", "remaining_events"])?;
                    let max_replay_events = bounded(
                        &numerical["max_replay_events"],
                        MAX_POLICY_PROMOTION_REPLAY_EVENTS,
                    )?;
                    let maximum = usize::try_from(MAX_CHANNEL_EXCHANGES)
                        .map_err(debug)?
                        .checked_mul(max_replay_events)
                        .ok_or("numerical replay bound overflow")?;
                    let events = bounded(&numerical["events"], maximum)?;
                    let remaining_events =
                        usize::try_from(decimal(&numerical["remaining_events"])?).map_err(debug)?;
                    if remaining_events > events {
                        return Err("governor inspection replenished numerical replay work".into());
                    }
                    Some(protocol::NumericalReplayInspection {
                        selected: super::profile::NumericalReplaySettings {
                            max_replay_events,
                            events,
                        },
                        remaining_events,
                    })
                }
            };
            Ok(ValidatedResponse::Inspection(protocol::Inspection {
                cut,
                campaigns,
                campaign_limit,
                report_bytes: bounded(&value["report_bytes"], report_bytes)?,
                stopped,
                source_interrupted,
                numerical_replay,
            }))
        }
        protocol::Command::Review {
            expected,
            candidate,
            ..
        } => {
            let value = object(&fields["value"], &["journal_revision", "reviewed_report"])?;
            let revision = decimal(&value["journal_revision"])?;
            if revision != basis_revision
                || Some(revision) != expected.journal_revision.checked_add(1)
            {
                return Err(
                    "governor review acknowledgment changed the original predecessor".into(),
                );
            }
            let report = value["reviewed_report"]
                .as_str()
                .ok_or("complete governor report must be text")?;
            validate_report(request, expected, candidate, report, report_bytes)?;
            Ok(ValidatedResponse::Review)
        }
        protocol::Command::Approve {
            expected_journal_revision,
            ..
        } => {
            let value = object(
                &fields["value"],
                &[
                    "journal_revision",
                    "approval",
                    "policy_changed",
                    "effect_authority",
                ],
            )?;
            let revision = decimal(&value["journal_revision"])?;
            if revision != basis_revision
                || Some(revision) != expected_journal_revision.checked_add(1)
                || value["approval"].as_str() != Some("delegated")
                || value["policy_changed"].as_bool() != Some(false)
                || value["effect_authority"].as_bool() != Some(false)
            {
                return Err("governor approval acknowledgment claims an unrelated transition or effect authority".into());
            }
            Ok(ValidatedResponse::Approval)
        }
    }
}

fn validate_report(
    request: &protocol::Request,
    expected: &protocol::Expected,
    candidate: &crate::policy_replay::Request,
    text: &str,
    maximum: usize,
) -> Result<(), String> {
    let maximum = maximum.min(candidate.output_bytes);
    let report = parse(text.as_bytes(), maximum, maximum, maximum / 2)?;
    let root = object(&report, &["schema_version", "knowledge"])?;
    if root["schema_version"].as_str() != Some("fa.policy-replay/1") {
        return Err("governor returned an unsupported complete report".into());
    }
    let knowledge = object(&root["knowledge"], &["kind", "basis", "value"])?;
    if knowledge["kind"].as_str() != Some("known") {
        return Err("governor returned an incomplete replay report".into());
    }
    let basis = object(
        &knowledge["basis"],
        &["class", "scope", "provenance", "cut"],
    )?;
    if basis["class"].as_str() != Some("bounded_model")
        || basis["scope"].as_str() != Some("retained_exact_policy_corpus")
        || basis["provenance"].as_str() != Some("operator_trusted_local_journal")
        || basis["cut"].as_str() != Some("single_canonical_image")
    {
        return Err("governor report changed the original evidence basis".into());
    }
    let value = object(
        &knowledge["value"],
        &[
            "operation",
            "historical",
            "replay_grade",
            "authority",
            "policy_changed",
            "helpers_rerun",
            "live_evidence_read",
            "cut",
            "limits",
            "previous_policy",
            "candidate_policy",
            "case_count",
            "input_bytes",
            "requires_shadow",
            "newly_reviewable",
            "newly_blocked",
            "cases",
        ],
    )?;
    let get = |key: &str| {
        value
            .get(key)
            .ok_or_else(|| format!("missing complete report field {key}"))
    };
    let cut = object(
        get("cut")?,
        &[
            "journal_revision",
            "control_sequence",
            "authority_epoch",
            "executions",
            "charged",
            "reserved",
        ],
    )?;
    if cut["journal_revision"].as_u64() != Some(expected.journal_revision)
        || cut["control_sequence"].as_u64() != Some(expected.control_sequence)
        || cut["authority_epoch"].as_u64() != Some(expected.authority_epoch)
        || ["executions", "charged", "reserved"]
            .iter()
            .any(|name| cut[*name].as_u64().is_none())
        || get("operation")?.as_str() != Some("policy-replay")
        || get("replay_grade")?.as_str() != Some("decision_replay")
        || get("authority")?.as_str() != Some("not_issued")
        || get("policy_changed")?.as_bool() != Some(false)
        || get("helpers_rerun")?.as_bool() != Some(false)
        || get("live_evidence_read")?.as_bool() != Some(false)
        || get("historical")?.as_bool() != Some(true)
    {
        return Err("complete governor report does not match the original historical cut".into());
    }
    let previous = policy_from_json(get("previous_policy")?.clone())?;
    let next = policy_from_json(get("candidate_policy")?.clone())?;
    if previous.generation() != expected.policy_generation
        || previous.generation() != candidate.expected_policy_generation
        || next != candidate.candidate
    {
        return Err("governor report changed the independently selected candidate policy".into());
    }
    let limits = object(get("limits")?, &["cases", "input_bytes", "output_bytes"])?;
    if limits["cases"].as_u64() != Some(candidate.limits.cases as u64)
        || limits["input_bytes"].as_u64() != Some(candidate.limits.input_bytes as u64)
        || limits["output_bytes"].as_u64() != Some(candidate.output_bytes as u64)
    {
        return Err("governor report changed its explicit replay or output bounds".into());
    }
    let cases = get("cases")?
        .as_array()
        .ok_or("missing complete governor case list")?;
    if cases.is_empty()
        || cases.len() > candidate.limits.cases
        || get("case_count")?.as_u64() != Some(cases.len() as u64)
        || get("input_bytes")?
            .as_u64()
            .is_none_or(|bytes| bytes > candidate.limits.input_bytes as u64)
        || get("requires_shadow")?.as_bool().is_none()
    {
        return Err("governor report case count or replay input bound is inconsistent".into());
    }
    let mut ids = BTreeSet::new();
    let mut newly_reviewable = Vec::new();
    let mut newly_blocked = Vec::new();
    let mut requires_shadow = false;
    // Validate the complete original rendering, without evaluating a policy,
    // supplying missing witnesses or treating remote bytes as native evidence.
    for case in cases {
        let fields = object(
            case,
            &[
                "id",
                "delta",
                "snapshot_semantic_epoch",
                "action",
                "original",
                "original_consequence",
                "candidate",
            ],
        )?;
        let id = case_id(&fields["id"])?;
        if !ids.insert(id) || fields["snapshot_semantic_epoch"].as_u64().is_none() {
            return Err("governor report has duplicate cases or an invalid retained epoch".into());
        }
        action_shape(
            &fields["action"],
            request.scope,
            candidate.limits.input_bytes,
        )?;
        evaluation_shape(&fields["original"], previous.nodes().len(), false)?;
        let consequence = &fields["original_consequence"];
        if !consequence.is_null()
            && !matches!(
                consequence.as_str(),
                Some(
                    "continue"
                        | "hold_effect"
                        | "deny"
                        | "narrow_authority"
                        | "suspend_run"
                        | "reset_to_checkpoint"
                )
            )
        {
            return Err("governor report contains an invalid original consequence".into());
        }
        let delta = fields["delta"]
            .as_str()
            .ok_or("missing original case delta")?;
        match delta {
            "unchanged" => {}
            "newly_reviewable" => newly_reviewable.push(id),
            "newly_blocked" => newly_blocked.push(id),
            "requires_shadow" => requires_shadow = true,
            _ => return Err("unregistered governor case delta".into()),
        }
        let candidate_evaluation = &fields["candidate"];
        if candidate_evaluation.get("kind").and_then(Json::as_str) == Some("unknown") {
            let unknown = object(
                candidate_evaluation,
                &["kind", "reason_code", "missing_nodes"],
            )?;
            if unknown["reason_code"].as_str() != Some("MissingRequiredObservation")
                || delta != "requires_shadow"
            {
                return Err("governor case hides missing observations".into());
            }
            let missing = unknown["missing_nodes"]
                .as_array()
                .ok_or("missing shadow node list")?;
            if missing.is_empty() || missing.len() > next.nodes().len() {
                return Err("governor shadow node list exceeds the original candidate".into());
            }
            let mut previous_node = None;
            for value in missing {
                let node = value
                    .as_u64()
                    .ok_or("shadow node must be an unsigned integer")?;
                if node >= next.nodes().len() as u64 || previous_node.is_some_and(|old| node <= old)
                {
                    return Err("governor shadow node list is not canonical".into());
                }
                previous_node = Some(node);
            }
        } else {
            evaluation_shape(candidate_evaluation, next.nodes().len(), true)?;
        }
    }
    if case_ids(get("newly_reviewable")?, cases.len())? != newly_reviewable
        || case_ids(get("newly_blocked")?, cases.len())? != newly_blocked
        || get("requires_shadow")?.as_bool() != Some(requires_shadow)
    {
        return Err(
            "governor report summary omits or changes its original case dispositions".into(),
        );
    }
    Ok(())
}

fn case_id(value: &Json) -> Result<ReplayCaseId, String> {
    let read = |fields: &std::collections::BTreeMap<String, Json>, name: &str| {
        fields[name]
            .as_u64()
            .filter(|value| *value != 0)
            .ok_or_else(|| format!("governor case {name} must be a nonzero unsigned integer"))
    };
    match value.get("kind").and_then(Json::as_str) {
        Some("proposal") => {
            let fields = object(value, &["kind", "attempt"])?;
            Ok(ReplayCaseId::Proposal(read(fields, "attempt")?))
        }
        Some("review") => {
            let fields = object(value, &["kind", "attempt", "round", "control_sequence"])?;
            Ok(ReplayCaseId::Review {
                attempt: read(fields, "attempt")?,
                // Native round identities may be zero; only the attempt and
                // recorded review transition have nonzero identity floors.
                round: fields["round"]
                    .as_u64()
                    .ok_or("governor case round must be an unsigned integer")?,
                control_sequence: read(fields, "control_sequence")?,
            })
        }
        _ => Err("unregistered governor case identity".into()),
    }
}

fn case_ids(value: &Json, maximum: usize) -> Result<Vec<ReplayCaseId>, String> {
    let values = value
        .as_array()
        .ok_or("missing complete governor disposition list")?;
    if values.len() > maximum {
        return Err("governor disposition list exceeds its complete case count".into());
    }
    values.iter().map(case_id).collect()
}

fn action_shape(value: &Json, expected: Scope, maximum_payload: usize) -> Result<(), String> {
    let action = object(
        value,
        &[
            "version",
            "scope",
            "target",
            "payload_hex",
            "units",
            "policy_epoch",
            "deadline",
        ],
    )?;
    let scope = object(
        &action["scope"],
        &[
            "tenant",
            "principal",
            "run",
            "branch",
            "authority",
            "purpose",
        ],
    )?;
    if scope["tenant"].as_u64() != Some(expected.tenant)
        || scope["principal"].as_u64() != Some(expected.principal)
        || scope["run"].as_u64() != Some(expected.run)
        || scope["branch"].as_u64() != Some(expected.branch)
        || scope["authority"].as_u64() != Some(expected.authority)
        || scope["purpose"].as_str() != Some("effect")
    {
        return Err("governor report contains a foreign effect audience".into());
    }
    if action["version"].as_u64() != Some(VERSION as u64)
        || ["units", "policy_epoch", "deadline"]
            .iter()
            .any(|name| action[*name].as_u64().is_none())
    {
        return Err("governor report contains malformed original action coordinates".into());
    }
    let payload = action["payload_hex"]
        .as_str()
        .ok_or("missing original action payload")?;
    if payload.len() % 2 != 0
        || payload.len() / 2 > maximum_payload
        || !payload
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err("governor report payload is not bounded canonical hexadecimal".into());
    }
    if !action["target"].is_null() {
        let names = [
            "adapter",
            "object",
            "contract_version",
            "expected_version",
            "generation",
        ];
        let target = object(&action["target"], &names)?;
        if names
            .iter()
            .any(|name| target[*name].as_u64().is_none_or(|value| value == 0))
        {
            return Err("governor report contains an incomplete target identity".into());
        }
    }
    Ok(())
}

fn evaluation_shape(value: &Json, nodes: usize, allow_unknown: bool) -> Result<(), String> {
    let fields = object(value, &["kind", "basis", "result", "trace"])?;
    let truth = |value: &Json| {
        matches!(value.as_str(), Some("satisfied" | "violated"))
            || (allow_unknown && value.as_str() == Some("unknown"))
    };
    if fields["kind"].as_str() != Some("known")
        || fields["basis"].as_str() != Some("retained_original_observations")
        || !truth(&fields["result"])
    {
        return Err("governor report contains an invalid retained evaluation".into());
    }
    let trace = fields["trace"]
        .as_array()
        .ok_or("missing complete original policy trace")?;
    if trace.len() != nodes || trace.is_empty() {
        return Err("governor report truncated the original policy trace".into());
    }
    for (index, step) in trace.iter().enumerate() {
        let step = object(step, &["node", "result"])?;
        if step["node"].as_u64() != Some(index as u64) || !truth(&step["result"]) {
            return Err("governor report contains a malformed policy trace step".into());
        }
    }
    if trace.last().and_then(|step| step.get("result")) != Some(&fields["result"]) {
        return Err("governor report result differs from its original root trace".into());
    }
    Ok(())
}
