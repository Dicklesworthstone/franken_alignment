//! Original retained visits, exact encoder budgets and real native stop records.
use super::*;
use super::super::super::test_support::*;
use std::fs;

fn visit(domain: u64, revision: Option<usize>) -> ShutdownVisit {
    ShutdownVisit { domain, kind: ShutdownVisitKind::Advance { at: ElapsedTick(100) },
        result: revision.map_or(ShutdownVisitResult::Entered, |revision|
            ShutdownVisitResult::Observed { domain_revision: revision as u64 }) }
}
fn stopped_head(host: &FileOversight) -> (Vec<u8>, usize) {
    let event = Event::Core(BaseEvent::Stop(request(host, 900)));
    (journal::encode_appended(&host.profile, host.store.identity(), &host.events, &event).unwrap(),
        host.events.len() + 1)
}
fn limit(control: &Directory, plan: &FileShutdownPlan, visits: &[ShutdownVisit],
    heads: &[(usize, Vec<u8>, usize)]) -> usize
{
    let mut campaign = plan.start();
    for (index, head, revision) in heads {
        campaign.head_bytes = campaign.head_bytes - campaign.slots[*index].head.len() + head.len();
        campaign.slots[*index].head = Rc::from(head.as_slice());
        campaign.slots[*index].revision = *revision;
    }
    let revision = visits.len() as u64 + visits.iter()
        .filter(|entry| entry.result != ShutdownVisitResult::Entered).count() as u64;
    let path = fs::canonicalize(&control.0).unwrap().join("publication");
    codec::encode(&path, &plan.encode().unwrap(), MAX_JOURNAL_BYTES,
        revision, visits, &campaign).unwrap().len()
}
fn enter_and_stop(c: &mut FileShutdownCoordinator, domain: u64, host: &mut FileOversight) {
    let pair = c.begin(c.revision(), domain, visit(domain, None).kind).unwrap();
    let actual = c.campaign.advance_inner(pair.0, pair.1, host, ElapsedTick(100)).unwrap();
    assert!(actual.0.stop.is_some());
    // Simulate interruption AFTER native acknowledgment but BEFORE coordinator
    // completion. Only the original Entered image is durable; drop and reopen.
    assert!(c.report().unavailable);
}
fn open(control: &Directory, plan: &FileShutdownPlan, cap: usize, revision: u64)
    -> FileShutdownCoordinator
{
    FileShutdownCoordinator::open(control.store(), plan, cap, revision).unwrap()
}

#[test]
fn exact_pending_completion_fits_but_one_byte_short_is_a_healthy_evidence_refusal() {
    for short in [false, true] { for quantum in [1, 3] {
        let root = Directory::new(); let control = Directory::new();
        let mut host = create(&root, 256); let p = plan(&host, 1);
        for tick in 2..=81 { host.observe_time(host.revision(), ElapsedTick(tick)).unwrap(); }
        let (head, revision) = stopped_head(&host);
        let exact = limit(&control, &p, &[visit(1, Some(revision))], &[(0, head.clone(), revision)]);
        let cap = exact - usize::from(short);
        let mut c = FileShutdownCoordinator::create(control.store(), &p, cap).unwrap();
        enter_and_stop(&mut c, 1, &mut host);
        assert_eq!(root.image(), head); drop(c);
        let mut c = open(&control, &p, cap, 1); let saved = control.image();
        let anchor = c.history_anchor().unwrap();
        let pending = root.store().join("delivery.pending"); fs::write(&pending, b"untouched").unwrap();
        if short {
            assert_eq!(c.begin_resolve_shutdown_visit(1, 0).unwrap_err(), Error::Limit.into());
            assert!(!c.report().unavailable); assert_eq!(c.revision(), 1);
            assert_eq!(control.image(), saved); assert_eq!(c.visits[0].result, ShutdownVisitResult::Entered);
            assert_eq!(c.campaign.slots[0].revision, p.domains()[0].registered_revision() as usize);
            assert_eq!(c.campaign.completion_head_limit, None);
            assert!(matches!(&c.report().current_session.attempts[0].result,
                FileShutdownResult::Refused(JournalError::Contract(Error::Limit))));
        } else {
            let mut read = c.begin_resolve_shutdown_visit(1, 0).unwrap();
            while read.progress().status == FileStopRecoveryStatus::Replaying {
                read.advance(read.progress().replayed_events, quantum).unwrap();
            }
            let observed = read.finish().unwrap();
            assert_eq!(observed.journal_revision, revision as u64);
            assert_eq!(c.revision(), 2); assert_eq!(control.image().len(), cap);
            assert_eq!(c.campaign.completion_head_limit, None);
            let successor = c.history_anchor_after(&anchor).unwrap(); let acknowledged = control.image();
            drop(c);
            let mut c = FileShutdownCoordinator::open_anchored(control.store(), &p, cap, 2, &successor).unwrap();
            // An exact already-observed refresh needs no additional bytes at capacity.
            c.resolve_shutdown_visit(2, 0).unwrap();
            assert_eq!(control.image(), acknowledged); assert_eq!(c.revision(), 2);
        }
        assert_eq!(root.image(), head); assert_eq!(fs::read(pending).unwrap(), b"untouched");
    }}
}

