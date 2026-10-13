//! Actual source-backed owners and the unchanged actor submission path.
//! Synthetic policy data tests enforcement, not detector quality or OS isolation.
use super::*;
use crate::action::{ActionState, Purpose};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalLimits};
use crate::action::consequence::delivery::persistent::observed::{FileOversightProfile,
    source::FileSourcePolicy};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::Predicate;
use crate::action::consequence::oversight::{CommitteeContract, HelperContract,
    actor::{ActorOutcome, Knowledge}, human::HumanReviewPolicy,
    evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource},
    policy_state::{StateFreshness, StateLimits, StateSource}};
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use std::{collections::BTreeMap, fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-precheck-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&root).unwrap(); Self(root)
    }
    fn store(&self) -> PathBuf { self.0.join("store") }
    fn source(&self) -> PathBuf { self.0.join("source.json") }
    fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fs::read_dir(self.store()).unwrap().map(|entry| entry.unwrap().path())
            .filter(|path| path.is_file()).map(|path| { let bytes = fs::read(&path).unwrap(); (path, bytes) }).collect()
    }
}
impl Drop for Root {
    fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("precheck cleanup: {error}"); } }
}
fn snapshot(epoch: u64, value: &[u8]) -> Snapshot {
    Snapshot { semantic_epoch: epoch, complete: true, values: BTreeMap::from([(7, value.to_vec())]) }
}
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            total: 100, max_attempts: 8,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
                model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
                grade: RestartGrade::AuditOnly }, vec![], vec![1], vec![2], 0).unwrap(),
            suspend_at_incident: 3,
            policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() },
                Predicate::PayloadAtMost(4), Predicate::All(vec![0, 1])]).unwrap(),
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([("r".into(),
                MemberPolicy { cohort: "test".into(), weight: 1 })]), caps: Caps { per_member: 1, per_cohort: 1 },
                continue_minimum: 1, continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3,
                minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
            retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("r".into(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: b"precheck".to_vec(),
                tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn write_source(root: &Root, epoch: u64, value: &[u8]) {
    let data = EvidenceSnapshot::new(EvidenceIdentity { source: 51, generation: 1,
        scope: profile().delivery.scope }, snapshot(epoch, value),
        BTreeMap::from([("r".into(), b"not disclosed to precheck".to_vec())])).unwrap();
    fs::write(root.source(), data.encode()).unwrap();
}
fn owner(root: &Root) -> (FileOversight, FileEvidenceSource) {
    let p = profile(); let (mut host, _) = FileOversight::create(root.store(), p.clone()).unwrap();
    host.enable_file_source(host.revision(), FileSourcePolicy {
        source: StateSource { scope: p.delivery.scope, source: 51, generation: 1 },
        limits: StateLimits::default(), freshness: StateFreshness::new(5).unwrap(),
    }).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    write_source(root, 1, b"ok");
    let mut source = FileEvidenceSource::new(root.source(), 51, p.delivery.scope, 1048576).unwrap();
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1)).unwrap();
    (host, source)
}
fn proposal(payload: &[u8]) -> ActorProposal {
    ActorProposal { target: profile().delivery.target, payload: payload.to_vec(), units: 8,
        deadline: ElapsedTick(100), expected_policy_epoch: 0 }
}
fn disclose(supervisor: &FileActorSupervisor<FileOversight>, queries: usize)
    -> (FileActorPrecheckPort, FilePrecheckDisclosure)
{
    supervisor.disclose_exact_precheck(supervisor.host().unwrap().revision(), 81, queries).unwrap()
}
fn known(observation: FilePrecheckObservation, expected: FilePrecheckOutcome, query: u64) -> FilePrecheckBasis {
    let Knowledge::Known { value, basis } = observation else { panic!("expected a typed precheck observation") };
    assert_eq!(value, expected);
    assert_eq!((basis.disclosure, basis.query, basis.policy_epoch), (81, query, 0));
    basis
}
fn unknown(port: &FileActorPrecheckPort) {
    assert_eq!(port.check(1, &proposal(b"ok")).unwrap(),
        Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable });
}

