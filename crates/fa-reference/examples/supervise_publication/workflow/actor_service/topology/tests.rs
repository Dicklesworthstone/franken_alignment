//! Actual live actor and independent human sockets, native publication and
//! synthetic helper processes. These are enforcement controls, not model scores.
use super::super::{FileOversight, RecoveryReserve, check_deployment};
use super::*;
use crate::workflow::{Duration, Instant, pause};
use fa_reference::action::consequence::delivery::persistent::observed::{
    mediation::source::TOPOLOGY_REFRESH_READ_BYTES,
    publication::{
        capture::{FileCaptureIdentity, FilePublicationCapture},
        witnesses::{FilePublicationInputs, FileWitnessInput},
    },
    reviewer::{client::ReviewClientProgress, wire::ReviewDecision},
};
use fa_reference::action::consequence::mediation::MAX_CHECK_EDGE_VISITS;
use fa_reference::action::consequence::oversight::{
    actor::{ActorOutcome, ActorProposal, Knowledge},
    actor_peer::PeerCredentials,
    actor_wire::{Command, WireResponse, decode_response, encode_command},
    evidence_source::{EvidenceIdentity, EvidenceSnapshot},
};
use fa_reference::action::{ActionSpec, FrozenAction, VERSION};
use fa_reference::{
    Snapshot,
    product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker},
    witness::{AdapterDomainInput, DomainClosure, DomainProjection, SnapshotEntry},
};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::{fs::DirBuilderExt, net::UnixStream},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

const TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/fixtures/supervised_publication.json"
));
const INVENTORY: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/authority_topology_inventory.json"
));
const RECIPE: &str = r#"[{"kind":"exact_value","key":0,"role":"subject"},{"kind":"absent_key","key":1},{"kind":"empty_range","start":6,"end":9},{"kind":"range_members","start":2,"end":6}]"#;

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = crate::test_directory::create(format!(
            "fa-live-cut-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        for child in ["actors", "reviewers"] {
            fs::DirBuilder::new()
                .mode(0o750)
                .create(root.join(child))
                .unwrap();
        }
        Self(root)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("checked live topology cleanup: {error}");
        }
    }
}
fn configuration(root: &Root) -> Config {
    let exe = std::env::current_exe().unwrap().display().to_string();
    let mut text = TEMPLATE
        .replace(
            "/operator/publication-store",
            root.path("store").to_str().unwrap(),
        )
        .replace(
            "/operator/evidence.json",
            root.path("evidence.json").to_str().unwrap(),
        )
        .replace(
            "\"directory\": \"/operator/helpers\"",
            &format!("\"directory\": {:?}", root.0.display().to_string()),
        )
        .replace(
            "\"arguments\": []",
            "\"arguments\": [\"--exact\",\"tests::synthetic_helper_process\",\"--nocapture\"]",
        )
        .replace("\"poll_ms\": 10", "\"poll_ms\": 1")
        .replace("\"runtime_ms\": 120000", "\"runtime_ms\": 15000");
    for member in ["alpha", "beta"] {
        text = text.replace(
            &format!("\"executable\": \"/operator/helpers/{member}\""),
            &format!("\"executable\": {exe:?}"),
        );
        text = text.replacen(
            "\"environment\": {}",
            &format!("\"environment\": {{\"FA_EXAMPLE_HELPER_MEMBER\":\"{member}\"}}"),
            1,
        );
    }
    fs::write(root.path("config.json"), &text).unwrap();
    Config::decode(text.as_bytes()).unwrap()
}
fn profiles(root: &Root, config: &Config) -> (Profile, PeerProfile) {
    let (socket, _) = UnixStream::pair().unwrap();
    let id = PeerCredentials::observe(&socket).unwrap();
    let s = config.profile.delivery.scope;
    let scope = format!(
        r#"{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}}"#,
        s.tenant, s.principal, s.run, s.branch, s.authority
    );
    let identity = format!(
        r#"{{"uid":{},"gid":{},"pid":{}}}"#,
        id.uid(),
        id.gid(),
        id.pid()
    );
    let actor = format!(
        r#"{{"schema":"fa.actor-service/1","clock":"unix_milliseconds","request":7,"scope":{scope},"socket":"{}","supervisor":{identity},"actor":{identity},"candidate_limit":8,"connection_limit":4,"exchange_limit":1024,"runtime_ms":15000,"poll_ms":1,"reply_ms":200}}"#,
        root.path("actors/actor.sock").display()
    );
    let reviewer = format!(
        r#"{{"version":1,"clock":"unix_milliseconds","scope":{scope},"reviewer_id":{},"socket_directory":"{}","supervisor":{identity},"reviewer":{identity},"candidate_limit":8,"runtime_ms":15000,"poll_ms":1}}"#,
        config.profile.human.reviewer_id,
        root.path("reviewers").display()
    );
    fs::write(root.path("actor.json"), &actor).unwrap();
    fs::write(root.path("reviewer.json"), &reviewer).unwrap();
    (
        Profile::decode(actor.as_bytes()).unwrap(),
        PeerProfile::decode(reviewer.as_bytes()).unwrap(),
    )
}
fn snapshot() -> Snapshot {
    Snapshot {
        semantic_epoch: 1,
        complete: true,
        values: BTreeMap::from([(7, b"ok".to_vec())]),
    }
}
fn evidence(root: &Root, config: &Config) {
    let evidence = EvidenceSnapshot::new(
        EvidenceIdentity {
            source: 51,
            generation: 1,
            scope: config.profile.delivery.scope,
        },
        snapshot(),
        ["alpha", "beta"]
            .into_iter()
            .map(|name| {
                (
                    name.to_owned(),
                    format!("review this fixture: {name}").into_bytes(),
                )
            })
            .collect(),
    )
    .unwrap();
    fs::write(root.path("evidence.json"), evidence.encode()).unwrap();
}
fn graph(bypass: bool) -> String {
    let extra = if bypass {
        r#",{"id":3,"from":1,"to":4,"channel":"dispatch","route":"publication","provenance":103}"#
    } else {
        ""
    };
    let reads = 32 * TOPOLOGY_REFRESH_READ_BYTES;
    format!(
        r#"{{"schema":"fa.authority-topology/1","generation":1,"inventory_generation":1,
        "complete_inventory_generation":1,
        "scope":{{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5,"purpose":"effect"}},
        "target":{{"adapter":10,"object":11,"contract_version":1,"expected_version":1,"generation":1}},
        "inventory_scope":{{"tenant":1,"principal":2,"purpose":1}},"family":"file-publication",
        "nodes":[{{"id":1,"kind":"actor"}},{{"id":2,"kind":"enforcer"}},{{"id":4,"kind":"sink"}}],
        "edges":[{{"id":1,"from":1,"to":2,"channel":"dispatch","route":"publication","provenance":101}},
            {{"id":2,"from":2,"to":4,"channel":"dispatch","route":"publication","provenance":102}}{extra}],
        "enforcers":[{{"node":2,"adapter":10,"contract_version":1,"generation":1,"provenance":102}}],
        "gates":[2],"check_edge_budget":{MAX_CHECK_EDGE_VISITS},"source_limits":{{"checks":32,"read_bytes":{reads}}}}}"#
    )
}
fn selection(root: &Root, bypass: bool) -> Selection {
    fs::write(root.path("topology.json"), graph(bypass)).unwrap();
    fs::write(root.path("inventory.json"), INVENTORY).unwrap();
    let text = format!(
        r#"{{"schema_version":"fa.mediated-create/1","topology_path":"{}","inventory_path":"{}"}}"#,
        root.path("topology.json").display(),
        root.path("inventory.json").display()
    );
    fs::write(root.path("topology-profile.json"), &text).unwrap();
    Selection::decode(text.as_bytes()).unwrap()
}
fn publication_text(root: &Root) -> String {
    format!(
        r#"{{"schema":"fa.supervised-witnesses/1","source":91,"original":"{}","current":"{}","limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":{RECIPE}}}"#,
        root.path("original.bin").display(),
        root.path("current.bin").display()
    )
}
fn publication(root: &Root) -> PublicationProfile {
    let text = publication_text(root);
    fs::write(root.path("witness.json"), &text).unwrap();
    PublicationProfile::decode(text.as_bytes()).unwrap()
}
fn capture(action: &FrozenAction, generation: u64, keys: &[u64]) -> FilePublicationCapture {
    let key = ProjectionKey {
        source: 40,
        branch: action.spec().scope.branch,
        projection: 7,
        source_epoch: 1,
    };
    let close = TrustedClosingMarker {
        key,
        final_sequence: 1,
        marker_generation: 1,
    };
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    frontiers
        .accept(key, FrontierStage::Authenticated, 1)
        .unwrap();
    frontiers.record_close(close).unwrap();
    let input = FileWitnessInput::new(
        generation,
        generation,
        20,
        AdapterDomainInput::new(
            DomainProjection::new(40, 1, key),
            DomainClosure::Closed(close),
        ),
        keys.iter()
            .map(|key| SnapshotEntry::new(*key, 1, b"original".to_vec()).unwrap())
            .collect(),
        &frontiers,
    )
    .unwrap();
    FilePublicationCapture::new(
        1,
        FileCaptureIdentity {
            source: 91,
            generation,
        },
        action,
        FilePublicationInputs::new(Some(input), None),
    )
    .unwrap()
}
fn write_capture(root: &Root, name: &str, packet: &FilePublicationCapture) {
    let pending = root.path("capture.pending");
    fs::write(&pending, packet.to_bytes().unwrap()).unwrap();
    fs::rename(pending, root.path(name)).unwrap();
}
fn prepare(
    root: &Root,
    config: &Config,
    deadline: ElapsedTick,
) -> (PublicationProfile, FrozenAction) {
    evidence(root, config);
    // The original policy freezes the SAME action that the actor will submit.
    // This disposable owner supplies capture data only; it never authorizes the service.
    let (mut host, _) =
        FileOversight::create(root.path("capture-owner"), config.profile.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000))
        .unwrap();
    let action = host
        .propose(
            host.revision(),
            1,
            ActionSpec {
                version: VERSION,
                scope: config.profile.delivery.scope,
                target: Some(config.profile.delivery.target),
                payload: b"network".to_vec(),
                required_witnesses: Vec::new(),
                policy_epoch: 0,
                deadline,
                units: 7,
            },
            snapshot(),
        )
        .unwrap();
    let original = capture(&action, 1, &[0, 2, 4]);
    write_capture(root, "original.bin", &original);
    write_capture(root, "current.bin", &original);
    let document = encode_command(&Command::Submit {
        request: 7,
        proposal: ActorProposal {
            target: config.profile.delivery.target,
            payload: b"network".to_vec(),
            units: 7,
            deadline,
            expected_policy_epoch: 0,
        },
    })
    .unwrap();
    fs::write(root.path("submit.json"), document).unwrap();
    (publication(root), action)
}
fn wait(path: &Path) {
    let started = Instant::now();
    while !path.exists() {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "missing {path:?}"
        );
        pause(1);
    }
}
fn human(profile: PeerProfile, decision: ReviewDecision) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        wait(&profile.socket(7));
        let mut client = profile.connect_client(7).unwrap();
        let start = Instant::now();
        loop {
            assert!(start.elapsed() < Duration::from_secs(10));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => client.respond(decision).unwrap(),
                ReviewClientProgress::Complete => return,
                _ => pause(1),
            }
        }
    })
}
fn actor(profile: Profile, path: PathBuf) -> std::thread::JoinHandle<WireResponse> {
    std::thread::spawn(move || {
        wait(&profile.socket);
        let mut output = Vec::new();
        super::super::client::submit(&profile, &path, &mut output).unwrap();
        decode_response(output.strip_suffix(b"\n").unwrap()).unwrap()
    })
}
fn executed(response: &WireResponse) -> bool {
    matches!(
        response.result,
        Ok(Knowledge::Known {
            value: ActorOutcome::Executed,
            ..
        })
    )
}

