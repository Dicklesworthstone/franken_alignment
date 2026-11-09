//! Real operator-profile transport, with grants fixed by the original server.
#[path = "operator/policy_replay.rs"]
mod policy_replay;
#[path = "operator/review_explanation.rs"]
mod review_explanation;

use super::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::oversight::actor::{
    ActorProposal, Knowledge as ActorKnowledge,
};
use fa_reference::action::consequence::oversight::actor_peer::PeerCredentials;
use fa_reference::address_resolver::{CallerScope, ResolverPlane};
use fa_reference::observation_algebra::AuthorityKind;
use fa_reference::tower::publication::PublicationTower;
use fa_reference::tower::{ReadArguments, TowerAccess};
use std::collections::BTreeSet;
use std::fs;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "../../../../../../../tests/support/file_oversight_actor.rs"]
mod native;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
const GENERATION: u64 = 11;

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(crate::test_directory::create(format!(
            "fa-op-client-{}-{stamp}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        )))
    }

    fn socket(&self) -> PathBuf {
        self.0.join("operator.sock")
    }

    fn settings(&self, supervisor_pid: u32, exchanges: u64) -> PathBuf {
        let credentials = credentials();
        let scope = native::profile().delivery.scope;
        let document = format!(
            concat!(
                "{{\"schema\":\"fa.tower-operator-service/1\",",
                "\"scope\":{{\"tenant\":{},\"principal\":{},\"run\":{},\"branch\":{},\"authority\":{}}},",
                "\"socket\":\"{}\",",
                "\"supervisor\":{{\"uid\":{},\"gid\":{},\"pid\":{}}},",
                "\"operator\":{{\"uid\":{},\"gid\":{},\"pid\":{}}},",
                "\"candidate_limit\":4,\"connection_limit\":4,\"exchange_limit\":{},",
                "\"runtime_ms\":10000,\"poll_ms\":1,\"inspection_queries\":3,",
                "\"explanation_queries\":2,\"explanation_replay_events\":1024,",
                "\"explanation_disclosure\":\"policy_and_selected_witnesses\"}}"
            ),
            scope.tenant,
            scope.principal,
            scope.run,
            scope.branch,
            scope.authority,
            self.socket().display(),
            credentials.uid(),
            credentials.gid(),
            supervisor_pid,
            credentials.uid(),
            credentials.gid(),
            credentials.pid(),
            exchanges,
        );
        let path = self.0.join("operator.json");
        fs::write(&path, document).unwrap();
        path
    }

    fn document(&self, name: &str, request: &TowerRequest) -> PathBuf {
        let path = self.0.join(name);
        fs::write(
            &path,
            encode_request(request).unwrap().replace(",\"", ",\n\""),
        )
        .unwrap();
        path
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("operator client fixture cleanup: {error}");
        }
    }
}

fn credentials() -> PeerCredentials {
    let (socket, _peer) = UnixStream::pair().unwrap();
    PeerCredentials::observe(&socket).unwrap()
}

fn args(profile: &Path, documents: &[PathBuf]) -> Vec<String> {
    [
        "tower-operator-exchange".to_owned(),
        profile.display().to_string(),
    ]
    .into_iter()
    .chain(documents.iter().map(|path| path.display().to_string()))
    .collect()
}

fn bounded_socket(listener: &UnixListener) -> UnixStream {
    let (socket, _) = listener.accept().unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    socket
        .set_write_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    socket
}

fn read_document(socket: &mut UnixStream) -> Vec<u8> {
    let mut document = Vec::new();
    let mut byte = [0_u8; 1];
    loop {
        socket.read_exact(&mut byte).unwrap();
        if byte[0] == b'\n' {
            return document;
        }
        assert!(document.len() < MAX_REQUEST_BYTES);
        document.push(byte[0]);
    }
}

fn mount() -> Address {
    let scope = native::profile().delivery.scope;
    Address::new(
        scope.tenant.to_string(),
        Noun::Run,
        scope.run.to_string(),
        Some(GENERATION),
    )
    .unwrap()
}

