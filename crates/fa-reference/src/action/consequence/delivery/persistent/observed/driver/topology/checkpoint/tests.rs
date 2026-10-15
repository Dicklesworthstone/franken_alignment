//! Real native source/recovery custody; transport validity is not authentication.
use super::*;
use crate::action::{ElapsedTick, Purpose, ResolvedTarget, Scope};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalIo, JournalLimits, RecoveryReserve};
use crate::action::consequence::delivery::persistent::observed::{FileOversightProfile,
    guarded::{FileGuardSet, FileRecoveryRequirements, mediated::{FileMediatedBootstrap,
        FileMediatedRequirements, FileTopologyRequirement}, recovery::FileGuardedRecoveryStatus},
    mediation::source::TopologySourceRecovery};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract, human::HumanReviewPolicy};
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use std::{collections::BTreeMap, fs, io::Cursor, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

const INVENTORY: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/authority_topology_inventory.json"));
struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("checkpoint cleanup: {error}"); } }
}
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile { delivery: FileDeliveryProfile {
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        total: 100, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 1,
            tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::AuditOnly },
            Vec::new(), vec![1], vec![2], 0).unwrap(),
        suspend_at_incident: 100, policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
        congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".into(),
            MemberPolicy { cohort: "one".into(), weight: 1 })]), caps: Caps { per_member: 1, per_cohort: 1 },
            continue_minimum: 1, continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3,
            minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
        retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
    }, committee: CommitteeContract::new(BTreeMap::from([("reviewer".into(),
        HelperContract::new(InputProfileBinding { profile_id: 1, profile_bytes: b"fixture".to_vec(),
            tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 } }
}
fn guards() -> FileGuardSet {
    FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None,
        identity: None, campaigns: None, credential: None }
}
fn document(generation: u64) -> String {
    format!(r#"{{"schema":"fa.authority-topology/1","generation":{generation},"inventory_generation":{generation},
      "complete_inventory_generation":{generation},
      "scope":{{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5,"purpose":"effect"}},
      "target":{{"adapter":10,"object":11,"contract_version":1,"expected_version":1,"generation":1}},
      "inventory_scope":{{"tenant":1,"principal":2,"purpose":1}},"family":"file-publication",
      "nodes":[{{"id":1,"kind":"actor"}},{{"id":2,"kind":"enforcer"}},{{"id":4,"kind":"sink"}}],
      "edges":[{{"id":1,"from":1,"to":2,"channel":"dispatch","route":"publication","provenance":101}},
        {{"id":2,"from":2,"to":4,"channel":"dispatch","route":"publication","provenance":102}}],
      "enforcers":[{{"node":2,"adapter":10,"contract_version":1,"generation":1,"provenance":102}}],
      "gates":[2],"check_edge_budget":65536,"source_limits":{{"checks":32,"read_bytes":67108928}}}}"#)
}
fn fixture() -> (Directory, FileOversight, FileDriverTopology, String) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let root = Directory(std::env::temp_dir().join(format!("fa-checkpoint-{}-{stamp}-{}",
        std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))));
    fs::create_dir(&root.0).unwrap();
    let document = document(1);
    fs::write(root.0.join("topology.json"), &document).unwrap();
    fs::write(root.0.join("inventory.json"), INVENTORY).unwrap();
    let loaded = load(document.as_bytes(), INVENTORY).unwrap();
    let (mut host, roles) = FileOversight::create_mediated_guarded_reserved(root.0.join("store"),
        profile(), &guards(), None, FileMediatedBootstrap { topology: loaded.graph().clone(),
            reserve: RecoveryReserve::terminal(), publication: None }, None, None).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let source = roles.topology_observer.bind_topology_source(&host, host.revision(),
        root.0.join("topology.json"), root.0.join("inventory.json"), loaded).unwrap();
    (root, host, FileDriverTopology::new(source), document)
}
fn archive(checkpoint: &TopologyCheckpoint) -> Vec<u8> {
    let mut bytes = Vec::new();
    assert_eq!(checkpoint.write_to(&mut bytes, MAX_TOPOLOGY_CHECKPOINT_BYTES).unwrap(), bytes.len());
    bytes
}
fn refresh(source: &mut FileDriverTopology, host: &mut FileOversight) {
    let revision = host.revision();
    source.refresh(host, revision, None).unwrap();
}

