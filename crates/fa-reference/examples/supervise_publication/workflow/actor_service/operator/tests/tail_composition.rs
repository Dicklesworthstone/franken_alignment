//! Original journal history composes with separate completed-review and Stop custody.

use super::*;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use fa_reference::action::consequence::delivery::persistent::requests::actor::MAX_REVIEW_EXPLANATION_EVENTS;
use fa_reference::tower::publication::request_address_for;

#[path = "../../../../../../tests/support/file_oversight.rs"]
mod native;

fn selection(queries: usize, events_per_page: usize) -> String {
    format!(
        r#"{{"queries":{queries},"events_per_page":{events_per_page},"disclosure":"redacted_journal_events"}}"#
    )
}

fn with_tail(profile: &str, selected: &str) -> String {
    profile.replacen('{', &format!("{{\"journal_tail\":{selected},"), 1)
}

fn fixture(queries: usize, events: usize, inspection: usize, explanation: usize) -> Fixture {
    let mut fixture = Fixture::configured(1, explanation, |config| {
        let clock_domain = config.profile.delivery.clock_domain;
        config.profile = native::profile();
        config.profile.delivery.clock_domain = clock_domain;
    });
    fixture.settings = Settings::decode(
        with_tail(
            &profile_json(
                &fixture.settings.socket,
                fixture.settings.operator,
                inspection,
                explanation,
            ),
            &selection(queries, events),
        )
        .as_bytes(),
    )
    .unwrap();
    fixture.settings.candidate_limit = 16;
    fixture.settings.connection_limit = 12;
    fixture
}

fn request(fixture: &Fixture, after_revision: u64) -> TowerRequest {
    let mut request = fixture.request(Verb::Tail);
    request.arguments = ReadArguments::JournalTail { after_revision };
    request
}

fn number(value: &Json) -> u64 {
    value.as_str().unwrap().parse().unwrap()
}

fn page(response: &Json) -> &Json {
    known(response, "journal-tail");
    field(response, &["result", "value", "knowledge", "value"])
}

fn remaining(service: &Service) -> usize {
    service
        .active
        .as_ref()
        .unwrap()
        .journal_tail
        .as_ref()
        .unwrap()
        .remaining_queries()
}

fn published(fixture: &mut Fixture) -> u64 {
    fixture.submit(7);
    let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
    let FileRequestDisposition::Admitted { attempt, .. } =
        host.request_status(7).unwrap().disposition
    else {
        panic!("original request was not admitted");
    };
    let action = host.request_action(7).unwrap().clone();
    let inputs = native::inputs(&action, b"private original helper evidence");
    native::review_existing(&mut host, attempt, 107, &inputs);
    let revision = host.revision();
    let automatic = host
        .authorize(revision, attempt, &inputs, native::snapshot())
        .unwrap();
    let revision = host.revision();
    let review = host
        .request_human_approval(revision, 1007, attempt, &inputs, ElapsedTick(1_030))
        .unwrap();
    let revision = host.revision();
    let human = fixture.human.approve(&mut host, revision, &review).unwrap();
    let revision = host.revision();
    host.dispatch(
        revision,
        &automatic,
        &human,
        &action,
        &inputs,
        native::snapshot(),
    )
    .unwrap();
    let revision = host.revision();
    let result = host
        .publish_checked(
            revision,
            attempt,
            Some(&inputs),
            native::snapshot(),
            ElapsedTick(1_001),
        )
        .unwrap();
    assert!(matches!(result.outcome, EndpointOutcome::Executed { .. }));
    attempt
}