#[test]
fn precheck_matches_exact_admission_but_never_submits_reserves_or_calls_a_helper() {
    for (payload, outcome, stage) in [
        (b"ok".as_slice(), FilePrecheckOutcome::PrecheckPassed, ActionState::Reviewing),
        (b"blocked".as_slice(), FilePrecheckOutcome::DeniedByPolicy, ActionState::Denied),
    ] {
        let root = Root::new(); let (host, source) = owner(&root);
        let (actor, mut supervisor) = host.into_actor_gateway();
        let revision = supervisor.host().unwrap().revision();
        supervisor.set_snapshot(revision, Some(snapshot(1, b"ok"))).unwrap();
        let (precheck, grant) = disclose(&supervisor, 3);
        let before = supervisor.host().unwrap().inspect(); let files = root.files();
        let reads = source.status().read_attempts;
        let candidate = proposal(payload);
        known(precheck.check(901, &candidate).unwrap(), outcome, 901);
        known(precheck.check(901, &candidate).unwrap(), outcome, 901);
        assert_eq!(grant.remaining_queries(), 1);
        assert_eq!(supervisor.host().unwrap().inspect(), before); assert_eq!(root.files(), files);
        assert_eq!(source.status().read_attempts, reads);
        assert_eq!(supervisor.host().unwrap().retained_requests(), 0);
        // The same label is still a genuinely NEW actor request. The precheck
        // neither used that ID nor consumed the one-shot admission snapshot.
        let ticket = actor.submit(901, &candidate).unwrap();
        let host = supervisor.host().unwrap();
        assert_eq!(host.inspect().control.ledger.stages[&1], stage);
        assert_eq!(host.inspect().control.ledger.available, 100);
        assert_eq!(host.inspect().control.ledger.reserved, 0);
        assert_eq!(host.inspect().executions, 0);
        assert!(host.inspect().control.decisions.is_empty());
        if stage == ActionState::Reviewing {
            assert_eq!(actor.poll(&ticket), Knowledge::Pending { request: 901 });
        } else {
            assert!(matches!(actor.poll(&ticket), Knowledge::Known { value: ActorOutcome::Denied, .. }));
        }
        unknown(&precheck);
    }
}

#[test]
fn missing_expired_and_withdrawn_source_never_substitutes_the_admission_snapshot() {
    let root = Root::new(); let (host, _) = FileOversight::create(root.store(), profile()).unwrap();
    let (_, mut supervisor) = host.into_actor_gateway();
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot(1, b"ok"))).unwrap();
    assert_eq!(supervisor.disclose_exact_precheck(revision, 81, 1).err(), Some(Error::WrongState.into()));
    drop(supervisor); drop(root);
    let root = Root::new(); let (host, _) = owner(&root);
    let (_, mut supervisor) = host.into_actor_gateway(); let (old, grant) = disclose(&supervisor, 2);
    {
        let mut host = supervisor.host_mut().unwrap(); let revision = host.revision();
        host.observe_time(revision, ElapsedTick(7)).unwrap();
    }
    unknown(&old); assert_eq!(grant.remaining_queries(), 2);
    assert!(supervisor.disclose_exact_precheck(supervisor.host().unwrap().revision(), 82, 1).is_err());
    let root2 = Root::new(); let (host, mut source) = owner(&root2);
    let (_, mut supervisor) = host.into_actor_gateway();
    // Keep a live custodian while the ORIGINAL source read fails.
    let (old, grant) = disclose(&supervisor, 2);
    fs::remove_file(root2.source()).unwrap();
    {
        let mut host = supervisor.host_mut().unwrap(); let revision = host.revision();
        assert!(host.refresh_file_source(revision, &mut source, ElapsedTick(1)).is_err());
    }
    let failed = root2.files(); unknown(&old); assert_eq!(grant.remaining_queries(), 2);
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot(1, b"ok"))).unwrap();
    assert!(supervisor.disclose_exact_precheck(revision, 82, 1).is_err()); assert_eq!(root2.files(), failed);
    // A real new source observation, not a cached snapshot, restores eligibility
    // for a NEW disclosure only, and its changed exact value still denies.
    write_source(&root2, 2, b"no");
    {
        let mut host = supervisor.host_mut().unwrap(); let revision = host.revision();
        host.refresh_file_source(revision, &mut source, ElapsedTick(1)).unwrap();
    }
    let (fresh, _fresh_grant) = disclose(&supervisor, 1);
    known(fresh.check(1, &proposal(b"ok")).unwrap(), FilePrecheckOutcome::DeniedByPolicy, 1);
    unknown(&old);
}

