//! The Tower replays detached evidence from a genuine original oversight session
//! inside a capability-free experiment branch. Fixture grants and retained
//! anchors are explicit reference assumptions, not authentication or live rights.
#![cfg(unix)]

#[path = "support/file_oversight.rs"]
mod native_fixture;

use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::delivery::PublicationEndpoint;
use fa_reference::action::consequence::experiment::{
    CounterfactualReport, EmpiricalStatus, Intervention, InterventionScope, MAX_INTERVENTIONS,
    NextRequirement,
};
use fa_reference::action::consequence::gate::TargetCeiling;
use fa_reference::action::consequence::gate::containment::session::policy::Truth;
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::replay::{
    ObservedDecisionArchive, ObservedReviewAnchor,
};
use fa_reference::action::consequence::oversight::{
    ObservedReceipt, OversightBroker, ReviewWindow,
};
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, Permit, VERSION};
use fa_reference::address_resolver::{Address, CallerScope, Noun, ResolvedObject, ResolverPlane};
use fa_reference::event_journal::{DomainEventJournal, JournalEventKind, TailBatch};
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::observation_algebra::{
    AuthorityKind, Basis, BasisIdentity, ClaimClass, Knowledge, ProofStates, ScopedProof,
    UnknownReason,
};
use fa_reference::reducer::MAX_VOTES;
use fa_reference::round::Verdict;
use fa_reference::strict_json::{self, Json};
use fa_reference::tower::replay::{
    ObservedReplayRecord, ObservedReplayReport, ReplayPlanMetadata, ReplayProfile,
};
use fa_reference::tower::{
    MAX_REPLAY_OPERATIONS, ReadArguments, ReasonCode, ReferenceTower, ResponseStatus, TowerAccess,
    TowerBuilder, TowerInstallError, TowerLimits, TowerRehearsalOutcome, TowerReplayPlan,
    TowerRequest, TowerResponse, TowerValue, Verb,
};
use fa_reference::typestate_cx::{ExperimentCx, PurposeTag, RootCx};
use fa_reference::{Error, Snapshot};
use std::rc::Rc;

const NATIVE_SEMANTIC_EPOCH: u64 = 23;
const ORIGINAL_EVIDENCE: &[u8] =
    b"Complete original helper evidence, including the decisive final paragraph.";
const TENANT: &str = "replay-tenant";
const DOMAIN: &str = "original-run";
const BRANCH_ID: u64 = 71;
const BRANCH_GENERATION: u64 = 42;
const BRANCH_POLICY_EPOCH: u64 = 11;
const SOURCE_CUT: u64 = 17;

#[derive(Clone, Copy)]
enum Roster {
    Complete,
    MissingBeta,
}

#[derive(Clone, Copy)]
enum OriginalPolicy {
    Satisfied,
    Violated,
}

struct NativeHistory {
    broker: OversightBroker,
    endpoint: PublicationEndpoint,
    anchor: ObservedReviewAnchor,
    archive: ObservedDecisionArchive,
    receipt: ObservedReceipt,
}

fn native_snapshot(policy: OriginalPolicy) -> Snapshot {
    let mut snapshot = Snapshot {
        semantic_epoch: NATIVE_SEMANTIC_EPOCH,
        ..native_fixture::snapshot()
    };
    if matches!(policy, OriginalPolicy::Violated) {
        snapshot.values.insert(7, b"bad".to_vec());
    }
    snapshot
}

fn original_history(roster: Roster, policy: OriginalPolicy, evidence: &[u8]) -> NativeHistory {
    let profile = native_fixture::profile();
    let delivery = profile.delivery;
    let mut endpoint = PublicationEndpoint::new(
        delivery.target,
        delivery.initial_payload,
        delivery.retention_ticks,
        delivery.max_deliveries,
    )
    .expect("original in-memory endpoint");
    let mut broker = OversightBroker::new(
        ControllerConfig {
            scope: delivery.scope,
            total: delivery.total,
            max_attempts: delivery.max_attempts,
            actor: delivery.actor,
            suspend_at_incident: delivery.suspend_at_incident,
            policy: delivery.policy,
            congress: delivery.congress,
            narrowed_targets: TargetCeiling::new(&delivery.narrowed_targets)
                .expect("target ceiling"),
        },
        &mut endpoint,
        profile.committee,
    )
    .expect("original oversight owner");
    let acknowledgment = endpoint
        .install_fence(broker.fence_request())
        .expect("original fence");
    broker
        .confirm_fence(acknowledgment)
        .expect("original fence acknowledgment");
    broker
        .observe_time(ElapsedTick(1))
        .expect("original review clock");
    let action = broker
        .propose(
            1,
            ActionSpec {
                version: VERSION,
                scope: delivery.scope,
                target: Some(delivery.target),
                payload: b"publish".to_vec(),
                required_witnesses: Vec::new(),
                policy_epoch: broker.inspect().ledger.epoch,
                deadline: ElapsedTick(100),
                units: 16,
            },
            &native_snapshot(OriginalPolicy::Satisfied),
        )
        .expect("original proposal")
        .action;
    let inputs = native_fixture::inputs(&action, evidence);
    broker
        .record_inputs(1, 0, inputs)
        .expect("whole actual input capture");
    let mut session = broker
        .begin_review(
            1,
            101,
            native_fixture::ROOT,
            ReviewWindow {
                commit_by: ElapsedTick(5),
                reveal_by: ElapsedTick(8),
            },
            &native_snapshot(policy),
        )
        .expect("original observed session");

    // This anchor is obtained before the first commitment. A reconstructed
    // post-vote anchor or a self-selected archive cannot replace this custody.
    let anchor = session
        .replay_anchor()
        .expect("independently retained pre-vote anchor");
    let members: &[&str] = match roster {
        Roster::Complete => &native_fixture::MEMBERS,
        Roster::MissingBeta => &["alpha"],
    };
    for member in members {
        let commitment = session
            .commitment(member, Verdict::Allow, &native_fixture::salt(member))
            .expect("original commitment");
        session
            .commit(member, commitment, ElapsedTick(2))
            .expect("accepted commitment");
    }
    let (opened_at, revealed_at, finished_at) = match roster {
        Roster::Complete => (ElapsedTick(3), ElapsedTick(4), ElapsedTick(4)),
        Roster::MissingBeta => {
            assert_eq!(session.open_reveals(ElapsedTick(3)), Err(Error::Incomplete));
            (ElapsedTick(5), ElapsedTick(6), ElapsedTick(8))
        }
    };
    session
        .open_reveals(opened_at)
        .expect("original phase transition");
    for member in members {
        session
            .reveal(
                member,
                Verdict::Allow,
                &native_fixture::salt(member),
                revealed_at,
            )
            .expect("accepted reveal");
    }
    let review = session
        .finish(finished_at)
        .expect("original completed review");
    let archive = review.replay_archive();
    assert_eq!(session.replay_anchor(), Err(Error::WrongState));
    archive
        .verify(&anchor)
        .expect("positive original replay control");
    broker.observe_time(finished_at).expect("application clock");
    let receipt = broker
        .apply_review(
            review,
            Some(anchor.inputs.as_ref()),
            &native_snapshot(policy),
        )
        .expect("actual original review application, without publication");
    archive
        .verify_receipt(&anchor, &receipt)
        .expect("actual receipt binding");
    assert_eq!(endpoint.execution_count(), 0);
    assert_eq!(endpoint.payload(), b"initial");
    NativeHistory {
        broker,
        endpoint,
        anchor,
        archive,
        receipt,
    }
}

fn source_basis() -> Basis {
    Basis::new(
        BasisIdentity {
            subject_id: 1701,
            question_id: 1702,
            profile_id: 1703,
            semantic_generation: NATIVE_SEMANTIC_EPOCH,
            policy_epoch: 0,
            model_epoch: 19,
            tokenizer_epoch: 29,
            codec_epoch: 31,
            control_seq: SOURCE_CUT,
        },
        ClaimClass::BoundedModel,
        ProofStates {
            authentic_origin: ScopedProof::Unsupported { scope_id: 1711 },
            complete_for_contract: ScopedProof::Declared {
                scope_id: 1712,
                generation: 23,
            },
            semantically_valid: ScopedProof::Gapped {
                scope_id: 1713,
                first_missing: 5,
            },
            available_for_replay: ScopedProof::Expired {
                scope_id: 1714,
                generation: 22,
            },
        },
    )
}

