//! Recovery of one exact credential operation from an older complete source
//! checkpoint. The native finalizer exposes only restricted maintenance custody.
use super::super::credential_lifecycle::{Recovered, Request};
use super::cooperative::{Progress, finish_replay};
use super::{Config, FileOversight, Loaded, RecoveryReserve, debug, requirements};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::{
    FileMediatedRequirements,
    credential::{FileCredentialMaintenanceOwner, FileCredentialMaintenanceRequest},
    predictive::{FileMediatedPredictor, FilePredictiveMediatedRequirements},
};

pub(in super::super) fn open(
    config: &Config,
    loaded: &Loaded,
    request: Request,
    checkpoint: &mut impl FnMut(Option<Progress>) -> Result<(), String>,
) -> Result<Recovered<FileCredentialMaintenanceOwner>, String> {
    super::super::credential::check_selection(loaded)?;
    if loaded.credential.is_none() || loaded.anchor_output.is_none() {
        return Err("credential checkpoint recovery requires an independent credential selection and new retained outputs".into());
    }
    let history = super::super::topology::history(loaded)?
        .ok_or("credential checkpoint recovery requires an explicit topology profile")?;
    let saved = history
        .checkpoint()
        .ok_or("credential checkpoint recovery requires an exact v2 source checkpoint")?;
    let anchor = loaded
        .recovery_anchor
        .as_ref()
        .ok_or("credential checkpoint recovery requires an independent history anchor")?;
    if saved.history_anchor() != anchor {
        return Err("source checkpoint and history anchor describe different cuts".into());
    }
    let request = match request {
        Request::Rotate(request) => FileCredentialMaintenanceRequest::Rotate(request),
        Request::Revoke(request) => FileCredentialMaintenanceRequest::Revoke(request),
    };
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
    checkpoint(None)?;
    let run = match predictor {
        Some(prediction) => {
            let expected = FilePredictiveMediatedRequirements {
                oversight: requirements(config, loaded),
                topology: history.requirement.clone(),
                prediction,
                evaluation: None,
            };
            FileOversight::begin_open_predictive_mediated_guarded_anchored_with_learned_generation(
                &config.store,
                config.profile.clone(),
                &expected,
                &loaded.generation,
                anchor,
            )
            .and_then(|run| run.require_recovery_reserve(RecoveryReserve::terminal()))
            .and_then(|run| run.require_credential_checkpoint(saved, request))
        }
        None => {
            let expected = FileMediatedRequirements {
                oversight: requirements(config, loaded),
                topology: history.requirement.clone(),
                prediction: None,
                evaluation: None,
            };
            FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
                &config.store,
                config.profile.clone(),
                &expected,
                &loaded.generation,
                anchor,
            )
            .and_then(|run| run.require_recovery_reserve(RecoveryReserve::terminal()))
            .and_then(|run| run.require_credential_checkpoint(saved, request))
        }
    }
    .map_err(debug)?;
    let run = finish_replay(
        run,
        checkpoint,
        |run| run.progress(),
        |run, done| run.advance(done, 1),
        Ok,
    )?;
    let current = run
        .credential_status()
        .map_err(debug)?
        .ok_or("credential checkpoint recovery requires the original credential guard")?;
    let retry = run.check_request().map_err(debug)?;
    checkpoint(Some(run.progress()))?;
    let host = run.finish_maintenance().map_err(debug)?;
    Ok(Recovered {
        host,
        current,
        retry,
    })
}
