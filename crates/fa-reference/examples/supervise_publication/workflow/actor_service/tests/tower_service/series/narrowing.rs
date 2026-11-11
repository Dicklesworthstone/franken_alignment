//! Real child helpers, independent human sockets and ordinary scheduled services
//! under a separately authenticated target restriction. The helper verdicts are
//! synthetic fixtures and make no model-quality qualification claim.

use super::*;
use super::super::operator::{settings_document, store_files};
use crate::workflow::actor_service::operator::Settings;
use fa_reference::action::{ResolvedTarget, Scope};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileNarrowBasis, MAX_NARROW_REPLAY_EVENTS,
};
use fa_reference::address_resolver::Address;
use fa_reference::tower::{ReadArguments, TowerRequest, Verb, encode_request};

fn settings(root: &Directory, config: &Config) -> Settings {
    Settings::decode(settings_document(root, config, 0, 0).replacen('{', &format!(
        "{{\"narrowing\":{{\"queries\":2,\"operations\":1,\"max_replay_events\":{MAX_NARROW_REPLAY_EVENTS},\"authority\":\"narrow_original_targets\"}},"
    ), 1).as_bytes()).unwrap()
}

fn connect_operator(settings: &Settings) -> UnixStream {
    wait(&settings.socket);
    let socket = UnixStream::connect(&settings.socket).unwrap();
    settings.supervisor.verify(&socket).unwrap();
    socket
}

fn next(scope: Scope, operation: u64, retained_targets: Vec<ResolvedTarget>) -> String {
    let mut request = TowerRequest::new(
        format!("narrow-plan-{operation}"),
        Address::parse(&format!("fa://{}/run/{}@{GENERATION}", scope.tenant, scope.run)).unwrap(),
        Verb::Next,
    );
    request.arguments = ReadArguments::NarrowingNext { operation, retained_targets };
    encode_request(&request).unwrap()
}

fn known<'a>(response: &'a Json, kind: &str) -> &'a Json {
    let result = field(response, "result");
    assert_eq!(field(result, "kind").as_str(), Some(kind));
    let knowledge = field(field(result, "value"), "knowledge");
    assert_eq!(field(knowledge, "state").as_str(), Some("known"));
    knowledge
}

fn number(value: &Json) -> u64 {
    value.as_str().unwrap().parse().unwrap()
}

fn action(scope: Scope, response: &Json, retained_targets: Vec<ResolvedTarget>) -> String {
    let plan = field(known(response, "narrowing-next"), "value");
    assert_eq!(field(plan, "can_narrow").as_bool(), Some(true));
    let action = field(plan, "action");
    let arguments = field(action, "arguments");
    let basis = field(arguments, "expected");
    let mut request = TowerRequest::new(
        field(action, "operation_id").as_str().unwrap(),
        Address::parse(field(action, "address").as_str().unwrap()).unwrap(),
        Verb::Narrow,
    );
    request.idempotency_key = Some(field(action, "idempotency_key").as_str().unwrap().to_owned());
    request.arguments = ReadArguments::Narrow {
        operation: number(field(arguments, "operation")),
        expected: FileNarrowBasis {
            scope,
            journal_revision: number(field(basis, "journal_revision")),
            control_sequence: number(field(basis, "control_sequence")),
            authority_epoch: number(field(basis, "authority_epoch")),
            recorded_at: basis.get("recorded_at").filter(|tick| !tick.is_null())
                .map(|tick| ElapsedTick(number(tick))),
        },
        retained_targets,
    };
    let document = encode_request(&request).unwrap();
    assert_eq!(&parse(document.as_bytes()), action);
    document
}

fn checked(root: &Directory, config: &Config) -> PublicationProfile {
    use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::{
        FilePublicationInputs,
        producer::{FilePublicationProducer, PublicationProducerProfile},
    };
    use fa_reference::full_input::{
        ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart,
    };
    let actual = ActualHelperInput::new(
        b"Q".to_vec(),
        InputProfileBinding { profile_id: 1, profile_bytes: Vec::new(), tokenizer_epoch: 1,
            policy_epoch: 0, model_epoch: 1 },
        vec![SubmittedPart { span: ByteSpan { start: 0, end: 1 }, kind: PartKind::Question }],
        Vec::new(),
    ).unwrap();
    let (_producer, _) = FilePublicationProducer::create(
        root.0.join("producer"),
        PublicationProducerProfile { source: 91, scope: config.profile.delivery.scope,
            feed: 41, clock_domain: CLOCK_DOMAIN, after: 0 },
        FilePublicationInputs::new(None, Some(actual)), ElapsedTick(1000),
    ).unwrap();
    PublicationProfile::decode(format!(
        r#"{{"schema":"fa.supervised-whole-input/1","source":91,"producer":{{"path":"{}/producer/delivery.bin","scope":{}}},"feed":{{"source":41,"after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}},"limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[]}}"#,
        root.0.display(), scope(config),
    ).as_bytes()).unwrap()
}

