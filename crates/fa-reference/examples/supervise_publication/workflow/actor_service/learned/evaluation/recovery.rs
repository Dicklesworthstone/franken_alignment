//! Exact accounted recovery over the original cooperative numerical cursor.
//! Every replay event and Ready retain the original independent Stop admission.
use super::{
    Config, Control, Deadline, ElapsedTick, FileIndependentEvaluator, FileOversight, Loaded,
    RecoveryReserve, Selection, debug, learned_recovery,
};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::evaluation::andon::{
    FileAndonRecoveryRequirements, learned::FileAndonLearnedRecovery,
};

impl Selection {
    pub(super) fn recovery_requirements(
        &self,
        config: &Config,
        loaded: &Loaded,
    ) -> Result<FileAndonRecoveryRequirements, String> {
        // Ordinary source-only requirements omit the original learned stream,
        // computed identity and credential guards. Bind the complete recipe's
        // expectations, including required absence, to the supplied inventory.
        let mut expected = FileAndonRecoveryRequirements::new(
            learned_recovery::requirements(config, loaded),
            self.settings.config.clone(),
            self.settings.windows.clone(),
            self.settings.minimum_evaluation_revision,
        )
        .map_err(debug)?;
        if let Some(publication) = &loaded.publication {
            expected = expected
                .require_publication(publication.simple_limits()?)
                .map_err(debug)?;
        }
        Ok(expected)
    }

    pub(in super::super) fn open<F>(
        &self,
        config: &Config,
        loaded: &Loaded,
        control: &mut Control,
        deadline: &Deadline,
        time: &mut F,
    ) -> Result<(learned_recovery::Custody, FileIndependentEvaluator), String>
    where
        F: FnMut() -> ElapsedTick,
    {
        self.open_selected(config, loaded, control, deadline, time, None)
    }

    /// Historical assessment uses this same fully guarded cursor and Stop
    /// arbitration, selecting the retained request before the sole Fence.
    pub(in super::super) fn open_history<F>(
        &self,
        config: &Config,
        loaded: &Loaded,
        control: &mut Control,
        deadline: &Deadline,
        time: &mut F,
        finish_after: Option<u64>,
    ) -> Result<(learned_recovery::Custody, FileIndependentEvaluator), String>
    where
        F: FnMut() -> ElapsedTick,
    {
        self.open_selected(config, loaded, control, deadline, time, Some(finish_after))
    }

    fn open_selected<F>(
        &self,
        config: &Config,
        loaded: &Loaded,
        control: &mut Control,
        deadline: &Deadline,
        time: &mut F,
        historical: Option<Option<u64>>,
    ) -> Result<(learned_recovery::Custody, FileIndependentEvaluator), String>
    where
        F: FnMut() -> ElapsedTick,
    {
        let expected = self.recovery_requirements(config, loaded)?;
        let anchor = loaded
            .recovery_anchor
            .as_ref()
            .ok_or("accounted learned recovery requires its exact retained history anchor")?;
        let mut pending = false;
        let recovery = {
            let mut checkpoint = |_| {
                deadline.check(time())?;
                pending |= control.recovery_pending_peer()?;
                Ok(())
            };
            checkpoint(None)?;
            let recovery =
                FileOversight::begin_open_andon_guarded_anchored_with_learned_generation(
                    &config.store,
                    config.profile.clone(),
                    &expected,
                    &self.settings.protocol,
                    &loaded.generation,
                    anchor,
                )
                .map_err(debug)?
                .require_recovery_reserve(RecoveryReserve::terminal())
                .map_err(debug)?;
            learned_recovery::finish_replay(
                recovery,
                &mut checkpoint,
                FileAndonLearnedRecovery::progress,
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
                // The pure historical selector may traverse the request book.
                // Re-arbitrate its remaining lease and any newly queued Stop
                // before acknowledging a Fence or issuing the evaluator role.
                deadline.check(time())?;
                pending = control.recovery_pending_peer()?;
            }
        }
        // Ready has its own checkpoint. A checked pending Stop reaches only
        // the native terminal target, before ordinary fencing or ANY live role.
        if pending {
            let target = recovery
                .into_recovery_stop(control.recovery_operation()?)
                .map_err(debug)?;
            let outcome = control.stop_recovery(target, deadline, time)?;
            let mut terminal = outcome.terminal_message();
            if let Ok(history) = &outcome.application.result {
                if let Some(path) = loaded.anchor_output.as_deref() {
                    if let Err(error) =
                        super::super::anchors::export_acknowledged(history.history_anchor(), path)
                    {
                        terminal.push_str(&format!(
                            "; history anchor retention failed: {error}; keep the previous archive and select a new output path"
                        ));
                    }
                }
            }
            return Err(terminal);
        }
        // An explicit restarted invocation gets a fresh finite transport lease;
        // the historical ledger, counters and native generation budget persist.
        // No old actor, approval, clock, source eligibility or evaluator revives.
        let (host, roles) = recovery.finish().map_err(debug)?;
        Ok((
            (
                host,
                roles.oversight.human,
                None,
                roles.oversight.identity_observer,
            ),
            roles.evaluator,
        ))
    }
}
