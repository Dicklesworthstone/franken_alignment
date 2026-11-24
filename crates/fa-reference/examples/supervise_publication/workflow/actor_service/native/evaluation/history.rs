//! Delayed independent labels on an exact original recorded native request.
//! No actor intake, human review, source refresh, helper, resume or effect retry.
use super::{
    Config, Inputs, PeerProfile, Profile, PublicationProfile, Selection, anchors, debug, recovery,
};
use super::super::{
    Control, Deadline, Duration, ElapsedTick, FileSupervisedDriver, Instant, RecoveryReserve,
    bootstrap, check_profiles, finish, pause, plus,
};
use super::super::super::evaluator;

impl Selection {
    /// Reopen an independently anchored history only for case discovery and
    /// assessment. The same Stop transport covers every replay event and the
    /// complete finite evaluator lifetime. A missing or different recorded
    /// request refuses before the original cursor can acknowledge its Fence.
    pub(in super::super) fn serve_history<F>(
        &self,
        config: Config,
        peers: (&Profile, &PeerProfile),
        inputs: Inputs,
        publication: Option<&PublicationProfile>,
        history: &anchors::Selection,
        finishing: Option<u64>,
        mut time: F,
    ) -> Result<(), String>
    where
        F: FnMut() -> ElapsedTick,
    {
        if !self.evaluate_only {
            return Err(super::USAGE.into());
        }
        let (actor, reviewer_profile) = peers;
        if let Some(previous) = finishing {
            finish::Intent::Finish(previous).check(true, actor.request)?;
        }
        check_profiles(&config, actor, reviewer_profile)?;
        history.check(&config, &[(actor, None)], reviewer_profile, None, true)?;
        self.check(
            &config, actor, reviewer_profile,
            (None, None, Some(history)), (true, publication, None),
        )?;
        if inputs.decoder.profile().identity().tenant != config.profile.delivery.scope.tenant {
            return Err("native model tenant mismatch".into());
        }
        let selected = bootstrap::selection(&config, &inputs, publication)?;
        let expected = self.recovery_requirements(&config, &inputs, &selected)?;
        let anchor = history.input().ok_or(super::USAGE)?;
        let runtime = actor.runtime_ms.min(config.timing.runtime_ms);
        let start = time();
        let deadline = Deadline {
            logical: plus(start, runtime)?,
            started: Instant::now(),
            wall: Duration::from_millis(runtime),
        };
        // The actor and review endpoints are never bound. This listener admits
        // only the existing independent Stop protocol, including during replay.
        let mut control = Control::new(&config, actor.request, Some(reviewer_profile))?;
        let (recovery, pending) = recovery::ready(
            &config, &inputs, selected, &expected, &self.settings.protocol, anchor,
            &mut control, &deadline, &mut time,
        )?;
        if pending {
            let target = recovery.into_recovery_stop(control.recovery_operation()?).map_err(debug)?;
            return Err(control.stop_recovery_with_history(
                target, Some(history), &deadline, &mut time,
            )?);
        }
        // These are immutable original-history selectors, after full guarded
        // Ready verification. They cannot create a missing Message or Finish,
        // reconcile an uncertain effect, or demand a newer numerical cursor.
        match finishing {
            Some(previous) => recovery.recorded_finish(
                previous, actor.request, inputs.generation, &inputs.request,
            ),
            None => recovery.recorded_message(
                actor.request, inputs.generation, &inputs.request,
            ),
        }.map_err(debug)?;
        deadline.check(time())?;
        if control.recovery_pending_peer()? {
            let target = recovery.into_recovery_stop(control.recovery_operation()?).map_err(debug)?;
            return Err(control.stop_recovery_with_history(
                target, Some(history), &deadline, &mut time,
            )?);
        }
        let (host, roles) = recovery.finish().map_err(debug)?;
        let checked: Result<(), String> = (|| {
            if !host.generated_text_stream_required().map_err(debug)?
                || !host.publication_guard_required()
                || host.file_source_status().map(|status| status.policy) != Some(config.source_policy)
                || host.journal_capacity().map_err(debug)?.reserve() != Some(RecoveryReserve::terminal())
                || host.publication_validation_profile().map_err(debug)?
                    != publication.map(|profile| profile.limits)
            {
                return Err(
                    "historical native source, reserve and publication contracts must match".into(),
                );
            }
            Ok(())
        })();
        if let Err(error) = checked {
            return history.retain(&host, Err(error));
        }
        // Gateway transfer adds no journal event. Retain this exact successor
        // even if transfer fails; no actor port escapes the evaluation service.
        let captured = history.capture(&host)?;
        let (port, supervisor) = match host.into_generated_text_actor_gateway() {
            Ok(gateway) => gateway,
            Err(error) => return history.finish_capture(Ok(captured), Err(debug(error))),
        };
        drop(port);
        let reviewer = roles.oversight.human;
        let mut driver = FileSupervisedDriver::new(supervisor);
        let outcome: Result<(), String> = (|| {
            deadline.check(time())?;
            if control.checkpoint(&mut driver, &reviewer, &deadline, &mut time)? {
                return Ok(());
            }
            let service = evaluator::Service::start(
                &self.peer, actor, roles.evaluator, &driver, &deadline,
            )?;
            control.install_evaluator(Some(service));
            while control.has_live_evaluator() && deadline.check(time()).is_ok() {
                if control.checkpoint(&mut driver, &reviewer, &deadline, &mut time)? {
                    break;
                }
                if control.has_live_evaluator() {
                    pause(actor.poll_ms.clamp(1, 50));
                }
            }
            Ok(())
        })();
        // Even an error or partial request cannot mutate the retained cut after
        // this point. Explicit Stop outcomes and unknown charges remain original.
        control.finish_evaluator();
        if let Some(mut service) = control.take_evaluator() {
            service.retire();
        }
        drop(control);
        history.retain_driver(&driver, outcome)
    }
}
