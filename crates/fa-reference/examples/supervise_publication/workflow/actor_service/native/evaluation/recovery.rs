//! Complete independently selected accounting over the original native cursor.
//! This driver supplies no replay state and cannot expose a mutable candidate.
//! Its Ready checkpoint precedes the sole Fence or authenticated terminal Stop.
use super::{Selection, Config, FileHumanReviewer, FileIndependentEvaluator, FileOversight,
    GeneratedSourceProfile, Inputs, anchors, andon_settings, debug};
use super::super::{Control, Deadline, ElapsedTick};
use crate::config::read_regular;
use fa_reference::action::consequence::delivery::persistent::observed::{
    guarded::{
        FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
        anchored::FileHistoryAnchor,
        evaluation::andon::{
            FileAndonRecoveryRequirements, generated::FileAndonGeneratedSourceRecovery,
        },
    },
    stream::generated::checked::source::{
        FileGeneratedSourceRecoveryStatus, GeneratedSourcePublication,
    },
};
use fa_reference::action::consequence::oversight::credibility::EvaluationProtocol;
use fa_reference::strict_json::{self, Limits};
use std::path::Path;

const MAX_BYTES: usize = 4096;

/// These three floors are operator inputs, never inferred from the journal.
/// The original config and recipe independently pin the complete supported guard
/// inventory and unchanged policy; the Andon profile pins all current windows,
/// evaluation floor, optional joint policy and the original accounting identity.
pub(in super::super) struct Requirements {
    minimum: FileRecoveryFloor,
}

impl Requirements {
    pub(in super::super) fn read(path: &Path) -> Result<Self, String> {
        Self::decode(&read_regular(path, MAX_BYTES)?)
    }

    fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut fields = andon_settings::object(strict_json::parse(
            bytes,
            Limits {
                max_bytes: MAX_BYTES,
                max_depth: 3,
                max_items: 16,
                max_string_bytes: 128,
            },
        ).map_err(debug)?)?;
        if andon_settings::take(&mut fields, "schema")?.as_str()
            != Some("fa.native-evaluation-recovery/1")
        {
            return Err("unsupported native evaluation recovery requirements schema".into());
        }
        let mut minimum = andon_settings::object(andon_settings::take(&mut fields, "minimum")?)?;
        let minimum_value = FileRecoveryFloor {
            journal_revision: andon_settings::number(&mut minimum, "journal_revision")?,
            control_sequence: andon_settings::number(&mut minimum, "control_sequence")?,
            authority_epoch: andon_settings::number(&mut minimum, "authority_epoch")?,
        };
        andon_settings::end(minimum)?;
        andon_settings::end(fields)?;
        Ok(Self { minimum: minimum_value })
    }

    /// Single and resident services select the same complete native inventory.
    /// The caller cannot replace an absent guard with whatever was found on disk.
    pub(in super::super) fn expected(
        &self,
        settings: &andon_settings::Settings,
        config: &Config,
        inputs: &Inputs,
        selected: &GeneratedSourceProfile,
    ) -> Result<FileAndonRecoveryRequirements, String> {
        let mut expected = FileAndonRecoveryRequirements::new(
            FileRecoveryRequirements {
                guards: FileGuardSet {
                    stream: Some(inputs.stream),
                    decoder: Some(inputs.decoder.clone()),
                    decoder_stop: None,
                    source: Some(config.source_policy),
                    identity: None,
                    campaigns: None,
                    credential: None,
                },
                effective_policy: config.profile.delivery.policy.clone(),
                credential_epoch: None,
                minimum: self.minimum,
            },
            settings.config.clone(),
            settings.windows.clone(),
            settings.minimum_evaluation_revision,
        ).map_err(debug)?;
        if let Some(policy) = settings.joint {
            expected = expected.require_joint(policy).map_err(debug)?;
        }
        match &selected.publication {
            GeneratedSourcePublication::Stream { .. } => {}
            GeneratedSourcePublication::Checked(publication)
            | GeneratedSourcePublication::Joint { publication, .. } => {
                expected = expected.require_publication(publication.limits).map_err(debug)?;
            }
        }
        Ok(expected)
    }
}