#[test]
fn plain_and_checked_argument_roles_are_distinct_and_complete() {
    let plain = [
        "serve-create",
        "/config",
        "/actor",
        "/reviewer",
        "--topology-profile",
        "/topology",
    ]
    .map(str::to_owned);
    let checked = [
        "serve-create-checked",
        "/config",
        "/actor",
        "/reviewer",
        "/witness",
        "--topology-profile",
        "/topology",
    ]
    .map(str::to_owned);
    assert_eq!(arguments(&plain).unwrap(), (None, Path::new("/topology")));
    assert_eq!(
        arguments(&checked).unwrap(),
        (Some(Path::new("/witness")), Path::new("/topology"))
    );
    let mut bad = Vec::new();
    for mode in [
        "serve-open",
        "serve-open-checked",
        "actor-submit",
        "create-learned-generated",
    ] {
        let mut changed = checked.to_vec();
        changed[0] = mode.into();
        bad.push(changed);
    }
    for index in [1, 2, 3, 4, 6] {
        for value in ["", "--receipts-only", "--topology-profile"] {
            let mut changed = checked.to_vec();
            changed[index] = value.into();
            bad.push(changed);
        }
    }
    for extra in [
        "--reconcile-receipts",
        "--requests",
        "--peers",
        "--native-text",
        "--credibility-activation",
    ] {
        let mut changed = checked.to_vec();
        changed.push(extra.into());
        bad.push(changed);
    }
    bad.extend([checked[..6].to_vec(), plain[..5].to_vec(), Vec::new()]);
    for args in bad {
        assert_eq!(
            command_with_clock(&args, None, || panic!("invalid grammar cannot read time")),
            Err(USAGE.into())
        );
        // The actual outer router must also reject mixed topology modes.
        if args.iter().any(|arg| arg == "--topology-profile") {
            assert_eq!(super::super::command(&args, None), Err(USAGE.into()));
        }
    }
    assert_eq!(
        command_with_clock(&checked, Some(Path::new("/capsule")), || panic!(
            "creation cannot activate credibility"
        )),
        Err(USAGE.into())
    );
}

