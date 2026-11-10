//! Original journal metadata and adversarial operator response correlation.

use super::super::super::journal_tail_validation::Sequence;
use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileSupervisorJournalTailGrant, MAX_JOURNAL_TAIL_PAGE_EVENTS,
};

struct NativeTail {
    tower: PublicationTower,
    supervisor: native::Supervisor,
    grant: FileSupervisorJournalTailGrant,
    _storage: native::Directory,
}

impl NativeTail {
    fn new(queries: usize, events_per_page: usize) -> Self {
        Self::create(queries, events_per_page, false)
    }

    fn published(queries: usize, events_per_page: usize) -> Self {
        Self::create(queries, events_per_page, true)
    }

    fn create(queries: usize, events_per_page: usize, publish: bool) -> Self {
        let storage = native::Directory::new();
        let (actor, mut supervisor, reviewer, proposal) = native::create(&storage);
        native::observe(&mut supervisor);
        actor.submit(7, &proposal).unwrap();
        if publish {
            let keys = native::ready(&mut supervisor, &reviewer, 7);
            let mut host = supervisor.host_mut().unwrap();
            native::oversight::dispatch(&mut host, &keys);
            let revision = host.revision();
            host.publish(revision, keys.automatic.attempt()).unwrap();
            let revision = host.revision();
            host.reconcile(revision, keys.automatic.attempt()).unwrap();
            assert_eq!(host.inspect().executions, 1);
        } else {
            let mut host = supervisor.host_mut().unwrap();
            for tick in 2..6 {
                let revision = host.revision();
                host.observe_time(revision, ElapsedTick(tick)).unwrap();
            }
        }
        let (port, grant) = supervisor
            .journal_tail_port(queries, events_per_page)
            .unwrap();
        let mut tower =
            PublicationTower::new(mount(), actor, PublicationLimits::default()).unwrap();
        tower.set_journal_tail(Some(port)).unwrap();
        Self {
            tower,
            supervisor,
            grant,
            _storage: storage,
        }
    }

    fn exchange(&mut self, request: &TowerRequest) -> String {
        let before = self.supervisor.host().unwrap().inspect();
        let bytes = fs::read(self._storage.store().join("delivery.bin")).unwrap();
        let response = self
            .tower
            .exchange(&access(), encode_request(request).unwrap().as_bytes());
        assert_eq!(self.supervisor.host().unwrap().inspect(), before);
        assert_eq!(
            fs::read(self._storage.store().join("delivery.bin")).unwrap(),
            bytes
        );
        response.to_json().unwrap()
    }
}

fn access() -> TowerAccess {
    TowerAccess::new(
        CallerScope {
            caller_tenant: native::profile().delivery.scope.tenant.to_string(),
            allowed_planes: BTreeSet::from([ResolverPlane::ControlLedger]),
            granted_authorities: BTreeSet::from([AuthorityKind::Verify]),
        },
        true,
        Vec::new(),
        0,
    )
}

fn request(after_revision: u64) -> TowerRequest {
    let mut request = TowerRequest::new("operator-tail", mount(), Verb::Tail);
    request.arguments = ReadArguments::JournalTail { after_revision };
    request
}

fn continuation(cursor: u64) -> TowerRequest {
    let mut request = request(0);
    request.arguments = ReadArguments::JournalTailPage { cursor };
    request
}

fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter()
        .fold(value, |value, key| value.get(key).unwrap())
}

fn field_mut<'a>(value: &'a mut Json, path: &[&str]) -> &'a mut Json {
    let Some((key, rest)) = path.split_first() else {
        return value;
    };
    let Json::Object(object) = value else {
        panic!("expected object");
    };
    field_mut(object.get_mut(*key).unwrap(), rest)
}

fn page(value: &Json) -> &Json {
    field(value, &["result", "value", "knowledge", "value"])
}

fn number(value: &Json) -> u64 {
    value.as_str().unwrap().parse().unwrap()
}

