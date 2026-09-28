//! Original numerics, journals and Unix wire; synthetic ballots test protocol only.
#![cfg(unix)]
#![forbid(unsafe_code)]
#[path = "support/restart_model.rs"]
#[allow(dead_code)]
mod model;
use fa_reference::{Error, Snapshot};
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{EndpointOutcome, persistent::{
    FileDeliveryProfile, JournalError, JournalLimits,
    observed::{FileOversight as Host, FileOversightProfile, FileHumanReviewer,
        decoder::learned::{FileLearnedConfig, sidecar::{FileLearnedSidecar, FileLearnedSidecarFinish}},
        helpers::{FileHelperSetupError, learned_sockets::{FileLearnedSocketRound, LearnedSocketLaunch, LearnedSocketStatus}}},
}};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::{CommitteeContract, HelperContract, ReviewWindow,
    helper_client::{HelperClient, ClientPhase}, helper_workers::{HelperLimits, HelperPhase},
    human::HumanReviewPolicy, decoder_monitoring::LearnedDecoderBindingLimits,
    learned_source::LearnedSourceConfig, learned_host::sidecar::LearnedSidecarRequest,
    sidecar::{SidecarCongressBudget, SidecarIdentity}};
use fa_reference::action::consequence::activation::probe::learned::{KvGroup, KvRow};
use fa_reference::action::consequence::activation::tensor::kv::{experiment::KvSide,
    decoder::sampling::{SamplingPolicy, SamplingStart,
        monitored::{GenerationBudget, GenerationSpec, GenerationTelemetryBudget}}};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-learned-sockets-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
    fn bytes(&self) -> Vec<u8> { std::fs::read(self.store().join("delivery.bin")).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("socket test cleanup: {error}"); } }
}
fn target() -> ResolvedTarget {
    ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 }
}
fn profile() -> FileOversightProfile {
    FileOversightProfile { delivery: FileDeliveryProfile {
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        total: 100, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
            model_generation: 3, tokenizer_generation: 4, state_schema_generation: 1,
            grade: RestartGrade::AuditOnly }, Vec::new(), vec![0], vec![0], 0).unwrap(),
        suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::PayloadAtMost(1024)]).unwrap(),
        congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".to_owned(),
            MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
            narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: vec![target()], target: target(), initial_payload: b"initial".to_vec(),
        retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
    }, committee: CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(), HelperContract::new(
        InputProfileBinding { profile_id: 1, profile_bytes: b"socket-v1".to_vec(),
            tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 7, b"approve?".to_vec(),
    ).unwrap())])).unwrap(), human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 } }
}
fn config() -> FileLearnedConfig {
    let model = model::model();
    let policy = model::policy(&model, 0, 1);
    FileLearnedConfig::new(model, LearnedSourceConfig { stream: 21, evaluation_origin: 201, monitor_generation: 31,
        spec: GenerationSpec::new(vec![0], 3, BTreeSet::new(), SamplingStart {
            policy: SamplingPolicy::new(1, 1, 3, 0.8, 1, 1.0).unwrap(), stream: 71, seed: 173,
        }).unwrap(), policy, budget: GenerationBudget::default(), telemetry: GenerationTelemetryBudget::default() },
        LearnedDecoderBindingLimits::default()).unwrap().with_required_sidecar().unwrap()
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn step(h: &mut Host) {
    let n = h.learned_generation_inspection().unwrap().numerical;
    h.advance_learned_generation(h.revision(), n.actor_revision, n.position).unwrap().unwrap();
}
fn setup() -> (Directory, Host, FileHumanReviewer, FrozenAction, FileLearnedSidecar) {
    let dir = Directory::new();
    let (mut h, reviewer) = Host::create(dir.store(), profile()).unwrap();
    h.enable_learned_generation(h.revision(), config()).unwrap();
    h.observe_time(h.revision(), ElapsedTick(1)).unwrap(); step(&mut h); step(&mut h);
    let action = h.propose(h.revision(), 1, ActionSpec { version: VERSION, scope: profile().delivery.scope,
        target: Some(target()), payload: b"visible".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: h.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 }, snapshot()).unwrap();
    let n = h.learned_generation_inspection().unwrap().numerical;
    let sidecar = h.begin_learned_sidecar_plan(h.revision(), 1, n.actor_revision, LearnedSidecarRequest {
        identity: SidecarIdentity { object_id: 1001, generation: 1, transform_id: 7 },
        priority: vec![KvGroup { row: KvRow { layer: 1, side: KvSide::Value, position: 1 }, head: 0 }],
        budget: SidecarCongressBudget::default(),
    }).unwrap();
    (dir, h, reviewer, action, sidecar)
}
fn launch() -> (LearnedSocketLaunch, UnixStream) {
    let (server, client) = UnixStream::pair().unwrap(); client.set_nonblocking(true).unwrap();
    (LearnedSocketLaunch { round: 101, evidence_root: [9; 32],
        window: ReviewWindow { commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30) },
        streams: BTreeMap::from([("reviewer".to_owned(), server)]), limits: HelperLimits::default() }, client)
}
fn client(stream: UnixStream, action: &FrozenAction) -> HelperClient<UnixStream> {
    HelperClient::from_unix(stream, profile().committee.members()["reviewer"].profile_at(action.spec().policy_epoch)).unwrap()
}
fn drive(round: &mut FileLearnedSocketRound, h: &mut Host, client: &mut HelperClient<UnixStream>, verdict: Verdict) {
    for _ in 0..256 {
        round.pump(h, round.revision(), ElapsedTick(1)).unwrap();
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference { client.respond(verdict, &[11; 32]).unwrap(); }
        if round.ready_to_finish() { return; }
    }
    panic!("original bounded wire round failed to finish");
}

