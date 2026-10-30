use super::*;
use crate::action::{ActionState, ElapsedTick, Purpose, ResolvedTarget, Scope};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalLimits};
use crate::action::consequence::delivery::persistent::observed::{FileOversight, FileOversightProfile,
    source::FileSourcePolicy};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract,
    actor::{ActorProposal, UnknownReason}, human::HumanReviewPolicy,
    evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource},
    policy_state::{StateFreshness, StateLimits, StateSource}};
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use crate::Snapshot;
use std::{collections::BTreeMap, fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture { root: PathBuf, source: FileEvidenceSource }
impl Fixture {
    fn new() -> (Self, FileOversight) {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-cap-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&root).unwrap();
        let scope = Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect };
        let target = proposal().target;
        let profile = FileOversightProfile {
            delivery: FileDeliveryProfile {
                scope, total: 100, max_attempts: 8,
                actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
                    model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
                    grade: RestartGrade::AuditOnly }, vec![], vec![1], vec![2], 0).unwrap(),
                suspend_at_incident: 3,
                policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap(),
                congress: CongressPolicy { generation: 1, members: BTreeMap::from([("r".into(),
                    MemberPolicy { cohort: "test".into(), weight: 1 })]), caps: Caps { per_member: 1, per_cohort: 1 },
                    continue_minimum: 1, continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3,
                    minimum_members: 1, minimum_cohorts: 1 },
                narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
                retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
            },
            committee: CommitteeContract::new(BTreeMap::from([("r".into(), HelperContract::new(
                InputProfileBinding { profile_id: 1, profile_bytes: b"private-profile".to_vec(),
                    tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 }, 7, b"private-question".to_vec()).unwrap())])).unwrap(),
            human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
        };
        let (mut host, _) = FileOversight::create(root.join("store"), profile).unwrap();
        host.enable_file_source(host.revision(), FileSourcePolicy {
            source: StateSource { scope, source: 51, generation: 1 }, limits: StateLimits::default(),
            freshness: StateFreshness::new(5).unwrap(),
        }).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let source_path = root.join("source.json");
        let evidence = EvidenceSnapshot::new(EvidenceIdentity { source: 51, generation: 1, scope }, snapshot(),
            BTreeMap::from([("r".into(), b"private-helper-input".to_vec())])).unwrap();
        fs::write(&source_path, evidence.encode()).unwrap();
        let mut source = FileEvidenceSource::new(source_path, 51, scope, 1048576).unwrap();
        host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1)).unwrap();
        (Self { root, source }, host)
    }
    fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fs::read_dir(self.root.join("store")).unwrap().map(|entry| entry.unwrap().path())
            .filter(|path| path.is_file()).map(|path| { let bytes = fs::read(&path).unwrap(); (path, bytes) }).collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.root) { eprintln!("discovery fixture cleanup: {error}"); }
    }
}
fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }
}
fn proposal() -> ActorProposal {
    ActorProposal { target: ResolvedTarget { adapter: 10, object: 11, contract_version: 1,
        expected_version: 1, generation: 1 }, payload: b"ok".to_vec(), units: 8,
        deadline: ElapsedTick(100), expected_policy_epoch: 0 }
}
fn response(query: u64, epoch: u64, remaining_queries: usize) -> PrecheckCapabilitiesResponse {
    PrecheckCapabilitiesResponse { query: Some(query), result: Ok(Knowledge::Known {
        value: PrecheckCapabilities { remaining_queries, witness_cache: false, full_policy: false }, basis: FilePrecheckBasis {
            disclosure: u64::MAX, query, policy_epoch: epoch, observed_at: ElapsedTick(u64::MAX),
        },
    }) }
}