#[test]
fn operator_profile_exchanges_original_pages_without_authority_gain_or_automatic_following() {
    let root = Directory::new();
    let profile = root.settings(credentials().pid(), 8);
    let mut proposed = TowerRequest::new("operator-proposal", mount(), Verb::Propose);
    proposed.idempotency_key = Some("901".into());
    proposed.arguments = ReadArguments::Propose {
        request: 901,
        proposal: ActorProposal {
            target: native::profile().delivery.target,
            payload: b"caller".to_vec(),
            units: 16,
            deadline: ElapsedTick(100),
            expected_policy_epoch: 0,
        },
    };
    let situation = TowerRequest::new("operator-situation", mount(), Verb::Situation);
    let scope = native::profile().delivery.scope;
    let explain = TowerRequest::new(
        "operator-explanation",
        Address::new(
            scope.tenant.to_string(),
            Noun::Attempt,
            format!(
                "request-{}-{}-{}-{}-1001",
                scope.run, scope.principal, scope.branch, scope.authority
            ),
            Some(GENERATION),
        )
        .unwrap(),
        Verb::Explain,
    );
    let documents = [
        root.document("proposal.json", &proposed),
        root.document("situation.json", &situation),
        root.document("explain.json", &explain),
    ];
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        // The server independently owns and grants the native capabilities.
        // The client's requested explanation disclosure is deliberately absent.
        let storage = native::Directory::new();
        let mut config = native::profile();
        config.delivery.max_attempts = 17;
        let (mut host, _reviewer) = FileOversight::create(storage.store(), config).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let proposal = native::proposal(&native::oversight::spec(&host, b"published"));
        let (actor, mut supervisor) = host.into_actor_gateway();
        for request in 1001..=1017 {
            native::observe(&mut supervisor);
            actor.submit(request, &proposal).unwrap();
        }
        let before = supervisor.host().unwrap().inspect();
        let (inspection, grant) = supervisor.inspection_port(3).unwrap();
        let mut tower =
            PublicationTower::new(mount(), actor, PublicationLimits::default()).unwrap();
        tower.set_inspection(Some(inspection)).unwrap();
        let access = TowerAccess::new(
            CallerScope {
                caller_tenant: scope.tenant.to_string(),
                allowed_planes: BTreeSet::from([ResolverPlane::ControlLedger]),
                granted_authorities: BTreeSet::from([AuthorityKind::Verify]),
            },
            true,
            Vec::new(),
            0,
        );
        let mut socket = bounded_socket(&listener);
        let mut replies = Vec::new();
        for index in 0..3 {
            let bytes = read_document(&mut socket);
            let request = decode_request(&bytes).unwrap();
            let response = tower.exchange(&access, &bytes);
            match index {
                0 => {
                    assert_eq!(request, proposed);
                    assert_eq!(
                        response.envelope.reason_code,
                        Some(ReasonCode::PurposeMismatch)
                    );
                }
                1 => {
                    assert_eq!(request, situation);
                    let PublicationResult::Situation(Ok(ActorKnowledge::Known { value, .. })) =
                        &response.result
                    else {
                        panic!("expected the original native situation page");
                    };
                    assert_eq!(value.summary().attempts, 17);
                    assert_eq!(value.attempts().len(), 16);
                    assert!(!value.enumeration_complete());
                    assert!(value.continuation().is_some());
                }
                _ => {
                    assert_eq!(request, explain);
                    assert_eq!(
                        response.envelope.reason_code,
                        Some(ReasonCode::CapabilityUnsupported)
                    );
                }
            }
            assert_eq!(supervisor.host().unwrap().inspect(), before);
            let response = response.to_json().unwrap();
            socket.write_all(response.as_bytes()).unwrap();
            socket.write_all(b"\n").unwrap();
            socket.flush().unwrap();
            replies.push(response);
        }
        assert_eq!(
            socket.read(&mut [0_u8; 1]).unwrap(),
            0,
            "unexpected extra query"
        );
        assert_eq!(
            grant.remaining_queries(),
            2,
            "client followed a page automatically"
        );
        replies
    });
    let mut output = Vec::new();
    operator_command(&args(&profile, &documents), &mut output).unwrap();
    let original = server.join().unwrap().join("\n") + "\n";
    assert_eq!(output, original.as_bytes());
}

