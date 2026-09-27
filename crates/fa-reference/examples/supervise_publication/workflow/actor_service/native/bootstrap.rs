//! One source-backed bootstrap for every native lifecycle, including receipts.
use super::{Config, FileOversight, FileHumanReviewer, Inputs, PublicationProfile,
    RecoveryReserve, debug};
use fa_reference::action::consequence::delivery::persistent::observed::stream::generated::checked::source::{
    GeneratedSourceProfile, GeneratedSourcePublication,
};

pub(super) fn selection(config: &Config, inputs: &Inputs, publication: Option<&PublicationProfile>)
    -> Result<GeneratedSourceProfile, String>
{
    let reserve = Some(RecoveryReserve::terminal());
    match publication {
        Some(publication) => Ok(publication.native_selection(&config.profile, inputs.stream, reserve)
            .map_err(debug)?.with_source(config.source_policy)),
        None => Ok(GeneratedSourceProfile { source: config.source_policy,
            publication: GeneratedSourcePublication::Stream { stream: inputs.stream, reserve } }),
    }
}

pub(super) fn prepare(config: &Config, inputs: &Inputs, selected: GeneratedSourceProfile, open: bool)
    -> Result<(FileOversight, FileHumanReviewer), String>
{
    // Source pinning occurs before recovery confirmation, staging cleanup and
    // authority fencing, even for receipt-only service. No evidence file opens.
    if open {
        FileOversight::open_generated_text_stream_from_source(&config.store, config.profile.clone(),
            &inputs.decoder, &inputs.tokenizer, selected)
    } else {
        FileOversight::create_generated_text_stream_from_source(&config.store, config.profile.clone(),
            inputs.decoder.clone(), inputs.tokenizer.clone(), selected)
    }.map_err(debug)
}
