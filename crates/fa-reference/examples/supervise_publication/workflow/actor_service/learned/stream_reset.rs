//! Complete one independently specified original stream-reset intent before its
//! recovery Fence. No new numerical checkpoint, reset intent, clock observation or publication is issued.
use super::{
    Config, Control, Deadline, Duration, ElapsedTick, FileOversight, Instant, Loaded,
    PeerProfile, anchors, checked, debug, fields, plus, recipe, recovery, topology,
};
use crate::config::read_regular;
use crate::workflow::continuation::checkpoint::AcknowledgedOutput;
use fa_reference::action::consequence::delivery::persistent::observed::{
    decoder::learned::checkpoint::FileLearnedResetIntent,
    driver::topology::checkpoint::TopologyCheckpoint,
    shutdown::recovery::FileAdmissionStoppedHistory,
};
use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[path = "stream_reset/request.rs"]
mod request;
#[path = "stream_reset/terminal.rs"]
pub(super) mod terminal;

const USAGE: &str = "create-learned-generated CONFIG LEARNED_RECIPE --complete-stream-reset RESET_REQUEST --history-anchor INPUT --retain-history-anchor NEW_OUTPUT [--topology-profile TOPOLOGY_PROFILE] [--stop-profile PEER_PROFILE]";
const REQUEST_BYTES: usize = 262_144;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Request {
    pub(super) intent: FileLearnedResetIntent,
    pub(super) first_incident_sequence: u64,
}

pub(super) fn decode(bytes: &[u8]) -> Result<Request, String> {
    request::decode(bytes)
}

/// Validate the whole grammar before stripping any option. Unsupported live
/// and predictive selections cannot be reinterpreted as reset data.
fn arguments(args: &[String]) -> Result<(Vec<String>, Option<PathBuf>), String> {
    if args.len() < 9
        || args.first().map(String::as_str) != Some("create-learned-generated")
        || args.get(3).map(String::as_str) != Some("--complete-stream-reset")
        || args[1..3]
            .iter()
            .any(|value| value.is_empty() || value.starts_with("--"))
    {
        return Err(USAGE.into());
    }
    fields::path(&args[1])?;
    fields::path(&args[2])?;
    fields::path(&args[4])?;
    let mut seen = BTreeSet::new();
    let mut remaining = args[..5].to_vec();
    let mut stop_profile = None;
    let mut at = 5;
    while at < args.len() {
        let flag = args[at].as_str();
        if !matches!(
            flag,
            "--history-anchor" | "--retain-history-anchor" | "--stop-profile" | "--topology-profile"
        ) || !seen.insert(flag)
        {
            return Err(USAGE.into());
        }
        let value = args.get(at + 1).ok_or(USAGE)?;
        if value.is_empty() || value.starts_with("--") {
            return Err(format!("{flag} requires an immediate non-option path"));
        }
        let path = fields::path(value)?;
        if flag == "--stop-profile" {
            stop_profile = Some(path);
        } else {
            remaining.extend([args[at].clone(), value.clone()]);
        }
        at += 2;
    }
    if !seen.contains("--history-anchor") || !seen.contains("--retain-history-anchor") {
        return Err(USAGE.into());
    }
    Ok((remaining, stop_profile))
}

/// The independent expectation is bounded data outside the mutable authority
/// store. It is never filled from the journal or the current pending operation.
fn read_request(config: &Config, path: &Path) -> Result<Request, String> {
    let store = fs::canonicalize(&config.store).map_err(debug)?;
    let selected = fs::canonicalize(path).map_err(debug)?;
    if selected.starts_with(store) {
        return Err("stream-reset expectation must be retained outside the authority store".into());
    }
    decode(&read_regular(path, REQUEST_BYTES)?)
}

