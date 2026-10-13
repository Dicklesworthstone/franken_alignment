//! Offline registrations feed the ORIGINAL shutdown coordinator, not saved stops.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod fixture;
use fixture::{Directory, profile};
use fa_reference::Error;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::{MAX_JOURNAL_BYTES, JournalError};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight,
    guarded::anchored::{FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES},
    shutdown::{FileShutdownDomain, FileShutdownMember, FileShutdownPlan, MAX_SHUTDOWN_HEAD_BYTES,
        coordinator::FileShutdownCoordinator}};

fn plan(domain: FileShutdownDomain, max_bytes: usize) -> FileShutdownPlan {
    FileShutdownPlan::new(900, vec![domain], 8, max_bytes).unwrap()
}
fn transport(anchor: &FileHistoryAnchor) -> Vec<u8> {
    let mut bytes = Vec::new();
    anchor.write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES).unwrap();
    bytes
}
fn imported(bytes: &[u8]) -> FileHistoryAnchor {
    FileHistoryAnchor::read_trusted(&mut std::io::Cursor::new(bytes), MAX_HISTORY_ANCHOR_BYTES).unwrap()
}

#[test]
fn offline_registration_is_byte_identical_and_keeps_an_unreachable_member() {
    let root = Directory::new(); let control = Directory::new();
    let (host, reviewer) = fixture::create(&root);
    let direct = host.shutdown_domain(17).unwrap(); let member = direct.member();
    let anchor = imported(&transport(&host.history_anchor().unwrap()));
    let original_plan = plan(direct, MAX_SHUTDOWN_HEAD_BYTES).encode().unwrap();
    drop(reviewer); drop(host);
    let hidden = root.0.join("offline"); std::fs::rename(root.store(), &hidden).unwrap();
    let restored = FileShutdownDomain::from_history_anchor(member, &anchor).unwrap();
    assert_eq!(restored.registered_revision(), anchor.revision());
    let p = plan(restored, MAX_SHUTDOWN_HEAD_BYTES);
    assert_eq!(p.encode().unwrap(), original_plan);
    let mut coordinator = FileShutdownCoordinator::create(control.store(), &p, MAX_JOURNAL_BYTES).unwrap();
    assert!(coordinator.recover_and_drain(0, 17, ElapsedTick(2)).unwrap().is_err());
    assert!(!coordinator.report().all_observed_drained());
    assert_eq!(coordinator.report().current_session.unobserved_stops(), vec![17]);
    std::fs::rename(&hidden, root.store()).unwrap();
    coordinator.recover_and_drain(coordinator.revision(), 17, ElapsedTick(2)).unwrap().unwrap();
    assert!(coordinator.report().all_observed_drained());
}

#[test]
fn anchored_successors_recover_original_reserved_sent_executed_and_unknown_liabilities() {
    for mode in 0..4 {
        let root = Directory::new(); let control = Directory::new();
        let (mut host, reviewer) = fixture::create(&root);
        let keys = fixture::ready(&mut host, &reviewer, 1, b"only once");
        let member = host.shutdown_domain(1).unwrap().member();
        let anchor = host.history_anchor().unwrap();
        if mode > 0 { fixture::dispatch(&mut host, &keys); }
        if mode == 2 { host.publish(host.revision(), 1).unwrap(); }
        let before = host.inspect(); drop(keys); drop(reviewer); drop(host);
        let p = plan(FileShutdownDomain::from_history_anchor(member, &anchor).unwrap(), MAX_SHUTDOWN_HEAD_BYTES);
        let mut coordinator = FileShutdownCoordinator::create(control.store(), &p, MAX_JOURNAL_BYTES).unwrap();
        let tick = ElapsedTick(if mode == 3 { 2000 } else { 2 });
        let observed = coordinator.recover_and_drain(0, 1, tick).unwrap().unwrap();
        assert_eq!(observed.executions, u64::from(mode == 2));
        assert_eq!(observed.stop.as_ref().unwrap().drained(), mode != 3);
        let after = FileOversight::read_publication(root.store(), &profile()).unwrap();
        assert_eq!(after.revision, before.revision + 3);
        assert_eq!(after.control.ledger.reserved, 0);
        assert_eq!(after.control.ledger.charged, if mode >= 2 { 16 } else { 0 });
        let bytes = std::fs::read(root.store().join("delivery.bin")).unwrap();
        drop(coordinator);
        let mut resumed = FileShutdownCoordinator::open(control.store(), &p, MAX_JOURNAL_BYTES, 2).unwrap();
        assert!(!resumed.report().all_observed_drained());
        resumed.resolve_shutdown_visit(2, 0).unwrap();
        assert_eq!(resumed.report().all_observed_drained(), mode != 3);
        assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), bytes);
    }
}

