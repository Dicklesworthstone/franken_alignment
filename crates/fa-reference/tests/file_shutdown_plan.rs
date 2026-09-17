//! Portable fixed-roster shutdown plans are not recovered approvals.
#![cfg(unix)]
#[path = "support/file_shutdown.rs"] mod support;
use support::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::shutdown::*;
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::Error;
use std::os::unix::ffi::OsStrExt;

fn members(roots: &[(&Directory, u64)]) -> Vec<FileShutdownMember> {
    roots.iter().map(|(root, id)| FileShutdownMember { id: *id,
        directory: root.store(), profile: profile(*id) }).collect()
}

#[test]
fn restored_roster_retains_offline_members_and_drives_original_recovered_owners() {
    let first = Directory::new(); let second = Directory::new();
    let (a, _) = create(&first, 1); let (b, _) = create(&second, 2);
    let encoded = plan(vec![b.shutdown_domain(2).unwrap(), a.shutdown_domain(1).unwrap()]).encode().unwrap();
    drop(a); drop(b);
    let offline = second.0.join("offline"); std::fs::rename(second.store(), &offline).unwrap();
    let restored = FileShutdownPlan::decode(&encoded, members(&[(&second, 2), (&first, 1)])).unwrap();
    assert_eq!(restored.encode().unwrap(), encoded);
    let mut campaign = restored.start();
    assert_eq!(campaign.report().unobserved_stops(), vec![1, 2]);
    assert!(!campaign.report().all_observed_stopped());
    assert!(campaign.inspect_canonical(2).is_err());
    let (mut a, _) = FileOversight::open(first.store(), profile(1)).unwrap();
    campaign.advance(1, &mut a, ElapsedTick(2)).unwrap();
    campaign.advance(1, &mut a, ElapsedTick(2)).unwrap();
    assert_eq!(campaign.report().unobserved_stops(), vec![2]);
    assert!(!campaign.report().all_observed_drained());
    std::fs::rename(offline, second.store()).unwrap();
    let (mut b, _) = FileOversight::open(second.store(), profile(2)).unwrap();
    campaign.advance(2, &mut b, ElapsedTick(2)).unwrap();
    campaign.advance(2, &mut b, ElapsedTick(2)).unwrap();
    assert!(campaign.report().all_observed_drained());
    assert_eq!(campaign.report().domains.len(), 2);
}

#[test]
fn exact_manual_wire_vector_truncation_suffix_and_bootstrap_bindings() {
    let root = Directory::new(); let (h, _) = create(&root, 1);
    let registered = h.shutdown_domain(1).unwrap(); let anchor = std::fs::read(root.store().join("delivery.bin")).unwrap();
    let plan = FileShutdownPlan::new(900, vec![registered], 7, MAX_SHUTDOWN_HEAD_BYTES).unwrap();
    let encoded = plan.encode().unwrap();
    let mut expected = b"FASHPLN\x01".to_vec();
    expected.extend_from_slice(&900_u64.to_be_bytes());
    for n in [7_u32, MAX_SHUTDOWN_HEAD_BYTES as u32, 1] { expected.extend_from_slice(&n.to_be_bytes()); }
    expected.extend_from_slice(&1_u64.to_be_bytes());
    let path = root.store(); let path = path.as_os_str().as_bytes();
    expected.extend_from_slice(&(path.len() as u32).to_be_bytes()); expected.extend_from_slice(path);
    expected.extend_from_slice(&(h.revision() as u32).to_be_bytes());
    expected.extend_from_slice(&(anchor.len() as u32).to_be_bytes()); expected.extend_from_slice(&anchor);
    assert_eq!(encoded, expected);
    for end in 0..encoded.len() {
        assert!(FileShutdownPlan::decode(&encoded[..end], members(&[(&root, 1)])).is_err(), "truncated at {end}");
    }
    let mut suffix = encoded.clone(); suffix.push(0);
    assert!(FileShutdownPlan::decode(&suffix, members(&[(&root, 1)])).is_err());
    let mut bad = members(&[(&root, 1)]); bad[0].profile.delivery.total += 1;
    assert!(FileShutdownPlan::decode(&encoded, bad).is_err());
    assert!(FileShutdownPlan::decode(&encoded, Vec::new()).is_err());
    assert!(FileShutdownPlan::decode(&encoded, members(&[(&root, 1), (&root, 1)])).is_err());
    assert!(FileShutdownPlan::decode(&encoded, members(&[(&root, 2)])).is_err());
    let decoded = FileShutdownPlan::decode(&encoded, members(&[(&root, 1)])).unwrap();
    assert_eq!(decoded.max_attempts(), 7); assert_eq!(decoded.domains()[0].clock_domain(), 101);
    assert!(h.storage_failure().is_none());
}

#[test]
fn restored_plan_does_not_recover_old_success_or_accept_invalid_canonical_evidence() {
    let root = Directory::new(); let (mut h, _) = create(&root, 1);
    let initial = plan(vec![h.shutdown_domain(1).unwrap()]);
    let bytes = initial.encode().unwrap(); let mut campaign = initial.start();
    campaign.advance(1, &mut h, ElapsedTick(1)).unwrap();
    campaign.advance(1, &mut h, ElapsedTick(1)).unwrap();
    assert!(campaign.report().all_observed_drained());
    let mut fresh = FileShutdownPlan::decode(&bytes, members(&[(&root, 1)])).unwrap().start();
    assert!(!fresh.report().all_observed_drained());
    fresh.inspect_canonical(1).unwrap(); assert!(fresh.report().all_observed_drained());
    // The native canonical decoder rejects an altered bootstrap. No restored
    // plan can convert corrupt bytes into an acknowledged owner or a stop.
    let canonical = root.store().join("delivery.bin"); let original = std::fs::read(&canonical).unwrap();
    let mut corrupt = original.clone(); corrupt[0] ^= 1; std::fs::write(&canonical, &corrupt).unwrap();
    assert!(fresh.inspect_canonical(1).is_err()); assert!(!fresh.report().all_observed_stopped());
    std::fs::write(canonical, original).unwrap(); fresh.inspect_canonical(1).unwrap();
    assert!(fresh.report().all_observed_drained());
}

#[test]
fn restored_limits_remain_effective_and_plan_loading_never_cleans_staging() {
    let root = Directory::new(); let (mut h, _) = create(&root, 1);
    let plan = FileShutdownPlan::new(17, vec![h.shutdown_domain(1).unwrap()], 1, MAX_SHUTDOWN_HEAD_BYTES).unwrap();
    let bytes = plan.encode().unwrap(); let pending = root.store().join("delivery.pending");
    std::fs::write(&pending, b"independent pending evidence").unwrap();
    let restored = FileShutdownPlan::decode(&bytes, members(&[(&root, 1)])).unwrap();
    assert_eq!(std::fs::read(&pending).unwrap(), b"independent pending evidence");
    std::fs::remove_file(pending).unwrap();
    let mut campaign = restored.start(); campaign.advance(1, &mut h, ElapsedTick(1)).unwrap();
    assert!(campaign.report().all_observed_stopped()); assert!(!campaign.report().all_observed_drained());
    assert_eq!(campaign.advance(1, &mut h, ElapsedTick(1)), Err(JournalError::Contract(Error::Limit)));
}
