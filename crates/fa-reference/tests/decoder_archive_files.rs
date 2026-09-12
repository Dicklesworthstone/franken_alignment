//! Real files and independently launched processes consume the same archive API.
#[path = "support/decoder_fixture.rs"]
#[allow(dead_code)]
mod fixture;
use fixture::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::archive::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::archive::files::*;
use fa_reference::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-sampled-archive-{}-{now}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("archive cleanup: {error}"); } }
}
fn loaded() -> DecoderModel {
    DecoderModel::from_safetensors(profile(48), include_bytes!("fixtures/decoder_mixed.safetensors")).unwrap().0
}
fn policy() -> SamplingPolicy { SamplingPolicy::new(4, 2, 6, 0.8, 5, 0.95).unwrap() }
fn start() -> SamplingStart { SamplingStart { policy: policy(), stream: 77, seed: 123 } }
fn budget() -> SampleBudget {
    SampleBudget { decoder: DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS },
        sampling: SamplingBudget { vocabulary: 6 } }
}
fn populated(m: &DecoderModel) -> SampledSession {
    let mut session = m.recompute_sampled(9, &[0, 3], budget().decoder, start()).unwrap();
    for _ in 0..4 { session.advance_sampled(session.position(), budget()).unwrap(); }
    session.advance_forced(6, 2, budget().decoder).unwrap();
    session
}
fn continue_twelve(s: &mut SampledSession) {
    for _ in 0..12 { s.advance_sampled(s.position(), budget()).unwrap(); }
}
fn same(a: &SampledSession, b: &SampledSession) {
    assert_eq!(a.tokens(), b.tokens()); assert_eq!(a.sampler_state(), b.sampler_state());
    assert_eq!(a.sampled_positions(), b.sampled_positions());
    let bits = |v: &[f32]| v.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(a.logits().unwrap()), bits(b.logits().unwrap()));
    let a = a.cache_image().unwrap(); let b = b.cache_image().unwrap();
    for id in a.profile().layers().keys() {
        assert_eq!(&a.layer(*id).unwrap().encode().unwrap()[156..], &b.layer(*id).unwrap().encode().unwrap()[156..]);
    }
}
fn child(mode: &str, directory: &Path) {
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "archive_process_entry", "--nocapture"])
        .env("FA_ARCHIVE_PROCESS_MODE", mode).env("FA_ARCHIVE_PROCESS_DIRECTORY", directory)
        .status().unwrap();
    assert!(status.success(), "archive child {mode} failed: {status}");
}

// Entry fixture, not an additional independent scenario or trained-model evidence.
#[test]
fn archive_process_entry() {
    let Some(mode) = std::env::var_os("FA_ARCHIVE_PROCESS_MODE") else { return; };
    let directory = PathBuf::from(std::env::var_os("FA_ARCHIVE_PROCESS_DIRECTORY").unwrap());
    let m = loaded();
    match mode.to_str().unwrap() {
        "save" => {
            let mut session = populated(&m);
            session.checkpoint().unwrap().save_archive_new(directory.join("source.bin"), ArchiveLimits::default()).unwrap();
            continue_twelve(&mut session);
            session.checkpoint().unwrap().save_archive_new(directory.join("expected.bin"), ArchiveLimits::default()).unwrap();
        }
        "resume" => {
            let (mut session, receipt) = m.recompute_sampled_file(directory.join("source.bin"), &policy(), 10,
                ArchiveLimits::default(), budget()).unwrap();
            assert_eq!(receipt.tokens_compared, 7); assert_eq!(receipt.sampled_tokens_compared, 4);
            continue_twelve(&mut session);
            session.checkpoint().unwrap().save_archive_new(directory.join("actual.bin"), ArchiveLimits::default()).unwrap();
        }
        "reject" => {
            assert_eq!(m.recompute_sampled_file(directory.join("damaged.bin"), &policy(), 10,
                ArchiveLimits::default(), budget()).unwrap_err(), ArchiveFileError::Replay(Error::Binding));
        }
        other => panic!("unexpected archive fixture mode {other}"),
    }
}

#[test]
fn another_process_reloads_weights_checks_state_and_continues_identically() {
    let directory = Directory::new();
    child("save", &directory.0);
    child("resume", &directory.0);
    let m = loaded();
    let (expected, _) = m.recompute_sampled_file(directory.0.join("expected.bin"), &policy(), 21,
        ArchiveLimits::default(), budget()).unwrap();
    let (actual, _) = m.recompute_sampled_file(directory.0.join("actual.bin"), &policy(), 22,
        ArchiveLimits::default(), budget()).unwrap();
    same(&expected, &actual);
    assert_eq!(actual.position(), 19); assert_eq!(actual.sampler_state().draws(), 16);
    let mut bytes = fs::read(directory.0.join("source.bin")).unwrap();
    let end = bytes.len(); bytes[end - 4..].copy_from_slice(&123.0_f32.to_le_bytes());
    fs::write(directory.0.join("damaged.bin"), bytes).unwrap();
    child("reject", &directory.0);
}

