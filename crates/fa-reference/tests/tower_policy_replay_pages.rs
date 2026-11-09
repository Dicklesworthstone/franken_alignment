//! Complete original policy campaigns delivered in bounded, custodied pages.
#![cfg(unix)]

#[path = "support/file_oversight_actor.rs"]
mod support;

#[path = "tower_policy_replay_pages/client.rs"]
mod client;

use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FilePolicyReplayPage, FilePolicyReplayPageResult,
};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::actor::{
    ActorError, Knowledge as ActorKnowledge, UnknownReason as ActorUnknown,
};
use fa_reference::action::consequence::policy_campaign::{PolicyDelta, ReplayCaseId, ReplayLimits};
use fa_reference::address_resolver::{Address, CallerScope, Noun};
use fa_reference::observation_algebra::Knowledge;
use fa_reference::strict_json::{self, ErrorKind, Json, Limits as JsonLimits};
use fa_reference::tower::publication::{
    PublicationLimits, PublicationResponse, PublicationResult, PublicationTower,
};
use fa_reference::tower::{
    ReadArguments, ReasonCode, TowerAccess, TowerEncodingError, TowerRequest, Verb, decode_request,
    encode_request,
};
use fa_reference::typestate_cx::PurposeTag;
use support::{Directory, Port, Supervisor, observe, oversight, profile, ready, snapshot};

const PRIVATE_PAYLOAD: &[u8] = b"PRIVATE-PAYLOAD";
const PRIVATE_WITNESS: &[u8] = b"PRIVATE-WITNESS-BYTES";

fn limits() -> ReplayLimits {
    ReplayLimits {
        cases: 128,
        input_bytes: 1_048_576,
    }
}

fn candidate() -> Policy {
    Policy::new(2, vec![Predicate::PayloadAtMost(128)]).unwrap()
}

fn mount() -> Address {
    Address::new("1", Noun::Run, "3", Some(41)).unwrap()
}

fn tower(actor: Port, max_response_bytes: usize) -> PublicationTower {
    PublicationTower::new(mount(), actor, PublicationLimits { max_response_bytes }).unwrap()
}

fn access() -> TowerAccess {
    TowerAccess::new(
        CallerScope::full_tenant_access("1"),
        true,
        vec![PurposeTag::Experiment],
        0,
    )
}

fn query(arguments: ReadArguments, campaign: u64) -> TowerRequest {
    let mut request = TowerRequest::new("compare-policy-pages", mount(), Verb::PolicyReplay);
    request.arguments = arguments;
    request.idempotency_key = Some(campaign.to_string());
    request
}

fn legacy(campaign: u64, candidate: Policy) -> TowerRequest {
    query(
        ReadArguments::PolicyReplay {
            campaign,
            expected_policy_generation: 1,
            candidate,
        },
        campaign,
    )
}

fn first(campaign: u64, candidate: Policy) -> TowerRequest {
    query(
        ReadArguments::PolicyReplayPaged {
            campaign,
            expected_policy_generation: 1,
            candidate,
        },
        campaign,
    )
}

fn continued(campaign: u64, cursor: u64) -> TowerRequest {
    query(
        ReadArguments::PolicyReplayContinue { campaign, cursor },
        campaign,
    )
}

fn native(response: &PublicationResponse) -> &FilePolicyReplayPageResult {
    let PublicationResult::PolicyReplayPage(linked) = &response.result else {
        panic!("expected original native policy page: {response:?}");
    };
    no_global_claims(response);
    linked.native()
}

fn known(response: &PublicationResponse) -> &FilePolicyReplayPage {
    let Ok(ActorKnowledge::Known { value, basis }) = native(response) else {
        panic!("expected a native retained page: {response:?}");
    };
    assert_eq!(value.basis(), *basis);
    assert_eq!(response.envelope.reason_code, None);
    value
}

fn known_native(result: FilePolicyReplayPageResult) -> FilePolicyReplayPage {
    let Ok(ActorKnowledge::Known { value, basis }) = result else {
        panic!("expected a native retained page: {result:?}");
    };
    assert_eq!(value.basis(), basis);
    value
}

fn page_body(document: &Json) -> &Json {
    field(field(wire_value(document), "knowledge"), "value")
}

fn old_client_limits() -> JsonLimits {
    JsonLimits {
        max_bytes: 1_048_576,
        max_depth: 16,
        max_items: 4096,
        max_string_bytes: 131_072,
    }
}

