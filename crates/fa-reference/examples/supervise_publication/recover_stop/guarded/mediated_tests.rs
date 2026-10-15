//! Original command, canonical storage, topology parser and terminal reducer.
//! Declared topology and fixed test inputs are not containment qualification.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileRecoveryFloor, anchored::{FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES}};
use fa_reference::action::consequence::delivery::persistent::observed::mediation::FileMediationUpdate;
use fa_reference::action::consequence::mediation::{CutCheck, MAX_CHECK_EDGE_VISITS};
use fa_reference::action::consequence::oversight::evidence_source::FileEvidenceSource;
use fa_reference::perimeter_inventory::topology::LoadedAuthorityTopology;
use fa_reference::strict_json::{self, Json, Limits};
use std::{cell::Cell, collections::BTreeMap, fs, io::{self, Cursor}, path::PathBuf,
    sync::atomic::{AtomicU64, Ordering}};

const CONFIG: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/supervised_publication.json"));
const INVENTORY: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/authority_topology_inventory.json"));

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-mediated-stop-command-{}-{stamp}-{}",
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
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("mediated operator cleanup: {error}"); }
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

fn configured(root: &Directory, events: usize) -> Config {
    let text = CONFIG.replace("/operator/publication-store", root.path("store").to_str().unwrap())
        .replace("/operator/evidence.json", root.path("absent-live-evidence.json").to_str().unwrap())
        .replace("\"journal_events\": 4096", &format!("\"journal_events\": {events}"));
    fs::write(root.path("config.json"), &text).unwrap();
    let mut c = Config::decode(text.as_bytes()).unwrap();
    c.source = FileEvidenceSource::new(root.path("absent-live-evidence.json"), 51,
        c.profile.delivery.scope, 1048576).unwrap();
    c.programs.clear(); // Neither native workers nor external reviewers are needed.
    c
}

fn setup(root: &Directory, mediated: bool, changed_graph: bool, fill: bool) -> (Config, FileRecoveryFloor) {
    let c = configured(root, if fill { 40 } else { 4096 });
    let guards = FileGuardSet { stream: None, decoder: None, decoder_stop: None,
        source: None, identity: None, campaigns: None, credential: None };
    let (mut owner, _) = FileOversight::create_guarded(&c.store, c.profile.clone(), &guards, None).unwrap();
    owner.enable_recovery_reserve(owner.revision(), RecoveryReserve::terminal()).unwrap();
    // Source installation is ordinary journal work, so the reserve must already
    // exist. The final independently checked guard inventory still requires it.
    owner.enable_file_source(owner.revision(), c.source_policy).unwrap();
    fs::write(root.path("initial.json"), topology(1)).unwrap();
    fs::write(root.path("current.json"), topology(if changed_graph { 2 } else { 1 })).unwrap();
    fs::write(root.path("inventory.json"), INVENTORY).unwrap();
    if mediated {
        let initial = LoadedAuthorityTopology::from_json_bytes(topology(1).as_bytes(), INVENTORY).unwrap();
        let observer = owner.enable_mediation(owner.revision(), initial.graph().clone()).unwrap();
        owner.observe_time(owner.revision(), ElapsedTick(1)).unwrap();
        if changed_graph {
            let current = LoadedAuthorityTopology::from_json_bytes(topology(2).as_bytes(), INVENTORY).unwrap();
            let update = FileMediationUpdate { operation: 71, expected_generation: 1,
                expected_authority_epoch: owner.inspect().control.ledger.epoch, next: Some(current.graph().clone()) };
            let revision = owner.revision(); observer.update(&mut owner, revision, &update).unwrap();
        }
        let current = owner.mediation_snapshot().unwrap();
        let proposal = current.graph.propose_cut(&[2]).unwrap();
        let revision = owner.revision(); let epoch = owner.inspect().control.ledger.epoch;
        assert!(matches!(observer.certify(&mut owner, revision, epoch, &proposal, MAX_CHECK_EDGE_VISITS)
            .unwrap().unwrap(), CutCheck::Verified(_)));
    } else { owner.observe_time(owner.revision(), ElapsedTick(1)).unwrap(); }
    if fill {
        while owner.journal_capacity().unwrap().ordinary_remaining().events != 0 {
            owner.observe_time(owner.revision(), ElapsedTick(1)).unwrap();
        }
    }
    let state = owner.inspect();
    let floor = FileRecoveryFloor { journal_revision: state.revision,
        control_sequence: state.control.sequence, authority_epoch: state.control.ledger.epoch };
    let anchor = owner.history_anchor().unwrap();
    let mut archive = Vec::new(); anchor.write_to(&mut archive, MAX_HISTORY_ANCHOR_BYTES).unwrap();
    fs::write(root.path("before.anchor"), archive).unwrap();
    (c, floor)
}