fn branch_basis(cut: u64) -> Basis {
    let proof = ScopedProof::Declared {
        scope_id: 71,
        generation: BRANCH_GENERATION,
    };
    Basis::new(
        BasisIdentity {
            subject_id: 7101,
            question_id: 7102,
            profile_id: 7103,
            semantic_generation: BRANCH_GENERATION,
            policy_epoch: BRANCH_POLICY_EPOCH,
            model_epoch: 101,
            tokenizer_epoch: 103,
            codec_epoch: 107,
            control_seq: cut,
        },
        ClaimClass::BoundedModel,
        ProofStates {
            authentic_origin: proof,
            complete_for_contract: proof,
            semantically_valid: proof,
            available_for_replay: proof,
        },
    )
}

fn context(tenant: u64, run: u64, branch: u64, epoch: u64) -> ExperimentCx {
    RootCx::new_nucleus(tenant, run, epoch)
        .mint_experiment(branch)
        .expect("nonzero experiment branch")
}

fn experiment_context() -> ExperimentCx {
    context(1, 3, BRANCH_ID, BRANCH_POLICY_EPOCH)
}

fn artifact_address() -> Address {
    Address::new(
        TENANT,
        Noun::Branch,
        "paired-round-101",
        Some(BRANCH_GENERATION),
    )
    .expect("branch artifact address")
}

fn source_address(with_receipt: bool) -> Address {
    Address::new(
        TENANT,
        if with_receipt {
            Noun::Receipt
        } else {
            Noun::Closure
        },
        "original-round-101",
        Some(NATIVE_SEMANTIC_EPOCH),
    )
    .expect("historical source address")
}

fn registration() -> InterventionScope {
    InterventionScope::new(BRANCH_ID, true, false, false, &[7]).expect("bounded registered edits")
}

fn record(history: &NativeHistory, with_receipt: bool) -> ObservedReplayRecord {
    ObservedReplayRecord::new(
        history.anchor.policy.action.spec().scope,
        history.anchor.clone(),
        history.archive.clone(),
        with_receipt.then(|| history.receipt.clone()),
        registration(),
    )
    .expect("bounded original history installation")
}

fn branch_builder(limits: TowerLimits, units: u64) -> TowerBuilder {
    let cx = experiment_context();
    let journal = DomainEventJournal::new_branch(&cx, TENANT, DOMAIN, BRANCH_ID);
    let mut builder = TowerBuilder::new(journal, branch_basis(0), limits, units)
        .expect("empty capability-free branch");
    builder
        .enable_replay(cx)
        .expect("explicit experiment context");
    builder
        .insert_object(
            ResolvedObject {
                address: artifact_address(),
                plane: ResolverPlane::ExperimentStore,
                summary_digest: 101,
                generation: BRANCH_GENERATION,
            },
            branch_basis(0),
        )
        .expect("new branch-owned artifact");
    builder
}

fn tower(
    record: Option<ObservedReplayRecord>,
    with_receipt: bool,
    limits: TowerLimits,
    units: u64,
) -> ReferenceTower {
    let mut builder = branch_builder(limits, units);
    if let Some(record) = record {
        builder
            .insert_replay(
                artifact_address(),
                branch_basis(0),
                source_address(with_receipt),
                source_basis(),
                record,
            )
            .expect("explicit original-source mapping");
    }
    builder.build()
}

fn access() -> TowerAccess {
    TowerAccess::new(
        CallerScope::full_tenant_access(TENANT),
        true,
        vec![PurposeTag::Experiment],
        10,
    )
}

fn replay_request(key: &str, interventions: Vec<Intervention>) -> TowerRequest {
    let mut request = TowerRequest::new("paired-replay", artifact_address(), Verb::Replay);
    request.idempotency_key = Some(key.to_owned());
    request.arguments = ReadArguments::Replay { interventions };
    request
}

fn plan_request() -> TowerRequest {
    TowerRequest::new("inspect-replay-plan", artifact_address(), Verb::ReplayPlan)
}

fn known<T>(knowledge: &Knowledge<T>) -> (&T, Basis) {
    match knowledge {
        Knowledge::Known { value, basis } => (value, *basis),
        other => panic!("expected Known, got {}", other.variant_name()),
    }
}

fn outcome(response: &TowerResponse) -> (&TowerRehearsalOutcome, Basis) {
    match known(&response.result) {
        (TowerValue::Replay(outcome), basis) => (outcome, basis),
        _ => panic!("expected a typed rehearsal outcome"),
    }
}

fn plan(response: &TowerResponse) -> (&TowerReplayPlan, Basis) {
    match known(&response.result) {
        (TowerValue::ReplayPlan(plan), basis) => (plan, basis),
        _ => panic!("expected an inspection of the registered replay plan"),
    }
}

fn assert_unverified_plan(metadata: &ReplayPlanMetadata) {
    for property in [
        &metadata.properties.replay_consistency,
        &metadata.properties.retained_anchor_binding,
        &metadata.properties.receipt_binding,
        &metadata.properties.cryptographic_authentication,
        &metadata.properties.independent_verifier_isolation,
        &metadata.properties.provider_truth,
        &metadata.properties.external_execution,
        &metadata.properties.current_eligibility,
        &metadata.properties.full_ledger_accounting,
    ] {
        assert_eq!(property, &Knowledge::unknown(UnknownReason::Unavailable));
    }
}

fn assert_evidence_limits(report: &ObservedReplayReport, with_receipt: bool) {
    assert_eq!(report.profile, ReplayProfile::ObservedPolicyExperiment);
    assert_eq!(
        known(&report.properties.replay_consistency),
        (&true, source_basis())
    );
    assert_eq!(
        known(&report.properties.retained_anchor_binding),
        (&true, source_basis())
    );
    if with_receipt {
        assert_eq!(
            known(&report.properties.receipt_binding),
            (&true, source_basis())
        );
    } else {
        assert_eq!(
            report.properties.receipt_binding,
            Knowledge::unknown(UnknownReason::Unavailable)
        );
    }
    for property in [
        &report.properties.cryptographic_authentication,
        &report.properties.independent_verifier_isolation,
        &report.properties.provider_truth,
        &report.properties.external_execution,
        &report.properties.current_eligibility,
        &report.properties.full_ledger_accounting,
    ] {
        assert_eq!(property, &Knowledge::unknown(UnknownReason::Unavailable));
    }
    assert!(
        report
            .properties
            .replay_consistency
            .as_known(ClaimClass::BoundedModel, 23)
            .is_err(),
        "successful replay must preserve the source's unsupported/gapped/expired proofs"
    );
}

fn parse(encoded: &str) -> Json {
    strict_json::parse(encoded.as_bytes(), KNOWLEDGE_JSON_LIMITS).expect("strict response JSON")
}

fn field<'a>(value: &'a Json, name: &str) -> &'a Json {
    value
        .get(name)
        .unwrap_or_else(|| panic!("missing field {name}"))
}

fn text<'a>(value: &'a Json, name: &str) -> &'a str {
    field(value, name)
        .as_str()
        .unwrap_or_else(|| panic!("non-string field {name}"))
}

fn number(value: &Json, name: &str) -> u64 {
    field(value, name)
        .as_u64()
        .unwrap_or_else(|| panic!("non-u64 field {name}"))
}

fn body(json: &Json) -> &Json {
    let payload = field(field(json, "result"), "value");
    assert_eq!(text(payload, "kind"), "replay");
    field(payload, "value")
}

fn plan_body(json: &Json) -> &Json {
    let payload = field(field(json, "result"), "value");
    assert_eq!(text(payload, "kind"), "replay-plan");
    field(payload, "value")
}

fn tail(tower: &mut ReferenceTower) -> TailBatch {
    let request = TowerRequest::new(
        "inspect-branch-journal",
        tower.domain_address().clone(),
        Verb::Tail,
    );
    let response = tower.dispatch(&access(), request);
    let (TowerValue::Tail(batch), _) = known(&response.result) else {
        panic!("branch tail");
    };
    batch.clone()
}

