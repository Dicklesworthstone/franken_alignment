//! Accounted resident suffix recovery on the original native Ready cursor.
//! Only custody adapts here: the shared driver owns bounded numerical replay,
//! and the resident retention owner exports its acknowledged final cut once.
use super::super::evaluation::recovery as accounted;
use super::super::{
    Config, Control, Deadline, ElapsedTick, FileHumanReviewer, FileOversight, Inputs, PeerProfile,
    Profile, debug, operator,
};
use super::{anchors, andon};
use fa_reference::action::consequence::delivery::persistent::observed::{
    credibility::FileIndependentEvaluator,
    governance::campaigns::FilePolicyGovernor,
    guarded::{FileRecoveryRequirements, anchored::FileHistoryAnchor},
    stream::generated::checked::source::GeneratedSourceProfile,
};
use std::path::{Path, PathBuf};

/// An independently held complete counter-floor document. Its canonical name
/// remains reserved against every scheduled input and future endpoint.
pub(in super::super) struct Recovery {
    path: PathBuf,
    requirements: accounted::Requirements,
}

impl Recovery {
    pub(super) fn read(path: &Path, config: &Config) -> Result<Self, String> {
        let canonical = operator::resolve(path)?;
        check_custody(&canonical, config)?;
        let requirements = accounted::Requirements::read(path)?;
        Ok(Self {
            path: canonical,
            requirements,
        })
    }

    pub(super) fn check_configuration(&self, config: &Config) -> Result<(), String> {
        check_custody(&self.path, config)
    }

    pub(super) fn check_files(&self, paths: &[&Path]) -> Result<(), String> {
        for path in paths {
            if overlaps(&self.path, &operator::resolve(path)?) {
                return Err("native evaluation recovery requirements overlap a selected input or future service endpoint".into());
            }
        }
        Ok(())
    }

    pub(super) fn check_actors(
        &self,
        actors: &[&Profile],
        reviewer: &PeerProfile,
        selected_operator: Option<&operator::Selection>,
        evaluator: &crate::workflow::actor_service::evaluator::Settings,
    ) -> Result<(), String> {
        let mut endpoints = vec![evaluator.socket_path().to_path_buf()];
        for actor in actors {
            endpoints.extend([
                actor.socket.clone(),
                reviewer.socket(actor.request),
                crate::workflow::control::socket_path(reviewer, actor.request),
            ]);
        }
        if let Some(operator) = selected_operator {
            endpoints.push(operator.settings.socket.clone());
        }
        self.check_files(&endpoints.iter().map(PathBuf::as_path).collect::<Vec<_>>())
    }

    pub(super) fn check_history(&self, history: &anchors::Selection) -> Result<(), String> {
        history.check_endpoint(&self.path)
    }

    pub(super) fn open<F>(
        &self,
        config: &Config,
        inputs: &Inputs,
        selected: GeneratedSourceProfile,
        accounting: andon::Selection<'_>,
        governed: Option<&FileRecoveryRequirements>,
        history: &anchors::Selection,
        control: &mut Control,
        deadline: &Deadline,
        time: &mut F,
        stopped: &mut Option<FileHistoryAnchor>,
    ) -> Result<(
        FileOversight, FileHumanReviewer, Option<FilePolicyGovernor>, FileIndependentEvaluator,
    ), String>
    where
        F: FnMut() -> ElapsedTick,
    {
        let expected =
            self.requirements
                .expected_selected(accounting.settings, config, inputs, &selected, governed)?;
        let anchor = history.input().ok_or(andon::INVALID)?;
        let (recovery, pending) = accounted::ready(
            config,
            inputs,
            selected,
            &expected,
            &accounting.settings.protocol,
            anchor,
            control,
            deadline,
            time,
        )?;
        if pending {
            let target = recovery
                .into_recovery_stop(control.recovery_operation()?)
                .map_err(debug)?;
            return Err(control.stop_recovery_with_anchor(target, deadline, time, stopped)?);
        }
        let (host, roles) = recovery.finish().map_err(debug)?;
        Ok((host, roles.oversight.human, roles.oversight.policy_governor, roles.evaluator))
    }
}

fn overlaps(left: &Path, right: &Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

fn check_custody(path: &Path, config: &Config) -> Result<(), String> {
    if overlaps(path, &operator::resolve(&config.store)?)
        || overlaps(path, &operator::resolve(config.source.path())?)
    {
        return Err("native evaluation recovery requirements need independent custody outside the authority store and evidence source".into());
    }
    Ok(())
}
