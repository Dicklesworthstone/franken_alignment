//! Trusted offline maintenance of the original learned credential epoch.
//! One independently anchored guarded replay precedes the original lifecycle
//! transition. No live publication capability, helper or actor service is made.
use super::{
    Config, Duration, FileOversight, Instant, JournalError, Loaded, anchors, checked, credential,
    debug, recipe, recovery, topology,
};
use fa_reference::action::consequence::delivery::{
    credential_broker::{
        CredentialChangeReceipt, CredentialRevocationRequest, CredentialRotationRequest,
    },
    persistent::observed::credential::FileCredentialStatus,
};
use std::{io::Write, path::Path};

#[path = "credential_lifecycle/topology.rs"]
mod custody;
#[path = "credential_lifecycle/owner.rs"]
mod owner;

const USAGE: &str = "create-learned-generated CONFIG LEARNED_RECIPE (--rotate-credential OPERATION EXPECTED_GENERATION NEXT_GENERATION | --revoke-credential OPERATION EXPECTED_GENERATION) --history-anchor INPUT --retain-history-anchor NEW_OUTPUT [--topology-profile PATH --retain-topology-checkpoint NEW_OUTPUT [--recover-credential-checkpoint]]";

#[derive(Clone, Copy)]
pub(super) enum Request {
    Rotate(CredentialRotationRequest),
    Revoke(CredentialRevocationRequest),
}

impl Request {
    fn apply(self, host: &mut FileOversight) -> Result<CredentialChangeReceipt, JournalError> {
        match self {
            Self::Rotate(request) => host.rotate_credential_guard(host.revision(), request),
            Self::Revoke(request) => host.revoke_credential_guard(host.revision(), request),
        }
    }
}

/// This owner stays private to maintenance. The original guard checks supply
/// both current state and the historical retry receipt before the recovery Fence.
pub(super) struct Recovered<H = FileOversight> {
    pub host: H,
    pub current: FileCredentialStatus,
    pub retry: Option<CredentialChangeReceipt>,
}

pub(super) fn is_option(value: &str) -> bool {
    matches!(value, "--rotate-credential" | "--revoke-credential")
}

pub(super) fn command(args: &[String], credibility: Option<&Path>) -> Result<(), String> {
    let started = Instant::now();
    command_with(
        args,
        credibility,
        &mut || started.elapsed(),
        &mut std::io::stdout().lock(),
    )
}

/// The actual command, with deterministic elapsed/output seams. Neither seam is
/// a logical clock, key, provider credential or permission to resume generation.
pub(super) fn command_with(
    args: &[String],
    credibility: Option<&Path>,
    elapsed: &mut impl FnMut() -> Duration,
    out: &mut impl Write,
) -> Result<(), String> {
    if credibility.is_some() {
        return Err("credential lifecycle cannot activate credibility".into());
    }
    let (args, topology_output, recover_checkpoint) = custody::take_options(args)?;
    let (args, topology_path) = topology::take_option(&args)?;
    custody::check_pair(
        topology_path.as_deref(),
        topology_output.as_deref(),
        recover_checkpoint,
    )?;
    let has_input = args.iter().any(|arg| arg == "--history-anchor");
    let has_output = args.iter().any(|arg| arg == "--retain-history-anchor");
    let (args, options) = anchors::take_options(&args)?;
    if !has_input
        || !has_output
        || args.first().map(String::as_str) != Some("create-learned-generated")
    {
        return Err(USAGE.into());
    }
    let request = match (args.len(), args.get(3).map(String::as_str)) {
        (7, Some("--rotate-credential")) => {
            let request = CredentialRotationRequest {
                operation: number(&args[4])?,
                expected_generation: number(&args[5])?,
                next_generation: number(&args[6])?,
            };
            if request.expected_generation.checked_add(1) != Some(request.next_generation) {
                return Err("credential rotation requires the exact next generation".into());
            }
            Request::Rotate(request)
        }
        (6, Some("--revoke-credential")) => Request::Revoke(CredentialRevocationRequest {
            operation: number(&args[4])?,
            expected_generation: number(&args[5])?,
        }),
        _ => return Err(USAGE.into()),
    };
    let config = Config::read(Path::new(&args[1]))?;
    let wall = Duration::from_millis(config.timing.runtime_ms);
    let mut checkpoint = || check_elapsed(elapsed, wall);
    checkpoint()?;
    // Full original numerical, identity, predictor, joint-policy and witness
    // admission remains mandatory. Historical loading skips live inventory,
    // secret inputs, native helper rosters and current evidence.
    let mut loaded = recipe::load(Path::new(&args[2]), &config, false)?;
    credential::check_selection(&loaded)?;
    checked::check_selection(&config, &loaded)?;
    if loaded.credential.is_none() {
        return Err(
            "credential lifecycle requires an independently selected credential profile".into(),
        );
    }
    checkpoint()?;
    options.admit(&config, &mut loaded, true)?;
    checkpoint()?;
    topology::admit(
        &config,
        &mut loaded,
        topology_path.as_deref(),
        topology::Mode::Historical,
    )?;
    checkpoint()?;
    let topology_output = custody::admit(&config, &loaded, topology_output.as_deref())?;
    checkpoint()?;
    run(
        &config,
        &loaded,
        request,
        topology_output.as_ref(),
        recover_checkpoint,
        elapsed,
        out,
    )
}