#[test]
fn metadata_shortage_precedes_domain_io_without_clearing_a_custody_conflict() {
    let root = Directory::new(); let control = Directory::new();
    let mut host = create(&root, 64); let p = plan(&host, 4);
    // Enough for the original Entered image, not even its Observed metadata.
    let cap = limit(&control, &p, &[visit(1, None)], &[]);
    let mut c = FileShutdownCoordinator::create(control.store(), &p, cap).unwrap();
    enter_and_stop(&mut c, 1, &mut host); drop(c); drop(host);
    let mut c = open(&control, &p, cap, 1); let saved = control.image();
    let parked = root.store().with_extension("parked"); fs::rename(root.store(), &parked).unwrap();
    for attempts in 1..=2 {
        assert_eq!(c.begin_resolve_shutdown_visit(1, 0).unwrap_err(), Error::Limit.into());
        assert!(!c.report().unavailable); assert_eq!(c.report().current_session.attempts.len(), attempts);
        assert_eq!(c.campaign.completion_head_limit, None); assert_eq!(control.image(), saved);
    }
    // The capacity refusal must not mask changed coordinator history or make
    // restoring its bytes rehabilitate the same quarantined owner.
    fs::write(control.store().join(storage::CANONICAL), b"changed custody").unwrap();
    assert_eq!(c.begin_resolve_shutdown_visit(1, 0).unwrap_err(), Error::Binding.into());
    assert!(c.report().unavailable);
    fs::write(control.store().join(storage::CANONICAL), &saved).unwrap();
    assert_eq!(c.begin_resolve_shutdown_visit(1, 0).unwrap_err(), JournalError::Unavailable);
    fs::rename(parked, root.store()).unwrap();
}

#[test]
fn unretainable_first_receipt_does_not_prevent_a_smaller_pending_member_from_resolving() {
    for constrained in [true, false] {
        let first = Directory::new(); let second = Directory::new(); let control = Directory::new();
        let mut one = member(&first, 1, 99); let mut two = member(&second, 2, 100);
        let p = FileShutdownPlan::new(900, vec![one.shutdown_domain(1).unwrap(),
            two.shutdown_domain(2).unwrap()], 2, MAX_SHUTDOWN_HEAD_BYTES).unwrap();
        for tick in 2..=81 { one.observe_time(one.revision(), ElapsedTick(tick)).unwrap(); }
        let (head1, rev1) = stopped_head(&one); let (head2, rev2) = stopped_head(&two);
        let cap = if constrained {
            limit(&control, &p, &[visit(1, None), visit(2, Some(rev2))], &[(1, head2.clone(), rev2)])
        } else {
            limit(&control, &p, &[visit(1, Some(rev1)), visit(2, Some(rev2))],
                &[(0, head1.clone(), rev1), (1, head2.clone(), rev2)])
        };
        let mut c = FileShutdownCoordinator::create(control.store(), &p, cap).unwrap();
        enter_and_stop(&mut c, 1, &mut one); drop(c);
        let mut c = open(&control, &p, cap, 1);
        enter_and_stop(&mut c, 2, &mut two); drop(c);
        let mut c = open(&control, &p, cap, 2); let anchor = c.history_anchor().unwrap();
        let pass = c.resume_registered_pass_with_clocks(2, &[
            FileShutdownClockBinding { domain: 2, clock_domain: 100 },
            FileShutdownClockBinding { domain: 1, clock_domain: 99 },
        ], 1, |_| panic!("resolution never acquires a settlement clock"), |_| true).unwrap();
        assert!(!c.report().unavailable); assert_eq!(pass.members().len(), 2);
        assert!(matches!(&pass.members()[1].result, FileShutdownPassResult::Observed(_)));
        assert!(pass.members().iter().all(|entry| entry.acquired_tick.is_none()));
        if constrained {
            assert!(matches!(&pass.members()[0].result,
                FileShutdownPassResult::EvidenceRefused(JournalError::Contract(Error::Limit))));
            assert_eq!(c.visits[0].result, ShutdownVisitResult::Entered);
            assert_eq!(c.revision(), 3); assert!(!pass.all_observed_stopped());
        } else {
            assert!(pass.all_observed_stopped()); assert_eq!(c.revision(), 4);
        }
        assert_eq!(control.image().len(), cap); assert_eq!(c.visits.len(), 2);
        c.history_anchor_after(&anchor).unwrap();
        assert_eq!(first.image(), head1); assert_eq!(second.image(), head2);
    }
}