/// Drive the SAME sealed numerical/accounting cursor to Ready, without cleanup,
/// a Fence or live roles. Both service forms retain one deadline and Stop quota
/// through every original event and the final Ready boundary. The caller must
/// consume pending Stop through the original terminal path before normal finish.
pub(in super::super) fn ready<F>(
    config: &Config,
    inputs: &Inputs,
    selected: GeneratedSourceProfile,
    expected: &FileAndonRecoveryRequirements,
    protocol: &EvaluationProtocol,
    anchor: &FileHistoryAnchor,
    control: &mut Control,
    deadline: &Deadline,
    time: &mut F,
) -> Result<(FileAndonGeneratedSourceRecovery, bool), String>
where
    F: FnMut() -> ElapsedTick,
{
    deadline.check(time())?;
    let mut pending = control.recovery_pending_peer()?;
    let mut recovery =
        FileOversight::begin_open_andon_guarded_generated_text_stream_from_source_anchored(
            &config.store,
            config.profile.clone(),
            expected,
            &inputs.decoder,
            &inputs.tokenizer,
            selected,
            protocol,
            anchor,
        )
        .map_err(debug)?;
    let total = recovery.progress().total_events;
    loop {
        let progress = recovery.progress();
        if progress.total_events != total
            || progress.replayed_events > total
            || (progress.status == FileGeneratedSourceRecoveryStatus::Ready
                && progress.replayed_events != total)
        {
            return Err("invalid original evaluated native recovery progress".into());
        }
        match progress.status {
            FileGeneratedSourceRecoveryStatus::Failed(error) => return Err(debug(error)),
            FileGeneratedSourceRecoveryStatus::Interrupted
            | FileGeneratedSourceRecoveryStatus::Cancelled => {
                return Err("evaluated native recovery interrupted before live custody".into());
            }
            FileGeneratedSourceRecoveryStatus::Replaying
            | FileGeneratedSourceRecoveryStatus::Ready => {}
        }
        deadline.check(time())?;
        pending |= control.recovery_pending_peer()?;
        if progress.status == FileGeneratedSourceRecoveryStatus::Ready {
            return Ok((recovery, pending));
        }
        let next = recovery
            .advance(progress.replayed_events, 1)
            .map_err(debug)?;
        if next != recovery.progress()
            || next.total_events != total
            || next.replayed_events < progress.replayed_events
            || next.replayed_events > progress.replayed_events.saturating_add(1)
            || (next.replayed_events == progress.replayed_events
                && next.status == FileGeneratedSourceRecoveryStatus::Replaying)
        {
            return Err("evaluated native recovery exceeded its single-event quantum".into());
        }
    }
}

impl Selection {
    pub(in super::super) fn recovery_requirements(
        &self,
        config: &Config,
        inputs: &Inputs,
        selected: &GeneratedSourceProfile,
    ) -> Result<FileAndonRecoveryRequirements, String> {
        self.recovery.as_ref().ok_or(super::USAGE)?.expected(
            &self.settings,
            config,
            inputs,
            selected,
        )
    }

    /// The same locked original source replay checks the independent anchor,
    /// numerical work, complete guards, accounting and all floors before any
    /// cleanup or role issue. Stop remains prior to this sole finalizer.
    pub(in super::super) fn open<F>(
        &self,
        config: &Config,
        inputs: &Inputs,
        selected: GeneratedSourceProfile,
        history: &anchors::Selection,
        control: &mut Control,
        deadline: &Deadline,
        time: &mut F,
    ) -> Result<(FileOversight, FileHumanReviewer, FileIndependentEvaluator), String>
    where
        F: FnMut() -> ElapsedTick,
    {
        if self.evaluate_only {
            return Err("historical native evaluation requires the evaluation-only service".into());
        }
        let expected = self.recovery_requirements(config, inputs, &selected)?;
        let anchor = history.input().ok_or(super::USAGE)?;
        let (recovery, pending) = ready(
            config,
            inputs,
            selected,
            &expected,
            &self.settings.protocol,
            anchor,
            control,
            deadline,
            time,
        )?;
        if pending {
            let target = recovery
                .into_recovery_stop(control.recovery_operation()?)
                .map_err(debug)?;
            return Err(control.stop_recovery_with_history(
                target,
                Some(history),
                deadline,
                time,
            )?);
        }
        let (host, roles) = recovery.finish().map_err(debug)?;
        Ok((host, roles.oversight.human, roles.evaluator))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOCUMENT: &str = r#"{"schema":"fa.native-evaluation-recovery/1","minimum":{"journal_revision":17,"control_sequence":3,"authority_epoch":1}}"#;

    #[test]
    fn recovery_floors_are_complete_explicit_and_not_silently_defaulted() {
        let selected = Requirements::decode(DOCUMENT.as_bytes()).unwrap();
        assert_eq!(selected.minimum, FileRecoveryFloor {
            journal_revision: 17, control_sequence: 3, authority_epoch: 1,
        });
        for changed in [
            DOCUMENT.replace("\"authority_epoch\":1", "\"authority_epoch\":-1"),
            DOCUMENT.replace(",\"authority_epoch\":1", ""),
            DOCUMENT.replace("\"control_sequence\":3", "\"control_sequence\":3,\"control_sequence\":0"),
            DOCUMENT.replace("\"journal_revision\":17", "\"journal_revision\":17,\"extra\":0"),
            DOCUMENT.replace("\"minimum\":", "\"extra\":0,\"minimum\":"),
            DOCUMENT.replace("fa.native-evaluation-recovery/1", "fa.native-evaluation-recovery/2"),
        ] {
            assert!(Requirements::decode(changed.as_bytes()).is_err(), "{changed}");
        }
    }
}