fn number(text: &str) -> Result<u64, String> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(USAGE.into());
    }
    let value = text.parse::<u64>().map_err(|_| USAGE.to_owned())?;
    if value == 0 {
        return Err(USAGE.into());
    }
    Ok(value)
}

fn check_elapsed(elapsed: &mut impl FnMut() -> Duration, wall: Duration) -> Result<(), String> {
    if elapsed() >= wall {
        Err("learned credential lifecycle deadline reached".into())
    } else {
        Ok(())
    }
}

fn run(
    config: &Config,
    loaded: &Loaded,
    request: Request,
    topology_output: Option<&custody::Output>,
    recover_checkpoint: bool,
    elapsed: &mut impl FnMut() -> Duration,
    out: &mut impl Write,
) -> Result<(), String> {
    if recover_checkpoint {
        return run_repaired(config, loaded, request, topology_output, elapsed, out);
    }
    let wall = Duration::from_millis(config.timing.runtime_ms);
    let recovered =
        recovery::open_selected_credential_with_checkpoints(config, loaded, request, &mut |_| {
            check_elapsed(elapsed, wall)
        })?;
    run_recovered(
        config,
        loaded,
        request,
        topology_output,
        recovered,
        elapsed,
        out,
    )
}

// Mutually exclusive native owners stay in separate startup frames.
fn run_repaired(
    config: &Config,
    loaded: &Loaded,
    request: Request,
    topology_output: Option<&custody::Output>,
    elapsed: &mut impl FnMut() -> Duration,
    out: &mut impl Write,
) -> Result<(), String> {
    let wall = Duration::from_millis(config.timing.runtime_ms);
    let recovered = recovery::credential::open(config, loaded, request, &mut |_| {
        check_elapsed(elapsed, wall)
    })?;
    run_recovered(
        config,
        loaded,
        request,
        topology_output,
        recovered,
        elapsed,
        out,
    )
}

