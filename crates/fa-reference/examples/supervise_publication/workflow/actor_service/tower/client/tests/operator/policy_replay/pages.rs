//! The actual authenticated CLI follows only the original issued cursors.
use super::*;
use fa_reference::tower::policy_replay_client::{
    POLICY_REPLAY_PAGE_JSON_LIMITS, PolicyReplayCapture,
};

struct NativePages {
    tower: PublicationTower,
    supervisor: native::Supervisor,
    grant: FileSupervisorPolicyReplayGrant,
    _storage: native::Directory,
}

impl NativePages {
    fn new(page_queries: usize) -> Self {
        let storage = native::Directory::new();
        let mut profile = native::profile();
        profile.delivery.total = 4_096;
        profile.delivery.max_attempts = 64;
        let (mut host, _reviewer) = FileOversight::create(storage.store(), profile).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let proposal = native::proposal(&native::oversight::spec(&host, b"published"));
        let (actor, mut supervisor) = host.into_actor_gateway();
        for request in 7..27 {
            native::observe(&mut supervisor);
            let ticket = actor.submit(request, &proposal).unwrap();
            assert_eq!(actor.poll(&ticket), ActorKnowledge::Pending { request });
        }
        let (port, grant) = supervisor
            .policy_replay_port_with_pages(
                1,
                ReplayLimits {
                    cases: 64,
                    input_bytes: 1_048_576,
                },
                page_queries,
            )
            .unwrap();
        let mut tower = PublicationTower::new(
            mount(),
            actor,
            PublicationLimits {
                max_response_bytes: MAX_PUBLICATION_RESPONSE_BYTES,
            },
        )
        .unwrap();
        tower.set_policy_replay(Some(port)).unwrap();
        Self {
            tower,
            supervisor,
            grant,
            _storage: storage,
        }
    }

    fn exchange(&mut self, request: &TowerRequest) -> (String, Option<u64>) {
        let before = self.supervisor.host().unwrap().inspect();
        let access = TowerAccess::new(
            CallerScope {
                caller_tenant: native::profile().delivery.scope.tenant.to_string(),
                allowed_planes: BTreeSet::from([ResolverPlane::ControlLedger]),
                granted_authorities: BTreeSet::from([AuthorityKind::Verify]),
            },
            true,
            vec![PurposeTag::Experiment],
            0,
        );
        let response = self
            .tower
            .exchange(&access, encode_request(request).unwrap().as_bytes());
        let PublicationResult::PolicyReplayPage(linked) = &response.result else {
            panic!("expected the genuine native page: {response:?}");
        };
        let Ok(ActorKnowledge::Known { value, .. }) = linked.native() else {
            panic!("expected known original report: {linked:?}");
        };
        assert_eq!(value.original().report().cases().len(), 20);
        assert_eq!(value.summary().newly_blocked, 20);
        assert!(!value.summary().requires_shadow);
        let next = value.continuation().map(|cursor| cursor.id());
        assert_eq!(self.supervisor.host().unwrap().inspect(), before);
        assert_eq!(self.grant.remaining_queries(), 0);
        (response.to_json().unwrap(), next)
    }
}

fn initial() -> TowerRequest {
    let mut request = replay_request(71);
    let ReadArguments::PolicyReplay {
        campaign,
        expected_policy_generation,
        candidate,
    } = request.arguments
    else {
        unreachable!()
    };
    request.arguments = ReadArguments::PolicyReplayPaged {
        campaign,
        expected_policy_generation,
        candidate,
    };
    request
}

fn settings(root: &Directory) -> PathBuf {
    let path = selected_replay_settings(root, MAX_PUBLICATION_RESPONSE_BYTES);
    let original = fs::read_to_string(&path).unwrap();
    let selected = original.replace(
        "\"policy_replay\":{\"queries\":1,\"cases\":16,",
        "\"policy_replay\":{\"queries\":1,\"page_queries\":8,\"cases\":64,",
    );
    assert_ne!(selected, original);
    assert_eq!(
        OperatorSettings::decode(selected.as_bytes())
            .unwrap()
            .policy_replay
            .unwrap()
            .page_queries,
        8,
    );
    fs::write(&path, selected).unwrap();
    path
}