fn page_document(response: &PublicationResponse) -> Json {
    let encoded = response.to_json().unwrap();
    assert!(encoded.len() <= 16 * 1024);
    for secret in [PRIVATE_PAYLOAD, PRIVATE_WITNESS, b"PRIVATE-SOURCE-BYTES"] {
        assert!(!encoded.contains(std::str::from_utf8(secret).unwrap()));
        let hex: String = secret.iter().map(|byte| format!("{byte:02x}")).collect();
        assert!(!encoded.contains(&hex));
    }
    let document = strict_json::parse(encoded.as_bytes(), old_client_limits()).unwrap();
    let body = wire_value(&document);
    assert_eq!(
        field(body, "schema").as_str(),
        Some("fa.publication-policy-replay/2")
    );
    assert_eq!(field(body, "historical").as_bool(), Some(true));
    assert_eq!(field(body, "rehearsal").as_bool(), Some(true));
    for name in [
        "policy_changed",
        "helpers_rerun",
        "live_evidence_read",
        "effect_authority",
        "promotion_authority",
        "payload_disclosed",
        "witness_values_disclosed",
    ] {
        assert_eq!(field(body, name).as_bool(), Some(false));
    }
    assert_eq!(
        field(body, "current_eligibility").as_str(),
        Some("not_observed")
    );
    document
}

fn image(root: &Directory) -> Vec<u8> {
    std::fs::read(root.store().join("delivery.bin")).unwrap()
}

/// One real admitted/reviewed action plus original policy-denied proposals.
/// Only the configured attempt capacity grows for this scale test. The original
/// total allowance, action charge, target, policy and review contract stay intact.
fn corpus(root: &Directory, denials: u64) -> (Port, Supervisor, oversight::Keys) {
    assert!(denials < 120);
    let mut selected = profile();
    selected.delivery.max_attempts = 128;
    let (mut host, reviewer) = FileOversight::create(root.store(), selected).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let proposal = support::proposal(&oversight::spec(&host, PRIVATE_PAYLOAD));
    let (actor, mut supervisor) = host.into_actor_gateway();
    observe(&mut supervisor);
    let _ticket = actor.submit(701, &proposal).unwrap();
    let keys = ready(&mut supervisor, &reviewer, 701);
    for attempt in 2..=denials + 1 {
        let mut host = supervisor.host_mut().unwrap();
        let mut denied = snapshot();
        denied.values.insert(7, PRIVATE_WITNESS.to_vec());
        let spec = oversight::spec(&host, b"denied");
        let revision = host.revision();
        host.propose(revision, attempt, spec, denied).unwrap();
    }
    (actor, supervisor, keys)
}

fn advance(supervisor: &mut Supervisor, attempt: u64) {
    let mut host = supervisor.host_mut().unwrap();
    let revision = host.revision();
    host.observe_time(revision, ElapsedTick(2)).unwrap();
    let spec = oversight::spec(&host, b"later proposal");
    let revision = host.revision();
    host.propose(revision, attempt, spec, snapshot()).unwrap();
}

fn field<'a>(document: &'a Json, name: &str) -> &'a Json {
    document
        .get(name)
        .unwrap_or_else(|| panic!("missing {name}"))
}

fn parse(response: &PublicationResponse) -> Json {
    let encoded = response.to_json().unwrap();
    strict_json::parse(encoded.as_bytes(), JsonLimits::default()).unwrap()
}

fn wire_value(document: &Json) -> &Json {
    field(field(document, "result"), "value")
}

fn no_global_claims(response: &PublicationResponse) {
    fn unknown<T>(value: &Knowledge<T>) {
        assert!(matches!(
            value,
            Knowledge::Unknown { .. } | Knowledge::Withheld { .. }
        ));
    }
    unknown(&response.envelope.generation);
    unknown(&response.envelope.control_seq);
    unknown(&response.envelope.source_frontiers);
    unknown(&response.envelope.budgets_remaining);
    unknown(&response.envelope.realized_cost);
    unknown(&response.envelope.affordances);
}

fn refused(response: &PublicationResponse, reason: ReasonCode) {
    assert!(matches!(response.result, PublicationResult::Refused(_)));
    assert_eq!(response.envelope.reason_code, Some(reason));
    no_global_claims(response);
}

#[test]
fn a_real_large_corpus_exceeds_legacy_transport_limits_without_changing_the_v1_contract() {
    let root = Directory::new();
    let (actor, supervisor, _keys) = corpus(&root, 80);
    let original = supervisor
        .host()
        .unwrap()
        .replay_candidate_policy(candidate(), limits())
        .unwrap();
    assert_eq!(original.cases().len(), 82);
    assert_eq!(original.newly_reviewable().len(), 80);
    let before = supervisor.host().unwrap().inspect();
    let bytes = image(&root);
    let (port, grant) = supervisor.policy_replay_port(1, limits()).unwrap();
    let request = legacy(71, candidate());
    let mut limited = tower(actor.clone(), 16 * 1024);
    limited.set_policy_replay(Some(port.clone())).unwrap();
    refused(
        &limited.dispatch(&access(), request.clone()),
        ReasonCode::BudgetExhausted,
    );
    assert_eq!(grant.remaining_queries(), 0);

    let mut larger = tower(actor, 512 * 1024);
    larger.set_policy_replay(Some(port)).unwrap();
    let response = larger.dispatch(&access(), request);
    let PublicationResult::PolicyReplay(linked) = &response.result else {
        panic!("legacy whole-corpus response: {response:?}");
    };
    let Ok(ActorKnowledge::Known { value, basis }) = linked.native() else {
        panic!("original complete native report: {response:?}");
    };
    assert_eq!(value.report(), &original);
    assert_eq!(value.basis(), *basis);
    let encoded = response.to_json().unwrap();
    assert!(encoded.len() > 16 * 1024);
    let error = strict_json::parse(encoded.as_bytes(), old_client_limits()).unwrap_err();
    assert_eq!(error.kind, ErrorKind::ItemLimit);
    let document = parse(&response);
    let body = wire_value(&document);
    assert_eq!(
        field(body, "schema").as_str(),
        Some("fa.publication-policy-replay/1")
    );
    let report = field(field(body, "knowledge"), "value");
    assert_eq!(field(report, "case_count").as_str(), Some("82"));
    assert_eq!(field(report, "cases").as_array().unwrap().len(), 82);
    assert!(report.get("continuation").is_none());
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(image(&root), bytes);
}

