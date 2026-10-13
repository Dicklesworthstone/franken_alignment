//! Real coordinator/domain images. Custody is independent fixture data, not authentication.
use super::*;
use crate::action::consequence::delivery::persistent::observed::{BaseEvent, Event, Machine};
#[path = "../recovery/test_support.rs"] mod fixture;
use fixture::*;
#[path = "../../recovery/fixture.rs"] mod effects;

fn coordinator(root: &Directory, p: &FileShutdownPlan) -> FileShutdownCoordinator {
    FileShutdownCoordinator::create(root.store(), p, MAX_JOURNAL_BYTES).unwrap()
}
fn imported(anchor: &FileShutdownCoordinatorAnchor) -> FileShutdownCoordinatorAnchor {
    FileShutdownCoordinatorAnchor::from_trusted_bytes(&anchor.to_bytes().unwrap()).unwrap()
}
fn replace(control: &Directory, p: &FileShutdownPlan, edit: impl FnOnce(&mut codec::Restored)) {
    let plan = p.encode().unwrap();
    let mut state = codec::decode(&control.store(), p, &plan, MAX_JOURNAL_BYTES, 0, &control.image()).unwrap();
    edit(&mut state);
    let bytes = codec::encode(&control.store(), &plan, MAX_JOURNAL_BYTES,
        state.revision, &state.visits, &state.campaign).unwrap();
    codec::decode(&control.store(), p, &plan, MAX_JOURNAL_BYTES, 0, &bytes).unwrap();
    std::fs::write(control.store().join(storage::CANONICAL), bytes).unwrap();
}

#[test]
fn genuine_successors_and_exact_retries_preserve_original_effect_accounting() {
    for mode in 0..4 {
        let domain = effects::Directory::new(); let control = Directory::new();
        let mut ready = effects::Ready::new(&domain, effects::profile(), true);
        if mode > 0 { ready.dispatch(); }
        if mode == 2 { ready.host.publish(ready.host.revision(), 1).unwrap(); }
        let p = FileShutdownPlan::new(900, vec![ready.host.shutdown_domain(1).unwrap()], 1, MAX_SHUTDOWN_HEAD_BYTES).unwrap();
        drop(ready);
        let mut c = coordinator(&control, &p); let initial = imported(&c.history_anchor().unwrap());
        let observed = c.recover_and_drain(0, 1, ElapsedTick(if mode == 3 { 2000 } else { 2 })).unwrap().unwrap();
        let anchored = imported(&c.history_anchor_after(&initial).unwrap());
        let native = domain.bytes(); let bytes = control.image(); drop(c);
        let mut c = FileShutdownCoordinator::open_anchored(control.store(), &p, MAX_JOURNAL_BYTES, 2, &anchored).unwrap();
        assert!(!c.report().all_observed_drained());
        let refreshed = c.resolve_shutdown_visit(2, 0).unwrap();
        assert_eq!(refreshed.journal_revision, observed.journal_revision);
        let stop = refreshed.stop.unwrap();
        assert_eq!(stop.charged_units, if mode >= 2 { 16 } else { 0 });
        assert_eq!(stop.drained(), mode != 3);
        assert_eq!(control.image(), bytes); assert_eq!(domain.bytes(), native);
        assert_eq!(c.history_anchor_after(&anchored).unwrap(), anchored);
    }
}

#[test]
fn pending_resolution_is_a_legal_extension_but_a_completed_result_is_immutable() {
    let root = Directory::new(); let control = Directory::new(); let mut h = create(&root, 20);
    let p = plan(&h, 2); let mut c = coordinator(&control, &p);
    enter(&mut c); assert_eq!(c.history_anchor(), Err(JournalError::Unavailable)); drop(c);
    let mut c = reopen(&control, &p, 1); let pending = c.history_anchor().unwrap();
    h.stop_and_drain(h.revision(), request(&h, 900), ElapsedTick(2)).unwrap();
    let native = root.image(); c.resolve_shutdown_visit(1, 0).unwrap();
    let final_anchor = c.history_anchor_after(&pending).unwrap(); drop(c);
    let c = FileShutdownCoordinator::open_anchored(control.store(), &p, MAX_JOURNAL_BYTES, 1, &pending).unwrap();
    assert_eq!(c.revision(), 2); drop(c);
    replace(&control, &p, |state| state.visits[0].kind = ShutdownVisitKind::Advance { at: ElapsedTick(2) });
    let staged = control.store().join("delivery.pending"); std::fs::write(&staged, b"keep").unwrap();
    assert!(matches!(FileShutdownCoordinator::open_anchored(control.store(), &p, MAX_JOURNAL_BYTES, 2, &final_anchor),
        Err(JournalError::Contract(Error::Binding))));
    assert!(staged.exists()); assert_eq!(root.image(), native);
}