fn assert_no_permit_conversion() {
    // The blanket candidates become ambiguous if an Into<Permit> conversion is
    // ever added. This is checked while compiling this actual integration test.
    struct Converts;
    trait NoPermit<A> {
        fn check() {}
    }
    impl<T> NoPermit<()> for T {}
    impl<T: Into<Permit>> NoPermit<Converts> for T {}
    let _ = <TowerRehearsalOutcome as NoPermit<_>>::check;
    let _ = <ObservedReplayReport as NoPermit<_>>::check;
    let _ = <CounterfactualReport as NoPermit<_>>::check;
}

#[test]
fn real_policy_repair_preserves_original_denial_source_proofs_and_live_owners() {
    assert_no_permit_conversion();
    let history = original_history(
        Roster::Complete,
        OriginalPolicy::Violated,
        ORIGINAL_EVIDENCE,
    );
    let original_state = history.broker.inspect();
    assert_eq!(original_state.ledger.stages[&1], ActionState::Denied);
    let original_replay = history
        .archive
        .verify_receipt(&history.anchor, &history.receipt)
        .expect("original reference replay");
    assert_eq!(original_replay.decision().consequence, Consequence::Deny);
    assert_ne!(history.receipt.policy.control.sequence, SOURCE_CUT);
    let mut tower = tower(
        Some(record(&history, true)),
        true,
        TowerLimits::default(),
        12,
    );
    let edits = vec![Intervention::Key {
        key: 7,
        expected: Some(b"bad".to_vec()),
        replacement: Some(b"ok".to_vec()),
    }];
    let request = replay_request("repair-original-denial", edits.clone());
    let response = tower.dispatch(&access(), request.clone());
    assert_eq!(response.envelope.status, ResponseStatus::Ok);
    assert!(response.envelope.retry_safe);
    let (rehearsal, result_basis) = outcome(&response);
    assert!(rehearsal.rehearsal);
    assert_eq!(rehearsal.branch_id, BRANCH_ID);
    assert_eq!(rehearsal.source_address, source_address(true));
    assert_eq!(rehearsal.source_basis, source_basis());
    assert_eq!(result_basis, branch_basis(1));
    assert_eq!(
        rehearsal.source.native_scope,
        history.anchor.policy.action.spec().scope
    );
    assert_eq!(rehearsal.source.native_attempt, 1);
    assert_eq!(rehearsal.source.native_round, 101);
    assert_eq!(
        rehearsal.source.native_expected_control_sequence,
        history.anchor.policy.expected_control_sequence
    );
    assert_eq!(
        rehearsal.source.native_receipt_control_sequence,
        Some(history.receipt.policy.control.sequence)
    );
    assert_eq!(rehearsal.source.replayed, original_replay);
    let counterfactual = &rehearsal.source.counterfactual;
    assert_eq!(counterfactual.baseline(), Truth::Violated);
    assert_eq!(counterfactual.counterfactual(), Truth::Satisfied);
    assert_eq!(
        counterfactual.empirical_status(),
        EmpiricalStatus::InvalidatedByIntervention
    );
    assert_eq!(
        counterfactual.next_requirement(),
        NextRequirement::FreshIndependentReview
    );
    assert_eq!(counterfactual.interventions(), edits);
    assert_eq!(
        counterfactual
            .changes()
            .iter()
            .map(|change| change.node)
            .collect::<Vec<_>>(),
        [0, 2]
    );
    assert_evidence_limits(&rehearsal.source, true);
    assert_eq!(*known(&response.envelope.realized_cost).0, 1);
    assert_eq!(tower.remaining_read_units(), 11);
    assert_eq!(tower.journal_control_seq(), 1);

    let json = parse(&response.to_json().expect("complete paired evidence"));
    let wire = body(&json);
    assert_eq!(field(wire, "rehearsal").as_bool(), Some(true));
    assert_eq!(number(wire, "branch_id"), BRANCH_ID);
    assert_eq!(
        text(wire, "source_address"),
        source_address(true).canonical_string()
    );
    let historical = field(wire, "source_basis");
    assert_eq!(text(historical, "state"), "Known");
    assert_eq!(number(historical, "value"), SOURCE_CUT);
    let source_identity = field(field(historical, "basis"), "identity");
    assert_eq!(number(source_identity, "control_seq"), SOURCE_CUT);
    assert_eq!(
        number(source_identity, "semantic_generation"),
        NATIVE_SEMANTIC_EPOCH
    );
    assert_eq!(number(source_identity, "policy_epoch"), 0);
    let branch_identity = field(field(field(&json, "result"), "basis"), "identity");
    assert_eq!(number(branch_identity, "control_seq"), 1);
    assert_eq!(
        number(branch_identity, "semantic_generation"),
        BRANCH_GENERATION
    );
    assert_eq!(number(branch_identity, "policy_epoch"), BRANCH_POLICY_EPOCH);
    let wire_source = field(wire, "source");
    assert_eq!(text(wire_source, "profile"), "observed_policy_experiment");
    assert_eq!(number(field(wire_source, "native_scope"), "tenant"), 1);
    assert_eq!(number(field(wire_source, "native_scope"), "run"), 3);
    assert_eq!(number(field(wire_source, "native_scope"), "branch"), 4);
    assert_eq!(
        number(wire_source, "native_receipt_control_sequence"),
        history.receipt.policy.control.sequence
    );
    let original_evaluation = field(field(wire_source, "replayed"), "evaluation");
    let witnesses = field(original_evaluation, "witnesses")
        .as_array()
        .expect("original exact evidence");
    assert_eq!(number(&witnesses[0], "key"), 7);
    assert_eq!(text(&witnesses[0], "value_hex"), "626164");
    let pair = field(wire_source, "counterfactual");
    assert_eq!(
        text(pair, "empirical_status"),
        "invalidated_by_intervention"
    );
    assert_eq!(text(pair, "next_requirement"), "fresh_independent_review");
    assert_eq!(
        field(pair, "trace")
            .as_array()
            .expect("complete paired trace")
            .len(),
        3
    );
    assert_eq!(
        field(pair, "changes")
            .as_array()
            .expect("actual node changes")
            .len(),
        2
    );
    let wire_edit = &field(pair, "interventions")
        .as_array()
        .expect("retained intervention")[0];
    assert_eq!(number(wire_edit, "key"), 7);
    assert_eq!(text(wire_edit, "expected_hex"), "626164");
    assert_eq!(text(wire_edit, "replacement_hex"), "6f6b");
    let properties = field(wire_source, "properties");
    assert_eq!(
        properties.as_object().expect("per-property evidence").len(),
        9
    );
    for name in [
        "cryptographic_authentication",
        "independent_verifier_isolation",
        "provider_truth",
        "external_execution",
        "current_eligibility",
        "full_ledger_accounting",
    ] {
        assert_eq!(text(field(properties, name), "state"), "Unknown");
        assert!(field(properties, name).get("value").is_none());
    }

    let retry = tower.dispatch(&access(), request.clone());
    assert_eq!(retry, response);
    assert_eq!(retry.to_json(), response.to_json());
    assert_eq!(tower.remaining_read_units(), 11);
    assert_eq!(tower.journal_control_seq(), 1);
    let journal = tail(&mut tower);
    assert_eq!(journal.events.len(), 1);
    assert!(matches!(
        journal.events[0].kind,
        JournalEventKind::BranchOutcome {
            branch_id: BRANCH_ID,
            ..
        }
    ));
    assert_eq!(
        tower.dispatch(&access(), request),
        response,
        "cached response survives later branch reads"
    );
    assert_eq!(tower.remaining_read_units(), 10);
    assert_eq!(history.broker.inspect(), original_state);
    assert_eq!(history.endpoint.execution_count(), 0);
    assert_eq!(history.endpoint.payload(), b"initial");
}

