use super::*;
use fa_reference::action::consequence::oversight::actor_wire::decode_command;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const TEMPLATE: &[u8] = include_bytes!("../../fixtures/supervised_publication.json");

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fa-proposal-next-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn config(&self) -> Config {
        let mut config = Config::decode(TEMPLATE).unwrap();
        config.store = self.0.join("store");
        config
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) {
            eprintln!("proposal fixture cleanup: {error:?}");
        }
    }
}

fn decoded(bytes: &[u8]) -> (u64, ActorProposal) {
    let Command::Submit { request, proposal } = decode_command(bytes).unwrap() else {
        panic!("proposal builder emitted a non-submit command");
    };
    (request, proposal)
}

#[test]
fn bootstrap_preserves_binary_payload_target_and_original_epoch() {
    let root = Directory::new();
    let config = root.config();
    let payload = vec![0, 255, b'\n', b'"', b'\\'];
    let bytes = document(&config, 41, payload.clone(), 1, ElapsedTick(10), Basis::Bootstrap).unwrap();
    let (request, proposal) = decoded(&bytes);
    assert_eq!(request, 41);
    assert_eq!(proposal.payload, payload);
    assert_eq!(proposal.units, 5);
    assert_eq!(proposal.target, config.profile.delivery.target);
    assert_eq!(proposal.expected_policy_epoch, 0);
    assert_eq!(proposal.deadline, ElapsedTick(11));
    assert!(!config.store.exists());
}

#[test]
fn empty_payload_still_costs_one_unit() {
    let root = Directory::new();
    let bytes = document(&root.config(), 1, Vec::new(), 1, ElapsedTick(10), Basis::Bootstrap).unwrap();
    let (_, proposal) = decoded(&bytes);
    assert!(proposal.payload.is_empty());
    assert_eq!(proposal.units, 1);
}

#[test]
fn existing_proposal_reads_without_fencing_and_matches_next_owner_open() {
    let root = Directory::new();
    let config = root.config();
    drop(FileOversight::create(&config.store, config.profile.clone()).unwrap());
    let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    let bytes = document(&config, 42, b"next".to_vec(), 1, ElapsedTick(10), Basis::Existing).unwrap();
    let (_, proposal) = decoded(&bytes);
    assert_eq!(proposal.target, before.target);
    assert_eq!(proposal.expected_policy_epoch, before.control.ledger.epoch + 1);
    assert_eq!(FileOversight::read_publication(&config.store, &config.profile).unwrap(), before);
    assert_eq!(document(&config, 42, b"next".to_vec(), 1, ElapsedTick(10), Basis::Existing).unwrap(), bytes);
    drop(FileOversight::open(&config.store, config.profile.clone()).unwrap());
    let opened = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(opened.control.ledger.epoch, proposal.expected_policy_epoch);
    assert_eq!(opened.control.ledger.charged, before.control.ledger.charged);
    assert_eq!(opened.executions, before.executions);
    assert_eq!(opened.payload, before.payload);
}

#[test]
fn construction_tracks_intervening_fences_instead_of_reusing_epoch_zero() {
    let root = Directory::new();
    let config = root.config();
    drop(FileOversight::create(&config.store, config.profile.clone()).unwrap());
    let (_, first) = decoded(&document(&config, 1, b"next".to_vec(), 1, ElapsedTick(10), Basis::Existing).unwrap());
    drop(FileOversight::open(&config.store, config.profile.clone()).unwrap());
    let (_, second) = decoded(&document(&config, 1, b"next".to_vec(), 1, ElapsedTick(10), Basis::Existing).unwrap());
    assert_eq!(second.expected_policy_epoch, first.expected_policy_epoch + 1);
    assert_eq!(first.target, second.target);
    assert_eq!(first.payload, second.payload);
}

#[test]
fn existing_mode_never_bootstraps_an_absent_store() {
    let root = Directory::new();
    let config = root.config();
    assert!(document(&config, 1, b"next".to_vec(), 1, ElapsedTick(10), Basis::Existing).is_err());
    assert!(!config.store.exists());
}

#[test]
fn invalid_identity_ttl_size_and_arithmetic_fail_before_store_access() {
    let root = Directory::new();
    let config = root.config();
    for basis in [Basis::Bootstrap, Basis::Existing] {
        assert!(document(&config, 0, vec![1], 1, ElapsedTick(10), basis).is_err());
        assert!(document(&config, 1, vec![1], 0, ElapsedTick(10), basis).is_err());
        assert!(document(&config, 1, vec![1], config.timing.runtime_ms + 1, ElapsedTick(10), basis).is_err());
        assert!(document(&config, 1, vec![1], 1, ElapsedTick(u64::MAX), basis).is_err());
        assert!(document(&config, 1, vec![0; MAX_PAYLOAD_BYTES + 1], 1, ElapsedTick(10), basis).is_err());
    }
    assert!(!config.store.exists());
}

#[test]
fn changed_bootstrap_profile_cannot_be_used_to_construct_retained_work() {
    let root = Directory::new();
    let config = root.config();
    drop(FileOversight::create(&config.store, config.profile.clone()).unwrap());
    let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    let mut changed = root.config();
    changed.profile.delivery.total += 1;
    assert!(document(&changed, 1, b"next".to_vec(), 1, ElapsedTick(10), Basis::Existing).is_err());
    assert_eq!(FileOversight::read_publication(&config.store, &config.profile).unwrap(), before);
}
