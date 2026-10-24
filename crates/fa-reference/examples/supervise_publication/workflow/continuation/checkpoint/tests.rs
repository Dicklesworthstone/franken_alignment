//! Actual operator commands with original synthetic helper and human socket plumbing.
//! These tests do not establish model quality, reviewer authentication or OS isolation.
use super::*;
use fa_reference::Snapshot;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::{
    guarded::anchored::{FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES},
    reviewer::{
        client::{ReviewClientProgress, ReviewerClient, ReviewerExpectation},
        wire::ReviewDecision,
    },
};
use fa_reference::action::consequence::oversight::{
    actor::ActorProposal,
    actor_wire::{Command, encode_command},
    evidence_source::{EvidenceIdentity, EvidenceSnapshot},
    helper_client::{ClientPhase, HelperClient},
};
use fa_reference::round::Verdict;
use std::{
    collections::BTreeMap,
    os::unix::net::UnixStream,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

const TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/fixtures/supervised_publication.json"
));
const INVENTORY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/authority_topology_inventory.json"
));
const CHILD: &str = "workflow::continuation::checkpoint::tests::synthetic_helper_process";
const MEMBER: &str = "FA_CHECKPOINT_MEMBER";
const EPOCH: &str = "FA_CHECKPOINT_EPOCH";
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = crate::test_directory::create(format!(
            "fa-checkpoint-command-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
    fn text(&self, name: &str) -> String {
        self.path(name).display().to_string()
    }
    fn configuration(&self, epoch: u64) -> Config {
        let mut text = TEMPLATE
            .replace("/operator/publication-store", &self.text("store"))
            .replace("/operator/evidence.json", &self.text("evidence.json"))
            .replace(
                "/operator/helpers/alpha",
                std::env::current_exe().unwrap().to_str().unwrap(),
            )
            .replace(
                "/operator/helpers/beta",
                std::env::current_exe().unwrap().to_str().unwrap(),
            )
            .replace("/operator/helpers", self.0.to_str().unwrap())
            .replace("\"suspend_at_incident\": 3", "\"suspend_at_incident\": 100")
            .replace("\"poll_ms\": 10", "\"poll_ms\": 1")
            .replace("\"runtime_ms\": 120000", "\"runtime_ms\": 15000")
            .replace(
                "\"arguments\": []",
                &format!(r#""arguments": ["--exact", "{CHILD}", "--nocapture"]"#),
            );
        for member in ["alpha", "beta"] {
            text = text.replacen(
                "\"environment\": {}",
                &format!(r#""environment": {{"{MEMBER}":"{member}","{EPOCH}":"{epoch}"}}"#),
                1,
            );
        }
        fs::write(self.path("config.json"), &text).unwrap();
        Config::decode(text.as_bytes()).unwrap()
    }
    fn install(&self, generation: u64, checks: u64) {
        let text = topology(generation, checks);
        fs::write(self.path("live-topology.json"), &text).unwrap();
        fs::write(self.path("live-inventory.json"), INVENTORY).unwrap();
        if generation == 1 {
            fs::write(self.path("initial-topology.json"), text).unwrap();
            fs::write(self.path("initial-inventory.json"), INVENTORY).unwrap();
        }
    }
    fn saved(&self, index: u64) -> TopologyCheckpoint {
        TopologyCheckpoint::read_trusted(
            &mut Cursor::new(fs::read(self.path(&format!("source{index}.bin"))).unwrap()),
            MAX_TOPOLOGY_CHECKPOINT_BYTES,
        )
        .unwrap()
    }
    fn paired(&self, index: u64) -> TopologyCheckpoint {
        let saved = self.saved(index);
        let history = FileHistoryAnchor::read_trusted(
            &mut Cursor::new(fs::read(self.path(&format!("history{index}.anchor"))).unwrap()),
            MAX_HISTORY_ANCHOR_BYTES,
        )
        .unwrap();
        assert_eq!(saved.history_anchor(), &history);
        saved
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("operator checkpoint cleanup: {error}");
        }
    }
}
fn topology(generation: u64, checks: u64) -> String {
    format!(
        r#"{{"schema":"fa.authority-topology/1","generation":{generation},"inventory_generation":{generation},
      "complete_inventory_generation":{generation},
      "scope":{{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5,"purpose":"effect"}},
      "target":{{"adapter":10,"object":11,"contract_version":1,"expected_version":1,"generation":1}},
      "inventory_scope":{{"tenant":1,"principal":2,"purpose":1}},"family":"file-publication",
      "nodes":[{{"id":1,"kind":"actor"}},{{"id":2,"kind":"enforcer"}},{{"id":4,"kind":"sink"}}],
      "edges":[{{"id":1,"from":1,"to":2,"channel":"dispatch","route":"publication","provenance":101}},
        {{"id":2,"from":2,"to":4,"channel":"dispatch","route":"publication","provenance":102}}],
      "enforcers":[{{"node":2,"adapter":10,"contract_version":1,"generation":1,"provenance":102}}],
      "gates":[2],"check_edge_budget":65536,"source_limits":{{"checks":{checks},"read_bytes":67108928}}}}"#
    )
}
#[test]
fn synthetic_helper_process() {
    let Ok(member) = std::env::var(MEMBER) else {
        return;
    };
    let epoch: u64 = std::env::var(EPOCH).unwrap().parse().unwrap();
    let config = Config::decode(TEMPLATE.as_bytes()).unwrap();
    let mut client = HelperClient::from_process_stdin(
        config.profile.committee.members()[&member].profile_at(epoch),
    )
    .unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10));
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference {
            client.respond(Verdict::Allow, member.as_bytes()).unwrap();
        }
        if client.phase() == ClientPhase::ReplySent {
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn reviewer(
    config: &Config,
    request: u64,
    choice: ReviewDecision,
    before_decision: impl FnOnce() + Send + 'static,
) -> std::thread::JoinHandle<()> {
    let path = config.socket(request);
    let expected = ReviewerExpectation {
        reviewer: config.profile.human.reviewer_id,
        scope: config.profile.delivery.scope,
        clock_domain: crate::config::CLOCK_DOMAIN,
    };
    std::thread::spawn(move || {
        let started = Instant::now();
        let socket = loop {
            match UnixStream::connect(&path) {
                Ok(socket) => break socket,
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                    ) =>
                {
                    assert!(started.elapsed() < Duration::from_secs(10));
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(error) => panic!("reviewer connection: {error}"),
            }
        };
        let mut client = ReviewerClient::from_unix(socket, expected).unwrap();
        let mut before = Some(before_decision);
        loop {
            assert!(started.elapsed() < Duration::from_secs(12));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    before.take().unwrap()();
                    client.respond(choice).unwrap();
                }
                ReviewClientProgress::Complete => return,
                _ => std::thread::sleep(Duration::from_millis(1)),
            }
        }
    })
}
fn proposal(root: &Directory, config: &Config, request: u64, epoch: u64, version: u64) -> PathBuf {
    let mut target = config.profile.delivery.target;
    target.expected_version = version;
    let payload = if request == 1 {
        b"first".to_vec()
    } else {
        b"second".to_vec()
    };
    let bytes = encode_command(&Command::Submit {
        request,
        proposal: ActorProposal {
            target,
            units: payload.len() as u64,
            payload,
            expected_policy_epoch: epoch,
            deadline: ElapsedTick(crate::workflow::clock().0 + 60000),
        },
    })
    .unwrap();
    let path = root.path(&format!("submit{request}.json"));
    fs::write(&path, bytes).unwrap();
    path
}
fn creation(root: &Directory, checks: u64) -> (Config, Vec<String>) {
    root.install(1, checks);
    let config = root.configuration(0);
    let evidence = EvidenceSnapshot::new(
        EvidenceIdentity {
            source: 51,
            generation: 1,
            scope: config.profile.delivery.scope,
        },
        Snapshot {
            semantic_epoch: 1,
            complete: true,
            values: BTreeMap::from([(7, b"ok".to_vec())]),
        },
        ["alpha", "beta"]
            .into_iter()
            .map(|member| {
                (
                    member.into(),
                    format!("review this fixture: {member}").into_bytes(),
                )
            })
            .collect(),
    )
    .unwrap();
    fs::write(root.path("evidence.json"), evidence.encode()).unwrap();
    fs::write(
        root.path("create.json"),
        format!(
            r#"{{"schema_version":"fa.mediated-create/2",
        "topology_path":"{}","inventory_path":"{}","retain_checkpoint":"{}"}}"#,
            root.text("live-topology.json"),
            root.text("live-inventory.json"),
            root.text("source0.bin")
        ),
    )
    .unwrap();
    let submit = proposal(root, &config, 1, 0, 1);
    let args = vec![
        "create".into(),
        root.text("config.json"),
        submit.display().to_string(),
        "--retain-history-anchor".into(),
        root.text("history0.anchor"),
        "--topology-profile".into(),
        root.text("create.json"),
    ];
    (config, args)
}
fn seed(root: &Directory, checks: u64) {
    let (config, args) = creation(root, checks);
    let human = reviewer(&config, 1, ReviewDecision::Approve, || {});
    let result = crate::command(args);
    human.join().unwrap();
    result.unwrap();
    assert_eq!(root.paired(0).work().checks, 2);
}
fn continuation(root: &Directory, index: u64, receipt_only: bool) -> (Config, Vec<String>) {
    let previous = root.saved(index - 1);
    let epoch = previous.minimum().authority_epoch + if previous.available() { 3 } else { 2 };
    let config = root.configuration(epoch);
    let document = if receipt_only {
        root.path(&format!("submit{index}.json"))
    } else {
        proposal(root, &config, index + 1, epoch, index + 1)
    };
    let floor = previous.minimum();
    fs::write(
        root.path("recover.json"),
        format!(
            r#"{{"schema_version":"fa.guarded-submit/1",
        "minimum":{{"journal_revision":{},"control_sequence":{},"authority_epoch":{}}},
        "history_anchor":"{}","retain_history_anchor":"{}"}}"#,
            floor.journal_revision,
            floor.control_sequence,
            floor.authority_epoch,
            root.text(&format!("history{}.anchor", index - 1)),
            root.text(&format!("history{index}.anchor"))
        ),
    )
    .unwrap();
    let output = if receipt_only {
        String::new()
    } else {
        format!(
            r#", "retain_checkpoint":"{}""#,
            root.text(&format!("source{index}.bin"))
        )
    };
    fs::write(
        root.path("continue.json"),
        format!(
            r#"{{"schema_version":"fa.mediated-submit/2",
        "initial":{{"topology_path":"{}","inventory_path":"{}"}},"source_checkpoint":"{}",
        "next":{{"topology_path":"{}","inventory_path":"{}"}},"operation":{}{output}}}"#,
            root.text("initial-topology.json"),
            root.text("initial-inventory.json"),
            root.text(&format!("source{}.bin", index - 1)),
            root.text("live-topology.json"),
            root.text("live-inventory.json"),
            previous.last_operation() + 1
        ),
    )
    .unwrap();
    (
        config,
        vec![
            "submit".into(),
            root.text("config.json"),
            document.display().to_string(),
            "--guarded".into(),
            root.text("recover.json"),
            "--topology-profile".into(),
            root.text("continue.json"),
        ],
    )
}