#[test]
fn cancellation_releases_only_the_temporary_ceiling_and_preserves_pending_history() {
    let root = Directory::new(); let control = Directory::new();
    let mut host = create(&root, 64); let p = plan(&host, 8);
    let (head, revision) = stopped_head(&host);
    let cap = limit(&control, &p, &[visit(1, Some(revision))], &[(0, head.clone(), revision)]);
    let mut c = FileShutdownCoordinator::create(control.store(), &p, cap).unwrap();
    enter_and_stop(&mut c, 1, &mut host); drop(c);
    let mut c = open(&control, &p, cap, 1); let saved = control.image();
    for count in [0, 1, revision] {
        let mut read = c.begin_resolve_shutdown_visit(1, 0).unwrap();
        for _ in 0..count { read.advance(read.progress().replayed_events, 1).unwrap(); }
        read.cancel().unwrap();
        assert!(!c.report().unavailable); assert_eq!(c.campaign.completion_head_limit, None);
        assert_eq!(control.image(), saved); assert_eq!(root.image(), head);
        assert_eq!(c.visits[0].result, ShutdownVisitResult::Entered);
    }
    assert_eq!(c.report().current_session.attempts.len(), 3);
    drop(c.begin_resolve_shutdown_visit(1, 0).unwrap());
    assert!(c.report().unavailable); drop(c);
    let mut c = open(&control, &p, cap, 1);
    assert_eq!(c.campaign.completion_head_limit, None);
    c.resolve_shutdown_visit(1, 0).unwrap(); assert_eq!(c.revision(), 2);
    assert_eq!(root.image(), head);
}

#[test]
fn real_acknowledgment_write_failures_still_quarantine_and_resume_from_visible_bytes() {
    for barrier in BARRIERS {
        let root = Directory::new(); let control = Directory::new();
        let mut host = create(&root, 64); let p = plan(&host, 8);
        let (head, revision) = stopped_head(&host);
        let cap = limit(&control, &p, &[visit(1, Some(revision))], &[(0, head.clone(), revision)]);
        let mut c = FileShutdownCoordinator::create(control.store(), &p, cap).unwrap();
        enter_and_stop(&mut c, 1, &mut host); drop(c);
        let mut c = open(&control, &p, cap, 1); let saved = control.image();
        let anchor = c.history_anchor().unwrap(); c.store.fail_once(barrier);
        let result = c.resolve_shutdown_visit(1, 0);
        assert!(matches!(result, Err(JournalError::Io(ref error)) if error.operation == barrier));
        assert!(c.report().unavailable); assert_eq!(c.revision(), 1);
        assert_eq!(c.acknowledged_image.as_deref(), Some(saved.as_slice()));
        assert_eq!(control.image() != saved, barrier == JournalIo::DirectorySync);
        assert_eq!(root.image(), head); drop(c);
        let mut c = FileShutdownCoordinator::open_anchored(control.store(), &p, cap, 1, &anchor).unwrap();
        let actual = c.resolve_shutdown_visit(c.revision(), 0).unwrap();
        assert_eq!(actual.journal_revision, revision as u64); assert_eq!(c.revision(), 2);
        assert_eq!(control.image().len(), cap); assert_eq!(root.image(), head);
    }
}
