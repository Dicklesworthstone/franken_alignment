//! Selected receipts use one original owner and one authenticated peer session.
//! Only transport scheduling is injected; all request dispositions are native.
use super::*;
use fa_reference::action::ActionSpec;
use fa_reference::action::consequence::delivery::persistent::requests::actor::MAX_RECOVERY_REQUESTS;
use fa_reference::action::consequence::oversight::actor_wire::Command;
use fa_reference::action::consequence::oversight::actor_wire::client::{
    ActorExchange, ClientIoBudget, ClientIoLimits,
};

struct PeerScript<'a> {
    actor: &'a Profile,
    commands: Vec<Command>,
    reconnect_after: Option<usize>,
    active: Option<ActorExchange<UnixStream>>,
    stream: Option<UnixStream>,
    budget: ClientIoBudget,
    replies: Vec<WireResponse>,
    waits: usize,
}
impl<'a> PeerScript<'a> {
    fn new(actor: &'a Profile, commands: Vec<Command>, reconnect_after: Option<usize>) -> Self {
        Self {
            actor,
            commands,
            reconnect_after,
            active: None,
            stream: None,
            budget: ClientIoBudget::new(ClientIoLimits::default()).unwrap(),
            replies: Vec::new(),
            waits: 0,
        }
    }

    fn step(&mut self, done: &Cell<bool>) {
        self.waits += 1;
        assert!(self.waits < self.commands.len() * 32, "receipt peer stopped making progress");
        if self.active.is_none() {
            let socket = self.stream.take().unwrap_or_else(|| {
                let socket = UnixStream::connect(&self.actor.socket).unwrap();
                self.actor.supervisor.verify(&socket).unwrap();
                socket.set_nonblocking(true).unwrap();
                socket
            });
            self.active = Some(ActorExchange::new(
                socket,
                self.commands[self.replies.len()].clone(),
                &mut self.budget,
            ).unwrap());
        }
        if let ClientProgress::Response(response) = self.active.as_mut().unwrap()
            .step(&mut self.budget).unwrap()
        {
            self.replies.push(response);
            let socket = self.active.take().unwrap().into_stream().unwrap();
            if self.reconnect_after == Some(self.replies.len()) {
                drop(socket);
            } else {
                self.stream = Some(socket);
            }
            if self.replies.len() == self.commands.len() {
                done.set(true);
            }
        }
    }
}

fn submit(request: u64, proposal: &ActorProposal) -> Command {
    Command::Submit { request, proposal: proposal.clone() }
}

fn retain_refusal(host: &mut FileOversight, key: u64, payload: &[u8]) -> ActorProposal {
    let mut original = proposal(host);
    original.payload = payload.to_vec();
    original.expected_policy_epoch = 99;
    let spec = ActionSpec {
        version: 1,
        scope: fixture::profile().delivery.scope,
        target: Some(original.target),
        payload: original.payload.clone(),
        required_witnesses: Vec::new(),
        policy_epoch: original.expected_policy_epoch,
        deadline: original.deadline,
        units: original.units,
    };
    host.submit_request(host.revision(), key, spec, snapshot()).unwrap();
    original
}

fn assert_refused(response: &WireResponse) {
    assert!(matches!(
        response.result,
        Ok(Knowledge::Known { value: ActorOutcome::NotAdmitted, .. })
    ));
}

fn assert_hidden(response: &WireResponse) {
    assert_eq!(
        response.result,
        Ok(Knowledge::Withheld { authority_required: "own_request" })
    );
}

