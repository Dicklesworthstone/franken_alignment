//! Original durable owners and wire accounting, with the optional cache selected.
use super::*;
use super::tests::{Fixture, request};
use super::cache::PrecheckCacheLimits;
use super::discovery::{PrecheckCapabilitiesRequest, decode_capabilities_response};
use super::super::{FilePrecheckBasis, FilePrecheckObservation, FilePrecheckOutcome};
use crate::{Error, action::{ActionState, ElapsedTick}};
use crate::action::consequence::oversight::actor::{Knowledge, UnknownReason};

fn known(value: FilePrecheckObservation, expected: FilePrecheckOutcome) -> FilePrecheckBasis {
    let Knowledge::Known { value, basis } = value else { panic!("expected exact precheck result") };
    assert_eq!(value, expected); basis
}

#[test]
fn cached_wire_queries_still_spend_work_and_normal_submission_still_enters_review() {
    let (fixture, _) = Fixture::new(16, 16);
    let cache = fixture.supervisor.precheck_cache(PrecheckCacheLimits::default()).unwrap();
    let before = fixture.supervisor.host().unwrap().inspect();
    let files = fixture.root.files(); let reads = fixture.source.status().read_attempts;
    let revision = fixture.supervisor.host().unwrap().revision();
    let (port, grant) = fixture.supervisor.disclose_exact_precheck_cached(revision, 82, 8, &cache).unwrap();
    let mut wire = FilePrecheckWire::new(port.clone(), 8).unwrap();
    for query in [901, 902] {
        let response = wire.exchange(&encode_precheck_request(&request(query, b"ok")).unwrap());
        let basis = known(response.result.unwrap(), FilePrecheckOutcome::PrecheckPassed);
        assert_eq!((basis.disclosure, basis.query), (82, query));
    }
    let bytes = wire.exchange_frame(&PrecheckCapabilitiesRequest { query: 903, expected_policy_epoch: 0 }.encode().unwrap()).unwrap();
    let Knowledge::Known { value, .. } = decode_capabilities_response(&bytes, 903, 0).unwrap().result.unwrap() else { panic!("discovery") };
    assert!(value.witness_cache_enabled()); assert_eq!(value.remaining_queries(), 5);
    assert_eq!((grant.remaining_queries(), wire.remaining_exchanges()), (5, 5));
    for query in [904, 905] {
        known(port.check(query, &request(query, b"denied").proposal).unwrap(), FilePrecheckOutcome::DeniedByPolicy);
    }
    assert_eq!((cache.status().unwrap().evaluations, cache.status().unwrap().hits), (2, 2));
    assert_eq!(fixture.supervisor.host().unwrap().inspect(), before);
    assert_eq!(fixture.root.files(), files); assert_eq!(fixture.source.status().read_attempts, reads);
    let ticket = fixture.actor.submit(901, &request(901, b"ok").proposal).unwrap();
    assert_eq!(fixture.actor.poll(&ticket), Knowledge::Pending { request: 901 });
    assert_eq!(fixture.supervisor.host().unwrap().inspect().control.ledger.stages[&1], ActionState::Reviewing);
    assert_eq!(fixture.supervisor.host().unwrap().inspect().executions, 0);
}

