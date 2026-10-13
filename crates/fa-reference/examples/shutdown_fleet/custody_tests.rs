//! Actual operator/coordinator files; independent archives are not signatures.
use super::*;
use fa_reference::action::consequence::delivery::persistent::{MAX_JOURNAL_BYTES, RecoveryReserve};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight,
    guarded::anchored::MAX_HISTORY_ANCHOR_BYTES,
    shutdown::{FileShutdownPlan, coordinator::FileShutdownCoordinatorAnchor}};
use fa_reference::strict_json::{self, Json, Limits};
use std::{fs::{self, DirBuilder, OpenOptions}, io, os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::PathBuf, sync::atomic::{AtomicU64, Ordering}};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Rig { root: PathBuf, plan: FileShutdownPlan }
fn private(path: &Path, bytes: &[u8]) {
    let mut f = OpenOptions::new().create(true).truncate(true).write(true).mode(0o600).open(path).unwrap();
    f.write_all(bytes).unwrap();
}
impl Rig {
    fn new(archived: bool) -> Self {
        let root = std::env::temp_dir().join(format!("fa-fleet-custody-{}-{}-{}", std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(), NEXT.fetch_add(1, Ordering::Relaxed)));
        DirBuilder::new().mode(0o700).create(&root).unwrap(); let root = fs::canonicalize(root).unwrap();
        let config = include_str!("../../fixtures/supervised_publication.json")
            .replace("/operator/publication-store", root.join("domain").to_str().unwrap());
        private(&root.join("member.json"), config.as_bytes());
        let c = crate::config::Config::decode(config.as_bytes()).unwrap();
        let (mut host, _) = FileOversight::create(&c.store, c.profile).unwrap();
        host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
        host.enable_publication_guard(host.revision()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let mut bytes = Vec::new(); host.history_anchor().unwrap().write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES).unwrap();
        private(&root.join("member.anchor"), &bytes);
        let plan = FileShutdownPlan::new(900, vec![host.shutdown_domain(1).unwrap()], 8, 4194304).unwrap();
        private(&root.join("plan.bin"), &plan.encode().unwrap()); drop(host);
        let rig = Self { root, plan };
        private(&rig.manifest(), rig.document(archived, 16777216).as_bytes()); rig
    }
    fn manifest(&self) -> PathBuf { self.root.join("fleet.json") }
    fn store(&self) -> PathBuf { self.root.join("coordinator") }
    fn document(&self, archive: bool, input: usize) -> String {
        let version = if archive { 2 } else { 1 };
        let selection = if archive { format!("\"plan_archive\":\"{}/plan.bin\",", self.root.display()) } else { String::new() };
        let anchor = if archive { String::new() } else { format!(",\"history_anchor\":\"{}/member.anchor\"", self.root.display()) };
        format!(concat!("{{\"schema_version\":\"fa.shutdown-fleet/{}\",{}\"operation\":\"900\",",
            "\"coordinator\":\"{}\",\"minimum_revision\":\"0\",\"max_visits\":8,\"max_head_bytes\":4194304,",
            "\"coordinator_bytes\":16777216,\"input_bytes\":{},\"runtime_ms\":10000,\"replay_events\":1,",
            "\"members\":[{{\"id\":\"1\",\"config\":\"{}/member.json\"{}}}]}}"),
            version, selection, self.store().display(), input, self.root.display(), anchor)
    }
    fn args(&self, mode: &str, input: Option<&Path>, output: Option<&Path>) -> Vec<String> {
        let mut args = vec![mode.to_owned(), self.manifest().display().to_string()];
        if let Some(path) = input { args.extend(["--coordinator-anchor".into(), path.display().to_string()]); }
        if let Some(path) = output { args.extend(["--retain-coordinator-anchor".into(), path.display().to_string()]); }
        args
    }
    fn image(&self) -> Vec<u8> { fs::read(self.store().join("delivery.bin")).unwrap() }
    fn domain(&self) -> Vec<u8> { fs::read(self.root.join("domain/delivery.bin")).unwrap() }
    fn run(&self, mode: &str, input: Option<&Path>, output: Option<&Path>) -> (Result<(), String>, Vec<u8>) {
        let mut out = Vec::new();
        let result = command_with(&self.args(mode, input, output), || Ok(ElapsedTick(2)), &mut || Duration::ZERO, &mut out);
        (result, out)
    }
}
impl Drop for Rig {
    fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.root) { eprintln!("custody cleanup: {error}"); } }
}
fn parse(out: &[u8]) -> Json { strict_json::parse(out, Limits::default()).unwrap() }
fn read_anchor(path: &Path) -> FileShutdownCoordinatorAnchor {
    FileShutdownCoordinatorAnchor::from_trusted_bytes(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn both_roster_formats_export_and_require_the_same_acknowledged_coordinator_cut() {
    for archived in [false, true] {
        let r = Rig::new(archived); let first = r.root.join("first.anchor"); let next = r.root.join("next.anchor");
        let (result, out) = r.run("create", None, Some(&first)); result.unwrap();
        assert_eq!(parse(&out).get("all_observed_drained").unwrap().as_bool(), Some(true));
        let old = fs::read(&first).unwrap(); let native = r.domain(); let control = r.image();
        let (result, out) = r.run("resume", Some(&first), Some(&next)); result.unwrap();
        assert_eq!(parse(&out).get("final_revision").unwrap().as_str(), Some("2"));
        assert_eq!(read_anchor(&first), read_anchor(&next)); assert_eq!(fs::read(&first).unwrap(), old);
        assert_eq!(r.image(), control); assert_eq!(r.domain(), native);
        assert_eq!(fs::metadata(&next).unwrap().permissions().mode() & 0o077, 0);
    }
}

#[test]
fn a_valid_counter_equal_fork_refuses_before_cleanup_or_any_member_work() {
    let r = Rig::new(false); let mut c = FileShutdownCoordinator::create(r.store(), &r.plan, MAX_JOURNAL_BYTES).unwrap();
    c.unavailable(0, 1, fa_reference::action::consequence::delivery::persistent::JournalError::Busy).unwrap();
    let anchor = r.root.join("required.anchor"); private(&anchor, &c.history_anchor().unwrap().to_bytes().unwrap()); drop(c);
    let native = r.domain(); let mut fork = r.image();
    let places: Vec<_> = fork.windows(4).enumerate().filter(|(_, w)| *w == b"Busy").map(|(i, _)| i).collect();
    assert_eq!(places.len(), 1); fork[places[0]..places[0] + 4].copy_from_slice(b"Bury");
    private(&r.store().join("delivery.bin"), &fork);
    drop(FileShutdownCoordinator::open(r.store(), &r.plan, MAX_JOURNAL_BYTES, 2).unwrap());
    let pending = r.store().join("delivery.pending"); private(&pending, b"preserve");
    let (result, out) = r.run("resume", Some(&anchor), None);
    assert!(result.unwrap_err().contains("coordinator acquisition")); assert!(out.is_empty());
    assert!(pending.exists()); assert_eq!(r.image(), fork); assert_eq!(r.domain(), native);
}

#[test]
fn closed_options_private_custody_and_existing_outputs_refuse_before_native_work() {
    let r = Rig::new(false); let native = r.domain();
    let nonexistent = r.root.join("missing.anchor");
    for args in [r.args("create", Some(&nonexistent), None),
        vec!["resume".into(), r.manifest().display().to_string(), "--coordinator-anchor".into()],
        vec!["resume".into(), r.manifest().display().to_string(), "--force".into(), "yes".into()],
        vec!["resume".into(), r.manifest().display().to_string(), "--coordinator-anchor".into(), nonexistent.display().to_string(),
            "--coordinator-anchor".into(), nonexistent.display().to_string()]] {
        assert!(command_with(&args, || panic!("admission before clock"), &mut || Duration::ZERO, &mut Vec::new()).is_err());
        assert!(!r.store().exists()); assert_eq!(r.domain(), native);
    }
    let destination = r.root.join("old.anchor"); private(&destination, b"old");
    assert!(r.run("create", None, Some(&destination)).0.is_err()); assert_eq!(fs::read(&destination).unwrap(), b"old");
    let nested = r.root.join("domain/new.anchor"); assert!(r.run("create", None, Some(&nested)).0.is_err());
    assert!(!r.store().exists());
    r.run("create", None, None).0.unwrap();
    let c = FileShutdownCoordinator::open(r.store(), &r.plan, MAX_JOURNAL_BYTES, 0).unwrap();
    let anchor = c.history_anchor().unwrap().to_bytes().unwrap(); drop(c);
    private(&destination, &anchor); fs::set_permissions(&destination, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(r.run("resume", Some(&destination), None).0.is_err());
    let nested = r.root.join("domain/private.anchor"); private(&nested, &anchor);
    assert!(r.run("resume", Some(&nested), None).0.is_err());
}

#[test]
fn anchor_bytes_share_the_exact_aggregate_input_allowance_in_both_formats() {
    for archived in [false, true] {
        let r = Rig::new(archived); let anchor = r.root.join("required.anchor");
        r.run("create", None, Some(&anchor)).0.unwrap();
        let data = fs::read(r.root.join("member.json")).unwrap().len()
            + fs::read(r.root.join(if archived { "plan.bin" } else { "member.anchor" })).unwrap().len()
            + fs::read(&anchor).unwrap().len();
        let mut exact = data;
        for _ in 0..8 { exact = data + r.document(archived, exact).len(); }
        assert_eq!(exact, data + r.document(archived, exact).len());
        let control = r.image(); let native = r.domain();
        private(&r.manifest(), r.document(archived, exact - 1).as_bytes());
        assert!(r.run("resume", Some(&anchor), None).0.is_err());
        assert_eq!(r.image(), control); assert_eq!(r.domain(), native);
        private(&r.manifest(), r.document(archived, exact).as_bytes());
        r.run("resume", Some(&anchor), None).0.unwrap(); assert_eq!(r.image(), control);
    }
}

#[test]
fn failed_output_preserves_exported_custody_and_does_not_repeat_a_domain_stop() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
        fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
    }
    let r = Rig::new(false); let anchor = r.root.join("retained.anchor");
    let error = command_with(&r.args("create", None, Some(&anchor)), || Ok(ElapsedTick(2)),
        &mut || Duration::ZERO, &mut Broken).unwrap_err();
    assert!(error.contains("output failed after coordinator revision 2")); assert_eq!(read_anchor(&anchor).revision(), 2);
    let control = r.image(); let native = r.domain(); r.run("resume", Some(&anchor), None).0.unwrap();
    assert_eq!(r.image(), control); assert_eq!(r.domain(), native);
}

#[test]
fn late_export_collision_does_not_hide_acknowledged_domain_results_or_overwrite_files() {
    let r = Rig::new(false); let archive = r.root.join("late.anchor"); let mut out = Vec::new();
    let mut created = false;
    let result = command_with(&r.args("create", None, Some(&archive)), || Ok(ElapsedTick(2)), &mut || {
        // Inspecting native history is read-only and does not need the coordinator lock.
        let config = crate::config::Config::read(&r.root.join("member.json")).unwrap();
        if !created && let Ok(state) = FileOversight::read_publication(&config.store, &config.profile)
            && state.stop.is_some() { private(&archive, b"raced output"); created = true; }
        Duration::ZERO
    }, &mut out);
    assert!(created); assert!(result.unwrap_err().contains("anchor retention failed"));
    assert_eq!(fs::read(&archive).unwrap(), b"raced output");
    assert_eq!(parse(&out).get("all_observed_drained").unwrap().as_bool(), Some(true));
    let native = r.domain(); r.run("resume", None, None).0.unwrap(); assert_eq!(r.domain(), native);
}

#[test]
fn elapsed_export_failure_still_reports_the_completed_original_pass() {
    let r = Rig::new(false); let archive = r.root.join("too-late.anchor"); let mut out = Vec::new();
    let result = command_with(&r.args("create", None, Some(&archive)), || Ok(ElapsedTick(2)), &mut || {
        let config = crate::config::Config::read(&r.root.join("member.json")).unwrap();
        let stopped = FileOversight::read_publication(&config.store, &config.profile).unwrap().stop.is_some();
        if stopped { Duration::from_secs(10) } else { Duration::ZERO }
    }, &mut out);
    assert!(result.unwrap_err().contains("anchor retention failed")); assert!(!archive.exists());
    assert_eq!(parse(&out).get("all_observed_drained").unwrap().as_bool(), Some(true));
    assert_eq!(parse(&out).get("final_revision").unwrap().as_str(), Some("2"));
}
