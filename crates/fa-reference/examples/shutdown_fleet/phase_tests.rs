//! Executable phase selection, original private inputs and real coordinator stores.
use super::*;
use super::super::{command, terminal};
use crate::config::Config;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::{RecoveryReserve,
    observed::{FileOversight, guarded::anchored::MAX_HISTORY_ANCHOR_BYTES,
        shutdown::coordinator::FileShutdownCoordinatorAnchor}};
use fa_reference::strict_json::{self, Json, Limits};
use std::{fs::{self, OpenOptions}, io::{self, Write}, os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf}, sync::atomic::{AtomicU64, Ordering}, time::{Duration, SystemTime, UNIX_EPOCH}};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture { root: PathBuf, configs: Vec<Config> }
fn private(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path).unwrap();
    file.write_all(bytes).unwrap();
}
impl Fixture {
    fn new(count: usize) -> Self {
        let root = std::env::temp_dir().join(format!("fa-phased-fleet-{}-{}-{}", std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap(); let root = fs::canonicalize(root).unwrap();
        let mut configs = Vec::new();
        for id in 1..=count {
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
            private(&root.join(format!("domain-{id}.anchor")), &anchor); configs.push(config);
        }
        let members = (1..=count).map(|id| format!("{{\"id\":\"{id}\",\"config\":\"{}/config-{id}.json\",\"history_anchor\":\"{}/domain-{id}.anchor\"}}",
            root.display(), root.display())).collect::<Vec<_>>().join(",");
        let text = format!(concat!("{{\"schema_version\":\"fa.shutdown-fleet/1\",\"operation\":\"900\",",
            "\"coordinator\":\"{}/coordinator\",\"minimum_revision\":\"0\",\"max_visits\":16,",
            "\"max_head_bytes\":4194304,\"coordinator_bytes\":16777216,\"input_bytes\":16777216,",
            "\"runtime_ms\":10000,\"replay_events\":1,\"members\":[{}]}}"), root.display(), members);
        private(&root.join("fleet.json"), text.as_bytes()); Self { root, configs }
    }
    fn args(&self, mode: &str, phase: Phase) -> Vec<String> {
        vec![mode.into(), self.root.join("fleet.json").display().to_string(),
            if phase == Phase::Admission { "--admission-only".into() } else { "--drain-only".into() }]
    }
    fn images(&self) -> Vec<Vec<u8>> {
        self.configs.iter().map(|c| fs::read(c.store.join("delivery.bin")).unwrap()).collect()
    }
    fn coordinated(&self) -> Vec<u8> { fs::read(self.root.join("coordinator/delivery.bin")).unwrap() }
}
impl Drop for Fixture {
    fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.root) { eprintln!("phased fleet cleanup: {error}"); } }
}
fn parse(bytes: &[u8]) -> Json { strict_json::parse(bytes, Limits::default()).unwrap() }
fn execute(args: &[String], clock: impl FnMut() -> Result<ElapsedTick, String>,
    elapsed: &mut impl FnMut() -> Duration, out: &mut impl Write) -> Result<(), String> {
    let (phase, clean) = take_flag(args)?.expect("explicit phase");
    terminal::command_for_phase(&clean, clock, elapsed, out, Some(phase))
}

#[test]
fn actual_flags_stop_and_resume_the_full_roster_without_logical_time() {
    let f = Fixture::new(2); let mut out = Vec::new();
    execute(&f.args("create", Phase::Admission), || panic!("admission clock"), &mut || Duration::ZERO, &mut out).unwrap();
    let json = parse(&out);
    assert_eq!(json.get("schema_version").unwrap().as_str(), Some("fa.shutdown-fleet-result/3"));
    assert_eq!(json.get("phase").unwrap().as_str(), Some("admission"));
    assert_eq!(json.get("settlement_requested").unwrap().as_bool(), Some(false));
    assert_eq!(json.get("all_observed_stopped").unwrap().as_bool(), Some(true));
    for member in json.get("members").unwrap().as_array().unwrap() {
        assert_eq!(member.get("acquired_tick"), Some(&Json::Null));
        assert_eq!(member.get("endpoint_fenced").unwrap().as_bool(), Some(true));
    }
    let before = f.images(); let control = f.coordinated();
    execute(&f.args("resume", Phase::Admission), || panic!("receipt time"), &mut || Duration::ZERO, &mut Vec::new()).unwrap();
    command(&f.args("resume", Phase::Admission), &mut Vec::new()).unwrap();
    assert_eq!(f.images(), before); assert_eq!(f.coordinated(), control);
    execute(&f.args("recover", Phase::Drain), || panic!("already drained"), &mut || Duration::ZERO, &mut Vec::new()).unwrap();
    assert_eq!(f.images(), before);
}

