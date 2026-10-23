//! Actual source-publication operator, original journals and independent files.
use super::*;
use super::super::guarded::{self, Profile};
use fa_reference::action::consequence::delivery::persistent::{FileDeliverySnapshot,
    observed::guarded::{FileRecoveryFloor, anchored::{FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES}}};
use std::{cell::Cell, fs, io::Cursor, path::Path, os::unix::fs::{PermissionsExt, symlink}};

struct Case {
    config: Config, floor: FileRecoveryFloor, anchor: PathBuf,
    archive: Vec<u8>, before: FileDeliverySnapshot,
}
fn capture(config: Config, host: FileOversight, root: &Directory) -> Case {
    let before = host.inspect();
    let floor = FileRecoveryFloor { journal_revision: before.revision,
        control_sequence: before.control.sequence, authority_epoch: before.control.ledger.epoch };
    let mut archive = Vec::new();
    host.history_anchor().unwrap().write_to(&mut archive, MAX_HISTORY_ANCHOR_BYTES).unwrap();
    let anchor = root.0.join("independent.anchor"); fs::write(&anchor, &archive).unwrap(); drop(host);
    Case { config, floor, anchor, archive, before }
}
fn case(root: &Directory) -> Case {
    let mut c = configured(root); c.programs.clear();
    let mut host = empty_owner(&c); host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    capture(c, host, root)
}
fn document(floor: FileRecoveryFloor, input: &Path, output: Option<&Path>) -> Vec<u8> {
    let retained = output.map(|path| format!(",\"retain_history_anchor\":{:?}", path.to_str().unwrap())).unwrap_or_default();
    format!(concat!("{{\"schema_version\":\"fa.guarded-stop/1\",\"minimum\":{{",
        "\"journal_revision\":{},\"control_sequence\":{},\"authority_epoch\":{}}},",
        "\"history_anchor\":{:?}{}}}"), floor.journal_revision, floor.control_sequence,
        floor.authority_epoch, input.to_str().unwrap(), retained).into_bytes()
}
fn selected(case: &Case, output: Option<&Path>) -> Profile {
    Profile::decode(&document(case.floor, &case.anchor, output)).unwrap()
}
fn read_anchor(path: &Path) -> FileHistoryAnchor {
    FileHistoryAnchor::read_trusted(&mut Cursor::new(fs::read(path).unwrap()), MAX_HISTORY_ANCHOR_BYTES).unwrap()
}
fn files(c: &Config) -> BTreeMap<PathBuf, Vec<u8>> {
    fs::read_dir(&c.store).unwrap().map(|entry| entry.unwrap().path()).filter(|path| path.is_file())
        .map(|path| { let bytes = fs::read(&path).unwrap(); (path, bytes) }).collect()
}

#[test]
fn guarded_profile_is_closed_bounded_and_requires_each_independent_floor_and_anchor() {
    let root = Directory::new(); let case = case(&root);
    let good = String::from_utf8(document(case.floor, &case.anchor, None)).unwrap();
    assert!(Profile::decode(good.as_bytes()).is_ok());
    for name in ["journal_revision", "control_sequence", "authority_epoch", "history_anchor"] {
        let bad = good.replace(&format!("\"{name}\""), "\"unexpected\"");
        assert!(Profile::decode(bad.as_bytes()).is_err(), "missing {name}");
    }
    for bad in [
        good.replace("fa.guarded-stop/1", "fa.guarded-stop/2"),
        good.replacen('{', "{\"unknown\":0,", 1),
        good.replacen('{', "{\"schema_version\":\"fa.guarded-stop/1\",", 1),
        good.replace(&format!("\"journal_revision\":{}", case.floor.journal_revision), "\"journal_revision\":-1"),
        good.replace(&format!("\"journal_revision\":{}", case.floor.journal_revision), "\"journal_revision\":1.5"),
        good.replace(&format!("\"journal_revision\":{}", case.floor.journal_revision), "\"journal_revision\":18446744073709551616"),
    ] { assert!(Profile::decode(bad.as_bytes()).is_err()); }
    assert!(Profile::decode(&document(case.floor, Path::new("relative.anchor"), None)).is_err());
    assert!(Profile::decode(&document(case.floor, &case.anchor, Some(Path::new("relative.output")))).is_err());
    let mut oversized = good.into_bytes(); oversized.resize(16 * 1024 + 1, b' ');
    assert!(Profile::decode(&oversized).is_err());
    assert!(Profile::read(&root.0.join("missing-profile")).is_err());
    assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
}