#[test]
fn explicit_revocation_closes_every_clone_without_revoking_the_real_actor() {
    let root = Root::new(); let (host, _) = owner(&root);
    let (actor, mut supervisor) = host.into_actor_gateway(); let (port, grant) = disclose(&supervisor, 4);
    let clone = port.clone(); known(clone.check(1, &proposal(b"ok")).unwrap(), FilePrecheckOutcome::PrecheckPassed, 1);
    let files = root.files(); grant.revoke();
    for port in [&port, &clone] {
        assert_eq!(port.check(1, &proposal(b"ok")).unwrap(),
            Knowledge::Withheld { authority_required: "exact_policy_disclosure" });
    }
    assert_eq!(root.files(), files);
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot(1, b"ok"))).unwrap();
    let ticket = actor.submit(1, &proposal(b"ok")).unwrap();
    assert_eq!(actor.poll(&ticket), Knowledge::Pending { request: 1 });
    let (fresh, grant) = disclose(&supervisor, 1); drop(grant);
    assert!(matches!(fresh.check(1, &proposal(b"ok")).unwrap(), Knowledge::Withheld { .. }));
}

#[test]
fn clones_and_failed_queries_share_one_nonrenewable_work_allowance() {
    let root = Root::new(); let (host, _) = owner(&root); let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = disclose(&supervisor, 3); let clone = port.clone(); let files = root.files();
    assert_eq!(port.check(0, &proposal(b"ok")).err(), Some(ActorError::MalformedProposal));
    let mut invalid = proposal(b"ok"); invalid.target.object = 0;
    assert_eq!(clone.check(1, &invalid).err(), Some(ActorError::MalformedProposal));
    known(port.check(1, &proposal(b"ok")).unwrap(), FilePrecheckOutcome::PrecheckPassed, 1);
    for _ in 0..3 { assert_eq!(clone.check(1, &proposal(b"ok")).err(), Some(ActorError::Capacity)); }
    assert_eq!(grant.remaining_queries(), 0); assert_eq!(root.files(), files);
    let (maximum, grant) = disclose(&supervisor, MAX_PRECHECK_QUERIES);
    for query in 1..=MAX_PRECHECK_QUERIES {
        known(maximum.check(query as u64, &proposal(b"ok")).unwrap(), FilePrecheckOutcome::PrecheckPassed, query as u64);
    }
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(maximum.check(1, &proposal(b"ok")).err(), Some(ActorError::Capacity)); assert_eq!(root.files(), files);
}

#[test]
fn exact_target_deadline_epoch_and_available_budget_are_not_ignored() {
    let root = Root::new(); let (host, _) = owner(&root); let (_, supervisor) = host.into_actor_gateway();
    let (port, _grant) = disclose(&supervisor, 12); let files = root.files();
    for dimension in 0..7 {
        let mut candidate = proposal(b"ok");
        match dimension {
            0 => candidate.target.adapter += 1, 1 => candidate.target.object += 1,
            2 => candidate.target.contract_version += 1, 3 => candidate.target.expected_version += 1,
            4 => candidate.target.generation += 1, 5 => candidate.deadline = ElapsedTick(1),
            _ => candidate.units = 101,
        }
        known(port.check(1, &candidate).unwrap(), FilePrecheckOutcome::DeniedByPolicy, 1);
    }
    let mut stale = proposal(b"ok"); stale.expected_policy_epoch = 99;
    assert_eq!(port.check(1, &stale).unwrap(), Knowledge::Stale { generation: 99, current: 0 });
    let mut exact = proposal(b"ok"); exact.deadline = ElapsedTick(2); exact.units = 100;
    known(port.check(1, &exact).unwrap(), FilePrecheckOutcome::PrecheckPassed, 1);
    assert_eq!(root.files(), files);
}

#[test]
fn any_acknowledged_transition_invalidates_old_disclosure_without_reporting_private_sequences() {
    let root = Root::new(); let (host, _) = owner(&root); let (_, mut supervisor) = host.into_actor_gateway();
    let (old, grant) = disclose(&supervisor, 3);
    {
        let mut host = supervisor.host_mut().unwrap(); let revision = host.revision();
        host.observe_time(revision, ElapsedTick(2)).unwrap();
    }
    unknown(&old); assert_eq!(grant.remaining_queries(), 3);
    let (fresh, _grant) = disclose(&supervisor, 1);
    assert_eq!(known(fresh.check(1, &proposal(b"ok")).unwrap(), FilePrecheckOutcome::PrecheckPassed, 1)
        .observed_at, ElapsedTick(2));
    unknown(&old);
}