#[test]
fn selected_drain_cannot_silently_stop_active_domains() {
    let f = Fixture::new(2); let before = f.images(); let mut out = Vec::new();
    assert!(execute(&f.args("create", Phase::Drain), || panic!("active clock"), &mut || Duration::ZERO, &mut out).is_err());
    assert_eq!(f.images(), before);
    let json = parse(&out); assert_eq!(json.get("phase").unwrap().as_str(), Some("drain"));
    for member in json.get("members").unwrap().as_array().unwrap() {
        assert_eq!(member.get("result").unwrap().as_str(), Some("refused"));
        assert_eq!(member.get("error").unwrap().as_str(), Some("wrong_state"));
    }
    execute(&f.args("recover", Phase::Admission), || panic!("explicit admission"), &mut || Duration::ZERO, &mut Vec::new()).unwrap();
    assert_ne!(f.images(), before);
}

#[test]
fn mixed_duplicate_missing_and_misplaced_phase_flags_refuse_before_profile_intake() {
    let base = vec!["create".to_owned(), "/not-a-fleet-profile".to_owned()];
    for suffix in [vec!["--admission-only", "--drain-only"], vec!["--admission-only", "--admission-only"],
        vec!["--terminal-clocks", "--admission-only"], vec!["--drain-only", "--unknown"],
        vec!["--coordinator-anchor", "--admission-only", "/anchor"],
        vec!["--admission-only", "--retain-coordinator-anchor"]] {
        let args = base.iter().cloned().chain(suffix.into_iter().map(String::from)).collect::<Vec<_>>();
        assert!(take_flag(&args).is_err()); assert!(command(&args, &mut Vec::new()).is_err());
    }
    let args = vec!["resume".into(), "/profile".into(), "--coordinator-anchor".into(), "/anchor".into(), "--admission-only".into()];
    let (phase, clean) = take_flag(&args).unwrap().unwrap();
    assert_eq!(phase, Phase::Admission); assert_eq!(clean, args[..4]);
    assert!(take_flag(&base).unwrap().is_none());
}

#[test]
fn every_admission_deadline_boundary_preserves_the_prior_domain_cut() {
    let f = Fixture::new(1); let mut calls = 0;
    execute(&f.args("create", Phase::Admission), || panic!("no clock"), &mut || { calls += 1; Duration::ZERO }, &mut Vec::new()).unwrap();
    for cut in 1..=calls {
        let f = Fixture::new(1); let before = f.images(); let mut seen = 0; let mut out = Vec::new();
        assert!(execute(&f.args("create", Phase::Admission), || panic!("no clock"), &mut || {
            seen += 1; if seen >= cut { Duration::from_secs(10) } else { Duration::ZERO }
        }, &mut out).is_err());
        assert_eq!(f.images(), before, "elapsed cut {cut}/{calls}");
        if !out.is_empty() {
            assert_eq!(parse(&out).get("all_observed_stopped").unwrap().as_bool(), Some(false));
        }
    }
}

#[test]
fn retained_custody_survives_lost_output_and_clock_free_resume() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
        fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
    }
    let f = Fixture::new(1); let first = f.root.join("first.anchor"); let second = f.root.join("second.anchor");
    let mut args = f.args("create", Phase::Admission);
    args.extend(["--retain-coordinator-anchor".into(), first.display().to_string()]);
    let error = execute(&args, || panic!("admission time"), &mut || Duration::ZERO, &mut Broken).unwrap_err();
    assert!(error.contains("output failed"));
    let anchor = FileShutdownCoordinatorAnchor::from_trusted_bytes(&fs::read(&first).unwrap()).unwrap();
    let before = f.images(); let control = f.coordinated();
    let mut args = f.args("resume", Phase::Admission);
    args.extend(["--coordinator-anchor".into(), first.display().to_string(),
        "--retain-coordinator-anchor".into(), second.display().to_string()]);
    execute(&args, || panic!("resolution time"), &mut || Duration::ZERO, &mut Vec::new()).unwrap();
    let next = FileShutdownCoordinatorAnchor::from_trusted_bytes(&fs::read(&second).unwrap()).unwrap();
    assert_eq!(anchor, next); assert_eq!(f.images(), before); assert_eq!(f.coordinated(), control);
}
