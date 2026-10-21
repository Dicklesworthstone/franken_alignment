//! Actual operator parsing, independent files and original coordinator receipts.
use super::*;
use crate::config::Config;
use fa_reference::action::consequence::delivery::StopRequest;
use fa_reference::action::consequence::delivery::persistent::{RecoveryReserve,
    observed::{FileOversight, guarded::anchored::MAX_HISTORY_ANCHOR_BYTES,
        shutdown::coordinator::{FileShutdownClockBinding, FileShutdownCoordinatorAnchor}}};
use fa_reference::strict_json::{self, Json, Limits};
use std::{cell::Cell, fs::{self, OpenOptions}, io::{self, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt}, path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering}, time::{Duration, SystemTime, UNIX_EPOCH}};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture { root: PathBuf, configs: Vec<Config> }
fn private(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path).unwrap();
    file.write_all(bytes).unwrap();
}
impl Fixture {
    fn new(visits: usize, stopped_second: bool) -> Self {
        let root = std::env::temp_dir().join(format!("fa-selected-fleet-{}-{}-{}", std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap(); let root = fs::canonicalize(root).unwrap();
        let mut configs = Vec::new();
        for id in 1..=2 {
            let text = include_str!("../../fixtures/supervised_publication.json")
                .replace("/operator/publication-store", root.join(format!("domain-{id}")).to_str().unwrap())
                .replace("\"run\": 3", &format!("\"run\": {id}"));
            let config = Config::decode(text.as_bytes()).unwrap();
            private(&root.join(format!("config-{id}.json")), text.as_bytes());
            let (mut host, _) = FileOversight::create(&config.store, config.profile.clone()).unwrap();
            host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
            host.enable_publication_guard(host.revision()).unwrap();
            host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
            let mut anchor = Vec::new(); host.history_anchor().unwrap().write_to(&mut anchor, MAX_HISTORY_ANCHOR_BYTES).unwrap();
            private(&root.join(format!("domain-{id}.anchor")), &anchor);
            if id == 2 && stopped_second {
                let before = host.inspect();
                host.request_stop(host.revision(), StopRequest { operation: 900,
                    expected_control_sequence: before.control.sequence,
                    expected_authority_epoch: before.control.ledger.epoch }).unwrap();
                assert!(!host.stop_progress().unwrap().drained());
            }
            configs.push(config);
        }
        let members = (1..=2).map(|id| format!("{{\"id\":\"{id}\",\"config\":\"{}/config-{id}.json\",\"history_anchor\":\"{}/domain-{id}.anchor\"}}",
            root.display(), root.display())).collect::<Vec<_>>().join(",");
        let text = format!(concat!("{{\"schema_version\":\"fa.shutdown-fleet/1\",\"operation\":\"900\",",
            "\"coordinator\":\"{}/coordinator\",\"minimum_revision\":\"0\",\"max_visits\":{},",
            "\"max_head_bytes\":4194304,\"coordinator_bytes\":16777216,\"input_bytes\":16777216,",
            "\"runtime_ms\":10000,\"replay_events\":1,\"members\":[{}]}}"), root.display(), visits, members);
        private(&root.join("fleet.json"), text.as_bytes()); Self { root, configs }
    }
    fn initialize(&self) -> FileShutdownPlan {
        let path = self.root.join("fleet.json"); let spec = Spec::read(&path).unwrap();
        let prepared = spec.prepare(&path, &mut || Ok(())).unwrap();
        let mut owner = FileShutdownCoordinator::create(&prepared.spec.coordinator,
            &prepared.plan, prepared.spec.coordinator_bytes).unwrap();
        owner.recover_admission_stop(0, FileShutdownClockBinding { domain: 1, clock_domain: self.configs[0].profile.delivery.clock_domain },
            1, |_| true).unwrap().unwrap();
        private(&self.root.join("before.anchor"), &owner.history_anchor().unwrap().to_bytes().unwrap());
        prepared.plan
    }
    fn args(&self, phase: &str) -> Vec<String> {
        let mut args = vec!["recover".into(), self.root.join("fleet.json").display().to_string(),
            "--recover-domain".into(), "2".into()];
        if !phase.is_empty() { args.push(phase.into()); }
        args
    }
    fn images(&self) -> Vec<Vec<u8>> {
        self.configs.iter().map(|c| fs::read(c.store.join("delivery.bin")).unwrap()).collect()
    }
    fn coordinated(&self) -> Vec<u8> { fs::read(self.root.join("coordinator/delivery.bin")).unwrap() }
}
impl Drop for Fixture {
    fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.root) { eprintln!("selected fleet cleanup: {error}"); } }
}
fn json(bytes: &[u8]) -> Json { strict_json::parse(bytes, Limits::default()).unwrap() }

