//! Exact original image sizes, original terminal consumers and real storage.
use super::*;
use super::super::test_support::*;
use std::cell::Cell;

fn binding() -> FileShutdownClockBinding { FileShutdownClockBinding { domain: 1, clock_domain: 99 } }
fn kind(mode: usize) -> ShutdownVisitKind {
    match mode {
        0 | 1 => ShutdownVisitKind::RecoverStopped { at: ElapsedTick(100) },
        2 => ShutdownVisitKind::RecoverStoppedAtCompletion,
        3 => ShutdownVisitKind::StopAdmission,
        4 => ShutdownVisitKind::DrainStoppedAtCompletion,
        _ => unreachable!(),
    }
}
fn refusal_limit(root: &Directory, plan: &FileShutdownPlan, kind: ShutdownVisitKind) -> usize {
    let identity = std::fs::canonicalize(&root.0).unwrap().join("publication");
    let visits = [ShutdownVisit { domain: 1, kind, result: ShutdownVisitResult::Refused {
        diagnostic: "x".repeat(MAX_REFUSAL_BYTES),
    } }];
    codec::encode(&identity, &plan.encode().unwrap(), MAX_JOURNAL_BYTES,
        2, &visits, &plan.start()).unwrap().len()
}
fn predicted(host: &FileOversight, mode: usize) -> (Vec<u8>, usize) {
    let stop = host.inspect().stop.map(|receipt| receipt.request()).unwrap_or_else(|| request(host, 900));
    let tail = match mode {
        0..=2 => vec![BaseEvent::Stop(stop), BaseEvent::Fence, BaseEvent::StopProgress(ElapsedTick(100))],
        3 => vec![BaseEvent::Stop(stop), BaseEvent::Fence],
        4 => vec![BaseEvent::StopProgress(ElapsedTick(100))],
        _ => unreachable!(),
    };
    let mut events = host.events.clone();
    let mut head = Vec::new();
    for event in tail {
        let event = Event::Core(event);
        head = journal::encode_appended(&host.profile, host.store.identity(), &events, &event).unwrap();
        events.push(event);
    }
    (head, events.len())
}
fn observed_limit(root: &Directory, p: &FileShutdownPlan, kind: ShutdownVisitKind,
    index: usize, head: &[u8], revision: usize) -> usize
{
    let mut campaign = p.start();
    campaign.head_bytes = campaign.head_bytes - campaign.slots[index].head.len() + head.len();
    campaign.slots[index].head = Rc::from(head);
    campaign.slots[index].revision = revision;
    let visits = [ShutdownVisit { domain: p.domains()[index].id(), kind,
        result: ShutdownVisitResult::Observed { domain_revision: revision as u64 } }];
    let identity = std::fs::canonicalize(&root.0).unwrap().join("publication");
    codec::encode(&identity, &p.encode().unwrap(), MAX_JOURNAL_BYTES, 2, &visits, &campaign).unwrap().len()
}
fn run(c: &mut FileShutdownCoordinator, mode: usize)
    -> Result<Result<FileShutdownObservation, JournalError>, JournalError>
{
    let revision = c.revision();
    match mode {
        0 => c.recover_and_drain(revision, 1, ElapsedTick(100)),
        1 => match c.begin_recover_and_drain(revision, 1, ElapsedTick(100))? {
            Ok(mut visit) => {
                while visit.progress().status == FileStopRecoveryStatus::Replaying {
                    visit.advance(visit.progress().replayed_events, 3)?;
                }
                visit.finish()
            }
            Err(error) => Ok(Err(error)),
        },
        2 => c.recover_and_drain_with_clock(revision, binding(), 3, |_| Ok(ElapsedTick(100)), |_| true),
        3 => c.recover_admission_stop(revision, binding(), 3, |_| true),
        4 => c.drain_stopped_with_clock(revision, binding(), 3, |_| Ok(ElapsedTick(100)), |_| true),
        _ => unreachable!(),
    }
}

#[test]
fn exact_success_budget_and_one_short_decide_before_native_mutation_for_every_terminal_path() {
    for mode in 0..5 { for short in [false, true] {
        let root = Directory::new(); let control = Directory::new();
        let mut host = create(&root, 256); let p = plan(&host, 8);
        // A genuine successor is larger than the previously retained prefix.
        // The old intent/refusal fits, but the new terminal head is the boundary.
        for tick in 2..=81 { host.observe_time(host.revision(), ElapsedTick(tick)).unwrap(); }
        if mode == 4 {
            let stop = request(&host, 900); host.request_stop(host.revision(), stop).unwrap();
            assert!(!host.stop_progress().unwrap().drained());
        }
        let (head, revision) = predicted(&host, mode);
        let exact = observed_limit(&control, &p, kind(mode), 0, &head, revision);
        let cap = exact - usize::from(short);
        assert!(cap >= refusal_limit(&control, &p, kind(mode)));
        let native = root.image(); drop(host);
        let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"retained").unwrap();
        let mut c = FileShutdownCoordinator::create(control.store(), &p, cap).unwrap();
        let initial = c.history_anchor().unwrap();
        let result = run(&mut c, mode);
        assert_eq!(c.revision(), 2); assert!(!c.report().unavailable);
        assert_eq!(c.campaign.completion_head_limit, None);
        if short {
            assert_eq!(result, Ok(Err(Error::Limit.into())));
            assert_eq!(root.image(), native); assert_eq!(std::fs::read(&pending).unwrap(), b"retained");
            assert!(matches!(c.report().visits[0].result, ShutdownVisitResult::Refused { .. }));
        } else {
            let observation = result.unwrap().unwrap();
            assert_eq!(observation.journal_revision, revision as u64);
            assert_eq!(root.image(), head); assert_eq!(control.image().len(), cap);
            let anchor = c.history_anchor_after(&initial).unwrap();
            let coordinator_bytes = control.image(); drop(c);
            let mut c = FileShutdownCoordinator::open_anchored(control.store(), &p, cap, 2, &anchor).unwrap();
            assert_eq!(c.campaign.completion_head_limit, None);
            c.resolve_shutdown_visit(2, 0).unwrap();
            assert_eq!(root.image(), head); assert_eq!(control.image(), coordinator_bytes);
        }
    }}
}