#[test]
fn real_file_round_trip_retains_source_after_loss_and_never_overwrites_existing_bytes() {
    let directory = Directory::new(); let m = loaded(); let mut original = populated(&m);
    let cp = original.checkpoint().unwrap(); let path = directory.0.join("state.bin");
    let length = cp.save_archive_new(&path, ArchiveLimits::default()).unwrap();
    let saved = fs::read(&path).unwrap(); assert_eq!(saved.len(), length);
    assert!(matches!(cp.save_archive_new(&path, ArchiveLimits::default()),
        Err(ArchiveFileError::Io { operation: ArchiveFileOperation::Create, kind: std::io::ErrorKind::AlreadyExists })));
    assert_eq!(fs::read(&path).unwrap(), saved);
    let (mut resumed, _) = m.recompute_sampled_file(&path, &policy(), 10, ArchiveLimits::default(), budget()).unwrap();
    fs::remove_file(&path).unwrap(); drop(cp);
    continue_twelve(&mut original); continue_twelve(&mut resumed); same(&original, &resumed);
    resumed.checkpoint().unwrap().save_archive_new(directory.0.join("continued.bin"), ArchiveLimits::default()).unwrap();
}

#[test]
fn refusal_precedes_output_creation_and_exact_file_byte_limits_are_enforced() {
    let directory = Directory::new(); let m = loaded(); let cp = populated(&m).checkpoint().unwrap();
    let path = directory.0.join("state.bin");
    assert_eq!(cp.save_archive_new(&path, ArchiveLimits { bytes: 0, ..ArchiveLimits::default() }),
        Err(ArchiveFileError::Format(Error::Limit)));
    assert!(!path.exists());
    let size = cp.save_archive_new(&path, ArchiveLimits::default()).unwrap();
    let exact = ArchiveLimits { bytes: size, ..ArchiveLimits::default() };
    assert!(SampledArchive::read_file(&path, &m, &policy(), exact).is_ok());
    assert_eq!(SampledArchive::read_file(&path, &m, &policy(), ArchiveLimits { bytes: size - 1, ..exact }).unwrap_err(),
        ArchiveFileError::Format(Error::Limit));
}

#[test]
fn absent_nonregular_truncated_and_inconsistent_files_have_distinct_failure_classes() {
    let directory = Directory::new(); let m = loaded(); let path = directory.0.join("state.bin");
    assert!(matches!(SampledArchive::read_file(&path, &m, &policy(), ArchiveLimits::default()),
        Err(ArchiveFileError::Io { operation: ArchiveFileOperation::Metadata, kind: std::io::ErrorKind::NotFound })));
    assert_eq!(SampledArchive::read_file(&directory.0, &m, &policy(), ArchiveLimits::default()).unwrap_err(), ArchiveFileError::NotRegular);
    let bytes = populated(&m).checkpoint().unwrap().encode_archive(ArchiveLimits::default()).unwrap();
    fs::write(&path, &bytes[..bytes.len() - 1]).unwrap();
    assert!(matches!(m.recompute_sampled_file(&path, &policy(), 10, ArchiveLimits::default(), budget()), Err(ArchiveFileError::Format(_))));
    let mut altered = bytes; let end = altered.len(); altered[end - 4..].copy_from_slice(&123.0_f32.to_le_bytes());
    fs::write(&path, altered).unwrap();
    assert_eq!(m.recompute_sampled_file(&path, &policy(), 10, ArchiveLimits::default(), budget()).unwrap_err(),
        ArchiveFileError::Replay(Error::Binding));
}

#[cfg(unix)]
#[test]
fn symlinks_refuse_and_new_archives_do_not_start_world_readable() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let directory = Directory::new(); let m = loaded(); let cp = populated(&m).checkpoint().unwrap();
    let path = directory.0.join("state.bin"); let link = directory.0.join("link.bin");
    cp.save_archive_new(&path, ArchiveLimits::default()).unwrap();
    assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o077, 0);
    symlink(&path, &link).unwrap();
    assert_eq!(SampledArchive::read_file(&link, &m, &policy(), ArchiveLimits::default()).unwrap_err(), ArchiveFileError::NotRegular);
    let before = fs::read(&path).unwrap();
    assert!(cp.save_archive_new(&link, ArchiveLimits::default()).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn independent_empty_format_fixture_matches_canonical_export_and_recomputation() {
    let m = model(profile(8));
    let session = m.sampled_session(9, start()).unwrap();
    let expected = include_bytes!("fixtures/sampled_checkpoint_empty_v1.bin");
    assert_eq!(session.checkpoint().unwrap().encode_archive(ArchiveLimits::default()).unwrap(), expected.as_slice());
    let parsed = SampledArchive::decode(expected, &m, &policy(), ArchiveLimits::default()).unwrap();
    let (replayed, receipt) = parsed.recompute(&m, 10, budget()).unwrap();
    assert!(replayed.tokens().is_empty()); assert_eq!(receipt.encoded_bytes, 676);
    assert_eq!(replayed.sampler_state(), session.sampler_state());
}
