//! Validate genuine native pages before committing any consumer progress.
use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorPolicyReplayGrant;
use fa_reference::tower::policy_replay_client::{PolicyReplayCapture, PolicyReplayClientError};

struct NativePages {
    facade: PublicationTower,
    supervisor: Supervisor,
    grant: FileSupervisorPolicyReplayGrant,
    root: Directory,
}

impl NativePages {
    fn new(denials: u64, page_queries: usize) -> Self {
        let root = Directory::new();
        let (actor, supervisor, _keys) = corpus(&root, denials);
        let (port, grant) = supervisor
            .policy_replay_port_with_pages(1, limits(), page_queries)
            .unwrap();
        let mut facade = tower(actor, 16 * 1024);
        facade.set_policy_replay(Some(port)).unwrap();
        Self {
            facade,
            supervisor,
            grant,
            root,
        }
    }

    fn issue(&mut self, request: TowerRequest) -> (TowerRequest, Vec<u8>, Option<u64>) {
        let response = self
            .facade
            .exchange(&access(), encode_request(&request).unwrap().as_bytes());
        let next = known(&response).continuation().map(|cursor| cursor.id());
        (request, response.to_json().unwrap().into_bytes(), next)
    }

    fn sequence(&mut self, policy: Policy) -> Vec<(TowerRequest, Vec<u8>)> {
        let (request, response, mut next) = self.issue(first(71, policy));
        let mut sequence = vec![(request, response)];
        while let Some(cursor) = next {
            let (request, response, following) = self.issue(continued(71, cursor));
            sequence.push((request, response));
            next = following;
        }
        sequence
    }
}

fn new_capture(initial: &TowerRequest, pages: usize) -> PolicyReplayCapture {
    PolicyReplayCapture::new(profile().delivery.scope, initial, pages).unwrap()
}

fn decode(bytes: &[u8]) -> Json {
    strict_json::parse(bytes, JsonLimits::default()).unwrap()
}

fn path_mut<'a>(root: &'a mut Json, path: &[&str]) -> &'a mut Json {
    let mut current = root;
    for key in path {
        let Json::Object(fields) = current else {
            panic!("object required at {key}")
        };
        current = fields
            .get_mut(*key)
            .unwrap_or_else(|| panic!("missing {key}"));
    }
    current
}

fn body_mut(root: &mut Json) -> &mut Json {
    path_mut(root, &["result", "value", "knowledge", "value"])
}

fn array_mut(value: &mut Json) -> &mut Vec<Json> {
    let Json::Array(values) = value else {
        panic!("array required")
    };
    values
}

fn rows_mut(root: &mut Json) -> &mut Vec<Json> {
    array_mut(path_mut(body_mut(root), &["cases"]))
}

fn set(root: &mut Json, path: &[&str], value: Json) {
    *path_mut(root, path) = value;
}

fn text(value: impl ToString) -> Json {
    Json::String(value.to_string())
}