fn run_recovered<H: owner::Host>(
    config: &Config,
    loaded: &Loaded,
    request: Request,
    topology_output: Option<&custody::Output>,
    recovered: Recovered<H>,
    elapsed: &mut impl FnMut() -> Duration,
    out: &mut impl Write,
) -> Result<(), String> {
    let previous = loaded.recovery_anchor.as_ref().ok_or(USAGE)?;
    let output = loaded.anchor_output.as_deref().ok_or(USAGE)?;
    let wall = Duration::from_millis(config.timing.runtime_ms);
    let Recovered {
        mut host,
        current: before,
        retry,
    } = recovered;
    let fence_revision = host.revision();
    // Capture the acknowledged Fence BEFORE attempting another write. If a
    // later journal replacement becomes ambiguous, this remains an honest
    // prefix; the unavailable owner's old RAM state cannot certify a new cut.
    let fence_anchor = match host.history_anchor_after(previous) {
        Ok(anchor) => anchor,
        Err(error) => {
            let failure = format!(
                "recovery Fence acknowledged at revision {fence_revision}, but its history anchor could not be captured: {}; credential transition was not attempted",
                debug(error)
            );
            let reported = report(
                "refused",
                None,
                &before,
                previous.revision(),
                fence_revision,
                fence_revision,
                Retained::empty(topology_output.is_some()),
                out,
            );
            return Err(combine(failure, reported.err()));
        }
    };
    // A later ambiguous credential write cannot certify any newer source cut.
    // Keep the SAME acknowledged Fence pair before attempting that write.
    let fence_checkpoint = match custody::capture(loaded, &host, &fence_anchor) {
        Ok(checkpoint) => checkpoint,
        Err(error) => {
            let retained = anchors::export_acknowledged(&fence_anchor, output);
            let failure = format!(
                "recovery Fence acknowledged at revision {fence_revision}, but its source checkpoint could not be captured: {error}; credential transition was not attempted"
            );
            let reported = report(
                "refused",
                None,
                &before,
                previous.revision(),
                fence_revision,
                fence_revision,
                Retained {
                    history: retained.as_ref().ok().map(|()| fence_revision),
                    topology: None,
                    topology_required: topology_output.is_some(),
                },
                out,
            );
            return Err(combine(combine(failure, retained.err()), reported.err()));
        }
    };
    let mut failures = Vec::new();
    // A fully checked exact retry already has an acknowledged historical
    // receipt, even if the deadline prevents invoking the idempotent API again.
    let mut receipt = retry;
    match check_elapsed(elapsed, wall) {
        Err(error) => failures.push(error),
        Ok(()) => match host.apply(request) {
            Ok(original) => receipt = Some(original),
            Err(error) => failures.push(format!(
                "native credential transition failed: {}",
                debug(error)
            )),
        },
    }
    // A deadline after acknowledgment cannot retract its receipt, skip custody
    // retention or cause a second transition. All remaining work is reporting.
    if let Err(error) = check_elapsed(elapsed, wall) {
        failures.push(error);
    }
    if host.storage_failed() {
        let retained = anchors::export_acknowledged(&fence_anchor, output);
        let source_retained = custody::retain(topology_output, fence_checkpoint.as_ref());
        let custody = match retained {
            Ok(()) => {
                format!("acknowledged Fence prefix retained through revision {fence_revision}")
            }
            Err(error) => format!(
                "acknowledged Fence prefix retention failed: {error}; keep the previous archive and select a new output path"
            ),
        };
        // No known-current report can be formed after an ambiguous replacement.
        // A retained prefix is not a receipt for the unconfirmed transition.
        let custody = match source_retained {
            Ok(Some(revision)) => format!(
                "{custody}; acknowledged Fence source checkpoint retained through revision {revision}"
            ),
            Ok(None) => custody,
            Err(error) => format!("{custody}; {error}"),
        };
        return Err(format!(
            "{}; credential transition and current status are unconfirmed after storage failure; {custody}; no transition was retried",
            failures.join("; ")
        ));
    }
    let current = match host.credential_status() {
        Ok(Some(current)) => current,
        Ok(None) | Err(_) => {
            let retained = anchors::export_acknowledged(&fence_anchor, output);
            let source_retained = custody::retain(topology_output, fence_checkpoint.as_ref());
            let failure = format!(
                "recovery Fence acknowledged at revision {fence_revision}, but current credential status is unavailable; no transition was retried"
            );
            return Err(combine(
                combine(failure, retained.err()),
                source_retained.err(),
            ));
        }
    };
    let revision = host.revision();
    let (anchor, source_checkpoint) = match host.history_anchor_after(previous) {
        Ok(anchor) => {
            let checkpoint = match custody::capture(loaded, &host, &anchor) {
                Ok(checkpoint) => checkpoint,
                Err(error) => {
                    failures.push(format!(
                        "latest acknowledged source checkpoint could not be captured: {error}"
                    ));
                    None
                }
            };
            (anchor, checkpoint)
        }
        Err(error) => {
            failures.push(format!(
                "latest acknowledged history anchor could not be captured: {}",
                debug(error)
            ));
            (fence_anchor, fence_checkpoint)
        }
    };
    let retained_revision = match anchors::export_acknowledged(&anchor, output) {
        Ok(()) => Some(anchor.revision()),
        Err(error) => {
            failures.push(format!("history anchor retention failed: {error}; keep the previous archive and select a new output path"));
            None
        }
    };
    // The two files are independent create-new writes. A failed history export
    // cannot suppress source retention, nor may a successful one imply the other.
    let retained_topology = match custody::retain(topology_output, source_checkpoint.as_ref()) {
        Ok(revision) => revision,
        Err(error) => {
            failures.push(error);
            None
        }
    };
    if let Err(error) = check_elapsed(elapsed, wall) {
        failures.push(error);
    }
    let status = match (receipt, retry) {
        (Some(_), Some(_)) => "already_applied",
        (Some(_), None) => "applied",
        (None, _) => "refused",
    };
    if let Err(error) = report(
        status,
        receipt,
        &current,
        previous.revision(),
        fence_revision,
        revision,
        Retained {
            history: retained_revision,
            topology: retained_topology,
            topology_required: topology_output.is_some(),
        },
        out,
    ) {
        failures.push(error);
    }
    if let Err(error) = check_elapsed(elapsed, wall) {
        failures.push(error);
    }
    if failures.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{}; recovery Fence acknowledged at revision {fence_revision}; latest acknowledged journal revision {revision}; credential transition {status}; no transition was retried",
        failures.join("; ")
    ))
}