#[test]
fn creation_and_two_continuations_use_exported_actual_work_without_handwritten_counters() {
    let root = Directory::new();
    seed(&root, 8);
    for index in 1..=2 {
        root.install(index + 1, 8);
        let (config, args) = continuation(&root, index, false);
        let human = reviewer(&config, index + 1, ReviewDecision::Approve, || {});
        let result = crate::command(args);
        human.join().unwrap();
        result.unwrap();
        let saved = root.paired(index);
        assert_eq!(saved.work().checks, (index + 1) * 2);
        let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(state.executions, index + 1);
        assert_eq!(state.control.ledger.charged, 5 + 6 * index);
        assert_eq!(state.control.ledger.reserved, 0);
        assert!(saved.available());
        assert_eq!(saved.minimum().journal_revision, state.revision);
    }
}

#[test]
fn receipt_only_checkpoint_recovery_needs_no_next_files_and_does_not_invent_new_work() {
    let root = Directory::new();
    seed(&root, 8);
    let old = fs::read(root.path("source0.bin")).unwrap();
    fs::remove_file(root.path("evidence.json")).unwrap();
    fs::remove_file(root.path("live-topology.json")).unwrap();
    fs::remove_file(root.path("live-inventory.json")).unwrap();
    let (config, args) = continuation(&root, 1, true);
    crate::command(args).unwrap();
    assert!(!root.path("source1.bin").exists());
    assert_eq!(fs::read(root.path("source0.bin")).unwrap(), old);
    assert_eq!(
        FileOversight::read_publication(&config.store, &config.profile)
            .unwrap()
            .executions,
        1
    );
}