#[test]
fn capture_roundtrip_preserves_actual_work_and_exact_acknowledged_history_without_io() {
    let (root, mut host, mut source, document) = fixture();
    for checks in 0..=1 {
        if checks != 0 { refresh(&mut source, &mut host); }
        let before = fs::read(root.0.join("store/delivery.bin")).unwrap();
        let revision = host.revision(); let work = source.source().work();
        let checkpoint = source.checkpoint(&host, document.as_bytes(), INVENTORY).unwrap();
        assert_eq!(checkpoint.history_anchor(), &host.history_anchor().unwrap());
        assert_eq!(checkpoint.work(), work); assert_eq!(work.checks, checks);
        assert_eq!(checkpoint.last_operation(), source.source().last_operation());
        assert_eq!(checkpoint.minimum().journal_revision, revision);
        assert_eq!(checkpoint.minimum().authority_epoch, host.inspect().control.ledger.epoch);
        let bytes = archive(&checkpoint);
        let restored = TopologyCheckpoint::read_trusted(&mut Cursor::new(&bytes), bytes.len()).unwrap();
        assert_eq!(restored, checkpoint);
        assert_eq!(host.revision(), revision); assert_eq!(source.source().work(), work);
        assert_eq!(fs::read(root.0.join("store/delivery.bin")).unwrap(), before);
        assert!(!format!("{checkpoint:?}").contains("file-owner"));
    }
}

#[test]
fn independent_archive_drives_real_recovery_and_preserves_source_spend() {
    let (root, mut host, mut source, old) = fixture();
    refresh(&mut source, &mut host);
    let saved = archive(&source.checkpoint(&host, old.as_bytes(), INVENTORY).unwrap());
    drop(source); drop(host);
    let saved = TopologyCheckpoint::read_trusted(&mut Cursor::new(saved), MAX_TOPOLOGY_CHECKPOINT_BYTES).unwrap();
    let expected = FileMediatedRequirements { oversight: FileRecoveryRequirements { guards: guards(),
        effective_policy: profile().delivery.policy, credential_epoch: None, minimum: saved.minimum() },
        topology: FileTopologyRequirement { initial: saved.expected().graph().clone(),
            current: saved.expected().graph().clone(), available: saved.available() }, prediction: None, evaluation: None };
    let mut run = FileOversight::begin_open_mediated_guarded_anchored(root.0.join("store"), profile(),
        &expected, saved.history_anchor()).unwrap().require_recovery_reserve(RecoveryReserve::terminal()).unwrap();
    while run.progress().status == FileGuardedRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 1).unwrap();
    }
    let (mut host, roles) = run.finish().unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let next = document(2); fs::write(root.0.join("topology.json"), &next).unwrap();
    let revision = host.revision();
    let source = roles.topology_observer.resume_topology_source(&mut host, revision,
        saved.last_operation() + 1, TopologySourceRecovery { topology_path: root.0.join("topology.json"),
            inventory_path: root.0.join("inventory.json"), previous: saved.expected().clone(),
            next: load(next.as_bytes(), INVENTORY).unwrap(), work: saved.work(), last_operation: saved.last_operation() }).unwrap();
    let mut source = FileDriverTopology::new(source);
    assert!(host.mediation_snapshot().unwrap().accepted.is_none());
    refresh(&mut source, &mut host);
    let continued = source.checkpoint(&host, next.as_bytes(), INVENTORY).unwrap();
    assert_eq!(continued.work().checks, saved.work().checks + 1);
    assert_eq!(continued.work().reserved_read_bytes, 2 * TOPOLOGY_REFRESH_READ_BYTES);
    assert_eq!(continued.last_operation(), saved.last_operation() + 2);
    assert!(continued.available()); assert_eq!(host.inspect().executions, 0);
}

