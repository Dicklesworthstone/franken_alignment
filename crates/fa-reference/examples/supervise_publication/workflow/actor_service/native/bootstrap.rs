//! One source-backed bootstrap for every native lifecycle, including receipts.
use super::{Config, FileOversight, FileHumanReviewer, Inputs, PublicationProfile,
    RecoveryReserve, debug};
use fa_reference::action::consequence::delivery::persistent::observed::stream::generated::checked::source::{
    GeneratedSourceProfile, GeneratedSourcePublication, evaluation::FileGeneratedEvaluationConfig,
};
use fa_reference::action::consequence::delivery::persistent::observed::{
    credibility::FileIndependentEvaluator,
    governance::campaigns::FilePolicyGovernor, guarded::FileCampaignRequirement,
};

#[path = "bootstrap/recovery.rs"]
mod recovery;
#[cfg(test)]
pub(super) use recovery::open_with_checkpoints;
pub(super) use recovery::{open_with_history, open_with_retained_history};

pub(super) fn selection(
    config: &Config,
    inputs: &Inputs,
    publication: Option<&PublicationProfile>,
) -> Result<GeneratedSourceProfile, String> {
    let reserve = Some(RecoveryReserve::terminal());
    match publication {
        Some(publication) => Ok(publication
            .native_selection(&config.profile, inputs.stream, reserve)
            .map_err(debug)?
            .with_source(config.source_policy)),
        None => Ok(GeneratedSourceProfile {
            source: config.source_policy,
            publication: GeneratedSourcePublication::Stream {
                stream: inputs.stream,
                reserve,
            },
        }),
    }
}

pub(super) fn prepare(
    config: &Config,
    inputs: &Inputs,
    selected: GeneratedSourceProfile,
    open: bool,
) -> Result<(FileOversight, FileHumanReviewer), String> {
    // Source pinning occurs before recovery confirmation, staging cleanup and
    // authority fencing, even for receipt-only service. No evidence file opens.
    if open {
        FileOversight::open_generated_text_stream_from_source(
            &config.store,
            config.profile.clone(),
            &inputs.decoder,
            &inputs.tokenizer,
            selected,
        )
    } else {
        FileOversight::create_generated_text_stream_from_source(
            &config.store,
            config.profile.clone(),
            inputs.decoder.clone(),
            inputs.tokenizer.clone(),
            selected,
        )
    }
    .map_err(debug)
}

/// The original complete first image returns the governor only after storage
/// acknowledgment. It does not install a guard into an existing owner.
pub(super) fn prepare_governed(
    config: &Config,
    inputs: &Inputs,
    selected: GeneratedSourceProfile,
    campaigns: FileCampaignRequirement,
) -> Result<(FileOversight, FileHumanReviewer, FilePolicyGovernor), String> {
    FileOversight::create_generated_text_stream_from_source_governed(
        &config.store,
        config.profile.clone(),
        inputs.decoder.clone(),
        inputs.tokenizer.clone(),
        selected,
        campaigns,
    )
    .map_err(debug)
}

/// Native generation, source provenance, the terminal reserve and independent
/// accounting are one original first image. No post-proposal enable or second
/// writer is used to obtain the evaluator role.
pub(super) fn prepare_andon(
    config: &Config,
    inputs: &Inputs,
    selected: GeneratedSourceProfile,
    settings: &super::super::andon_settings::Settings,
) -> Result<(FileOversight, FileHumanReviewer, FileIndependentEvaluator), String> {
    settings.creation()?;
    if settings.joint.is_some() {
        return Err("native live Andon creation requires the marginal evaluation profile".into());
    }
    let evaluation = FileGeneratedEvaluationConfig::new(settings.protocol.clone())
        .with_andon(settings.config.clone())
        .map_err(debug)?;
    let (host, roles) = FileOversight::create_generated_text_stream_from_source_evaluated(
        &config.store,
        config.profile.clone(),
        inputs.decoder.clone(),
        inputs.tokenizer.clone(),
        selected,
        evaluation,
    )
    .map_err(debug)?;
    Ok((host, roles.oversight.human, roles.evaluator))
}
