//! Real held congresses and finite authenticated operator documents. Client
//! validation checks correlation and stated bounds, not remote authority truth.
use super::*;
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileConveneRequest, FileSupervisorConveneGrant,
};
use fa_reference::typestate_cx::PurposeTag;

#[path = "../../../../../../../../tests/support/file_convene.rs"]
mod held;

struct NativeConvene {
    file: held::FileRig,
    tower: PublicationTower,
    grant: FileSupervisorConveneGrant,
}

impl NativeConvene {
    fn new() -> Self {
        let file = held::held();
        let (port, grant) = file.rig.driver.supervisor().convene_port(8, 2).unwrap();
        let mut tower = PublicationTower::new_control_operator(
            mount(),
            file.rig.port.clone(),
            PublicationLimits::default(),
        )
        .unwrap();
        tower.set_convene(Some(port)).unwrap();
        Self { file, tower, grant }
    }

    fn response(&mut self, request: &TowerRequest) -> PublicationResponse {
        let scope = native::profile().delivery.scope;
        let access = TowerAccess::new(
            CallerScope {
                caller_tenant: scope.tenant.to_string(),
                allowed_planes: BTreeSet::from([ResolverPlane::ControlLedger]),
                granted_authorities: BTreeSet::from([AuthorityKind::Verify]),
            },
            true,
            vec![PurposeTag::Permit],
            0,
        );
        self.tower
            .exchange(&access, encode_request(request).unwrap().as_bytes())
    }

    fn dispatch(&mut self, request: &TowerRequest) -> String {
        self.response(request).to_json().unwrap()
    }

    fn command(&mut self) -> TowerRequest {
        let PublicationResult::ConveneNext(next) = self.response(&next()).result else {
            panic!("expected the original native held plan");
        };
        next.action().unwrap().unwrap()
    }
}

fn next() -> TowerRequest {
    let scope = native::profile().delivery.scope;
    let mut request = TowerRequest::new(
        "held-review-next",
        Address::new(
            scope.tenant.to_string(),
            Noun::Attempt,
            format!(
                "request-{}-{}-{}-{}-{}",
                scope.run,
                scope.principal,
                scope.branch,
                scope.authority,
                held::REQUEST
            ),
            Some(GENERATION),
        )
        .unwrap(),
        Verb::Next,
    );
    request.arguments = ReadArguments::ConveneNext {
        round: held::NEXT_ROUND,
    };
    request
}

fn native_request(command: &TowerRequest) -> FileConveneRequest {
    let ReadArguments::Convene { round, expected } = &command.arguments else {
        panic!("expected explicit held successor request");
    };
    FileConveneRequest::new(held::REQUEST, *round, *expected).unwrap()
}

fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter()
        .fold(value, |value, name| value.get(name).unwrap())
}

fn settings(root: &Directory) -> PathBuf {
    let path = root.settings(credentials().pid(), 8);
    let original = fs::read_to_string(&path).unwrap();
    let selected = original.replacen(
        '{',
        "{\"convening\":{\"queries\":8,\"operations\":2,\"authority\":\"convene_held_review\"},",
        1,
    );
    assert!(
        OperatorSettings::decode(selected.as_bytes())
            .unwrap()
            .convening
            .is_some()
    );
    fs::write(&path, selected).unwrap();
    path
}

fn authenticate(socket: &UnixStream) {
    let peer = credentials();
    PeerPolicy::new(peer.uid(), peer.gid(), Some(peer.pid()))
        .unwrap()
        .verify(socket)
        .unwrap();
}

#[test]
fn genuine_held_plan_binds_the_exact_predecessor_round_and_registration() {
    let mut native = NativeConvene::new();
    let query = next();
    let bytes = held::image(&native.file);
    let reads = native.file.source.status();
    let original = native.dispatch(&query);
    let parsed = parse_response(original.as_bytes(), &query).unwrap();
    assert_eq!(
        field(&parsed, &["result", "kind"]).as_str(),
        Some("convene-next")
    );
    assert_eq!(
        field(
            &parsed,
            &["result", "value", "knowledge", "value", "can_convene"]
        ),
        &Json::Bool(true)
    );
    assert_eq!(native.grant.remaining_operations(), 2);
    assert_eq!(held::image(&native.file), bytes);
    assert_eq!(native.file.source.status(), reads);
    for damaged in [
        original.replace(
            "fa.publication-convene-next/1",
            "fa.publication-convene-next/2",
        ),
        original.replace("original_held_review", "all_reviews"),
        original.replace("single_connected_held_review", "complete_action_ranking"),
        original.replace("live_convening_grant", "durable_global_operation"),
        original.replacen("\"round\":\"102\"", "\"round\":\"103\"", 1),
        original.replace(
            "\"registration\":\"fa://1/run/3@11\"",
            "\"registration\":\"fa://1/run/3@12\"",
        ),
        original.replacen("\"held_round\":\"101\"", "\"held_round\":\"100\"", 1),
        original.replacen("\"principal\":\"2\"", "\"principal\":\"22\"", 1),
        original.replace("\"can_convene\":true", "\"can_convene\":false"),
        original.replace(
            "\"block_reason\":null",
            "\"block_reason\":\"operation_limit\"",
        ),
        original.replace("\"verb\":\"convene\"", "\"verb\":\"fence\""),
        original.replace("\"idempotency_key\":\"102\"", "\"idempotency_key\":\"103\""),
        original.replace("\"effect_authority\":false", "\"effect_authority\":true"),
        original.replace("\"automatic_retry\":false", "\"automatic_retry\":true"),
    ] {
        assert_ne!(damaged, original, "fixture mutation missed its target");
        assert!(validate_response(damaged.as_bytes(), &query).is_err());
    }
    let mut unqualified = query;
    unqualified.address.generation = None;
    validate_response(native.dispatch(&unqualified).as_bytes(), &unqualified).unwrap();
    assert_eq!(held::image(&native.file), bytes);
}