fn document(root: &Directory, floor: FileRecoveryFloor, mediated: bool, available: bool,
    output: Option<&str>) -> String
{
    let topology = if mediated { format!(r#", "topology":{{
        "initial_topology":"{}","initial_inventory":"{}",
        "current_topology":"{}","current_inventory":"{}","available":{available}}}"#,
        root.path("initial.json").display(), root.path("inventory.json").display(),
        root.path("current.json").display(), root.path("inventory.json").display()) } else { String::new() };
    let output = output.map(|name| format!(r#", "retain_history_anchor":"{}""#, root.path(name).display()))
        .unwrap_or_default();
    let version = if mediated { 2 } else { 1 };
    format!(r#"{{"schema_version":"fa.guarded-stop/{version}","minimum":{{"journal_revision":{},
        "control_sequence":{},"authority_epoch":{}}},"history_anchor":"{}"{output}{topology}}}"#,
        floor.journal_revision, floor.control_sequence, floor.authority_epoch, root.path("before.anchor").display())
}
fn recipe(root: &Directory, floor: FileRecoveryFloor, mediated: bool, available: bool,
    output: Option<&str>) -> Profile
{ Profile::decode(document(root, floor, mediated, available, output).as_bytes()).unwrap() }
fn json(bytes: &[u8]) -> Json { strict_json::parse(bytes, Limits::default()).unwrap() }

#[test]
fn real_recover_stop_command_accepts_explicit_mediation_and_retains_its_acknowledged_cut() {
    let root = Directory::new(); let (c, floor) = setup(&root, true, true, false);
    fs::write(root.path("stop.json"), document(&root, floor, true, true, Some("after.anchor"))).unwrap();
    let before_archive = fs::read(root.path("before.anchor")).unwrap();
    assert!(!root.path("absent-live-evidence.json").exists());
    crate::command(vec!["recover-stop".into(), root.path("config.json").display().to_string(),
        "900".into(), "--guarded".into(), root.path("stop.json").display().to_string()]).unwrap();
    let state = FileOversight::read_publication(&c.store, &c.profile).unwrap();
    assert_eq!(state.revision, floor.journal_revision + 3);
    assert!(state.stop.is_some()); assert_eq!(state.executions, 0);
    let archived = fs::read(root.path("after.anchor")).unwrap();
    let anchor = FileHistoryAnchor::read_trusted(&mut Cursor::new(archived), MAX_HISTORY_ANCHOR_BYTES).unwrap();
    assert_eq!(anchor.revision(), state.revision);
    assert_eq!(fs::read(root.path("before.anchor")).unwrap(), before_archive);
    let canonical = root.bytes(); let mut out = Vec::new();
    run(&c, &recipe(&root, floor, true, false, None), 900, || panic!("drained retry clock"), &mut out).unwrap();
    assert_eq!(json(&out).get("recovery").unwrap().as_str(), Some("already_drained"));
    assert_eq!(root.bytes(), canonical);
}

