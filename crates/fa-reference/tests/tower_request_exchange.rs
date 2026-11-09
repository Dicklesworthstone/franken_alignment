//! Hand-authored request documents enter the real snapshot and durable owner.
//! Positive publication controls retain original source capture, commit/reveal,
//! both approval keys and reconciliation; syntax refusal never replaces them.
#![cfg(unix)]

#[path = "tower_request_exchange/fixture.rs"]
mod fixture;
#[path = "support/file_oversight_actor.rs"]
mod support;

use std::collections::BTreeSet;
use std::fmt::Write as _;

use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FilePrecheckOutcome, precheck_wire::PrecheckWireError,
};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::oversight::{
    actor::{
        ActorOutcome, ActorProposal, BasisSource, Knowledge as ActorKnowledge,
        UnknownReason as ActorUnknown,
    },
    actor_wire::WireError,
};
use fa_reference::address_resolver::Address;
use fa_reference::observation_algebra::Knowledge;
use fa_reference::tower::router::{RoutedResponse, TowerRouter};
use fa_reference::tower::{
    ENVELOPE_FIELDS, MAX_REQUEST_BYTES, ReadArguments, ReasonCode, TowerRequest, TowerValue, Verb,
};
use fixture::*;
use support::{Directory, observe, oversight};

const DISCLOSURE: u64 = 81;

// Build independent wire examples instead of using the encoder under test.
// Every interpolated identity comes from a trusted fixture; arguments/key are
// intentionally raw JSON so negative cases can preserve invalid input exactly.
fn document(address: &Address, verb: &str, arguments: &str, key: &str) -> String {
    format!(
        r#"{{"schema_version":"fa.tower_request/1","operation_id":"wire-regression","address":"{}","verb":"{verb}","arguments":{arguments},"idempotency_key":{key}}}"#,
        address.canonical_string()
    )
}

fn get_document(address: &Address) -> String {
    document(address, "get", "null", "null")
}

fn proposal_json(proposal: &ActorProposal) -> String {
    let target = proposal.target;
    let mut payload = String::new();
    for byte in &proposal.payload {
        write!(payload, "{byte:02x}").unwrap();
    }
    format!(
        r#"{{"target":{{"adapter":"{}","object":"{}","contract_version":"{}","expected_version":"{}","generation":"{}"}},"payload_hex":"{payload}","units":"{}","deadline":"{}","expected_policy_epoch":"{}"}}"#,
        target.adapter,
        target.object,
        target.contract_version,
        target.expected_version,
        target.generation,
        proposal.units,
        proposal.deadline.0,
        proposal.expected_policy_epoch,
    )
}

