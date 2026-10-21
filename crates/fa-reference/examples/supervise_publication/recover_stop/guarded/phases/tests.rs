//! Original native journals/topology and actual command routing; no model-quality claim.
use super::*;
use super::super::{FileOversight, FileGuardSet, RecoveryReserve};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileRecoveryFloor, anchored::{FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES}};
use fa_reference::action::consequence::mediation::{CutCheck, MAX_CHECK_EDGE_VISITS};
use fa_reference::perimeter_inventory::topology::LoadedAuthorityTopology;
use fa_reference::strict_json::{self, Json, Limits};
use std::{cell::Cell, collections::BTreeMap, fs, io::{self, Cursor}, path::PathBuf,
    panic::{AssertUnwindSafe, catch_unwind}, sync::atomic::{AtomicU64, Ordering}};

const CONFIG: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/supervised_publication.json"));
const INVENTORY: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/authority_topology_inventory.json"));
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-guarded-phase-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&root).unwrap(); Self(root)
    }
    fn path(&self, name: &str) -> PathBuf { self.0.join(name) }
    fn bytes(&self) -> Vec<u8> { fs::read(self.path("store/delivery.bin")).unwrap() }
    fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fs::read_dir(self.path("store")).unwrap().map(|entry| {
            let path = entry.unwrap().path(); (path.clone(), fs::read(path).unwrap())
        }).collect()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("guarded phase cleanup: {error}"); }
    }
}
fn topology(generation: u64) -> String {
    format!(r#"{{"schema":"fa.authority-topology/1","generation":{generation},"inventory_generation":{generation},
        "complete_inventory_generation":{generation},
        "scope":{{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5,"purpose":"effect"}},
        "target":{{"adapter":10,"object":11,"contract_version":1,"expected_version":1,"generation":1}},
        "inventory_scope":{{"tenant":1,"principal":2,"purpose":1}},"family":"file-publication",
        "nodes":[{{"id":1,"kind":"actor"}},{{"id":2,"kind":"enforcer"}},{{"id":4,"kind":"sink"}}],
        "edges":[{{"id":1,"from":1,"to":2,"channel":"dispatch","route":"publication","provenance":101}},
            {{"id":2,"from":2,"to":4,"channel":"dispatch","route":"publication","provenance":102}}],
        "enforcers":[{{"node":2,"adapter":10,"contract_version":1,"generation":1,"provenance":102}}],
        "gates":[2],"check_edge_budget":{MAX_CHECK_EDGE_VISITS},"source_limits":{{"checks":32,"read_bytes":10000000}}}}"#)
}
fn setup(root: &Directory, mediated: bool, full: bool) -> (Config, FileRecoveryFloor) {
    let text = CONFIG.replace("/operator/publication-store", root.path("store").to_str().unwrap())
        .replace("/operator/evidence.json", root.path("absent-live-evidence.json").to_str().unwrap())
        .replace("\"journal_events\": 4096", "\"journal_events\": 40");
    fs::write(root.path("config.json"), &text).unwrap();
    let mut c = Config::decode(text.as_bytes()).unwrap(); c.programs.clear();
    assert_eq!(c.profile.delivery.limits.events, 40);
    let guards = FileGuardSet { stream: None, decoder: None, decoder_stop: None,
        source: None, identity: None, campaigns: None, credential: None };
    let (mut host, _) = FileOversight::create_guarded(&c.store, c.profile.clone(), &guards, None).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    host.enable_file_source(host.revision(), c.source_policy).unwrap();
    fs::write(root.path("initial.json"), topology(1)).unwrap();
    fs::write(root.path("current.json"), topology(1)).unwrap();
    fs::write(root.path("inventory.json"), INVENTORY).unwrap();
    if mediated {
        let loaded = LoadedAuthorityTopology::from_json_bytes(topology(1).as_bytes(), INVENTORY).unwrap();
        let observer = host.enable_mediation(host.revision(), loaded.graph().clone()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let proposal = host.mediation_snapshot().unwrap().graph.propose_cut(&[2]).unwrap();
        let revision = host.revision(); let epoch = host.inspect().control.ledger.epoch;
        assert!(matches!(observer.certify(&mut host, revision, epoch, &proposal, MAX_CHECK_EDGE_VISITS)
            .unwrap().unwrap(), CutCheck::Verified(_)));
    } else { host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); }
    if full {
        while host.journal_capacity().unwrap().ordinary_remaining().events != 0 {
            host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        }
    }
    let state = host.inspect();
    let floor = FileRecoveryFloor { journal_revision: state.revision,
        control_sequence: state.control.sequence, authority_epoch: state.control.ledger.epoch };
    let mut bytes = Vec::new(); host.history_anchor().unwrap().write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES).unwrap();
    fs::write(root.path("before.anchor"), bytes).unwrap();
    (c, floor)
}
fn document(root: &Directory, floor: FileRecoveryFloor, mediated: bool, available: bool,
    input: &str, output: Option<&str>) -> String
{
    let topology = if mediated { format!(r#", "topology":{{"initial_topology":"{}","initial_inventory":"{}",
        "current_topology":"{}","current_inventory":"{}","available":{available}}}"#,
        root.path("initial.json").display(), root.path("inventory.json").display(),
        root.path("current.json").display(), root.path("inventory.json").display()) } else { String::new() };
    let output = output.map(|name| format!(r#", "retain_history_anchor":"{}""#, root.path(name).display())).unwrap_or_default();
    let version = if mediated { 2 } else { 1 };
    format!(r#"{{"schema_version":"fa.guarded-stop/{version}","minimum":{{"journal_revision":{},
        "control_sequence":{},"authority_epoch":{}}},"history_anchor":"{}"{output}{topology}}}"#,
        floor.journal_revision, floor.control_sequence, floor.authority_epoch, root.path(input).display())
}
fn recipe(root: &Directory, floor: FileRecoveryFloor, mediated: bool, available: bool,
    input: &str, output: Option<&str>) -> Profile
{ Profile::decode(document(root, floor, mediated, available, input, output).as_bytes()).unwrap() }
fn json(bytes: &[u8]) -> Json { strict_json::parse(bytes, Limits::default()).unwrap() }
fn anchor(root: &Directory, name: &str) -> FileHistoryAnchor {
    FileHistoryAnchor::read_trusted(&mut Cursor::new(fs::read(root.path(name)).unwrap()), MAX_HISTORY_ANCHOR_BYTES).unwrap()
}
fn stop(root: &Directory, c: &Config, floor: FileRecoveryFloor, mediated: bool) {
    let p = recipe(root, floor, mediated, true, "before.anchor", None);
    run_with_elapsed(c, &p, Phase::Admission, 900, || panic!("clock-free admission"),
        &mut || Duration::ZERO, &mut Vec::new()).unwrap();
}
fn args(root: &Directory, phase: Phase) -> Vec<String> {
    vec![match phase { Phase::Admission => "stop-admission", Phase::Drain => "drain-stopped" }.into(),
        root.path("config.json").display().to_string(), "900".into(), "--guarded".into(),
        root.path("phase.json").display().to_string()]
}

