//! Real temporary-file operations through the public oversight API.
//! Helpers and snapshots are explicit fixtures, not authenticated live providers.
#![cfg(unix)]

use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{
    DispatchEnvelope, EndpointOutcome, EndpointStatus, FileEndpointRecovery, NonExecutionReason, PublicationEndpoint,
};
use fa_reference::action::consequence::delivery::filesystem::{FilePublicationError, FilePublicationLimits};
use fa_reference::action::consequence::delivery::stream::StreamProfile;
use fa_reference::action::consequence::gate::{ReviewBinding, TargetCeiling};
use fa_reference::action::consequence::gate::containment::{ActorState, ResetRequest, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::{
    CommitteeContract, CommitteeInput, HelperContract, OversightBroker, ReviewWindow, action_frame,
};
use fa_reference::action::consequence::oversight::human::{HumanDisposition, HumanReviewPolicy};
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Permit, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use fa_reference::{Error, Snapshot};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const TOTAL: u64 = 100_000;
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let tick = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-file-public-{}-{tick}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("test directory cleanup failed: {error}"); }
    }
}
fn target(version: u64) -> ResolvedTarget {
    ResolvedTarget { adapter: 1, object: 2, contract_version: 3, expected_version: version, generation: 4 }
}
fn scope() -> Scope {
    Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect }
}
fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, vec![9])]) }
}
fn config() -> ControllerConfig {
    ControllerConfig {
        scope: scope(), total: TOTAL, max_attempts: 32,
        actor: ActorState::new(RestartProfile {
            id: 1, generation: 1, host_generation: 1, model_generation: 1, tokenizer_generation: 1,
            state_schema_generation: 1, grade: RestartGrade::FunctionalRestart,
        }, vec![1], vec![2], vec![3], 1).unwrap(),
        suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: vec![9] }]).unwrap(),
        congress: CongressPolicy {
            generation: 1, members: BTreeMap::from([("helper".to_owned(), MemberPolicy { cohort: "a".to_owned(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
            narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1,
        },
        narrowed_targets: TargetCeiling::new(&[target(1), target(2), target(3), target(4)]).unwrap(),
    }
}
fn fixture(stream: bool) -> (Temp, OversightBroker, PublicationEndpoint, FileEndpointRecovery, CommitteeContract) {
    let root = Temp::new();
    let limits = FilePublicationLimits { mutations: 128, bytes: 1_048_576 };
    let path = root.0.join("publication");
    let (mut endpoint, key) = if stream {
        PublicationEndpoint::create_file_stream(path, target(1), StreamProfile::new(9, 2, 8, 64, 512).unwrap(), 200, 32, limits)
    } else {
        PublicationEndpoint::create_file_publication(path, target(1), b"original".to_vec(), 200, 32, limits)
    }.unwrap();
    let contracts = CommitteeContract::new(BTreeMap::from([("helper".to_owned(), HelperContract::new(
        InputProfileBinding { profile_id: 1, profile_bytes: b"whole-publication".to_vec(),
            model_epoch: 1, tokenizer_epoch: 1, policy_epoch: 0 },
        9, b"Review the complete publication and cumulative stream context".to_vec(),
    ).unwrap())])).unwrap();
    let mut broker = OversightBroker::new(config(), &mut endpoint, contracts.clone()).unwrap();
    broker.observe_time(ElapsedTick(1)).unwrap();
    endpoint.observe_time(ElapsedTick(1)).unwrap();
    broker.confirm_fence(endpoint.install_fence(broker.fence_request()).unwrap()).unwrap();
    (root, broker, endpoint, key, contracts)
}
fn prepare(
    broker: &mut OversightBroker, contracts: &CommitteeContract, id: u64, message: Option<&str>,
) -> (FrozenAction, CommitteeInput) {
    let spec = if broker.stream_state().is_some() {
        match message {
            Some(message) => broker.stream_message_spec(message, ElapsedTick(100)).unwrap(),
            None => broker.stream_finish_spec(ElapsedTick(100)).unwrap(),
        }
    } else {
        let payload = message.unwrap_or("").as_bytes().to_vec();
        ActionSpec { version: VERSION, scope: scope(), target: Some(target(1)),
            units: (payload.len() as u64).max(1), payload, required_witnesses: Vec::new(),
            policy_epoch: broker.inspect().ledger.epoch, deadline: ElapsedTick(100) }
    };
    let action = broker.propose(id, spec, &snapshot()).unwrap().action;
    let helper = &contracts.members()["helper"];
    let mut bytes = action_frame(&action);
    let boundary = bytes.len();
    bytes.extend_from_slice(helper.question());
    let actual = ActualHelperInput::new(bytes.clone(), helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { span: ByteSpan { start: 0, end: boundary }, kind: PartKind::Other },
        SubmittedPart { span: ByteSpan { start: boundary, end: bytes.len() }, kind: PartKind::Question },
    ], Vec::new()).unwrap();
    let view = EvidenceViewManifest::new(actual, AuthorizationProjection {
        projection_id: 9, policy_epoch: action.spec().policy_epoch, projected_originals: Vec::new(),
    }, Vec::new()).unwrap();
    let inputs = CommitteeInput::capture(&action, contracts, BTreeMap::from([("helper".to_owned(), view)])).unwrap();
    broker.record_inputs(id, 0, inputs.clone()).unwrap();
    (action, inputs)
}
fn judge(broker: &mut OversightBroker, id: u64, round: u64, inputs: &CommitteeInput, verdict: Verdict) {
    let now = broker.inspect().ledger.elapsed.unwrap();
    let mut session = broker.begin_review(id, round, [1; 32], ReviewWindow {
        commit_by: ElapsedTick(now.0 + 5), reveal_by: ElapsedTick(now.0 + 10),
    }, &snapshot()).unwrap();
    let commitment = session.commitment("helper", verdict, b"salt").unwrap();
    session.commit("helper", commitment, now).unwrap();
    session.open_reveals(now).unwrap();
    session.reveal("helper", verdict, b"salt", now).unwrap();
    broker.apply_review(session.finish(now).unwrap(), Some(inputs), &snapshot()).unwrap();
}
fn ready(
    broker: &mut OversightBroker, contracts: &CommitteeContract, id: u64, message: Option<&str>,
) -> (FrozenAction, CommitteeInput, Permit) {
    let (action, inputs) = prepare(broker, contracts, id, message);
    judge(broker, id, id, &inputs, Verdict::Allow);
    let permit = broker.authorize(id, Some(&inputs), &snapshot()).unwrap();
    (action, inputs, permit)
}
fn send(broker: &mut OversightBroker, contracts: &CommitteeContract, id: u64, message: Option<&str>) -> DispatchEnvelope {
    let (action, inputs, permit) = ready(broker, contracts, id, message);
    broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap()
}
fn conserved(broker: &OversightBroker) {
    let state = broker.inspect().ledger;
    assert_eq!(state.available + state.reserved + state.charged, TOTAL);
}

#[test]
fn approved_publication_writes_real_bytes_and_lost_ack_reconciles_after_reopen() {
    for stream in [false, true] {
        let (_root, mut broker, mut endpoint, key, contracts) = fixture(stream);
        let message = send(&mut broker, &contracts, 1, Some("reviewed payload"));
        let receipt = endpoint.deliver(&message).unwrap();
        broker.acknowledgment_lost(1).unwrap();
        let visible = PublicationEndpoint::read_file_publication(key.directory()).unwrap();
        assert_eq!(visible.payload, b"reviewed payload");
        assert_eq!(visible.execution_count, 1);
        assert_eq!(visible.target.expected_version, 2);
        assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Unknown);
        assert!(broker.inspect().ledger.charged > 0);
        drop(endpoint);
        let mut recovered = key.reopen().unwrap();
        let query = broker.status_query(1).unwrap();
        assert_eq!(recovered.status(&query), Err(Error::Incomplete));
        recovered.observe_time(ElapsedTick(1)).unwrap();
        assert_eq!(recovered.status(&query).unwrap(), EndpointStatus::Resolved(receipt.clone()));
        let cut = key.visible_revision();
        assert_eq!(recovered.deliver(&message).unwrap(), receipt);
        assert_eq!(key.visible_revision(), cut);
        assert!(broker.accept_receipt(receipt.clone()).unwrap());
        assert!(!broker.accept_receipt(receipt).unwrap());
        assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Confirmed);
        assert_eq!(recovered.execution_count(), 1);
        conserved(&broker);
    }
}

