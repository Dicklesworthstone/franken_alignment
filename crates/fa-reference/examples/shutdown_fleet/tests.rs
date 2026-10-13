//! Actual operator intake and original coordinator; no external model claims.
use super::*;
use fa_reference::action::consequence::delivery::persistent::{MAX_JOURNAL_BYTES, RecoveryReserve};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight,
    guarded::anchored::MAX_HISTORY_ANCHOR_BYTES,
    shutdown::{FileShutdownDomain, FileShutdownPlan, MAX_SHUTDOWN_HEAD_BYTES}};
use fa_reference::strict_json::{self, Json, Limits};
use std::{fs::{self, DirBuilder, OpenOptions}, io, os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::PathBuf, sync::atomic::{AtomicU64, Ordering}};
#[path = "../../tests/support/file_oversight.rs"] mod fixture;
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("fa-fleet-operator-{}-{}-{}", std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(), NEXT.fetch_add(1, Ordering::Relaxed)));
        DirBuilder::new().mode(0o700).create(&path).unwrap(); Self(fs::canonicalize(path).unwrap())
    }
    fn coordinator(&self) -> PathBuf { self.0.join("coordinator") }
    fn manifest(&self) -> PathBuf { self.0.join("fleet.json") }
}
impl Drop for Root {
    fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("fleet fixture cleanup: {error}"); } }
}
fn private(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new().create(true).truncate(true).write(true).mode(0o600).open(path).unwrap();
    file.write_all(bytes).unwrap();
}
fn member(root: &Root, id: u64) -> FileOversight {
    let store = root.0.join(format!("domain-{id}"));
    let text = include_str!("../../fixtures/supervised_publication.json")
        .replace("/operator/publication-store", store.to_str().unwrap())
        .replace("\"run\": 3", &format!("\"run\": {id}"));
    let config = crate::config::Config::decode(text.as_bytes()).unwrap();
    private(&root.0.join(format!("member-{id}.json")), text.as_bytes());
    let (mut host, _) = FileOversight::create(&store, config.profile).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    host.enable_publication_guard(host.revision()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let mut anchor = Vec::new(); host.history_anchor().unwrap().write_to(&mut anchor, MAX_HISTORY_ANCHOR_BYTES).unwrap();
    private(&root.0.join(format!("member-{id}.anchor")), &anchor);
    host
}
fn document(root: &Root, count: u64) -> String {
    let members = (1..=count).map(|id| format!(
        "{{\"id\":\"{id}\",\"config\":\"{}/member-{id}.json\",\"history_anchor\":\"{}/member-{id}.anchor\"}}",
        root.0.display(), root.0.display())).collect::<Vec<_>>().join(",");
    format!(concat!("{{\"schema_version\":\"fa.shutdown-fleet/1\",\"operation\":\"900\",",
        "\"coordinator\":\"{}\",\"minimum_revision\":\"0\",\"max_visits\":16,",
        "\"max_head_bytes\":4194304,\"coordinator_bytes\":16777216,\"input_bytes\":16777216,",
        "\"runtime_ms\":10000,\"replay_events\":1,\"members\":[{}]}}"), root.coordinator().display(), members)
}
fn manifest(root: &Root, count: u64) { private(&root.manifest(), document(root, count).as_bytes()); }
fn args(root: &Root, mode: &str) -> Vec<String> { vec![mode.into(), root.manifest().display().to_string()] }
fn run_ok(root: &Root, mode: &str) -> Json {
    let mut out = Vec::new();
    command_with(&args(root, mode), || Ok(ElapsedTick(2)), &mut || Duration::ZERO, &mut out).unwrap();
    parse(&out)
}
fn parse(out: &[u8]) -> Json { strict_json::parse(out, Limits::default()).unwrap() }
fn image(root: &Root, id: u64) -> Vec<u8> { fs::read(root.0.join(format!("domain-{id}/delivery.bin"))).unwrap() }
fn coordinated(root: &Root) -> Vec<u8> { fs::read(root.coordinator().join("delivery.bin")).unwrap() }

#[test]
fn create_and_resume_use_original_domain_results_without_live_helpers_or_new_domain_writes() {
    let root = Root::new(); drop(member(&root, 1)); drop(member(&root, 2)); manifest(&root, 2);
    let json = run_ok(&root, "create");
    assert_eq!(json.get("all_observed_drained").unwrap().as_bool(), Some(true));
    assert_eq!(json.get("final_revision").unwrap().as_str(), Some("4"));
    let sources = [image(&root, 1), image(&root, 2)]; let journal = coordinated(&root);
    let json = run_ok(&root, "resume");
    assert_eq!(json.get("final_revision").unwrap().as_str(), Some("4"));
    assert_eq!(json.get("all_observed_drained").unwrap().as_bool(), Some(true));
    assert_eq!([image(&root, 1), image(&root, 2)], sources); assert_eq!(coordinated(&root), journal);
    let error = command_with(&args(&root, "create"), || Ok(ElapsedTick(3)), &mut || Duration::ZERO, &mut Vec::new()).unwrap_err();
    assert!(error.contains("coordinator acquisition")); assert_eq!(coordinated(&root), journal);
}

#[test]
fn busy_and_missing_members_are_reported_while_a_reachable_member_stops() {
    let root = Root::new(); drop(member(&root, 1)); let busy = member(&root, 2); drop(member(&root, 3)); manifest(&root, 3);
    let hidden = root.0.join("offline"); fs::rename(root.0.join("domain-3"), &hidden).unwrap();
    let mut out = Vec::new();
    assert!(command_with(&args(&root, "create"), || Ok(ElapsedTick(2)), &mut || Duration::ZERO, &mut out).is_err());
    let json = parse(&out); let members = json.get("members").unwrap().as_array().unwrap();
    assert_eq!(members.len(), 3);
    assert_eq!(members[0].get("result").unwrap().as_str(), Some("observed"));
    assert_eq!(members[1].get("error").unwrap().as_str(), Some("busy"));
    assert_eq!(members[2].get("result").unwrap().as_str(), Some("refused"));
    assert_eq!(json.get("all_observed_drained").unwrap().as_bool(), Some(false));
    assert!(busy.inspect().stop.is_none()); drop(busy);
    fs::rename(&hidden, root.0.join("domain-3")).unwrap();
    assert_eq!(run_ok(&root, "recover").get("all_observed_drained").unwrap().as_bool(), Some(true));
}

#[test]
fn every_elapsed_boundary_before_finalization_preserves_the_domain() {
    let root = Root::new(); drop(member(&root, 1)); manifest(&root, 1);
    let mut count = 0;
    command_with(&args(&root, "create"), || Ok(ElapsedTick(2)), &mut || { count += 1; Duration::ZERO }, &mut Vec::new()).unwrap();
    for stop in 1..=count {
        let root = Root::new(); drop(member(&root, 1)); manifest(&root, 1);
        let before = image(&root, 1); let mut calls = 0; let mut out = Vec::new();
        let result = command_with(&args(&root, "create"), || Ok(ElapsedTick(2)), &mut || {
            calls += 1; if calls >= stop { Duration::from_secs(10) } else { Duration::ZERO }
        }, &mut out);
        assert!(result.is_err(), "cut {stop}/{count}");
        assert_eq!(image(&root, 1), before, "cut {stop}/{count}");
        if !out.is_empty() { assert_eq!(parse(&out).get("all_observed_drained").unwrap().as_bool(), Some(false)); }
    }
}

#[test]
fn closed_input_admission_and_custody_precede_any_coordinator_creation() {
    let root = Root::new(); drop(member(&root, 1)); manifest(&root, 1);
    let original = document(&root, 1); let before = image(&root, 1);
    for invalid in [vec![], vec!["resume".into()], vec!["force".into(), root.manifest().display().to_string()],
        vec!["resume".into(), root.manifest().display().to_string(), "--force".into()]] {
        assert!(command_with(&invalid, || panic!("closed options"), &mut || Duration::ZERO, &mut Vec::new()).is_err());
        assert!(!root.coordinator().exists());
    }
    for invalid in [original.replace("\"900\"", "\"0900\""), original.replace("\"replay_events\":1", "\"replay_events\":0"),
        original.replace("\"members\":[", "\"unknown\":true,\"members\":["),
        original.replace("\"input_bytes\":16777216", "\"input_bytes\":1"),
        original.replace("\"minimum_revision\":\"0\"", "\"minimum_revision\":\"1\"")] {
        private(&root.manifest(), invalid.as_bytes());
        assert!(command_with(&args(&root, "create"), || panic!("invalid profile before clock"), &mut || Duration::ZERO, &mut Vec::new()).is_err());
        assert!(!root.coordinator().exists()); assert_eq!(image(&root, 1), before);
    }
    manifest(&root, 1);
    let anchor = root.0.join("member-1.anchor"); let saved = fs::read(&anchor).unwrap();
    private(&anchor, b"malformed");
    assert!(command_with(&args(&root, "create"), || panic!("bad anchor before clock"), &mut || Duration::ZERO, &mut Vec::new()).is_err());
    private(&anchor, &saved); fs::set_permissions(&anchor, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(command_with(&args(&root, "create"), || panic!("public archive"), &mut || Duration::ZERO, &mut Vec::new()).is_err());
    fs::set_permissions(&anchor, fs::Permissions::from_mode(0o600)).unwrap();
    let nested = root.0.join("domain-1/custody.anchor"); private(&nested, &saved);
    private(&root.manifest(), original.replace(anchor.to_str().unwrap(), nested.to_str().unwrap()).as_bytes());
    assert!(command_with(&args(&root, "create"), || panic!("joint custody"), &mut || Duration::ZERO, &mut Vec::new()).is_err());
    assert!(!root.coordinator().exists()); assert_eq!(image(&root, 1), before);
}

#[test]
fn changed_roster_or_revision_floor_refuses_before_cleanup_or_domain_work() {
    let root = Root::new(); drop(member(&root, 1)); manifest(&root, 1); run_ok(&root, "create");
    let before = coordinated(&root); let domain = image(&root, 1);
    let pending = root.coordinator().join("delivery.pending"); private(&pending, b"untrusted staged bytes");
    for invalid in [document(&root, 1).replace("\"max_visits\":16", "\"max_visits\":17"),
        document(&root, 1).replace("\"minimum_revision\":\"0\"", "\"minimum_revision\":\"3\"")] {
        private(&root.manifest(), invalid.as_bytes()); let mut out = Vec::new();
        assert!(command_with(&args(&root, "resume"), || Ok(ElapsedTick(3)), &mut || Duration::ZERO, &mut out).is_err());
        assert!(out.is_empty()); assert!(pending.exists()); assert_eq!(coordinated(&root), before); assert_eq!(image(&root, 1), domain);
    }
    manifest(&root, 1); run_ok(&root, "resume"); assert!(!pending.exists());
    assert_eq!(coordinated(&root), before); assert_eq!(image(&root, 1), domain);
}

#[test]
fn lost_output_never_repeats_a_completed_domain_stop_on_resume() {
    struct Broken(bool);
    impl Write for Broken {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> { if self.0 { Ok(bytes.len()) } else { Err(io::ErrorKind::BrokenPipe.into()) } }
        fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
    }
    for flush in [false, true] {
        let root = Root::new(); drop(member(&root, 1)); manifest(&root, 1);
        let error = command_with(&args(&root, "create"), || Ok(ElapsedTick(2)), &mut || Duration::ZERO, &mut Broken(flush)).unwrap_err();
        assert!(error.contains("after coordinator revision 2"));
        let domain = image(&root, 1); let control = coordinated(&root);
        run_ok(&root, "resume"); assert_eq!(image(&root, 1), domain); assert_eq!(coordinated(&root), control);
    }
}

#[test]
fn receipt_rendering_preserves_actual_two_key_executed_and_unknown_charges() {
    for mode in 0..4 {
        let root = fixture::Directory::new(); let control = fixture::Directory::new();
        let (mut host, reviewer) = fixture::create(&root);
        let keys = fixture::ready(&mut host, &reviewer, 1, b"actual effect");
        if mode > 0 { fixture::dispatch(&mut host, &keys); }
        if mode == 2 { host.publish(host.revision(), 1).unwrap(); }
        let anchor = host.history_anchor().unwrap();
        let member = host.shutdown_domain(1).unwrap().member();
        let domain = FileShutdownDomain::from_history_anchor(member, &anchor).unwrap();
        let p = FileShutdownPlan::new(900, vec![domain], 1, MAX_SHUTDOWN_HEAD_BYTES).unwrap();
        drop(keys); drop(reviewer); drop(host);
        let mut coordinator = FileShutdownCoordinator::create(control.store(), &p, MAX_JOURNAL_BYTES).unwrap();
        let pass = coordinator.recover_registered_pass_with_checkpoints(0, &[FileShutdownClock {
            domain: 1, clock_domain: 99, at: ElapsedTick(if mode == 3 { 2000 } else { 2 }),
        }], 1, |_| true).unwrap();
        let json = parse(render::encode("recover", false, &pass).as_bytes());
        let record = &json.get("members").unwrap().as_array().unwrap()[0];
        assert_eq!(record.get("charged").unwrap().as_str(), Some(if mode >= 2 { "16" } else { "0" }));
        assert_eq!(record.get("executions").unwrap().as_str(), Some(if mode == 2 { "1" } else { "0" }));
        assert_eq!(json.get("all_observed_drained").unwrap().as_bool(), Some(mode != 3));
    }
}

#[test]
fn a_callback_unwind_retains_entered_intent_and_resume_does_not_execute_it() {
    let root = Root::new(); drop(member(&root, 1)); manifest(&root, 1);
    let original = image(&root, 1);
    let mut initial_coordinator = None;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        command_with(&args(&root, "create"), || Ok(ElapsedTick(2)), &mut || {
            if let Ok(bytes) = fs::read(root.coordinator().join("delivery.bin")) {
                match &initial_coordinator {
                    None => initial_coordinator = Some(bytes),
                    // Only the ORIGINAL intent write can change the initial
                    // coordinator between these first two member checkpoints.
                    Some(initial) if *initial != bytes => panic!("after acknowledged visit intent"),
                    _ => {}
                }
            }
            Duration::ZERO
        }, &mut Vec::new())
    }));
    assert!(result.is_err()); assert!(initial_coordinator.is_some());
    assert_eq!(image(&root, 1), original);
    let mut out = Vec::new();
    assert!(command_with(&args(&root, "resume"), || Ok(ElapsedTick(3)), &mut || Duration::ZERO, &mut out).is_err());
    assert_eq!(parse(&out).get("members").unwrap().as_array().unwrap()[0].get("result").unwrap().as_str(), Some("evidence_refused"));
    assert_eq!(image(&root, 1), original);
}