#[test]
fn both_actual_command_routes_preserve_guards_anchors_and_the_three_event_reserve() {
    for mediated in [false, true] {
        let root = Directory::new(); let (c, floor) = setup(&root, mediated, true);
        let before = fs::read(root.path("before.anchor")).unwrap();
        fs::write(root.path("phase.json"), document(&root, floor, mediated, true, "before.anchor", Some("stopped.anchor"))).unwrap();
        let mut out = Vec::new();
        command_with(&args(&root, Phase::Admission), || panic!("no clock"), &mut || Duration::ZERO, &mut out).unwrap();
        let report = json(&out);
        assert_eq!(report.get("drain_attempted").unwrap().as_bool(), Some(false));
        assert_eq!(report.get("appended_events").unwrap().as_u64(), Some(2));
        let stopped = FileOversight::read_publication(&c.store, &c.profile).unwrap();
        assert_eq!(stopped.revision, floor.journal_revision + 2);
        assert_eq!(stopped.control.ledger.elapsed, Some(ElapsedTick(1)));
        assert_eq!(anchor(&root, "stopped.anchor").revision(), stopped.revision);
        let saved = root.bytes();
        fs::write(root.path("phase.json"), document(&root, floor, mediated, false, "stopped.anchor", None)).unwrap();
        crate::command(args(&root, Phase::Admission)).unwrap();
        assert_eq!(root.bytes(), saved);
        fs::write(root.path("phase.json"), document(&root, floor, mediated, false, "stopped.anchor", Some("drained.anchor"))).unwrap();
        out.clear();
        command_with(&args(&root, Phase::Drain), || Ok(ElapsedTick(2)), &mut || Duration::ZERO, &mut out).unwrap();
        assert_eq!(json(&out).get("drained").unwrap().as_bool(), Some(true));
        let drained = FileOversight::read_publication(&c.store, &c.profile).unwrap();
        assert_eq!(drained.revision, floor.journal_revision + 3);
        assert_eq!(drained.revision as usize, c.profile.delivery.limits.events);
        assert_eq!(anchor(&root, "drained.anchor").revision(), drained.revision);
        let saved = root.bytes();
        fs::write(root.path("phase.json"), document(&root, floor, mediated, false, "drained.anchor", None)).unwrap();
        command_with(&args(&root, Phase::Drain), || panic!("completed drain clock"), &mut || Duration::ZERO, &mut Vec::new()).unwrap();
        crate::command(args(&root, Phase::Drain)).unwrap();
        assert_eq!(root.bytes(), saved); assert_eq!(fs::read(root.path("before.anchor")).unwrap(), before);
        assert!(!root.path("absent-live-evidence.json").exists());
    }
}