#[test]
fn operator_server_identity_is_verified_before_any_request_bytes() {
    let root = Directory::new();
    let wrong_pid = credentials().pid().checked_add(1).unwrap();
    let profile = root.settings(wrong_pid, 4);
    let document = root.document("request.json", &request());
    let listener = UnixListener::bind(root.socket()).unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut output = Vec::new();
    assert!(operator_command(&args(&profile, &[document]), &mut output).is_err());
    assert!(output.is_empty());
    let mut socket = bounded_socket(&listener);
    assert_eq!(socket.read(&mut [0_u8; 1]).unwrap(), 0);
}

#[test]
fn later_malformed_operator_document_and_excess_exchange_count_refuse_before_connection() {
    for excessive in [false, true] {
        let root = Directory::new();
        let profile = root.settings(credentials().pid(), if excessive { 1 } else { 4 });
        let first = root.document("first.json", &request());
        let second = root.0.join("second.json");
        fs::write(&second, b"{\"operator\":true,\"schema_version\":").unwrap();
        let listener = UnixListener::bind(root.socket()).unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut output = Vec::new();
        let error = operator_command(&args(&profile, &[first, second]), &mut output).unwrap_err();
        assert!(error.contains(if excessive {
            "exchange allowance"
        } else {
            "invalid bounded Tower"
        }));
        assert!(output.is_empty());
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }
}

#[test]
fn operator_output_failure_never_sends_the_next_explicit_document() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            panic!("failed output cannot flush");
        }
    }
    let root = Directory::new();
    let profile = root.settings(credentials().pid(), 4);
    let first = request();
    let mut later = first.clone();
    later.operation_id = "later-explicit-request".into();
    let documents = [
        root.document("first.json", &first),
        root.document("later.json", &later),
    ];
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let mut socket = bounded_socket(&listener);
        assert_eq!(decode_request(&read_document(&mut socket)).unwrap(), first);
        socket.write_all(response(&first).as_bytes()).unwrap();
        socket.write_all(b"\n").unwrap();
        socket.flush().unwrap();
        assert_eq!(socket.read(&mut [0_u8; 1]).unwrap(), 0);
    });
    let error = operator_command(&args(&profile, &documents), &mut Broken).unwrap_err();
    assert!(error.contains("output failed"));
    server.join().unwrap();
}

#[test]
fn operator_cli_owns_its_grammar_and_rejects_actor_profiles_without_connecting() {
    for values in [
        vec!["tower-operator-exchange"],
        vec!["tower-operator-exchange", "missing-profile"],
        vec![
            "tower-operator-exchange",
            "missing-profile",
            "request",
            "--tower-generation",
            "11",
        ],
        vec![
            "tower-operator-exchange",
            "missing-profile",
            "request",
            "--credibility-activation",
            "private",
        ],
        vec!["tower-exchange", "missing-profile", "request"],
    ] {
        let args: Vec<_> = values.into_iter().map(str::to_owned).collect();
        let mut output = Vec::new();
        let error = operator_command(&args, &mut output).unwrap_err();
        assert_eq!(error, OPERATOR_USAGE);
        assert!(output.is_empty());
    }
    let root = Directory::new();
    let profile = root.0.join("actor-profile.json");
    fs::write(&profile, b"{\"schema\":\"fa.actor-service/1\"}").unwrap();
    let document = root.document("request.json", &request());
    let listener = UnixListener::bind(root.socket()).unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut output = Vec::new();
    assert!(operator_command(&args(&profile, &[document]), &mut output).is_err());
    assert!(output.is_empty());
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
}