#[test]
fn counter_equal_valid_domain_forks_are_rejected_before_cleanup() {
    let root = Directory::new(); let control = Directory::new(); let mut h = create(&root, 20);
    let native_profile = h.profile.clone(); let p = plan(&h, 4);
    h.observe_time(h.revision(), ElapsedTick(2)).unwrap();
    let mut c = coordinator(&control, &p); c.inspect_canonical(0, 1).unwrap().unwrap();
    let anchor = c.history_anchor().unwrap(); drop(c); drop(h);
    replace(&control, &p, |state| {
        let slot = &mut state.campaign.slots[0];
        let mut events = journal::decode(&native_profile, &root.store(), &slot.head).unwrap();
        *events.last_mut().unwrap() = Event::Core(BaseEvent::Time(ElapsedTick(3)));
        Machine::replay(&native_profile, &events).unwrap();
        let fork = journal::encode(&native_profile, &root.store(), &events).unwrap();
        assert_eq!(fork.len(), slot.head.len()); slot.head = fork.into();
    });
    // The original plan and numeric floor alone intentionally cannot distinguish this fork.
    drop(FileShutdownCoordinator::open(control.store(), &p, MAX_JOURNAL_BYTES, 2).unwrap());
    let bytes = control.image(); let staged = control.store().join("delivery.pending");
    std::fs::write(&staged, b"not authoritative").unwrap();
    assert!(matches!(FileShutdownCoordinator::open_anchored(control.store(), &p, MAX_JOURNAL_BYTES, 2, &anchor),
        Err(JournalError::Contract(Error::Binding))));
    assert_eq!(control.image(), bytes); assert!(staged.exists());
}

#[test]
fn appended_visits_cannot_hide_rewritten_refusals_or_changed_intent_identity() {
    for mutation in 0..3 {
        let root = Directory::new(); let control = Directory::new(); let h = create(&root, 20);
        let p = plan(&h, 4); let mut c = coordinator(&control, &p);
        c.unavailable(0, 1, JournalError::Busy).unwrap(); let anchor = c.history_anchor().unwrap();
        c.unavailable(2, 1, JournalError::Unavailable).unwrap(); drop(c);
        replace(&control, &p, |state| match mutation {
            0 => state.visits[0].result = ShutdownVisitResult::Refused { diagnostic: "changed".into() },
            1 => state.visits[0].kind = ShutdownVisitKind::InspectCanonical,
            2 => { state.visits.remove(0); state.revision = 2; }
            _ => unreachable!(),
        });
        let bytes = control.image();
        assert!(FileShutdownCoordinator::open_anchored(control.store(), &p, MAX_JOURNAL_BYTES, 0, &anchor).is_err());
        assert_eq!(control.image(), bytes);
    }
}

#[test]
fn plan_path_capacity_and_revision_pins_apply_to_both_images() {
    let root = Directory::new(); let control = Directory::new(); let h = create(&root, 20);
    let p = plan(&h, 4); let c = coordinator(&control, &p); let anchor = c.history_anchor().unwrap(); drop(c);
    let bytes = control.image(); let staged = control.store().join("delivery.pending"); std::fs::write(&staged, b"keep").unwrap();
    assert!(FileShutdownCoordinator::open_anchored(control.store(), &p, MAX_JOURNAL_BYTES, 1, &anchor).is_err());
    assert!(FileShutdownCoordinator::open_anchored(control.store(), &p, MAX_JOURNAL_BYTES - 1, 0, &anchor).is_err());
    assert!(FileShutdownCoordinator::open_anchored(control.store(), &plan(&h, 5), MAX_JOURNAL_BYTES, 0, &anchor).is_err());
    assert!(staged.exists()); assert_eq!(control.image(), bytes);
    let other = Directory::new(); let c = coordinator(&other, &p); drop(c);
    assert!(FileShutdownCoordinator::open_anchored(other.store(), &p, MAX_JOURNAL_BYTES, 0, &anchor).is_err());
    let c = FileShutdownCoordinator::open_anchored(control.store(), &p, MAX_JOURNAL_BYTES, 0, &anchor).unwrap();
    assert_eq!(c.revision(), 0); assert!(!staged.exists()); assert_eq!(control.image(), bytes);
}