fn settings(root: &Directory, exchanges: u64) -> PathBuf {
    let path = root.settings(credentials().pid(), exchanges);
    let selected = fs::read_to_string(&path).unwrap()
        .replace("\"inspection_queries\":3", "\"inspection_queries\":0")
        .replace("\"explanation_queries\":2", "\"explanation_queries\":0")
        .replace("\"explanation_replay_events\":1024", "\"explanation_replay_events\":0")
        .replace("policy_and_selected_witnesses", "none")
        .replacen('{', "{\"journal_tail\":{\"queries\":4,\"events_per_page\":1,\"disclosure\":\"redacted_journal_events\"},", 1);
    let decoded = OperatorSettings::decode(selected.as_bytes()).unwrap();
    assert_eq!(
        (decoded.inspection_queries, decoded.explanation_queries),
        (0, 0)
    );
    assert!(decoded.explanation_numerical_replay_events.is_none());
    let tail = decoded.journal_tail.unwrap();
    assert_eq!((tail.queries, tail.events_per_page), (4, 1));
    fs::write(&path, selected).unwrap();
    path
}

fn authenticate(socket: &UnixStream) {
    let observed = credentials();
    PeerPolicy::new(observed.uid(), observed.gid(), Some(observed.pid()))
        .unwrap()
        .verify(socket)
        .unwrap();
}