#[test]
fn external_original_client_drives_durable_sidecar_two_keys_and_publication() {
    let (dir, mut h, reviewer, action, plan) = setup();
    let input = h.current_learned_sidecar(&plan).unwrap().clone();
    let before = h.learned_generation_inspection().unwrap().numerical;
    let (launch, stream) = launch(); let mut client = client(stream, &action);
    let mut round = h.begin_learned_socket_round(h.revision(), plan, launch, snapshot()).unwrap();
    drive(&mut round, &mut h, &mut client, Verdict::Allow);
    assert_eq!(client.input().unwrap().actual_input(), input.views()["reviewer"].actual_input());
    assert!(matches!(round.finish(&mut h, round.revision(), snapshot()).unwrap(),
        FileLearnedSidecarFinish::Applied { receipt: Ok(_), .. }));
    assert_eq!(round.status(), LearnedSocketStatus::Finished);
    assert_eq!(h.learned_generation_inspection().unwrap().numerical, before);
    let stored = h.learned_sidecar_outcome(101).unwrap();
    assert_eq!(stored.result.archive(), round.outcome().unwrap().archive());
    assert_eq!(Host::read_learned_sidecar_outcome(dir.store(), &profile(), &config(), 101)
        .unwrap().result.archive(), stored.result.archive());
    let automatic = h.authorize(h.revision(), 1, &input, snapshot()).unwrap();
    let request = h.request_human_approval(h.revision(), 1001, 1, &input, ElapsedTick(40)).unwrap();
    let rev = h.revision(); let human = reviewer.approve(&mut h, rev, &request).unwrap();
    h.dispatch(h.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
    assert_eq!(h.publish_checked(h.revision(), 1, Some(&input), snapshot(), ElapsedTick(2)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
    h.reconcile(h.revision(), 1).unwrap();
    assert_eq!(h.inspect().payload, b"visible"); assert_eq!(h.inspect().executions, 1);
    assert!(h.commit_review(h.revision(), 101, "reviewer", fa_reference::round::commitment(101,
        "reviewer", &[9; 32], Verdict::Allow, b"other").unwrap()).is_err());
}

#[test]
fn changed_source_closes_transport_before_any_original_packet_is_sent() {
    let (_dir, mut h, _, _, plan) = setup(); let (launch, mut peer) = launch();
    let mut round = h.begin_learned_socket_round(h.revision(), plan, launch, snapshot()).unwrap();
    step(&mut h);
    let failure = round.pump(&mut h, 0, ElapsedTick(1)).unwrap_err();
    assert!(failure.progress.io.is_empty()); assert_eq!(round.connection_steps(), 0);
    assert_eq!(round.status(), LearnedSocketStatus::Failed); assert!(round.outcome().is_none());
    assert_eq!(peer.read(&mut [0; 1]).unwrap(), 0);
    assert!(round.pump(&mut h, round.revision(), ElapsedTick(1)).is_err());
    assert_eq!(h.inspect().executions, 0);
}

#[test]
fn stale_foreign_and_incomplete_calls_preserve_a_usable_original_round() {
    let (_dir, mut h, _, action, plan) = setup();
    let (_other_dir, mut other, _, _, _) = setup();
    let (launch, stream) = launch(); let mut client = client(stream, &action);
    let mut round = h.begin_learned_socket_round(h.revision(), plan, launch, snapshot()).unwrap();
    let rev = h.revision();
    assert_eq!(round.pump(&mut other, 0, ElapsedTick(1)).unwrap_err().error, JournalError::Contract(Error::Binding));
    assert_eq!(round.pump(&mut h, 1, ElapsedTick(1)).unwrap_err().error, JournalError::Contract(Error::Stale));
    assert_eq!(round.pump(&mut h, 0, ElapsedTick(0)).unwrap_err().error, JournalError::Contract(Error::Stale));
    assert_eq!(round.finish(&mut h, 0, snapshot()).err(), Some(JournalError::Contract(Error::Incomplete)));
    assert_eq!(h.revision(), rev); assert_eq!(round.revision(), 0); assert_eq!(round.connection_steps(), 0);
    drive(&mut round, &mut h, &mut client, Verdict::Allow);
    assert!(matches!(round.finish(&mut h, round.revision(), snapshot()).unwrap(),
        FileLearnedSidecarFinish::Applied { receipt: Ok(_), .. }));
}

#[test]
fn failed_commit_sync_retains_io_but_cannot_release_the_reveal_request() {
    let (dir, mut h, _, action, plan) = setup(); let (launch, stream) = launch();
    let mut client = client(stream, &action);
    let mut round = h.begin_learned_socket_round(h.revision(), plan, launch, snapshot()).unwrap();
    for _ in 0..256 {
        round.pump(&mut h, round.revision(), ElapsedTick(1)).unwrap(); client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference { break; }
    }
    assert_eq!(client.phase(), ClientPhase::NeedsInference);
    client.respond(Verdict::Allow, &[11; 32]).unwrap();
    while client.phase() == ClientPhase::SendingCommitment { client.step().unwrap(); }
    assert_eq!(client.phase(), ClientPhase::AwaitingReveal);
    let before = dir.bytes(); let revision = h.revision();
    std::fs::write(dir.store().join("delivery.pending"), b"occupied stage").unwrap();
    let error = round.pump(&mut h, round.revision(), ElapsedTick(1)).unwrap_err();
    assert!(!error.progress.io.is_empty()); assert_eq!(dir.bytes(), before);
    assert_eq!(h.revision(), revision); assert!(h.storage_failure().is_some());
    assert_eq!(round.status(), LearnedSocketStatus::Failed); assert!(round.outcome().is_none());
    assert!(!round.progress().workers["reviewer"].committed);
    assert!(client.step().is_err()); // closed before a reveal signal can be sent
    drop(h);
    let (h, _) = Host::open_with_learned_generation(dir.store(), profile(), &config()).unwrap();
    assert!(h.learned_sidecar_outcome(101).is_err());
    assert_eq!(h.inspect().executions, 0);
}

#[test]
fn missing_worker_stays_missing_and_receives_nothing_after_commit_expiry() {
    let (_dir, mut h, _, _, plan) = setup(); let (launch, mut peer) = launch();
    let input = h.current_learned_sidecar(&plan).unwrap().clone();
    let mut round = h.begin_learned_socket_round(h.revision(), plan, launch, snapshot()).unwrap();
    round.pump(&mut h, 0, ElapsedTick(20)).unwrap();
    assert_eq!(round.progress().workers["reviewer"].phase, HelperPhase::Failed);
    assert!(matches!(peer.read(&mut [0; 1]), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock));
    round.pump(&mut h, round.revision(), ElapsedTick(30)).unwrap();
    round.finish(&mut h, round.revision(), snapshot()).unwrap();
    assert_eq!(round.input_revision(), 1); assert!(h.authorize(h.revision(), 1, &input, snapshot()).is_err());
    assert_eq!(peer.read(&mut [0; 1]).unwrap(), 0); assert_eq!(h.inspect().executions, 0);
}

#[test]
fn cancellation_and_drop_close_sockets_without_refunding_or_manual_takeover() {
    for cancel in [false, true] {
        let (dir, mut h, _, _, plan) = setup(); let (launch, mut peer) = launch();
        let mut round = h.begin_learned_socket_round(h.revision(), plan, launch, snapshot()).unwrap();
        let revision = h.revision(); let bytes = dir.bytes();
        if cancel { round.cancel(0).unwrap(); assert_eq!(round.status(), LearnedSocketStatus::Cancelled); }
        drop(round); assert_eq!(peer.read(&mut [0; 1]).unwrap(), 0);
        assert_eq!(h.revision(), revision); assert_eq!(dir.bytes(), bytes);
        assert_eq!(h.inspect().control.ledger.stages[&1], ActionState::Reviewing);
        assert!(h.open_reveals(h.revision(), 101).is_err()); assert_eq!(h.inspect().executions, 0);
    }
}

#[test]
fn incomplete_socket_roster_refuses_before_a_journaled_round() {
    let (_dir, mut h, _, _, plan) = setup(); let (mut launch, mut peer) = launch();
    launch.streams.clear(); let before = h.revision();
    assert!(matches!(h.begin_learned_socket_round(before, plan, launch, snapshot()),
        Err(FileHelperSetupError::Journal(JournalError::Contract(Error::Binding)))));
    assert_eq!(h.revision(), before); assert_eq!(peer.read(&mut [0; 1]).unwrap(), 0);
}

#[path = "durable_learned_sockets/sequence.rs"]
mod sequence;
