//! Single-request evaluated startup retains the original topology source and
//! independently selected history/source archives. No ordinary recovery fallback
//! may discard accounting, joint policy, source work or topology custody.
use super::super::{startup_custody::FenceRetention, topology};
use super::{
    Config, Control, Deadline, ElapsedTick, FileIndependentEvaluator, FileOversight, Loaded,
    RecoveryReserve, Selection, debug, learned_recovery,
};
use crate::workflow::continuation::{
    checkpoint::AcknowledgedOutput,
    creation::topology::Selection as Creation,
    guarded::mediated::profile::{History, Selection as Continuation},
};
use fa_reference::action::consequence::delivery::persistent::observed::{
    driver::topology::checkpoint::TopologyCheckpoint,
    guarded::evaluation::andon::learned::FileMediatedAndonLearnedRecovery,
    mediation::FileMediationObserver,
};

pub(super) fn check_selection(loaded: &Loaded, continuing: bool) -> Result<(), String> {
    match loaded.topology.as_ref() {
        None => Ok(()),
        Some(topology::Selection::Create(selection)) => {
            if continuing || selection.acknowledged_output().is_none() {
                return Err(
                    "evaluated learned topology creation requires a new paired source checkpoint"
                        .into(),
                );
            }
            Ok(())
        }
        Some(topology::Selection::Recover { selection, history }) => {
            if !continuing || selection.acknowledged_output().is_none() {
                return Err(
                    "evaluated learned topology recovery requires a new paired source checkpoint"
                        .into(),
                );
            }
            let saved = history.checkpoint().ok_or(
                "evaluated learned topology recovery requires an exact v2 source checkpoint",
            )?;
            if Some(saved.history_anchor()) != loaded.recovery_anchor.as_ref() {
                return Err(
                    "evaluated source checkpoint and history anchor describe different cuts".into(),
                );
            }
            Ok(())
        }
    }
}

impl Selection {
    /// Keep the source, selected checkpoint output and independently issued
    /// evaluator beside the SAME original owner until driver attachment.
    pub(in super::super) fn start<F>(
        &self,
        config: &Config,
        loaded: &Loaded,
        continuing: bool,
        control: &mut Control,
        deadline: &Deadline,
        time: &mut F,
    ) -> Result<(topology::Started, FileIndependentEvaluator), String>
    where
        F: FnMut() -> ElapsedTick,
    {
        check_selection(loaded, continuing)?;
        match loaded.topology.as_ref() {
            None => {
                let (custody, evaluator) = if continuing {
                    self.open(config, loaded, control, deadline, time)?
                } else {
                    self.create(config, loaded)?
                };
                Ok((
                    topology::Started {
                        custody,
                        source: None,
                        output: None,
                    },
                    evaluator,
                ))
            }
            Some(topology::Selection::Create(selection)) => {
                self.create_mediated(config, loaded, selection, control, deadline, time)
            }
            Some(topology::Selection::Recover { selection, history }) => {
                self.open_mediated(config, loaded, selection, history, control, deadline, time)
            }
        }
    }