#[test]
fn missing_specialist_and_noop_replays_stay_historical_without_receipt_or_permission_claims() {
    let history = original_history(
        Roster::MissingBeta,
        OriginalPolicy::Satisfied,
        ORIGINAL_EVIDENCE,
    );
    let original = history.broker.inspect();
    let mut tower = tower(
        Some(record(&history, false)),
        false,
        TowerLimits::default(),
        4,
    );
    for (key, edits) in [
        ("unchanged-history", Vec::new()),
        (
            "explicit-noop",
            vec![Intervention::Payload(b"publish".to_vec())],
        ),
    ] {
        let response = tower.dispatch(&access(), replay_request(key, edits));
        let (rehearsal, _) = outcome(&response);
        assert_eq!(
            rehearsal.source.replayed.decision().consequence,
            Consequence::HoldEffect
        );
        assert_eq!(rehearsal.source.replayed.missing(), &["beta".to_owned()]);
        assert_eq!(rehearsal.source.counterfactual.baseline(), Truth::Satisfied);
        assert_eq!(
            rehearsal.source.counterfactual.counterfactual(),
            Truth::Satisfied
        );
        assert_eq!(
            rehearsal.source.counterfactual.empirical_status(),
            EmpiricalStatus::HistoricalOnly
        );
        assert_eq!(rehearsal.source.native_receipt_control_sequence, None);
        assert_evidence_limits(&rehearsal.source, false);
    }
    assert_eq!(tower.remaining_read_units(), 2);
    assert_eq!(tower.journal_control_seq(), 2);
    assert_eq!(history.broker.inspect(), original);
    assert_eq!(history.endpoint.execution_count(), 0);
}

#[test]
fn original_anchor_input_vote_time_and_receipt_failures_spend_once_without_branch_events() {
    let history = original_history(
        Roster::Complete,
        OriginalPolicy::Satisfied,
        ORIGINAL_EVIDENCE,
    );
    let original = history.broker.inspect();
    let mut cases = Vec::new();
    let mut anchor = history.anchor.clone();
    anchor.input_revision += 1;
    cases.push((
        "different-anchor",
        anchor,
        history.archive.clone(),
        history.receipt.clone(),
    ));
    let mut changed = history.archive.clone();
    changed.inputs = Rc::new(native_fixture::inputs(
        &history.anchor.policy.action,
        b"spliced decisive paragraph",
    ));
    cases.push((
        "different-whole-input",
        history.anchor.clone(),
        changed,
        history.receipt.clone(),
    ));
    let mut changed = history.archive.clone();
    changed.policy.transcript.reveals[0].verdict = Verdict::Deny;
    cases.push((
        "changed-vote",
        history.anchor.clone(),
        changed,
        history.receipt.clone(),
    ));
    let mut changed = history.archive.clone();
    changed.commit_times[0] = history.anchor.window.commit_by;
    cases.push((
        "commit-at-closed-deadline",
        history.anchor.clone(),
        changed,
        history.receipt.clone(),
    ));
    let mut receipt = history.receipt.clone();
    receipt.input_revision += 1;
    cases.push((
        "different-receipt",
        history.anchor.clone(),
        history.archive.clone(),
        receipt,
    ));

    for (name, expected, archive, receipt) in cases {
        assert!(
            archive.verify_receipt(&expected, &receipt).is_err(),
            "native negative control {name}"
        );
        let source = ObservedReplayRecord::new(
            history.anchor.policy.action.spec().scope,
            expected,
            archive,
            Some(receipt),
            registration(),
        )
        .expect("bounded untrusted data may be installed");
        let mut failed = tower(Some(source), true, TowerLimits::default(), 1);
        let request = replay_request(name, Vec::new());
        let response = failed.dispatch(&access(), request.clone());
        assert_eq!(response.envelope.status, ResponseStatus::Error, "{name}");
        assert_eq!(
            response.envelope.reason_code,
            Some(ReasonCode::IntegrityFailure),
            "{name}"
        );
        assert_eq!(
            response.result,
            Knowledge::unknown(UnknownReason::Incomplete)
        );
        assert_eq!(*known(&response.envelope.realized_cost).0, 1);
        assert_eq!(failed.remaining_read_units(), 0);
        assert_eq!(failed.journal_control_seq(), 0);
        assert_eq!(
            failed.dispatch(&access(), request),
            response,
            "failed work is cached even after the last unit was spent"
        );
        assert_eq!(failed.remaining_read_units(), 0);
        assert_eq!(failed.journal_control_seq(), 0);
        let mut permitted = tower(
            Some(record(&history, true)),
            true,
            TowerLimits::default(),
            1,
        );
        assert!(
            matches!(
                permitted
                    .dispatch(&access(), replay_request(name, Vec::new()))
                    .result,
                Knowledge::Known {
                    value: TowerValue::Replay(_),
                    ..
                }
            ),
            "permitted twin {name}"
        );
    }
    assert_eq!(history.broker.inspect(), original);
    assert_eq!(history.endpoint.execution_count(), 0);
}

#[test]
fn authorization_is_checked_before_history_lookup_and_before_returning_cached_results() {
    let history = original_history(
        Roster::Complete,
        OriginalPolicy::Satisfied,
        ORIGINAL_EVIDENCE,
    );
    let mut missing_plane = CallerScope::full_tenant_access(TENANT);
    missing_plane
        .allowed_planes
        .remove(&ResolverPlane::ControlLedger);
    let mut missing_authority = CallerScope::full_tenant_access(TENANT);
    missing_authority
        .granted_authorities
        .remove(&AuthorityKind::Verify);
    for denied in [
        TowerAccess::new(
            CallerScope::full_tenant_access("other-tenant"),
            true,
            vec![PurposeTag::Experiment],
            10,
        ),
        TowerAccess::new(
            CallerScope::full_tenant_access(TENANT),
            false,
            vec![PurposeTag::Experiment],
            10,
        ),
        TowerAccess::new(missing_plane, true, vec![PurposeTag::Experiment], 10),
        TowerAccess::new(missing_authority, true, vec![PurposeTag::Experiment], 10),
    ] {
        let mut present = tower(
            Some(record(&history, true)),
            true,
            TowerLimits::default(),
            5,
        );
        let mut missing = tower(None, true, TowerLimits::default(), 9);
        let request = replay_request("reauthorize-original", Vec::new());
        let refusal = present.dispatch(&denied, request.clone());
        let no_history = missing.dispatch(&denied, request.clone());
        assert_eq!(refusal, no_history);
        assert_eq!(refusal.to_json(), no_history.to_json());
        assert!(matches!(refusal.result, Knowledge::Withheld { .. }));
        assert_eq!(
            refusal.envelope.reason_code,
            Some(ReasonCode::DeniedByPolicy)
        );
        assert_eq!(present.remaining_read_units(), 5);
        assert_eq!(missing.remaining_read_units(), 9);
        assert_eq!(present.journal_control_seq(), 0);
        let accepted = present.dispatch(&access(), request.clone());
        assert!(matches!(
            accepted.result,
            Knowledge::Known {
                value: TowerValue::Replay(_),
                ..
            }
        ));
        let revoked = present.dispatch(&denied, request);
        assert!(
            matches!(revoked.result, Knowledge::Withheld { .. }),
            "cache is not an authorization bypass"
        );
        assert_eq!(present.remaining_read_units(), 4);
        assert_eq!(present.journal_control_seq(), 1);
    }
    let mut tower = tower(
        Some(record(&history, true)),
        true,
        TowerLimits::default(),
        2,
    );
    let request = replay_request("purpose-control", Vec::new());
    let wrong_purpose = TowerAccess::new(
        CallerScope::full_tenant_access(TENANT),
        true,
        vec![PurposeTag::Permit, PurposeTag::Capture],
        10,
    );
    let refusal = tower.dispatch(&wrong_purpose, request.clone());
    assert_eq!(
        refusal.envelope.reason_code,
        Some(ReasonCode::PurposeMismatch)
    );
    assert_eq!(tower.remaining_read_units(), 2);
    assert_eq!(tower.journal_control_seq(), 0);
    assert!(matches!(
        tower.dispatch(&access(), request).result,
        Knowledge::Known { .. }
    ));
}