// This serializer only carries parsed genuine response trees back onto the
// wire after an isolated corruption. It knows no policy/page validation rules.
fn encoded(value: &Json) -> Vec<u8> {
    fn quote(value: &str) -> String {
        let mut out = String::from("\"");
        for ch in value.chars() {
            match ch {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                ch if ch <= '\u{1f}' => out.push_str(&format!("\\u{:04x}", ch as u32)),
                ch => out.push(ch),
            }
        }
        out.push('"');
        out
    }
    fn json(value: &Json) -> String {
        match value {
            Json::Null => "null".into(),
            Json::Bool(value) => value.to_string(),
            Json::Number(value) => value.lexeme().to_owned(),
            Json::String(value) => quote(value),
            Json::Array(values) => format!(
                "[{}]",
                values.iter().map(json).collect::<Vec<_>>().join(",")
            ),
            Json::Object(fields) => format!(
                "{{{}}}",
                fields
                    .iter()
                    .map(|(key, value)| format!("{}:{}", quote(key), json(value)))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }
    json(value).into_bytes()
}

/// Rejection must preserve all consumer state, including hidden seen-ID sets.
/// Accepting the untouched same page and every real successor proves rollback.
fn rejects_then_finishes(sequence: &[(TowerRequest, Vec<u8>)], index: usize, damaged: &[u8]) {
    let mut capture = new_capture(&sequence[0].0, sequence.len());
    for (request, response) in &sequence[..index] {
        capture.accept(request, response).unwrap();
    }
    let before = capture.progress();
    assert_eq!(
        capture.accept(&sequence[index].0, damaged),
        Err(PolicyReplayClientError::InvalidResponse)
    );
    assert_eq!(capture.progress(), before);
    for (request, response) in &sequence[index..] {
        capture.accept(request, response).unwrap();
    }
    assert!(capture.progress().complete);
    assert_eq!(capture.progress().next_cursor, None);
}

#[test]
fn consumer_traverses_every_original_case_after_the_owner_advances_without_an_extra_campaign() {
    let mut native = NativePages::new(80, 10);
    let initial = first(71, candidate());
    let mut capture = new_capture(&initial, 10);
    assert_eq!(capture.progress().cases_seen, 0);
    assert_eq!(capture.progress().pages_seen, 0);
    assert!(!capture.progress().complete);
    let (request, response, _) = native.issue(initial);
    // Object-key order is JSON syntax, not an authority or page identity.
    let progress = capture
        .accept(&request, &encoded(&decode(&response)))
        .unwrap();
    assert_eq!(progress.pages_seen, 1);
    assert_eq!(progress.cases_seen, 0);
    assert!(!progress.complete);
    assert!(progress.next_cursor.is_some());
    advance(&mut native.supervisor, 90);
    let owner = native.supervisor.host().unwrap().inspect();
    let journal = image(&native.root);
    let mut count = 0;
    while let Some(cursor) = capture.progress().next_cursor {
        let (request, response, _) = native.issue(continued(71, cursor));
        let document = decode(&response);
        count += field(page_body(&document), "cases")
            .as_array()
            .unwrap()
            .len();
        let progress = capture.accept(&request, &encoded(&document)).unwrap();
        assert_eq!(progress.cases_seen, count);
    }
    assert_eq!(count, 82);
    assert_eq!(capture.progress().cases_seen, 82);
    assert_eq!(capture.progress().pages_seen, 7);
    assert!(capture.progress().complete);
    assert_eq!(native.grant.remaining_queries(), 0);
    assert_eq!(native.grant.remaining_pages(), 3);
    assert_eq!(native.supervisor.host().unwrap().inspect(), owner);
    assert_eq!(image(&native.root), journal);
    let before = capture.progress();
    assert_eq!(
        capture.accept(&request, &response),
        Err(PolicyReplayClientError::Finished)
    );
    assert_eq!(capture.progress(), before);
}

#[test]
fn genuine_header_corruption_cannot_install_foreign_scope_basis_policy_or_summary() {
    let mut native = NativePages::new(18, 3);
    let sequence = native.sequence(candidate());
    let original = decode(&sequence[0].1);
    let mut damaged = Vec::new();
    for (path, value) in [
        (
            vec![
                "result",
                "value",
                "knowledge",
                "basis",
                "scope",
                "principal",
            ],
            text(99),
        ),
        (
            vec!["result", "value", "knowledge", "basis", "scope", "run"],
            text(4),
        ),
        (
            vec!["result", "value", "knowledge", "basis", "campaign"],
            text(72),
        ),
        (
            vec!["result", "value", "knowledge", "basis", "policy_generation"],
            text(2),
        ),
        (
            vec!["result", "value", "knowledge", "basis", "origin"],
            text("canonical_image"),
        ),
        (vec!["result", "value", "policy_changed"], Json::Bool(true)),
        (vec!["envelope", "idempotency_key"], text(72)),
    ] {
        let mut changed = original.clone();
        set(&mut changed, &path, value);
        damaged.push(changed);
    }
    let mut moved = original.clone();
    let mut other_mount = mount();
    other_mount.generation = Some(42);
    set(
        &mut moved,
        &["envelope", "address"],
        text(other_mount.canonical_string()),
    );
    damaged.push(moved);
    let mut policy = original.clone();
    let nodes = array_mut(path_mut(
        body_mut(&mut policy),
        &["candidate_policy", "nodes"],
    ));
    set(&mut nodes[0], &["maximum"], text(129));
    damaged.push(policy);
    for (path, value) in [
        (vec!["page", "count"], text(1)),
        (vec!["page", "offset"], text(16)),
        (vec!["page", "enumeration_complete"], Json::Bool(true)),
        (vec!["summary", "case_count"], text(21)),
        (vec!["summary", "delta_counts", "unchanged"], text(3)),
        (vec!["summary", "requires_shadow"], Json::Bool(true)),
    ] {
        let mut changed = original.clone();
        set(body_mut(&mut changed), &path, value);
        damaged.push(changed);
    }
    for changed in damaged {
        rejects_then_finishes(&sequence, 0, &encoded(&changed));
    }
}

#[test]
fn genuine_case_pages_cannot_change_the_basis_cursor_counts_or_frozen_summary() {
    let mut native = NativePages::new(18, 3);
    let sequence = native.sequence(candidate());
    let original = decode(&sequence[1].1);
    let mut damaged = Vec::new();
    for name in [
        "journal_revision",
        "control_sequence",
        "policy_epoch",
        "recorded_at",
    ] {
        let mut changed = original.clone();
        let basis = path_mut(&mut changed, &["result", "value", "knowledge", "basis"]);
        let actual = field(basis, name).as_str().unwrap().parse::<u64>().unwrap();
        set(basis, &[name], text(actual + 1));
        damaged.push(changed);
    }
    for (path, value) in [
        (vec!["page", "request_cursor"], text(u64::MAX)),
        (vec!["page", "offset"], text(16)),
        (vec!["page", "count"], text(15)),
        (vec!["page", "total"], text(19)),
        (vec!["page", "enumeration_complete"], Json::Bool(true)),
        (vec!["summary", "input_bytes"], text(1)),
        (vec!["summary", "remaining_campaigns_at_capture"], text(1)),
        (
            vec!["previous_policy"],
            field(page_body(&decode(&sequence[0].1)), "previous_policy").clone(),
        ),
    ] {
        let mut changed = original.clone();
        set(body_mut(&mut changed), &path, value);
        damaged.push(changed);
    }
    let mut repeated_cursor = original.clone();
    let request_cursor = field(field(page_body(&original), "page"), "request_cursor").clone();
    set(
        body_mut(&mut repeated_cursor),
        &["page", "next_cursor"],
        request_cursor,
    );
    damaged.push(repeated_cursor);
    for changed in damaged {
        rejects_then_finishes(&sequence, 1, &encoded(&changed));
    }
}

#[test]
fn missing_duplicate_or_reordered_original_case_ids_never_advance_consumer_state() {
    let mut native = NativePages::new(18, 3);
    let sequence = native.sequence(candidate());
    let original = decode(&sequence[1].1);
    let mut missing = original.clone();
    rows_mut(&mut missing).pop().unwrap();
    rejects_then_finishes(&sequence, 1, &encoded(&missing));

    let mut duplicate = original.clone();
    let first_id = field(&rows_mut(&mut duplicate)[1], "id").clone();
    set(&mut rows_mut(&mut duplicate)[2], &["id"], first_id);
    rejects_then_finishes(&sequence, 1, &encoded(&duplicate));

    // Swap both cases AND their matching local delta IDs. Local consistency
    // does not make a reordered original proposal enumeration acceptable.
    let mut reordered = original.clone();
    rows_mut(&mut reordered).swap(1, 2);
    array_mut(path_mut(body_mut(&mut reordered), &["newly_reviewable"])).swap(0, 1);
    rejects_then_finishes(&sequence, 1, &encoded(&reordered));

    let mut across_pages = decode(&sequence[2].1);
    let old_id = field(
        &field(page_body(&original), "cases").as_array().unwrap()[1],
        "id",
    )
    .clone();
    set(&mut rows_mut(&mut across_pages)[0], &["id"], old_id.clone());
    array_mut(path_mut(body_mut(&mut across_pages), &["newly_reviewable"]))[0] = old_id;
    rejects_then_finishes(&sequence, 2, &encoded(&across_pages));
}

#[test]
fn malformed_original_traces_and_delta_claims_are_rejected_before_ids_are_committed() {
    let mut native = NativePages::new(18, 3);
    let sequence = native.sequence(candidate());
    let original = decode(&sequence[1].1);
    let mut damaged = Vec::new();
    // Every actual FrozenAction has a concrete target and nonzero units/deadline.
    // These policies do not reference those fields, so predicate checks alone
    // cannot reject these impossible original action projections.
    for (name, value) in [
        ("target", Json::Null),
        ("units", text(0)),
        ("deadline", text(0)),
    ] {
        let mut changed = original.clone();
        set(&mut rows_mut(&mut changed)[0], &["action", name], value);
        damaged.push(changed);
    }
    let mut short_trace = original.clone();
    array_mut(path_mut(
        &mut rows_mut(&mut short_trace)[0],
        &["original", "trace"],
    ))
    .pop()
    .unwrap();
    damaged.push(short_trace);
    let mut bad_node = original.clone();
    let trace = array_mut(path_mut(
        &mut rows_mut(&mut bad_node)[0],
        &["candidate", "trace"],
    ));
    set(&mut trace[0], &["node"], text(1));
    damaged.push(bad_node);
    let mut bad_witness = original.clone();
    let trace = array_mut(path_mut(
        &mut rows_mut(&mut bad_witness)[0],
        &["original", "trace"],
    ));
    set(&mut trace[0], &["witness"], text(1));
    damaged.push(bad_witness);
    let mut bad_result = original.clone();
    let trace = array_mut(path_mut(
        &mut rows_mut(&mut bad_result)[0],
        &["original", "trace"],
    ));
    set(&mut trace[2], &["result"], text("violated"));
    damaged.push(bad_result);
    let mut wrong_delta = original.clone();
    set(
        &mut rows_mut(&mut wrong_delta)[0],
        &["delta"],
        text("newly_blocked"),
    );
    damaged.push(wrong_delta);
    let mut wrong_list = original.clone();
    array_mut(path_mut(body_mut(&mut wrong_list), &["newly_reviewable"]))
        .pop()
        .unwrap();
    damaged.push(wrong_list);
    for changed in damaged {
        rejects_then_finishes(&sequence, 1, &encoded(&changed));
    }
}

#[test]
fn unknown_candidate_reads_require_real_missing_read_nodes_and_matching_shadow_delta() {
    let mut native = NativePages::new(18, 3);
    let policy = Policy::new(
        2,
        vec![
            Predicate::PayloadAtMost(128),
            Predicate::Absent { key: 99 },
            Predicate::Any(vec![0, 1]),
        ],
    )
    .unwrap();
    let sequence = native.sequence(policy);
    let original = decode(&sequence[1].1);
    let mut damaged = Vec::new();
    for nodes in [vec![], vec![text(0)], vec![text(3)], vec![text(1), text(1)]] {
        let mut changed = original.clone();
        set(
            &mut rows_mut(&mut changed)[0],
            &["candidate", "missing_nodes"],
            Json::Array(nodes),
        );
        damaged.push(changed);
    }
    let mut wrong_delta = original.clone();
    set(
        &mut rows_mut(&mut wrong_delta)[0],
        &["delta"],
        text("unchanged"),
    );
    damaged.push(wrong_delta);
    let mut wrong_reason = original.clone();
    set(
        &mut rows_mut(&mut wrong_reason)[0],
        &["candidate", "reason"],
        text("not_needed"),
    );
    damaged.push(wrong_reason);
    for changed in damaged {
        rejects_then_finishes(&sequence, 1, &encoded(&changed));
    }
}

#[test]
fn consumer_page_limit_preserves_an_honest_incomplete_result_without_accepting_more() {
    let mut native = NativePages::new(18, 3);
    let sequence = native.sequence(candidate());
    let mut capture = new_capture(&sequence[0].0, 2);
    capture.accept(&sequence[0].0, &sequence[0].1).unwrap();
    let progress = capture.accept(&sequence[1].0, &sequence[1].1).unwrap();
    assert_eq!(progress.pages_seen, 2);
    assert_eq!(progress.cases_seen, 16);
    assert!(!progress.complete);
    assert!(progress.next_cursor.is_some());
    assert_eq!(
        capture.accept(&sequence[2].0, &sequence[2].1),
        Err(PolicyReplayClientError::PageLimit)
    );
    assert_eq!(capture.progress(), progress);
}

#[test]
fn native_page_exhaustion_stops_without_claiming_the_remaining_cases() {
    let mut native = NativePages::new(18, 2);
    let sequence = native.sequence(candidate());
    assert_eq!(sequence.len(), 2);
    let mut capture = new_capture(&sequence[0].0, 10);
    for (request, response) in &sequence {
        capture.accept(request, response).unwrap();
    }
    let progress = capture.progress();
    assert_eq!(progress.pages_seen, 2);
    assert_eq!(progress.cases_seen, 16);
    assert!(!progress.complete);
    assert_eq!(progress.next_cursor, None);
    assert_eq!(
        capture.accept(&sequence[1].0, &sequence[1].1),
        Err(PolicyReplayClientError::Finished)
    );
    assert_eq!(capture.progress(), progress);
}

#[test]
fn consumer_requires_the_explicit_paged_initial_request_and_bounded_traversal() {
    for initial in [legacy(71, candidate()), continued(71, 1)] {
        assert!(matches!(
            PolicyReplayCapture::new(profile().delivery.scope, &initial, 4),
            Err(PolicyReplayClientError::InvalidRequest)
        ));
    }
    for maximum in [0, 1025, usize::MAX] {
        assert!(matches!(
            PolicyReplayCapture::new(profile().delivery.scope, &first(71, candidate()), maximum),
            Err(PolicyReplayClientError::InvalidRequest)
        ));
    }
    let mut foreign_scope = profile().delivery.scope;
    foreign_scope.run += 1;
    assert!(matches!(
        PolicyReplayCapture::new(foreign_scope, &first(71, candidate()), 4),
        Err(PolicyReplayClientError::InvalidRequest)
    ));
}

#[test]
fn genuine_stale_mount_advice_stops_safely_and_cannot_redirect_to_another_run() {
    let mut native = NativePages::new(1, 2);
    let mut initial = first(71, candidate());
    initial.address.generation = Some(40);
    let response = native.facade.dispatch(&access(), initial.clone());
    refused(&response, ReasonCode::StaleGeneration);
    assert_eq!(response.envelope.address, initial.address);
    assert_eq!(response.envelope.missing, vec![initial.address.clone()]);
    assert_eq!(response.envelope.would_change_this, vec![mount()]);
    let encoded_response = response.to_json().unwrap().into_bytes();
    let mut capture = new_capture(&initial, 4);
    let before = capture.progress();
    for redirect in [
        Address::new("2", Noun::Run, "3", Some(41)).unwrap(),
        Address::new("1", Noun::Run, "4", Some(41)).unwrap(),
        Address::new("1", Noun::Run, "3", None).unwrap(),
    ] {
        let mut changed = decode(&encoded_response);
        array_mut(path_mut(&mut changed, &["envelope", "would_change_this"]))[0] =
            text(redirect.canonical_string());
        assert_eq!(
            capture.accept(&initial, &encoded(&changed)),
            Err(PolicyReplayClientError::InvalidResponse)
        );
        assert_eq!(capture.progress(), before);
    }
    let stopped = capture.accept(&initial, &encoded_response).unwrap();
    assert_eq!(stopped.pages_seen, 1);
    assert_eq!(stopped.cases_seen, 0);
    assert!(!stopped.complete);
    assert_eq!(stopped.next_cursor, None);
    assert_eq!(
        capture.accept(&initial, &encoded_response),
        Err(PolicyReplayClientError::Finished)
    );
    assert_eq!(native.grant.remaining_queries(), 1);
    assert_eq!(native.grant.remaining_pages(), 2);
}