#[test]
fn oversight_hold_has_no_file_effect_and_a_new_allowing_review_can_publish() {
    let (_root, mut broker, mut endpoint, key, contracts) = fixture(true);
    let (action, inputs) = prepare(&mut broker, &contracts, 1, Some("held then reviewed"));
    judge(&mut broker, 1, 11, &inputs, Verdict::Hold);
    assert!(broker.authorize(1, Some(&inputs), &snapshot()).is_err());
    assert!(PublicationEndpoint::read_file_publication(key.directory()).unwrap().payload.is_empty());
    judge(&mut broker, 1, 12, &inputs, Verdict::Allow);
    let permit = broker.authorize(1, Some(&inputs), &snapshot()).unwrap();
    let mut changed = snapshot();
    changed.values.insert(7, vec![8]);
    assert!(broker.dispatch(&permit, &action, Some(&inputs), &changed).is_err());
    assert!(PublicationEndpoint::read_file_publication(key.directory()).unwrap().payload.is_empty());
    let message = broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(PublicationEndpoint::read_file_publication(key.directory()).unwrap().payload, b"held then reviewed");
    conserved(&broker);
}

#[test]
fn an_absent_record_stays_unknown_until_a_persisted_seal_prevents_late_execution() {
    let (_root, mut broker, endpoint, key, contracts) = fixture(true);
    let message = send(&mut broker, &contracts, 1, Some("must not appear"));
    broker.acknowledgment_lost(1).unwrap();
    drop(endpoint);
    let mut recovered = key.reopen().unwrap();
    recovered.observe_time(ElapsedTick(1)).unwrap();
    let query = broker.status_query(1).unwrap();
    assert_eq!(recovered.status(&query).unwrap(), EndpointStatus::AwaitingResolution);
    assert!(broker.inspect().ledger.charged > 0);
    let receipt = recovered.seal_unexecuted(&query).unwrap();
    assert_eq!(receipt.outcome(), EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed });
    broker.accept_receipt(receipt.clone()).unwrap();
    assert_eq!(broker.inspect().ledger.available, TOTAL);
    drop(recovered);
    let mut recovered = key.reopen().unwrap();
    recovered.observe_time(ElapsedTick(1)).unwrap();
    assert_eq!(recovered.deliver(&message).unwrap(), receipt);
    assert!(PublicationEndpoint::read_file_publication(key.directory()).unwrap().payload.is_empty());
    assert_eq!(recovered.execution_count(), 0);
    conserved(&broker);
}