#[test]
fn authenticated_reconnect_continues_the_original_review_and_publication_cut_without_new_work() {
    let mut fixture = fixture(1, 2, 1, 1);
    let selected = Settings::decode(
        profile_json(&fixture.settings.socket, fixture.settings.operator, 1, 1)
            .replacen(
                '{',
                &format!(
                    "{{\"review_explanation\":{{\"queries\":1,\"max_replay_events\":{MAX_REVIEW_EXPLANATION_EVENTS},\"disclosure\":\"policy_and_congress_trace\"}},"
                ),
                1,
            )
            .as_bytes(),
    )
    .unwrap();
    fixture.settings.review_explanation = selected.review_explanation;
    let attempt = published(&mut fixture);
    let original_revision = fixture.driver.supervisor().host().unwrap().revision();
    assert!(original_revision > 2);
    let queries = usize::try_from(original_revision).unwrap().div_ceil(2);
    fixture.settings.journal_tail.as_mut().unwrap().queries = queries;
    // Retain one transport exchange after the exhausted Tail and separately
    // granted Explain so their independent native counters remain observable.
    fixture.settings.candidate_limit = queries as u64 + 3;
    fixture.settings.connection_limit = queries as u64 + 3;
    fixture.settings.exchange_limit = queries as u64 + 3;
    let mut service = fixture.start();
    let first = fixture.query(&mut service, request(&fixture, 0));
    let first_page = page(&first);
    assert_eq!(
        number(field(
            &first,
            &["result", "value", "knowledge", "basis", "journal_revision"]
        )),
        original_revision
    );
    assert_eq!(number(first_page.get("offset").unwrap()), 0);
    assert_eq!(number(first_page.get("after_revision").unwrap()), 0);
    assert_eq!(remaining(&service), queries - 1);
    assert_eq!(
        service
            .active
            .as_ref()
            .unwrap()
            .journal_tail
            .as_ref()
            .unwrap()
            .events_per_page(),
        2
    );
    // Progress the actual owner between authenticated connections. The pending
    // cursor must continue its captured cut instead of following live progress.
    {
        let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(1_002)).unwrap();
    }
    let state = fixture.driver.supervisor().host().unwrap().inspect();
    let bytes = fixture.files();
    let mut events = first_page
        .get("events")
        .unwrap()
        .as_array()
        .unwrap()
        .to_vec();
    let mut current = first.clone();
    for _ in 1..queries {
        let cursor = number(page(&current).get("continuation").unwrap());
        let mut continuation = request(&fixture, 0);
        continuation.arguments = ReadArguments::JournalTailPage { cursor };
        current = fixture.query(&mut service, continuation);
        assert_eq!(
            field(&first, &["result", "value", "knowledge", "basis"]),
            field(&current, &["result", "value", "knowledge", "basis"])
        );
        let next_page = page(&current);
        assert_eq!(
            number(next_page.get("offset").unwrap()),
            events.len() as u64
        );
        let rows = next_page.get("events").unwrap().as_array().unwrap();
        assert_eq!(rows.len(), 2.min(original_revision as usize - events.len()));
        events.extend_from_slice(rows);
    }
    assert_eq!(
        number(page(&current).get("through_revision").unwrap()),
        original_revision
    );
    assert_eq!(
        page(&current)
            .get("enumeration_complete")
            .and_then(Json::as_bool),
        Some(true)
    );
    assert!(page(&current).get("continuation").unwrap().is_null());
    assert_eq!(events.len() as u64, original_revision);
    for (index, event) in events.iter().enumerate() {
        assert_eq!(number(event.get("revision").unwrap()), index as u64 + 1);
        assert_eq!(event.as_object().unwrap().len(), 5);
        if !event.get("attempt").unwrap().is_null() {
            assert_eq!(number(event.get("attempt").unwrap()), attempt);
        }
        if !event.get("request").unwrap().is_null() {
            assert_eq!(number(event.get("request").unwrap()), 7);
            assert_eq!(
                event.get("kind").and_then(Json::as_str),
                Some("submit_request")
            );
        }
    }
    for name in [
        "begin_review",
        "commit",
        "open_reveals",
        "reveal",
        "finish_review",
    ] {
        let event = events
            .iter()
            .find(|event| event.get("kind").and_then(Json::as_str) == Some(name))
            .unwrap();
        assert_eq!(number(event.get("round").unwrap()), 107);
    }
    let begin = events
        .iter()
        .find(|event| event.get("kind").and_then(Json::as_str) == Some("begin_review"))
        .unwrap();
    assert_eq!(number(begin.get("attempt").unwrap()), attempt);
    for name in [
        "submit_request",
        "request_human",
        "human_decision",
        "dispatch",
        "publish_checked",
    ] {
        assert!(
            events
                .iter()
                .any(|event| event.get("kind").and_then(Json::as_str) == Some(name))
        );
    }
    let human = events
        .iter()
        .find(|event| event.get("kind").and_then(Json::as_str) == Some("human_decision"))
        .unwrap();
    assert!(
        human.get("request").unwrap().is_null(),
        "human approval IDs are not actor request IDs"
    );
    let exhausted = fixture.query(&mut service, request(&fixture, original_revision));
    assert_eq!(
        field(&exhausted, &["result", "value", "error", "code"]).as_str(),
        Some("capacity")
    );
    let active = service.active.as_ref().unwrap();
    assert_eq!(remaining(&service), 0);
    assert_eq!(active.inspection.as_ref().unwrap().remaining_queries(), 1);
    assert_eq!(active.explanation.as_ref().unwrap().remaining_queries(), 1);
    assert_eq!(
        active
            .review_explanation
            .as_ref()
            .unwrap()
            .remaining_queries(),
        1
    );

    // Navigate using the original request and round learned from the captured
    // history. Completed-review disclosure has its own grant after Tail is spent.
    let registration = Address::parse(
        field(&first, &["result", "value", "registration"])
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let submission = events
        .iter()
        .find(|event| event.get("kind").and_then(Json::as_str) == Some("submit_request"))
        .unwrap();
    let original_request = number(submission.get("request").unwrap());
    assert_eq!(original_request, 7);
    let original_round = number(begin.get("round").unwrap());
    let finished = events
        .iter()
        .find(|event| event.get("kind").and_then(Json::as_str) == Some("finish_review"))
        .unwrap();
    assert_eq!(number(finished.get("round").unwrap()), original_round);
    let address =
        request_address_for(fixture.actor.scope, &registration, original_request).unwrap();
    let mut explain = TowerRequest::new("tail-discovered-review", address, Verb::Explain);
    explain.arguments = ReadArguments::ReviewExplanation {
        round: original_round,
    };
    let explained = fixture.query(&mut service, explain);
    known(&explained, "review-explanation");
    assert_eq!(
        number(field(&explained, &["result", "value", "round"])),
        original_round
    );
    let basis = field(&explained, &["result", "value", "knowledge", "basis"]);
    assert_eq!(number(basis.get("request").unwrap()), original_request);
    assert_eq!(number(basis.get("attempt").unwrap()), attempt);
    assert_eq!(number(basis.get("round").unwrap()), original_round);
    assert_eq!(
        number(basis.get("begin_revision").unwrap()),
        number(begin.get("revision").unwrap())
    );
    assert_eq!(
        number(basis.get("completed_revision").unwrap()),
        number(finished.get("revision").unwrap())
    );
    assert_eq!(
        number(basis.get("journal_revision").unwrap()),
        state.revision
    );
    assert_eq!(
        field(
            &explained,
            &[
                "result",
                "value",
                "knowledge",
                "value",
                "effect",
                "outcome",
                "state"
            ]
        )
        .as_str(),
        Some("known")
    );
    assert_eq!(
        field(
            &explained,
            &[
                "result",
                "value",
                "knowledge",
                "value",
                "effect",
                "outcome",
                "value",
                "outcome",
                "kind"
            ]
        )
        .as_str(),
        Some("executed")
    );
    assert_eq!(
        field(&explained, &["result", "value", "effect_authority"]).as_bool(),
        Some(false)
    );
    let active = service.active.as_ref().unwrap();
    assert_eq!(remaining(&service), 0);
    assert_eq!(
        active
            .review_explanation
            .as_ref()
            .unwrap()
            .remaining_queries(),
        0
    );
    assert_eq!(active.inspection.as_ref().unwrap().remaining_queries(), 1);
    assert_eq!(active.explanation.as_ref().unwrap().remaining_queries(), 1);
    assert_eq!(fixture.driver.supervisor().host().unwrap().inspect(), state);
    assert_eq!(fixture.files(), bytes);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    service.retire();
    assert!(!fixture.settings.socket.exists());
}

