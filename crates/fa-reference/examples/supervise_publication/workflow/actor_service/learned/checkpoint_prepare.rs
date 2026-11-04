//! Fresh original prompt preparation ending at a durable, unsampled checkpoint.
//! This is a supervisor maintenance result, never a completed actor message.
use super::{Config, Control, Deadline, Duration, ElapsedTick, FileOversight,
    FileSupervisedDriver, GenerationGoal, GenerationOutcome, Instant, Loaded,
    anchors, campaigns, checked, checkpoint_on_generation::Selection, cleanup,
    debug, drive_generation, fields, plus, prediction, qualification, recipe, stop, topology};
use crate::workflow::continuation::checkpoint::Output;
use fa_reference::action::consequence::delivery::persistent::{FileDeliverySnapshot,
    observed::{decoder::learned::checkpoint::FileLearnedCheckpointInfo,
        driver::topology::checkpoint::TopologyCheckpoint,
        guarded::anchored::FileHistoryAnchor}};
use fa_reference::action::consequence::oversight::learned_host::{HostedLearnedInspection,
    checkpoint::HostedLearnedRecoveryUsage};
use std::{collections::BTreeSet, io::Write, path::Path};

const USAGE: &str = "create-learned-generated CONFIG LEARNED_RECIPE --prepare-stream-checkpoint CHECKPOINT_PROFILE --retain-history-anchor NEW_OUTPUT [--campaign-profile PATH] [--topology-profile CREATE_PROFILE]";

/// Own the complete grammar before any shared option stripper runs. There is no
/// recovery, actor, reviewer or automatic reset selection in this mode.
fn arguments(args: &[String]) -> Result<(), String> {
    if args.len() < 7 || args.first().map(String::as_str) != Some("create-learned-generated")
        || args.get(3).map(String::as_str) != Some("--prepare-stream-checkpoint") {
        return Err(USAGE.into());
    }
    for at in [1, 2, 4] { fields::path(&args[at])?; }
    let mut seen = BTreeSet::new();
    let mut at = 5;
    while at < args.len() {
        let flag = args[at].as_str();
        if !matches!(flag, "--retain-history-anchor" | "--campaign-profile" | "--topology-profile")
            || !seen.insert(flag) { return Err(USAGE.into()); }
        fields::path(args.get(at + 1).ok_or(USAGE)?)?;
        at += 2;
    }
    if !seen.contains("--retain-history-anchor") { return Err(USAGE.into()); }
    Ok(())
}

pub(super) fn command<F>(args: &[String], credibility: Option<&Path>, mut time: F,
    output: &mut impl Write) -> Result<(), String>