#[test]
fn plain_and_checked_services_narrow_before_intake_then_complete_original_review_and_publication() {
    for with_checked in [false, true] {
        let root = Directory::new();
        let config = configured(&root);
        evidence(&root, &config);
        let source = root.0.join("evidence.json");
        let source_bytes = fs::read(&source).unwrap();
        fs::remove_file(&source).unwrap();
        let (mut actor, reviewer) = profiles(&root, &config);
        actor.reply_ms = 1000;
        let settings = settings(&root, &config);
        let publication = with_checked.then(|| checked(&root, &config));
        let scope = config.profile.delivery.scope;
        let target = config.profile.delivery.target;
        let first = document(&config, 7, 1, "6669727374");
        let first_poll = poll(&config, 7);
        let store = config.store.clone();
        let bootstrap = config.profile.clone();
        let peer_store = store.clone();
        let peer_bootstrap = bootstrap.clone();
        let actor_peer = actor.clone();
        let operator_peer = settings.clone();
        let peer = std::thread::spawn(move || {
            let mut actor = connect(&actor_peer);
            let mut operator = connect_operator(&operator_peer);
            let before = FileOversight::read_publication(&peer_store, &peer_bootstrap).unwrap();
            let files = store_files(&peer_store);
            let plan = raw_exchange(&mut operator, &next(scope, 801, vec![target]));
            assert_eq!(store_files(&peer_store), files);
            let command = action(scope, &plan, vec![target]);
            let narrowed = raw_exchange(&mut operator, &command);
            let receipt = field(known(&narrowed, "narrowing"), "value");
            assert_eq!(field(receipt, "refunded_units").as_str(), Some("0"));
            assert!(field(receipt, "cancelled").as_array().unwrap().is_empty());
            let after = FileOversight::read_publication(&peer_store, &peer_bootstrap).unwrap();
            assert_eq!(after.revision, before.revision + 1);
            assert_eq!(after.control.sequence, before.control.sequence + 1);
            assert_eq!(after.control.ledger.epoch, before.control.ledger.epoch);
            assert_eq!(after.control.ledger.elapsed, before.control.ledger.elapsed);
            assert!(after.control.ledger.stages.is_empty());
            assert_eq!(after.executions, 0);
            assert!(!source.exists());
            fs::write(&source, source_bytes).unwrap();
            assert!(executed(&finish(&mut actor, &first, &first_poll)));
            let files = store_files(&peer_store);
            assert_eq!(raw_exchange(&mut operator, &command), narrowed);
            assert_eq!(store_files(&peer_store), files);
        });
        let human = human_for(reviewer.clone(), 7);
        let result = service::serve_tower(
            config, &actor, &reviewer, publication.as_ref(),
            Options { requests: &[7], open: false, credibility: None },
            GENERATION, Some(&settings), || ElapsedTick(1000),
        );
        peer.join().unwrap();
        human.join().unwrap();
        assert!(result.is_ok(), "checked={with_checked}: {result:?}");
        let after = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!((after.executions, after.payload.as_slice()), (1, b"first".as_slice()));
        assert_eq!(after.control.ledger.stages[&1], ActionState::Confirmed);
        assert_eq!((after.control.ledger.charged, after.control.ledger.reserved), (7, 0));
        assert_eq!(after.control.ledger.epoch, 0);
        assert!(after.stop.is_none());
        assert!(!actor.socket.exists());
        assert!(!settings.socket.exists());
        assert!(!reviewer.socket(7).exists());
    }
}