#[test]
fn generation_key_argument_and_work_admission_failures_do_not_consume_or_poison_replay() {
    let history = original_history(
        Roster::Complete,
        OriginalPolicy::Satisfied,
        ORIGINAL_EVIDENCE,
    );
    let mut tower = tower(
        Some(record(&history, true)),
        true,
        TowerLimits::default(),
        4,
    );
    for generation in [0, BRANCH_GENERATION - 1, BRANCH_GENERATION + 1, u64::MAX] {
        let mut request = replay_request("generation-control", Vec::new());
        request.address.generation = Some(generation);
        let response = tower.dispatch(&access(), request);
        assert_eq!(
            response.envelope.reason_code,
            Some(ReasonCode::StaleGeneration)
        );
        if generation < BRANCH_GENERATION {
            assert_eq!(
                response.result,
                Knowledge::stale(branch_basis(0), BRANCH_GENERATION)
            );
        } else {
            assert_eq!(
                response.result,
                Knowledge::unknown(UnknownReason::IncompatibleGeneration)
            );
        }
        assert_eq!(*known(&response.envelope.realized_cost).0, 0);
        assert_eq!(tower.remaining_read_units(), 4);
        assert_eq!(tower.journal_control_seq(), 0);
    }
    let mut missing = replay_request("absent-record", Vec::new());
    missing.address.id = "absent-branch-artifact".to_owned();
    assert_eq!(
        tower.dispatch(&access(), missing).envelope.reason_code,
        Some(ReasonCode::ReplayClosureUnavailable)
    );
    let mut no_key = replay_request("unused", Vec::new());
    no_key.idempotency_key = None;
    let mut wrong_arguments = replay_request("unused", Vec::new());
    wrong_arguments.arguments = ReadArguments::Tail {
        since_seq: 0,
        max_events: 1,
    };
    for request in [no_key, wrong_arguments] {
        assert_eq!(
            tower.dispatch(&access(), request).envelope.reason_code,
            Some(ReasonCode::IntegrityFailure)
        );
    }
    let too_many_edits = replay_request(
        "edit-limit",
        vec![Intervention::Payload(Vec::new()); MAX_INTERVENTIONS + 1],
    );
    assert_eq!(
        tower
            .dispatch(&access(), too_many_edits)
            .envelope
            .reason_code,
        Some(ReasonCode::BudgetExhausted)
    );
    assert_eq!(tower.remaining_read_units(), 4);
    assert_eq!(tower.journal_control_seq(), 0);
    let original = replay_request("generation-control", Vec::new());
    let accepted = tower.dispatch(&access(), original.clone());
    assert!(matches!(accepted.result, Knowledge::Known { .. }));
    for changed in [
        {
            let mut changed = original.clone();
            changed.operation_id = "different-operation".to_owned();
            changed
        },
        replay_request(
            "generation-control",
            vec![Intervention::Payload(b"changed".to_vec())],
        ),
    ] {
        assert_eq!(
            tower.dispatch(&access(), changed).envelope.reason_code,
            Some(ReasonCode::IntegrityFailure)
        );
        assert_eq!(tower.remaining_read_units(), 3);
        assert_eq!(tower.journal_control_seq(), 1);
    }
    assert_eq!(tower.dispatch(&access(), original), accepted);
    let verify = TowerRequest::new(
        "independent-verifier-not-connected",
        artifact_address(),
        Verb::Verify,
    );
    assert_eq!(
        tower.dispatch(&access(), verify).envelope.reason_code,
        Some(ReasonCode::CapabilityUnsupported)
    );
    assert_eq!(tower.remaining_read_units(), 3);
    assert_eq!(tower.journal_control_seq(), 1);
}

#[test]
fn source_registration_binds_native_identity_historical_metadata_and_a_fresh_branch_context() {
    let history = original_history(
        Roster::Complete,
        OriginalPolicy::Satisfied,
        ORIGINAL_EVIDENCE,
    );
    let native_scope = history.anchor.policy.action.spec().scope;
    assert!(matches!(
        ObservedReplayRecord::new(
            fa_reference::action::Scope {
                tenant: native_scope.tenant + 1,
                ..native_scope
            },
            history.anchor.clone(),
            history.archive.clone(),
            Some(history.receipt.clone()),
            registration(),
        ),
        Err(Error::Binding)
    ));
    let mut oversized = history.archive.clone();
    oversized.commit_times = vec![ElapsedTick(2); MAX_VOTES + 1];
    assert!(matches!(
        ObservedReplayRecord::new(
            native_scope,
            history.anchor.clone(),
            oversized,
            Some(history.receipt.clone()),
            registration()
        ),
        Err(Error::Limit)
    ));
    assert!(record(&history, true).retained_bytes() > history.anchor.inputs.logical_bytes());
    for cx in [
        context(2, 3, BRANCH_ID, BRANCH_POLICY_EPOCH),
        context(1, 4, BRANCH_ID, BRANCH_POLICY_EPOCH),
        context(1, 3, native_scope.branch, BRANCH_POLICY_EPOCH),
    ] {
        assert!(matches!(
            record(&history, true).run(&cx, &[], source_basis()),
            Err(Error::Binding)
        ));
    }
    assert!(
        record(&history, true)
            .run(&experiment_context(), &[], source_basis())
            .is_ok(),
        "a newer branch policy epoch must not be equated with the historical action epoch"
    );

    let mut builder = branch_builder(TowerLimits::default(), 2);
    for (source_address, source_basis) in [
        (
            Address {
                generation: Some(NATIVE_SEMANTIC_EPOCH + 1),
                ..source_address(true)
            },
            source_basis(),
        ),
        (
            Address {
                tenant: "private-tenant".to_owned(),
                ..source_address(true)
            },
            source_basis(),
        ),
        (
            Address {
                kind: Noun::Evidence,
                ..source_address(true)
            },
            source_basis(),
        ),
        (
            source_address(true),
            Basis::new(
                BasisIdentity {
                    policy_epoch: 1,
                    ..source_basis().identity()
                },
                source_basis().claim_class(),
                source_basis().proofs(),
            ),
        ),
    ] {
        assert_eq!(
            builder.insert_replay(
                artifact_address(),
                branch_basis(0),
                source_address,
                source_basis,
                record(&history, true)
            ),
            Err(TowerInstallError::InvalidReplaySource)
        );
    }
    assert_eq!(
        builder.insert_replay(
            artifact_address(),
            branch_basis(0),
            source_address(true),
            source_basis(),
            record(&history, false)
        ),
        Err(TowerInstallError::InvalidReplaySource)
    );
    assert_eq!(
        builder.insert_replay(
            Address {
                generation: Some(BRANCH_GENERATION - 1),
                ..artifact_address()
            },
            branch_basis(0),
            source_address(true),
            source_basis(),
            record(&history, true),
        ),
        Err(TowerInstallError::SnapshotMismatch)
    );
    assert_eq!(
        builder.insert_replay(
            artifact_address(),
            branch_basis(1),
            source_address(true),
            source_basis(),
            record(&history, true)
        ),
        Err(TowerInstallError::SnapshotMismatch)
    );
    builder
        .insert_replay(
            artifact_address(),
            branch_basis(0),
            source_address(true),
            source_basis(),
            record(&history, true),
        )
        .expect("corrected source/cut mapping");
    assert_eq!(
        builder.insert_replay(
            artifact_address(),
            branch_basis(0),
            source_address(true),
            source_basis(),
            record(&history, true)
        ),
        Err(TowerInstallError::DuplicateObject)
    );
    let mut tower = builder.build();
    let response = tower.dispatch(&access(), replay_request("corrected-import", Vec::new()));
    assert_eq!(outcome(&response).0.source_basis, source_basis());
}

#[test]
fn production_missing_context_and_wrong_branch_epoch_cannot_enter_replay() {
    let mut production = TowerBuilder::new(
        DomainEventJournal::new_production(TENANT, DOMAIN),
        branch_basis(0),
        TowerLimits::default(),
        2,
    )
    .expect("production read snapshot");
    assert_eq!(
        production.enable_replay(experiment_context()),
        Err(TowerInstallError::ReplayContextRequired)
    );
    let mut production = production.build();
    let response = production.dispatch(&access(), replay_request("production-denial", Vec::new()));
    assert_eq!(
        response.envelope.reason_code,
        Some(ReasonCode::PurposeMismatch)
    );
    assert_eq!(production.remaining_read_units(), 2);
    assert_eq!(production.journal_control_seq(), 0);

    let cx = experiment_context();
    let mut branch = TowerBuilder::new(
        DomainEventJournal::new_branch(&cx, TENANT, DOMAIN, BRANCH_ID),
        branch_basis(0),
        TowerLimits::default(),
        2,
    )
    .expect("branch snapshot without context");
    assert_eq!(
        branch.enable_replay(context(1, 3, BRANCH_ID + 1, BRANCH_POLICY_EPOCH)),
        Err(TowerInstallError::SnapshotMismatch)
    );
    assert_eq!(
        branch.enable_replay(context(1, 3, BRANCH_ID, BRANCH_POLICY_EPOCH + 1)),
        Err(TowerInstallError::SnapshotMismatch)
    );
    let mut missing_context = branch.build();
    let response =
        missing_context.dispatch(&access(), replay_request("missing-context", Vec::new()));
    assert_eq!(
        response.envelope.reason_code,
        Some(ReasonCode::PurposeMismatch)
    );
    assert_eq!(missing_context.remaining_read_units(), 2);
    assert_eq!(missing_context.journal_control_seq(), 0);
    let history = original_history(
        Roster::Complete,
        OriginalPolicy::Satisfied,
        ORIGINAL_EVIDENCE,
    );
    let mut permitted = tower(
        Some(record(&history, true)),
        true,
        TowerLimits::default(),
        2,
    );
    assert!(matches!(
        permitted
            .dispatch(&access(), replay_request("context-control", Vec::new()))
            .result,
        Knowledge::Known {
            value: TowerValue::Replay(_),
            ..
        }
    ));
}

