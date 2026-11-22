//! Explicit one-round successors retain the same original native wrapper.
//! All evaluator owners were fixed and fully funded before its initial Begin.
use super::{
    Config, Control, Deadline, ElapsedTick, FileNativeSupervisedDriver, Intake, debug, ingress,
    plus,
};
use fa_reference::action::consequence::oversight::ReviewWindow;

/// The fixed-slot path remains separate. This wait is eligible only while the
/// original wrapper owns an applied Hold and a separately reserved evaluator.
pub(super) fn continue_held<F>(
    run: &mut FileNativeSupervisedDriver,
    config: &mut Config,
    control: &mut Control,
    deadline: &Deadline,
    time: &mut F,
    intake: &mut Option<Intake>,
    operator: &mut Option<super::super::operator::Service>,
    stop_requested: &mut bool,
) -> Result<bool, String>
where
    F: FnMut() -> ElapsedTick,
{
    if !run.has_reserved_successor() {
        return Ok(false);
    }
    loop {
        deadline.check(time())?;
        // Independent Stop remains first; every peer keeps its original
        // credentials, listener, allowance and invocation deadline.
        if control.pending_peer()? {
            *stop_requested = true;
            run.cancel(run.review().revision()).map_err(debug)?;
            return Ok(false);
        }
        super::super::operator::checkpoint_supervisor(operator, run.supervisor_mut());
        if run
            .supervisor()
            .host()
            .map_err(debug)?
            .inspect()
            .stop
            .is_some()
        {
            *stop_requested = true;
            run.cancel(run.review().revision()).map_err(debug)?;
            return Ok(false);
        }
        if ingress::observe(intake, run.supervisor_mut())? {
            run.cancel(run.review().revision()).map_err(debug)?;
            return Ok(false);
        }
        if !run.supervisor().has_live_convening().map_err(debug)? || !run.has_reserved_successor() {
            return Ok(false);
        }
        if let Some(ticket) = run.take_pending_convene().map_err(debug)? {
            let now = time();
            deadline.check(now)?;
            let window = ReviewWindow {
                commit_by: plus(now, config.timing.commit_ms)?,
                reveal_by: plus(now, config.timing.reveal_ms)?,
            };
            if window.reveal_by >= deadline.logical {
                return Err(
                    "insufficient original lifetime for the learned successor review".into(),
                );
            }
            // The wrapper consumes its own weak custody offer, original ticket,
            // one preprovisioned roster and current source. No new helper or
            // sidecar is created, and no original limit is replenished.
            run.convene_reserved_from_policy_file(&mut config.source, ticket, window, &mut *time)
                .result
                .map_err(debug)?;
            return Ok(true);
        }
        super::super::super::super::pause(config.timing.poll_ms.clamp(1, 50));
    }
}
