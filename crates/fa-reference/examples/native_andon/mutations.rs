//! Offline assessment/window continuation through the original sealed recovery.
//! The operator is a trusted evaluator custodian; JSON is NOT a signed label.
use super::{Settings, report, settings};
use crate::config::{Config, debug, hex, read_regular};
use crate::profile::ResumeProfile;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileOversight, credibility::FileIndependentEvaluator,
    guarded::recovery::FileGuardedRecoveryStatus,
};
use fa_reference::action::consequence::oversight::credibility::{Assessment, GroundTruth};
use fa_reference::round::review::andon::AndonBudget;
use fa_reference::strict_json::{self, Json, Limits};
use settings::{end, number, object, take};
use std::{collections::BTreeMap, io::Write, path::Path};

pub(super) enum Mutation {
    Assess { round: u64, attempt: u64, policy: u64, congress: u64,
        root: [u8; 32], expected_revision: u64, assessment: Assessment },
    Window(AndonBudget),
}
impl Mutation {
    pub(super) fn read(mode: &str, path: &Path) -> Result<Self, String> {
        Self::decode(mode, &read_regular(path, 8192)?)
    }
    pub(super) fn decode(mode: &str, bytes: &[u8]) -> Result<Self, String> {
        let mut fields = object(strict_json::parse(bytes, Limits {
            max_bytes: 8192, max_depth: 4, max_items: 64, max_string_bytes: 256,
        }).map_err(debug)?)?;
        let schema = take(&mut fields, "schema_version")?;
        match mode {
            "register-window" if schema.as_str() == Some("fa.native-andon-window/1") => {
                let window = settings::windows(Json::Array(vec![Json::Object(fields)]))?
                    .into_iter().next().ok_or("missing window")?;
                // Validate the original single-window law before acquiring an owner.
                let mut ledger = fa_reference::round::review::andon::AndonLedger::new(
                    fa_reference::round::review::andon::LedgerIdentity { deployment: 1, evaluator: 1 })
                    .map_err(debug)?;
                ledger.register_window(window).map_err(debug)?;
                Ok(Self::Window(window))
            }
            "assess" if schema.as_str() == Some("fa.native-assessment/1") => {
                let round = number(&mut fields, "round")?;
                let attempt = number(&mut fields, "attempt")?;
                let policy = number(&mut fields, "policy_generation")?;
                let congress = number(&mut fields, "congress_generation")?;
                let root = digest(&mut fields, "case_evidence_root_hex")?;
                let expected_revision = number(&mut fields, "expected_evaluation_revision")?;
                let origin = number(&mut fields, "origin")?;
                let evidence_id = digest(&mut fields, "evidence_id_hex")?;
                let truth = match take(&mut fields, "truth")?.as_str() {
                    Some("benign") => GroundTruth::Benign,
                    Some("violation") => GroundTruth::Violation,
                    Some("censored") => GroundTruth::Censored,
                    _ => return Err("truth must be benign, violation or censored".into()),
                };
                end(fields)?;
                if [round, attempt, policy, congress, origin].contains(&0) {
                    return Err("assessment case and origin identities must be nonzero".into());
                }
                Ok(Self::Assess { round, attempt, policy, congress, root, expected_revision,
                    assessment: Assessment { origin, evidence_id, truth } })
            }
            _ => Err("wrong native mutation schema or command".into()),
        }
    }
    fn operation(&self) -> &'static str {
        match self { Self::Assess { .. } => "assess", Self::Window(_) => "register-window" }
    }
    fn apply(&self, host: &mut FileOversight, evaluator: &FileIndependentEvaluator,
        checkpoint: &mut impl FnMut() -> Result<(), String>,
        clock: &mut impl FnMut() -> Result<ElapsedTick, String>) -> Result<bool, String>
    {
        checkpoint()?;
        match self {
            Self::Window(window) => host.register_andon_window(host.revision(), *window).map_err(debug),
            Self::Assess { round, attempt, policy, congress, root, expected_revision, assessment } => {
                let ticket = host.evaluation_ticket(*round).map_err(debug)?;
                let case = ticket.case();
                if case.attempt != *attempt || case.policy_generation != *policy
                    || case.congress_generation != *congress || case.evidence_root != *root {
                    return Err("assessment does not bind the original applied case".into());
                }
                // Original exact retry needs neither a fresh clock nor new label
                // revision. Still use the original issuer/ticket check below.
                let repeated = host.evaluation_history(*round).map_err(debug)?.last() == Some(assessment);
                if !repeated {
                    if host.credibility_report().map_err(debug)?.revision != *expected_revision {
                        return Err("stale expected evaluation revision".into());
                    }
                    if assessment.truth == GroundTruth::Benign {
                        checkpoint()?;
                        let tick = clock()?;
                        checkpoint()?;
                        host.observe_time(host.revision(), tick).map_err(debug)?;
                        // Time and assessment are separate original journal events.
                        // An expired budget can leave an acknowledged Time, never a
                        // fabricated label. The resulting anchor retains that cut.
                        checkpoint()?;
                    }
                }
                let revision = host.revision();
                evaluator.assess(host, revision, &ticket, *assessment).map_err(debug)
            }
        }
    }
}