#[test]
fn semantic_edit_failures_are_charged_and_branch_capacity_refuses_before_more_work() {
    let history = original_history(
        Roster::Complete,
        OriginalPolicy::Satisfied,
        ORIGINAL_EVIDENCE,
    );
    let mut tower = tower(
        Some(record(&history, true)),
        true,
        TowerLimits {
            max_journal_events: 1,
            ..TowerLimits::default()
        },
        4,
    );
    for (key, edit) in [
        (
            "stale-expected-value",
            Intervention::Key {
                key: 7,
                expected: Some(b"not-ok".to_vec()),
                replacement: Some(b"bad".to_vec()),
            },
        ),
        ("outside-registration", Intervention::Units(9)),
    ] {
        let request = replay_request(key, vec![edit]);
        let before = tower.remaining_read_units();
        let refused = tower.dispatch(&access(), request.clone());
        assert_eq!(
            refused.envelope.reason_code,
            Some(ReasonCode::IntegrityFailure)
        );
        assert_eq!(*known(&refused.envelope.realized_cost).0, 1);
        assert_eq!(tower.remaining_read_units(), before - 1);
        assert_eq!(tower.journal_control_seq(), 0);
        assert_eq!(tower.dispatch(&access(), request), refused);
        assert_eq!(tower.remaining_read_units(), before - 1);
    }
    let query = replay_request(
        "permitted-causal-change",
        vec![Intervention::Key {
            key: 7,
            expected: Some(b"ok".to_vec()),
            replacement: Some(b"bad".to_vec()),
        }],
    );
    let accepted = tower.dispatch(&access(), query.clone());
    assert_eq!(
        outcome(&accepted).0.source.counterfactual.counterfactual(),
        Truth::Violated
    );
    assert_eq!(tower.remaining_read_units(), 1);
    assert_eq!(tower.journal_control_seq(), 1);
    let full = tower.dispatch(
        &access(),
        replay_request("would-overflow-journal", Vec::new()),
    );
    assert_eq!(full.envelope.reason_code, Some(ReasonCode::BudgetExhausted));
    assert_eq!(*known(&full.envelope.realized_cost).0, 0);
    assert_eq!(tower.remaining_read_units(), 1);
    assert_eq!(tower.journal_control_seq(), 1);
    assert_eq!(
        tower.dispatch(&access(), query),
        accepted,
        "capacity cannot invalidate the original retry"
    );
    assert_eq!(tower.remaining_read_units(), 1);
}

#[test]
fn output_limit_keeps_completed_branch_work_and_caches_the_exact_unknown_result() {
    let history = original_history(
        Roster::Complete,
        OriginalPolicy::Satisfied,
        ORIGINAL_EVIDENCE,
    );
    let query = replay_request(
        "bounded-large-intervention",
        vec![Intervention::Payload(vec![b'x'; 16 * 1024])],
    );
    let mut permitted = tower(
        Some(record(&history, true)),
        true,
        TowerLimits::default(),
        2,
    );
    let complete = permitted.dispatch(&access(), query.clone());
    assert_eq!(
        outcome(&complete).0.source.counterfactual.counterfactual(),
        Truth::Violated
    );
    let complete_json = complete.to_json().expect("complete larger response");
    assert!(
        complete_json.len() > 16 * 1024,
        "positive control must cross the smaller output cap"
    );
    let mut bounded = tower(
        Some(record(&history, true)),
        true,
        TowerLimits {
            max_response_bytes: 16 * 1024,
            ..TowerLimits::default()
        },
        1,
    );
    let refused = bounded.dispatch(&access(), query.clone());
    assert_eq!(refused.envelope.status, ResponseStatus::Error);
    assert_eq!(
        refused.envelope.reason_code,
        Some(ReasonCode::BudgetExhausted)
    );
    assert_eq!(
        refused.result,
        Knowledge::unknown(UnknownReason::Incomplete)
    );
    assert_eq!(*known(&refused.envelope.realized_cost).0, 1);
    assert_eq!(bounded.remaining_read_units(), 0);
    assert_eq!(bounded.journal_control_seq(), 1);
    let refused_json = refused.to_json().expect("bounded failure response");
    assert!(refused_json.len() <= 16 * 1024);
    assert!(
        field(&parse(&refused_json), "result")
            .get("value")
            .is_none()
    );
    let retry = bounded.dispatch(&access(), query);
    assert_eq!(retry, refused);
    assert_eq!(retry.to_json().expect("same refusal"), refused_json);
    assert_eq!(bounded.remaining_read_units(), 0);
    assert_eq!(bounded.journal_control_seq(), 1);
    assert_eq!(history.endpoint.execution_count(), 0);
    assert_eq!(history.endpoint.payload(), b"initial");
}