#[test]
fn checked_bootstrap_and_deployment_check_never_omit_or_install_a_guard() {
    let root = Root::new();
    let config = configuration(&root);
    let publication = publication(&root);
    let prepared = selection(&root, false)
        .prepare(&config, &mut || Ok(()))
        .unwrap();
    let (host, _, _) = prepared
        .create(&config, Some(publication.simple_limits().unwrap()))
        .unwrap();
    let revision = host.revision();
    let bytes = fs::read(config.store.join("delivery.bin")).unwrap();
    assert!(check_deployment(&config, &host, Some(&publication)).is_ok());
    assert!(check_deployment(&config, &host, None).is_err());
    assert_eq!(
        host.publication_validation_profile().unwrap(),
        Some(publication.limits)
    );
    assert_eq!(
        host.journal_capacity().unwrap().reserve(),
        Some(RecoveryReserve::terminal())
    );
    assert_eq!(
        host.file_source_status().unwrap().policy,
        config.source_policy
    );
    assert!(host.publication_guard_required());
    assert!(host.mediation_snapshot().unwrap().accepted.is_none());
    assert_eq!(host.revision(), revision);
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), bytes);
    for name in ["original.bin", "current.bin", "evidence.json"] {
        assert!(!root.path(name).exists());
    }
}