where F: FnMut() -> ElapsedTick {
    let started = Instant::now();
    if credibility.is_some() { return Err("prompt checkpoint preparation cannot activate credibility".into()); }
    arguments(args)?;
    let (args, campaign) = campaigns::take_option(args)?;
    let (args, topology_profile) = topology::take_option(&args)?;
    let (args, custody) = anchors::take_options(&args)?;
    let mut config = Config::read_learned_generation(Path::new(&args[1]))?;
    let selected = Selection::read(Path::new(&args[4]))?;
    // Original numerical, source, forecast and guard contracts are admitted;
    // helper execution assets belong to the later independently reviewed release.
    let mut loaded = recipe::load(Path::new(&args[2]), &config, false)?;
    campaigns::admit(&config, &mut loaded, campaign.as_deref(), false)?;
    custody.admit(&config, &mut loaded, false)?;
    topology::admit(&config, &mut loaded, topology_profile.as_deref(), topology::Mode::Create)?;
    check_selection(&config, &loaded, selected)?;
    let start = time();
    let lifetime = loaded.ttl_ms.min(config.timing.runtime_ms);
    let deadline = Deadline { logical: plus(start, lifetime)?, started,
        wall: Duration::from_millis(lifetime) };
    deadline.check(start)?;
    let mut control = Control::new(&config, loaded.request, None)?;
    let topology::Started { custody: (host, reviewer, observer, identity), source,
        output: source_output } = topology::start(&config, &loaded, false,
            &mut control, &deadline, &mut time)?;
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().map_err(debug)?;
    // No actor request or endpoint is admitted. The existing driver owns only
    // original generation, source freshness, deadlines and terminal cleanup.
    drop(port);
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut prediction = observer.map(|observer| prediction::Session::new(loaded.request, observer));
    let prepared = (|| {
        topology::attach(&mut driver, source)?;
        if identity.is_some() { return Err("unexpected identity custody during prompt preparation".into()); }
        if !qualification::prepare(&mut driver, &config, None,
            (&mut control, &reviewer, &deadline), &mut time)? {
            return Ok(GenerationOutcome::Stopped);
        }
        if driver.topology().is_some() {
            if control.checkpoint(&mut driver, &reviewer, &deadline, &mut time)? {
                return Ok(GenerationOutcome::Stopped);
            }
            topology::certify(&mut driver, &deadline, &mut time)?;
        }
        drive_generation(&mut driver, &reviewer, &mut config, &mut control, &deadline,
            &mut time, (prediction.as_mut(), Some(selected), GenerationGoal::Checkpoint))
    })();
    match prepared {
        Ok(GenerationOutcome::CheckpointCaptured) => {
            // Capture is already acknowledged. Later clock/archive/report errors
            // cannot turn this successful maintenance boundary into a Stop.
            let acknowledged = Prepared::capture(&driver, selected, source_output.as_ref());
            drop(prediction);
            drop(driver);
            let acknowledged = acknowledged.map_err(|error| format!(
                "prompt checkpoint {} acknowledged but its report is unavailable: {error}; preserve the original store",
                selected.checkpoint_id()))?;
            acknowledged.finish(&loaded, &deadline, &mut time, source_output.as_ref(), output)
        }
        result => {
            let error = match result {
                Err(error) => error,
                Ok(GenerationOutcome::Stopped) => "prompt preparation interrupted by the original Stop".into(),
                Ok(_) => "prompt preparation ended without its selected checkpoint".into(),
            };
            let error = match stop(&mut driver, loaded.request, &mut time) {
                Ok(()) => error,
                Err(stopping) => format!("{error}; stop/drain: {stopping}"),
            };
            let error = anchors::retain(&loaded, &driver, Some(error))
                .expect("incoming preparation failure is retained");
            let error = topology::retain(source_output.as_ref(), &driver, Some(error))
                .expect("incoming preparation failure is retained");
            drop(prediction);
            let pending = cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms);
            Err(if pending == 0 { error } else {
                format!("{error}; {pending} original helper children remain unconfirmed")
            })
        }
    }
}

/// Unsupported stronger contracts are refused before creating an owner. The
/// later release must still select the exact retained recipe and campaign guard.
pub(super) fn check_selection(config: &Config, loaded: &Loaded, selected: Selection)
    -> Result<(), String>
{
    checked::check_selection(config, loaded)?;
    recipe::check_creation(config, loaded)?;
    selected.check(config, loaded)?;
    if loaded.generation.required_owned_pre_output_forecast().is_none()
        || !loaded.generation.requires_owned_forecast_stream_restart_audit()
        || loaded.generation.text_stream_profile().is_none() {
        return Err("prompt preparation requires the original owned prompt-audit stream recipe".into());
    }
    if loaded.identity.is_some() || loaded.generation.requires_computed_identity()
        || loaded.publication.is_some() || loaded.generation.required_publication_validation().is_some()
        || loaded.credential.is_some() || loaded.generation.required_credential_policy().is_some()
        || loaded.generation.required_held_out_joint().is_some() {
        return Err("prompt preparation does not support identity, checked publication, credential or held-out activation selections".into());
    }
    if loaded.recovery_anchor.is_some() || loaded.floor.journal_revision != 0
        || loaded.floor.control_sequence != 0 || loaded.floor.authority_epoch != 0 {
        return Err("prompt preparation creates a fresh history and cannot adopt recovery floors or custody".into());
    }
    if loaded.anchor_output.is_none() { return Err(USAGE.into()); }
    if loaded.topology.as_ref().is_some_and(|selected|
        !matches!(selected, topology::Selection::Create(_))) {
        return Err("prompt preparation requires intentional topology creation inputs".into());
    }
    Ok(())
}