    fn create_mediated<F>(
        &self,
        config: &Config,
        loaded: &Loaded,
        selection: &Creation,
        control: &mut Control,
        deadline: &Deadline,
        time: &mut F,
    ) -> Result<(topology::Started, FileIndependentEvaluator), String>
    where
        F: FnMut() -> ElapsedTick,
    {
        self.settings.creation()?;
        super::super::recipe::check_creation(config, loaded)?;
        super::super::credential::check_creation(loaded)?;
        let registration = super::super::credential::registration(loaded)?;
        let mut checkpoint = || {
            deadline.check(time())?;
            if control.pending_peer()? {
                return Err("evaluated learned topology intake interrupted by authenticated stop peer; no durable Stop applied or acknowledged".into());
            }
            Ok(())
        };
        let mut prepared = selection.clone().prepare(config, &mut checkpoint)?;
        let output = prepared.take_output();
        let graph = prepared.loaded().graph().clone();
        let guards = learned_recovery::requirements(config, loaded).guards;
        let (host, roles) = match self.settings.joint {
            Some(joint) => {
                FileOversight::create_joint_mediated_andon_guarded_with_learned_generation_reserved(
                    &config.store,
                    config.profile.clone(),
                    &guards,
                    registration,
                    graph,
                    self.settings.protocol.clone(),
                    joint,
                    &self.settings.config,
                    loaded.generation.clone(),
                    RecoveryReserve::terminal(),
                )
            }
            None => FileOversight::create_mediated_andon_guarded_with_learned_generation_reserved(
                &config.store,
                config.profile.clone(),
                &guards,
                registration,
                graph,
                self.settings.protocol.clone(),
                &self.settings.config,
                loaded.generation.clone(),
                RecoveryReserve::terminal(),
            ),
        }
        .map_err(debug)?;
        let evaluator = roles.evaluator.ok_or(
            "native evaluated topology startup did not provision its independent evaluator",
        )?;
        let source = prepared.bind(&host, roles.topology_observer)?;
        Ok((
            topology::Started {
                custody: (
                    host,
                    roles.oversight.human,
                    roles.consistency_observer,
                    roles.oversight.identity_observer,
                ),
                source: Some(source),
                output,
            },
            evaluator,
        ))
    }

    fn open_mediated<F>(
        &self,
        config: &Config,
        loaded: &Loaded,
        selection: &Continuation,
        history: &History,
        control: &mut Control,
        deadline: &Deadline,
        time: &mut F,
    ) -> Result<(topology::Started, FileIndependentEvaluator), String>
    where
        F: FnMut() -> ElapsedTick,
    {
        // Source destinations are admitted before the sole Fence. A later
        // missing live document must leave the new acknowledged cut recoverable.
        let mut retention = FenceRetention::admit(config, loaded, history, selection)?;
        let recovery =
            self.replay_mediated(config, loaded, history, control, deadline, time, None)?;
        let (mut host, roles) = recovery.finish().map_err(debug)?;
        retention
            .capture(&host, Some(&roles.topology_observer))
            .map_err(|error| retention.retain_error(&host, error))?;
        let evaluator = roles.evaluator.ok_or_else(|| {
            retention.retain_error(
                &host,
                "native evaluated topology recovery did not provision its independent evaluator"
                    .into(),
            )
        })?;
        let (source, output) = topology::bind_recovered_source(
            config,
            loaded,
            selection,
            history,
            &mut host,
            Some(roles.topology_observer),
            control,
            deadline,
            time,
        )
        .map_err(|error| retention.retain_error(&host, error))?;
        Ok((
            topology::Started {
                custody: (
                    host,
                    roles.oversight.human,
                    roles.consistency_observer,
                    roles.oversight.identity_observer,
                ),
                source: Some(source),
                output,
            },
            evaluator,
        ))
    }