#[test]
fn pending_and_actual_opened_receipts_have_distinct_checked_meanings() {
    let mut native = NativeConvene::new();
    let command = native.command();
    let input = native_request(&command);
    let before = held::image(&native.file);
    let reads = native.file.source.status();
    let pending = native.dispatch(&command);
    let parsed = parse_response(pending.as_bytes(), &command).unwrap();
    assert_eq!(
        field(
            &parsed,
            &["result", "value", "knowledge", "value", "outcome"]
        )
        .as_str(),
        Some("pending")
    );
    assert!(
        field(
            &parsed,
            &["result", "value", "knowledge", "value", "opened"]
        )
        .is_null()
    );
    assert_eq!(native.dispatch(&command), pending);
    assert_eq!(native.grant.remaining_operations(), 1);
    assert_eq!(held::image(&native.file), before);
    assert_eq!(native.file.source.status(), reads);
    for damaged in [
        pending.replace("\"outcome\":\"pending\"", "\"outcome\":\"opened\""),
        pending.replace("\"round_opened\":false", "\"round_opened\":true"),
        pending.replace("\"opened\":null", "\"opened\":{}"),
        pending.replace(
            "\"queued_work_is_durable\":false",
            "\"queued_work_is_durable\":true",
        ),
        pending.replace(
            "\"live_evidence_read\":false",
            "\"live_evidence_read\":true",
        ),
        pending.replacen("\"held_round\":\"101\"", "\"held_round\":\"102\"", 1),
    ] {
        assert_ne!(damaged, pending);
        assert!(validate_response(damaged.as_bytes(), &command).is_err());
    }

    held::launch(&mut native.file, &input);
    let opened = native.dispatch(&command);
    let parsed = parse_response(opened.as_bytes(), &command).unwrap();
    assert_eq!(
        field(
            &parsed,
            &["result", "value", "knowledge", "value", "outcome"]
        )
        .as_str(),
        Some("opened")
    );
    let revision = field(
        &parsed,
        &[
            "result",
            "value",
            "knowledge",
            "value",
            "opened",
            "journal_revision",
        ],
    )
    .as_str()
    .unwrap();
    let input_revision = field(
        &parsed,
        &[
            "result",
            "value",
            "knowledge",
            "value",
            "opened",
            "input_revision",
        ],
    )
    .as_str()
    .unwrap();
    let revision_prefix = format!("\"opened\":{{\"journal_revision\":\"{revision}\"");
    for damaged in [
        opened.replace("\"outcome\":\"opened\"", "\"outcome\":\"pending\""),
        opened.replace("\"round_opened\":true", "\"round_opened\":false"),
        opened.replace(
            &revision_prefix,
            &format!(
                "\"opened\":{{\"journal_revision\":\"{}\"",
                input.expected().journal_revision
            ),
        ),
        opened.replace(
            &format!("\"input_revision\":\"{input_revision}\",\"started_at\":"),
            &format!(
                "\"input_revision\":\"{}\",\"started_at\":",
                input.expected().input_revision - 1
            ),
        ),
        opened.replace("\"started_at\":\"1\"", "\"started_at\":\"0\""),
        opened.replace(
            "\"started_at\":\"1\"",
            &format!("\"started_at\":\"{}\"", input.expected().deadline.0),
        ),
        opened.replace("\"started_at\":\"1\"", "\"started_at\":\"01\""),
        opened.replacen(
            "\"knowledge\":",
            "\"error\":{\"code\":\"unavailable\"},\"knowledge\":",
            1,
        ),
    ] {
        assert_ne!(damaged, opened, "opened fixture mutation missed its target");
        assert!(validate_response(damaged.as_bytes(), &command).is_err());
    }
    held::finish_continue(&mut native.file);
    held::publish(&mut native.file);
    let published = held::image(&native.file);
    assert_eq!(native.dispatch(&command), opened);
    assert_eq!(held::image(&native.file), published);
    assert_eq!(
        native
            .file
            .rig
            .driver
            .supervisor()
            .host()
            .unwrap()
            .inspect()
            .executions,
        1
    );
}

