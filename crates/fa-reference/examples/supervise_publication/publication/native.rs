//! Project the existing explicit witness profile into the atomic native owner.
//! Parsing and receipt recovery never open a capture, producer or feed file.
use super::{PublicationProfile, FileOversightProfile, JournalError, Error};
use fa_reference::action::consequence::delivery::persistent::RecoveryReserve;
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanReviewer};
use fa_reference::action::consequence::delivery::persistent::observed::decoder::FileDecoderConfig;
use fa_reference::action::consequence::delivery::persistent::observed::credibility::held_out_joint::HeldOutJointPolicy;
use fa_reference::action::consequence::delivery::persistent::observed::stream::generated::checked::{
    GeneratedPublicationProfile, GeneratedPublicationFeed,
};
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::ByteBpe;
use fa_reference::action::consequence::delivery::stream::StreamProfile;
use std::path::Path;

// The executable cannot accidentally project a joint selection to its weaker
// witness-only fields. Both creation and recovery dispatch the complete choice
// to the existing atomic library constructors; no policy is installed later.
pub(crate) struct NativePublicationSelection {
    publication: GeneratedPublicationProfile,
    joint: Option<HeldOutJointPolicy>,
}
impl NativePublicationSelection {
    pub(crate) fn create(self, directory: &Path, profile: FileOversightProfile,
        decoder: FileDecoderConfig, tokenizer: ByteBpe)
        -> Result<(FileOversight, FileHumanReviewer), JournalError>
    {
        match self.joint {
            Some(joint) => FileOversight::create_generated_text_stream_with_joint_publication(
                directory, profile, decoder, tokenizer, self.publication, joint),
            None => FileOversight::create_generated_text_stream_checked(
                directory, profile, decoder, tokenizer, self.publication),
        }
    }
    pub(crate) fn open(self, directory: &Path, profile: FileOversightProfile,
        decoder: &FileDecoderConfig, tokenizer: &ByteBpe)
        -> Result<(FileOversight, FileHumanReviewer), JournalError>
    {
        match self.joint {
            Some(joint) => FileOversight::open_generated_text_stream_with_joint_publication(
                directory, profile, decoder, tokenizer, self.publication, joint),
            None => {
                let result = FileOversight::open_generated_text_stream_checked(
                    directory, profile, decoder, tokenizer, self.publication)?;
                // The generic opener replays every stored guard and has already
                // fenced. Do not return its stronger owner under an omitted
                // joint selection. No stored guard is disabled by this refusal.
                if result.0.held_out_joint_policy()?.is_some() { return Err(Error::Binding.into()); }
                Ok(result)
            }
        }
    }
}

impl PublicationProfile {
    // Retain the original expressly non-joint projection for existing callers.
    // Its refusal cannot be changed into a silent loss of the joint requirement.
    pub(crate) fn generated_profile(&self, profile: &FileOversightProfile, stream: StreamProfile,
        reserve: Option<RecoveryReserve>) -> Result<GeneratedPublicationProfile, JournalError>
    {
        self.check_scope(profile)?;
        if self.joint.is_some() { return Err(Error::Binding.into()); }
        self.generated_fields(stream, reserve)
    }

    pub(crate) fn native_selection(&self, profile: &FileOversightProfile, stream: StreamProfile,
        reserve: Option<RecoveryReserve>) -> Result<NativePublicationSelection, JournalError>
    {
        let publication = if self.joint.is_some() {
            self.check_scope(profile)?;
            self.generated_fields(stream, reserve)?
        } else { self.generated_profile(profile, stream, reserve)? };
        Ok(NativePublicationSelection { publication, joint: self.joint })
    }

    fn generated_fields(&self, stream: StreamProfile, reserve: Option<RecoveryReserve>)
        -> Result<GeneratedPublicationProfile, JournalError>
    {
        if self.snapshot_fallback && self.feed.is_none() { return Err(Error::Binding.into()); }
        Ok(GeneratedPublicationProfile { stream, reserve, limits: self.limits,
            feed: self.feed.as_ref().map(|feed| GeneratedPublicationFeed {
                changes: feed.changes, freshness: feed.freshness, snapshot_fallback: self.snapshot_fallback,
            }) })
    }
}