#[test]
fn discovery_is_source_bound_read_only_and_not_an_admission_snapshot_consumer() {
    let (fixture, host) = Fixture::new(); let (actor, mut supervisor) = host.into_actor_gateway();
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
    let (port, grant) = supervisor.disclose_exact_precheck(revision, 81, 3).unwrap();
    let before = supervisor.host().unwrap().inspect(); let files = fixture.files();
    let reads = fixture.source.status().read_attempts;
    let result = port.capabilities(901, 0).unwrap();
    let Knowledge::Known { value, basis } = &result else { panic!("expected capabilities") };
    assert_eq!(value.remaining_queries(), 2);
    assert_eq!((basis.disclosure, basis.query, basis.policy_epoch, basis.observed_at), (81, 901, 0, ElapsedTick(1)));
    let bytes = PrecheckCapabilitiesResponse { query: Some(901), result: Ok(result.clone()) }.encode().unwrap();
    assert_eq!(decode_capabilities_response(&bytes, 901, 0).unwrap().result, Ok(result));
    let json = String::from_utf8(bytes).unwrap();
    for private in ["private-profile", "private-question", "private-helper-input", "control_sequence"] {
        assert!(!json.contains(private));
    }
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(fixture.files(), files); assert_eq!(fixture.source.status().read_attempts, reads);
    assert_eq!(grant.remaining_queries(), 2);
    let ticket = actor.submit(901, &proposal()).unwrap();
    assert_eq!(actor.poll(&ticket), Knowledge::Pending { request: 901 });
    assert_eq!(supervisor.host().unwrap().inspect().control.ledger.stages[&1], ActionState::Reviewing);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
}

#[test]
fn discovery_and_evaluation_share_one_allowance_across_clones_and_bad_queries() {
    let (_fixture, host) = Fixture::new(); let (_, supervisor) = host.into_actor_gateway();
    let revision = supervisor.host().unwrap().revision();
    let (port, grant) = supervisor.disclose_exact_precheck(revision, 81, 4).unwrap();
    let clone = port.clone();
    assert_eq!(port.capabilities(0, 0), Err(ActorError::MalformedProposal));
    assert_eq!(clone.capabilities(1, 9).unwrap(), Knowledge::Stale { generation: 9, current: 0 });
    assert!(matches!(port.check(1, &proposal()).unwrap(), Knowledge::Known { .. }));
    let Knowledge::Known { value, .. } = clone.capabilities(1, 0).unwrap() else { panic!("known") };
    assert_eq!(value.remaining_queries(), 0); assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(port.capabilities(1, 0), Err(ActorError::Capacity));
    assert_eq!(clone.check(1, &proposal()), Err(ActorError::Capacity));
}

#[test]
fn unavailable_owner_borrow_revision_and_revocation_never_disclose_or_spend() {
    let (_fixture, host) = Fixture::new(); let (_, mut supervisor) = host.into_actor_gateway();
    let revision = supervisor.host().unwrap().revision();
    let (port, grant) = supervisor.disclose_exact_precheck(revision, 81, 4).unwrap();
    let unavailable = Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable };
    {
        let mut host = supervisor.host_mut().unwrap();
        assert_eq!(port.capabilities(1, 0).unwrap(), unavailable);
        host.observe_time(revision, ElapsedTick(7)).unwrap();
    }
    assert_eq!(port.capabilities(1, 0).unwrap(), unavailable);
    assert_eq!(grant.remaining_queries(), 4);
    drop(supervisor);
    assert_eq!(port.capabilities(1, 0).unwrap(), unavailable);
    assert_eq!(grant.remaining_queries(), 4);
    grant.revoke();
    assert_eq!(port.capabilities(1, 0).unwrap(), Knowledge::Withheld { authority_required: "exact_policy_disclosure" });
}

#[test]
fn capabilities_request_preserves_full_width_ids_and_rejects_other_operations() {
    let request = PrecheckCapabilitiesRequest { query: u64::MAX, expected_policy_epoch: u64::MAX };
    let bytes = request.encode().unwrap(); assert!(bytes.len() <= MAX_CAPABILITIES_REQUEST_BYTES);
    assert_eq!(decode_capabilities_request(&bytes).unwrap(), request);
    let text = String::from_utf8(bytes).unwrap();
    for invalid in [
        text.replace("capabilities\"", "precheck\""),
        text.replace("/1\"", "/2\""), text.replace("\"query\":\"18446744073709551615\"", "\"query\":0"),
        text.replace("\"query\":\"18446744073709551615\"", "\"query\":\"01\""),
        text.replacen('{', "{\"query\":\"1\",", 1),
        text.replacen('{', "{\"payload_hex\":\"\",", 1),
        format!("{text}{{}}"),
    ] { assert!(decode_capabilities_request(invalid.as_bytes()).is_err()); }
    assert!(PrecheckCapabilitiesRequest { query: 0, expected_policy_epoch: 0 }.encode().is_err());
    assert!(decode_capabilities_request(&vec![b' '; MAX_CAPABILITIES_REQUEST_BYTES + 1]).is_err());
}