    fn replay_mediated<F>(
        &self,
        config: &Config,
        loaded: &Loaded,
        history: &History,
        control: &mut Control,
        deadline: &Deadline,
        time: &mut F,
        historical: Option<Option<u64>>,
    ) -> Result<FileMediatedAndonLearnedRecovery, String>
    where
        F: FnMut() -> ElapsedTick,
    {
        let expected = self.recovery_requirements(config, loaded)?;
        let anchor = loaded.recovery_anchor.as_ref().ok_or(
            "evaluated learned topology requires an independently retained history anchor",
        )?;
        let mut pending = false;
        let recovery =
            {
                let mut checkpoint = |_| {
                    deadline.check(time())?;
                    pending |= control.recovery_pending_peer()?;
                    Ok(())
                };
                checkpoint(None)?;
                let recovery =
                FileOversight::begin_open_mediated_andon_guarded_anchored_with_learned_generation(
                    &config.store, config.profile.clone(), &expected, &history.requirement,
                    &self.settings.protocol, &loaded.generation, anchor,
                )
                .map_err(debug)?
                .require_recovery_reserve(RecoveryReserve::terminal())
                .map_err(debug)?;
                // An older source-work cut cannot cover an uncheckpointed suffix.
                // This check precedes event zero, staging cleanup and role release.
                history.check_extent(recovery.progress().total_events)?;
                learned_recovery::finish_replay(
                    recovery,
                    &mut checkpoint,
                    FileMediatedAndonLearnedRecovery::progress,
                    |recovery, done| recovery.advance(done, 1),
                    Ok,
                )?
            };
        if !pending {
            if let Some(finish_after) = historical {
                match finish_after {
                    Some(after) => recovery
                        .learned_text_stream_finish_request(after, loaded.request)
                        .map_err(debug)?,
                    None => recovery
                        .learned_text_stream_message_request(loaded.request)
                        .map_err(debug)?,
                };
                // Selection can traverse the original request book. Stop and
                // the finite deadline retain priority before the sole Fence.
                deadline.check(time())?;
                pending = control.recovery_pending_peer()?;
            }
        }
        if pending {
            let retained = learned_recovery::StopRetention::admit(config, loaded)?;
            let target = recovery
                .into_recovery_stop(control.recovery_operation()?)
                .map_err(debug)?;
            let outcome = control.stop_recovery(target, deadline, time)?;
            let mut terminal = outcome.terminal_message();
            if let Ok(history) = &outcome.application.result {
                if let Err(error) = retained.retain(config, history) {
                    terminal.push_str("; ");
                    terminal.push_str(&error);
                }
            }
            return Err(terminal);
        }
        Ok(recovery)
    }

    /// Historical assessment retains the original unbound topology observer.
    /// It never loads next-source documents, creates a source or certifies a cut.
    pub(in super::super) fn start_history<F>(
        &self,
        config: &Config,
        loaded: &Loaded,
        control: &mut Control,
        deadline: &Deadline,
        time: &mut F,
        finish_after: Option<u64>,
    ) -> Result<
        (
            learned_recovery::Custody,
            FileIndependentEvaluator,
            Option<HistoricalTopology>,
        ),
        String,
    >
    where
        F: FnMut() -> ElapsedTick,
    {
        check_selection(loaded, true)?;
        let Some(topology::Selection::Recover { selection, history }) = loaded.topology.as_ref()
        else {
            let (custody, evaluator) =
                self.open_history(config, loaded, control, deadline, time, finish_after)?;
            return Ok((custody, evaluator, None));
        };
        let mut retention = FenceRetention::admit(config, loaded, history, selection)?;
        let output = AcknowledgedOutput::admit(
            config,
            selection
                .acknowledged_output()
                .ok_or("historical evaluated topology requires a new paired source checkpoint")?,
        )?;
        let original = history
            .checkpoint()
            .ok_or("historical evaluated topology requires an exact v2 source checkpoint")?
            .clone();
        let recovery = self.replay_mediated(
            config,
            loaded,
            history,
            control,
            deadline,
            time,
            Some(finish_after),
        )?;
        let (host, roles) = recovery.finish().map_err(debug)?;
        retention
            .capture(&host, Some(&roles.topology_observer))
            .map_err(|error| retention.retain_error(&host, error))?;
        let evaluator = roles.evaluator.ok_or_else(|| {
            retention.retain_error(
                &host,
                "native historical topology recovery did not provision its independent evaluator"
                    .into(),
            )
        })?;
        Ok((
            (
                host,
                roles.oversight.human,
                roles.consistency_observer,
                roles.oversight.identity_observer,
            ),
            evaluator,
            Some(HistoricalTopology {
                observer: roles.topology_observer,
                original,
                output,
            }),
        ))
    }
}

/// No live source exists while this observer is held. Independent labels and
/// terminal Stop may advance its history, but never its original source work.
pub(in super::super) struct HistoricalTopology {
    observer: FileMediationObserver,
    original: TopologyCheckpoint,
    output: AcknowledgedOutput,
}

impl HistoricalTopology {
    pub(super) fn capture(&self, host: &FileOversight) -> Result<TopologyCheckpoint, String> {
        self.observer
            .evaluated_topology_checkpoint(host, &self.original)
            .map_err(debug)
    }

    pub(super) fn retain(&self, saved: &TopologyCheckpoint) -> Result<(), String> {
        self.output.retain(saved)
    }
}