#[test]
fn one_selected_peer_recovers_multiple_liabilities_and_retains_tickets_across_reconnect() {
    for phase in 0..4 {
        let root = directory();
        let (mut host, first) = authorized(&root, phase);
        let second = retain_refusal(&mut host, 701, b"second retained refusal");
        let outside = retain_refusal(&mut host, 702, b"outside retained refusal");
        let actor = actor(&root, fixture::profile().delivery.scope);
        let before = host.inspect();
        let bytes = fs::read(root.store().join("delivery.bin")).unwrap();
        let mut changed = second.clone();
        changed.deadline.0 += 1;
        let commands = vec![
            Command::Poll { request: 700 },
            submit(701, &second),
            submit(700, &first),
            submit(702, &outside),
            submit(704, &first),
            submit(701, &changed),
            Command::Poll { request: 701 },
            // The same PeerSession keeps original tickets across a newly
            // authenticated connection; it does not recreate its owner.
            Command::Poll { request: 701 },
            submit(700, &first),
            submit(701, &second),
            Command::Cancel { request: 700 },
            Command::Cancel { request: 701 },
            Command::Poll { request: 700 },
        ];
        let mut script = PeerScript::new(&actor, commands, Some(7));
        let done = Cell::new(false);
        let socket = BoundSocket::bind(&actor.socket, None).unwrap();
        actor.secure_socket().unwrap();
        let report = serve_owner_selected(
            host,
            &actor,
            socket,
            &[700, 701, 704],
            Duration::from_millis(100),
            &mut || if done.get() { Duration::from_millis(100) } else { Duration::ZERO },
            &mut |_| script.step(&done),
        ).unwrap();
        assert_eq!(report.frames, script.commands.len());
        assert_eq!(report.candidates, 2);
        let responses = &script.replies;
        assert_hidden(&responses[0]);
        for index in [1, 6, 7, 9, 11] {
            assert_refused(&responses[index]);
        }
        assert_eq!(responses[3].result, Err(WireError::Withheld));
        assert_eq!(responses[4].result, Err(WireError::Withheld));
        assert_eq!(responses[5].result, Err(WireError::IdempotencyConflict));
        assert_eq!(responses[2], responses[8]);
        assert_eq!(responses[10], responses[12]);
        let after = FileOversight::read_publication(root.store(), &fixture::profile()).unwrap();
        if phase == 0 {
            assert_eq!(responses[2].result, Ok(Knowledge::Pending { request: 700 }));
            assert!(matches!(responses[10].result, Ok(Knowledge::Known {
                value: ActorOutcome::CancelledBeforeDispatch, ..
            })));
            assert_eq!(before.control.ledger.reserved, 16);
            assert_eq!(after.control.ledger.reserved, 0);
            assert_eq!(after.control.ledger.charged, 0);
            assert_eq!(after.executions, 0);
        } else {
            assert_eq!(after, before);
            assert_eq!(fs::read(root.store().join("delivery.bin")).unwrap(), bytes);
            assert_eq!(after.control.ledger.charged, 16);
            if phase < 3 {
                assert_eq!(responses[10].result, Ok(Knowledge::Unknown {
                    reason: UnknownReason::OutcomeUnknown,
                }));
            } else {
                assert!(matches!(responses[10].result, Ok(Knowledge::Known {
                    value: ActorOutcome::Executed, ..
                })));
            }
        }
        assert_eq!(report.revision, after.revision);
        assert!(!actor.socket.exists());
        let (reopened, _) = FileOversight::open(root.store(), fixture::profile()).unwrap();
        assert_eq!(reopened.retained_requests(), 3);
        assert!(reopened.request_action(704).is_err());
    }
}

#[test]
fn switching_selected_keys_cannot_renew_the_connection_allowance_or_service_window() {
    for expire in [false, true] {
        let root = directory();
        let (mut host, first) = authorized(&root, 3);
        let second = retain_refusal(&mut host, 701, b"another original receipt");
        let mut actor = actor(&root, fixture::profile().delivery.scope);
        actor.connections = 1;
        let before = host.inspect();
        let bytes = fs::read(root.store().join("delivery.bin")).unwrap();
        let mut script = PeerScript::new(&actor, vec![
            submit(701, &second),
            submit(700, &first),
        ], Some(1));
        let done = Cell::new(false);
        let tick = Cell::new(0);
        let socket = BoundSocket::bind(&actor.socket, None).unwrap();
        actor.secure_socket().unwrap();
        let result = serve_owner_selected(
            host,
            &actor,
            socket,
            &[700, 701],
            Duration::from_millis(100),
            &mut || Duration::from_millis(tick.get()),
            &mut |_| {
                script.step(&done);
                if expire && script.replies.len() == 1 {
                    tick.set(100);
                }
            },
        );
        assert_eq!(script.replies.len(), 1);
        assert_refused(&script.replies[0]);
        if expire {
            let report = result.unwrap();
            assert_eq!(report.frames, 1);
            assert_eq!(report.candidates, 1);
        } else {
            assert!(result.is_err(), "another selected key cannot buy a second connection");
        }
        assert_eq!(FileOversight::read_publication(root.store(), &fixture::profile()).unwrap(), before);
        assert_eq!(fs::read(root.store().join("delivery.bin")).unwrap(), bytes);
        assert!(!actor.socket.exists());
    }
}

fn file_configuration(root: &Directory) -> Config {
    let text = include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/fixtures/supervised_publication.json"))
        .replace("/operator/publication-store", root.store().to_str().unwrap())
        .replace("/operator/evidence.json", root.0.join("missing-evidence.json").to_str().unwrap())
        .replace("/operator/helpers", root.0.join("missing-helpers").to_str().unwrap());
    fs::write(root.0.join("config.json"), text).unwrap();
    Config::read(&root.0.join("config.json")).unwrap()
}

fn write_actor(root: &Directory, actor: &Profile) {
    let scope = actor.scope;
    let identity = format!(
        r#"{{"uid":{},"gid":{},"pid":{}}}"#,
        actor.actor.uid(), actor.actor.gid(), actor.actor.pid().unwrap(),
    );
    fs::write(root.0.join("actor.json"), format!(
        r#"{{"schema":"fa.actor-service/1","clock":"unix_milliseconds","request":{},"scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},"socket":"{}","supervisor":{},"actor":{},"candidate_limit":{},"connection_limit":{},"exchange_limit":{},"runtime_ms":{},"poll_ms":{},"reply_ms":{}}}"#,
        actor.request, scope.tenant, scope.principal, scope.run, scope.branch, scope.authority,
        actor.socket.display(), identity, identity, actor.candidates, actor.connections,
        actor.exchanges, actor.runtime_ms, actor.poll_ms, actor.reply_ms,
    )).unwrap();
}