#[test]
fn all_large_corpus_pages_keep_the_original_cut_and_one_campaign_after_owner_advances() {
    let root = Directory::new();
    let (actor, mut supervisor, keys) = corpus(&root, 80);
    let original = supervisor
        .host()
        .unwrap()
        .replay_candidate_policy(candidate(), limits())
        .unwrap();
    let before = supervisor.host().unwrap().inspect();
    let bytes = image(&root);
    let (port, grant) = supervisor
        .policy_replay_port_with_pages(1, limits(), 10)
        .unwrap();
    assert!(grant.pages_enabled());
    let mut facade = tower(actor, 16 * 1024);
    facade.set_policy_replay(Some(port)).unwrap();

    let request = first(71, candidate());
    let header = facade.exchange(&access(), encode_request(&request).unwrap().as_bytes());
    let first_page = known(&header);
    assert!(first_page.is_header());
    assert_eq!(first_page.offset(), 0);
    assert!(first_page.cases().is_empty());
    assert!(!first_page.enumeration_complete());
    assert_eq!(first_page.request_cursor(), None);
    assert_eq!(first_page.remaining_pages_at_capture(), 9);
    assert_eq!(first_page.original().report(), &original);
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(grant.remaining_pages(), 9);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(image(&root), bytes);
    let basis = first_page.basis();
    let first_json = page_document(&header);
    let report = page_body(&first_json);
    let summary = field(report, "summary").clone();
    assert_eq!(field(&summary, "case_count").as_str(), Some("82"));
    assert_eq!(field(&summary, "requires_shadow").as_bool(), Some(false));
    assert_eq!(
        field(field(&summary, "delta_counts"), "newly_reviewable").as_str(),
        Some("80")
    );
    assert_eq!(
        field(field(report, "page"), "kind").as_str(),
        Some("header")
    );
    assert!(!field(report, "previous_policy").is_null());
    assert!(!field(report, "candidate_policy").is_null());
    assert!(field(report, "cases").as_array().unwrap().is_empty());
    let mut next = first_page.continuation().cloned();

    advance(&mut supervisor, 90);
    let later = supervisor.host().unwrap().inspect();
    assert!(later.revision > basis.journal_revision);
    let bytes_after_advance = image(&root);
    let mut recovered = Vec::new();
    let mut pages = 0;
    while let Some(cursor) = next {
        let request = continued(71, cursor.id());
        let encoded = encode_request(&request).unwrap();
        let response = facade.exchange(&access(), encoded.as_bytes());
        let page = known(&response);
        assert!(!page.is_header());
        assert_eq!(page.basis(), basis);
        assert_eq!(page.original().report(), &original);
        assert_eq!(page.request_cursor(), Some(cursor.id()));
        assert_eq!(page.offset(), recovered.len());
        assert!(!page.cases().is_empty());
        assert!(page.cases().len() <= 16);
        let json = page_document(&response);
        let report = page_body(&json);
        assert_eq!(field(report, "summary"), &summary);
        assert!(field(report, "previous_policy").is_null());
        assert!(field(report, "candidate_policy").is_null());
        let metadata = field(report, "page");
        assert_eq!(field(metadata, "kind").as_str(), Some("cases"));
        assert_eq!(field(metadata, "total").as_str(), Some("82"));
        assert_eq!(
            field(metadata, "request_cursor").as_str(),
            Some(cursor.id().to_string().as_str())
        );
        assert_eq!(
            field(report, "cases").as_array().unwrap().len(),
            page.cases().len()
        );
        assert_eq!(
            field(report, "newly_reviewable").as_array().unwrap().len(),
            page.cases()
                .iter()
                .filter(|case| case.delta() == PolicyDelta::NewlyReviewable)
                .count()
        );
        recovered.extend_from_slice(page.cases());
        assert_eq!(page.enumeration_complete(), recovered.len() == 82);
        assert_eq!(
            field(metadata, "enumeration_complete").as_bool(),
            Some(page.enumeration_complete())
        );
        next = page.continuation().cloned();
        assert_eq!(field(metadata, "next_cursor").is_null(), next.is_none());
        pages += 1;
    }
    assert_eq!(pages, 6);
    assert_eq!(recovered, original.cases());
    assert_eq!(
        recovered
            .iter()
            .filter(|case| matches!(case.id(), ReplayCaseId::Review { .. }))
            .count(),
        1
    );
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(grant.remaining_pages(), 3);
    assert_eq!(supervisor.host().unwrap().inspect(), later);
    assert_eq!(image(&root), bytes_after_advance);

    // The original automatic and human keys remain usable after every page.
    let mut host = supervisor.host_mut().unwrap();
    oversight::dispatch(&mut host, &keys);
    let revision = host.revision();
    host.publish(revision, keys.automatic.attempt()).unwrap();
    let revision = host.revision();
    host.reconcile(revision, keys.automatic.attempt()).unwrap();
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.current_policy().unwrap(), &profile().delivery.policy);
}