pub(super) fn command<F>(
    args: &[String],
    credibility: Option<&Path>,
    mut time: F,
    output: &mut impl Write,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    let started = Instant::now();
    if credibility.is_some() {
        return Err("stream-reset recovery cannot activate credibility".into());
    }
    let (args, stop_profile) = arguments(args)?;
    let (args, topology_profile) = topology::take_option(&args)?;
    let (args, options) = anchors::take_options(&args)?;
    let start = time();
    let config = Config::read_learned_stream_reset(Path::new(&args[1]))?;
    let deadline = Deadline {
        logical: plus(start, config.timing.runtime_ms)?,
        started,
        wall: Duration::from_millis(config.timing.runtime_ms),
    };
    deadline.check(time())?;
    let request = read_request(&config, Path::new(&args[4]))?;
    deadline.check(time())?;
    // Historical selection loads original models and complete guard contracts,
    // without reading current helper evidence or provisioning live helpers.
    let mut loaded = recipe::load(Path::new(&args[2]), &config, false)?;
    checked::check_selection(&config, &loaded)?;
    deadline.check(time())?;
    options.admit(&config, &mut loaded, true)?;
    // Admit historical source custody plus its output destination. This does
    // not invoke selection.next, bind a source, or read any next-source files.
    topology::admit(&config, &mut loaded, topology_profile.as_deref(), topology::Mode::Live)?;
    check_selection(&loaded)?;
    deadline.check(time())?;
    let peers = stop_profile.as_deref().map(PeerProfile::read).transpose()?;
    if let Some(peers) = peers.as_ref() {
        peers.check_host(&config)?;
    }
    deadline.check(time())?;
    let mut control = Control::new(&config, request.intent.control().operation, peers.as_ref())?;
    run(
        &config,
        &loaded,
        &request,
        &mut control,
        &deadline,
        &mut time,
        output,
    )
}

/// Topology requires exact paired source/history custody. Predictive histories
/// retain their original native refusal and cannot downgrade to plain recovery.
fn check_selection(loaded: &Loaded) -> Result<(), String> {
    if loaded.generation.required_pre_output_forecast().is_some()
        || loaded
            .generation
            .required_owned_pre_output_forecast()
            .is_some()
    {
        return Err("stream-reset recovery does not support predictive histories".into());
    }
    if loaded.generation.text_stream_profile().is_none() {
        return Err("stream-reset recovery requires the original learned text stream".into());
    }
    let anchor = loaded.recovery_anchor.as_ref()
        .ok_or("stream-reset recovery requires an independent input history anchor and a new retained output")?;
    if loaded.anchor_output.is_none() {
        return Err("stream-reset recovery requires an independent input history anchor and a new retained output".into());
    }
    if let Some(selected) = loaded.topology.as_ref() {
        let topology::Selection::Recover { selection, history } = selected else {
            return Err("topology creation inputs cannot complete a historical stream reset".into());
        };
        let saved = history.checkpoint()
            .ok_or("stream-reset recovery requires an exact v2 topology checkpoint; manual source minima cannot establish custody")?;
        if saved.history_anchor() != anchor {
            return Err("stream-reset source checkpoint and input history anchor describe different cuts".into());
        }
        if selection.acknowledged_output().is_none() {
            return Err("stream-reset topology recovery requires a new retained topology checkpoint output".into());
        }
    }
    Ok(())
}