fn witness_profile(root: &Directory) -> PublicationProfile {
    let text = format!(
        r#"{{"schema":"fa.supervised-witnesses/1","source":91,"original":"{}","current":"{}","limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[{{"kind":"exact_value","key":7,"role":"subject"}}]}}"#,
        root.0.join("missing-original.bin").display(),
        root.0.join("missing-current.bin").display(),
    );
    fs::write(root.0.join("witness.json"), &text).unwrap();
    PublicationProfile::decode(text.as_bytes()).unwrap()
}

fn arguments(root: &Directory, checked: bool, selection: &str) -> Vec<String> {
    let mut args = vec![
        if checked { "serve-open-checked" } else { "serve-open" }.to_owned(),
        root.0.join("config.json").display().to_string(),
        root.0.join("actor.json").display().to_string(),
    ];
    if checked {
        args.push(root.0.join("witness.json").display().to_string());
    }
    args.extend(["--receipts-only".to_owned(), "--requests".to_owned(), selection.to_owned()]);
    args
}

#[test]
fn selected_plain_and_checked_cli_open_only_retrieves_original_receipts_without_live_assets() {
    for checked in [false, true] {
        let root = directory();
        let config = file_configuration(&root);
        let actor = actor(&root, config.profile.delivery.scope);
        write_actor(&root, &actor);
        let publication = checked.then(|| witness_profile(&root));
        let (mut host, _) = prepare_host(&config, publication.as_ref(), false).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let first = retain_refusal(&mut host, 700, b"first retained refusal");
        let second = retain_refusal(&mut host, 701, b"second retained refusal");
        let outside = retain_refusal(&mut host, 702, b"excluded retained refusal");
        let before = host.inspect();
        drop(host);
        let done = Cell::new(false);
        let mut script = PeerScript::new(&actor, vec![
            submit(701, &second),
            submit(700, &first),
            submit(702, &outside),
            submit(704, &first),
            Command::Cancel { request: 701 },
            Command::Poll { request: 700 },
        ], None);
        let report = command_with(
            &arguments(&root, checked, "700,701,704"),
            None,
            &mut || if done.get() { Duration::from_millis(100) } else { Duration::ZERO },
            &mut |_| script.step(&done),
        ).unwrap();
        for index in [0, 1, 4, 5] {
            assert_refused(&script.replies[index]);
        }
        assert_eq!(script.replies[2].result, Err(WireError::Withheld));
        assert_eq!(script.replies[3].result, Err(WireError::Withheld));
        assert_eq!(report.frames, 6);
        assert_eq!(report.candidates, 1);
        assert_eq!(report.revision, before.revision + 1, "only original recovery Fence appends");
        let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(after.revision, report.revision);
        assert_eq!(after.executions, 0);
        assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
        assert_eq!(after.control.ledger.reserved, before.control.ledger.reserved);
        assert!(!actor.socket.exists());
        for absent in ["missing-evidence.json", "missing-helpers", "missing-original.bin", "missing-current.bin"] {
            assert!(!root.0.join(absent).exists());
        }
        let (reopened, _) = prepare_host(&config, publication.as_ref(), true).unwrap();
        assert_eq!(reopened.retained_requests(), 3);
        assert!(reopened.request_action(704).is_err());
    }
}

#[test]
fn invalid_selected_cli_never_opens_the_original_store_or_creates_a_socket() {
    let root = directory();
    let config = file_configuration(&root);
    let actor = actor(&root, config.profile.delivery.scope);
    write_actor(&root, &actor);
    let (host, _) = prepare_host(&config, None, false).unwrap();
    drop(host);
    let before = fs::read(config.store.join("delivery.bin")).unwrap();
    let too_many = (700..700 + MAX_RECOVERY_REQUESTS as u64 + 1)
        .map(|key| key.to_string()).collect::<Vec<_>>().join(",");
    for selection in ["", "0", "700,0", "700,700", "700,,701", "701,700", too_many.as_str()] {
        assert!(command_with(
            &arguments(&root, false, selection),
            None,
            &mut || panic!("invalid selected identity reached service scheduling"),
            &mut |_| panic!("invalid selected identity reached peer work"),
        ).is_err(), "selection={selection:?}");
        assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), before);
        assert!(!actor.socket.exists());
    }
    let mut unsupported = arguments(&root, false, "700,701");
    unsupported.extend(["--operator-profile".to_owned(), "/missing/operator.json".to_owned()]);
    assert!(command_with(
        &unsupported,
        None,
        &mut || panic!("unsupported authority reached service scheduling"),
        &mut |_| panic!("unsupported authority reached peer work"),
    ).is_err());
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), before);
}