#[test]
fn only_a_new_supervisor_grant_can_reuse_evaluation_after_an_owner_transition() {
    let (mut fixture, _) = Fixture::new(8, 8);
    let cache = fixture.supervisor.precheck_cache(PrecheckCacheLimits::default()).unwrap();
    let revision = fixture.supervisor.host().unwrap().revision();
    let (old, old_grant) = fixture.supervisor.disclose_exact_precheck_cached(revision, 82, 3, &cache).unwrap();
    known(old.check(7, &request(7, b"ok").proposal).unwrap(), FilePrecheckOutcome::PrecheckPassed);
    {
        let mut host = fixture.supervisor.host_mut().unwrap();
        host.observe_time(revision, ElapsedTick(2)).unwrap();
    }
    let files = fixture.root.files(); let before = cache.status().unwrap();
    assert_eq!(old.check(8, &request(8, b"ok").proposal).unwrap(),
        Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable });
    assert_eq!(old_grant.remaining_queries(), 2); assert_eq!(cache.status().unwrap(), before);
    let revision = fixture.supervisor.host().unwrap().revision();
    let (fresh, _grant) = fixture.supervisor.disclose_exact_precheck_cached(revision, 83, 3, &cache).unwrap();
    let basis = known(fresh.check(9, &request(9, b"ok").proposal).unwrap(), FilePrecheckOutcome::PrecheckPassed);
    assert_eq!((basis.disclosure, basis.query, basis.observed_at), (83, 9, ElapsedTick(2)));
    assert_eq!((cache.status().unwrap().evaluations, cache.status().unwrap().hits), (1, 1));
    assert_eq!(fixture.root.files(), files);
    assert!(matches!(old.check(10, &request(10, b"ok").proposal).unwrap(), Knowledge::Unknown { .. }));
}

#[test]
fn a_cached_policy_pass_never_overrides_the_new_grants_deadline_screen() {
    let (mut fixture, _) = Fixture::new(8, 8);
    let cache = fixture.supervisor.precheck_cache(PrecheckCacheLimits::default()).unwrap();
    let revision = fixture.supervisor.host().unwrap().revision();
    let (port, _grant) = fixture.supervisor.disclose_exact_precheck_cached(revision, 82, 3, &cache).unwrap();
    let mut proposal = request(7, b"ok").proposal; proposal.deadline = ElapsedTick(2);
    known(port.check(7, &proposal).unwrap(), FilePrecheckOutcome::PrecheckPassed);
    {
        let mut host = fixture.supervisor.host_mut().unwrap(); host.observe_time(revision, ElapsedTick(2)).unwrap();
    }
    let revision = fixture.supervisor.host().unwrap().revision();
    let (fresh, _fresh_grant) = fixture.supervisor.disclose_exact_precheck_cached(revision, 83, 3, &cache).unwrap();
    known(fresh.check(8, &proposal).unwrap(), FilePrecheckOutcome::DeniedByPolicy);
    assert_eq!((cache.status().unwrap().evaluations, cache.status().unwrap().hits), (1, 0));
}

#[test]
fn query_exhaustion_and_revocation_are_checked_before_cache_hits() {
    let (fixture, _) = Fixture::new(8, 8);
    let cache = fixture.supervisor.precheck_cache(PrecheckCacheLimits::default()).unwrap();
    let revision = fixture.supervisor.host().unwrap().revision();
    let (port, grant) = fixture.supervisor.disclose_exact_precheck_cached(revision, 82, 2, &cache).unwrap();
    for _ in 0..2 { known(port.check(7, &request(7, b"ok").proposal).unwrap(), FilePrecheckOutcome::PrecheckPassed); }
    let before = cache.status().unwrap();
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(port.check(7, &request(7, b"ok").proposal), Err(ActorError::Capacity));
    assert_eq!(port.capabilities(7, 0), Err(ActorError::Capacity));
    grant.revoke();
    assert_eq!(port.check(7, &request(7, b"ok").proposal).unwrap(),
        Knowledge::Withheld { authority_required: "exact_policy_disclosure" });
    assert_eq!(cache.status().unwrap(), before);
}

#[test]
fn cache_identity_is_not_transferable_to_an_equal_looking_owner() {
    let (first, _) = Fixture::new(8, 8); let (second, _) = Fixture::new(8, 8);
    let cache = first.supervisor.precheck_cache(PrecheckCacheLimits::default()).unwrap();
    let revision = second.supervisor.host().unwrap().revision();
    assert_eq!(second.supervisor.disclose_exact_precheck_cached(revision, 82, 2, &cache).err(), Some(Error::Binding.into()));
    assert_eq!(cache.status().unwrap().entries, 0);
}