#[test]
fn original_publication_tail_requires_closed_shape_and_direct_event_identities() {
    let mut native = NativeTail::published(3, MAX_JOURNAL_TAIL_PAGE_EVENTS);
    let mut query = request(0);
    // Correlation labels remain separate from the native grant's query budget.
    query.idempotency_key = Some("tail-observation".into());
    let original = native.exchange(&query);
    let parsed = parse_response(original.as_bytes(), &query).unwrap();
    Sequence::new(
        native::profile().delivery.scope,
        Some(MAX_JOURNAL_TAIL_PAGE_EVENTS),
    )
    .accept(&parsed, &query)
    .unwrap();
    let cut = number(field(
        &parsed,
        &["result", "value", "knowledge", "basis", "journal_revision"],
    ));
    assert!(cut > 2);
    assert_eq!(
        page(&parsed)
            .get("enumeration_complete")
            .and_then(Json::as_bool),
        Some(true)
    );
    let events = page(&parsed).get("events").unwrap().as_array().unwrap();
    let event = |kind: &str| {
        events
            .iter()
            .find(|event| event.get("kind").and_then(Json::as_str) == Some(kind))
            .unwrap()
    };
    assert_eq!(
        event("submit_request")
            .get("request")
            .and_then(Json::as_str),
        Some("7")
    );
    assert!(event("submit_request").get("attempt").unwrap().is_null());
    assert_eq!(
        event("begin_review").get("round").and_then(Json::as_str),
        Some("107")
    );
    assert!(!event("begin_review").get("attempt").unwrap().is_null());
    assert!(event("begin_review").get("request").unwrap().is_null());
    assert_eq!(
        event("finish_review").get("round").and_then(Json::as_str),
        Some("107")
    );
    assert!(event("finish_review").get("attempt").unwrap().is_null());
    assert!(event("human_decision").get("request").unwrap().is_null());
    assert!(!event("publish").get("attempt").unwrap().is_null());
    assert!(event("publish").get("request").unwrap().is_null());
    for secret in [
        "PRIVATE-SOURCE-BYTES",
        "payload",
        "request_address",
        "external_request",
        "human_key",
    ] {
        assert!(!original.contains(secret));
    }
    for damaged in [
        original.replace(
            "fa.publication-journal-tail/1",
            "fa.publication-journal-tail/2",
        ),
        original.replace("\"operation\":\"tail\"", "\"operation\":\"situation\""),
        original.replace("\"historical\":true", "\"historical\":false"),
        original.replace(
            "\"disclosure\":\"redacted_journal_events\"",
            "\"disclosure\":\"payload\"",
        ),
        original.replace(
            "\"current_eligibility\":\"not_observed\"",
            "\"current_eligibility\":\"known\"",
        ),
        original.replace(
            "\"query_address\":",
            "\"selection\":null,\"query_address\":",
        ),
        original.replace(
            &format!("\"query_address\":\"{}\"", mount().canonical_string()),
            "\"query_address\":\"fa://1/run/3\"",
        ),
        original.replace("\"after_revision\":\"0\"", "\"after_revision\":\"1\""),
        original.replace(
            "\"origin\":\"acknowledged_owner\"",
            "\"origin\":\"reopened_disk\"",
        ),
        original.replace("\"anchored_through\":null", "\"anchored_through\":\"1\""),
        original.replace("\"purpose\":\"effect\"", "\"purpose\":\"experiment\""),
        original.replace("\"offset\":\"0\"", "\"offset\":\"1\""),
        original.replace(
            &format!("\"through_revision\":\"{cut}\""),
            &format!("\"through_revision\":\"{}\"", cut - 1),
        ),
        original.replace(
            "\"enumeration_complete\":true",
            "\"enumeration_complete\":false",
        ),
        original.replace("\"continuation\":null", "\"continuation\":\"1\""),
        original.replacen("\"revision\":\"1\"", "\"revision\":\"2\"", 1),
        original.replacen("\"kind\":\"time\"", "\"kind\":\"invented_event\"", 1),
        original.replacen(
            "\"kind\":\"time\",\"attempt\":null",
            "\"kind\":\"time\",\"attempt\":\"1\"",
            1,
        ),
        original.replacen(
            "\"kind\":\"submit_request\",\"attempt\":null",
            "\"kind\":\"submit_request\",\"attempt\":\"1\"",
            1,
        ),
        original.replacen("\"round\":null", "\"round\":\"107\"", 1),
        original.replacen("\"request\":\"7\"", "\"request\":\"07\"", 1),
        original.replacen("\"request\":\"7\"", "\"request\":null", 1),
        original.replace(
            &format!("\"registration\":\"{}\"", mount().canonical_string()),
            "\"registration\":\"fa://1/run/3@12\"",
        ),
        original.replace(
            "\"idempotency_key\":\"tail-observation\"",
            "\"idempotency_key\":null",
        ),
        original.replace(
            "\"operation_id\":\"operator-tail\"",
            "\"operation_id\":\"other-tail\"",
        ),
    ] {
        assert_ne!(damaged, original, "mutation must change the original reply");
        assert!(
            validate_response(damaged.as_bytes(), &query).is_err(),
            "{damaged}"
        );
    }
    // The endpoint profile fixes identity coordinates absent from the run URI.
    for identity in ["principal", "branch", "authority"] {
        let before = field(
            &parsed,
            &["result", "value", "knowledge", "basis", "scope", identity],
        )
        .as_str()
        .unwrap();
        let changed = original.replacen(
            &format!("\"{identity}\":\"{before}\""),
            &format!("\"{identity}\":\"99\""),
            1,
        );
        let damaged = parse_response(changed.as_bytes(), &query).unwrap();
        assert!(
            Sequence::new(
                native::profile().delivery.scope,
                Some(MAX_JOURNAL_TAIL_PAGE_EVENTS)
            )
            .accept(&damaged, &query)
            .is_err()
        );
    }
    let mut unqualified = query.clone();
    unqualified.address.generation = None;
    let reply = native.exchange(&unqualified);
    let parsed = parse_response(reply.as_bytes(), &unqualified).unwrap();
    Sequence::new(
        native::profile().delivery.scope,
        Some(MAX_JOURNAL_TAIL_PAGE_EVENTS),
    )
    .accept(&parsed, &unqualified)
    .unwrap();
}

