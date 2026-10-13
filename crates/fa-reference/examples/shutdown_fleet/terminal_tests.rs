//! Runnable file intake and original native settlement; no helper processes.
use super::*;
use crate::config::Config;
use fa_reference::action::consequence::delivery::persistent::{RecoveryReserve,
    observed::{FileOversight, guarded::anchored::MAX_HISTORY_ANCHOR_BYTES,
        shutdown::coordinator::FileShutdownCoordinatorAnchor}};
use fa_reference::strict_json::{self, Json, Limits};
use std::{fs, io, os::unix::fs::{DirBuilderExt, OpenOptionsExt}, path::PathBuf,
    sync::atomic::{AtomicU64, Ordering}};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture { root: PathBuf, configs: Vec<Config> }
fn private(path: &Path, bytes: &[u8]) {
    let mut file = fs::OpenOptions::new().create_new(true).write(true).mode(0o600).open(path).unwrap();
    file.write_all(bytes).unwrap();
}
impl Fixture {
    fn new(count: usize) -> Self {
        let root = std::env::temp_dir().join(format!("fa-terminal-fleet-{}-{}-{}", std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap(); let root = fs::canonicalize(root).unwrap();
        let mut configs = Vec::new();
        for id in 1..=count {
            let text = include_str!("../../fixtures/supervised_publication.json")
                .replace("/operator/publication-store", root.join(format!("domain-{id}")).to_str().unwrap())
                .replace("\"run\": 3", &format!("\"run\": {id}"));
            private(&root.join(format!("member-{id}.json")), text.as_bytes());
            let config = Config::decode(text.as_bytes()).unwrap();
            let (mut host, _) = FileOversight::create(&config.store, config.profile.clone()).unwrap();
            host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
            host.enable_publication_guard(host.revision()).unwrap();
            host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
            let mut bytes = Vec::new(); host.history_anchor().unwrap().write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES).unwrap();
            private(&root.join(format!("member-{id}.anchor")), &bytes);
            configs.push(config);
        }
        let members = (1..=count).map(|id| format!(
            "{{\"id\":\"{id}\",\"config\":\"{}/member-{id}.json\",\"history_anchor\":\"{}/member-{id}.anchor\"}}",
            root.display(), root.display())).collect::<Vec<_>>().join(",");
        let text = format!(concat!("{{\"schema_version\":\"fa.shutdown-fleet/1\",\"operation\":\"900\",",
            "\"coordinator\":\"{}\",\"minimum_revision\":\"0\",\"max_visits\":{},",
            "\"max_head_bytes\":4194304,\"coordinator_bytes\":16777216,\"input_bytes\":16777216,",
            "\"runtime_ms\":10000,\"replay_events\":1,\"members\":[{}]}}"), root.join("coordinator").display(), count, members);
        private(&root.join("fleet.json"), text.as_bytes()); Self { root, configs }
    }
    fn args(&self, mode: &str) -> Vec<String> { vec![mode.into(), self.root.join("fleet.json").display().to_string()] }
    fn image(&self, member: usize) -> Vec<u8> { fs::read(self.configs[member].store.join("delivery.bin")).unwrap() }
}
impl Drop for Fixture {
    fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.root) { eprintln!("terminal fleet cleanup: {error}"); } }
}
fn parse(bytes: &[u8]) -> Json { strict_json::parse(bytes, Limits::default()).unwrap() }
fn members(json: &Json) -> &[Json] { json.get("members").unwrap().as_array().unwrap() }

#[test]
fn distinct_late_ticks_are_reported_and_applied_after_the_prior_member_settles() {
    let f = Fixture::new(2); let mut calls = 0; let mut out = Vec::new();
    command_with(&f.args("create"), || {
        calls += 1;
        if calls == 2 {
            let first = FileOversight::read_publication(&f.configs[0].store, &f.configs[0].profile).unwrap();
            assert!(first.stop.is_some()); assert_eq!(first.control.ledger.elapsed, Some(ElapsedTick(2)));
        }
        Ok(ElapsedTick(calls + 1))
    }, &mut || Duration::ZERO, &mut out).unwrap();
    assert_eq!(calls, 2); let json = parse(&out);
    assert_eq!(json.get("schema_version").unwrap().as_str(), Some("fa.shutdown-fleet-result/2"));
    assert_eq!(members(&json)[0].get("acquired_tick").unwrap().as_str(), Some("2"));
    assert_eq!(members(&json)[1].get("acquired_tick").unwrap().as_str(), Some("3"));
    assert_eq!(json.get("all_observed_drained").unwrap().as_bool(), Some(true));
    for (i, config) in f.configs.iter().enumerate() {
        let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(state.control.ledger.elapsed, Some(ElapsedTick(i as u64 + 2)));
    }
}

#[test]
fn exact_resume_at_the_visit_ceiling_needs_no_clock_and_changes_no_domain_image() {
    let f = Fixture::new(2); let mut out = Vec::new();
    command_with(&f.args("create"), || Ok(ElapsedTick(2)), &mut || Duration::ZERO, &mut out).unwrap();
    let images = [f.image(0), f.image(1)]; let coordinator = fs::read(f.root.join("coordinator/delivery.bin")).unwrap();
    out.clear();
    command_with(&f.args("resume"), || panic!("receipt-only resume must not acquire time"), &mut || Duration::ZERO, &mut out).unwrap();
    let json = parse(&out); assert_eq!(json.get("final_revision").unwrap().as_str(), Some("4"));
    assert!(members(&json).iter().all(|m| matches!(m.get("acquired_tick"), Some(Json::Null))));
    assert_eq!([f.image(0), f.image(1)], images);
    assert_eq!(fs::read(f.root.join("coordinator/delivery.bin")).unwrap(), coordinator);
}

