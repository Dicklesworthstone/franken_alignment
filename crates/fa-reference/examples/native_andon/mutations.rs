//! Offline assessment/window continuation through the original sealed recovery.
//! The operator is a trusted evaluator custodian; JSON is NOT a signed label.
#[path = "assessment.rs"]
mod assessment;
#[path = "promotion.rs"]
mod promotion;
use super::{Settings, report, settings};
use crate::config::{Config, debug, read_regular};
use crate::profile::ResumeProfile;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileOversight,
    credibility::{FileCredibilityUpdate, FileIndependentEvaluator},
    guarded::recovery::FileGuardedRecoveryStatus,
};
use fa_reference::action::consequence::oversight::credibility::Assessment;
use fa_reference::round::review::andon::AndonBudget;
use fa_reference::strict_json::{self, Json, Limits};
use settings::{end, number, object, take};
use std::{io::Write, path::Path};

pub(super) enum Mutation {
    Assess {
        round: u64,
        attempt: u64,
        policy: u64,
        congress: u64,
        root: [u8; 32],
        expected_revision: u64,
        assessment: Assessment,
    },
    Window(AndonBudget),
    Promote(FileCredibilityUpdate),
}
impl Mutation {
    pub(super) fn read(mode: &str, path: &Path) -> Result<Self, String> {
        Self::decode(mode, &read_regular(path, 8192)?)
    }
    pub(super) fn decode(mode: &str, bytes: &[u8]) -> Result<Self, String> {
        if mode == "assess" {
            let request = assessment::AssessmentRequest::decode(bytes)?;
            return Ok(Self::Assess {
                round: request.round,
                attempt: request.attempt,
                policy: request.policy,
                congress: request.congress,
                root: request.root,
                expected_revision: request.expected_revision,
                assessment: request.assessment,
            });
        }
        let mut fields = object(
            strict_json::parse(
                bytes,
                Limits {
                    max_bytes: 8192,
                    max_depth: 4,
                    max_items: 64,
                    max_string_bytes: 256,
                },
            )
            .map_err(debug)?,
        )?;
        let schema = take(&mut fields, "schema_version")?;
        match mode {
            "promote" if schema.as_str() == Some("fa.native-andon-promotion/1") => {
                let request = FileCredibilityUpdate {
                    operation: number(&mut fields, "operation")?,
                    expected_control_sequence: number(&mut fields, "expected_control_sequence")?,
                    expected_authority_epoch: number(&mut fields, "expected_authority_epoch")?,
                    expected_evaluation_revision: number(
                        &mut fields,
                        "expected_evaluation_revision",
                    )?,
                };
                end(fields)?;
                if request.operation == 0 {
                    return Err("promotion operation must be nonzero".into());
                }
                Ok(Self::Promote(request))
            }
            "register-window" if schema.as_str() == Some("fa.native-andon-window/1") => {
                let window = settings::windows(Json::Array(vec![Json::Object(fields)]))?
                    .into_iter()
                    .next()
                    .ok_or("missing window")?;
                // Validate the original single-window law before acquiring an owner.
                let mut ledger = fa_reference::round::review::andon::AndonLedger::new(
                    fa_reference::round::review::andon::LedgerIdentity {
                        deployment: 1,
                        evaluator: 1,
                    },
                )
                .map_err(debug)?;
                ledger.register_window(window).map_err(debug)?;
                Ok(Self::Window(window))
            }
            _ => Err("wrong native mutation schema or command".into()),
        }
    }
    fn operation(&self) -> &'static str {
        match self {
            Self::Assess { .. } => "assess",
            Self::Window(_) => "register-window",
            Self::Promote(_) => "promote",
        }
    }
    fn apply(
        &self,
        host: &mut FileOversight,
        evaluator: &FileIndependentEvaluator,
        checkpoint: &mut impl FnMut() -> Result<(), String>,
        clock: &mut impl FnMut() -> Result<ElapsedTick, String>,
    ) -> Result<bool, String> {
        match self {
            Self::Promote(request) => {
                checkpoint()?;
                promotion::apply(host, request, checkpoint, clock)
            }
            Self::Window(window) => {
                checkpoint()?;
                host.register_andon_window(host.revision(), *window)
                    .map_err(debug)
            }
            Self::Assess {
                round,
                attempt,
                policy,
                congress,
                root,
                expected_revision,
                assessment,
            } => assessment::AssessmentRequest {
                round: *round,
                attempt: *attempt,
                policy: *policy,
                congress: *congress,
                root: *root,
                expected_revision: *expected_revision,
                assessment: *assessment,
            }
            .apply(host, evaluator, checkpoint, clock),
        }
    }
}