#[test]
fn each_independent_requirement_remains_mandatory_for_both_phases() {
    for phase in [Phase::Admission, Phase::Drain] {
        let root = Directory::new(); let (mut c, floor) = setup(&root, true, false);
        if phase == Phase::Drain { stop(&root, &c, floor, true); }
        fs::write(root.path("store/delivery.pending"), b"not a native result").unwrap();
        let before = root.files();
        for mode in 0..6 {
            let mut required = floor;
            if mode == 0 { fs::write(root.path("initial.json"), topology(2)).unwrap(); }
            if mode == 1 { fs::write(root.path("current.json"), topology(2)).unwrap(); }
            if mode == 3 { required.journal_revision = u64::MAX; }
            if mode == 4 { c.source_policy.source.generation += 1; }
            let available = (phase == Phase::Admission) != (mode == 2);
            let p = recipe(&root, required, mode != 5, available, "before.anchor", None);
            assert!(run_with_elapsed(&c, &p, phase, 900, || panic!("no clock after failed binding"),
                &mut || Duration::ZERO, &mut Vec::new()).is_err(), "{phase:?} {mode}");
            assert_eq!(root.files(), before);
            fs::write(root.path("initial.json"), topology(1)).unwrap();
            fs::write(root.path("current.json"), topology(1)).unwrap();
            if mode == 4 { c.source_policy.source.generation -= 1; }
        }
        let p = recipe(&root, floor, true, phase == Phase::Admission, "before.anchor", None);
        run_with_elapsed(&c, &p, phase, 900, || Ok(ElapsedTick(2)), &mut || Duration::ZERO, &mut Vec::new()).unwrap();
    }
}

#[test]
fn drain_requires_the_existing_operation_and_independently_retained_stopped_prefix() {
    for mediated in [false, true] {
        let root = Directory::new(); let (c, floor) = setup(&root, mediated, false);
        let original = root.bytes();
        let active = recipe(&root, floor, mediated, true, "before.anchor", Some("stopped.anchor"));
        assert!(run_with_elapsed(&c, &active, Phase::Drain, 900, || panic!("active drain"),
            &mut || Duration::ZERO, &mut Vec::new()).is_err());
        assert_eq!(root.bytes(), original); assert!(!root.path("stopped.anchor").exists());
        run_with_elapsed(&c, &active, Phase::Admission, 900, || panic!("no clock"), &mut || Duration::ZERO, &mut Vec::new()).unwrap();
        let stopped = root.bytes();
        let p = recipe(&root, floor, mediated, false, "stopped.anchor", None);
        fs::write(root.path("store/delivery.bin"), &original).unwrap();
        fs::write(root.path("store/delivery.pending"), b"preserve on rollback").unwrap();
        let rollback = root.files();
        assert!(run_with_elapsed(&c, &p, Phase::Drain, 900, || panic!("rollback"), &mut || Duration::ZERO, &mut Vec::new()).is_err());
        assert_eq!(root.files(), rollback);
        fs::write(root.path("store/delivery.bin"), &stopped).unwrap();
        let before = root.files();
        assert!(run_with_elapsed(&c, &p, Phase::Drain, 901, || panic!("foreign operation"), &mut || Duration::ZERO, &mut Vec::new()).is_err());
        assert_eq!(root.files(), before);
        run_with_elapsed(&c, &p, Phase::Drain, 900, || Ok(ElapsedTick(2)), &mut || Duration::ZERO, &mut Vec::new()).unwrap();
    }
}