pub(super) fn clock() -> Result<ElapsedTick, String> {
    let duration = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("Unix clock unavailable: {error}"))?;
    let tick = u64::try_from(duration.as_millis()).map_err(debug)?;
    Ok(ElapsedTick(tick))
}

pub(super) fn run(config: &Config, settings: &Settings, input: &Path, mutation: &Mutation,
    checkpoint: &mut impl FnMut() -> Result<(), String>,
    clock: &mut impl FnMut() -> Result<ElapsedTick, String>, out: &mut impl Write) -> Result<(), String>
{
    let request = ResumeProfile::read(input)?;
    let custody = request.admit(config)?;
    if !custody.retention_requested() {
        return Err("mutation requires a new independent retain_history_anchor path".into());
    }
    let expected = settings.requirements(config, request.minimum())?;
    checkpoint()?;
    let mut recovery = FileOversight::begin_open_andon_guarded(&config.store,
        config.profile.clone(), &expected, &settings.protocol, custody.anchor()).map_err(debug)?;
    loop {
        // Ready has its own checkpoint; no last-event timeout can publish a fence.
        checkpoint()?;
        let progress = recovery.progress();
        if progress.status == FileGuardedRecoveryStatus::Ready { break; }
        recovery.advance(progress.replayed_events, 1).map_err(debug)?;
    }
    let (mut host, roles) = recovery.finish().map_err(|error| format!(
        "native Andon recovery is unconfirmed: {}; no assessment was attempted", debug(error)))?;
    let fence_revision = host.revision();
    let outcome = mutation.apply(&mut host, &roles.evaluator, checkpoint, clock);
    let deadline = checkpoint();
    // A failed replacement may have published a newer state than RAM. Do not
    // retain a stale successor anchor or render stale accounting as current.
    if let Some(failure) = host.storage_failure() {
        return Err(format!("{} persistence is unconfirmed after recovery revision {fence_revision}; last acknowledged revision {}; {failure:?}; keep the previous anchor; no retry was attempted",
            mutation.operation(), host.revision()));
    }
    let retained = host.history_anchor_after(custody.anchor()).map_err(debug)
        .and_then(|anchor| custody.retain(&anchor));
    let status = match &outcome { Ok(true) => "applied", Ok(false) => "already_applied", Err(_) => "refused" };
    let reported = (|| -> Result<(), String> {
        let mut state = Vec::new();
        report(mutation.operation(), false, retained.is_ok(), &host.inspect(),
            &host.credibility_report().map_err(debug)?, host.andon_ledger().map_err(debug)?, &mut state)?;
        write!(out, "{{\"schema_version\":\"fa.native-andon-mutation-result/1\",\"status\":\"{status}\",\"recovery_fence_revision\":\"{fence_revision}\",\"state\":")
            .map_err(debug)?;
        out.write_all(&state).map_err(debug)?;
        writeln!(out, "}}").map_err(debug)?;
        out.flush().map_err(debug)
    })();
    let errors: Vec<_> = [outcome.map(|_| ()), deadline, retained, reported].into_iter()
        .filter_map(Result::err).collect();
    if errors.is_empty() { Ok(()) } else {
        Err(format!("native {} {status}; recovery acknowledged at {fence_revision}, latest acknowledged revision {}: {}; no operation was retried",
            mutation.operation(), host.revision(), errors.join("; ")))
    }
}
fn digest(fields: &mut BTreeMap<String, Json>, key: &str) -> Result<[u8; 32], String> {
    let value = take(fields, key)?;
    let bytes = hex(value.as_str().ok_or("evidence identity must be lowercase hex")?)?;
    let digest: [u8; 32] = bytes.try_into().map_err(|_| "evidence identity must be 32 bytes")?;
    if digest == [0; 32] { return Err("evidence identity must be nonzero".into()); }
    Ok(digest)
}
