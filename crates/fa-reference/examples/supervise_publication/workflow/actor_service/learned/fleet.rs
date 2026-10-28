//! Learned fleet intake is the existing independently provisioned recipe.
//! The shared fleet command retains the original coordinator, roster and phases.
use super::{Config, Duration, ElapsedTick, Instant, RecoveryReserve, recipe, recovery};
use fa_reference::action::consequence::delivery::persistent::observed::{
    guarded::predictive::{FilePredictiveRequirements, owned::FileOwnedPredictiveRequirements},
    shutdown::coordinator::{FileShutdownLearnedInputs, FileShutdownLearnedRequirements},
};
use std::{
    io::Write,
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
    remaining: &mut usize,
    forbidden: &[PathBuf],
) -> Result<FileShutdownLearnedInputs, String> {
    let loaded = recipe::load_for_fleet(file, config, remaining, forbidden)?;
    let requirements = if let Some(prediction) = loaded.generation.required_pre_output_forecast() {
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
    Ok(FileShutdownLearnedInputs {
        generation: loaded.generation,
        requirements,
        reserve: RecoveryReserve::terminal(),
        topology_checkpoint: None,
    })
}