#[test]
fn selected_operator_drains_at_visit_ceiling_and_refreshes_the_other_receipt() {
    let f = Fixture::new(2, true); let plan = f.initialize(); let before = f.images(); let journal = f.coordinated();
    let plain = vec!["recover".into(), f.root.join("fleet.json").display().to_string()];
    assert!(terminal::command_for_phase(&plain, || panic!("all-member visit allowance"),
        &mut || Duration::ZERO, &mut Vec::new(), Some(phases::Phase::Drain)).is_err());
    assert_eq!(f.images(), before); assert_eq!(f.coordinated(), journal);
    let mut out = Vec::new(); let mut clocks = 0;
    command_with(&f.args("--drain-only"), || { clocks += 1; Ok(ElapsedTick(2)) },
        &mut || Duration::ZERO, &mut out).unwrap();
    assert_eq!(clocks, 1); assert_eq!(f.images()[0], before[0]); assert_ne!(f.images()[1], before[1]);
    let result = json(&out);
    assert_eq!(result.get("schema_version").unwrap().as_str(), Some("fa.shutdown-fleet-result/4"));
    assert_eq!(result.get("phase").unwrap().as_str(), Some("drain"));
    assert_eq!(result.get("selected_domains").unwrap().as_array().unwrap(), &[Json::String("2".into())]);
    assert_eq!(result.get("all_observed_drained").unwrap().as_bool(), Some(true));
    let members = result.get("members").unwrap().as_array().unwrap(); assert_eq!(members.len(), 2);
    assert_eq!(members[0].get("acquired_tick"), Some(&Json::Null));
    assert_eq!(members[1].get("acquired_tick").unwrap().as_str(), Some("2"));
    let owner = FileShutdownCoordinator::open(f.root.join("coordinator"), &plan, 16777216, 4).unwrap();
    assert_eq!(owner.report().visits.len(), 2);
}

#[test]
fn actual_command_routes_clock_free_selection_and_leaves_legacy_schema_unchanged() {
    let f = Fixture::new(2, false); f.initialize(); let before = f.images(); let mut out = Vec::new();
    super::super::command(&f.args("--admission-only"), &mut out).unwrap();
    assert_eq!(f.images()[0], before[0]); assert_ne!(f.images()[1], before[1]);
    assert_eq!(json(&out).get("settlement_requested").unwrap().as_bool(), Some(false));
    let saved = f.coordinated(); let images = f.images(); out.clear();
    let args = vec!["resume".into(), f.root.join("fleet.json").display().to_string()];
    terminal::command_for_phase(&args, || panic!("passive resume is clock-free"),
        &mut || Duration::ZERO, &mut out, Some(phases::Phase::Admission)).unwrap();
    assert_eq!(json(&out).get("schema_version").unwrap().as_str(), Some("fa.shutdown-fleet-result/3"));
    assert!(json(&out).get("selected_domains").is_none());
    assert_eq!(f.coordinated(), saved); assert_eq!(f.images(), images);
}

#[test]
fn closed_selection_parser_refuses_mixed_missing_duplicate_and_value_slot_options() {
    for mode in ["create", "resume", "unknown"] {
        let args = vec![mode.into(), "/missing-profile".into(), "--recover-domain".into(), "2".into()];
        assert!(command_with(&args, || panic!("parse before clock"), &mut || panic!("parse before profile"), &mut Vec::new()).is_err());
    }
    for suffix in [vec!["--recover-domain"], vec!["--recover-domain", "0"], vec!["--recover-domain", "-1"],
        vec!["--recover-domain", "+2"], vec!["--recover-domain", "18446744073709551616"],
        vec!["--recover-domain", "2", "--recover-domain", "02"],
        vec!["--recover-domain", "2", "--admission-only", "--terminal-clocks"],
        vec!["--recover-domain", "2", "--terminal-clocks", "--terminal-clocks"],
        vec!["--coordinator-anchor", "--recover-domain", "2"],
        vec!["--recover-domain", "2", "--retain-coordinator-anchor"],
        vec!["--recover-domain", "2", "--unrecognized"]] {
        let args = ["recover", "/missing-profile"].into_iter().chain(suffix).map(String::from).collect::<Vec<_>>();
        assert!(command_with(&args, || panic!("invalid clock"), &mut || panic!("invalid input"), &mut Vec::new()).is_err());
    }
    let good = ["recover", "/profile", "--recover-domain", "2", "--coordinator-anchor", "/anchor",
        "--recover-domain", "1", "--drain-only"].map(String::from);
    let parsed = parse(&good).unwrap(); assert_eq!(parsed.domains, vec![1, 2]);
    assert_eq!(parsed.phase, Some(phases::Phase::Drain));
    assert_eq!(parsed.args, ["recover", "/profile", "--coordinator-anchor", "/anchor"].map(String::from));
}