#[test]
fn failed_final_source_check_exports_spent_work_and_withdrawn_coverage_not_changed_files() {
    let root = Directory::new();
    let (config, args) = creation(&root, 8);
    let inventory = root.path("live-inventory.json");
    let human = reviewer(&config, 1, ReviewDecision::Approve, move || {
        fs::remove_file(inventory).unwrap();
    });
    let result = crate::command(args);
    human.join().unwrap();
    assert!(result.is_err());
    let saved = root.paired(0);
    assert_eq!(saved.work().checks, 2);
    assert!(!saved.available());
    assert_eq!(saved.inventory_bytes(), INVENTORY.as_bytes());
    assert_eq!(
        FileOversight::read_publication(&config.store, &config.profile)
            .unwrap()
            .executions,
        0
    );
}

#[test]
fn output_race_does_not_overwrite_custody_or_undo_an_acknowledged_publication() {
    let root = Directory::new();
    let (config, args) = creation(&root, 8);
    let output = root.path("source0.bin");
    let human = reviewer(&config, 1, ReviewDecision::Approve, move || {
        fs::write(output, b"occupied").unwrap();
    });
    let result = crate::command(args);
    human.join().unwrap();
    assert!(matches!(result, Err(error) if error.contains("checkpoint retention failed")));
    assert_eq!(fs::read(root.path("source0.bin")).unwrap(), b"occupied");
    assert!(root.path("history0.anchor").exists());
    assert_eq!(
        FileOversight::read_publication(&config.store, &config.profile)
            .unwrap()
            .executions,
        1
    );
}