#[test]
fn native_page_retries_share_one_allowance_and_retain_every_previously_issued_cut() {
    let root = Directory::new();
    let (_actor, mut supervisor, _keys) = corpus(&root, 18);
    let (port, grant) = supervisor
        .policy_replay_port_with_pages(1, limits(), 3)
        .unwrap();
    let alias = port.clone();
    let header = known_native(port.replay_candidate_policy_page(71, 1, candidate()));
    let first_cursor = header.continuation().unwrap().clone();
    assert_eq!(first_cursor.campaign(), 71);
    assert_eq!(grant.remaining_pages(), 2);
    assert_eq!(
        known_native(alias.replay_candidate_policy_page(71, 1, candidate())),
        header
    );
    assert_eq!(grant.remaining_pages(), 2);
    let first_page = known_native(alias.continue_policy_replay(71, &first_cursor));
    assert_eq!(first_page.cases().len(), 16);
    let last_cursor = first_page.continuation().unwrap().clone();
    let last_page = known_native(port.continue_policy_replay(71, &last_cursor));
    assert_eq!(last_page.cases().len(), 4);
    assert!(last_page.enumeration_complete());
    assert!(last_page.continuation().is_none());
    assert_eq!(grant.remaining_pages(), 0);
    assert_eq!(grant.remaining_queries(), 0);
    let retained = grant.retained_bytes();
    advance(&mut supervisor, 90);
    let bytes = image(&root);
    assert_eq!(
        known_native(alias.replay_candidate_policy_page(71, 1, candidate())),
        header
    );
    assert_eq!(
        known_native(port.continue_policy_replay(71, &first_cursor)),
        first_page
    );
    assert_eq!(
        known_native(alias.continue_policy_replay(71, &last_cursor)),
        last_page
    );
    assert_eq!(grant.remaining_pages(), 0);
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(grant.retained_bytes(), retained);
    assert_eq!(image(&root), bytes);
}

#[test]
fn exhausted_page_allowances_never_label_a_prefix_or_header_complete() {
    let root = Directory::new();
    let (actor, supervisor, _keys) = corpus(&root, 18);
    for pages in [1, 2] {
        let (port, grant) = supervisor
            .policy_replay_port_with_pages(1, limits(), pages)
            .unwrap();
        let mut facade = tower(actor.clone(), 16 * 1024);
        facade.set_policy_replay(Some(port)).unwrap();
        let header = facade.dispatch(&access(), first(71, candidate()));
        let response = if pages == 1 {
            assert!(known(&header).continuation().is_none());
            header
        } else {
            let cursor = known(&header).continuation().unwrap().id();
            facade.dispatch(&access(), continued(71, cursor))
        };
        let page = known(&response);
        assert!(!page.enumeration_complete());
        assert!(page.continuation().is_none());
        assert_eq!(page.remaining_pages_at_capture(), 0);
        assert_eq!(page.cases().len(), if pages == 1 { 0 } else { 16 });
        assert_eq!(page.original().report().cases().len(), 20);
        let document = page_document(&response);
        let metadata = field(page_body(&document), "page");
        assert_eq!(
            field(metadata, "enumeration_complete").as_bool(),
            Some(false)
        );
        assert!(field(metadata, "next_cursor").is_null());
        assert_eq!(field(metadata, "total").as_str(), Some("20"));
        assert_eq!(grant.remaining_queries(), 0);
        assert_eq!(grant.remaining_pages(), 0);
    }
}