pub(super) fn clock() -> Result<ElapsedTick, String> {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("Unix clock unavailable: {error}"))?;
    let tick = u64::try_from(duration.as_millis()).map_err(debug)?;
    Ok(ElapsedTick(tick))
}

pub(super) fn run(
    config: &Config,
    settings: &Settings,
    input: &Path,
    mutation: &Mutation,
    checkpoint: &mut impl FnMut() -> Result<(), String>,
    clock: &mut impl FnMut() -> Result<ElapsedTick, String>,
    out: &mut impl Write,
) -> Result<(), String> {
    if matches!(mutation, Mutation::Promote(_)) && settings.joint.is_none() {
        return Err("operator promotion requires an explicit joint native Andon /2 profile".into());
    }
    let request = ResumeProfile::read(input)?;
    let custody = request.admit(config)?;
    if !custody.retention_requested() {
        return Err("mutation requires a new independent retain_history_anchor path".into());
    }
    let expected = settings.requirements(config, request.minimum())?;
    checkpoint()?;
    let mut recovery = FileOversight::begin_open_andon_guarded(
        &config.store,
        config.profile.clone(),
        &expected,
        &settings.protocol,
        custody.anchor(),
    )
    .map_err(debug)?;
    loop {
        // Ready has its own checkpoint; no last-event timeout can publish a fence.
        checkpoint()?;
        let progress = recovery.progress();
        if progress.status == FileGuardedRecoveryStatus::Ready {
            break;
        }
        recovery
            .advance(progress.replayed_events, 1)
            .map_err(debug)?;
    }
    let (mut host, roles) = recovery.finish().map_err(|error| {
        format!(
            "native Andon recovery is unconfirmed: {}; no assessment was attempted",
            debug(error)
        )
    })?;
    let fence_revision = host.revision();
    let outcome = mutation.apply(&mut host, &roles.evaluator, checkpoint, clock);
    let deadline = checkpoint();
    // A failed replacement may have published a newer state than RAM. Do not
    // retain a stale successor anchor or render stale accounting as current.
    if let Some(failure) = host.storage_failure() {
        return Err(format!(
            "{} persistence is unconfirmed after recovery revision {fence_revision}; last acknowledged revision {}; {failure:?}; keep the previous anchor; no retry was attempted",
            mutation.operation(),
            host.revision()
        ));
    }
    let retained = host
        .history_anchor_after(custody.anchor())
        .map_err(debug)
        .and_then(|anchor| custody.retain(&anchor));
    let status = match &outcome {
        Ok(true) => "applied",
        Ok(false) => "already_applied",
        Err(_) => "refused",
    };
    let reported = (|| -> Result<(), String> {
        let mut state = Vec::new();
        report(
            mutation.operation(),
            false,
            retained.is_ok(),
            &host.inspect(),
            &host.credibility_report().map_err(debug)?,
            host.andon_ledger().map_err(debug)?,
            &mut state,
        )?;
        write!(out, "{{\"schema_version\":\"fa.native-andon-mutation-result/1\",\"status\":\"{status}\",\"recovery_fence_revision\":\"{fence_revision}\",\"state\":")
            .map_err(debug)?;
        out.write_all(&state).map_err(debug)?;
        if let Mutation::Promote(request) = mutation
            && outcome.is_ok()
        {
            promotion::report(&host, request.operation, out)?;
        }
        writeln!(out, "}}").map_err(debug)?;
        out.flush().map_err(debug)
    })();
    let errors: Vec<_> = [outcome.map(|_| ()), deadline, retained, reported]
        .into_iter()
        .filter_map(Result::err)
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "native {} {status}; recovery acknowledged at {fence_revision}, latest acknowledged revision {}: {}; no operation was retried",
            mutation.operation(),
            host.revision(),
            errors.join("; ")
        ))
    }
}
