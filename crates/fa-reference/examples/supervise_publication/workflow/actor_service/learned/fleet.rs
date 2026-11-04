//! Learned fleet intake reuses independently provisioned numerical and source custody.
//! The shared fleet command retains the original coordinator, roster and phases.
use super::{
    Config, Duration, ElapsedTick, Instant, Loaded, RecoveryReserve, debug, fields, recipe,
    recovery,
};
use crate::fleet_shutdown::LearnedTopologyPaths;
use crate::workflow::continuation::guarded::mediated::profile::{History, Selection};
use fa_reference::action::consequence::delivery::persistent::observed::{
    guarded::{
        anchored::{FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES},
        mediated::{
            FileMediatedRequirements,
            predictive::{FileMediatedPredictor, FilePredictiveMediatedRequirements},
        },
        predictive::{FilePredictiveRequirements, owned::FileOwnedPredictiveRequirements},
    },
    shutdown::coordinator::{FileShutdownLearnedInputs, FileShutdownLearnedRequirements},
};
use std::{
    io::{Cursor, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) fn command(args: &[String], output: &mut impl Write) -> Result<(), String> {
    let started = Instant::now();
    command_with(
        args,
        || {
            let duration = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| format!("Unix clock unavailable: {error}"))?;
            Ok(ElapsedTick(
                u64::try_from(duration.as_millis())
                    .map_err(|_| "Unix clock overflow".to_owned())?,
            ))
        },
        &mut || started.elapsed(),
        output,
    )
}

/// No live helpers, reviewer sockets, current policy file, generation continuation
/// or effect retry is provisioned. The original full guarded recovery is opened
/// by the native coordinator only after its durable member intent.
pub(super) fn command_with(
    args: &[String],
    clock: impl FnMut() -> Result<ElapsedTick, String>,
    elapsed: &mut impl FnMut() -> Duration,
    output: &mut impl Write,
) -> Result<(), String> {
    crate::fleet_shutdown::command_with_learned(args, clock, elapsed, &mut load, output)
}

fn load(
    config: &Config,
    file: &Path,
    topology: Option<&LearnedTopologyPaths>,
    remaining: &mut usize,
    forbidden: &[PathBuf],
    checkpoint: &mut dyn FnMut() -> Result<(), String>,
) -> Result<FileShutdownLearnedInputs, String> {
    let mut loaded = recipe::load_for_fleet(file, config, remaining, forbidden)?;
    let (requirements, topology_checkpoint) = if let Some(paths) = topology {
        let history =
            historical_topology(config, &mut loaded, paths, remaining, forbidden, checkpoint)?;
        let saved = history
            .checkpoint()
            .cloned()
            .ok_or("fleet topology requires an exact v2 source checkpoint")?;
        let predictor = loaded
            .generation
            .required_pre_output_forecast()
            .map(|config| FileMediatedPredictor::Raw(config.clone()))
            .or_else(|| {
                loaded
                    .generation
                    .required_owned_pre_output_forecast()
                    .map(|config| FileMediatedPredictor::Owned(config.clone()))
            });
        let requirements = match predictor {
            Some(prediction) => FileShutdownLearnedRequirements::PredictiveMediated(
                FilePredictiveMediatedRequirements {
                    oversight: recovery::requirements(config, &loaded),
                    topology: history.requirement.clone(),
                    prediction,
                    evaluation: None,
                },
            ),
            None => FileShutdownLearnedRequirements::Mediated(FileMediatedRequirements {
                oversight: recovery::requirements(config, &loaded),
                topology: history.requirement.clone(),
                prediction: None,
                evaluation: None,
            }),
        };
        (requirements, Some(saved))
    } else {
        let requirements = if let Some(prediction) =
            loaded.generation.required_pre_output_forecast()
        {
            FileShutdownLearnedRequirements::Raw(FilePredictiveRequirements {
                oversight: recovery::requirements(config, &loaded),
                prediction: prediction.clone(),
                evaluation: None,
            })
        } else if let Some(prediction) = loaded.generation.required_owned_pre_output_forecast() {
            FileShutdownLearnedRequirements::Owned(FileOwnedPredictiveRequirements {
                oversight: recovery::requirements(config, &loaded),
                prediction: prediction.clone(),
                evaluation: None,
            })
        } else {
            FileShutdownLearnedRequirements::Plain(recovery::requirements(config, &loaded))
        };
        (requirements, None)
    };
    Ok(FileShutdownLearnedInputs {
        generation: loaded.generation,
        requirements,
        reserve: RecoveryReserve::terminal(),
        topology_checkpoint,
    })
}

/// The source anchor is supplied independently of both the checkpoint and the
/// immutable fleet registration. The native cursor later proves the complete
/// registered prefix and permits only terminal successors of this exact cut.
fn historical_topology(
    config: &Config,
    loaded: &mut Loaded,
    paths: &LearnedTopologyPaths,
    remaining: &mut usize,
    forbidden: &[PathBuf],
    checkpoint: &mut dyn FnMut() -> Result<(), String>,
) -> Result<History, String> {
    let mut assets = fields::Assets::for_fleet(*remaining, forbidden)?;
    let result = (|| {
        checkpoint()?;
        let store = fields::resolve_root(&config.store)?;
        let bytes = assets.read(&paths.history_anchor, MAX_HISTORY_ANCHOR_BYTES)?;
        let anchor =
            FileHistoryAnchor::read_trusted(&mut Cursor::new(bytes), MAX_HISTORY_ANCHOR_BYTES)
                .map_err(debug)?;
        checkpoint()?;
        let selection = Selection::read_with_reader(&paths.profile, &mut |path, maximum| {
            assets.read(path, maximum)
        })?;
        checkpoint()?;
        if !selection.checkpoint_selected() {
            return Err("fleet topology requires an exact v2 source checkpoint".into());
        }
        if selection.retention_requested() {
            return Err(
                "historical fleet recovery cannot export a fresh topology-source checkpoint".into(),
            );
        }
        let history =
            selection.history_with_reader(config, &store, checkpoint, &mut |path, maximum| {
                assets.read(path, maximum)
            })?;
        loaded.floor = history.minimum(&anchor, loaded.floor)?;
        Ok(history)
    })();
    *remaining = remaining.saturating_sub(assets.consumed());
    result
}