#[test]
fn expired_or_withdrawn_source_cannot_be_replaced_by_a_warm_cache() {
    for withdraw in [false, true] {
        let (mut fixture, _) = Fixture::new(8, 8);
        let cache = fixture.supervisor.precheck_cache(PrecheckCacheLimits::default()).unwrap();
        let revision = fixture.supervisor.host().unwrap().revision();
        let (port, grant) = fixture.supervisor.disclose_exact_precheck_cached(revision, 82, 3, &cache).unwrap();
        known(port.check(7, &request(7, b"ok").proposal).unwrap(), FilePrecheckOutcome::PrecheckPassed);
        {
            let mut host = fixture.supervisor.host_mut().unwrap();
            if withdraw { host.withdraw_file_source(revision).unwrap(); }
            else { host.observe_time(revision, ElapsedTick(6)).unwrap(); }
        }
        let revision = fixture.supervisor.host().unwrap().revision(); let before = cache.status().unwrap();
        assert!(fixture.supervisor.disclose_exact_precheck_cached(revision, 83, 3, &cache).is_err());
        assert!(matches!(port.check(8, &request(8, b"ok").proposal).unwrap(), Knowledge::Unknown { .. }));
        assert_eq!(grant.remaining_queries(), 2); assert_eq!(cache.status().unwrap(), before);
    }
}

#[test]
fn a_cache_handle_does_not_enable_ordinary_grants_or_keep_an_owner_alive() {
    let (fixture, mut wire) = Fixture::new(8, 8);
    let cache = fixture.supervisor.precheck_cache(PrecheckCacheLimits::default()).unwrap();
    let bytes = wire.exchange_frame(&PrecheckCapabilitiesRequest { query: 7, expected_policy_epoch: 0 }.encode().unwrap()).unwrap();
    let Knowledge::Known { value, .. } = decode_capabilities_response(&bytes, 7, 0).unwrap().result.unwrap() else { panic!("discovery") };
    assert!(!value.witness_cache_enabled()); assert_eq!(cache.status().unwrap().entries, 0);
    let revision = fixture.supervisor.host().unwrap().revision();
    let (port, _grant) = fixture.supervisor.disclose_exact_precheck_cached(revision, 82, 3, &cache).unwrap();
    known(port.check(7, &request(7, b"ok").proposal).unwrap(), FilePrecheckOutcome::PrecheckPassed);
    drop(fixture.supervisor);
    assert!(matches!(port.check(8, &request(8, b"ok").proposal).unwrap(), Knowledge::Unknown { .. }));
    assert_eq!(cache.status().unwrap().hits, 0);
}

#[test]
fn cached_discovery_keeps_strict_correlation_and_full_width_response_bounds() {
    let (fixture, _) = Fixture::new(8, 8);
    let cache = fixture.supervisor.precheck_cache(PrecheckCacheLimits::default()).unwrap();
    let revision = fixture.supervisor.host().unwrap().revision();
    let (port, _grant) = fixture.supervisor.disclose_exact_precheck_cached(revision, u64::MAX, 128, &cache).unwrap();
    let mut response = discovery::PrecheckCapabilitiesResponse { query: Some(u64::MAX), result: port.capabilities(u64::MAX, 0).map_err(PrecheckWireError::from) };
    if let Ok(Knowledge::Known { basis, .. }) = &mut response.result {
        basis.policy_epoch = u64::MAX; basis.observed_at = ElapsedTick(u64::MAX);
    }
    let bytes = response.encode().unwrap(); assert!(bytes.len() <= MAX_PRECHECK_RESPONSE_BYTES);
    assert_eq!(decode_capabilities_response(&bytes, u64::MAX, u64::MAX).unwrap(), response);
    let text = String::from_utf8(bytes).unwrap();
    for claim in ["full", "replayed_congress", "unbounded"] {
        assert!(decode_capabilities_response(text.replace("exact_witnesses", claim).as_bytes(), u64::MAX, u64::MAX).is_err());
    }
    assert!(decode_precheck_response(text.as_bytes(), u64::MAX, u64::MAX).is_err());
}