#[test]
fn an_unretainable_acquired_successor_refuses_before_replay_or_clock() {
    let root = Directory::new(); let control = Directory::new();
    let mut host = create(&root, 256); let p = plan(&host, 8);
    for tick in 2..=81 { host.observe_time(host.revision(), ElapsedTick(tick)).unwrap(); }
    let native = root.image(); drop(host);
    let cap = refusal_limit(&control, &p, kind(2));
    let mut c = FileShutdownCoordinator::create(control.store(), &p, cap).unwrap();
    assert_eq!(c.recover_and_drain_with_clock(0, binding(), 1,
        |_| panic!("no clock for rejected retained image"), |_| panic!("no replay")),
        Ok(Err(Error::Limit.into())));
    assert_eq!(root.image(), native); assert_eq!(c.revision(), 2);
    assert!(!c.report().unavailable); assert_eq!(c.campaign.completion_head_limit, None);
}

#[test]
fn success_capacity_includes_every_other_registered_head_not_just_the_changed_domain() {
    for short in [false, true] {
        let root = Directory::new(); let other = Directory::new(); let control = Directory::new();
        let mut host = create(&root, 256); let tail = member(&other, 2, 100);
        let p = FileShutdownPlan::new(900, vec![host.shutdown_domain(1).unwrap(), tail.shutdown_domain(2).unwrap()],
            8, MAX_SHUTDOWN_HEAD_BYTES).unwrap();
        for tick in 2..=81 { host.observe_time(host.revision(), ElapsedTick(tick)).unwrap(); }
        let (head, revision) = predicted(&host, 3);
        let cap = observed_limit(&control, &p, kind(3), 0, &head, revision) - usize::from(short);
        let first = root.image(); let second = other.image(); drop(host); drop(tail);
        let mut c = FileShutdownCoordinator::create(control.store(), &p, cap).unwrap();
        let result = run(&mut c, 3).unwrap();
        assert_eq!(result.is_ok(), !short);
        assert_eq!(root.image(), if short { first } else { head });
        assert_eq!(other.image(), second); assert!(!c.report().all_observed_drained());
        assert_eq!(c.report().visits.len(), 1); assert_eq!(c.campaign.plan.max_head_bytes, p.max_head_bytes);
    }
}

#[test]
fn the_completion_ceiling_is_not_replayed_or_carried_into_unrelated_campaign_entries() {
    let root = Directory::new(); let control = Directory::new();
    let host = create(&root, 64); let p = plan(&host, 8); drop(host);
    let mut c = FileShutdownCoordinator::create(control.store(), &p, MAX_JOURNAL_BYTES).unwrap();
    let pair = c.begin_registered_terminal(0, 1, kind(3)).unwrap();
    assert!(c.campaign.completion_head_limit.is_some());
    c.finish(pair.0, pair.1, Err(JournalError::Unavailable)).unwrap().unwrap_err();
    assert_eq!(c.campaign.completion_head_limit, None);
    let anchor = c.history_anchor().unwrap(); let bytes = control.image(); drop(c);
    let mut c = FileShutdownCoordinator::open_anchored(control.store(), &p, MAX_JOURNAL_BYTES, 2, &anchor).unwrap();
    assert_eq!(c.campaign.completion_head_limit, None); assert_eq!(control.image(), bytes);
    c.inspect_canonical(2, 1).unwrap().unwrap();
    assert_eq!(c.campaign.completion_head_limit, None);
}

#[test]
fn actual_completion_io_failure_still_requires_resolution_not_a_second_domain_command() {
    for barrier in BARRIERS {
        let root = Directory::new(); let control = Directory::new();
        let host = create(&root, 64); let p = plan(&host, 8); drop(host);
        let mut c = FileShutdownCoordinator::create(control.store(), &p, MAX_JOURNAL_BYTES).unwrap();
        let pair = c.begin_registered_terminal(0, 1, kind(0)).unwrap();
        let result = c.recover_domain(pair.0, pair.1, ElapsedTick(100));
        assert!(result.is_ok()); let native = root.image();
        c.store.fail_once(barrier);
        assert!(matches!(c.finish(pair.0, pair.1, result), Err(JournalError::Io(ref e)) if e.operation == barrier));
        assert!(c.report().unavailable); drop(c);
        let mut c = reopen(&control, &p, 1);
        let calls = Cell::new(0);
        let pass = c.resume_registered_pass_with_clocks(c.revision(), &[binding()], 1,
            |_| { calls.set(calls.get() + 1); panic!("passive resolution needs no time") }, |_| true).unwrap();
        assert!(pass.all_observed_drained()); assert_eq!(calls.get(), 0);
        assert_eq!(root.image(), native);
    }
}