#[test]
fn paged_request_codec_accepts_only_exact_header_and_continuation_forms() {
    let header = first(u64::MAX, candidate());
    let continuation = continued(u64::MAX, u64::MAX - 1);
    for request in [&header, &continuation] {
        let wire = encode_request(request).unwrap();
        assert_eq!(decode_request(wire.as_bytes()).unwrap(), *request);
        assert_eq!(
            encode_request(&decode_request(wire.as_bytes()).unwrap()).unwrap(),
            wire
        );
        for field in ["operator", "grant", "trusted"] {
            let forged = wire.replacen(
                "\"arguments\":",
                &format!("\"{field}\":true,\"arguments\":"),
                1,
            );
            assert_ne!(forged, wire);
            assert!(decode_request(forged.as_bytes()).is_err());
        }
        let duplicate = wire.replacen("\"campaign\":", "\"campaign\":\"1\",\"campaign\":", 1);
        assert!(decode_request(duplicate.as_bytes()).is_err());
        let alias = wire.replace(
            "\"campaign\":\"18446744073709551615\"",
            "\"campaign\":\"018446744073709551615\"",
        );
        assert_ne!(alias, wire);
        assert!(decode_request(alias.as_bytes()).is_err());
        let mut wrong_key = request.clone();
        wrong_key.idempotency_key = Some("1".into());
        assert!(encode_request(&wrong_key).is_err());
        let mut implicit_mount = request.clone();
        implicit_mount.address.generation = None;
        assert!(encode_request(&implicit_mount).is_err());
    }
    let header_wire = encode_request(&header).unwrap();
    assert!(header_wire.contains("\"paged\":true"));
    for replacement in ["false", "null", "\"true\"", "1"] {
        let invalid = header_wire.replace("\"paged\":true", &format!("\"paged\":{replacement}"));
        assert!(decode_request(invalid.as_bytes()).is_err());
    }
    let mixed = header_wire.replace("\"paged\":true", "\"paged\":true,\"cursor\":\"7\"");
    assert!(decode_request(mixed.as_bytes()).is_err());
    let cursor_wire = encode_request(&continuation).unwrap();
    assert!(!cursor_wire.contains("candidate"));
    assert!(!cursor_wire.contains("expected_policy_generation"));
    for extra in [
        "\"paged\":true",
        "\"expected_policy_generation\":\"1\"",
        "\"candidate\":null",
        "\"offset\":\"16\"",
    ] {
        let mixed = cursor_wire.replacen("\"cursor\":", &format!("{extra},\"cursor\":"), 1);
        assert!(decode_request(mixed.as_bytes()).is_err());
    }
    for cursor in ["\"0\"", "\"01\"", "1", "null", "\"-1\""] {
        let invalid = cursor_wire.replace(
            "\"cursor\":\"18446744073709551614\"",
            &format!("\"cursor\":{cursor}"),
        );
        assert_ne!(invalid, cursor_wire);
        assert!(decode_request(invalid.as_bytes()).is_err());
    }
    let ordinary = legacy(71, candidate());
    let encoded = encode_request(&ordinary).unwrap();
    assert!(!encoded.contains("paged"));
    assert_eq!(decode_request(encoded.as_bytes()).unwrap(), ordinary);
}

#[test]
fn original_cursor_brands_and_whole_campaign_identity_are_checked_before_spending() {
    let root = Directory::new();
    let (_actor, supervisor, _keys) = corpus(&root, 1);
    let bytes = image(&root);
    let (first_port, first_grant) = supervisor
        .policy_replay_port_with_pages(2, limits(), 4)
        .unwrap();
    let (other_port, other_grant) = supervisor
        .policy_replay_port_with_pages(1, limits(), 3)
        .unwrap();
    let header = known_native(first_port.replay_candidate_policy_page(71, 1, candidate()));
    let other_header = known_native(other_port.replay_candidate_policy_page(71, 1, candidate()));
    let cursor = header.continuation().unwrap();
    let other_cursor = other_header.continuation().unwrap();
    assert_ne!(cursor.id(), other_cursor.id());
    assert_eq!(
        first_port.continue_policy_replay(72, cursor),
        Err(ActorError::IdempotencyConflict)
    );
    assert_eq!(
        first_port.continue_policy_replay(71, other_cursor),
        Err(ActorError::IdempotencyConflict)
    );
    assert_eq!(
        other_port.continue_policy_replay(71, cursor),
        Err(ActorError::IdempotencyConflict)
    );
    let foreign_root = Directory::new();
    let (_foreign_actor, foreign_owner, _reviewer, _proposal) = support::create(&foreign_root);
    let (foreign_port, foreign_grant) = foreign_owner
        .policy_replay_port_with_pages(1, limits(), 3)
        .unwrap();
    assert_eq!(
        foreign_port.continue_policy_replay(71, cursor),
        Err(ActorError::IdempotencyConflict)
    );
    assert_eq!(foreign_grant.remaining_queries(), 1);
    assert_eq!(foreign_grant.remaining_pages(), 3);
    let changed = Policy::new(3, vec![Predicate::PayloadAtMost(128)]).unwrap();
    assert_eq!(
        first_port.replay_candidate_policy_page(71, 1, changed.clone()),
        Err(ActorError::IdempotencyConflict)
    );
    assert_eq!(
        first_port.replay_candidate_policy_page(71, 2, changed.clone()),
        Err(ActorError::IdempotencyConflict)
    );
    assert_eq!(first_grant.remaining_queries(), 1);
    assert_eq!(first_grant.remaining_pages(), 3);
    assert_eq!(other_grant.remaining_queries(), 0);
    assert_eq!(other_grant.remaining_pages(), 2);

    let old = first_port
        .replay_candidate_policy(72, 1, candidate())
        .unwrap();
    assert!(matches!(old, ActorKnowledge::Known { .. }));
    assert_eq!(
        first_port.replay_candidate_policy_page(72, 1, changed),
        Err(ActorError::IdempotencyConflict)
    );
    assert_eq!(first_grant.remaining_queries(), 0);
    assert_eq!(first_grant.remaining_pages(), 3);
    let second_campaign = known_native(first_port.replay_candidate_policy_page(72, 1, candidate()));
    assert_eq!(second_campaign.basis().query, 72);
    assert_eq!(first_grant.remaining_queries(), 0);
    assert_eq!(first_grant.remaining_pages(), 2);
    assert_eq!(
        known_native(first_port.replay_candidate_policy_page(71, 1, candidate())),
        header
    );
    assert_eq!(first_grant.remaining_pages(), 2);
    assert_eq!(image(&root), bytes);
}