#[test]
fn successive_pages_keep_original_basis_fixed_page_size_and_global_cursor_identity() {
    let mut native = NativeTail::new(4, 2);
    let mut initial = request(0);
    initial.address.generation = None;
    let response = native.exchange(&initial);
    let first = parse_response(response.as_bytes(), &initial).unwrap();
    let cursor = number(page(&first).get("continuation").unwrap());
    // Generic actor transport has no selected operator page size; its observed
    // first full page still fixes the size for this capture's later pages.
    let mut sequence = Sequence::new(native::profile().delivery.scope, None);
    sequence.accept(&first, &initial).unwrap();
    {
        let mut host = native.supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(6)).unwrap();
    }
    let mut query = continuation(cursor);
    query.address.generation = None;
    let response = native.exchange(&query);
    let second = parse_response(response.as_bytes(), &query).unwrap();
    assert_eq!(
        field(&first, &["result", "value", "knowledge", "basis"]),
        field(&second, &["result", "value", "knowledge", "basis"]),
    );
    let changed_registration = response.replace("@11", "@12");
    assert_ne!(changed_registration, response);
    let changed = parse_response(changed_registration.as_bytes(), &query).unwrap();
    assert!(
        sequence.accept(&changed, &query).is_err(),
        "a known cursor cannot move registration under an unqualified request"
    );
    for coordinate in ["control_sequence", "recorded_at"] {
        let original = field(
            &second,
            &["result", "value", "knowledge", "basis", coordinate],
        );
        let value = number(original);
        let changed = response.replacen(
            &format!("\"{coordinate}\":\"{value}\""),
            &format!("\"{coordinate}\":\"{}\"", value + 1),
            1,
        );
        let changed = parse_response(changed.as_bytes(), &query).unwrap();
        assert!(sequence.accept(&changed, &query).is_err());
    }
    let mut shortened = second.clone();
    let Json::Array(events) = field_mut(
        &mut shortened,
        &["result", "value", "knowledge", "value", "events"],
    ) else {
        panic!("events");
    };
    events.truncate(1);
    *field_mut(
        &mut shortened,
        &["result", "value", "knowledge", "value", "through_revision"],
    ) = Json::String("3".into());
    assert!(super::super::super::journal_tail_validation::matches(
        field(&shortened, &["result", "value"]),
        &query
    ));
    assert!(
        sequence.accept(&shortened, &query).is_err(),
        "host page size cannot shrink within the original capture"
    );
    sequence.accept(&second, &query).unwrap();
    assert!(
        sequence.accept(&second, &query).is_err(),
        "cursor must be consumed once"
    );
    // A consumed cursor cannot be reissued by claiming a different registration.
    let mut rebound_first = first.clone();
    *field_mut(&mut rebound_first, &["result", "value", "registration"]) =
        Json::String("fa://1/run/3@12".into());
    assert!(super::super::super::journal_tail_validation::matches(
        field(&rebound_first, &["result", "value"]),
        &initial
    ));
    assert!(sequence.accept(&rebound_first, &initial).is_err());
    let cursor = number(page(&second).get("continuation").unwrap());
    let last = continuation(cursor);
    let response = native.exchange(&last);
    let last_page = parse_response(response.as_bytes(), &last).unwrap();
    sequence.accept(&last_page, &last).unwrap();
    assert_eq!(
        page(&last_page)
            .get("enumeration_complete")
            .and_then(Json::as_bool),
        Some(true)
    );
    assert_eq!(
        number(page(&last_page).get("through_revision").unwrap()),
        number(field(
            &first,
            &["result", "value", "knowledge", "basis", "journal_revision"]
        ))
    );
    assert_eq!(native.grant.remaining_queries(), 1);
}