#[test]
fn one_elapsed_budget_cancels_every_intake_replay_ready_and_acquisition_cut() {
    for mediated in [false, true] { for phase in [Phase::Admission, Phase::Drain] {
        let root = Directory::new(); let (c, floor) = setup(&root, mediated, false);
        if phase == Phase::Drain { stop(&root, &c, floor, mediated); }
        let p = recipe(&root, floor, mediated, phase == Phase::Admission, "before.anchor", None);
        let count = Cell::new(0);
        run_with_elapsed(&c, &p, phase, 900, || Ok(ElapsedTick(2)), &mut || {
            count.set(count.get() + 1); Duration::ZERO
        }, &mut Vec::new()).unwrap();
        for cut in 1..=count.get() {
            let root = Directory::new(); let (c, floor) = setup(&root, mediated, false);
            if phase == Phase::Drain { stop(&root, &c, floor, mediated); }
            let p = recipe(&root, floor, mediated, phase == Phase::Admission, "before.anchor", Some("after.anchor"));
            fs::write(root.path("store/delivery.pending"), b"never promote").unwrap();
            let before = root.files(); let mut calls = 0; let mut out = Vec::new();
            let error = run_with_elapsed(&c, &p, phase, 900, || Ok(ElapsedTick(2)), &mut || {
                calls += 1;
                if calls >= cut { Duration::from_millis(c.timing.runtime_ms) } else { Duration::ZERO }
            }, &mut out).unwrap_err();
            assert!(error.contains("deadline"), "{mediated} {phase:?} {cut}: {error}");
            assert!(out.is_empty()); assert_eq!(root.files(), before); assert!(!root.path("after.anchor").exists());
            // Cancellation released the original lock without a role/fence.
            run_with_elapsed(&c, &p, phase, 900, || Ok(ElapsedTick(2)), &mut || Duration::ZERO, &mut Vec::new()).unwrap();
        }
    }}
}

#[test]
fn failed_panicking_and_slow_clocks_cannot_undo_the_prior_guarded_admission_stop() {
    for mediated in [false, true] { for mode in 0..3 {
        let root = Directory::new(); let (c, floor) = setup(&root, mediated, false);
        stop(&root, &c, floor, mediated);
        let p = recipe(&root, floor, mediated, false, "before.anchor", Some("after.anchor"));
        fs::write(root.path("store/delivery.pending"), b"unchanged on clock failure").unwrap();
        let before = root.files(); let expired = Cell::new(false); let mut out = Vec::new();
        let result = catch_unwind(AssertUnwindSafe(|| run_with_elapsed(&c, &p, Phase::Drain, 900, || {
            match mode { 0 => Err(JournalError::Unavailable), 1 => panic!("clock interrupted"),
                _ => { expired.set(true); Ok(ElapsedTick(2)) } }
        }, &mut || if expired.get() { Duration::from_millis(c.timing.runtime_ms) } else { Duration::ZERO }, &mut out)));
        if mode == 1 { assert!(result.is_err()); } else { assert!(result.unwrap().is_err()); }
        assert!(out.is_empty()); assert_eq!(root.files(), before); assert!(!root.path("after.anchor").exists());
        run_with_elapsed(&c, &p, Phase::Drain, 900, || Ok(ElapsedTick(2)), &mut || Duration::ZERO, &mut Vec::new()).unwrap();
    }}
}

#[test]
fn late_export_collision_and_lost_stdout_keep_the_original_native_acknowledgment() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
        fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
    }
    for mediated in [false, true] { for phase in [Phase::Admission, Phase::Drain] {
        let sizing = Directory::new(); let (c, floor) = setup(&sizing, mediated, false);
        if phase == Phase::Drain { stop(&sizing, &c, floor, mediated); }
        let p = recipe(&sizing, floor, mediated, phase == Phase::Admission, "before.anchor", None);
        let mut calls = 0;
        run_with_elapsed(&c, &p, phase, 900, || Ok(ElapsedTick(2)), &mut || { calls += 1; Duration::ZERO }, &mut Vec::new()).unwrap();
        for collision in [false, true] {
            let root = Directory::new(); let (c, floor) = setup(&root, mediated, false);
            if phase == Phase::Drain { stop(&root, &c, floor, mediated); }
            let p = recipe(&root, floor, mediated, phase == Phase::Admission, "before.anchor", Some("after.anchor"));
            let old = fs::read(root.path("before.anchor")).unwrap(); let mut count = 0;
            let error = run_with_elapsed(&c, &p, phase, 900, || Ok(ElapsedTick(2)), &mut || {
                count += 1;
                if collision && count == calls { fs::write(root.path("after.anchor"), b"other custody").unwrap(); }
                Duration::ZERO
            }, &mut Broken).unwrap_err();
            assert!(error.contains("output failed")); assert!(error.contains("acknowledged"));
            let state = FileOversight::read_publication(&c.store, &c.profile).unwrap();
            assert!(state.stop.is_some());
            if collision {
                assert!(error.contains("retention failed")); assert_eq!(fs::read(root.path("after.anchor")).unwrap(), b"other custody");
            } else { assert_eq!(anchor(&root, "after.anchor").revision(), state.revision); }
            assert_eq!(fs::read(root.path("before.anchor")).unwrap(), old);
            let native = root.bytes(); let retry = recipe(&root, floor, mediated, false, "before.anchor", None);
            run_with_elapsed(&c, &retry, phase, 900, || panic!("exact retry needs no time"), &mut || Duration::ZERO, &mut Vec::new()).unwrap();
            assert_eq!(root.bytes(), native);
        }
    }}
}