#[test]
fn foreign_profiles_paths_and_transport_only_revision_assertions_cannot_register() {
    let root = Directory::new(); let (host, _) = fixture::create(&root);
    let member = host.shutdown_domain(1).unwrap().member();
    let anchor = host.history_anchor().unwrap();
    let before = std::fs::read(root.store().join("delivery.bin")).unwrap();
    let mut foreign = member.clone(); foreign.profile.delivery.total += 1;
    assert!(matches!(FileShutdownDomain::from_history_anchor(foreign, &anchor), Err(Error::Binding)));
    let mut foreign = member.clone(); foreign.directory = root.0.join("another");
    assert!(matches!(FileShutdownDomain::from_history_anchor(foreign, &anchor), Err(Error::Binding)));
    let mut foreign = member.clone(); foreign.directory = "relative".into();
    assert!(matches!(FileShutdownDomain::from_history_anchor(foreign, &anchor), Err(Error::InvalidInput)));
    let mut foreign = member.clone(); foreign.id = 0;
    assert!(matches!(FileShutdownDomain::from_history_anchor(foreign, &anchor), Err(Error::InvalidInput)));
    let mut bytes = transport(&anchor);
    // Anchor transport checks bounds, not the embedded journal's event count.
    bytes[8..16].copy_from_slice(&0_u64.to_le_bytes());
    assert!(matches!(FileShutdownDomain::from_history_anchor(member.clone(), &imported(&bytes)), Err(Error::Binding)));
    let mut bytes = transport(&anchor); bytes[24] ^= 1;
    assert!(FileShutdownDomain::from_history_anchor(member, &imported(&bytes)).is_err());
    assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), before);
}

#[test]
fn new_plan_preserves_path_roster_and_total_evidence_bounds() {
    let root = Directory::new(); let (host, _) = fixture::create(&root);
    let member = host.shutdown_domain(1).unwrap().member();
    let anchor = host.history_anchor().unwrap();
    let mut oversized = member.clone(); oversized.directory = format!("/{}", "x".repeat(4096)).into();
    assert!(matches!(FileShutdownDomain::from_history_anchor(oversized, &anchor), Err(Error::Limit)));
    let registration = FileShutdownDomain::from_history_anchor(member, &anchor).unwrap();
    let size = registration.retained_anchor_bytes();
    assert!(matches!(FileShutdownPlan::new(900, vec![registration.clone()], 8, size - 1), Err(Error::Limit)));
    assert_eq!(plan(registration.clone(), size).domains().len(), 1);
    assert!(matches!(FileShutdownPlan::new(900, vec![registration.clone(), registration], 8, size * 2), Err(Error::Duplicate)));
}

#[test]
fn initial_zero_event_anchor_is_valid_without_claiming_a_stopped_domain() {
    let root = Directory::new();
    let (host, _) = FileOversight::create(root.store(), profile()).unwrap();
    let member = FileShutdownMember { id: 1, directory: root.store(), profile: profile() };
    let anchor = host.history_anchor().unwrap(); assert_eq!(anchor.revision(), 0);
    let registered = FileShutdownDomain::from_history_anchor(member, &anchor).unwrap();
    let p = plan(registered, MAX_SHUTDOWN_HEAD_BYTES);
    let campaign = p.start();
    assert!(!campaign.report().all_observed_stopped());
    assert_eq!(host.revision(), 0);
}

#[test]
fn newest_independent_anchor_prevents_recovery_from_an_older_valid_image() {
    let root = Directory::new(); let control = Directory::new();
    let (mut host, reviewer) = fixture::create(&root);
    let old = std::fs::read(root.store().join("delivery.bin")).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let current = std::fs::read(root.store().join("delivery.bin")).unwrap();
    let anchor = host.history_anchor().unwrap(); let member = host.shutdown_domain(1).unwrap().member();
    drop(reviewer); drop(host);
    let p = plan(FileShutdownDomain::from_history_anchor(member, &anchor).unwrap(), MAX_SHUTDOWN_HEAD_BYTES);
    let mut coordinator = FileShutdownCoordinator::create(control.store(), &p, MAX_JOURNAL_BYTES).unwrap();
    std::fs::write(root.store().join("delivery.bin"), &old).unwrap();
    assert_eq!(coordinator.recover_and_drain(0, 1, ElapsedTick(3)).unwrap(), Err(JournalError::Contract(Error::Stale)));
    assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), old);
    std::fs::write(root.store().join("delivery.bin"), &current).unwrap();
    coordinator.recover_and_drain(coordinator.revision(), 1, ElapsedTick(3)).unwrap().unwrap();
    assert!(coordinator.report().all_observed_drained());
}