#[test]
fn ordinary_guarded_stop_needs_no_live_assets_and_exports_an_identical_drained_retry() {
    for export in [false, true] {
        let root = Directory::new(); let case = case(&root); let c = &case.config;
        assert!(c.programs.is_empty()); assert!(!root.0.join("evidence.json").exists());
        let target = root.0.join("stopped.anchor"); let profile = selected(&case, export.then_some(target.as_path()));
        let reads = c.source.status().read_attempts; let calls = Cell::new(0); let mut stdout = Vec::new();
        guarded::run(c, &profile, 900, || { calls.set(calls.get() + 1); ElapsedTick(2) }, &mut stdout).unwrap();
        assert_eq!(calls.get(), 1); assert_eq!(output(&stdout).get("status").unwrap().as_str(), Some("stopped_drained"));
        let state = FileOversight::read_publication(&c.store, &c.profile).unwrap();
        assert_eq!(state.revision, case.before.revision + 3); assert_eq!(state.executions, 0);
        assert_eq!(target.exists(), export);
        if export {
            assert_eq!(read_anchor(&target).revision(), state.revision);
            assert_eq!(fs::metadata(&target).unwrap().permissions().mode() & 0o077, 0);
        }
        let completed = files(c); let retry = root.0.join("retry.anchor");
        let input = if export { &target } else { &case.anchor };
        let profile = Profile::decode(&document(case.floor, input, Some(&retry))).unwrap();
        let mut stdout = Vec::new();
        guarded::run(c, &profile, 900, || panic!("drained stop never obtains terminal time"), &mut stdout).unwrap();
        assert_eq!(output(&stdout).get("recovery").unwrap().as_str(), Some("already_drained"));
        assert_eq!(files(c), completed); assert_eq!(read_anchor(&retry).revision(), state.revision);
        if export { assert_eq!(fs::read(&retry).unwrap(), fs::read(&target).unwrap()); }
        assert_eq!(c.source.status().read_attempts, reads); assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
        assert!(!c.socket(1).exists());
    }
}

#[test]
fn every_elapsed_boundary_including_ready_cancels_without_terminal_or_export_mutation() {
    let root = Directory::new(); let mut case = case(&root); case.config.timing.runtime_ms = 50;
    let c = &case.config; let target = root.0.join("bounded.anchor"); let profile = selected(&case, Some(&target));
    fs::write(c.store.join("delivery.pending"), b"preserve staging").unwrap();
    let before = files(c); let limit = Duration::from_millis(50);
    let below = limit.checked_sub(Duration::from_nanos(1)).unwrap(); let events = case.before.revision as usize;
    // Before custody, before open, then before each event and at Ready.
    for cut in 0..=events + 2 {
        let mut calls = 0; let mut stdout = Vec::new();
        let result = guarded::run_with_elapsed(c, &profile, 900,
            || panic!("expired replay cannot read terminal time"), &mut || {
                let current = calls; calls += 1; if current == cut { limit } else { below }
            }, &mut stdout);
        assert_eq!(result.err().as_deref(), Some("guarded stop recovery replay deadline reached"));
        assert_eq!(calls, cut + 1); assert!(stdout.is_empty()); assert!(!target.exists());
        assert_eq!(files(c), before); assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
    }
    let mut calls = 0;
    guarded::run_with_elapsed(c, &profile, 900, || ElapsedTick(2),
        &mut || { calls += 1; below }, &mut Vec::new()).unwrap();
    assert_eq!(calls, events + 3); assert_eq!(read_anchor(&target).revision(), case.before.revision + 3);
}