#[test]
fn guarded_options_are_closed_before_config_and_never_fall_back_to_ordinary_recovery() {
    let base = vec!["stop-admission".to_owned(), "/missing-config".into(), "900".into(), "--guarded".into(), "/missing-profile".into()];
    for mutation in 0..7 {
        let mut a = base.clone();
        match mutation {
            0 => { a.pop(); },
            1 => a.extend(["--guarded".into(), "/second".into()]),
            2 => a.extend(["--retain-history-anchor".into(), "/output".into()]),
            3 => a[4] = "--credibility-activation".into(),
            4 => a[3] = "--reviewer-profile".into(),
            5 => a[2] = "0".into(),
            _ => a[4].clear(),
        }
        let error = command_with(&a, || panic!("invalid options clock"), &mut || panic!("invalid options elapsed"), &mut Vec::new()).unwrap_err();
        assert!(error.contains("usage") || error.contains("nonzero"));
        assert!(crate::command(a).is_err());
    }
    for mediated in [false, true] {
        let root = Directory::new(); let (c, floor) = setup(&root, mediated, false);
        fs::write(root.path("phase.json"), b"{}").unwrap(); let before = root.files();
        assert!(crate::command(args(&root, Phase::Admission)).is_err()); assert_eq!(root.files(), before);
        fs::write(root.path("phase.json"), document(&root, floor, !mediated, true, "before.anchor", None)).unwrap();
        assert!(crate::command(args(&root, Phase::Admission)).is_err()); assert_eq!(root.files(), before);
        fs::write(root.path("phase.json"), document(&root, floor, mediated, true, "before.anchor", None)).unwrap();
        crate::command(args(&root, Phase::Admission)).unwrap();
        assert!(FileOversight::read_publication(&c.store, &c.profile).unwrap().stop.is_some());
    }
}

#[test]
fn missing_or_malformed_independent_anchor_and_existing_output_refuse_before_mutation() {
    for mediated in [false, true] {
        let root = Directory::new(); let (c, floor) = setup(&root, mediated, false);
        let original = fs::read(root.path("before.anchor")).unwrap();
        let p = recipe(&root, floor, mediated, true, "before.anchor", Some("after.anchor"));
        fs::write(root.path("store/delivery.pending"), b"retain pending").unwrap();
        let before = root.files();
        for mode in 0..3 {
            match mode {
                0 => fs::remove_file(root.path("before.anchor")).unwrap(),
                1 => fs::write(root.path("before.anchor"), b"invalid custody").unwrap(),
                _ => fs::write(root.path("after.anchor"), b"old archive").unwrap(),
            }
            let mut out = Vec::new();
            assert!(run_with_elapsed(&c, &p, Phase::Admission, 900, || panic!("no logical clock"), &mut || Duration::ZERO, &mut out).is_err());
            assert!(out.is_empty()); assert_eq!(root.files(), before);
            fs::write(root.path("before.anchor"), &original).unwrap();
        }
        assert_eq!(fs::read(root.path("after.anchor")).unwrap(), b"old archive");
        let corrected = recipe(&root, floor, mediated, true, "before.anchor", Some("corrected.anchor"));
        run_with_elapsed(&c, &corrected, Phase::Admission, 900, || panic!("clock-free positive"), &mut || Duration::ZERO, &mut Vec::new()).unwrap();
        assert!(root.path("corrected.anchor").exists());
    }
}

mod combined;
