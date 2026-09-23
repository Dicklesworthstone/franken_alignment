//! Same exact text/history pin composed with the ORIGINAL higher guard validators.
//! No evaluator, predictor or topology requirement is converted to a base guard.

use super::*;
use super::super::super::{
    evaluation::{self, FileEvaluatedOversightRoles},
    predictive::{self, FilePredictiveRequirements, FilePredictiveRoles},
    mediated::{self, FileMediatedRequirements, FileMediatedRoles},
};
use crate::action::consequence::oversight::credibility::EvaluationProtocol;

impl FileOversight {
    /// Exact text/history recovery with independently evaluated helper custody.
    /// Reuse the original evaluation-protocol and complete base-guard validator;
    /// an anchor never substitutes for the evaluator's denominator or labels.
    /// Old evaluator tickets/roles and effect approvals remain withdrawn. This
    /// is the existing marginal-evaluation profile, not a joint-policy fallback.
    pub fn open_evaluated_guarded_text_anchored(directory: impl AsRef<Path>, profile: FileOversightProfile,
        expected: &FileRecoveryRequirements, protocol: &EvaluationProtocol,
        tokenizer: &ByteBpe, anchor: &FileHistoryAnchor)
        -> Result<(Self, FileEvaluatedOversightRoles), JournalError>
    {
        profile.delivery.limits.check()?;
        let canonical = tokenizer_bytes(expected, tokenizer)?;
        let store = storage::Store::open(directory.as_ref())?;
        Self::open_evaluated_text_store(store, profile, expected, protocol, &canonical, anchor)
    }

    pub(super) fn open_evaluated_text_store(store: storage::Store, profile: FileOversightProfile,
        expected: &FileRecoveryRequirements, protocol: &EvaluationProtocol,
        canonical: &[u8], anchor: &FileHistoryAnchor)
        -> Result<(Self, FileEvaluatedOversightRoles), JournalError>
    {
        let config = expected.guards.decoder.as_ref().ok_or(Error::Incomplete)?;
        let (host, human) = open_text_store(store, profile, config, canonical, anchor,
            |profile, events| evaluation::checked_events(profile, events, expected, protocol))?;
        let oversight = FileOversightRoles::provision(&host, human);
        let roles = FileEvaluatedOversightRoles::provision(&host, oversight);
        Ok((host, roles))
    }

    /// Preserve exact prediction coefficients, calibration, categories, lifetime
    /// bounds and optional evaluation presence in anchored text recovery. The
    /// native fence retains prediction evidence and marks unanswered coverage
    /// lost; a newly issued observer is not an old forecast or a fresh source.
    /// All text cursor/work and base-role rules of open_guarded_text_anchored apply.
    pub fn open_predictive_guarded_text_anchored(directory: impl AsRef<Path>, profile: FileOversightProfile,
        expected: &FilePredictiveRequirements, tokenizer: &ByteBpe, anchor: &FileHistoryAnchor)
        -> Result<(Self, FilePredictiveRoles), JournalError>
    {
        profile.delivery.limits.check()?;
        let canonical = tokenizer_bytes(&expected.oversight, tokenizer)?;
        let store = storage::Store::open(directory.as_ref())?;
        Self::open_predictive_text_store(store, profile, expected, &canonical, anchor)
    }

    pub(super) fn open_predictive_text_store(store: storage::Store, profile: FileOversightProfile,
        expected: &FilePredictiveRequirements, canonical: &[u8], anchor: &FileHistoryAnchor)
        -> Result<(Self, FilePredictiveRoles), JournalError>
    {
        let config = expected.oversight.guards.decoder.as_ref().ok_or(Error::Incomplete)?;
        let (host, human) = open_text_store(store, profile, config, canonical, anchor,
            |profile, events| predictive::checked_events(profile, events, expected))?;
        let oversight = FileOversightRoles::provision(&host, human);
        let roles = FilePredictiveRoles::provision(&host, oversight, expected.evaluation.is_some());
        Ok((host, roles))
    }

    /// Recover all separately held topology/prediction/evaluation/base roles
    /// without dropping the exact text or anti-rollback requirement. The original
    /// validator matches BOTH initial/current topology and pre-fence availability,
    /// plus exact optional prediction/evaluation presence, before the one fence.
    ///
    /// Recovery withdraws the old topology certificate and availability. A new
    /// observer must supply a newer topology and obtain a native cut before new
    /// effect admission. A matching anchor does not resurrect any certificate,
    /// credential, forecast, saved identity observation or approved publication.
    pub fn open_mediated_guarded_text_anchored(directory: impl AsRef<Path>, profile: FileOversightProfile,
        expected: &FileMediatedRequirements, tokenizer: &ByteBpe, anchor: &FileHistoryAnchor)
        -> Result<(Self, FileMediatedRoles), JournalError>
    {
        profile.delivery.limits.check()?;
        let canonical = tokenizer_bytes(&expected.oversight, tokenizer)?;
        let store = storage::Store::open(directory.as_ref())?;
        Self::open_mediated_text_store(store, profile, expected, &canonical, anchor)
    }

    pub(super) fn open_mediated_text_store(store: storage::Store, profile: FileOversightProfile,
        expected: &FileMediatedRequirements, canonical: &[u8], anchor: &FileHistoryAnchor)
        -> Result<(Self, FileMediatedRoles), JournalError>
    {
        let config = expected.oversight.guards.decoder.as_ref().ok_or(Error::Incomplete)?;
        let (host, human) = open_text_store(store, profile, config, canonical, anchor,
            |profile, events| mediated::checked_events(profile, events, expected))?;
        let oversight = FileOversightRoles::provision(&host, human);
        let roles = FileMediatedRoles::provision(&host, oversight,
            expected.prediction.is_some(), expected.evaluation.is_some());
        Ok((host, roles))
    }
}