#[test]
fn ports_and_custodians_do_not_keep_a_dropped_authority_lock_alive() {
    let root = Root::new(); let (host, _) = owner(&root); let (_, supervisor) = host.into_actor_gateway();
    let (old, grant) = disclose(&supervisor, 2); drop(supervisor); unknown(&old);
    let (mut reopened, _) = FileOversight::open(root.store(), profile()).unwrap();
    reopened.observe_time(reopened.revision(), ElapsedTick(2)).unwrap();
    unknown(&old); assert_eq!(grant.remaining_queries(), 2);
    let (_, supervisor) = reopened.into_actor_gateway();
    assert!(supervisor.disclose_exact_precheck(supervisor.host().unwrap().revision(), 82, 1).is_err());
}

#[test]
fn disclosure_admission_and_borrow_refusals_do_no_query_work() {
    let root = Root::new(); let (host, _) = owner(&root); let (_, mut supervisor) = host.into_actor_gateway();
    let revision = supervisor.host().unwrap().revision(); let files = root.files();
    for (id, queries, error) in [(0, 1, Error::InvalidInput), (1, 0, Error::InvalidInput),
        (1, MAX_PRECHECK_QUERIES + 1, Error::Limit)] {
        assert_eq!(supervisor.disclose_exact_precheck(revision, id, queries).err(), Some(error.into()));
    }
    assert_eq!(supervisor.disclose_exact_precheck(revision + 1, 81, 1).err(), Some(Error::Stale.into()));
    let (port, grant) = disclose(&supervisor, 1);
    {
        let _borrow = supervisor.host_mut().unwrap(); unknown(&port);
        assert_eq!(grant.remaining_queries(), 1);
    }
    known(port.check(1, &proposal(b"ok")).unwrap(), FilePrecheckOutcome::PrecheckPassed, 1);
    assert_eq!(root.files(), files);
    let debug = format!("{port:?} {grant:?}");
    assert!(!debug.contains("ExactValue")); assert!(!debug.contains("not disclosed"));
    assert!(!debug.contains("snapshot"));
}

#[test]
fn payload_bounds_precede_allocation_and_an_empty_payload_is_an_explicit_candidate() {
    let root = Root::new(); let (host, _) = owner(&root); let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = disclose(&supervisor, 3); let files = root.files();
    let mut candidate = proposal(&vec![0xff; MAX_PAYLOAD_BYTES + 1]);
    candidate.units = candidate.payload.len() as u64;
    assert_eq!(port.check(1, &candidate).err(), Some(ActorError::Capacity));
    candidate.payload.pop(); candidate.units -= 1;
    known(port.check(1, &candidate).unwrap(), FilePrecheckOutcome::DeniedByPolicy, 1);
    assert_eq!(known(port.check(1, &proposal(b"")).unwrap(), FilePrecheckOutcome::PrecheckPassed, 1)
        .observed_at, ElapsedTick(1));
    assert_eq!(grant.remaining_queries(), 0); assert_eq!(root.files(), files);
}

#[test]
fn a_source_backed_stream_cannot_obtain_a_raw_proposal_precheck_fallback() {
    use crate::action::consequence::delivery::stream::StreamProfile;
    let root = Root::new(); let mut p = profile(); p.delivery.initial_payload.clear();
    let stream = StreamProfile::new(31, 1, 2, 16, 32).unwrap();
    let (mut host, _) = FileOversight::create_stream(root.store(), p.clone(), stream).unwrap();
    host.enable_file_source(host.revision(), FileSourcePolicy {
        source: StateSource { scope: p.delivery.scope, source: 51, generation: 1 },
        limits: StateLimits::default(), freshness: StateFreshness::new(5).unwrap(),
    }).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); write_source(&root, 1, b"ok");
    let mut source = FileEvidenceSource::new(root.source(), 51, p.delivery.scope, 1048576).unwrap();
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1)).unwrap();
    assert!(host.capture_file_policy_state().is_ok());
    let (_, supervisor) = host.into_actor_gateway(); let files = root.files();
    assert_eq!(supervisor.disclose_exact_precheck(supervisor.host().unwrap().revision(), 81, 1).err(),
        Some(Error::WrongState.into()));
    assert_eq!(root.files(), files);
    // An otherwise equivalent ordinary source-backed owner remains supported.
    let ordinary = Root::new(); let (host, _) = owner(&ordinary); let (_, supervisor) = host.into_actor_gateway();
    let (port, _grant) = disclose(&supervisor, 1);
    known(port.check(1, &proposal(b"ok")).unwrap(), FilePrecheckOutcome::PrecheckPassed, 1);
}
