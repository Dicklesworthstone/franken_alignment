//! Real source-backed grants plus exact codec boundaries; not deployment proof.
use super::*;
use super::super::{tests::{Fixture, request}, decode_precheck_response};
use crate::action::{ActionState, ElapsedTick, ResolvedTarget};
use crate::action::consequence::gate::containment::session::policy::{
    Predicate, MAX_POLICY_NODES, MAX_POLICY_EDGES, MAX_POLICY_LITERAL_BYTES,
};
use crate::action::consequence::oversight::actor::UnknownReason;

fn response(policy: Policy) -> PolicyResponse {
    PolicyResponse { query: Some(u64::MAX), result: Ok(Knowledge::Known {
        value: DisclosedPolicy { definition: policy }, basis: FilePrecheckBasis {
            disclosure: u64::MAX, query: u64::MAX, policy_epoch: u64::MAX,
            observed_at: ElapsedTick(u64::MAX),
        },
    }) }
}
fn decode(bytes: &[u8]) -> Result<PolicyResponse, PrecheckResponseError> {
    decode_policy_response(bytes, u64::MAX, u64::MAX)
}
fn simple() -> Policy { Policy::new(1, vec![Predicate::PayloadIs(b"policy-literal".to_vec())]).unwrap() }

#[test]
fn explicit_full_disclosure_is_read_only_and_actual_submission_still_reviews() {
    let (fixture, _wire) = Fixture::new(8, 8);
    let revision = fixture.supervisor.host().unwrap().revision();
    let (full, grant) = fixture.supervisor.disclose_full_precheck(revision, 82, 4, None).unwrap();
    let files = fixture.root.files(); let before = fixture.supervisor.host().unwrap().inspect();
    let reads = fixture.source.status().read_attempts;
    let Knowledge::Known { value, basis } = full.policy(901, 0).unwrap() else { panic!("policy") };
    assert_eq!(value.definition().nodes(), &[
        Predicate::ExactValue { key: 7, value: b"ok".to_vec() },
        Predicate::PayloadAtMost(4), Predicate::All(vec![0, 1]),
    ]);
    assert_eq!((basis.disclosure, basis.query, basis.policy_epoch, basis.observed_at), (82, 901, 0, ElapsedTick(1)));
    let bytes = PolicyResponse { query: Some(901), result: Ok(Knowledge::Known { value, basis }) }.encode().unwrap();
    let text = String::from_utf8(bytes.clone()).unwrap();
    for secret in ["private review input", "precheck-transport", "control_sequence", "reserved", "stopped"] {
        assert!(!text.contains(secret));
    }
    assert!(!format!("{full:?}").contains("ok"));
    assert_eq!(grant.remaining_queries(), 3);
    assert_eq!(fixture.root.files(), files);
    assert_eq!(fixture.supervisor.host().unwrap().inspect(), before);
    assert_eq!(fixture.source.status().read_attempts, reads);
    assert!(decode_precheck_response(&bytes, 901, 0).is_err());
    let ticket = fixture.actor.submit(901, &request(901, b"ok").proposal).unwrap();
    assert_eq!(fixture.actor.poll(&ticket), Knowledge::Pending { request: 901 });
    assert_eq!(fixture.supervisor.host().unwrap().inspect().control.ledger.stages[&1], ActionState::Reviewing);
    assert_eq!(full.policy(902, 0).unwrap(), Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable });
    assert_eq!(grant.remaining_queries(), 3);
}

#[test]
fn full_clones_exact_checks_discovery_and_invalid_reads_share_one_budget() {
    let (fixture, _wire) = Fixture::new(8, 8);
    let (full, grant) = fixture.supervisor.disclose_full_precheck(
        fixture.supervisor.host().unwrap().revision(), 82, 5, None).unwrap();
    let clone = full.clone(); let exact = full.prechecks();
    assert_eq!(full.policy(0, 0), Err(ActorError::MalformedProposal));
    assert_eq!(clone.policy(7, 9).unwrap(), Knowledge::Stale { generation: 9, current: 0 });
    assert!(matches!(exact.check(7, &request(7, b"ok").proposal).unwrap(), Knowledge::Known { .. }));
    assert!(matches!(exact.capabilities(7, 0).unwrap(), Knowledge::Known { .. }));
    assert!(matches!(full.policy(7, 0).unwrap(), Knowledge::Known { .. }));
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(clone.policy(7, 0), Err(ActorError::Capacity));
    assert_eq!(exact.check(7, &request(7, b"ok").proposal), Err(ActorError::Capacity));
    grant.revoke();
    assert_eq!(clone.policy(7, 0).unwrap(), Knowledge::Withheld { authority_required: "exact_policy_disclosure" });
}