#[test]
fn an_old_grant_never_acquires_page_custody_through_a_new_wire_flag() {
    let root = Directory::new();
    let (actor, supervisor, _keys) = corpus(&root, 1);
    let (port, grant) = supervisor.policy_replay_port(1, limits()).unwrap();
    assert!(!grant.pages_enabled());
    assert_eq!(grant.remaining_pages(), 0);
    assert_eq!(
        port.replay_candidate_policy_page(71, 1, candidate()),
        Err(ActorError::Withheld)
    );
    assert_eq!(grant.remaining_queries(), 1);
    let mut facade = tower(actor, 16 * 1024);
    facade.set_policy_replay(Some(port)).unwrap();
    let denied = facade.dispatch(&access(), first(71, candidate()));
    assert_eq!(native(&denied), &Err(ActorError::Withheld));
    assert_eq!(
        denied.envelope.reason_code,
        Some(ReasonCode::DeniedByPolicy)
    );
    assert_eq!(grant.remaining_queries(), 1);
    let old = facade.dispatch(&access(), legacy(71, candidate()));
    assert!(matches!(old.result, PublicationResult::PolicyReplay(_)));
    assert_eq!(old.envelope.reason_code, None);
    assert_eq!(grant.remaining_queries(), 0);
}

#[test]
fn scalar_cursors_cannot_reconstruct_native_custody_in_another_facade() {
    let root = Directory::new();
    let (actor, supervisor, _keys) = corpus(&root, 18);
    let (port, grant) = supervisor
        .policy_replay_port_with_pages(1, limits(), 3)
        .unwrap();
    let mut first_facade = tower(actor.clone(), 16 * 1024);
    first_facade.set_policy_replay(Some(port.clone())).unwrap();
    let header = first_facade.dispatch(&access(), first(71, candidate()));
    let cursor = known(&header).continuation().unwrap().id();
    assert_eq!(grant.remaining_pages(), 2);
    let mut other = tower(actor, 16 * 1024);
    other.set_policy_replay(Some(port)).unwrap();
    let unissued = other.dispatch(&access(), continued(71, cursor));
    assert_eq!(native(&unissued), &Err(ActorError::IdempotencyConflict));
    assert_eq!(
        unissued.envelope.reason_code,
        Some(ReasonCode::IntegrityFailure)
    );
    let forged = first_facade.dispatch(&access(), continued(71, u64::MAX));
    assert_eq!(native(&forged), &Err(ActorError::IdempotencyConflict));
    assert_eq!(
        forged.envelope.reason_code,
        Some(ReasonCode::IntegrityFailure)
    );
    let wrong_campaign = first_facade.dispatch(&access(), continued(72, cursor));
    assert_eq!(
        wrong_campaign.envelope.reason_code,
        Some(ReasonCode::IntegrityFailure)
    );
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(grant.remaining_pages(), 2);

    let repeated = other.dispatch(&access(), first(71, candidate()));
    assert_eq!(native(&repeated), native(&header));
    assert_eq!(grant.remaining_pages(), 2);
    let page = other.dispatch(&access(), continued(71, cursor));
    assert_eq!(known(&page).cases().len(), 16);
    assert_eq!(grant.remaining_pages(), 1);
    let original_alias = first_facade.dispatch(&access(), continued(71, cursor));
    assert_eq!(native(&original_alias), native(&page));
    assert_eq!(grant.remaining_pages(), 1);
}