#[test]
fn configured_width_checks_first_pages_and_previously_unseen_continuation_alignment() {
    let mut native = NativeTail::new(4, 4);
    let scope = native::profile().delivery.scope;
    let initial = request(0);
    let response = native.exchange(&initial);
    let first = parse_response(response.as_bytes(), &initial).unwrap();
    assert_eq!(
        page(&first)
            .get("events")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        4
    );
    let cut = number(field(
        &first,
        &["result", "value", "knowledge", "basis", "journal_revision"],
    ));
    assert!(
        cut > 4 && cut < 8,
        "fixture must have a short terminal page"
    );
    let mut selected = Sequence::new(scope, Some(4));

    // This still has valid ordinals, range, link types and a continuation. It
    // cannot be the first page of the explicitly selected four-event grant.
    let mut shortened = first.clone();
    let Json::Array(events) = field_mut(
        &mut shortened,
        &["result", "value", "knowledge", "value", "events"],
    ) else {
        panic!("events");
    };
    events.truncate(3);
    *field_mut(
        &mut shortened,
        &["result", "value", "knowledge", "value", "through_revision"],
    ) = Json::String("3".into());
    assert!(super::super::super::journal_tail_validation::matches(
        field(&shortened, &["result", "value"]),
        &initial
    ));
    assert!(selected.accept(&shortened, &initial).is_err());
    selected.accept(&first, &initial).unwrap();
    for invalid in [0, MAX_JOURNAL_TAIL_PAGE_EVENTS + 1, usize::MAX] {
        assert!(
            Sequence::new(scope, Some(invalid))
                .accept(&first, &initial)
                .is_err()
        );
    }

    let cursor = number(page(&first).get("continuation").unwrap());
    let query = continuation(cursor);
    let response = native.exchange(&query);
    let second = parse_response(response.as_bytes(), &query).unwrap();
    assert_eq!(number(page(&second).get("offset").unwrap()), 4);
    assert_eq!(
        page(&second)
            .get("enumeration_complete")
            .and_then(Json::as_bool),
        Some(true)
    );
    assert_eq!(
        page(&second)
            .get("events")
            .unwrap()
            .as_array()
            .unwrap()
            .len() as u64,
        cut - 4
    );

    // A reconnect may first observe this legitimate short last page. Make a
    // shape-consistent alternative whose offset is not a grant-sized multiple:
    // its count still exactly equals its reported remaining interval.
    let mut shifted = second.clone();
    *field_mut(
        &mut shifted,
        &["result", "value", "knowledge", "basis", "journal_revision"],
    ) = Json::String((cut - 1).to_string());
    *field_mut(
        &mut shifted,
        &["result", "value", "knowledge", "value", "offset"],
    ) = Json::String("3".into());
    *field_mut(
        &mut shifted,
        &["result", "value", "knowledge", "value", "through_revision"],
    ) = Json::String((cut - 1).to_string());
    let Json::Array(events) = field_mut(
        &mut shifted,
        &["result", "value", "knowledge", "value", "events"],
    ) else {
        panic!("events");
    };
    for event in events {
        let revision = number(event.get("revision").unwrap());
        *field_mut(event, &["revision"]) = Json::String((revision - 1).to_string());
    }
    assert!(super::super::super::journal_tail_validation::matches(
        field(&shifted, &["result", "value"]),
        &query
    ));
    let mut reconnected = Sequence::new(scope, Some(4));
    assert!(reconnected.accept(&shifted, &query).is_err());
    reconnected.accept(&second, &query).unwrap();
    selected.accept(&second, &query).unwrap();

    // A complete empty initial suffix remains valid under the same bound.
    let empty = request(cut);
    let response = native.exchange(&empty);
    let parsed = parse_response(response.as_bytes(), &empty).unwrap();
    assert!(
        page(&parsed)
            .get("events")
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty()
    );
    selected.accept(&parsed, &empty).unwrap();
    assert_eq!(native.grant.remaining_queries(), 1);
}