#[test]
fn a_failed_clock_is_explicit_and_does_not_prevent_a_later_reachable_member() {
    let f = Fixture::new(2); let original = f.image(0); let mut calls = 0; let mut out = Vec::new();
    let error = command_with(&f.args("create"), || {
        calls += 1;
        if calls == 1 { Err("clock unavailable".into()) } else { Ok(ElapsedTick(2)) }
    }, &mut || Duration::ZERO, &mut out).unwrap_err();
    assert!(error.contains("incomplete")); assert_eq!(calls, 2); assert_eq!(f.image(0), original);
    let json = parse(&out);
    assert_eq!(members(&json)[0].get("result").unwrap().as_str(), Some("refused"));
    assert!(matches!(members(&json)[0].get("acquired_tick"), Some(Json::Null)));
    assert_eq!(members(&json)[1].get("result").unwrap().as_str(), Some("observed"));
    assert_eq!(members(&json)[1].get("acquired_tick").unwrap().as_str(), Some("2"));
}

#[test]
fn deadline_expiry_during_clock_acquisition_prevents_settlement_and_preserves_the_tail() {
    let f = Fixture::new(2); let original = [f.image(0), f.image(1)]; let expired = Cell::new(false);
    let mut out = Vec::new();
    let error = command_with(&f.args("create"), || { expired.set(true); Ok(ElapsedTick(2)) },
        &mut || if expired.get() { Duration::from_secs(10) } else { Duration::ZERO }, &mut out).unwrap_err();
    assert!(error.contains("deadline")); assert_eq!([f.image(0), f.image(1)], original);
    let json = parse(&out); assert_eq!(json.get("deadline_reached").unwrap().as_bool(), Some(true));
    assert_eq!(members(&json)[0].get("result").unwrap().as_str(), Some("refused"));
    assert_eq!(members(&json)[1].get("result").unwrap().as_str(), Some("not_visited"));
    assert!(members(&json).iter().all(|m| matches!(m.get("acquired_tick"), Some(Json::Null))));
}

#[test]
fn clock_unwind_leaves_only_intent_and_resume_never_retries_the_command() {
    let f = Fixture::new(1); let original = f.image(0); let mut out = Vec::new();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| command_with(&f.args("create"),
        || panic!("clock interrupted"), &mut || Duration::ZERO, &mut out))).is_err());
    assert!(out.is_empty()); assert_eq!(f.image(0), original);
    assert!(command_with(&f.args("resume"), || panic!("no retry"), &mut || Duration::ZERO, &mut out).is_err());
    let json = parse(&out);
    assert_eq!(members(&json)[0].get("result").unwrap().as_str(), Some("evidence_refused"));
    assert_eq!(json.get("final_revision").unwrap().as_str(), Some("1")); assert_eq!(f.image(0), original);
}

#[test]
fn explicit_flag_preserves_legacy_selection_and_rejects_duplicate_options() {
    let f = Fixture::new(1);
    assert!(take_flag(&f.args("create")).unwrap().is_none());
    let mut args = f.args("create"); args.push("--terminal-clocks".into());
    assert_eq!(take_flag(&args).unwrap(), Some(f.args("create")));
    args.push("--terminal-clocks".into()); assert!(take_flag(&args).is_err());
    assert!(!f.root.join("coordinator").exists());
    args.pop(); let mut out = Vec::new();
    super::super::command(&args, &mut out).unwrap(); // Actual CLI routing and Unix clock.
    assert_eq!(parse(&out).get("schema_version").unwrap().as_str(), Some("fa.shutdown-fleet-result/2"));
    let legacy = Fixture::new(1); let mut out = Vec::new();
    super::super::command_with(&legacy.args("create"), || Ok(ElapsedTick(2)), &mut || Duration::ZERO, &mut out).unwrap();
    let json = parse(&out); assert_eq!(json.get("schema_version").unwrap().as_str(), Some("fa.shutdown-fleet-result/1"));
    assert!(members(&json)[0].get("acquired_tick").is_none());
}

#[test]
fn archive_retention_precedes_failed_stdout_and_anchored_resume_is_clock_free() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
        fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
    }
    let f = Fixture::new(1); let archive = f.root.join("after.anchor");
    let mut args = f.args("create");
    args.extend(["--retain-coordinator-anchor".into(), archive.display().to_string()]);
    let error = command_with(&args, || Ok(ElapsedTick(2)), &mut || Duration::ZERO, &mut Broken).unwrap_err();
    assert!(error.contains("output failed")); let raw = fs::read(&archive).unwrap();
    assert_eq!(FileShutdownCoordinatorAnchor::from_trusted_bytes(&raw).unwrap().revision(), 2);
    let native = f.image(0); let mut args = f.args("resume");
    args.extend(["--coordinator-anchor".into(), archive.display().to_string()]);
    let mut out = Vec::new();
    command_with(&args, || panic!("already complete"), &mut || Duration::ZERO, &mut out).unwrap();
    assert_eq!(parse(&out).get("all_observed_drained").unwrap().as_bool(), Some(true));
    assert_eq!(f.image(0), native); assert_eq!(fs::read(archive).unwrap(), raw);
}