#[test]
fn dispatcher_fence_survives_endpoint_reopen_and_does_not_invent_nonexecution() {
    let (_root, mut broker, endpoint, key, contracts) = fixture(true);
    let old_fence = broker.fence_request();
    let message = send(&mut broker, &contracts, 1, Some("old incarnation"));
    let fence = broker.restart_dispatcher().unwrap();
    drop(endpoint);
    let mut recovered = key.reopen().unwrap();
    broker.confirm_fence(recovered.install_fence(fence).unwrap()).unwrap();
    recovered.observe_time(ElapsedTick(1)).unwrap();
    assert_eq!(recovered.deliver(&message).unwrap_err(), Error::Stale);
    assert!(broker.inspect().ledger.charged > 0);
    let query = broker.status_query(1).unwrap();
    assert_eq!(recovered.status(&query).unwrap(), EndpointStatus::AwaitingResolution);
    broker.accept_receipt(recovered.seal_unexecuted(&query).unwrap()).unwrap();
    drop(recovered);
    let mut recovered = key.reopen().unwrap();
    assert_eq!(recovered.install_fence(old_fence).unwrap_err(), Error::Stale);
    recovered.observe_time(ElapsedTick(1)).unwrap();
    let fresh = send(&mut broker, &contracts, 2, Some("new incarnation"));
    broker.accept_receipt(recovered.deliver(&fresh).unwrap()).unwrap();
    assert_eq!(PublicationEndpoint::read_file_publication(key.directory()).unwrap().payload, b"new incarnation");
    conserved(&broker);
}

#[test]
fn human_expiry_is_not_extended_by_recovery_and_cannot_refund_a_prior_execution() {
    for executed_before_loss in [false, true] {
        let (_root, mut broker, mut endpoint, key, contracts) = fixture(true);
        let reviewer = broker.enable_human_review(HumanReviewPolicy {
            reviewer_id: 99, max_validity_ticks: 10, max_requests: 8,
        }).unwrap();
        let (action, inputs, permit) = ready(&mut broker, &contracts, 1, Some("two-key output"));
        let request = broker.request_human_approval(1, 1, Some(&inputs), ElapsedTick(5)).unwrap();
        let human = reviewer.approve(&request, ElapsedTick(1)).unwrap();
        assert!(broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).is_err());
        let message = broker.dispatch_with_human(&permit, &human, &action, Some(&inputs), &snapshot()).unwrap();
        if executed_before_loss { endpoint.deliver(&message).unwrap(); }
        broker.acknowledgment_lost(1).unwrap();
        drop(endpoint);
        let mut recovered = key.reopen().unwrap();
        assert_eq!(recovered.deliver(&message).unwrap_err(), Error::Incomplete);
        recovered.observe_time(ElapsedTick(5)).unwrap();
        let receipt = recovered.deliver(&message).unwrap();
        assert_eq!(receipt.outcome(), if executed_before_loss { EndpointOutcome::Executed { resulting_version: 2 } }
            else { EndpointOutcome::NotExecuted { reason: NonExecutionReason::DeadlineElapsed } });
        assert_eq!(receipt.request().approval().unwrap().expires_at(), ElapsedTick(5));
        broker.accept_receipt(receipt).unwrap();
        assert_eq!(broker.human_status(1).unwrap().disposition, HumanDisposition::Consumed);
        assert_eq!(recovered.execution_count(), u64::from(executed_before_loss));
        assert!(broker.dispatch_with_human(&permit, &human, &action, Some(&inputs), &snapshot()).is_err());
        conserved(&broker);
    }
}

