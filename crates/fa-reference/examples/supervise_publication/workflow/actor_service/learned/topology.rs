//! Select the original topology custodian for the original learned workflow.
//! No graph parser, cut checker, publication executor or recovery law is copied.
use super::{Config, Loaded, FileOversight, FileSupervisedDriver, Control, Deadline,
    ElapsedTick, RecoveryReserve, debug, fields, recovery};
use crate::workflow::continuation::{checkpoint::Output,
    creation::topology::Selection as Creation,
    guarded::mediated::profile::{Selection as Continuation, History}};
use fa_reference::action::consequence::delivery::persistent::{JournalError,
    observed::{driver::topology::FileDriverTopology,
        guarded::mediated::predictive::FileMediatedPredictor}};
use fa_reference::action::consequence::mediation::CutCheck;
use fa_reference::Error;
use std::path::{Path, PathBuf};

pub(super) enum Selection {
    Create(Creation),
    Recover { selection: Continuation, history: History },
}

#[derive(Clone, Copy)]
pub(super) enum Mode { Create, Live, Historical }

const FLAG: &str = "--topology-profile";
const INVALID: &str = "learned topology requires one explicit --topology-profile PATH after the command inputs";

/// Remove one complete path-valued option; the original mode grammar still owns
/// every remaining flag. A missing path never consumes a neighboring option.
pub(super) fn take_option(args: &[String]) -> Result<(Vec<String>, Option<PathBuf>), String> {
    let mut remaining = Vec::new();
    let mut selected = None;
    let mut at = 0;
    while at < args.len() {
        if args[at] == FLAG {
            if at < 4 || selected.is_some() { return Err(INVALID.into()); }
            let value = args.get(at + 1).ok_or(INVALID)?;
            selected = Some(fields::path(value)?);
            at += 2;
        } else {
            remaining.push(args[at].clone());
            at += 1;
        }
    }
    Ok((remaining, selected))
}

/// Contracts are independently selected before opening the authority owner.
/// Recovery loads only historical expectations here, never the next live files.
pub(super) fn admit(config: &Config, loaded: &mut Loaded, path: Option<&Path>, mode: Mode)
    -> Result<(), String>
{
    let Some(path) = path else { return Ok(()); };
    if loaded.topology.is_some() { return Err("learned topology is already selected".into()); }
    let selected = match mode {
        Mode::Create => Selection::Create(Creation::read(path)?),
        Mode::Live | Mode::Historical => {
            let anchor = loaded.recovery_anchor.as_ref()
                .ok_or("learned topology recovery requires an independent history anchor")?;
            let selection = Continuation::read(path)?;
            if matches!(mode, Mode::Historical) && selection.retention_requested() {
                return Err("historical recovery cannot export a fresh topology-source checkpoint".into());
            }
            let history = selection.history(config, &mut || Ok(()))?;
            loaded.floor = history.minimum(anchor, loaded.floor)?;
            Selection::Recover { selection, history }
        }
    };
    loaded.topology = Some(selected);
    Ok(())
}

pub(super) fn history(loaded: &Loaded) -> Result<Option<&History>, String> {
    match loaded.topology.as_ref() {
        None => Ok(None),
        Some(Selection::Recover { history, .. }) => Ok(Some(history)),
        Some(Selection::Create(_)) => Err("topology creation inputs cannot reopen a learned history".into()),
    }
}

pub(super) struct Started {
    pub custody: recovery::Custody,
    pub source: Option<FileDriverTopology>,
    pub output: Option<Output>,
}

/// Atomic native startup or one fully guarded original recovery. Topology and
/// predictor custody stay separate; a recovered graph is not a fresh cut.
pub(super) fn start<F>(config: &Config, loaded: &Loaded, continuing: bool,
    control: &mut Control, deadline: &Deadline, time: &mut F) -> Result<Started, String>