struct Prepared {
    checkpoint: FileLearnedCheckpointInfo,
    snapshot: FileDeliverySnapshot,
    numerical: HostedLearnedInspection,
    usage: HostedLearnedRecoveryUsage,
    anchor: FileHistoryAnchor,
    topology: Option<Result<TopologyCheckpoint, String>>,
}
impl Prepared {
    fn capture(driver: &FileSupervisedDriver, selected: Selection, source_output: Option<&Output>)
        -> Result<Self, String> {
        let host = driver.supervisor().host().map_err(debug)?;
        let mut acknowledged = Self::from_host(&host, selected)?;
        acknowledged.topology = source_output.map(|output| {
            let saved = output.capture(&host, driver.topology())?;
            if saved.history_anchor() != &acknowledged.anchor {
                return Err("prepared source and history describe different cuts".into());
            }
            Ok(saved)
        });
        Ok(acknowledged)
    }

    fn from_host(host: &FileOversight, selected: Selection) -> Result<Self, String> {
        let checkpoint = *host.learned_stream_checkpoint(selected.checkpoint_id()).map_err(debug)?.info();
        Ok(Self { checkpoint, snapshot: host.inspect(),
            numerical: host.learned_generation_inspection().map_err(debug)?.numerical,
            usage: host.learned_recovery_usage().map_err(debug)?,
            anchor: host.history_anchor().map_err(debug)?, topology: None })
    }

    fn finish<F>(&self, loaded: &Loaded, deadline: &Deadline, time: &mut F,
        topology_output: Option<&Output>, output: &mut impl Write) -> Result<(), String>
    where F: FnMut() -> ElapsedTick {
        let mut failures = Vec::new();
        let late = deadline.check(time()).is_err();
        if late { failures.push("original preparation deadline reached after acknowledgment".into()); }
        let retained = anchors::export_acknowledged(&self.anchor,
            loaded.anchor_output.as_deref().ok_or(USAGE)?);
        if let Err(error) = &retained { failures.push(format!("history anchor retention failed: {error}")); }
        // Both immutable captures were taken before dropping the original owner.
        // Export cannot refresh, reread or acquire a second source allowance.
        let source_retained = match (topology_output, self.topology.as_ref()) {
            (Some(output), Some(Ok(saved))) => Some(output.retain_acknowledged(saved)),
            (Some(_), Some(Err(error))) => Some(Err(error.clone())),
            (None, None) => None,
            _ => Some(Err("prepared result is missing selected source custody".into())),
        };
        if let Some(Err(error)) = &source_retained { failures.push(error.clone()); }
        if let Err(error) = self.report(retained.is_ok(), source_retained.as_ref().map(Result::is_ok),
            late, output) { failures.push(error); }
        if !late && deadline.check(time()).is_err() {
            failures.push("original preparation deadline reached during acknowledged output".into());
        }
        if failures.is_empty() { Ok(()) } else { Err(format!(
            "prompt checkpoint {} acknowledged at revision {}: {}; preserve the original store and any successfully retained anchor; fresh preparation cannot recreate it",
            self.checkpoint.checkpoint, self.snapshot.revision, failures.join("; "))) }
    }

    fn report(&self, retained: bool, source_retained: Option<bool>, late: bool,
        output: &mut impl Write) -> Result<(), String> {
        let cut = self.checkpoint;
        let state = &self.snapshot;
        let source_retained = source_retained.map_or("null", |retained| if retained { "true" } else { "false" });
        let bytes = format!(concat!(
            "{{\"schema\":\"fa.learned-stream-preparation-result/1\",\"outcome\":\"prepared\",",
            "\"checkpoint\":\"{}\",\"revision\":\"{}\",\"actor_revision\":\"{}\",",
            "\"authority_epoch\":\"{}\",\"control_sequence\":\"{}\",\"stream\":\"{}\",",
            "\"position\":\"{}\",\"sampled_draws\":\"{}\",\"sampling_attempts\":\"{}\",",
            "\"executions\":\"{}\",\"charged\":\"{}\",\"reserved\":\"{}\",",
            "\"checkpoints\":\"{}\",\"restart_attempts\":\"{}\",",
            "\"history_anchor_retained\":{},\"topology_checkpoint_retained\":{},\"deadline_reached\":{},\"effect_authority\":\"not_issued\"}}\n"),
            cut.checkpoint, state.revision, cut.actor_revision, cut.authority_epoch,
            cut.control_sequence, cut.stream, cut.position, cut.sampled_draws,
            self.numerical.cumulative_work.sampling_attempts, state.executions,
            state.control.ledger.charged, state.control.ledger.reserved,
            self.usage.checkpoints, self.usage.restart_attempts, retained, source_retained, late);
        output.write_all(bytes.as_bytes()).and_then(|()| output.flush()).map_err(debug)
    }
}