#[test]
fn saved_headers_and_pages_reauthenticate_busy_dropped_and_revoked_original_owners() {
    let root = Directory::new();
    let (actor, mut supervisor, _keys) = corpus(&root, 18);
    let (port, grant) = supervisor
        .policy_replay_port_with_pages(2, limits(), 6)
        .unwrap();
    let mut facade = tower(actor, 16 * 1024);
    facade.set_policy_replay(Some(port)).unwrap();
    let header_query = first(71, candidate());
    let header = facade.dispatch(&access(), header_query.clone());
    let cursor = known(&header).continuation().unwrap().id();
    let page_query = continued(71, cursor);
    let page = facade.dispatch(&access(), page_query.clone());
    let last = continued(71, known(&page).continuation().unwrap().id());
    let retained = grant.retained_bytes();
    {
        let _busy = supervisor.host_mut().unwrap();
        for request in [header_query.clone(), page_query.clone(), last.clone()] {
            let denied = facade.dispatch(&access(), request);
            assert_eq!(
                native(&denied),
                &Ok(ActorKnowledge::Unknown {
                    reason: ActorUnknown::ControllerUnavailable,
                })
            );
            assert_eq!(
                denied.envelope.reason_code,
                Some(ReasonCode::LowerLayerUnverified)
            );
        }
    }
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(grant.remaining_pages(), 4);
    assert_eq!(grant.retained_bytes(), retained);
    assert_eq!(
        native(&facade.dispatch(&access(), page_query.clone())),
        native(&page)
    );
    assert_eq!(grant.remaining_pages(), 4);
    drop(supervisor);
    for request in [header_query.clone(), page_query.clone(), last.clone()] {
        assert_eq!(
            native(&facade.dispatch(&access(), request)),
            &Ok(ActorKnowledge::Unknown {
                reason: ActorUnknown::ControllerUnavailable,
            })
        );
    }
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(grant.remaining_pages(), 4);
    grant.revoke();
    for request in [header_query, page_query, last] {
        let denied = facade.dispatch(&access(), request);
        assert_eq!(
            native(&denied),
            &Ok(ActorKnowledge::Withheld {
                authority_required: "supervisor_policy_replay",
            })
        );
        assert_eq!(
            denied.envelope.reason_code,
            Some(ReasonCode::DeniedByPolicy)
        );
    }
}

#[test]
fn oversized_header_refusal_keeps_its_original_cursor_for_a_larger_transport_retry() {
    let root = Directory::new();
    let (actor, supervisor, _keys) = corpus(&root, 1);
    let (port, grant) = supervisor
        .policy_replay_port_with_pages(1, limits(), 2)
        .unwrap();
    let policy = Policy::new(2, vec![Predicate::PayloadIs(vec![0xaa; 20_000])]).unwrap();
    let query = first(71, policy);
    let mut bounded = tower(actor.clone(), 16 * 1024);
    bounded.set_policy_replay(Some(port.clone())).unwrap();
    let bytes = image(&root);
    let response = bounded.dispatch(&access(), query.clone());
    refused(&response, ReasonCode::BudgetExhausted);
    assert!(!response.to_json().unwrap().contains("aaaa"));
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(grant.remaining_pages(), 1);
    let retained = grant.retained_bytes();
    let mut larger = tower(actor, 128 * 1024);
    larger.set_policy_replay(Some(port)).unwrap();
    let header = larger.dispatch(&access(), query);
    let page = known(&header);
    assert!(page.is_header());
    assert!(page.cases().is_empty());
    assert_eq!(page.remaining_pages_at_capture(), 1);
    let encoded = header.to_json().unwrap();
    assert!(encoded.len() > 16 * 1024);
    assert!(encoded.contains("aaaa"));
    assert_eq!(grant.remaining_pages(), 1);
    assert_eq!(grant.retained_bytes(), retained);
    let cursor = page.continuation().unwrap().id();
    let response = larger.dispatch(&access(), continued(71, cursor));
    assert_eq!(known(&response).cases().len(), 3);
    assert!(known(&response).enumeration_complete());
    assert_eq!(grant.remaining_pages(), 0);
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(image(&root), bytes);
}

#[test]
fn an_oversized_whole_case_page_can_be_retried_without_a_second_native_page_charge() {
    let root = Directory::new();
    let (actor, supervisor, _keys) = corpus(&root, 18);
    let (port, grant) = supervisor
        .policy_replay_port_with_pages(1, limits(), 3)
        .unwrap();
    let mut nodes = vec![Predicate::PayloadAtMost(128); 127];
    nodes.push(Predicate::All((0..127).collect()));
    let policy = Policy::new(2, nodes).unwrap();
    let query = first(71, policy);
    let mut bounded = tower(actor.clone(), 16 * 1024);
    bounded.set_policy_replay(Some(port.clone())).unwrap();
    let header = bounded.dispatch(&access(), query.clone());
    let cursor = known(&header).continuation().unwrap().id();
    assert!(header.to_json().unwrap().len() <= 16 * 1024);
    let response = bounded.dispatch(&access(), continued(71, cursor));
    refused(&response, ReasonCode::BudgetExhausted);
    assert!(!response.to_json().unwrap().contains("\"cases\""));
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(grant.remaining_pages(), 1);
    let retained = grant.retained_bytes();
    let bytes = image(&root);
    let mut larger = tower(actor, 512 * 1024);
    larger.set_policy_replay(Some(port)).unwrap();
    let recovered_header = larger.dispatch(&access(), query);
    assert_eq!(native(&recovered_header), native(&header));
    let recovered = larger.dispatch(&access(), continued(71, cursor));
    let page = known(&recovered);
    assert_eq!(page.offset(), 0);
    assert_eq!(page.cases().len(), 16);
    assert!(!page.enumeration_complete());
    assert_eq!(page.remaining_pages_at_capture(), 1);
    assert!(recovered.to_json().unwrap().len() > 16 * 1024);
    assert_eq!(grant.remaining_pages(), 1);
    assert_eq!(grant.retained_bytes(), retained);
    let last = larger.dispatch(&access(), continued(71, page.continuation().unwrap().id()));
    assert_eq!(known(&last).offset(), 16);
    assert_eq!(known(&last).cases().len(), 4);
    assert!(known(&last).enumeration_complete());
    assert_eq!(grant.remaining_pages(), 0);
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(image(&root), bytes);
}