/// The returned owner stays inside this bounded recovery operation. Its reset
/// result is historical evidence; it is not a resumed actor or an effect key.
pub(super) fn run<F>(
    config: &Config,
    loaded: &Loaded,
    request: &Request,
    control: &mut Control,
    deadline: &Deadline,
    time: &mut F,
    output: &mut impl Write,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    checked::check_selection(config, loaded)?;
    check_selection(loaded)?;
    let source = SourceRetention::admit(config, loaded)?;
    let host = match source.as_ref() {
        Some(source) => recovery::stream_reset::complete(
            config, loaded, request, source, control, deadline, time),
        None => recovery::complete_stream_reset_live(
            config, loaded, &request.intent, request.first_incident_sequence,
            control, deadline, time,
        ).map(|((host, _human, _forecast, _identity), _topology)| host),
    }.map_err(|error| format!(
        "native stream-reset completion is unconfirmed: {error}; inspect the original history before choosing a retry"
    ))?;

    // Native completion and Fence are already acknowledged. Retention and a
    // truthful report are both attempted even when time or output later fails.
    let mut failures = Vec::new();
    let retained = retain(&host, loaded);
    if let Err(error) = &retained {
        failures.push(format!("history anchor retention failed: {error}"));
    }
    // Each archive describes this same acknowledged cut. Failure of either
    // export cannot suppress the other or justify repeating the reset audit.
    let source_retained = source.as_ref().map(|source| source.retain(&host, request));
    if let Some(Err(error)) = &source_retained {
        failures.push(format!("topology checkpoint retention failed: {error}"));
    }
    let late = deadline.check(time()).is_err();
    if late {
        failures.push("original recovery deadline reached after acknowledgment".into());
    }
    let outcome = host
        .learned_reset_result(request.intent.control().operation)
        .map_err(debug);
    let restored = match &outcome {
        Ok(Ok(receipt)) => {
            if !receipt.control.restored {
                failures.push(
                    "original reset acknowledged containment without numerical restoration".into(),
                );
            }
            receipt.control.restored
        }
        Ok(Err(error)) => {
            failures.push(format!("original reset acknowledged refusal: {error:?}"));
            false
        }
        Err(error) => {
            failures.push(format!(
                "acknowledged reset result inspection failed: {error}"
            ));
            false
        }
    };
    let status = match &outcome {
        Ok(Ok(_)) if restored => "restored",
        Ok(Ok(_)) => "contained",
        Ok(Err(_)) => "refused",
        Err(_) => "unavailable",
    };
    let refunded = match &outcome {
        Ok(Ok(receipt)) => format!("\"{}\"", receipt.control.refunded_units),
        _ => "null".into(),
    };
    if let Err(error) = report(
        &host,
        request,
        status,
        restored,
        &refunded,
        retained.is_ok(),
        source_retained.as_ref().map(Result::is_ok),
        late,
        output,
    ) {
        failures.push(format!("acknowledged reset report failed: {error}"));
    }
    // A slow or blocked report cannot extend the same original budget. Its
    // acknowledged bytes are never rewritten or followed by another reset.
    if deadline.check(time()).is_err() && !late {
        failures.push("original recovery deadline reached during acknowledged output".into());
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "native stream reset acknowledged at revision {}: {}; preserve the original expectation and paired input custody; retry only that exact expectation with new output paths",
            host.revision(),
            failures.join("; ")
        ))
    }
}

fn retain(host: &FileOversight, loaded: &Loaded) -> Result<(), String> {
    let previous = loaded
        .recovery_anchor
        .as_ref()
        .ok_or("missing independent input anchor")?;
    let path = loaded
        .anchor_output
        .as_deref()
        .ok_or("missing new retained anchor output")?;
    let anchor = host.history_anchor_after(previous).map_err(debug)?;
    anchors::export_acknowledged(&anchor, path)
}