#[test]
fn tail_and_fence_keep_independent_custody_and_original_cuts_without_refunding_unknown() {
    use fa_reference::action::ActionState;
    use fa_reference::action::consequence::delivery::persistent::RecoveryReserve;
    use fa_reference::action::consequence::delivery::persistent::requests::actor::{
        FileFenceBasis, MAX_FENCE_REPLAY_EVENTS,
    };

    let mut fixture = fixture(3, 1, 0, 0);
    {
        let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        host.enable_recovery_reserve(revision, RecoveryReserve::terminal())
            .unwrap();
    }
    // Provision both capabilities on the same restricted operator endpoint.
    // Tail gets three reads; Fence retains its separate one-operation custody.
    let document = with_tail(
        &profile_json(&fixture.settings.socket, fixture.settings.operator, 0, 0),
        &selection(3, 1),
    )
    .replacen(
        '{',
        &format!(
            "{{\"fence\":{{\"max_replay_events\":{MAX_FENCE_REPLAY_EVENTS},\"authority\":\"stop_original_admission\"}},"
        ),
        1,
    );
    fixture.settings = Settings::decode(document.as_bytes()).unwrap();
    fixture.settings.candidate_limit = 16;
    fixture.settings.connection_limit = 12;

    // The same real congress, human approval and dispatch used by the Fence
    // fixture precede an actual missing-receipt reconciliation. This produces
    // charged Unknown without fabricating a publication or a nonexecution proof.
    fixture.submit(7);
    let uncertain = {
        let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
        let FileRequestDisposition::Admitted { attempt, .. } =
            host.request_status(7).unwrap().disposition
        else {
            panic!("original request was not admitted");
        };
        let action = host.request_action(7).unwrap().clone();
        let inputs = native::inputs(&action, b"original review evidence");
        native::review_existing(&mut host, attempt, 107, &inputs);
        let revision = host.revision();
        let automatic = host
            .authorize(revision, attempt, &inputs, native::snapshot())
            .unwrap();
        let revision = host.revision();
        let review = host
            .request_human_approval(revision, 1007, attempt, &inputs, ElapsedTick(1_030))
            .unwrap();
        let revision = host.revision();
        let human = fixture.human.approve(&mut host, revision, &review).unwrap();
        let revision = host.revision();
        host.dispatch(
            revision,
            &automatic,
            &human,
            &action,
            &inputs,
            native::snapshot(),
        )
        .unwrap();
        let revision = host.revision();
        host.reconcile(revision, attempt).unwrap();
        assert_eq!(
            host.inspect().control.ledger.stages[&attempt],
            ActionState::Unknown
        );
        attempt
    };
    fixture.submit(8);
    let reserved = match fixture
        .driver
        .supervisor()
        .host()
        .unwrap()
        .request_status(8)
        .unwrap()
        .disposition
    {
        FileRequestDisposition::Admitted { attempt, .. } => attempt,
        other => panic!("second request was not admitted: {other:?}"),
    };
    let before = fixture.driver.supervisor().host().unwrap().inspect();
    assert_eq!(
        before.control.ledger.stages[&uncertain],
        ActionState::Unknown
    );
    assert_eq!(
        (
            before.control.ledger.charged,
            before.control.ledger.reserved
        ),
        (8, 8)
    );
    assert_eq!(before.executions, 0);
    assert!(before.stop.is_none());
    let original_files = fixture.files();
    let mut service = fixture.start();
    let fence_remaining = |service: &Service| {
        service
            .active
            .as_ref()
            .unwrap()
            .fence
            .as_ref()
            .unwrap()
            .remaining_operations()
    };
    assert_eq!((remaining(&service), fence_remaining(&service)), (3, 1));

    // The final two native records are the real unresolved reconciliation and
    // the second admission. The first page freezes both before Fence runs.
    let first = fixture.query(&mut service, request(&fixture, before.revision - 2));
    let first_page = page(&first);
    let original_basis = field(&first, &["result", "value", "knowledge", "basis"]);
    assert_eq!(
        number(original_basis.get("journal_revision").unwrap()),
        before.revision
    );
    let first_rows = first_page.get("events").unwrap().as_array().unwrap();
    assert_eq!(first_rows.len(), 1);
    assert_eq!(
        first_rows[0].get("kind").and_then(Json::as_str),
        Some("reconcile")
    );
    assert_eq!(number(first_rows[0].get("attempt").unwrap()), uncertain);
    assert_eq!(
        number(first_rows[0].get("revision").unwrap()),
        before.revision - 1
    );
    assert_eq!(
        first_page
            .get("enumeration_complete")
            .and_then(Json::as_bool),
        Some(false)
    );
    let cursor = number(first_page.get("continuation").unwrap());
    assert_eq!((remaining(&service), fence_remaining(&service)), (2, 1));
    assert_eq!(fixture.files(), original_files);
    assert_eq!(
        fixture.driver.supervisor().host().unwrap().inspect(),
        before
    );

    let operation = 1901;
    let mut fence = fixture.request(Verb::Fence);
    fence.idempotency_key = Some(operation.to_string());
    fence.arguments = ReadArguments::Fence {
        operation,
        expected: FileFenceBasis {
            scope: fixture.actor.scope,
            journal_revision: before.revision,
            control_sequence: before.control.sequence,
            authority_epoch: before.control.ledger.epoch,
        },
    };
    let fenced = fixture.query(&mut service, fence.clone());
    known(&fenced, "fence");
    assert_eq!(
        field(&fenced, &["result", "value", "scope"]).as_str(),
        Some("original_domain_admission")
    );
    assert_eq!(
        field(&fenced, &["result", "value", "drain_attempted"]).as_bool(),
        Some(false)
    );
    assert_eq!(
        field(&fenced, &["result", "value", "publication_attempted"]).as_bool(),
        Some(false)
    );
    let progress = field(
        &fenced,
        &["result", "value", "knowledge", "value", "progress"],
    );
    assert_eq!(
        progress.get("endpoint_fenced").and_then(Json::as_bool),
        Some(false)
    );
    assert_eq!(progress.get("drained").and_then(Json::as_bool), Some(false));
    assert_eq!(number(progress.get("charged_units").unwrap()), 8);
    assert_eq!(number(progress.get("reserved_units").unwrap()), 0);
    let unresolved = progress.get("unresolved").unwrap().as_array().unwrap();
    assert_eq!(unresolved.len(), 1);
    assert_eq!(number(&unresolved[0]), uncertain);
    let stop = field(&fenced, &["result", "value", "knowledge", "value", "stop"]);
    assert_eq!(number(stop.get("operation").unwrap()), operation);
    assert_eq!(number(stop.get("refunded_units").unwrap()), 8);
    let cancelled = stop.get("cancelled").unwrap().as_array().unwrap();
    assert_eq!(cancelled.len(), 1);
    assert_eq!(number(&cancelled[0]), reserved);
    for unknown in ["actor_halted", "fleet_fenced"] {
        assert_eq!(
            field(
                &fenced,
                &["result", "value", "knowledge", "value", unknown, "state"]
            )
            .as_str(),
            Some("unknown")
        );
    }
    let after = fixture.driver.supervisor().host().unwrap().inspect();
    assert_eq!(after.revision, before.revision + 1);
    assert!(after.stop.is_some());
    assert_eq!(
        after.control.ledger.stages[&uncertain],
        ActionState::Unknown
    );
    assert_eq!(
        after.control.ledger.stages[&reserved],
        ActionState::Cancelled
    );
    assert_eq!(
        (after.control.ledger.charged, after.control.ledger.reserved),
        (8, 0)
    );
    assert_eq!(after.control.ledger.epoch, before.control.ledger.epoch + 1);
    assert_eq!(after.control.ledger.elapsed, before.control.ledger.elapsed);
    assert_eq!(after.executions, 0);
    assert_eq!(after.payload, before.payload);
    assert_eq!((remaining(&service), fence_remaining(&service)), (2, 0));
    let stopped_files = fixture.files();
    assert_ne!(stopped_files, original_files);

    let mut continuation = request(&fixture, 0);
    continuation.arguments = ReadArguments::JournalTailPage { cursor };
    let continued = fixture.query(&mut service, continuation);
    assert_eq!(
        field(&continued, &["result", "value", "knowledge", "basis"]),
        original_basis
    );
    let continued_page = page(&continued);
    assert_eq!(number(continued_page.get("offset").unwrap()), 1);
    assert_eq!(
        number(continued_page.get("through_revision").unwrap()),
        before.revision
    );
    assert_eq!(
        continued_page
            .get("enumeration_complete")
            .and_then(Json::as_bool),
        Some(true)
    );
    assert!(continued_page.get("continuation").unwrap().is_null());
    let rows = continued_page.get("events").unwrap().as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].get("kind").and_then(Json::as_str),
        Some("submit_request")
    );
    assert_eq!(number(rows[0].get("request").unwrap()), 8);
    assert_eq!((remaining(&service), fence_remaining(&service)), (1, 0));

    let fresh = fixture.query(&mut service, request(&fixture, before.revision));
    let fresh_page = page(&fresh);
    assert_eq!(
        number(field(
            &fresh,
            &["result", "value", "knowledge", "basis", "journal_revision"]
        )),
        after.revision
    );
    assert_eq!(
        fresh_page
            .get("enumeration_complete")
            .and_then(Json::as_bool),
        Some(true)
    );
    let rows = fresh_page.get("events").unwrap().as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("kind").and_then(Json::as_str), Some("stop"));
    assert_eq!(number(rows[0].get("revision").unwrap()), after.revision);
    for link in ["attempt", "request", "round"] {
        assert!(rows[0].get(link).unwrap().is_null());
    }
    assert_eq!(
        field(&fresh, &["result", "value", "current_eligibility"]).as_str(),
        Some("not_observed")
    );
    assert_eq!((remaining(&service), fence_remaining(&service)), (0, 0));

    let exhausted = fixture.query(&mut service, request(&fixture, after.revision));
    assert_eq!(
        field(&exhausted, &["result", "value", "error", "code"]).as_str(),
        Some("capacity")
    );
    // Exhausting Tail neither erases the Stop receipt nor renews either grant.
    assert_eq!(fixture.query(&mut service, fence), fenced);
    assert_eq!((remaining(&service), fence_remaining(&service)), (0, 0));
    assert_eq!(fixture.driver.supervisor().host().unwrap().inspect(), after);
    assert_eq!(fixture.files(), stopped_files);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    service.retire();
    assert!(service.active.is_none());
    assert!(!fixture.settings.socket.exists());
}