#[test]
fn replay_plan_reports_actual_registration_without_verifying_or_executing_history() {
    let history = original_history(
        Roster::Complete,
        OriginalPolicy::Satisfied,
        ORIGINAL_EVIDENCE,
    );
    let original = history.broker.inspect();
    for with_receipt in [false, true] {
        let source = ObservedReplayRecord::new(
            history.anchor.policy.action.spec().scope,
            history.anchor.clone(),
            history.archive.clone(),
            with_receipt.then(|| history.receipt.clone()),
            InterventionScope::new(BRANCH_ID, false, true, true, &[9, 7, 3])
                .expect("actual host registration"),
        )
        .expect("bounded original source");
        let retained_bytes = source.retained_bytes();
        let mut tower = tower(Some(source), with_receipt, TowerLimits::default(), 2);
        let response = tower.dispatch(&access(), plan_request());
        assert_eq!(response.envelope.status, ResponseStatus::Ok);
        assert!(response.envelope.retry_safe);
        let (plan, basis) = plan(&response);
        assert_eq!(basis, branch_basis(0));
        assert_eq!(plan.branch_id, BRANCH_ID);
        assert_eq!(plan.branch_policy_epoch, BRANCH_POLICY_EPOCH);
        assert_eq!(plan.source_address, source_address(with_receipt));
        assert_eq!(plan.source_basis, source_basis());
        let (metadata, metadata_basis) = known(&plan.metadata);
        assert_eq!(metadata_basis, source_basis());
        assert_eq!(metadata.profile, ReplayProfile::ObservedPolicyExperiment);
        assert_eq!(
            metadata.native_scope,
            history.anchor.policy.action.spec().scope
        );
        assert_eq!(metadata.native_attempt, 1);
        assert_eq!(metadata.native_round, 101);
        assert_eq!(
            metadata.native_expected_control_sequence,
            history.anchor.policy.expected_control_sequence
        );
        assert_eq!(
            metadata.native_receipt_control_sequence,
            with_receipt.then_some(history.receipt.policy.control.sequence)
        );
        assert_eq!(metadata.has_receipt, with_receipt);
        assert_eq!(metadata.retained_bytes, retained_bytes);
        assert_eq!(metadata.registration.id, BRANCH_ID);
        assert!(!metadata.registration.allow_payload);
        assert!(metadata.registration.allow_target);
        assert!(metadata.registration.allow_units);
        assert_eq!(
            metadata.registration.keys,
            [3, 7, 9],
            "inspection reports every registered key, including unobserved keys"
        );
        assert_unverified_plan(metadata);
        assert_eq!(*known(&response.envelope.realized_cost).0, 1);
        assert_eq!(tower.remaining_read_units(), 1);
        assert_eq!(tower.journal_control_seq(), 0);

        let json = parse(&response.to_json().expect("complete plan JSON"));
        let wire = plan_body(&json);
        assert_eq!(number(wire, "branch_id"), BRANCH_ID);
        assert_eq!(number(wire, "branch_policy_epoch"), BRANCH_POLICY_EPOCH);
        assert_eq!(
            text(wire, "source_address"),
            source_address(with_receipt).canonical_string()
        );
        let source = field(wire, "source_basis");
        assert_eq!(number(source, "value"), SOURCE_CUT);
        assert_eq!(
            number(field(field(source, "basis"), "identity"), "policy_epoch"),
            0
        );
        let metadata = field(wire, "metadata");
        assert_eq!(text(metadata, "state"), "Known");
        assert_eq!(
            number(field(field(metadata, "basis"), "identity"), "control_seq"),
            SOURCE_CUT
        );
        let metadata = field(metadata, "value");
        assert_eq!(field(metadata, "has_receipt").as_bool(), Some(with_receipt));
        assert_eq!(number(metadata, "retained_bytes"), retained_bytes as u64);
        let registration = field(metadata, "registration");
        assert_eq!(field(registration, "allow_payload").as_bool(), Some(false));
        assert_eq!(field(registration, "allow_target").as_bool(), Some(true));
        assert_eq!(field(registration, "allow_units").as_bool(), Some(true));
        assert_eq!(
            field(registration, "keys")
                .as_array()
                .expect("all registered keys")
                .iter()
                .map(|key| key.as_u64().expect("native key"))
                .collect::<Vec<_>>(),
            [3, 7, 9]
        );
        assert_eq!(text(wire, "supported_semantics"), "decision_replay");
        assert_eq!(text(wire, "retention"), "in_memory");
        assert_eq!(number(wire, "logical_replay_cost"), 1);
        assert_eq!(
            number(field(wire, "bounds"), "max_interventions"),
            MAX_INTERVENTIONS as u64
        );
        let requirements = field(wire, "requirements");
        assert_eq!(field(requirements, "operator").as_bool(), Some(true));
        assert_eq!(text(requirements, "purpose"), "experiment");
        assert_eq!(text(requirements, "idempotency_key"), "exact_request");
        for (_, property) in field(metadata, "properties")
            .as_object()
            .expect("per-property uncertainty")
        {
            assert_eq!(text(property, "state"), "Unknown");
            assert!(property.get("value").is_none());
        }
    }
    assert_eq!(history.broker.inspect(), original);
    assert_eq!(history.endpoint.execution_count(), 0);
    assert_eq!(history.endpoint.payload(), b"initial");
}

#[test]
fn plan_inspection_needs_no_replay_purpose_and_withholds_partial_scope_metadata() {
    let history = original_history(
        Roster::Complete,
        OriginalPolicy::Satisfied,
        ORIGINAL_EVIDENCE,
    );
    let original = history.broker.inspect();
    let mut tower = tower(
        Some(record(&history, false)),
        false,
        TowerLimits::default(),
        5,
    );
    let nonoperator = TowerAccess::new(
        CallerScope::full_tenant_access(TENANT),
        false,
        Vec::new(),
        0,
    );
    let inspected = tower.dispatch(&nonoperator, plan_request());
    assert_unverified_plan(known(&plan(&inspected).0.metadata).0);
    for accounting in [
        &inspected.envelope.budgets_remaining,
        &inspected.envelope.realized_cost,
    ] {
        assert_eq!(accounting, &Knowledge::withheld(AuthorityKind::Verify));
    }
    assert_eq!(tower.remaining_read_units(), 4);
    let refused = tower.dispatch(
        &nonoperator,
        replay_request("requires-operator", Vec::new()),
    );
    assert!(matches!(refused.result, Knowledge::Withheld { .. }));
    assert_eq!(tower.remaining_read_units(), 4);

    let no_experiment = TowerAccess::new(
        CallerScope::full_tenant_access(TENANT),
        true,
        vec![PurposeTag::Capture],
        0,
    );
    let inspected = tower.dispatch(&no_experiment, plan_request());
    assert_unverified_plan(known(&plan(&inspected).0.metadata).0);
    assert_eq!(*known(&inspected.envelope.realized_cost).0, 1);
    let refused = tower.dispatch(
        &no_experiment,
        replay_request("requires-experiment", Vec::new()),
    );
    assert_eq!(
        refused.envelope.reason_code,
        Some(ReasonCode::PurposeMismatch)
    );
    assert_eq!(tower.remaining_read_units(), 3);

    let mut partial = CallerScope::full_tenant_access(TENANT);
    partial.allowed_planes.remove(&ResolverPlane::EvidenceStore);
    partial
        .allowed_planes
        .remove(&ResolverPlane::DerivedProjections);
    partial
        .granted_authorities
        .remove(&AuthorityKind::ExactRead);
    partial.granted_authorities.remove(&AuthorityKind::Judge);
    let partial = TowerAccess::new(partial, false, Vec::new(), 0);
    let inspected = tower.dispatch(&partial, plan_request());
    let (inspected_plan, basis) = plan(&inspected);
    assert_eq!(basis, branch_basis(0));
    assert_eq!(inspected_plan.source_address, source_address(false));
    assert_eq!(inspected_plan.source_basis, source_basis());
    assert_eq!(
        inspected_plan.metadata,
        Knowledge::withheld(AuthorityKind::ExactRead)
    );
    assert_eq!(
        inspected.envelope.realized_cost,
        Knowledge::withheld(AuthorityKind::Verify)
    );
    let encoded = inspected.to_json().expect("redacted partial plan");
    let json = parse(&encoded);
    let metadata = field(plan_body(&json), "metadata");
    assert_eq!(text(metadata, "state"), "Withheld");
    assert!(metadata.get("value").is_none());
    assert!(!encoded.contains("\"native_scope\""));
    assert!(!encoded.contains("\"registration\""));
    assert_eq!(tower.remaining_read_units(), 2);
    assert_eq!(tower.journal_control_seq(), 0);
    assert!(matches!(
        tower
            .dispatch(
                &partial,
                replay_request("partial-cannot-execute", Vec::new())
            )
            .result,
        Knowledge::Withheld { .. }
    ));
    assert_eq!(tower.remaining_read_units(), 2);

    let replayed = tower.dispatch(&access(), replay_request("authorized-twin", Vec::new()));
    assert!(outcome(&replayed).0.rehearsal);
    assert_eq!(tower.remaining_read_units(), 1);
    assert_eq!(tower.journal_control_seq(), 1);
    assert_eq!(history.broker.inspect(), original);
    assert_eq!(history.endpoint.execution_count(), 0);
}

#[test]
fn replay_plan_withholds_source_families_before_record_lookup() {
    let history = original_history(
        Roster::Complete,
        OriginalPolicy::Satisfied,
        ORIGINAL_EVIDENCE,
    );
    let mut missing_source_plane = CallerScope::full_tenant_access(TENANT);
    missing_source_plane
        .allowed_planes
        .remove(&ResolverPlane::ControlLedger);
    let mut missing_source_authority = CallerScope::full_tenant_access(TENANT);
    missing_source_authority
        .granted_authorities
        .remove(&AuthorityKind::Verify);
    let mut missing_subject_plane = CallerScope::full_tenant_access(TENANT);
    missing_subject_plane
        .allowed_planes
        .remove(&ResolverPlane::ExperimentStore);
    let mut missing_subject_authority = CallerScope::full_tenant_access(TENANT);
    missing_subject_authority
        .granted_authorities
        .remove(&AuthorityKind::Probe);
    let denied_scopes = [
        CallerScope::full_tenant_access("another-tenant"),
        missing_source_plane,
        missing_source_authority,
        missing_subject_plane,
        missing_subject_authority,
    ];
    for with_receipt in [false, true] {
        for scope in &denied_scopes {
            let denied = TowerAccess::new(scope.clone(), true, Vec::new(), 0);
            let mut present = tower(
                Some(record(&history, with_receipt)),
                with_receipt,
                TowerLimits::default(),
                3,
            );
            let mut absent = tower(None, with_receipt, TowerLimits::default(), 9);
            let refused = present.dispatch(&denied, plan_request());
            let missing = absent.dispatch(&denied, plan_request());
            assert_eq!(refused, missing);
            assert_eq!(refused.to_json(), missing.to_json());
            assert_eq!(refused.result, Knowledge::withheld(AuthorityKind::Verify));
            assert_eq!(
                refused.envelope.reason_code,
                Some(ReasonCode::DeniedByPolicy)
            );
            let json = parse(&refused.to_json().expect("uniform refusal"));
            assert!(field(&json, "result").get("value").is_none());
            assert_eq!(present.remaining_read_units(), 3);
            assert_eq!(absent.remaining_read_units(), 9);
            assert_eq!(present.journal_control_seq(), 0);
            assert_eq!(absent.journal_control_seq(), 0);
            let accepted = present.dispatch(&access(), plan_request());
            assert_eq!(
                plan(&accepted).0.source_address,
                source_address(with_receipt)
            );
            assert_unverified_plan(known(&plan(&accepted).0.metadata).0);
            assert_eq!(present.remaining_read_units(), 2);
            assert_eq!(present.journal_control_seq(), 0);
        }
    }
    assert_eq!(history.endpoint.execution_count(), 0);
}