where F: FnMut() -> ElapsedTick {
    let Some(selected) = loaded.topology.as_ref() else {
        let custody = if continuing {
            recovery::open_selected_live(config, loaded, control, deadline, time)?
        } else { recovery::create_selected(config, loaded)? };
        return Ok(Started { custody, source: None, output: None });
    };
    match selected {
        Selection::Create(selection) => {
            if continuing { return Err("topology creation cannot replace a recovery contract".into()); }
            let mut checkpoint = || {
                deadline.check(time())?;
                if control.pending_peer()? {
                    return Err("learned topology intake interrupted by authenticated stop peer; no durable Stop applied or acknowledged".into());
                }
                Ok(())
            };
            let mut prepared = selection.clone().prepare(config, &mut checkpoint)?;
            let output = prepared.take_output();
            let graph = prepared.loaded().graph().clone();
            let guards = recovery::requirements(config, loaded).guards;
            let predictor = loaded.generation.required_pre_output_forecast()
                .map(|config| FileMediatedPredictor::Raw(config.clone()))
                .or_else(|| loaded.generation.required_owned_pre_output_forecast()
                    .map(|config| FileMediatedPredictor::Owned(config.clone())));
            let (host, roles) = match predictor {
                Some(predictor) => FileOversight::create_predictive_mediated_guarded_with_learned_generation_reserved(
                    &config.store, config.profile.clone(), &guards, None, graph, predictor,
                    None, loaded.generation.clone(), RecoveryReserve::terminal()),
                None => FileOversight::create_mediated_guarded_with_learned_generation_reserved(
                    &config.store, config.profile.clone(), &guards, None, graph,
                    None, loaded.generation.clone(), RecoveryReserve::terminal()),
            }.map_err(debug)?;
            let source = prepared.bind(&host, roles.topology_observer)?;
            Ok(Started { custody: (host, roles.oversight.human, roles.consistency_observer),
                source: Some(source), output })
        }
        Selection::Recover { selection, history } => {
            if !continuing { return Err("topology recovery inputs cannot create a learned history".into()); }
            let ((mut host, human, forecast), observer) = recovery::open_selected_live_with_topology(
                config, loaded, control, deadline, time)?;
            match host.request_status(loaded.request) {
                Err(JournalError::Contract(Error::Missing)) => {}
                Ok(_) => return Err("learned request already exists; use --resume for its original receipt".into()),
                Err(error) => return Err(debug(error)),
            }
            let mut checkpoint = || {
                deadline.check(time())?;
                if control.pending_peer()? {
                    return Err("learned topology recovery interrupted before live source binding; no durable Stop applied or acknowledged".into());
                }
                Ok(())
            };
            let next = selection.next(config, history.clone(), &mut checkpoint)?;
            let observer = observer.ok_or("recovered topology observer custody is missing")?;
            let (source, output) = next.bind(&mut host, observer)?;
            Ok(Started { custody: (host, human, forecast), source: Some(source), output })
        }
    }
}

/// Install before any fallible preparation so selected custody cannot disappear.
/// The actual original cut is obtained separately after fresh qualification.
pub(super) fn attach(driver: &mut FileSupervisedDriver, source: Option<FileDriverTopology>)
    -> Result<(), String>
{
    if let Some(source) = source {
        let revision = driver.supervisor().host().map_err(debug)?.revision();
        driver.require_topology(revision, source).map_err(debug)?;
    }
    Ok(())
}

pub(super) fn certify<F>(driver: &mut FileSupervisedDriver, deadline: &Deadline, time: &mut F)
    -> Result<(), String>
where F: FnMut() -> ElapsedTick {
    if driver.topology().is_none() { return Ok(()); }
    let before = time(); deadline.check(before)?;
    {
        let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
        let revision = host.revision();
        host.observe_time(revision, before).map_err(debug)?;
    }
    let revision = driver.supervisor().host().map_err(debug)?.revision();
    let result = driver.refresh_topology(revision).map_err(debug)?;
    let after = time(); deadline.check(after)?;
    {
        let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
        let revision = host.revision();
        host.observe_time(revision, after).map_err(debug)?;
    }
    if result.refusal.is_some() || !matches!(result.checked, Some(Ok(CutCheck::Verified(_)))) {
        return Err("initial learned topology was not certified by the native checker".into());
    }
    Ok(())
}

/// A failed service cannot mint a healthy source checkpoint. Output failure
/// remains separate from the original effect and never requests another send.
pub(super) fn retain(output: Option<&Output>, driver: &FileSupervisedDriver,
    failure: Option<String>) -> Option<String>
{
    if failure.is_some() { return failure; }
    let Some(output) = output else { return None; };
    let result = (|| {
        let host = driver.supervisor().host().map_err(debug)?;
        output.retain(&host, driver.topology())
    })();
    result.err()
}