#[test]
fn source_expiry_busy_owner_and_owner_loss_do_not_reveal_or_charge() {
    let (mut fixture, _wire) = Fixture::new(8, 8);
    let revision = fixture.supervisor.host().unwrap().revision();
    let (full, grant) = fixture.supervisor.disclose_full_precheck(revision, 82, 4, None).unwrap();
    let unknown = Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable };
    {
        let mut host = fixture.supervisor.host_mut().unwrap();
        assert_eq!(full.policy(7, 0).unwrap(), unknown);
        host.observe_time(revision, ElapsedTick(7)).unwrap();
    }
    assert_eq!(full.policy(7, 0).unwrap(), unknown);
    assert_eq!(grant.remaining_queries(), 4);
    drop(fixture);
    assert_eq!(full.policy(7, 0).unwrap(), unknown);
    assert_eq!(grant.remaining_queries(), 4);
}

#[test]
fn cached_full_grants_keep_owner_binding_and_policy_reads_do_not_evaluate() {
    let (fixture, _wire) = Fixture::new(8, 8);
    let cache = fixture.supervisor.precheck_cache(super::super::cache::PrecheckCacheLimits::default()).unwrap();
    let revision = fixture.supervisor.host().unwrap().revision();
    let (full, _grant) = fixture.supervisor.disclose_full_precheck(revision, 82, 4, Some(&cache)).unwrap();
    full.policy(7, 0).unwrap(); assert_eq!(cache.status().unwrap().evaluations, 0);
    let exact = full.prechecks();
    exact.check(8, &request(8, b"ok").proposal).unwrap();
    exact.check(9, &request(9, b"ok").proposal).unwrap();
    assert_eq!(cache.status().unwrap().hits, 1);
    let (other, _other_wire) = Fixture::new(8, 8);
    assert!(other.supervisor.disclose_full_precheck(
        other.supervisor.host().unwrap().revision(), 82, 4, Some(&cache)).is_err());
}

#[test]
fn complete_policy_codec_preserves_every_predicate_and_full_width_identifiers() {
    let target = ResolvedTarget { adapter: u64::MAX, object: u64::MAX, contract_version: u64::MAX,
        expected_version: u64::MAX, generation: u64::MAX };
    let policy = Policy::new(u64::MAX, vec![
        Predicate::TargetIs(target), Predicate::PayloadIs(vec![0, 255]), Predicate::PayloadAtMost(0),
        Predicate::UnitsAtMost(u64::MAX), Predicate::ExactValue { key: 0, value: vec![0, 128, 255] },
        Predicate::Absent { key: u64::MAX }, Predicate::EmptyRange { start: 0, end: u64::MAX },
        Predicate::Any(vec![0, 1]), Predicate::Not(7), Predicate::All(vec![2, 3, 4, 5, 6, 8]),
    ]).unwrap();
    let expected = response(policy); let bytes = expected.encode().unwrap();
    assert_eq!(decode(&bytes).unwrap(), expected);
    assert_eq!(decode(&bytes).unwrap().encode().unwrap(), bytes);
    assert!(!format!("{expected:?}").contains("value_hex"));
}

#[test]
fn maximum_literals_nodes_edges_and_target_fields_fit_the_closed_envelope() {
    let mut chain = vec![Predicate::PayloadIs(vec![255; MAX_POLICY_LITERAL_BYTES])];
    for index in 1..MAX_POLICY_NODES { chain.push(Predicate::Not(index - 1)); }
    let mut targets = vec![Predicate::TargetIs(ResolvedTarget { adapter: u64::MAX, object: u64::MAX,
        contract_version: u64::MAX, expected_version: u64::MAX, generation: u64::MAX }); MAX_POLICY_NODES - 1];
    targets.push(Predicate::All((0..MAX_POLICY_NODES - 1).collect()));
    for nodes in [chain, targets, vec![Predicate::PayloadIs(vec![0; MAX_POLICY_LITERAL_BYTES]),
        Predicate::All(vec![0; MAX_POLICY_EDGES])]] {
        let expected = response(Policy::new(u64::MAX, nodes).unwrap());
        let bytes = expected.encode().unwrap(); assert!(bytes.len() <= MAX_POLICY_RESPONSE_BYTES);
        assert_eq!(decode(&bytes).unwrap(), expected);
    }
    assert_eq!(decode(&vec![b' '; MAX_POLICY_RESPONSE_BYTES + 1]), Err(PrecheckResponseError::Capacity));
}