fn arguments(profile: &Path, document: &Path, maximum: usize) -> Vec<String> {
    vec![
        "tower-policy-replay".into(),
        profile.display().to_string(),
        document.display().to_string(),
        "--max-pages".into(),
        maximum.to_string(),
    ]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ending {
    Complete,
    PageCap,
    NativeExhaustion,
    DamagedPage,
    OutputFailure,
}

fn run(ending: Ending) {
    let root = Directory::new();
    let profile = settings(&root);
    let first = initial();
    let document = root.document("paged-policy.json", &first);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let admitted_pages = match ending {
        Ending::Complete => 3,
        Ending::PageCap | Ending::DamagedPage => 2,
        Ending::NativeExhaustion | Ending::OutputFailure => 1,
    };
    let native_allowance = if ending == Ending::NativeExhaustion {
        1
    } else {
        8
    };
    let server = std::thread::spawn(move || {
        // Rc owner, native grant and immutable report all stay on this thread.
        let mut native = NativePages::new(native_allowance);
        let owner = native.supervisor.host().unwrap().inspect();
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        let mut next = None;
        let mut records = Vec::new();
        let mut retained = None;
        for page in 0..admitted_pages {
            let request = decode_request(&read_document(&mut socket)).unwrap();
            if page == 0 {
                assert_eq!(request, first);
            } else {
                assert_eq!(request.address, first.address);
                assert_eq!(request.verb, Verb::PolicyReplay);
                assert_eq!(request.idempotency_key, first.idempotency_key);
                assert_eq!(
                    request.arguments,
                    ReadArguments::PolicyReplayContinue {
                        campaign: 71,
                        cursor: next.unwrap(),
                    }
                );
            }
            let (original, cursor) = native.exchange(&request);
            next = cursor;
            if let Some(retained) = retained {
                assert_eq!(native.grant.retained_bytes(), retained);
            } else {
                retained = Some(native.grant.retained_bytes());
            }
            let sent = if ending == Ending::DamagedPage && page == 1 {
                let changed = original.replacen("\"offset\":\"0\"", "\"offset\":\"1\"", 1);
                assert_ne!(changed, original);
                changed
            } else {
                original.clone()
            };
            socket.write_all(sent.as_bytes()).unwrap();
            socket.write_all(b"\n").unwrap();
            socket.flush().unwrap();
            records.push((request, original));
        }
        assert_eq!(
            socket.read(&mut [0_u8; 1]).unwrap(),
            0,
            "unissued automatic query"
        );
        assert_eq!(native.grant.remaining_queries(), 0);
        assert_eq!(
            native.grant.remaining_pages(),
            native_allowance - admitted_pages
        );
        assert_eq!(native.supervisor.host().unwrap().inspect(), owner);
        records
    });
    let maximum = if ending == Ending::PageCap { 2 } else { 8 };
    let mut output = Vec::new();
    let result = if ending == Ending::OutputFailure {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        policy_replay_command(&arguments(&profile, &document, maximum), &mut Broken)
    } else {
        policy_replay_command(&arguments(&profile, &document, maximum), &mut output)
    };
    let records = server.join().unwrap();
    assert_eq!(
        result.is_ok(),
        ending == Ending::Complete,
        "{ending:?}: {result:?}"
    );
    let emitted = match ending {
        Ending::OutputFailure => 0,
        Ending::DamagedPage => 1,
        _ => records.len(),
    };
    let expected: String = records
        .iter()
        .take(emitted)
        .map(|(_, response)| format!("{response}\n"))
        .collect();
    assert_eq!(output, expected.as_bytes());
    if ending == Ending::Complete {
        let mut capture =
            PolicyReplayCapture::new(native::profile().delivery.scope, &initial(), 8).unwrap();
        for (request, response) in &records {
            strict_json::parse(response.as_bytes(), POLICY_REPLAY_PAGE_JSON_LIMITS).unwrap();
            capture.accept(request, response.as_bytes()).unwrap();
        }
        assert!(capture.progress().complete);
        assert_eq!(capture.progress().cases_seen, 20);
        assert_eq!(capture.progress().pages_seen, 3);
    }
}

#[test]
fn authenticated_cli_emits_every_original_page_before_claiming_complete_coverage() {
    run(Ending::Complete);
}

#[test]
fn caller_cap_and_native_exhaustion_keep_partial_pages_without_fresh_work() {
    run(Ending::PageCap);
    run(Ending::NativeExhaustion);
}

#[test]
fn damaged_page_stops_before_output_and_before_following_its_cursor() {
    run(Ending::DamagedPage);
}

#[test]
fn output_failure_prevents_continuation_without_refunding_native_work() {
    run(Ending::OutputFailure);
}

#[test]
fn explicit_initial_page_and_closed_local_bounds_are_required_before_connecting() {
    let root = Directory::new();
    let profile = settings(&root);
    let document = root.document("initial.json", &initial());
    for invalid in ["0", "1025", "01", "+1", "1.0", "-1"] {
        let mut args = arguments(&profile, &document, 1);
        args[4] = invalid.into();
        let mut output = Vec::new();
        assert!(
            policy_replay_command(&args, &mut output)
                .unwrap_err()
                .starts_with("tower-policy-replay")
        );
        assert!(output.is_empty());
    }
    let legacy = root.document("legacy.json", &replay_request(71));
    let mut output = Vec::new();
    assert!(
        policy_replay_command(&arguments(&profile, &legacy, 8), &mut output)
            .unwrap_err()
            .contains("initial paged request")
    );
    let disabled = replay_settings(&root);
    assert!(
        policy_replay_command(&arguments(&disabled, &document, 8), &mut output)
            .unwrap_err()
            .contains("explicit policy_replay.page_queries")
    );
    assert!(output.is_empty());
    assert!(!root.socket().exists());
}