#[test]
fn acknowledged_loss_keeps_spent_work_but_foreign_poisoned_and_changed_input_capture_refuse() {
    let (root, mut host, mut source, document) = fixture();
    let (_other_root, other, _, _) = fixture();
    assert!(source.checkpoint(&other, document.as_bytes(), INVENTORY).is_err());
    assert!(source.checkpoint(&host, document.replace("65536", "65535").as_bytes(), INVENTORY).is_err());
    fs::remove_file(root.0.join("inventory.json")).unwrap();
    refresh(&mut source, &mut host);
    let lost = source.checkpoint(&host, document.as_bytes(), INVENTORY).unwrap();
    assert_eq!(lost.work().checks, 1); assert!(!lost.available());
    let (_root, mut host, mut source, document) = fixture();
    host.store.fail_once(JournalIo::DirectorySync);
    let revision = host.revision();
    assert!(source.refresh(&mut host, revision, None).is_err());
    assert!(source.checkpoint(&host, document.as_bytes(), INVENTORY).is_err());
}

#[test]
fn framing_limits_truncation_and_trailing_data_never_import_partial_custody() {
    let (_root, host, source, document) = fixture();
    let checkpoint = source.checkpoint(&host, document.as_bytes(), INVENTORY).unwrap();
    let bytes = archive(&checkpoint);
    let mut output = Vec::new();
    assert!(checkpoint.write_to(&mut output, bytes.len() - 1).is_err()); assert!(output.is_empty());
    for cut in 0..bytes.len() {
        assert!(TopologyCheckpoint::read_trusted(&mut Cursor::new(&bytes[..cut]), bytes.len()).is_err(), "cut {cut}");
    }
    for (offset, value) in [(48, u64::MAX), (56, 0), (64, MAX_HISTORY_ANCHOR_BYTES as u64 + 1)] {
        let mut bad = bytes.clone(); bad[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        let mut input = Cursor::new(bad);
        assert!(TopologyCheckpoint::read_trusted(&mut input, bytes.len()).is_err());
        assert_eq!(input.position(), HEADER_BYTES as u64);
    }
    for index in [0, 72] {
        let mut bad = bytes.clone(); bad[index] = 255;
        assert!(TopologyCheckpoint::read_trusted(&mut Cursor::new(bad), bytes.len()).is_err());
    }
    let mut bad = bytes.clone(); bad[24..32].copy_from_slice(&1_u64.to_le_bytes());
    assert!(TopologyCheckpoint::read_trusted(&mut Cursor::new(bad), bytes.len()).is_err());
    let mut bad = bytes.clone(); bad.push(0);
    assert!(TopologyCheckpoint::read_trusted(&mut Cursor::new(bad), bytes.len()).is_err());
}

#[test]
fn fragmented_interrupted_transport_and_failed_output_keep_original_owner_unchanged() {
    struct Fragmented { input: Cursor<Vec<u8>>, interrupted: bool }
    impl Read for Fragmented {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            if !self.interrupted { self.interrupted = true; return Err(io::ErrorKind::Interrupted.into()); }
            let count = out.len().min(1); self.input.read(&mut out[..count])
        }
    }
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
        fn flush(&mut self) -> io::Result<()> { panic!("caller owns flushing") }
    }
    let (_root, host, source, document) = fixture();
    let checkpoint = source.checkpoint(&host, document.as_bytes(), INVENTORY).unwrap();
    let mut input = Fragmented { input: Cursor::new(archive(&checkpoint)), interrupted: false };
    assert_eq!(TopologyCheckpoint::read_trusted(&mut input, MAX_TOPOLOGY_CHECKPOINT_BYTES).unwrap(), checkpoint);
    assert_eq!(checkpoint.write_to(&mut Broken, MAX_TOPOLOGY_CHECKPOINT_BYTES),
        Err(FileHistoryAnchorError::Io(io::ErrorKind::BrokenPipe)));
    assert_eq!(source.checkpoint(&host, document.as_bytes(), INVENTORY).unwrap(), checkpoint);
}