#[test]
fn actual_future_revision_quota_and_owner_loss_preserve_native_epistemic_states() {
    let mut native = NativeTail::new(2, 1);
    let mut sequence = Sequence::new(native::profile().delivery.scope, Some(1));
    let future = request(u64::MAX);
    let response = native.exchange(&future);
    let parsed = parse_response(response.as_bytes(), &future).unwrap();
    sequence.accept(&parsed, &future).unwrap();
    assert_eq!(
        field(&parsed, &["result", "value", "knowledge", "state"]).as_str(),
        Some("unknown")
    );
    assert_eq!(native.grant.remaining_queries(), 1);
    let query = request(0);
    let response = native.exchange(&query);
    let parsed = parse_response(response.as_bytes(), &query).unwrap();
    sequence.accept(&parsed, &query).unwrap();
    assert_eq!(
        page(&parsed)
            .get("enumeration_complete")
            .and_then(Json::as_bool),
        Some(false)
    );
    assert!(page(&parsed).get("continuation").unwrap().is_null());
    let response = native.exchange(&query);
    let parsed = parse_response(response.as_bytes(), &query).unwrap();
    sequence.accept(&parsed, &query).unwrap();
    assert_eq!(
        field(&parsed, &["result", "value", "error", "code"]).as_str(),
        Some("capacity")
    );
    let NativeTail {
        mut tower,
        supervisor,
        grant,
        _storage,
    } = native;
    drop(supervisor);
    let response = tower
        .exchange(&access(), encode_request(&query).unwrap().as_bytes())
        .to_json()
        .unwrap();
    let parsed = parse_response(response.as_bytes(), &query).unwrap();
    sequence.accept(&parsed, &query).unwrap();
    assert_eq!(
        field(&parsed, &["result", "value", "knowledge", "state"]).as_str(),
        Some("unknown")
    );
    grant.revoke();
    let response = tower
        .exchange(&access(), encode_request(&query).unwrap().as_bytes())
        .to_json()
        .unwrap();
    let parsed = parse_response(response.as_bytes(), &query).unwrap();
    sequence.accept(&parsed, &query).unwrap();
    assert_eq!(
        field(
            &parsed,
            &["result", "value", "knowledge", "authority_required"]
        )
        .as_str(),
        Some("supervisor_journal_tail")
    );
    drop(_storage);
}

#[test]
fn authenticated_explicit_tail_emits_one_page_and_never_follows_its_cursor_implicitly() {
    let root = Directory::new();
    let profile = settings(&root, 4);
    let query = request(0);
    let documents = [root.document("tail.json", &query)];
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let mut native = NativeTail::new(2, 1);
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        assert_eq!(decode_request(&read_document(&mut socket)).unwrap(), query);
        let response = native.exchange(&query);
        let parsed = parse_response(response.as_bytes(), &query).unwrap();
        assert!(!page(&parsed).get("continuation").unwrap().is_null());
        socket.write_all(response.as_bytes()).unwrap();
        socket.write_all(b"\n").unwrap();
        socket.flush().unwrap();
        assert_eq!(
            socket.read(&mut [0_u8]).unwrap(),
            0,
            "client followed an unrequested cursor"
        );
        assert_eq!(native.grant.remaining_queries(), 1);
        response
    });
    let mut output = Vec::new();
    operator_command(&args(&profile, &documents), &mut output).unwrap();
    assert_eq!(output, (server.join().unwrap() + "\n").as_bytes());
}

#[test]
fn changed_scope_or_configured_width_stops_before_output_and_the_next_explicit_request() {
    for changed_scope in [true, false] {
        let root = Directory::new();
        let profile = settings(&root, 4);
        let query = request(0);
        let documents = [
            root.document("first.json", &query),
            root.document("later.json", &request(1)),
        ];
        let listener = UnixListener::bind(root.socket()).unwrap();
        let server = std::thread::spawn(move || {
            // The public profile selects one event. The second case supplies
            // an authentic native two-event page, not a fabricated success.
            let mut native = NativeTail::new(2, if changed_scope { 1 } else { 2 });
            let mut socket = bounded_socket(&listener);
            authenticate(&socket);
            assert_eq!(decode_request(&read_document(&mut socket)).unwrap(), query);
            let original = native.exchange(&query);
            let changed = if changed_scope {
                let changed = original.replacen("\"principal\":\"2\"", "\"principal\":\"22\"", 1);
                assert_ne!(changed, original);
                changed
            } else {
                original
            };
            socket.write_all(changed.as_bytes()).unwrap();
            socket.write_all(b"\n").unwrap();
            socket.flush().unwrap();
            assert_eq!(
                socket.read(&mut [0_u8]).unwrap(),
                0,
                "changed owner scope or page width caused another request"
            );
            assert_eq!(native.grant.remaining_queries(), 1);
        });
        let mut output = Vec::new();
        let error = operator_command(&args(&profile, &documents), &mut output).unwrap_err();
        assert!(error.contains("invalid or inconsistent native tail page"));
        assert!(output.is_empty());
        server.join().unwrap();
    }
}