#[test]
fn completed_stream_and_unknown_finish_survive_actor_reset_and_endpoint_recovery() {
    let (_root, mut broker, mut endpoint, key, contracts) = fixture(true);
    let checkpoint = broker.capture_checkpoint(1, 0).unwrap();
    let first = send(&mut broker, &contracts, 1, Some("already disclosed"));
    broker.accept_receipt(endpoint.deliver(&first).unwrap()).unwrap();
    let finish = send(&mut broker, &contracts, 2, None);
    endpoint.deliver(&finish).unwrap();
    broker.acknowledgment_lost(2).unwrap();
    broker.reset(ResetRequest {
        checkpoint, expected_control_sequence: broker.inspect().sequence,
        expected_actor_revision: broker.actor_revision(),
        binding: ReviewBinding { round: 90, evidence_root: [9; 32], reducer_generation: 1 },
        retained_targets: TargetCeiling::new(&[target(1), target(2), target(3)]).unwrap(),
    }).unwrap();
    drop(endpoint);
    let mut recovered = key.reopen().unwrap();
    assert!(recovered.stream_view().unwrap().finished());
    assert_eq!(recovered.payload(), b"already disclosed");
    recovered.observe_time(ElapsedTick(1)).unwrap();
    let query = broker.status_query(2).unwrap();
    let EndpointStatus::Resolved(receipt) = recovered.status(&query).unwrap() else { panic!("missing actual finish"); };
    broker.accept_receipt(receipt).unwrap();
    assert!(broker.stream_message_spec("cannot reopen", ElapsedTick(100)).is_err());
    assert_eq!(broker.inspect().ledger.stages[&2], ActionState::Confirmed);
    assert_eq!(recovered.execution_count(), 2);
    conserved(&broker);
}

#[test]
fn recovered_endpoint_cannot_attach_a_new_independently_funded_controller() {
    let (_root, broker, endpoint, key, contracts) = fixture(true);
    assert!(matches!(key.reopen(), Err(FilePublicationError::LockUnavailable(_))));
    drop(endpoint);
    let mut recovered = key.reopen().unwrap();
    assert_eq!(OversightBroker::new(config(), &mut recovered, contracts).unwrap_err(), Error::Duplicate);
    assert_eq!(broker.inspect().ledger.available, TOTAL);
    assert_eq!(recovered.execution_count(), 0);
}

#[test]
fn actual_io_failure_keeps_original_charge_until_recovered_endpoint_seals_the_key() {
    let (_root, mut broker, mut endpoint, key, contracts) = fixture(true);
    let message = send(&mut broker, &contracts, 1, Some("unacknowledged"));
    fs::write(key.directory().join("publication.pending"), b"partial staging").unwrap();
    assert_eq!(endpoint.deliver(&message).unwrap_err(), Error::Incomplete);
    broker.acknowledgment_lost(1).unwrap();
    assert!(broker.inspect().ledger.charged > 0);
    assert!(broker.cancel(1).is_err());
    assert!(PublicationEndpoint::read_file_publication(key.directory()).unwrap().payload.is_empty());
    drop(endpoint);
    let mut recovered = key.reopen().unwrap();
    recovered.observe_time(ElapsedTick(1)).unwrap();
    let query = broker.status_query(1).unwrap();
    broker.accept_receipt(recovered.seal_unexecuted(&query).unwrap()).unwrap();
    assert_eq!(broker.inspect().ledger.available, TOTAL);
    assert_eq!(recovered.deliver(&message).unwrap().outcome(), EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed });
    conserved(&broker);
}
