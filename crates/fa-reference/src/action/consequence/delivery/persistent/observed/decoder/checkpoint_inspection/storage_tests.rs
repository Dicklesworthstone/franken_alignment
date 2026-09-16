//! Original Store fault barriers, real numerical replay and no replacement seam.
use super::*;
use super::super::checkpoint::FileDecoderCheckpoint;
use super::super::super::containment::FileResetRequest;
use super::super::super::source::FileSourcePolicy;
use crate::action::{ActionSpec, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::activation::{HEADER_BYTES, monitor::decoder::MonitoringStatus};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalIo, JournalLimits};
use crate::action::consequence::gate::{ReviewBinding, containment::{ActorState, RestartGrade, RestartProfile}};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract, human::HumanReviewPolicy};
use crate::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use crate::action::consequence::oversight::policy_state::{StateFreshness, StateLimits, StateSource};
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use crate::Snapshot;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync];
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-cp-{}-{now}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory { fn drop(&mut self) { if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("checkpoint cleanup: {error}"); } } }
fn host_profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile { delivery: FileDeliveryProfile {
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        total: 100, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 1,
            tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
        suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
        congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".into(), MemberPolicy { cohort: "one".into(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
            narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(), retention_ticks: 1000,
        max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
    }, committee: CommitteeContract::new(BTreeMap::from([("reviewer".into(), HelperContract::new(InputProfileBinding {
        profile_id: 1, profile_bytes: b"checkpoint-test".to_vec(), tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1,
    }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 } }
}
fn limited_config() -> FileDecoderConfig {
    let monitor = String::from_utf8(data::monitor(3.0)).unwrap().replace("\"encoded_bytes\":10000",
        &format!("\"encoded_bytes\":{}", HEADER_BYTES + 8));
    FileDecoderConfig::new(profile(), data::weights(), monitor.into_bytes(), data::sampling(), 5, DecoderBindingLimits::default()).unwrap()
}
fn owner(root: &Directory, configuration: &FileDecoderConfig) -> FileOversight {
    let (mut host, _) = FileOversight::create(root.store(), host_profile()).unwrap();
    host.enable_decoder(host.revision(), configuration.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    assert!(matches!(host.advance_decoder_forced(host.revision(), n.actor_revision, n.position, 0, budget()).unwrap().unwrap(), MonitoredStep::Released(_)));
    host
}
fn capture(host: &mut FileOversight) -> Result<FileDecoderCheckpoint, JournalError> {
    let n = host.decoder_inspection().unwrap().numerical;
    host.capture_decoder_checkpoint(host.revision(), 7, n.actor_revision, host.inspect().control.ledger.epoch)
}
fn request(host: &FileOversight, operation: u64) -> FileResetRequest {
    let c = host.inspect().control;
    FileResetRequest { operation, expected_control_sequence: c.sequence,
        expected_actor_revision: host.decoder_inspection().unwrap().numerical.actor_revision,
        expected_authority_epoch: c.ledger.epoch,
        binding: ReviewBinding { round: 5000 + operation, evidence_root: [17; 32], reducer_generation: 1 },
        retained_targets: vec![host.inspect().target] }
}
fn fault(error: JournalError, stage: JournalIo) {
    let JournalError::Io(failure) = error else { panic!("missing injected storage failure"); };
    assert_eq!(failure.operation, stage);
    assert_eq!(failure.replacement_may_be_visible, matches!(stage, JournalIo::Rename | JournalIo::DirectorySync));
}

#[test]
fn capture_faults_never_return_a_speculative_handle_and_only_recover_canonical_pairs() {
    for stage in BARRIERS {
        let root = Directory::new(); let config = config(3.0); let mut host = owner(&root, &config);
        let before = host.inspect(); let numerical = host.decoder_inspection().unwrap().numerical;
        host.store.fail_once(stage); fault(capture(&mut host).unwrap_err(), stage);
        assert_eq!(host.inspect(), before); assert!(matches!(host.decoder_checkpoint(7), Err(JournalError::Unavailable)));
        drop(host);
        let (mut host, _) = FileOversight::open_with_decoder(root.store(), host_profile(), &config).unwrap();
        assert_eq!(host.decoder_checkpoint(7).is_ok(), stage == JournalIo::DirectorySync);
        assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
        assert!(host.decoder_inspection().unwrap().paused);
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        host.resume_decoder(host.revision(), numerical.actor_revision, numerical.position).unwrap();
        if stage != JournalIo::DirectorySync { capture(&mut host).unwrap(); }
        assert_eq!(host.decoder_recovery_usage().unwrap().checkpoints, 1);
    }
}

#[test]
fn reset_failure_barriers_preserve_success_or_failure_only_when_canonical_and_never_retry() {
    for replay_fails in [false, true] {
        for stage in BARRIERS {
            let root = Directory::new(); let config = if replay_fails { limited_config() } else { config(3.0) };
            let mut host = owner(&root, &config); let cp = capture(&mut host).unwrap();
            let original = request(&host, 1); let before = host.inspect();
            host.store.fail_once(stage);
            fault(host.reset_decoder_checkpoint(host.revision(), &cp, original.clone(), budget()).unwrap_err(), stage);
            assert_eq!(host.inspect(), before);
            assert_eq!(host.reset_decoder_checkpoint(host.revision(), &cp, original.clone(), budget()), Err(JournalError::Unavailable));
            let disk = FileOversight::read_decoder_reset(root.store(), &host_profile(), &config, 1);
            let visible = stage == JournalIo::DirectorySync;
            if visible {
                let disk = disk.unwrap(); assert_eq!(disk.result.is_err(), replay_fails);
                assert_eq!(disk.recovery.replay_attempts, 1);
                assert!(disk.recovery.admitted_products > 0);
            } else { assert_eq!(disk, Err(JournalError::Contract(Error::Missing))); }
            drop(host);
            let (mut host, _) = FileOversight::open_with_decoder(root.store(), host_profile(), &config).unwrap();
            let fresh = host.decoder_checkpoint(7).unwrap();
            assert_eq!(host.reset_decoder_checkpoint(host.revision(), &cp, original.clone(), budget()), Err(JournalError::Contract(Error::Binding)));
            assert!(host.decoder_inspection().unwrap().paused);
            assert_eq!(host.decoder_recovery_usage().unwrap().replay_attempts, usize::from(visible));
            if visible {
                let before = host.inspect(); let usage = host.decoder_recovery_usage().unwrap();
                // Exact historical retry works without fresh time and does not
                // turn an ambiguous reset into another replay or incident.
                assert_eq!(host.reset_decoder_checkpoint(0, &fresh, original, budget()).unwrap().is_err(), replay_fails);
                assert_eq!(host.inspect(), before); assert_eq!(host.decoder_recovery_usage().unwrap(), usage);
                if replay_fails { assert!(matches!(host.decoder_inspection().unwrap().numerical.status, MonitoringStatus::Failed(_))); }
            } else {
                assert!(matches!(host.decoder_reset_result(1), Err(JournalError::Contract(Error::Missing))));
                host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
                let n = host.decoder_inspection().unwrap().numerical;
                host.resume_decoder(host.revision(), n.actor_revision, n.position).unwrap();
                assert_eq!(host.reset_decoder_checkpoint(host.revision(), &fresh, request(&host, 2), budget()).unwrap().is_err(), replay_fails);
            }
        }
    }
}

#[test]
fn restrictive_reset_cannot_clear_source_interruption_but_a_real_fresh_read_can() {
    let root = Directory::new(); let p = host_profile();
    let source_policy = FileSourcePolicy { source: StateSource { scope: p.delivery.scope, source: 77, generation: 1 },
        limits: StateLimits::default(), freshness: StateFreshness::new(100).unwrap() };
    let (mut host, _) = FileOversight::create(root.store(), p.clone()).unwrap();
    host.enable_file_source(host.revision(), source_policy).unwrap();
    host.enable_decoder(host.revision(), config(3.0)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let evidence = EvidenceSnapshot::new(EvidenceIdentity { source: 77, generation: 1, scope: p.delivery.scope },
        Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() },
        BTreeMap::from([("reviewer".to_owned(), b"complete evidence".to_vec())])).unwrap();
    let path = root.0.join("source.json"); std::fs::write(&path, evidence.encode()).unwrap();
    let mut source = FileEvidenceSource::new(path, 77, p.delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    host.advance_decoder_forced(host.revision(), n.actor_revision, 0, 0, budget()).unwrap().unwrap();
    let cp = capture(&mut host).unwrap();
    host.source_interrupted = true; // Exact process latch set before a source read.
    host.reset_decoder_checkpoint(host.revision(), &cp, request(&host, 1), budget()).unwrap().unwrap();
    assert!(host.source_interrupted);
    let spec = ActionSpec { version: VERSION, scope: p.delivery.scope, target: Some(host.inspect().target),
        payload: b"new work".to_vec(), required_witnesses: Vec::new(), policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(20), units: 16 };
    assert_eq!(host.propose(host.revision(), 1, spec.clone(), evidence.snapshot().clone()), Err(JournalError::Contract(Error::Incomplete)));
    // Only the original acknowledged withdrawal + new file acquisition clears it.
    let captured = host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1)).unwrap();
    assert!(!host.source_interrupted);
    assert!(host.propose(host.revision(), 1, spec, captured.snapshot().clone()).is_ok());
}