fn submit_document(address: &Address, request: u64, proposal: &ActorProposal) -> String {
    document(
        address,
        "propose",
        &format!(
            r#"{{"request":"{request}","proposal":{}}}"#,
            proposal_json(proposal)
        ),
        &format!(r#""{request}""#),
    )
}

fn precheck_document(address: &Address, query: u64, proposal: &ActorProposal) -> String {
    document(
        address,
        "precheck",
        &format!(
            r#"{{"query":"{query}","proposal":{}}}"#,
            proposal_json(proposal)
        ),
        "null",
    )
}

fn extra_field(document: &str, field: &str) -> String {
    format!("{},{}{}", document.strip_suffix('}').unwrap(), field, '}')
}

#[test]
fn strict_framing_and_claimed_authority_are_redacted_before_any_reference_charge() {
    let mut tower = imported(TENANT, &[], 1);
    let good = get_document(&evidence(TENANT));
    let malformed = [
        extra_field(&good, r#""operator":true"#),
        extra_field(&good, r#""scope":{"caller_tenant":"1"}"#),
        extra_field(&good, r#""allowed_purposes":["Permit"]"#),
        extra_field(&good, r#""operation_id":"PRIVATE-AUTHORITY-CLAIM""#),
        extra_field(&good, r#""\u006fperation_id":"PRIVATE-AUTHORITY-CLAIM""#),
        good.replace(r#""arguments":null"#, r#""arguments":{"operator":true}"#),
        good.replace("retained-source@7", "retained-source@07"),
        good.replace(r#""idempotency_key":null"#, r#""idempotency_key":"""#),
        format!("{good}{{}}"),
        format!("[{good}]"),
        document(
            tower.domain_address(),
            "tail",
            r#"{"since_seq":0,"max_events":"64"}"#,
            "null",
        ),
        document(
            tower.domain_address(),
            "tail",
            r#"{"since_seq":"00","max_events":"64"}"#,
            "null",
        ),
        document(
            tower.domain_address(),
            "tail",
            r#"{"since_seq":"18446744073709551616","max_events":"64"}"#,
            "null",
        ),
    ];
    for bytes in malformed
        .iter()
        .map(|value| value.as_bytes())
        .chain(std::iter::once(&b"\xffPRIVATE-AUTHORITY-CLAIM"[..]))
    {
        let response = tower.exchange(&operator(TENANT), bytes);
        assert_eq!(
            response.envelope.reason_code,
            Some(ReasonCode::IntegrityFailure)
        );
        assert_eq!(response.envelope.operation_id, "<invalid-request>");
        assert_eq!(response.envelope.idempotency_key, None);
        assert!(!response.envelope.retry_safe);
        let encoded = response.to_json().unwrap();
        assert!(!encoded.contains("PRIVATE-AUTHORITY-CLAIM"));
        let decoded = parse(encoded.as_bytes());
        assert_eq!(
            field(&decoded, "envelope")
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            ENVELOPE_FIELDS.into_iter().collect::<BTreeSet<_>>()
        );
        assert_eq!(tower.remaining_read_units(), 1);
        assert_eq!(tower.journal_control_seq(), 0);
    }
    let over_capacity = vec![b' '; MAX_REQUEST_BYTES + 1];
    let refused = tower.exchange(&operator(TENANT), &over_capacity);
    assert_eq!(
        refused.envelope.reason_code,
        Some(ReasonCode::BudgetExhausted)
    );
    assert_eq!(refused.envelope.operation_id, "<invalid-request>");
    assert_eq!(tower.remaining_read_units(), 1);
    for unsupported in [
        good.replace("fa.tower_request/1", "fa.tower_request/999"),
        good.replace(r#""verb":"get""#, r#""verb":"invented-command""#),
        document(&evidence(TENANT), "verify", "null", "null"),
    ] {
        assert_eq!(
            tower
                .exchange(&operator(TENANT), unsupported.as_bytes())
                .envelope
                .reason_code,
            Some(ReasonCode::CapabilityUnsupported)
        );
        assert_eq!(tower.remaining_read_units(), 1);
    }
    let successful = tower.exchange(&reader(), format!(" \n{good}\t").as_bytes());
    assert!(matches!(
        successful.result,
        Knowledge::Known {
            value: TowerValue::Object(_),
            ..
        }
    ));
    assert_eq!(tower.remaining_read_units(), 0);
    assert_eq!(
        tower
            .exchange(&reader(), good.as_bytes())
            .envelope
            .reason_code,
        Some(ReasonCode::BudgetExhausted)
    );
}

#[test]
fn full_width_tail_arguments_match_original_dispatch_without_losing_decimal_precision() {
    let mut wire = imported(TENANT, &[], 1);
    let mut typed = imported(TENANT, &[], 1);
    let address = wire.domain_address().clone();
    let bytes = document(
        &address,
        "tail",
        r#"{"since_seq":"18446744073709551615","max_events":"64"}"#,
        "null",
    );
    let mut request = TowerRequest::new("wire-regression", address, Verb::Tail);
    request.arguments = ReadArguments::Tail {
        since_seq: u64::MAX,
        max_events: 64,
    };
    let response = wire.exchange(&operator(TENANT), bytes.as_bytes());
    assert_eq!(response, typed.dispatch(&operator(TENANT), request));
    assert_eq!(wire.remaining_read_units(), 0);
    assert_eq!(wire.journal_control_seq(), 0);
}

#[test]
fn source_precheck_and_real_publication_keep_native_charges_and_exact_retry_observations() {
    let root = Directory::new();
    let (port, mut supervisor, reviewer, proposal, mut source) = source_owner(&root);
    let original_port = port.clone();
    let mut adapter = publication(port);
    let mount = adapter.mount_address().clone();
    let address = adapter.request_address(70).unwrap();
    let (precheck_port, grant) = supervisor
        .disclose_exact_precheck(supervisor.host().unwrap().revision(), DISCLOSURE, 3)
        .unwrap();
    adapter.set_precheck(Some(precheck_port)).unwrap();
    let mut router = TowerRouter::with_publication(imported(TENANT, &[], 1), adapter).unwrap();
    let before = canonical(&root);
    let source_reads = source.status().read_attempts;
    let precheck = precheck_document(&mount, u64::MAX, &proposal);
    let malformed = extra_field(&precheck, r#""disclosure":{"query_budget":"999999"}"#);
    let refused = router.exchange(&reader(), malformed.as_bytes());
    assert!(matches!(refused, RoutedResponse::Reference(_)));
    assert_eq!(refused.reason_code(), Some(ReasonCode::IntegrityFailure));
    assert_eq!(grant.remaining_queries(), 3);

    let checked = router.exchange(&reader(), precheck.as_bytes());
    let Ok(ActorKnowledge::Known { value, basis }) = &precheck_result(&checked).result else {
        panic!("original source-backed precheck must run through the wire request");
    };
    assert_eq!(*value, FilePrecheckOutcome::PrecheckPassed);
    assert_eq!(
        (basis.disclosure, basis.query, basis.policy_epoch),
        (DISCLOSURE, u64::MAX, 0)
    );
    assert_eq!(grant.remaining_queries(), 2);
    let repeated = router.exchange(&reader(), precheck.as_bytes());
    assert_eq!(
        precheck_result(&repeated).result,
        precheck_result(&checked).result
    );
    assert_eq!(
        grant.remaining_queries(),
        1,
        "an exact precheck retry still spends a query"
    );
    let mut denied_proposal = proposal.clone();
    denied_proposal.target.object += 1;
    let denied = router.exchange(
        &reader(),
        precheck_document(&mount, 2, &denied_proposal).as_bytes(),
    );
    assert!(matches!(
        precheck_result(&denied).result,
        Ok(ActorKnowledge::Known {
            value: FilePrecheckOutcome::DeniedByPolicy,
            ..
        })
    ));
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(
        precheck_result(&router.exchange(&reader(), precheck.as_bytes())).result,
        Err(PrecheckWireError::Capacity)
    );
    assert_eq!(source.status().read_attempts, source_reads);
    assert_eq!(canonical(&root), before);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 0);

    // An exact policy pass cannot manufacture an admission snapshot or ticket.
    let call = submit_document(&mount, 70, &proposal);
    assert_eq!(
        request_result(&router.exchange(&reader(), get_document(&address).as_bytes())).result,
        Ok(ActorKnowledge::Withheld {
            authority_required: "own_request"
        })
    );
    assert_eq!(
        request_result(&router.exchange(&access(), call.as_bytes())).result,
        Err(WireError::Unavailable)
    );
    assert_eq!(canonical(&root), before);
    observe(&mut supervisor);
    assert_eq!(
        request_result(&router.exchange(&access(), call.as_bytes())).result,
        Ok(ActorKnowledge::Pending { request: 70 })
    );
    let admitted_image = canonical(&root);
    let retry = router.exchange(&access(), call.as_bytes());
    assert_eq!(
        request_result(&retry).result,
        Ok(ActorKnowledge::Pending { request: 70 })
    );
    assert_eq!(canonical(&root), admitted_image);
    let mut changed = proposal.clone();
    changed.payload.push(b'!');
    assert_eq!(
        request_result(
            &router.exchange(&access(), submit_document(&mount, 70, &changed).as_bytes())
        )
        .result,
        Err(WireError::IdempotencyConflict)
    );
    assert_eq!(canonical(&root), admitted_image);

    let ticket = original_port.submit(70, &proposal).unwrap();
    let keys = ready_from_source(&mut supervisor, &reviewer, 70);
    let attempt = keys.automatic.attempt();
    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &keys);
    }
    let uncertain = router.exchange(&reader(), get_document(&address).as_bytes());
    assert_eq!(
        request_result(&uncertain).result,
        Ok(ActorKnowledge::Unknown {
            reason: ActorUnknown::OutcomeUnknown
        })
    );
    assert_eq!(uncertain.reason_code(), Some(ReasonCode::OutcomeUnknown));
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        let before = canonical(&root);
        assert_eq!(
            host.publish(revision, attempt),
            Err(JournalError::Contract(fa_reference::Error::Incomplete))
        );
        assert_eq!(host.revision(), revision);
        assert_eq!(canonical(&root), before);
        assert_eq!(host.inspect().executions, 0);

        // The original file source installs a mandatory publication guard.
        // Capture again after dispatch; a Tower observation cannot replace it.
        let captured = host
            .refresh_file_source(revision, &mut source, ElapsedTick(2))
            .unwrap();
        let current = captured
            .inputs_for(&keys.action, &support::profile().committee)
            .unwrap();
        let revision = host.revision();
        let published = host
            .publish_checked(
                revision,
                attempt,
                Some(&current),
                captured.snapshot().clone(),
                ElapsedTick(3),
            )
            .unwrap();
        assert_eq!(published.basis, PublicationBasis::Revalidated);
        assert_eq!(host.inspect().payload, proposal.payload);
        assert_eq!(host.inspect().executions, 1);
    }
    assert_eq!(source.status().read_attempts, source_reads + 1);
    assert_eq!(
        request_result(&router.exchange(&access(), call.as_bytes())).result,
        request_result(&uncertain).result
    );
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        assert!(matches!(
            host.reconcile(revision, attempt).unwrap(),
            Reconciliation::Resolved(_)
        ));
    }
    let completed = router.exchange(&reader(), get_document(&address).as_bytes());
    assert_eq!(
        request_result(&completed).result,
        Ok(original_port.poll(&ticket))
    );
    let Ok(ActorKnowledge::Known { value, basis }) = &request_result(&completed).result else {
        panic!("publication outcome must come from original durable reconciliation");
    };
    assert_eq!(*value, ActorOutcome::Executed);
    assert_eq!(
        (basis.request, basis.source),
        (70, BasisSource::ControlLedger)
    );
    assert_native_json(&completed);
    let completed_image = canonical(&root);
    assert_eq!(
        request_result(&router.exchange(&access(), call.as_bytes())).result,
        request_result(&completed).result
    );
    assert_eq!(canonical(&root), completed_image);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 1);
    let last_read = router.exchange(
        &operator(TENANT),
        get_document(&evidence(TENANT)).as_bytes(),
    );
    known_evidence(&last_read, TENANT);
    assert!(matches!(
        reference(&last_read).envelope.budgets_remaining,
        Knowledge::Known { value: 0, .. }
    ));
}

#[test]
fn malformed_native_documents_preserve_the_admission_snapshot_and_full_width_request_identity() {
    let root = Directory::new();
    let (port, mut supervisor, _reviewer, proposal) = support::create(&root);
    let adapter = publication(port);
    let mount = adapter.mount_address().clone();
    let address = adapter.request_address(u64::MAX).unwrap();
    let mut router = TowerRouter::with_publication(imported(TENANT, &[], 1), adapter).unwrap();
    observe(&mut supervisor);
    let before = canonical(&root);
    let call = submit_document(&mount, u64::MAX, &proposal);
    let malformed = [
        extra_field(
            &call,
            r#""access":{"operator":true,"allowed_purposes":["Permit"]}"#,
        ),
        call.replace(
            r#""request":"18446744073709551615""#,
            r#""request":18446744073709551615"#,
        ),
        call.replace(
            r#""request":"18446744073709551615""#,
            r#""request":"18446744073709551616""#,
        ),
        call.replace(
            r#""idempotency_key":"18446744073709551615""#,
            r#""idempotency_key":"18446744073709551614""#,
        ),
        call.replace(r#""generation":"1""#, r#""generation":"0""#),
        call.replace(r#""units":"16""#, r#""units":"016""#),
        submit_document(&mount.without_generation(), u64::MAX, &proposal),
    ];
    for invalid in malformed {
        assert_ne!(
            invalid, call,
            "each negative control must actually alter the request"
        );
        let response = router.exchange(&access(), invalid.as_bytes());
        assert!(matches!(response, RoutedResponse::Reference(_)));
        assert_eq!(response.reason_code(), Some(ReasonCode::IntegrityFailure));
        assert_eq!(canonical(&root), before);
        assert_eq!(supervisor.host().unwrap().retained_requests(), 0);
    }
    let no_purpose = router.exchange(&reader(), call.as_bytes());
    assert_eq!(no_purpose.reason_code(), Some(ReasonCode::PurposeMismatch));
    let foreign = router.exchange(&operator("other-tenant"), call.as_bytes());
    assert_eq!(foreign.reason_code(), Some(ReasonCode::DeniedByPolicy));
    assert_eq!(canonical(&root), before);
    let submitted = router.exchange(&access(), call.as_bytes());
    assert_eq!(request_result(&submitted).request, Some(u64::MAX));
    assert_eq!(
        request_result(&submitted).result,
        Ok(ActorKnowledge::Pending { request: u64::MAX })
    );
    assert_eq!(supervisor.host().unwrap().retained_requests(), 1);
    assert_eq!(
        request_result(&router.exchange(&reader(), get_document(&address).as_bytes())).result,
        request_result(&submitted).result
    );
    assert_native_json(&submitted);
    let admitted_image = canonical(&root);
    assert_eq!(
        request_result(&router.exchange(&access(), call.as_bytes())).result,
        request_result(&submitted).result
    );
    assert_eq!(canonical(&root), admitted_image);
    known_evidence(
        &router.exchange(
            &operator(TENANT),
            get_document(&evidence(TENANT)).as_bytes(),
        ),
        TENANT,
    );
}

#[test]
fn disclosure_withdrawal_and_owner_loss_never_turn_native_requests_into_snapshot_reads() {
    let root = Directory::new();
    let (port, mut supervisor, _reviewer, proposal, _source) = source_owner(&root);
    let mut adapter = publication(port);
    let mount = adapter.mount_address().clone();
    let address = adapter.request_address(73).unwrap();
    let (precheck_port, grant) = supervisor
        .disclose_exact_precheck(supervisor.host().unwrap().revision(), DISCLOSURE, 2)
        .unwrap();
    adapter.set_precheck(Some(precheck_port)).unwrap();
    let decoys = [mount.clone(), address.clone()];
    // Positive control: both conflicting imports really exist in the snapshot.
    let mut baseline = imported(TENANT, &decoys, 2);
    for decoy in &decoys {
        assert!(matches!(
            baseline
                .exchange(
                    &reader(),
                    get_document(&decoy.without_generation()).as_bytes()
                )
                .result,
            Knowledge::Known {
                value: TowerValue::Object(_),
                ..
            }
        ));
    }
    let mut router = TowerRouter::with_publication(imported(TENANT, &decoys, 1), adapter).unwrap();
    let precheck = precheck_document(&mount, 1, &proposal);
    assert!(matches!(
        precheck_result(&router.exchange(&reader(), precheck.as_bytes())).result,
        Ok(ActorKnowledge::Known {
            value: FilePrecheckOutcome::PrecheckPassed,
            ..
        })
    ));
    drop(grant);
    let revoked = router.exchange(&reader(), precheck.as_bytes());
    assert_eq!(revoked.reason_code(), Some(ReasonCode::DeniedByPolicy));
    assert!(matches!(
        precheck_result(&revoked).result,
        Ok(ActorKnowledge::Withheld { .. })
    ));
    observe(&mut supervisor);
    let call = submit_document(&mount, 73, &proposal);
    assert_eq!(
        request_result(&router.exchange(&access(), call.as_bytes())).result,
        Ok(ActorKnowledge::Pending { request: 73 })
    );
    drop(supervisor);
    let before = canonical(&root);
    let unavailable = router.exchange(&reader(), get_document(&address).as_bytes());
    assert_eq!(
        unavailable.reason_code(),
        Some(ReasonCode::LowerLayerUnverified)
    );
    assert_eq!(
        request_result(&unavailable).result,
        Ok(ActorKnowledge::Unknown {
            reason: ActorUnknown::ControllerUnavailable
        })
    );
    assert_eq!(
        request_result(&router.exchange(&access(), call.as_bytes())).result,
        Err(WireError::Unavailable)
    );
    assert_eq!(
        live(&router.exchange(&reader(), get_document(&mount).as_bytes()))
            .envelope
            .reason_code,
        Some(ReasonCode::MissingRequiredObservation)
    );
    assert_eq!(canonical(&root), before);
    known_evidence(
        &router.exchange(
            &operator(TENANT),
            get_document(&evidence(TENANT)).as_bytes(),
        ),
        TENANT,
    );
}