#[test]
fn stop_schema_is_not_a_receipt_submit_or_investigation_profile_and_v1_does_not_ignore_topology() {
    let root = Directory::new(); let (c, floor) = setup(&root, true, false, false);
    let text = document(&root, floor, true, true, None);
    assert!(ResumeProfile::decode(text.as_bytes()).is_err());
    assert!(SubmitProfile::decode(text.as_bytes()).is_err());
    let profile = Profile::decode(text.as_bytes()).unwrap();
    assert!(profile.admit_investigation(&c).unwrap_err().contains("not read-only"));
    for invalid in [text.replace("fa.guarded-stop/2", "fa.guarded-stop/1"),
        text.replace("fa.guarded-stop/2", "fa.guarded-stop/3"),
        text.replace("\"available\":true", "\"available\":null"),
        text.replace("\"available\":true", "\"available\":true,\"accept_stored\":true"),
        document(&root, floor, false, true, None).replace("fa.guarded-stop/1", "fa.guarded-stop/2")] {
        assert!(Profile::decode(invalid.as_bytes()).is_err());
    }
    let before = root.files();
    assert!(run(&c, &recipe(&root, floor, false, true, None), 900,
        || panic!("base profile must not admit mediation"), &mut Vec::new()).is_err());
    assert_eq!(root.files(), before);
    run(&c, &profile, 900, || ElapsedTick(2), &mut Vec::new()).unwrap();
}

#[test]
fn optional_mediation_cannot_be_inferred_from_a_base_journal_and_legacy_stop_still_works() {
    let root = Directory::new(); let (c, floor) = setup(&root, false, false, false);
    let before = root.files();
    assert!(run(&c, &recipe(&root, floor, true, true, None), 900,
        || panic!("missing mediated gate"), &mut Vec::new()).is_err());
    assert_eq!(root.files(), before);
    run(&c, &recipe(&root, floor, false, true, None), 900, || ElapsedTick(2), &mut Vec::new()).unwrap();
    assert_eq!(FileOversight::read_publication(&c.store, &c.profile).unwrap().revision, floor.journal_revision + 3);
}

#[test]
fn independent_initial_current_availability_source_and_floors_refuse_without_mutation() {
    let root = Directory::new(); let (mut c, floor) = setup(&root, true, false, false);
    let original = topology(1); let before = root.files();
    for mode in 0..5 {
        let mut selected = floor;
        if mode == 0 { fs::write(root.path("initial.json"), topology(2)).unwrap(); }
        if mode == 1 { fs::write(root.path("current.json"), topology(2)).unwrap(); }
        if mode == 3 { selected.journal_revision = u64::MAX; }
        if mode == 4 { c.source_policy.source.generation += 1; }
        assert!(run(&c, &recipe(&root, selected, true, mode != 2, None), 900,
            || panic!("mismatching requirements must precede clock"), &mut Vec::new()).is_err());
        assert_eq!(root.files(), before);
        fs::write(root.path("initial.json"), &original).unwrap();
        fs::write(root.path("current.json"), &original).unwrap();
        if mode == 4 { c.source_policy.source.generation -= 1; }
    }
    run(&c, &recipe(&root, floor, true, true, None), 900, || ElapsedTick(2), &mut Vec::new()).unwrap();
}

#[test]
fn topology_custody_rejects_missing_malformed_symlinked_and_in_store_inputs() {
    let root = Directory::new(); let (c, floor) = setup(&root, true, false, false);
    let text = document(&root, floor, true, true, None);
    for mode in 0..4 {
        let outside = root.path("current.json");
        fs::remove_file(&outside).unwrap();
        let mut candidate = text.clone();
        match mode {
            0 => {}
            1 => fs::write(&outside, b"{}").unwrap(),
            2 => std::os::unix::fs::symlink(root.path("initial.json"), &outside).unwrap(),
            _ => {
                fs::write(&outside, topology(1)).unwrap();
                fs::write(root.path("store/inside.json"), topology(1)).unwrap();
                candidate = text.replace(outside.to_str().unwrap(), root.path("store/inside.json").to_str().unwrap());
            }
        }
        let before = root.files();
        assert!(run(&c, &Profile::decode(candidate.as_bytes()).unwrap(), 900,
            || panic!("unadmitted topology must precede clock"), &mut Vec::new()).is_err());
        assert_eq!(root.files(), before);
        if fs::symlink_metadata(&outside).is_ok() { fs::remove_file(&outside).unwrap(); }
        fs::write(&outside, topology(1)).unwrap();
    }
    run(&c, &recipe(&root, floor, true, true, None), 900, || ElapsedTick(2), &mut Vec::new()).unwrap();
}

