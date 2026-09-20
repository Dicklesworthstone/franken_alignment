use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::FileWitnessInput;
use fa_reference::product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker};
use fa_reference::witness::{AdapterDomainInput, DomainClosure, DomainProjection, SnapshotEntry};
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-producer-cli-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn write(&self, name: &str, bytes: &[u8]) -> String {
        let path = self.0.join(name); std::fs::write(&path, bytes).unwrap(); path.to_str().unwrap().to_owned()
    }
}
impl Drop for Directory { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
fn profile(root: &Directory) -> Vec<u8> {
    format!(r#"{{"schema":"fa.publication-producer/1","directory":"{}/producer","source":91,"feed":41,"after":0,"clock":"unix_milliseconds","scope":{{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5}},"minimum_generation":1}}"#, root.0.display()).into_bytes()
}
fn inputs(revision: u64, value: u8) -> FilePublicationInputs {
    let key = ProjectionKey { source: 40, branch: 4, projection: 7, source_epoch: 1 };
    let close = TrustedClosingMarker { key, final_sequence: 1, marker_generation: 1 };
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap(); frontiers.record_close(close).unwrap();
    FilePublicationInputs::new(Some(FileWitnessInput::new(revision, revision, 20,
        AdapterDomainInput::new(DomainProjection::new(40, 1, key), DomainClosure::Closed(close)),
        vec![SnapshotEntry::new(0, 1, vec![0, value, 255]).unwrap()], &frontiers).unwrap()), None)
}
fn observation(expected_generation: u64, observed_at: u64, inputs: FilePublicationInputs) -> Vec<u8> {
    Observation { expected_generation, observed_at: ElapsedTick(observed_at), inputs }.encode().unwrap()
}
fn call(args: &[&str]) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new(); run(&args.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>(), &mut bytes)?; Ok(bytes)
}
fn inspect(root: &Directory) -> fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::producer::PublicationProducerImage {
    let p = Profile::decode(&profile(root)).unwrap();
    FilePublicationProducer::read_image(&p.directory, p.identity, p.minimum_generation).unwrap()
}

#[test]
fn observation_builder_seals_original_time_and_binary_native_input_without_store_io() {
    let root = Directory::new(); let original = inputs(1, 128);
    let packet = root.write("inputs.bin", &original.to_bytes().unwrap());
    let bytes = call(&["observation", "9007199254740993", "18446744073709551615", &packet]).unwrap();
    let decoded = Observation::decode(&bytes).unwrap();
    assert_eq!(decoded.expected_generation, 9007199254740993);
    assert_eq!(decoded.observed_at, ElapsedTick(u64::MAX)); assert_eq!(decoded.inputs, original);
    assert_eq!(decoded.encode().unwrap(), bytes);
    assert!(!root.0.join("producer").exists());
    assert!(call(&["observation", "18446744073709551615", "1000", &packet]).is_err());
}

#[test]
fn lifecycle_persists_complete_inputs_and_retries_do_not_refresh_time_or_emit_changes() {
    let root = Directory::new(); let p = root.write("profile.json", &profile(&root));
    let first = inputs(1, 1); let initial = root.write("initial.json", &observation(0, 1000, first.clone()));
    assert!(String::from_utf8(call(&["create", &p, &initial]).unwrap()).unwrap().contains("\"created\""));
    let before = std::fs::read(root.0.join("producer/delivery.bin")).unwrap();
    assert!(String::from_utf8(call(&["publish", &p, &initial]).unwrap()).unwrap().contains("\"already_current\""));
    assert_eq!(std::fs::read(root.0.join("producer/delivery.bin")).unwrap(), before);
    let heartbeat = root.write("heartbeat.json", &observation(1, 1001, first));
    call(&["publish", &p, &heartbeat]).unwrap();
    let quiet = inspect(&root); assert_eq!(quiet.generation(), 2); assert_eq!(quiet.input_generation(), 1);
    assert!(quiet.batch().records().is_empty());
    let changed = root.write("changed.json", &observation(2, 1002, inputs(2, 2)));
    call(&["publish", &p, &changed]).unwrap(); let changed_image = inspect(&root);
    assert_eq!(changed_image.generation(), 3); assert_eq!(changed_image.input_generation(), 2);
    assert_eq!(changed_image.batch().heartbeat().through, 1); assert_eq!(changed_image.batch().records().len(), 1);
    call(&["publish", &p, &changed]).unwrap(); assert_eq!(inspect(&root), changed_image);
    assert!(call(&["publish", &p, &initial]).is_err()); assert_eq!(inspect(&root), changed_image);
}

struct LostOutput;
impl Write for LostOutput {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
    fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
}
#[test]
fn lost_stdout_receipt_does_not_undo_or_duplicate_the_committed_observation() {
    let root = Directory::new(); let p = root.write("profile.json", &profile(&root));
    let initial = root.write("initial.json", &observation(0, 1000, inputs(1, 1)));
    assert!(run(&["create".into(), p.clone(), initial.clone()], &mut LostOutput).is_err());
    let committed = inspect(&root); assert_eq!(committed.generation(), 1);
    assert!(call(&["create", &p, &initial]).is_err(), "create must not open an existing store");
    call(&["publish", &p, &initial]).unwrap(); assert_eq!(inspect(&root), committed);
    let conflict = root.write("conflict.json", &observation(0, 1001, inputs(1, 1)));
    assert!(call(&["publish", &p, &conflict]).is_err()); assert_eq!(inspect(&root), committed);
}

#[test]
fn inspection_is_read_only_with_writer_lock_and_staging_leftover_and_checks_floor() {
    let root = Directory::new(); let p = root.write("profile.json", &profile(&root));
    let parsed = Profile::read(Path::new(&p)).unwrap();
    let (owner, _) = FilePublicationProducer::create(&parsed.directory, parsed.identity, inputs(1, 1), ElapsedTick(1000)).unwrap();
    let pending = parsed.directory.join("delivery.pending"); std::fs::write(&pending, b"unconfirmed").unwrap();
    let bytes = std::fs::read(parsed.directory.join("delivery.bin")).unwrap();
    assert!(String::from_utf8(call(&["inspect", &p]).unwrap()).unwrap().contains("\"historical\""));
    assert_eq!(std::fs::read(&pending).unwrap(), b"unconfirmed");
    assert_eq!(std::fs::read(parsed.directory.join("delivery.bin")).unwrap(), bytes);
    assert_eq!(inspect(&root), *owner.image());
    assert!(FilePublicationProducer::read_image(&parsed.directory, parsed.identity, 2).is_err());
    let mut wrong = parsed.identity; wrong.scope.authority += 1;
    assert!(FilePublicationProducer::read_image(&parsed.directory, wrong, 1).is_err());
    assert_eq!(std::fs::read(&pending).unwrap(), b"unconfirmed");
}

#[test]
fn malformed_observation_is_rejected_before_recovery_cleanup_or_writes() {
    let root = Directory::new(); let p = root.write("profile.json", &profile(&root));
    let original = observation(0, 1000, inputs(1, 1)); let initial = root.write("initial.json", &original);
    call(&["create", &p, &initial]).unwrap();
    let pending = root.0.join("producer/delivery.pending"); std::fs::write(&pending, b"do not clean").unwrap();
    let before = std::fs::read(root.0.join("producer/delivery.bin")).unwrap();
    let text = String::from_utf8(original).unwrap();
    for bad in [text.replace("\"inputs_hex\":\"", "\"inputs_hex\":\"0"),
        text.replace("\"inputs_hex\":\"", "\"inputs_hex\":\"zz"),
        text.replace("\"expected_generation\":0", "\"expected_generation\":0,\"changes\":[]"),
        text.replace("\"expected_generation\":0", "\"expected_generation\":0,\"expected_generation\":1"),
        text.replace("fa.publication-observation/1", "fa.publication-observation/2")] {
        let path = root.write("bad.json", bad.as_bytes()); assert!(call(&["publish", &p, &path]).is_err());
        assert_eq!(std::fs::read(&pending).unwrap(), b"do not clean");
        assert_eq!(std::fs::read(root.0.join("producer/delivery.bin")).unwrap(), before);
    }
}

#[test]
fn no_missing_store_fallback_and_bounded_regular_file_acquisition() {
    let root = Directory::new(); let p = root.write("profile.json", &profile(&root));
    let initial = root.write("initial.json", &observation(0, 1000, inputs(1, 1)));
    assert!(call(&["publish", &p, &initial]).is_err()); assert!(call(&["inspect", &p]).is_err());
    assert!(!root.0.join("producer").exists());
    let packet = root.write("packet.bin", b"01234567");
    assert!(read_regular(Path::new(&packet), 4).is_err());
    let link = root.0.join("link.bin"); std::os::unix::fs::symlink(&packet, &link).unwrap();
    assert!(read_regular(&link, 100).is_err()); assert!(read_regular(&root.0, 100).is_err());
    assert!(call(&["observation", "0", "1000", &packet]).is_err());
}

#[test]
fn profile_rejects_missing_scope_zero_floor_and_unrecognized_fields() {
    let root = Directory::new(); let text = String::from_utf8(profile(&root)).unwrap();
    assert!(Profile::decode(text.as_bytes()).is_ok());
    for bad in [text.replace("\"minimum_generation\":1", "\"minimum_generation\":0"),
        text.replace("\"source\":91", "\"source\":0"), text.replace("unix_milliseconds", "wall_seconds"),
        text.replace("\"after\":0", "\"after\":0,\"generation\":1"),
        text.replace("\"scope\":{", "\"scope\":{\"purpose\":\"effect\","),
        text.replace("\"source\":91", "\"source\":91,\"source\":92")] {
        assert!(Profile::decode(bad.as_bytes()).is_err());
    }
    assert!(!root.0.join("producer").exists());
}

#[path = "source_tests.rs"]
mod source_tests;