fn combine(failure: String, other: Option<String>) -> String {
    match other {
        Some(other) => format!("{failure}; {other}"),
        None => failure,
    }
}

struct Retained {
    history: Option<u64>,
    topology: Option<u64>,
    topology_required: bool,
}
impl Retained {
    fn empty(topology_required: bool) -> Self {
        Self {
            history: None,
            topology: None,
            topology_required,
        }
    }
}

fn report(
    status: &str,
    receipt: Option<CredentialChangeReceipt>,
    current: &FileCredentialStatus,
    anchored_through: u64,
    fence_revision: u64,
    revision: u64,
    retained: Retained,
    out: &mut impl Write,
) -> Result<(), String> {
    // The bounded report includes only native operation receipts and current
    // nonsecret epoch state. Receipt generation and current generation remain
    // independent, especially when an old exact operation is retried later.
    let receipt = receipt.map_or_else(
        || "null".to_owned(),
        |receipt| {
            format!(
                "{{\"operation\":\"{}\",\"generation\":\"{}\",\"revoked\":{}}}",
                receipt.operation, receipt.generation, receipt.revoked
            )
        },
    );
    let history_revision = retained
        .history
        .map_or_else(|| "null".to_owned(), |revision| format!("\"{revision}\""));
    let topology_revision = retained
        .topology
        .map_or_else(|| "null".to_owned(), |revision| format!("\"{revision}\""));
    let report = format!(
        "{{\"schema\":\"fa.learned-credential-lifecycle-result/1\",\"status\":\"{status}\",\"knowledge\":{{\"kind\":\"known\",\"basis\":{{\"class\":\"bounded_model\",\"scope\":\"original_credential_lifecycle\",\"authentic_origin\":\"unknown\",\"independent_history_through\":\"{anchored_through}\",\"acknowledged_journal_revision\":\"{revision}\"}},\"value\":{{\"receipt\":{receipt},\"current\":{{\"generation\":\"{}\",\"revoked\":{},\"retained_changes\":\"{}\"}},\"recovery_fence_revision\":\"{fence_revision}\",\"journal_revision\":\"{revision}\",\"history_retained\":{},\"retained_history_revision\":{history_revision},\"topology_checkpoint_required\":{},\"topology_checkpoint_retained\":{},\"retained_topology_checkpoint_revision\":{topology_revision}}}}},\"authority\":\"none\"}}\n",
        current.generation,
        current.revoked,
        current.retained_changes,
        retained.history == Some(revision),
        retained.topology_required,
        retained.topology_required && retained.topology == Some(revision)
    );
    out.write_all(report.as_bytes())
        .and_then(|()| out.flush())
        .map_err(|error| format!("credential lifecycle result output failed: {error}"))
}