#[test]
fn installed_tampered_history_has_inspectable_metadata_but_fails_actual_replay() {
    let history = original_history(
        Roster::Complete,
        OriginalPolicy::Satisfied,
        ORIGINAL_EVIDENCE,
    );
    let original = history.broker.inspect();
    let mut tampered = history.archive.clone();
    tampered.inputs = Rc::new(native_fixture::inputs(
        &history.anchor.policy.action,
        b"spliced final paragraph after the original commitments",
    ));
    assert!(
        tampered
            .verify_receipt(&history.anchor, &history.receipt)
            .is_err(),
        "causal negative control uses the actual native replay backend"
    );
    let source = ObservedReplayRecord::new(
        history.anchor.policy.action.spec().scope,
        history.anchor.clone(),
        tampered,
        Some(history.receipt.clone()),
        registration(),
    )
    .expect("bounded retained data are not verified at installation");
    let mut tower = tower(Some(source), true, TowerLimits::default(), 4);
    let inspected = tower.dispatch(&access(), plan_request());
    let (inspected_plan, basis) = plan(&inspected);
    assert_eq!(basis, branch_basis(0));
    assert_eq!(inspected_plan.source_basis, source_basis());
    assert_unverified_plan(known(&inspected_plan.metadata).0);
    assert_eq!(tower.remaining_read_units(), 3);
    assert_eq!(tower.journal_control_seq(), 0);
    let request = replay_request("real-verification-is-required", Vec::new());
    let failed = tower.dispatch(&access(), request.clone());
    assert_eq!(
        failed.envelope.reason_code,
        Some(ReasonCode::IntegrityFailure)
    );
    assert_eq!(*known(&failed.envelope.realized_cost).0, 1);
    assert_eq!(tower.remaining_read_units(), 2);
    assert_eq!(tower.journal_control_seq(), 0);
    assert_eq!(tower.dispatch(&access(), request), failed);
    assert_eq!(tower.remaining_read_units(), 2);
    let after_failure = tower.dispatch(&access(), plan_request());
    assert_eq!(plan(&after_failure).0, inspected_plan);
    assert_unverified_plan(known(&plan(&after_failure).0.metadata).0);
    assert_eq!(tower.remaining_read_units(), 1);
    assert_eq!(tower.journal_control_seq(), 0);

    let mut permitted = self::tower(
        Some(record(&history, true)),
        true,
        TowerLimits::default(),
        2,
    );
    let positive_plan = permitted.dispatch(&access(), plan_request());
    assert_unverified_plan(known(&plan(&positive_plan).0.metadata).0);
    let accepted = permitted.dispatch(
        &access(),
        replay_request("real-verification-is-required", Vec::new()),
    );
    assert!(outcome(&accepted).0.rehearsal);
    assert_eq!(permitted.remaining_read_units(), 0);
    assert_eq!(permitted.journal_control_seq(), 1);
    assert_eq!(history.broker.inspect(), original);
    assert_eq!(history.endpoint.execution_count(), 0);
}

#[test]
fn replay_plan_reads_do_not_occupy_or_overwrite_the_replay_idempotency_cache() {
    let history = original_history(
        Roster::Complete,
        OriginalPolicy::Satisfied,
        ORIGINAL_EVIDENCE,
    );
    let initial_units = MAX_REPLAY_OPERATIONS as u64 + 10;
    let mut tower = tower(
        Some(record(&history, true)),
        true,
        TowerLimits::default(),
        initial_units,
    );
    let mut wrong_arguments = plan_request();
    wrong_arguments.arguments = ReadArguments::Replay {
        interventions: vec![Intervention::Payload(b"not-a-plan-argument".to_vec())],
    };
    let refused = tower.dispatch(&access(), wrong_arguments);
    assert_eq!(
        refused.envelope.reason_code,
        Some(ReasonCode::IntegrityFailure)
    );
    assert_eq!(*known(&refused.envelope.realized_cost).0, 0);
    assert_eq!(tower.remaining_read_units(), initial_units);

    for generation in [BRANCH_GENERATION - 1, BRANCH_GENERATION + 1] {
        let mut request = plan_request();
        request.address.generation = Some(generation);
        let response = tower.dispatch(&access(), request);
        assert_eq!(
            response.envelope.reason_code,
            Some(ReasonCode::StaleGeneration)
        );
        if generation < BRANCH_GENERATION {
            assert_eq!(
                response.result,
                Knowledge::stale(branch_basis(0), BRANCH_GENERATION)
            );
        } else {
            assert_eq!(
                response.result,
                Knowledge::unknown(UnknownReason::IncompatibleGeneration)
            );
        }
        assert_eq!(*known(&response.envelope.realized_cost).0, 1);
        assert_eq!(tower.journal_control_seq(), 0);
    }
    let mut absent = plan_request();
    absent.address.id = "missing-replay-plan".to_owned();
    let missing = tower.dispatch(&access(), absent);
    assert!(matches!(missing.result, Knowledge::Unknown { .. }));
    assert_eq!(*known(&missing.envelope.realized_cost).0, 1);
    assert_eq!(tower.remaining_read_units(), initial_units - 3);

    for index in 0..=MAX_REPLAY_OPERATIONS {
        let mut request = plan_request();
        request.idempotency_key = Some(format!("plan-key-{index}"));
        let response = tower.dispatch(&access(), request);
        assert_unverified_plan(known(&plan(&response).0.metadata).0);
        assert_eq!(*known(&response.envelope.realized_cost).0, 1);
        assert_eq!(tower.journal_control_seq(), 0);
    }
    let before_replay = tower.remaining_read_units();
    assert_eq!(before_replay, 6);
    let request = replay_request("plan-key-0", Vec::new());
    let accepted = tower.dispatch(&access(), request.clone());
    assert!(outcome(&accepted).0.rehearsal);
    assert_eq!(tower.remaining_read_units(), before_replay - 1);
    assert_eq!(tower.journal_control_seq(), 1);
    assert_eq!(tower.dispatch(&access(), request.clone()), accepted);
    assert_eq!(tower.remaining_read_units(), before_replay - 1);

    let mut inspection = plan_request();
    inspection.idempotency_key = Some("plan-key-0".to_owned());
    let after_replay = tower.dispatch(&access(), inspection);
    let (inspected, basis) = plan(&after_replay);
    assert_eq!(basis, branch_basis(0));
    assert_eq!(inspected.source_basis, source_basis());
    let (metadata, metadata_basis) = known(&inspected.metadata);
    assert_eq!(metadata_basis, source_basis());
    assert_unverified_plan(metadata);
    assert_eq!(tower.remaining_read_units(), before_replay - 2);
    assert_eq!(tower.journal_control_seq(), 1);
    assert_eq!(
        tower.dispatch(&access(), request),
        accepted,
        "a plan read with the same key cannot replace the exact replay response"
    );
    assert_eq!(tower.remaining_read_units(), before_replay - 2);
    assert_eq!(tower.journal_control_seq(), 1);
    assert_eq!(history.endpoint.execution_count(), 0);
    assert_eq!(history.endpoint.payload(), b"initial");
}