#[test]
fn narrowing_excluded_active_review_retires_without_stop_and_schedule_keeps_the_empty_ceiling() {
    for with_checked in [false, true] {
        let root = Directory::new();
        let config = configured(&root);
        evidence(&root, &config);
        let (mut actor, reviewer) = profiles(&root, &config);
        actor.reply_ms = 1000;
        let settings = settings(&root, &config);
        let publication = with_checked.then(|| checked(&root, &config));
        let scope = config.profile.delivery.scope;
        let target = config.profile.delivery.target;
        let first = document(&config, 7, 1, "6669727374");
        let first_poll = poll(&config, 7);
        let second = document(&config, 8, 1, "7365636f6e64");
        let second_poll = poll(&config, 8);
        let human_socket = reviewer.socket(7);
        let second_human = reviewer.socket(8);
        let second_control = crate::workflow::control::socket_path(&reviewer, 8);
        let store = config.store.clone();
        let bootstrap = config.profile.clone();
        let peer_store = store.clone();
        let peer_bootstrap = bootstrap.clone();
        let actor_peer = actor.clone();
        let operator_peer = settings.clone();
        let peer = std::thread::spawn(move || {
            let mut actor = connect(&actor_peer);
            let first_reply = raw_exchange(&mut actor, &first);
            assert_eq!(field(knowledge(&first_reply), "state").as_str(), Some("pending"));
            // The real helper cohort finished; the driver waits for a distinct
            // human peer. No reviewer connects or creates a human decision.
            wait(&human_socket);
            let mut operator = connect_operator(&operator_peer);
            let before = FileOversight::read_publication(&peer_store, &peer_bootstrap).unwrap();
            // The driver is AwaitingDispatch locally, but reserves automatic
            // units only after the independent human approval arrives.
            assert_eq!(before.control.ledger.stages[&1], ActionState::Reviewing);
            assert_eq!(before.control.ledger.reserved, 0);
            let plan = raw_exchange(&mut operator, &next(scope, 802, Vec::new()));
            let command = action(scope, &plan, Vec::new());
            let response = raw_exchange(&mut operator, &command);
            let receipt = field(known(&response, "narrowing"), "value");
            assert_eq!(field(receipt, "cancelled").as_array().unwrap(), &[Json::String("1".into())]);
            assert_eq!(field(receipt, "refunded_units").as_str(), Some("0"));
            let cancelled = raw_exchange(&mut actor, &first_poll);
            assert_eq!(field(knowledge(&cancelled), "state").as_str(), Some("known"));
            assert_eq!(field(knowledge(&cancelled), "value").as_str(), Some("cancelled_before_dispatch"));
            // The new Stop listener appears only after original helper custody
            // has been retired and the same schedule advances to request 8.
            wait(&second_control);
            assert!(!human_socket.exists());
            assert_eq!(raw_exchange(&mut operator, &command), response);
            let blocked = raw_exchange(&mut operator, &next(scope, 803, vec![target]));
            let plan = field(known(&blocked, "narrowing-next"), "value");
            assert_eq!(field(plan, "block_reason").as_str(), Some("operation_limit"));
            assert!(field(plan, "effective_targets").as_array().unwrap().is_empty());
            let second = finish(&mut actor, &second, &second_poll);
            assert_eq!(field(knowledge(&second), "state").as_str(), Some("known"));
            assert_eq!(field(knowledge(&second), "value").as_str(), Some("not_admitted"));
            assert!(!second_human.exists());
        });
        let result = service::serve_tower(
            config, &actor, &reviewer, publication.as_ref(),
            Options { requests: &[7, 8], open: false, credibility: None },
            GENERATION, Some(&settings), || ElapsedTick(1000),
        );
        peer.join().unwrap();
        assert!(result.is_ok(), "checked={with_checked}: {result:?}");
        let after = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(after.control.ledger.stages[&1], ActionState::Cancelled);
        assert_eq!(after.control.ledger.stages.len(), 1);
        assert_eq!(after.control.ledger.available, bootstrap.delivery.total);
        assert_eq!((after.control.ledger.charged, after.control.ledger.reserved), (0, 0));
        assert_eq!(after.control.ledger.epoch, 0);
        assert_eq!(after.executions, 0);
        assert!(after.stop.is_none());
        assert!(!actor.socket.exists());
        assert!(!settings.socket.exists());
        assert!(!reviewer.socket(7).exists());
        assert!(!reviewer.socket(8).exists());
    }
}
