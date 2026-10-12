//! One-event driver for the original guarded recovery implementations.
//! Callbacks see progress, not the candidate machine, journal or recovered roles.
use fa_reference::action::consequence::delivery::persistent::{
    JournalError,
    observed::decoder::learned::FileLearnedRecoveryStatus as Status,
};
pub(super) use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::FileLearnedRecoveryProgress as Progress;
use super::debug;

pub(super) fn finish_replay<R, T>(
    mut recovery: R,
    checkpoint: &mut impl FnMut(Option<Progress>) -> Result<(), String>,
    progress: impl Fn(&R) -> Progress,
    mut advance: impl FnMut(&mut R, usize) -> Result<Progress, JournalError>,
    finish: impl FnOnce(R) -> Result<T, JournalError>,
) -> Result<T, String> {
    let mut current = progress(&recovery);
    let total = current.total_events;
    loop {
        if current.total_events != total || current.replayed_events > total
            || (current.status == Status::Ready && current.replayed_events != total) {
            return Err("invalid original recovery progress".into());
        }
        match current.status {
            Status::Failed(error) => return Err(debug(error)),
            Status::Interrupted => return Err(debug(JournalError::Unavailable)),
            Status::Replaying | Status::Ready => {}
        }
        // In particular, Ready gets its OWN checkpoint. Cancellation after the
        // last expensive event must not fall through to cleanup/fence/role issue.
        checkpoint(Some(current))?;
        if current.status == Status::Ready {
            return finish(recovery).map_err(debug);
        }
        let next = advance(&mut recovery, current.replayed_events).map_err(debug)?;
        if next != progress(&recovery) || next.total_events != total
            || next.replayed_events < current.replayed_events
            || next.replayed_events > current.replayed_events.saturating_add(1)
            || (next.replayed_events == current.replayed_events && next.status == Status::Replaying) {
            return Err("original recovery did not honor its single-event quantum".into());
        }
        current = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fa_reference::Error;
    use std::cell::{Cell, RefCell};

    fn pending(total: usize) -> Progress {
        Progress { replayed_events: 0, total_events: total, status: Status::Replaying }
    }
    fn advance(run: &mut Progress, done: usize) -> Result<Progress, JournalError> {
        assert_eq!(done, run.replayed_events);
        run.replayed_events = (done + 1).min(run.total_events);
        if run.replayed_events == run.total_events { run.status = Status::Ready; }
        Ok(*run)
    }

    #[test]
    fn checks_every_predecessor_and_ready_before_finishing_once() {
        let trace = RefCell::new(Vec::new());
        let result = finish_replay(pending(3), &mut |view| {
            let view = view.unwrap();
            trace.borrow_mut().push(("check", view.replayed_events));
            Ok(())
        }, |run| *run, |run, done| {
            trace.borrow_mut().push(("advance", done));
            advance(run, done)
        }, |run| {
            trace.borrow_mut().push(("finish", run.replayed_events));
            Ok(run.replayed_events)
        }).unwrap();
        assert_eq!(result, 3);
        assert_eq!(*trace.borrow(), vec![("check", 0), ("advance", 0),
            ("check", 1), ("advance", 1), ("check", 2), ("advance", 2),
            ("check", 3), ("finish", 3)]);
    }

    #[test]
    fn cancellation_at_every_boundary_including_ready_never_finishes() {
        for cut in 0..=3 {
            let finished = Cell::new(false);
            let result = finish_replay(pending(3), &mut |view| {
                if view.unwrap().replayed_events == cut { Err("cancelled".into()) } else { Ok(()) }
            }, |run| *run, advance, |_| { finished.set(true); Ok(()) });
            assert_eq!(result, Err("cancelled".into()));
            assert!(!finished.get());
        }
    }

    #[test]
    fn empty_history_still_needs_ready_checkpoint_before_finish() {
        let seen = RefCell::new(Vec::new());
        finish_replay(pending(0), &mut |view| { seen.borrow_mut().push(view.unwrap().status); Ok(()) },
            |run| *run, advance, |_| Ok(())).unwrap();
        assert_eq!(*seen.borrow(), [Status::Replaying, Status::Ready]);
    }

    #[test]
    fn original_reducer_error_is_not_retried_or_finished() {
        let calls = Cell::new(0);
        let result = finish_replay(pending(3), &mut |_| Ok(()), |run| *run,
            |_, _| { calls.set(calls.get() + 1); Err(Error::Binding.into()) },
            |_| -> Result<(), JournalError> { panic!("failed replay cannot finish") });
        assert_eq!(result, Err(debug(JournalError::Contract(Error::Binding))));
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn stalled_skipped_or_changed_history_progress_cannot_spin_or_finish() {
        for (done, total) in [(0, 3), (2, 3), (1, 4)] {
            let result = finish_replay(pending(3), &mut |_| Ok(()), |run| *run,
                |run, _| { run.replayed_events = done; run.total_events = total; Ok(*run) },
                |_| -> Result<(), JournalError> { panic!("invalid progress cannot finish") });
            assert!(result.unwrap_err().contains("single-event quantum"));
        }
    }

    #[test]
    fn failed_interrupted_or_premature_ready_never_reaches_a_checkpoint_or_role() {
        for status in [Status::Failed(Error::Binding), Status::Interrupted, Status::Ready] {
            let mut run = pending(3); run.status = status;
            let result = finish_replay(run,
                &mut |_| -> Result<(), String> { panic!("invalid progress is not ready") },
                |run| *run, advance,
                |_| -> Result<(), JournalError> { panic!("invalid progress cannot finish") });
            assert!(result.is_err());
        }
    }

    #[test]
    fn finish_storage_error_returns_no_result_and_is_not_retried() {
        let calls = Cell::new(0);
        let result = finish_replay(pending(1), &mut |_| Ok(()), |run| *run, advance,
            |_| -> Result<(), JournalError> { calls.set(calls.get() + 1); Err(JournalError::Unavailable) });
        assert_eq!(result, Err(debug(JournalError::Unavailable)));
        assert_eq!(calls.get(), 1);
    }
}