#[test]
fn discovery_response_roundtrips_all_legal_refusals_and_full_width_basis() {
    let known = response(u64::MAX, u64::MAX, MAX_PRECHECK_QUERIES - 1);
    let bytes = known.encode().unwrap(); assert!(bytes.len() <= super::super::MAX_PRECHECK_RESPONSE_BYTES);
    assert_eq!(decode_capabilities_response(&bytes, u64::MAX, u64::MAX).unwrap(), known);
    for result in [
        Ok(Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable }),
        Ok(Knowledge::Withheld { authority_required: "exact_policy_disclosure" }),
        Ok(Knowledge::Stale { generation: 0, current: 1 }),
        Err(PrecheckWireError::Capacity), Err(PrecheckWireError::Unavailable), Err(PrecheckWireError::Withheld),
    ] {
        let response = PrecheckCapabilitiesResponse { query: Some(1), result };
        assert_eq!(decode_capabilities_response(&response.encode().unwrap(), 1, 0).unwrap(), response);
    }
    for error in [PrecheckWireError::MalformedRequest, PrecheckWireError::UnsupportedVersion, PrecheckWireError::Capacity] {
        let response = PrecheckCapabilitiesResponse { query: None, result: Err(error) };
        assert_eq!(decode_capabilities_response(&response.encode().unwrap(), 1, 0).unwrap(), response);
    }
}

#[test]
fn discovery_refuses_claimed_authority_private_feedback_and_uncorrelated_results() {
    let encoded = String::from_utf8(response(1, 0, 2).encode().unwrap()).unwrap();
    for invalid in [
        encoded.replace("\"informational\":true", "\"informational\":false"),
        encoded.replace("\"effect_authority\":false", "\"effect_authority\":true"),
        encoded.replace("\"signed_profile\":false", "\"signed_profile\":true"),
        encoded.replace("\"review_required\":true", "\"review_required\":false"),
        encoded.replace("exact_predicates", "full"), encoded.replace("exact_policy", "congress_dry_run"),
        encoded.replace("\"helper_feedback\":\"withheld\"", "\"helper_feedback\":\"public\""),
        encoded.replace("\"remaining_queries\":\"2\"", "\"remaining_queries\":\"128\""),
        encoded.replace("\"query\":\"1\"", "\"query\":\"2\""),
        encoded.replace("\"policy_epoch\":\"0\"", "\"policy_epoch\":\"1\""),
        encoded.replacen('{', "{\"permit\":true,", 1), encoded.replacen('{', "{\"schema\":\"duplicate\",", 1),
    ] { assert!(decode_capabilities_response(invalid.as_bytes(), 1, 0).is_err()); }
    assert!(super::super::decode_precheck_response(encoded.as_bytes(), 1, 0).is_err());
    for result in [Ok(Knowledge::Pending { request: 1 }), Ok(Knowledge::Absent { closed_domain: 1, frontier: 1 }),
        Ok(Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown })] {
        assert!(PrecheckCapabilitiesResponse { query: Some(1), result }.encode().is_err());
    }
    assert!(PrecheckCapabilitiesResponse { query: None, result: response(1, 0, 2).result }.encode().is_err());
}

#[test]
fn original_precheck_success_and_denial_bytes_remain_unchanged() {
    use super::super::PrecheckWireResponse;
    use super::super::super::FilePrecheckOutcome;
    for (value, body) in [
        (FilePrecheckOutcome::DeniedByPolicy, "\"value\":\"denied_by_policy\""),
        (FilePrecheckOutcome::PrecheckPassed, "\"value\":\"precheck_passed\",\"review_required\":true"),
    ] {
        let response = PrecheckWireResponse { query: Some(1), result: Ok(Knowledge::Known { value,
            basis: FilePrecheckBasis { disclosure: 81, query: 1, policy_epoch: 0, observed_at: ElapsedTick(1) },
        }) };
        let expected = format!("{{\"schema\":\"fa.actor-precheck-response/1\",\"precheck\":true,\"query\":\"1\",\"status\":\"ok\",\"knowledge\":{{\"state\":\"known\",{body},\"basis\":{{\"disclosure\":\"81\",\"query\":\"1\",\"policy_epoch\":\"0\",\"observed_at\":\"1\"}}}}}}");
        assert_eq!(response.encode().unwrap(), expected.as_bytes());
        assert_eq!(super::super::decode_precheck_response(expected.as_bytes(), 1, 0).unwrap(), response);
        assert!(decode_capabilities_response(expected.as_bytes(), 1, 0).is_err());
    }
}

mod transport;