#[test]
fn ambiguous_writes_export_no_anchor_and_reopen_checks_the_actual_visible_successor() {
    for barrier in BARRIERS {
        let root = Directory::new(); let control = Directory::new(); let h = create(&root, 20);
        let p = plan(&h, 4); let mut c = coordinator(&control, &p); let anchor = c.history_anchor().unwrap();
        c.store.fail_once(barrier);
        assert!(c.unavailable(0, 1, JournalError::Busy).is_err());
        assert_eq!(c.history_anchor(), Err(JournalError::Unavailable));
        assert_eq!(c.history_anchor_after(&anchor), Err(JournalError::Unavailable)); drop(c);
        let c = FileShutdownCoordinator::open_anchored(control.store(), &p, MAX_JOURNAL_BYTES, 0, &anchor).unwrap();
        assert_eq!(c.revision(), u64::from(barrier == JournalIo::DirectorySync));
        assert!(!c.report().all_observed_drained()); c.history_anchor_after(&anchor).unwrap();
    }
}

#[test]
fn archived_transport_is_bounded_versioned_exact_and_debug_redacted() {
    let root = Directory::new(); let control = Directory::new(); let h = create(&root, 20);
    let p = plan(&h, 4); let mut c = coordinator(&control, &p); c.unavailable(0, 1, JournalError::Busy).unwrap();
    let anchor = c.history_anchor().unwrap(); let bytes = anchor.to_bytes().unwrap();
    assert_eq!(imported(&anchor), anchor);
    for end in 0..bytes.len() { assert!(FileShutdownCoordinatorAnchor::from_trusted_bytes(&bytes[..end]).is_err()); }
    let mut invalid = bytes.clone(); invalid.push(0);
    assert!(FileShutdownCoordinatorAnchor::from_trusted_bytes(&invalid).is_err());
    invalid = bytes.clone(); invalid[7] = b'2';
    assert_eq!(FileShutdownCoordinatorAnchor::from_trusted_bytes(&invalid), Err(Error::Binding));
    invalid = bytes.clone(); invalid[8..16].copy_from_slice(&u64::MAX.to_be_bytes());
    assert_eq!(FileShutdownCoordinatorAnchor::from_trusted_bytes(&invalid), Err(Error::Limit));
    let debug = format!("{anchor:?}"); assert!(!debug.contains("Busy")); assert!(!debug.contains("shutdown-fixture"));
    // Framing alone does not establish the wrapper's asserted revision.
    invalid = bytes; invalid[8..16].copy_from_slice(&1_u64.to_be_bytes());
    let wrong = FileShutdownCoordinatorAnchor::from_trusted_bytes(&invalid).unwrap(); drop(c);
    assert!(FileShutdownCoordinator::open_anchored(control.store(), &p, MAX_JOURNAL_BYTES, 0, &wrong).is_err());
}

#[test]
fn custody_cannot_be_advanced_from_an_older_healthy_owner() {
    let root = Directory::new(); let control = Directory::new(); let h = create(&root, 20);
    let p = plan(&h, 4); let mut c = coordinator(&control, &p); let old = control.image();
    c.unavailable(0, 1, JournalError::Busy).unwrap(); let newer = c.history_anchor().unwrap(); drop(c);
    std::fs::write(control.store().join(storage::CANONICAL), old).unwrap();
    let c = FileShutdownCoordinator::open(control.store(), &p, MAX_JOURNAL_BYTES, 0).unwrap();
    assert_eq!(c.history_anchor_after(&newer), Err(Error::Stale.into())); drop(c);
    assert!(matches!(FileShutdownCoordinator::open_anchored(control.store(), &p, MAX_JOURNAL_BYTES, 0, &newer),
        Err(JournalError::Contract(Error::Stale))));
}