#[test]
fn live_final_gate_checks_both_topology_and_witnesses_after_human_approved_dispatch() {
    for mode in [
        "unrelated",
        "phantom",
        "missing-witness",
        "missing-topology",
        "both-missing",
    ] {
        let root = Root::new();
        let config = configuration(&root);
        let (actor_profile, reviewer) = profiles(&root, &config);
        let (publication, action) = prepare(&root, &config, ElapsedTick(100000));
        let selected = selection(&root, false);
        let store = config.store.clone();
        let bootstrap = config.profile.clone();
        let peer = actor(actor_profile.clone(), root.path("submit.json"));
        let human = human(reviewer.clone(), ReviewDecision::Approve);
        let mut changed = false;
        let result = serve_selected(
            config,
            &actor_profile,
            &reviewer,
            Some(&publication),
            false,
            None,
            Some(selected),
            || {
                if !changed
                    && let Ok(state) = FileOversight::read_publication(&store, &bootstrap)
                    && state.control.ledger.charged != 0
                    && state.executions == 0
                {
                    match mode {
                        "unrelated" => write_capture(
                            &root,
                            "current.bin",
                            &capture(&action, 2, &[0, 2, 4, 99]),
                        ),
                        "phantom" => {
                            write_capture(&root, "current.bin", &capture(&action, 2, &[0, 1, 2, 4]))
                        }
                        "missing-witness" => fs::remove_file(root.path("current.bin")).unwrap(),
                        "missing-topology" => fs::remove_file(root.path("inventory.json")).unwrap(),
                        "both-missing" => {
                            fs::remove_file(root.path("current.bin")).unwrap();
                            fs::remove_file(root.path("inventory.json")).unwrap();
                        }
                        _ => unreachable!(),
                    }
                    changed = true;
                }
                ElapsedTick(1000)
            },
        );
        human.join().unwrap();
        let response = peer.join().unwrap();
        assert!(changed, "{mode}: {result:?}");
        if mode == "unrelated" {
            assert!(result.is_ok(), "{result:?}");
        }
        assert_eq!(
            executed(&response),
            mode == "unrelated",
            "{mode}: {response:?}"
        );
        let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(state.executions, u64::from(mode == "unrelated"));
        assert_eq!(
            state.payload,
            if mode == "unrelated" {
                b"network".to_vec()
            } else {
                b"initial".to_vec()
            }
        );
        assert_eq!(
            state.control.ledger.available
                + state.control.ledger.reserved
                + state.control.ledger.charged,
            bootstrap.delivery.total
        );
        assert_eq!(state.control.ledger.reserved, 0);
        assert!(!actor_profile.socket.exists());
    }
}

#[test]
fn checked_command_loads_all_profiles_and_still_requires_independent_human_approval() {
    for decision in [ReviewDecision::Approve, ReviewDecision::Reject] {
        let root = Root::new();
        let config = configuration(&root);
        let (actor_profile, reviewer) = profiles(&root, &config);
        let (_publication, _) = prepare(&root, &config, ElapsedTick(100000));
        let _selected = selection(&root, false);
        let peer = actor(actor_profile.clone(), root.path("submit.json"));
        let human = human(reviewer.clone(), decision);
        let args = vec![
            "serve-create-checked".into(),
            root.path("config.json").display().to_string(),
            root.path("actor.json").display().to_string(),
            root.path("reviewer.json").display().to_string(),
            root.path("witness.json").display().to_string(),
            "--topology-profile".into(),
            root.path("topology-profile.json").display().to_string(),
        ];
        let result = command_with_clock(&args, None, || ElapsedTick(1000));
        human.join().unwrap();
        let response = peer.join().unwrap();
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(executed(&response), decision == ReviewDecision::Approve);
        let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(
            state.executions,
            u64::from(decision == ReviewDecision::Approve)
        );
        if decision == ReviewDecision::Reject {
            assert_eq!(
                state.control.ledger.available,
                config.profile.delivery.total
            );
        }
        assert!(!actor_profile.socket.exists());
    }
}