#[test]
fn independent_source_floors_and_anchor_failures_do_not_fall_back_to_legacy_recovery() {
    let root = Directory::new(); let mut case = case(&root); let target = root.0.join("guarded.anchor");
    fs::write(case.config.store.join("delivery.pending"), b"keep on guard failure").unwrap();
    let before = files(&case.config); let profile = selected(&case, Some(&target));
    let original_source = case.config.source_policy;
    case.config.source_policy.source.generation += 1;
    let error = guarded::run(&case.config, &profile, 900, || panic!("wrong source"), &mut Vec::new()).unwrap_err();
    assert!(error.contains("Binding")); assert_eq!(files(&case.config), before); case.config.source_policy = original_source;
    for dimension in 0..3 {
        let mut floor = case.floor;
        match dimension { 0 => floor.journal_revision = u64::MAX, 1 => floor.control_sequence = u64::MAX,
            _ => floor.authority_epoch = u64::MAX }
        let profile = Profile::decode(&document(floor, &case.anchor, Some(&target))).unwrap();
        let error = guarded::run(&case.config, &profile, 900, || panic!("wrong floor"), &mut Vec::new()).unwrap_err();
        assert!(error.contains("Stale")); assert_eq!(files(&case.config), before); assert!(!target.exists());
    }
    let foreign_root = Directory::new(); let foreign = self::case(&foreign_root);
    let missing = root.0.join("missing.anchor");
    for input in [&foreign.anchor, &missing] {
        let profile = Profile::decode(&document(case.floor, input, Some(&target))).unwrap();
        assert!(guarded::run(&case.config, &profile, 900, || panic!("invalid independent anchor"), &mut Vec::new()).is_err());
        assert_eq!(files(&case.config), before); assert!(!target.exists());
    }
    guarded::run(&case.config, &profile, 900, || ElapsedTick(2), &mut Vec::new()).unwrap();
    assert_eq!(read_anchor(&target).revision(), case.before.revision + 3);
}

#[test]
fn unsafe_custody_paths_are_refused_before_opening_the_original_recovery() {
    let root = Directory::new(); let case = case(&root); let c = &case.config;
    let inside = c.store.join("inside.anchor"); let dangling = root.0.join("dangling.anchor");
    symlink(root.0.join("absent"), &dangling).unwrap();
    let alias = root.0.join("store-alias"); symlink(&c.store, &alias).unwrap();
    let aliased = alias.join("not-independent.anchor"); let before = files(c);
    for path in [&inside, &case.anchor, &dangling, &aliased] {
        let profile = selected(&case, Some(path)); let mut stdout = Vec::new();
        assert!(guarded::run(c, &profile, 900, || panic!("unsafe output"), &mut stdout).is_err());
        assert!(stdout.is_empty()); assert_eq!(files(c), before);
    }
    assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
    assert!(fs::symlink_metadata(&dangling).unwrap().file_type().is_symlink());
    let bad_input = c.store.join("not-independent-input.anchor"); fs::write(&bad_input, &case.archive).unwrap();
    let target = root.0.join("valid.anchor"); let profile = Profile::decode(&document(case.floor, &bad_input, Some(&target))).unwrap();
    let before = files(c);
    assert!(guarded::run(c, &profile, 900, || panic!("inside input"), &mut Vec::new()).unwrap_err().contains("outside the authority store"));
    assert_eq!(files(c), before); assert!(!target.exists());
    guarded::run(c, &selected(&case, Some(&target)), 900, || ElapsedTick(2), &mut Vec::new()).unwrap();
    assert_eq!(read_anchor(&target).revision(), case.before.revision + 3);
}