#[test]
fn one_deadline_covers_independent_intake_every_replayed_event_and_ready() {
    let root = Directory::new(); let (c, floor) = setup(&root, true, false, false);
    let profile = recipe(&root, floor, true, true, None); let original = root.bytes();
    let total = Cell::new(0);
    run_with_elapsed(&c, &profile, 900, || ElapsedTick(2), &mut || {
        total.set(total.get() + 1); Duration::ZERO
    }, &mut Vec::new()).unwrap();
    assert!(total.get() > floor.journal_revision as usize);
    for boundary in 0..total.get() {
        fs::write(c.store.join("delivery.bin"), &original).unwrap();
        fs::write(c.store.join("delivery.pending"), b"preserve on deadline").unwrap();
        let before = root.files(); let mut calls = 0;
        let error = run_with_elapsed(&c, &profile, 900, || panic!("expired stop clock"), &mut || {
            let expired = calls == boundary; calls += 1;
            if expired { Duration::from_millis(c.timing.runtime_ms) } else { Duration::ZERO }
        }, &mut Vec::new()).unwrap_err();
        assert!(error.contains("deadline"), "{error}"); assert_eq!(root.files(), before);
    }
    run(&c, &profile, 900, || ElapsedTick(2), &mut Vec::new()).unwrap();
}

#[test]
fn exhausted_ordinary_capacity_uses_only_the_original_terminal_tail_and_drained_retry_needs_no_clock() {
    let root = Directory::new(); let (c, floor) = setup(&root, true, false, true);
    assert_eq!(floor.journal_revision as usize + 3, c.profile.delivery.limits.events);
    let mut out = Vec::new();
    run(&c, &recipe(&root, floor, true, true, None), 900, || ElapsedTick(2), &mut out).unwrap();
    assert_eq!(json(&out).get("drained").unwrap().as_bool(), Some(true));
    let bytes = root.bytes();
    run(&c, &recipe(&root, floor, true, false, None), 900,
        || panic!("full journal must not spend another event"), &mut Vec::new()).unwrap();
    assert_eq!(root.bytes(), bytes);
}

#[test]
fn lost_output_and_racing_archive_creation_do_not_undo_acknowledged_stop_or_overwrite_custody() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
        fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
    }
    let root = Directory::new(); let (c, floor) = setup(&root, true, false, false);
    let archive = fs::read(root.path("before.anchor")).unwrap();
    let profile = recipe(&root, floor, true, true, Some("raced.anchor"));
    let error = run(&c, &profile, 900, || {
        fs::write(root.path("raced.anchor"), b"another custodian").unwrap(); ElapsedTick(2)
    }, &mut Broken).unwrap_err();
    assert!(error.contains("native stop acknowledged"));
    assert!(error.contains("output failed")); assert!(error.contains("retention failed"));
    assert_eq!(fs::read(root.path("raced.anchor")).unwrap(), b"another custodian");
    assert_eq!(fs::read(root.path("before.anchor")).unwrap(), archive);
    let canonical = root.bytes();
    run(&c, &recipe(&root, floor, true, false, Some("corrected.anchor")), 900,
        || panic!("output retry clock"), &mut Vec::new()).unwrap();
    assert_eq!(root.bytes(), canonical); assert!(root.path("corrected.anchor").is_file());
}