#[test]
fn exhausted_checkpoint_budget_refuses_another_publication_without_resetting_work() {
    let root = Directory::new();
    seed(&root, 4);
    root.install(2, 4);
    let (config, args) = continuation(&root, 1, false);
    let human = reviewer(&config, 2, ReviewDecision::Approve, || {});
    let result = crate::command(args);
    human.join().unwrap();
    result.unwrap();
    assert_eq!(root.paired(1).work().checks, 4);
    root.install(3, 4);
    let (config, args) = continuation(&root, 2, false);
    assert!(matches!(crate::command(args), Err(error) if error.contains("topology budget")));
    assert!(!root.path("source2.bin").exists());
    assert!(!config.socket(3).exists());
    assert_eq!(
        FileOversight::read_publication(&config.store, &config.profile)
            .unwrap()
            .executions,
        2
    );
}

#[test]
fn forged_counter_override_and_mismatched_history_refuse_before_journal_mutation() {
    let root = Directory::new();
    seed(&root, 8);
    root.install(2, 8);
    let (_config, args) = continuation(&root, 1, false);
    let before = fs::read(root.path("store/delivery.bin")).unwrap();
    let original = fs::read_to_string(root.path("continue.json")).unwrap();
    let changed = original.replacen(
        "\"operation\":3",
        "\"operation\":3,\"source\":{\"checks\":0}",
        1,
    );
    assert_ne!(changed, original);
    fs::write(root.path("continue.json"), changed).unwrap();
    assert!(crate::command(args.clone()).is_err());
    assert_eq!(fs::read(root.path("store/delivery.bin")).unwrap(), before);
    fs::write(root.path("continue.json"), original).unwrap();
    // A different but correctly framed old anchor must not pair with these counters.
    let mut anchor = fs::read(root.path("history0.anchor")).unwrap();
    *anchor.last_mut().unwrap() ^= 1;
    fs::write(root.path("history0.anchor"), anchor).unwrap();
    assert!(matches!(crate::command(args), Err(error) if error.contains("different cuts")));
    assert_eq!(fs::read(root.path("store/delivery.bin")).unwrap(), before);
}

#[test]
fn newer_canonical_history_cannot_reuse_an_older_checkpoint_as_complete_work_accounting() {
    let root = Directory::new();
    seed(&root, 8);
    root.install(2, 8);
    let (config, args) = continuation(&root, 1, false);
    let retained = root.paired(0);
    // A real native reopen adds an original recovery fence. It is a valid
    // history successor, not corrupt framing, but this strict operator mode
    // cannot assume any post-checkpoint history has complete source accounting.
    let (owner, _) = FileOversight::open(&config.store, config.profile.clone()).unwrap();
    assert!(owner.revision() > retained.minimum().journal_revision);
    drop(owner);
    let before = fs::read(root.path("store/delivery.bin")).unwrap();
    assert!(
        matches!(crate::command(args), Err(error) if error.contains("advanced beyond the source checkpoint"))
    );
    assert_eq!(fs::read(root.path("store/delivery.bin")).unwrap(), before);
    assert!(!root.path("source1.bin").exists());
    assert!(!config.socket(2).exists());
}

#[test]
fn creation_refuses_bad_output_before_storage_and_retains_spent_work_on_human_rejection() {
    let root = Directory::new();
    let (config, args) = creation(&root, 8);
    fs::write(root.path("source0.bin"), b"previous custody").unwrap();
    assert!(
        matches!(crate::command(args.clone()), Err(error) if error.contains("checkpoint output must be a new file"))
    );
    assert!(!config.store.exists());
    assert_eq!(
        fs::read(root.path("source0.bin")).unwrap(),
        b"previous custody"
    );
    fs::remove_file(root.path("source0.bin")).unwrap();
    let original = fs::read_to_string(root.path("create.json")).unwrap();
    fs::write(
        root.path("create.json"),
        original.replace("fa.mediated-create/2", "fa.mediated-create/1"),
    )
    .unwrap();
    assert!(crate::command(args.clone()).is_err());
    assert!(!config.store.exists());
    fs::write(root.path("create.json"), original).unwrap();
    let human = reviewer(&config, 1, ReviewDecision::Reject, || {});
    let result = crate::command(args);
    human.join().unwrap();
    assert!(result.is_err());
    let saved = root.paired(0);
    assert_eq!(
        saved.work().checks,
        1,
        "initial check remains spent; no publication check ran"
    );
    let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(state.executions, 0);
    assert_eq!(state.control.ledger.charged, 0);
    assert_eq!(state.control.ledger.reserved, 0);
}
