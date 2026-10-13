//! Independent custody around actual clock-free stop/drain acknowledgments.
use super::*;
use fa_reference::action::consequence::delivery::persistent::RecoveryReserve;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::anchored::{
    FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES};
use std::{fs, io::{Cursor, Write}, path::Path, time::Duration};
#[path = "../../../../tests/support/file_oversight.rs"]
mod fixture;
use fixture::Directory;

fn configured(root: &Directory) -> Config {
    let mut c = Config::decode(include_bytes!("../../../../fixtures/supervised_publication.json")).unwrap();
    c.store = root.store(); c.programs.clear(); c.timing.runtime_ms = 10000; c
}
fn owner(root: &Directory) -> Config {
    let c = configured(root);
    let (mut host, _) = FileOversight::create(&c.store, c.profile.clone()).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    drop(host); c
}
fn read(path: &Path) -> FileHistoryAnchor {
    let bytes = fs::read(path).unwrap();
    FileHistoryAnchor::read_trusted(&mut Cursor::new(bytes), MAX_HISTORY_ANCHOR_BYTES).unwrap()
}

#[test]
fn admission_and_drain_export_distinct_acknowledged_successors_without_overwrite() {
    let root = Directory::new(); let c = owner(&root);
    let parent = c.store.parent().unwrap();
    let first = parent.join("admission.anchor"); let second = parent.join("drain.anchor");
    let out1 = AnchorOutput::admit(&c, &first).unwrap();
    run_with_retention(&c, Phase::Admission, 900, || panic!("admission is clock-free"),
        &mut || Duration::ZERO, &mut Vec::new(), Some(&out1)).unwrap();
    let a1 = read(&first);
    let stopped = FileOversight::read_publication(&c.store, &c.profile).unwrap();
    assert_eq!(a1.revision(), stopped.revision);
    let retained_first = fs::read(&first).unwrap();

    let out2 = AnchorOutput::admit(&c, &second).unwrap();
    run_with_retention(&c, Phase::Drain, 900, || Ok(ElapsedTick(2)),
        &mut || Duration::ZERO, &mut Vec::new(), Some(&out2)).unwrap();
    let a2 = read(&second);
    let drained = FileOversight::read_publication(&c.store, &c.profile).unwrap();
    assert_eq!(a2.revision(), drained.revision);
    assert!(a2.revision() > a1.revision());
    assert_eq!(fs::read(&first).unwrap(), retained_first);
    assert!(AnchorOutput::admit(&c, &first).is_err());
}

#[test]
fn custody_admission_refuses_existing_or_authority_local_outputs_before_native_work() {
    let root = Directory::new(); let c = owner(&root);
    let before = fs::read(c.store.join("delivery.bin")).unwrap();
    let existing = c.store.parent().unwrap().join("existing.anchor");
    fs::write(&existing, b"keep").unwrap();
    assert!(AnchorOutput::admit(&c, &existing).is_err());
    assert!(AnchorOutput::admit(&c, &c.store.join("inside.anchor")).is_err());
    assert_eq!(fs::read(c.store.join("delivery.bin")).unwrap(), before);
    assert_eq!(fs::read(existing).unwrap(), b"keep");
}

#[test]
fn lost_stdout_after_export_keeps_archive_and_exact_retry_appends_nothing() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> { Err(std::io::ErrorKind::BrokenPipe.into()) }
        fn flush(&mut self) -> std::io::Result<()> { Err(std::io::ErrorKind::BrokenPipe.into()) }
    }
    let root = Directory::new(); let c = owner(&root);
    let path = c.store.parent().unwrap().join("retained.anchor");
    let output = AnchorOutput::admit(&c, &path).unwrap();
    let error = run_with_retention(&c, Phase::Admission, 900, || panic!("clock-free"),
        &mut || Duration::ZERO, &mut Broken, Some(&output)).unwrap_err();
    assert!(error.contains("output failed"));
    let anchor = read(&path);
    let state = FileOversight::read_publication(&c.store, &c.profile).unwrap();
    assert_eq!(anchor.revision(), state.revision);
    let native = fs::read(c.store.join("delivery.bin")).unwrap();
    run_with_elapsed(&c, Phase::Admission, 900, || panic!("exact retry"),
        &mut || Duration::ZERO, &mut Vec::new()).unwrap();
    assert_eq!(fs::read(c.store.join("delivery.bin")).unwrap(), native);
}