struct Broken;
impl Write for Broken {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
    fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
}
#[test]
fn late_export_collision_and_lost_reports_preserve_acknowledgment_and_retention_only_retry() {
    for collide in [false, true] {
        for broken in [false, true] {
            let root = Directory::new(); let case = case(&root); let c = &case.config;
            let target = root.0.join("competing.anchor"); let profile = selected(&case, Some(&target));
            let clock = || { if collide { fs::write(&target, b"other custodian").unwrap(); } ElapsedTick(2) };
            let mut stdout = Vec::new();
            let result = if broken { guarded::run(c, &profile, 900, clock, &mut Broken) }
                else { guarded::run(c, &profile, 900, clock, &mut stdout) };
            assert_eq!(result.is_err(), collide || broken);
            if let Err(error) = result {
                assert!(error.contains("native stop acknowledged"));
                assert_eq!(error.contains("output failed"), broken);
                assert_eq!(error.contains("history anchor retention failed"), collide);
            }
            if !broken { assert_eq!(output(&stdout).get("status").unwrap().as_str(), Some("stopped_drained")); }
            let state = FileOversight::read_publication(&c.store, &c.profile).unwrap();
            assert_eq!(state.revision, case.before.revision + 3);
            if collide { assert_eq!(fs::read(&target).unwrap(), b"other custodian"); }
            else { assert_eq!(read_anchor(&target).revision(), state.revision); }
            let before = files(c); let retry = root.0.join("corrected.anchor");
            guarded::run(c, &selected(&case, Some(&retry)), 900,
                || panic!("export/report failure never repeats a drained stop"), &mut Vec::new()).unwrap();
            assert_eq!(files(c), before); assert_eq!(read_anchor(&retry).revision(), state.revision);
            assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
        }
    }
}

#[test]
fn actual_guarded_stop_command_uses_the_independent_profile_without_live_assets() {
    let root = Directory::new(); let case = case(&root); let target = root.0.join("cli.anchor");
    let text = include_str!("../../../fixtures/supervised_publication.json")
        .replace("/operator/publication-store", case.config.store.to_str().unwrap())
        .replace("/operator/evidence.json", root.0.join("evidence.json").to_str().unwrap());
    let config = root.0.join("config.json"); fs::write(&config, text).unwrap();
    let parsed = Config::read(&config).unwrap();
    crate::config::assert_oversight_profiles_eq(&parsed.profile, &case.config.profile);
    let profile = root.0.join("recovery.json"); fs::write(&profile, document(case.floor, &case.anchor, Some(&target))).unwrap();
    let args = vec!["recover-stop".into(), config.to_str().unwrap().into(), "900".into(),
        "--guarded".into(), profile.to_str().unwrap().into()];
    crate::command(args.clone()).unwrap(); let completed = files(&case.config);
    assert_eq!(read_anchor(&target).revision(), case.before.revision + 3);
    let retry = root.0.join("cli-retry.anchor"); fs::write(&profile, document(case.floor, &target, Some(&retry))).unwrap();
    crate::command(args).unwrap(); assert_eq!(files(&case.config), completed);
    assert_eq!(fs::read(&retry).unwrap(), fs::read(&target).unwrap());
    assert!(!root.0.join("evidence.json").exists()); assert!(!case.config.socket(1).exists());
    assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
}

#[test]
fn guarded_operator_keeps_original_completed_and_expired_publication_charges() {
    for expired in [false, true] {
        let root = Directory::new(); let (c, tick) = publish(&root, expired);
        // Fixture custodian explicitly acknowledges and retains this original
        // prefix before the guarded stop invocation begins. This is not how
        // production obtains an independent anchor from an untrusted archive.
        let (host, _) = FileOversight::open(&c.store, c.profile.clone()).unwrap();
        let case = capture(c, host, &root); let c = &case.config;
        let operation = case.before.stop.as_ref().map(|stop| stop.request().operation).unwrap_or(900);
        let target = root.0.join("liability.anchor"); let profile = selected(&case, Some(&target)); let mut stdout = Vec::new();
        let result = guarded::run(c, &profile, operation, || ElapsedTick(tick), &mut stdout);
        assert_eq!(result.is_err(), expired);
        if expired { assert!(result.unwrap_err().contains("unresolved obligations")); }
        let json = output(&stdout); assert_eq!(json.get("charged").unwrap().as_str(), Some("4"));
        assert_eq!(json.get("drained").unwrap().as_bool(), Some(!expired));
        let state = FileOversight::read_publication(&c.store, &c.profile).unwrap();
        assert_eq!(state.executions, u64::from(!expired)); assert_eq!(state.control.ledger.charged, 4);
        assert_eq!(state.payload, case.before.payload); assert_eq!(read_anchor(&target).revision(), state.revision);
        assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
    }
}
