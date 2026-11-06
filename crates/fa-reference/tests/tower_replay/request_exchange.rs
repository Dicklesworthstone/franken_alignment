//! Real paired replay through independently written request documents.

use super::*;
use fa_reference::tower::router::{RoutedResponse, TowerRouter};

fn document(verb: &str, arguments: &str, key: &str) -> String {
    format!(
        r#"{{"schema_version":"fa.tower_request/1","operation_id":"wire-replay","address":"{}","verb":"{verb}","arguments":{arguments},"idempotency_key":{key}}}"#,
        artifact_address().canonical_string()
    )
}

#[test]
fn json_plan_and_paired_repair_keep_original_branch_custody_and_exact_retries() {
    let history = original_history(
        Roster::Complete,
        OriginalPolicy::Violated,
        ORIGINAL_EVIDENCE,
    );
    let original = history.broker.inspect();
    let mut tower = tower(
        Some(record(&history, true)),
        true,
        TowerLimits::default(),
        4,
    );
    let inspected = tower.exchange(
        &access(),
        document("replay-plan", "null", "null").as_bytes(),
    );
    let (inspected, _) = plan(&inspected);
    assert_unverified_plan(known(&inspected.metadata).0);
    assert_eq!(tower.remaining_read_units(), 3);
    assert_eq!(tower.journal_control_seq(), 0);

    let bytes = document(
        "replay",
        r#"{"interventions":[{"kind":"key","key":"7","expected_hex":"626164","replacement_hex":"6f6b"}]}"#,
        r#""wire-repair""#,
    );
    let forged = bytes.replace(
        r#""arguments":{"interventions""#,
        r#""arguments":{"experiment_context":{"branch":"71"},"interventions""#,
    );
    assert_ne!(forged, bytes);
    assert_eq!(
        tower
            .exchange(&access(), forged.as_bytes())
            .envelope
            .reason_code,
        Some(ReasonCode::IntegrityFailure)
    );
    let no_experiment = TowerAccess::new(
        CallerScope::full_tenant_access(TENANT),
        true,
        vec![PurposeTag::Permit],
        10,
    );
    assert_eq!(
        tower
            .exchange(&no_experiment, bytes.as_bytes())
            .envelope
            .reason_code,
        Some(ReasonCode::PurposeMismatch)
    );
    assert_eq!(tower.remaining_read_units(), 3);
    assert_eq!(tower.journal_control_seq(), 0);

    let response = tower.exchange(&access(), bytes.as_bytes());
    let (result, basis) = outcome(&response);
    assert_eq!(basis, branch_basis(1));
    assert_eq!(result.source.counterfactual.baseline(), Truth::Violated);
    assert_eq!(
        result.source.counterfactual.counterfactual(),
        Truth::Satisfied
    );
    assert_eq!(
        result.source.counterfactual.next_requirement(),
        NextRequirement::FreshIndependentReview
    );
    assert_evidence_limits(&result.source, true);
    assert_eq!(tower.remaining_read_units(), 2);
    assert_eq!(tower.journal_control_seq(), 1);
    assert_eq!(tower.exchange(&access(), bytes.as_bytes()), response);
    assert_eq!(
        tower.exchange(&access(), format!(" \n{bytes}\t").as_bytes()),
        response
    );
    let changed = bytes.replace("6f6b", "6e6f");
    assert_eq!(
        tower
            .exchange(&access(), changed.as_bytes())
            .envelope
            .reason_code,
        Some(ReasonCode::IntegrityFailure)
    );
    assert_eq!(tower.remaining_read_units(), 2);
    assert_eq!(tower.journal_control_seq(), 1);

    // The same capability-free branch and retry cache survive router ownership.
    let domain = tower.domain_address().canonical_string();
    let mut router = TowerRouter::new(tower);
    let RoutedResponse::Reference(retry) = router.exchange(&access(), bytes.as_bytes()) else {
        panic!("branch requests must retain the original reference route");
    };
    assert_eq!(retry, response);
    let tail = format!(
        r#"{{"schema_version":"fa.tower_request/1","operation_id":"wire-tail","address":"{domain}","verb":"tail","arguments":{{"since_seq":"0","max_events":"64"}},"idempotency_key":null}}"#
    );
    let RoutedResponse::Reference(tail) = router.exchange(&access(), tail.as_bytes()) else {
        panic!("branch journal must remain in the original reference route");
    };
    let (TowerValue::Tail(batch), _) = known(&tail.result) else {
        panic!("actual branch event page");
    };
    assert_eq!(batch.events.len(), 1);
    assert!(matches!(
        batch.events[0].kind,
        JournalEventKind::BranchOutcome { .. }
    ));
    assert_eq!(*known(&tail.envelope.budgets_remaining).0, 1);
    assert_eq!(history.broker.inspect(), original);
    assert_eq!(history.endpoint.execution_count(), 0);
    assert_eq!(history.endpoint.payload(), b"initial");
}

#[test]
fn syntactically_valid_unregistered_edits_keep_original_spent_work_and_failed_retry() {
    let history = original_history(
        Roster::Complete,
        OriginalPolicy::Satisfied,
        ORIGINAL_EVIDENCE,
    );
    let mut wire = tower(
        Some(record(&history, true)),
        true,
        TowerLimits::default(),
        2,
    );
    let mut typed = tower(
        Some(record(&history, true)),
        true,
        TowerLimits::default(),
        2,
    );
    // This registration permits payload/key edits but explicitly forbids units.
    // The parser must retain a valid trial for the original experiment to judge.
    let bytes = document(
        "replay",
        r#"{"interventions":[{"kind":"units","units":"1"}]}"#,
        r#""unregistered-edit""#,
    );
    let mut request = replay_request("unregistered-edit", vec![Intervention::Units(1)]);
    request.operation_id = "wire-replay".to_owned();
    let response = wire.exchange(&access(), bytes.as_bytes());
    assert_eq!(response, typed.dispatch(&access(), request));
    assert_eq!(
        response.envelope.reason_code,
        Some(ReasonCode::IntegrityFailure)
    );
    assert_eq!(wire.remaining_read_units(), 1);
    assert_eq!(wire.journal_control_seq(), 0);
    assert_eq!(wire.exchange(&access(), bytes.as_bytes()), response);
    assert_eq!(wire.remaining_read_units(), 1);

    let malformed = bytes.replace(r#""units":"1""#, r#""units":1"#);
    assert_ne!(malformed, bytes);
    let rejected = wire.exchange(&access(), malformed.as_bytes());
    assert_eq!(rejected.envelope.operation_id, "<invalid-request>");
    assert_eq!(wire.remaining_read_units(), 1);
    assert_eq!(wire.journal_control_seq(), 0);
    assert_eq!(history.endpoint.execution_count(), 0);
}