#[test]
fn page_encoding_cannot_rebind_a_saved_header_or_page_to_another_mount_or_campaign() {
    let root = Directory::new();
    let (actor, supervisor, _keys) = corpus(&root, 1);
    let (port, _grant) = supervisor
        .policy_replay_port_with_pages(1, limits(), 2)
        .unwrap();
    let mut facade = tower(actor, 16 * 1024);
    facade.set_policy_replay(Some(port)).unwrap();
    let header = facade.dispatch(&access(), first(71, candidate()));
    let cursor = known(&header).continuation().unwrap().id();
    let page = facade.dispatch(&access(), continued(71, cursor));
    let exhausted = facade.dispatch(&access(), first(72, candidate()));
    assert_eq!(native(&exhausted), &Err(ActorError::Capacity));
    for response in [header, page, exhausted] {
        let PublicationResult::PolicyReplayPage(linked) = &response.result else {
            panic!("linked native page");
        };
        assert_eq!(linked.registration(), &mount());
        assert_eq!(
            response.envelope.idempotency_key.as_deref(),
            Some(linked.campaign().to_string().as_str())
        );
        response.to_json().unwrap();
        for generation in [None, Some(0), Some(40), Some(42), Some(u64::MAX)] {
            let mut changed = response.clone();
            changed.envelope.address.generation = generation;
            assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
        }
        let mut subject = response.clone();
        subject.envelope.address = facade.request_address(701).unwrap();
        assert_eq!(subject.to_json(), Err(TowerEncodingError::InvalidKnowledge));
        let mut tenant = response.clone();
        tenant.envelope.address.tenant = "another-owner".into();
        assert_eq!(tenant.to_json(), Err(TowerEncodingError::InvalidKnowledge));
        for key in [None, Some("999".into()), Some("071".into())] {
            let mut changed = response.clone();
            changed.envelope.idempotency_key = key;
            assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
        }
    }
}

#[test]
fn paged_missing_observations_remain_requires_shadow_in_summary_and_every_case() {
    let root = Directory::new();
    let (actor, supervisor, _keys) = corpus(&root, 18);
    let (port, grant) = supervisor
        .policy_replay_port_with_pages(1, limits(), 3)
        .unwrap();
    let policy = Policy::new(
        2,
        vec![
            Predicate::PayloadAtMost(128),
            Predicate::Absent { key: 99 },
            Predicate::Any(vec![0, 1]),
        ],
    )
    .unwrap();
    let before = image(&root);
    let mut facade = tower(actor, 16 * 1024);
    facade.set_policy_replay(Some(port)).unwrap();
    let header = facade.dispatch(&access(), first(71, policy));
    let header_json = page_document(&header);
    let summary = field(page_body(&header_json), "summary").clone();
    assert_eq!(field(&summary, "requires_shadow").as_bool(), Some(true));
    assert_eq!(
        field(field(&summary, "delta_counts"), "requires_shadow").as_str(),
        Some("20")
    );
    let mut next = known(&header).continuation().cloned();
    let mut cases = 0;
    while let Some(cursor) = next {
        let response = facade.dispatch(&access(), continued(71, cursor.id()));
        let page = known(&response);
        for case in page.cases() {
            assert_eq!(case.delta(), PolicyDelta::RequiresShadow);
            assert_eq!(case.missing_nodes(), &[1]);
            assert!(case.candidate().is_none());
        }
        let json = page_document(&response);
        let body = page_body(&json);
        assert_eq!(field(body, "summary"), &summary);
        assert!(
            field(body, "newly_reviewable")
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(field(body, "newly_blocked").as_array().unwrap().is_empty());
        for case in field(body, "cases").as_array().unwrap() {
            let comparison = field(case, "candidate");
            assert_eq!(field(comparison, "state").as_str(), Some("unknown"));
            assert_eq!(
                field(comparison, "reason").as_str(),
                Some("missing_retained_observations")
            );
        }
        cases += page.cases().len();
        next = page.continuation().cloned();
        if next.is_none() {
            assert!(page.enumeration_complete());
        }
    }
    assert_eq!(cases, 20);
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(grant.remaining_pages(), 0);
    assert_eq!(image(&root), before);
}
