//! Native archived plans and real two-store recovery. No imported success reports.
use super::*;
use super::super::super::command_with;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::{StopRequest, persistent::{RecoveryReserve,
    observed::{FileOversight, shutdown::coordinator::FileShutdownCoordinator}}};
use std::{io::{self, Write}, os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    sync::atomic::{AtomicU64, Ordering}, time::{SystemTime, UNIX_EPOCH}};

static NEXT: AtomicU64 = AtomicU64::new(0);
const BYTES: usize = 16 * 1024 * 1024;
struct Fixture {
    root: PathBuf,
    plan: FileShutdownPlan,
    configs: Vec<Config>,
    hosts: Vec<Option<FileOversight>>,
}
fn private(path: &Path, bytes: &[u8]) {
    let mut file = fs::OpenOptions::new().create(true).truncate(true).write(true).mode(0o600).open(path).unwrap();
    file.write_all(bytes).unwrap();
}
impl Fixture {
    fn new(count: usize, visits: usize) -> Self {
        let root = std::env::temp_dir().join(format!("fa-archived-fleet-{}-{}-{}", std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        let mut hosts = Vec::new(); let mut configs = Vec::new(); let mut domains = Vec::new();
        for id in 1..=count {
            let text = include_str!("../../../../fixtures/supervised_publication.json")
                .replace("/operator/publication-store", root.join(format!("domain-{id}")).to_str().unwrap())
                .replace("\"run\": 3", &format!("\"run\": {id}"));
            private(&root.join(format!("member-{id}.json")), text.as_bytes());
            let config = Config::decode(text.as_bytes()).unwrap();
            let (mut host, _) = FileOversight::create(&config.store, config.profile.clone()).unwrap();
            host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
            host.enable_publication_guard(host.revision()).unwrap();
            host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
            domains.push(host.shutdown_domain(id as u64).unwrap());
            configs.push(config); hosts.push(Some(host));
        }
        let plan = FileShutdownPlan::new(900, domains, visits, MAX_SHUTDOWN_HEAD_BYTES).unwrap();
        let fixture = Self { root, plan, configs, hosts };
        private(&fixture.archive(), &fixture.plan.encode().unwrap());
        fixture.select(0, BYTES);
        fixture
    }
    fn archive(&self) -> PathBuf { self.root.join("retained-plan.bin") }
    fn manifest(&self) -> PathBuf { self.root.join("fleet.json") }
    fn coordinator(&self) -> PathBuf { self.root.join("coordinator") }
    fn text(&self, floor: u64, inputs: usize) -> String {
        let members = (1..=self.configs.len()).map(|id| format!("{{\"id\":\"{id}\",\"config\":\"{}\"}}",
            self.root.join(format!("member-{id}.json")).display())).collect::<Vec<_>>().join(",");
        format!(concat!("{{\"schema_version\":\"fa.shutdown-fleet/2\",\"plan_archive\":\"{}\",",
            "\"operation\":\"900\",\"coordinator\":\"{}\",\"minimum_revision\":\"{}\",",
            "\"max_visits\":{},\"max_head_bytes\":{},\"coordinator_bytes\":{},\"input_bytes\":{},",
            "\"runtime_ms\":10000,\"replay_events\":1,\"members\":[{}]}}"),
            self.archive().display(), self.coordinator().display(), floor, self.plan.max_attempts(),
            self.plan.max_head_bytes(), BYTES, inputs, members)
    }
    fn select(&self, floor: u64, inputs: usize) { private(&self.manifest(), self.text(floor, inputs).as_bytes()); }
    fn release(&mut self) { for host in &mut self.hosts { *host = None; } }
    fn args(&self, mode: &str) -> Vec<String> { vec![mode.into(), self.manifest().display().to_string()] }
    fn invoke(&self, mode: &str, out: &mut impl Write) -> Result<(), String> {
        command_with(&self.args(mode), || Ok(ElapsedTick(2)), &mut || Duration::ZERO, out)
    }
    fn images(&self) -> Vec<Vec<u8>> {
        self.configs.iter().map(|c| fs::read(c.store.join("delivery.bin")).unwrap()).collect()
    }
    fn coordinated(&self) -> Vec<u8> { fs::read(self.coordinator().join("delivery.bin")).unwrap() }
    fn prepare(&self) -> Result<Prepared, String> {
        Spec::read(&self.manifest())?.prepare(&self.manifest(), &mut || Ok(()))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.release();
        if let Err(error) = fs::remove_dir_all(&self.root) { eprintln!("archive fixture cleanup: {error}"); }
    }
}
fn json(out: &[u8]) -> Json { strict_json::parse(out, Limits::default()).unwrap() }
fn results(value: &Json) -> Vec<&str> {
    value.get("members").unwrap().as_array().unwrap().iter()
        .map(|member| member.get("result").unwrap().as_str().unwrap()).collect()
}

#[test]
fn retained_plan_drives_real_roster_and_exact_resume_at_the_visit_ceiling() {
    let mut f = Fixture::new(3, 3);
    assert_eq!(f.prepare().unwrap().plan.encode().unwrap(), f.plan.encode().unwrap());
    f.release();
    assert!(!f.root.join("member-1.anchor").exists(), "no per-member anchor regeneration");
    let mut out = Vec::new(); f.invoke("create", &mut out).unwrap();
    assert_eq!(results(&json(&out)), vec!["observed"; 3]);
    let native = f.images(); let coordinator = f.coordinated();
    f.select(6, BYTES); out.clear(); f.invoke("resume", &mut out).unwrap();
    assert_eq!(json(&out).get("final_revision").unwrap().as_str(), Some("6"));
    assert_eq!(json(&out).get("all_observed_drained").unwrap().as_bool(), Some(true));
    assert_eq!(f.images(), native); assert_eq!(f.coordinated(), coordinator);
    out.clear(); assert!(f.invoke("recover", &mut out).is_err());
    assert!(out.is_empty()); assert_eq!(f.images(), native);
}

#[test]
fn archive_and_anchor_profiles_reconstruct_identical_coordinator_registration() {
    let mut f = Fixture::new(2, 4);
    for (index, host) in f.hosts.iter().enumerate() {
        let mut bytes = Vec::new();
        host.as_ref().unwrap().history_anchor().unwrap().write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES).unwrap();
        private(&f.root.join(format!("member-{}.anchor", index + 1)), &bytes);
    }
    let mut v1 = f.text(0, BYTES).replace("fa.shutdown-fleet/2", "fa.shutdown-fleet/1")
        .replace(&format!("\"plan_archive\":\"{}\",", f.archive().display()), "");
    for id in 1..=2 {
        v1 = v1.replace(&format!("\"id\":\"{id}\",\"config\""),
            &format!("\"id\":\"{id}\",\"history_anchor\":\"{}\",\"config\"", f.root.join(format!("member-{id}.anchor")).display()));
    }
    private(&f.manifest(), v1.as_bytes());
    assert_eq!(f.prepare().unwrap().plan.encode().unwrap(), f.plan.encode().unwrap());
    f.release(); f.invoke("create", &mut Vec::new()).unwrap();
    let coordinator = f.coordinated(); let native = f.images();
    for id in 1..=2 { fs::remove_file(f.root.join(format!("member-{id}.anchor"))).unwrap(); }
    f.select(4, BYTES); f.invoke("resume", &mut Vec::new()).unwrap();
    assert_eq!(f.coordinated(), coordinator); assert_eq!(f.images(), native);
}

#[test]
fn archived_pending_visits_are_resolved_without_repeating_an_unstopped_command() {
    let mut f = Fixture::new(3, 3); f.release();
    let mut coordinator = FileShutdownCoordinator::create(f.coordinator(), &f.plan, BYTES).unwrap();
    drop(coordinator.begin_recover_and_drain(0, 1, ElapsedTick(2)).unwrap().unwrap());
    drop(coordinator);
    let first = &f.configs[0];
    let before = FileOversight::read_publication(&first.store, &first.profile).unwrap();
    FileOversight::recover_stopped(&first.store, first.profile.clone(), StopRequest {
        operation: 900, expected_control_sequence: before.control.sequence,
        expected_authority_epoch: before.control.ledger.epoch,
    }, || ElapsedTick(2)).unwrap();
    let mut coordinator = FileShutdownCoordinator::open(f.coordinator(), &f.plan, BYTES, 1).unwrap();
    drop(coordinator.begin_recover_and_drain(1, 2, ElapsedTick(2)).unwrap().unwrap());
    drop(coordinator);
    let native = f.images(); f.select(2, BYTES);
    let mut out = Vec::new(); assert!(f.invoke("resume", &mut out).is_err());
    assert_eq!(results(&json(&out)), vec!["observed", "evidence_refused", "observed"]);
    assert_eq!(json(&out).get("final_revision").unwrap().as_str(), Some("5"));
    assert_eq!(&f.images()[..2], &native[..2]); assert_ne!(f.images()[2], native[2]);
}

#[test]
fn complete_roster_configuration_operation_and_limits_are_independent_pins() {
    let f = Fixture::new(2, 4); let native = f.images(); let original = f.text(0, BYTES);
    let saved = fs::read(f.root.join("member-2.json")).unwrap();
    let wrong = String::from_utf8(saved.clone()).unwrap().replace("\"total_units\": 4096", "\"total_units\": 4097");
    private(&f.root.join("member-2.json"), wrong.as_bytes());
    assert!(f.prepare().is_err()); private(&f.root.join("member-2.json"), &saved);
    for changed in [original.replace("\"operation\":\"900\"", "\"operation\":\"901\""),
        original.replace("\"max_visits\":4", "\"max_visits\":5"),
        original.replace("\"max_head_bytes\":33554432", "\"max_head_bytes\":33554431"),
        original.replace("\"id\":\"2\"", "\"id\":\"3\"")] {
        private(&f.manifest(), changed.as_bytes());
        assert!(command_with(&f.args("create"), || panic!("independent mismatch before clock"),
            &mut || Duration::ZERO, &mut Vec::new()).is_err());
        assert!(!f.coordinator().exists()); assert_eq!(f.images(), native);
    }
    f.select(0, BYTES); assert!(f.prepare().is_ok());
}

#[test]
fn closed_versioned_intake_never_guesses_or_falls_back_to_other_custody() {
    let f = Fixture::new(2, 4); let original = f.text(0, BYTES);
    for changed in [original.replace("fa.shutdown-fleet/2", "fa.shutdown-fleet/1"),
        original.replace("fa.shutdown-fleet/2", "fa.shutdown-fleet/3"),
        original.replace(&format!("\"plan_archive\":\"{}\",", f.archive().display()), ""),
        original.replace("\"id\":\"1\",", "\"id\":\"1\",\"history_anchor\":\"/no/fallback\","),
        original.replace("\"members\":[", "\"extra\":true,\"members\":["),
        original.replace("\"id\":\"2\"", "\"id\":\"1\"")] {
        assert!(Spec::decode(changed.as_bytes()).is_err());
    }
    let archive = fs::read(f.archive()).unwrap();
    for bytes in [&archive[..archive.len() - 1], b"not a plan".as_slice()] {
        private(&f.archive(), bytes); assert!(f.prepare().is_err());
    }
    let mut trailing = archive.clone(); trailing.push(0); private(&f.archive(), &trailing);
    assert!(f.prepare().is_err());
    private(&f.archive(), &archive); assert!(f.prepare().is_ok());
    assert!(!f.coordinator().exists());
}

#[test]
fn archive_bytes_share_the_original_aggregate_allowance_with_manifest_and_configs() {
    let f = Fixture::new(2, 4);
    let inputs = fs::read(f.archive()).unwrap().len()
        + (1..=2).map(|id| fs::read(f.root.join(format!("member-{id}.json"))).unwrap().len()).sum::<usize>();
    let mut exact = inputs;
    loop {
        let next = inputs + f.text(0, exact).len();
        if next == exact { break; } exact = next;
    }
    f.select(0, exact - 1); assert!(f.prepare().is_err());
    f.select(0, exact); assert!(f.prepare().is_ok());
    assert!(!f.coordinator().exists());
}

#[test]
fn missing_offline_domains_stay_registered_but_private_custody_must_be_available() {
    let mut f = Fixture::new(2, 4); f.release();
    let hidden = f.root.join("offline-domain"); fs::rename(&f.configs[0].store, &hidden).unwrap();
    assert!(f.prepare().is_ok(), "registration does not open domains");
    let mut out = Vec::new(); assert!(f.invoke("create", &mut out).is_err());
    assert_eq!(results(&json(&out)), vec!["refused", "observed"]);
    fs::rename(hidden, &f.configs[0].store).unwrap();
    let saved = fs::read(f.archive()).unwrap(); let coordinator = f.coordinated();
    fs::set_permissions(f.archive(), fs::Permissions::from_mode(0o644)).unwrap();
    assert!(f.prepare().is_err()); fs::set_permissions(f.archive(), fs::Permissions::from_mode(0o600)).unwrap();
    let nested = f.configs[0].store.join("bad-custody.bin"); private(&nested, &saved);
    private(&f.manifest(), f.text(0, BYTES).replace(f.archive().to_str().unwrap(), nested.to_str().unwrap()).as_bytes());
    assert!(f.prepare().is_err()); assert_eq!(f.coordinated(), coordinator);
    f.select(2, BYTES); f.invoke("recover", &mut Vec::new()).unwrap();
}

#[test]
fn archive_command_deadlines_and_failed_output_preserve_original_recovery_semantics() {
    let mut control = Fixture::new(1, 1); control.release(); let mut checkpoints = 0;
    command_with(&control.args("create"), || Ok(ElapsedTick(2)), &mut || {
        checkpoints += 1; Duration::ZERO
    }, &mut Vec::new()).unwrap();
    for boundary in 1..=checkpoints {
        let mut f = Fixture::new(1, 1); f.release(); let before = f.images(); let mut calls = 0;
        assert!(command_with(&f.args("create"), || Ok(ElapsedTick(2)), &mut || {
            calls += 1; if calls >= boundary { Duration::from_secs(10) } else { Duration::ZERO }
        }, &mut Vec::new()).is_err());
        assert_eq!(f.images(), before, "boundary {boundary}/{checkpoints}");
    }
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
        fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
    }
    let mut f = Fixture::new(1, 1); f.release();
    assert!(f.invoke("create", &mut Broken).unwrap_err().contains("output failed"));
    let before = f.images(); let coordinator = f.coordinated();
    f.select(2, BYTES); f.invoke("resume", &mut Vec::new()).unwrap();
    assert_eq!(f.images(), before); assert_eq!(f.coordinated(), coordinator);
}