#[test]
fn unknown_selection_refuses_before_coordinator_cleanup_and_missing_receipts_stay_visible() {
    let f = Fixture::new(4, true); f.initialize(); let original = f.coordinated(); let before = f.images();
    let pending = f.root.join("coordinator/delivery.pending"); private(&pending, b"preserve");
    let mut unknown = f.args("--drain-only"); unknown[3] = "99".into();
    assert!(command_with(&unknown, || panic!("unknown domain"), &mut || Duration::ZERO, &mut Vec::new()).is_err());
    assert_eq!(fs::read(&pending).unwrap(), b"preserve"); assert_eq!(f.coordinated(), original); assert_eq!(f.images(), before);
    fs::remove_file(f.configs[0].store.join("delivery.bin")).unwrap();
    let mut out = Vec::new();
    assert!(command_with(&f.args("--drain-only"), || Ok(ElapsedTick(2)), &mut || Duration::ZERO, &mut out).is_err());
    let value = json(&out); let members = value.get("members").unwrap().as_array().unwrap();
    assert_eq!(members.len(), 2); assert_eq!(members[0].get("result").unwrap().as_str(), Some("evidence_refused"));
    assert_eq!(members[1].get("result").unwrap().as_str(), Some("observed"));
    assert_eq!(value.get("all_observed_stopped").unwrap().as_bool(), Some(false));
    assert!(!f.configs[0].store.join("delivery.bin").exists());
}

#[test]
fn failed_or_slow_selected_clock_never_changes_the_unselected_or_stopped_domain() {
    for slow in [false, true] {
        let f = Fixture::new(4, true); f.initialize(); let before = f.images(); let called = Cell::new(false); let mut out = Vec::new();
        let error = command_with(&f.args("--drain-only"), || {
            called.set(true); if slow { Ok(ElapsedTick(2)) } else { Err("clock unavailable".into()) }
        }, &mut || if slow && called.get() { Duration::from_secs(10) } else { Duration::ZERO }, &mut out).unwrap_err();
        assert!(called.get()); assert_eq!(f.images(), before);
        let value = json(&out); let members = value.get("members").unwrap().as_array().unwrap();
        assert_eq!(members[0].get("result").unwrap().as_str(), Some("observed"));
        assert_eq!(members[1].get("result").unwrap().as_str(), Some("refused"));
        assert_eq!(members[1].get("acquired_tick"), Some(&Json::Null));
        assert_eq!(value.get("deadline_reached").unwrap().as_bool(), Some(slow));
        if slow { assert!(error.contains("deadline")); }
        command_with(&f.args("--drain-only"), || Ok(ElapsedTick(3)), &mut || Duration::ZERO, &mut Vec::new()).unwrap();
        assert_eq!(f.images()[0], before[0]);
    }
}

#[test]
fn combined_selection_requires_time_only_for_new_work_and_drain_refuses_an_active_member() {
    for combined in [false, true] {
        let f = Fixture::new(4, false); f.initialize(); let before = f.images(); let mut calls = 0; let mut out = Vec::new();
        let result = command_with(&f.args(if combined { "" } else { "--drain-only" }), || {
            calls += 1; Ok(ElapsedTick(2))
        }, &mut || Duration::ZERO, &mut out);
        assert_eq!(result.is_ok(), combined); assert_eq!(calls, usize::from(combined));
        assert_eq!(f.images()[0], before[0]);
        assert_eq!(json(&out).get("phase").unwrap().as_str(), Some(if combined { "combined" } else { "drain" }));
        if !combined { assert_eq!(f.images(), before); }
    }
}

#[test]
fn selected_acknowledgment_and_retained_anchor_survive_lost_stdout_without_repeating_work() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
        fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
    }
    let f = Fixture::new(2, true); f.initialize(); let first = f.root.join("before.anchor"); let second = f.root.join("after.anchor");
    let old = fs::read(&first).unwrap(); let mut args = f.args("--drain-only");
    args.extend(["--coordinator-anchor".into(), first.display().to_string(),
        "--retain-coordinator-anchor".into(), second.display().to_string()]);
    let error = command_with(&args, || Ok(ElapsedTick(2)), &mut || Duration::ZERO, &mut Broken).unwrap_err();
    assert!(error.contains("output failed")); assert_eq!(fs::read(&first).unwrap(), old);
    let anchor = FileShutdownCoordinatorAnchor::from_trusted_bytes(&fs::read(&second).unwrap()).unwrap();
    assert_eq!(anchor.revision(), 4); let images = f.images(); let journal = f.coordinated();
    let args = vec!["resume".into(), f.root.join("fleet.json").display().to_string(),
        "--coordinator-anchor".into(), second.display().to_string()];
    terminal::command_with(&args, || panic!("exact receipts do not repeat selected work"),
        &mut || Duration::ZERO, &mut Vec::new()).unwrap();
    assert_eq!(f.images(), images); assert_eq!(f.coordinated(), journal);
}