#[test]
fn policy_codec_rejects_cycles_unreachable_nodes_unknown_ops_and_bad_literals() {
    let good = String::from_utf8(response(simple()).encode().unwrap()).unwrap();
    for changed in [
        good.replace("payload_is", "publish"), good.replace("706f6c6963792d6c69746572616c", "FF"),
        good.replace("706f6c6963792d6c69746572616c", "f"),
        good.replace("{\"op\":\"payload_is\",\"value_hex\":\"706f6c6963792d6c69746572616c\"}", "{\"op\":\"not\",\"child\":\"0\"}"),
        good.replace("{\"op\":\"payload_is\",\"value_hex\":\"706f6c6963792d6c69746572616c\"}", "{\"op\":\"all\",\"children\":[]}"),
        good.replace("\"nodes\":[", "\"nodes\":[{\"op\":\"units_at_most\",\"maximum\":\"1\"},"),
        good.replace("\"generation\":\"1\"", "\"generation\":\"0\""),
        good.replace("\"generation\":\"1\"", "\"generation\":\"01\""),
    ] { assert_ne!(changed, good); assert!(decode(changed.as_bytes()).is_err()); }
}

#[test]
fn policy_observation_never_accepts_authority_private_data_or_wrong_correlation() {
    let good = String::from_utf8(response(simple()).encode().unwrap()).unwrap();
    for changed in [
        good.replace("\"effect_authority\":false", "\"effect_authority\":true"),
        good.replace("\"review_required\":true", "\"review_required\":false"),
        good.replace("\"helper_feedback\":\"withheld\"", "\"helper_feedback\":\"public\""),
        good.replace("\"consequences\":\"withheld\"", "\"consequences\":\"public\""),
        good.replace("\"source_values\":\"withheld\"", "\"source_values\":\"public\""),
        good.replace("\"informational\":true", "\"informational\":false"),
        good.replacen('{', "{\"permit\":true,", 1),
        good.replacen('{', "{\"schema\":\"duplicate\",", 1),
        good.replace("\"query\":\"18446744073709551615\"", "\"query\":\"1\""),
        good.replace("\"policy_epoch\":\"18446744073709551615\"", "\"policy_epoch\":\"1\""),
    ] { assert!(decode(changed.as_bytes()).is_err()); }
    assert!(decode_precheck_response(good.as_bytes(), u64::MAX, u64::MAX).is_err());
    assert!(super::super::discovery::decode_capabilities_response(good.as_bytes(), u64::MAX, u64::MAX).is_err());
}

#[test]
fn policy_request_is_closed_and_preserves_zero_epoch_and_maximum_query() {
    let request = PolicyRequest { query: u64::MAX, expected_policy_epoch: 0 };
    let bytes = request.encode().unwrap(); assert!(bytes.len() <= MAX_POLICY_REQUEST_BYTES);
    assert_eq!(decode_policy_request(&bytes).unwrap(), request);
    let good = String::from_utf8(bytes).unwrap();
    for changed in [good.replace("get", "precheck"), good.replace("/1", "/2"),
        good.replace("18446744073709551615", "01"), good.replace("18446744073709551615", "0"),
        good.replacen('{', "{\"query\":\"2\",", 1), good.replacen('{', "{\"full\":true,", 1),
        format!("{good}{{}}")]
    { assert!(decode_policy_request(changed.as_bytes()).is_err()); }
    assert!(decode_policy_request(&vec![b' '; MAX_POLICY_REQUEST_BYTES + 1]).is_err());
}

#[test]
fn native_refusals_roundtrip_without_fabricating_a_definition() {
    for result in [
        Ok(Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable }),
        Ok(Knowledge::Withheld { authority_required: "exact_policy_disclosure" }),
        Ok(Knowledge::Stale { generation: u64::MAX, current: 0 }),
        Err(PrecheckWireError::Capacity), Err(PrecheckWireError::Unavailable),
    ] {
        let expected = PolicyResponse { query: Some(u64::MAX), result };
        assert_eq!(decode(&expected.encode().unwrap()).unwrap(), expected);
    }
    for result in [Ok(Knowledge::Pending { request: 1 }),
        Ok(Knowledge::Absent { closed_domain: 1, frontier: 1 })] {
        assert!(PolicyResponse { query: Some(1), result }.encode().is_err());
    }
}