#[test]
fn initial_bypass_or_socket_collision_cannot_start_checked_actor_admission() {
    for collision in [false, true] {
        let root = Root::new();
        let mut config = configuration(&root);
        config.programs.clear();
        let (actor, reviewer) = profiles(&root, &config);
        let publication = publication(&root);
        let selected = selection(&root, !collision);
        let store = config.store.clone();
        let bootstrap = config.profile.clone();
        if collision {
            fs::write(&actor.socket, b"unrelated socket owner").unwrap();
        }
        let result = serve_selected(
            config,
            &actor,
            &reviewer,
            Some(&publication),
            false,
            None,
            Some(selected),
            || ElapsedTick(1000),
        );
        assert!(result.is_err());
        if collision {
            assert_eq!(fs::read(&actor.socket).unwrap(), b"unrelated socket owner");
            assert!(!store.exists());
        } else {
            assert!(
                result
                    .unwrap_err()
                    .contains("initial live-service topology was not certified")
            );
            let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
            assert_eq!(state.executions, 0);
            assert!(state.control.ledger.stages.is_empty());
            assert!(!actor.socket.exists());
        }
        assert!(!reviewer.socket(7).exists());
    }
}

#[test]
fn stronger_profiles_are_refused_before_any_topology_io_clock_or_journal_creation() {
    for waiting in [false, true] {
        let root = Root::new();
        let config = configuration(&root);
        let (actor, reviewer) = profiles(&root, &config);
        let schema = if waiting {
            "fa.supervised-witnesses/4"
        } else {
            "fa.supervised-witnesses/2"
        };
        let extra = if waiting { ",\"max_retries\":2" } else { "" };
        let text = publication_text(&root).replace("fa.supervised-witnesses/1", schema)
            .replace("\"source\":91", &format!(r#""source":91{extra},"feed":{{"path":"{}","source":41,"after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}}"#,
                root.path("missing-feed.bin").display()));
        let publication = PublicationProfile::decode(text.as_bytes()).unwrap();
        let selected = selection(&root, false);
        fs::remove_file(root.path("topology.json")).unwrap();
        let store = config.store.clone();
        let error = serve_selected(
            config,
            &actor,
            &reviewer,
            Some(&publication),
            false,
            None,
            Some(selected),
            || panic!("unsupported composition must not read time"),
        )
        .unwrap_err();
        assert!(error.contains("not downgraded"));
        assert!(!store.exists());
        assert!(!actor.socket.exists());
    }
}

#[test]
fn topology_checked_open_and_invalid_witness_files_cannot_fall_back_to_creation() {
    let root = Root::new();
    let config = configuration(&root);
    let (actor, reviewer) = profiles(&root, &config);
    let publication = publication(&root);
    let selected = selection(&root, false);
    let store = config.store.clone();
    assert!(
        serve_selected(
            config,
            &actor,
            &reviewer,
            Some(&publication),
            true,
            None,
            Some(selected),
            || panic!("open is not a creation fallback")
        )
        .is_err()
    );
    assert!(!store.exists());
    assert!(!actor.socket.exists());
    fs::write(root.path("witness.json"), b"{}").unwrap();
    let args = vec![
        "serve-create-checked".into(),
        root.path("config.json").display().to_string(),
        root.path("actor.json").display().to_string(),
        root.path("reviewer.json").display().to_string(),
        root.path("witness.json").display().to_string(),
        "--topology-profile".into(),
        root.path("topology-profile.json").display().to_string(),
    ];
    assert!(
        command_with_clock(&args, None, || panic!("invalid witness file precedes time")).is_err()
    );
    assert!(!store.exists());
    assert!(!actor.socket.exists());
}