fn report(
    host: &FileOversight,
    request: &Request,
    status: &str,
    restored: bool,
    refunded: &str,
    retained: bool,
    source_retained: Option<bool>,
    late: bool,
    output: &mut impl Write,
) -> Result<(), String> {
    let state = host.inspect();
    let generation = host.learned_generation_inspection().map_err(debug)?;
    let usage = host.learned_recovery_usage().map_err(debug)?;
    let actor = host.actor_snapshot().map_err(debug)?;
    let source_retained = source_retained.map_or("null", |retained| if retained { "true" } else { "false" });
    // Only fixed labels and bounded original numeric observations reach stdout.
    // Counters are decimal strings, preserving the entire u64 range.
    let text = format!(
        concat!(
            "{{\"schema\":\"fa.learned-stream-reset-result/1\",\"acknowledged\":true,",
            "\"outcome\":\"{}\",\"restored\":{},\"operation\":\"{}\",",
            "\"checkpoint\":\"{}\",\"first_incident_sequence\":\"{}\",\"revision\":\"{}\",",
            "\"control_sequence\":\"{}\",\"authority_epoch\":\"{}\",\"actor_revision\":\"{}\",",
            "\"position\":\"{}\",\"sampled_draws\":\"{}\",\"paused\":{},\"incident_count\":\"{}\",",
            "\"charged\":\"{}\",\"reserved\":\"{}\",\"refunded_units\":{},\"executions\":\"{}\",",
            "\"restart_attempts\":\"{}\",\"reserved_cache_values\":\"{}\",\"values_restored\":\"{}\",",
            "\"history_anchor_retained\":{},\"topology_checkpoint_retained\":{},",
            "\"deadline_reached\":{},\"authority\":\"none\"}}\n"
        ),
        status,
        restored,
        request.intent.control().operation,
        request.intent.checkpoint(),
        request.first_incident_sequence,
        state.revision,
        state.control.sequence,
        state.control.ledger.epoch,
        generation.numerical.actor_revision,
        generation.numerical.position,
        generation.numerical.sampled_draws,
        generation.paused,
        actor.incident_count,
        state.control.ledger.charged,
        state.control.ledger.reserved,
        refunded,
        state.executions,
        usage.restart_attempts,
        usage.reserved_cache_values,
        usage.values_restored,
        retained,
        source_retained,
        late,
    );
    output
        .write_all(text.as_bytes())
        .and_then(|()| output.flush())
        .map_err(debug)
}

/// Independent output destinations are admitted before replay. Only an exact
/// native successor can advance this retained source work; next-source files and
/// live observers are never used for reset completion or terminal retention.
pub(super) struct SourceRetention<'a> {
    pub(super) saved: &'a TopologyCheckpoint,
    archive: AcknowledgedOutput,
    identity: PathBuf,
}
impl<'a> SourceRetention<'a> {
    fn admit(config: &Config, loaded: &'a Loaded) -> Result<Option<Self>, String> {
        let Some(topology::Selection::Recover { selection, history }) = loaded.topology.as_ref() else {
            return Ok(None);
        };
        let saved = history.checkpoint().ok_or("missing exact stream-reset source checkpoint")?;
        let path = selection.acknowledged_output().ok_or("missing new stream-reset source output")?;
        let archive = AcknowledgedOutput::admit(config, path)?;
        if loaded.anchor_output.as_deref() == Some(archive.path()) {
            return Err("history anchor and topology checkpoint outputs must be distinct new files".into());
        }
        let identity = fs::canonicalize(&config.store).map_err(debug)?;
        Ok(Some(Self { saved, archive, identity }))
    }

    fn retain(&self, host: &FileOversight, request: &Request) -> Result<(), String> {
        let successor = self.saved.after_learned_stream_reset(host,
            &request.intent, request.first_incident_sequence).map_err(debug)?;
        self.archive.retain(&successor)
    }

    pub(super) fn retain_stop(&self, config: &Config, loaded: &Loaded,
        stopped: &FileAdmissionStoppedHistory, request: &Request,
    ) -> Result<(), String> {
        let history = loaded.anchor_output.as_deref().ok_or_else(|| "missing new retained history output".to_owned())
            .and_then(|path| anchors::export_acknowledged(stopped.history_anchor(), path))
            .map_err(|error| format!("history anchor retention failed: {error}"));
        let topology = self.saved.after_stream_reset_admission_stop(&config.profile,
            &self.identity, stopped, &request.intent, request.first_incident_sequence)
            .map_err(debug).and_then(|saved| self.archive.retain(&saved))
            .map_err(|error| format!("terminal topology checkpoint retention failed: {error}"));
        let failures: Vec<_> = [history, topology].into_iter().filter_map(Result::err).collect();
        if failures.is_empty() { Ok(()) } else {
            Err(format!(
                "{}; preserve previous paired custody, canonical stopped history, and the Stop receipt; use --recover-stopped-stream-reset with the exact reset request and recorded Stop operation, paired inputs and new output paths",
                failures.join("; ")))
        }
    }
}