#[test]
fn authenticated_plan_only_does_not_send_its_suggested_convene_action() {
    let root = Directory::new();
    let profile = settings(&root);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let query = next();
    let expected = query.clone();
    let server = std::thread::spawn(move || {
        let mut native = NativeConvene::new();
        let before = held::image(&native.file);
        let reads = native.file.source.status();
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        assert_eq!(
            decode_request(&read_document(&mut socket)).unwrap(),
            expected
        );
        let response = native.dispatch(&expected);
        socket
            .write_all(format!("{response}\n").as_bytes())
            .unwrap();
        assert_eq!(
            socket.read(&mut [0_u8]).unwrap(),
            0,
            "client automatically followed the held plan"
        );
        assert_eq!(native.grant.remaining_operations(), 2);
        assert_eq!(held::image(&native.file), before);
        assert_eq!(native.file.source.status(), reads);
        response
    });
    let mut output = Vec::new();
    operator_command(
        &args(&profile, &[root.document("plan.json", &query)]),
        &mut output,
    )
    .unwrap();
    assert_eq!(output, format!("{}\n", server.join().unwrap()).as_bytes());
}

#[test]
fn authenticated_explicit_convene_and_retry_queue_once_without_launching() {
    let root = Directory::new();
    let profile = settings(&root);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let (prepared, received) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut native = NativeConvene::new();
        let command = native.command();
        prepared.send(encode_request(&command).unwrap()).unwrap();
        let before = held::image(&native.file);
        let reads = native.file.source.status();
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        let mut replies = Vec::new();
        for _ in 0..2 {
            assert_eq!(
                decode_request(&read_document(&mut socket)).unwrap(),
                command
            );
            let response = native.dispatch(&command);
            validate_response(response.as_bytes(), &command).unwrap();
            socket
                .write_all(format!("{response}\n").as_bytes())
                .unwrap();
            replies.push(response);
        }
        assert_eq!(
            socket.read(&mut [0_u8]).unwrap(),
            0,
            "unexpected automatic retry or poll"
        );
        assert_eq!(replies[0], replies[1]);
        assert_eq!(native.grant.remaining_operations(), 1);
        assert_eq!(held::image(&native.file), before);
        assert_eq!(native.file.source.status(), reads);
        let state = native
            .file
            .rig
            .driver
            .supervisor()
            .host()
            .unwrap()
            .inspect();
        assert_eq!(state.control.decisions[&1], Consequence::HoldEffect);
        assert_eq!(state.executions, 0);
        replies
    });
    let command = decode_request(
        received
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .as_bytes(),
    )
    .unwrap();
    let documents = [
        root.document("convene.json", &command),
        root.document("explicit-retry.json", &command),
    ];
    let mut output = Vec::new();
    operator_command(&args(&profile, &documents), &mut output).unwrap();
    assert_eq!(
        output,
        (server.join().unwrap().join("\n") + "\n").as_bytes()
    );
}

#[test]
fn pending_receipt_relabelled_opened_emits_nothing_and_prevents_the_later_document() {
    let root = Directory::new();
    let profile = settings(&root);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let (prepared, received) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut native = NativeConvene::new();
        let command = native.command();
        prepared.send(encode_request(&command).unwrap()).unwrap();
        let before = held::image(&native.file);
        let reads = native.file.source.status();
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        assert_eq!(
            decode_request(&read_document(&mut socket)).unwrap(),
            command
        );
        let original = native.dispatch(&command);
        let forged = original.replace("\"outcome\":\"pending\"", "\"outcome\":\"opened\"");
        assert_ne!(forged, original);
        socket.write_all(format!("{forged}\n").as_bytes()).unwrap();
        assert_eq!(
            socket.read(&mut [0_u8]).unwrap(),
            0,
            "client continued after an invalid opened claim"
        );
        assert_eq!(held::image(&native.file), before);
        assert_eq!(native.file.source.status(), reads);
        assert_eq!(native.grant.remaining_operations(), 1);
    });
    let command = decode_request(
        received
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .as_bytes(),
    )
    .unwrap();
    let documents = [
        root.document("convene.json", &command),
        root.document("must-not-send.json", &command),
    ];
    let mut output = Vec::new();
    assert!(operator_command(&args(&profile, &documents), &mut output).is_err());
    assert!(output.is_empty());
    server.join().unwrap();
}
