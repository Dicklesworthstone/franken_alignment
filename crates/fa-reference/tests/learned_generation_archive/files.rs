//! Actual files and a separate test process; synthetic weights are not a model
//! or deployment qualification. Rebuild every native input in the child process.
use super::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::replay::archive::files::{
    ArchiveFileError, ArchiveFileOperation,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-learned-archive-{}-{now}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("archive test cleanup: {error}"); }
    }
}
fn blueprint() -> ReplayableGeneration {
    let model = fixture::model(fixture::profile(16));
    run(&model, spec(&model, vec![4, 0, 3], 5, BTreeSet::new(), 4), false, GenerationTelemetryBudget::default())
}

#[test]
fn new_file_roundtrip_preserves_existing_files_and_invalid_exports_create_nothing() {
    let root = Directory::new(); let path = root.0.join("checkpoint.bin");
    let (original, bytes) = framed();
    let checkpoint = original.checkpoint(CheckpointLimits::default()).unwrap();
    assert_eq!(checkpoint.save_archive_new(&path, ArchiveLimits::default()).unwrap(), bytes.len());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o077, 0);
    }
    assert!(matches!(checkpoint.save_archive_new(&path, ArchiveLimits::default()),
        Err(ArchiveFileError::Io { operation: ArchiveFileOperation::Create, .. })));
    assert_eq!(fs::read(&path).unwrap(), bytes);
    let rejected = root.0.join("rejected.bin");
    assert_eq!(checkpoint.save_archive_new(&rejected, ArchiveLimits { bytes: 1, ..ArchiveLimits::default() }),
        Err(ArchiveFileError::Format(Error::Limit)));
    assert!(!rejected.exists());
    let independent = blueprint();
    let mut pending = independent.begin_replay_file(&path, ArchiveLimits::default(), ReplayBudget::default()).unwrap();
    fs::remove_file(&path).unwrap(); // Replay owns the checked image, not a pathname.
    assert_eq!(pending.advance(0).unwrap(), ReplayStatus::Pending { compared: 0, remaining: 5 });
    pending.advance(5).unwrap();
    let (recovered, _) = pending.finish().unwrap(); equivalent(&original, &recovered);
    assert_eq!(independent.generation().position(), 0);
}

#[test]
fn file_shape_errors_and_recomputation_mismatches_return_no_owner() {
    let root = Directory::new(); let path = root.0.join("checkpoint.bin");
    let (original, bytes) = framed(); let independent = blueprint();
    assert!(matches!(independent.read_archive_file(&root.0, ArchiveLimits::default()), Err(ArchiveFileError::NotRegular)));
    fs::write(&path, &bytes[..bytes.len() - 1]).unwrap();
    assert!(matches!(independent.replay_file(&path, ArchiveLimits::default(), ReplayBudget::default()),
        Err(ArchiveFileError::Format(_))));
    fs::write(&path, &bytes).unwrap();
    assert!(matches!(independent.read_archive_file(&path, ArchiveLimits { bytes: bytes.len() - 1, ..ArchiveLimits::default() }),
        Err(ArchiveFileError::Format(Error::Limit))));
    let mut changed = bytes.clone(); *changed.last_mut().unwrap() ^= 1;
    fs::write(&path, changed).unwrap();
    assert!(matches!(independent.replay_file(&path, ArchiveLimits::default(), ReplayBudget::default()),
        Err(ArchiveFileError::Replay(Error::Binding))));
    fs::write(&path, bytes).unwrap();
    let (recovered, _) = independent.replay_file(&path, ArchiveLimits::default(), ReplayBudget::default()).unwrap();
    equivalent(&original, &recovered); assert_eq!(independent.generation().position(), 0);
    #[cfg(unix)] {
        let link = root.0.join("link.bin"); std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(matches!(independent.read_archive_file(&link, ArchiveLimits::default()), Err(ArchiveFileError::NotRegular)));
        let before = fs::read(&path).unwrap();
        assert!(original.checkpoint(CheckpointLimits::default()).unwrap().save_archive_new(&link, ArchiveLimits::default()).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}

#[test]
fn replay_in_fresh_process_child() {
    let Some(path) = std::env::var_os("FA_LEARNED_ARCHIVE_INPUT") else { return; };
    let output = std::env::var_os("FA_LEARNED_ARCHIVE_OUTPUT").expect("child output path");
    let independent = blueprint();
    let (mut recovered, receipt) = independent.replay_file(PathBuf::from(path), ArchiveLimits::default(), ReplayBudget::default()).unwrap();
    assert_eq!(receipt.positions, 5); assert_eq!(receipt.sampled_positions, 2);
    assert_eq!(recovered.generation().telemetry_work(), receipt.telemetry_recomputation);
    recovered.run_to_stop().unwrap();
    recovered.checkpoint(CheckpointLimits::default()).unwrap().save_archive_new(PathBuf::from(output), ArchiveLimits::default()).unwrap();
}

#[test]
fn new_process_replays_then_continues_the_exact_original_sampled_stream() {
    let root = Directory::new(); let input = root.0.join("input.bin"); let output = root.0.join("output.bin");
    {
        let (original, _) = framed();
        original.checkpoint(CheckpointLimits::default()).unwrap().save_archive_new(&input, ArchiveLimits::default()).unwrap();
    } // No live original generation, checkpoint or construction recipe remains.
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "files::replay_in_fresh_process_child", "--nocapture"])
        .env_clear().env("FA_LEARNED_ARCHIVE_INPUT", &input).env("FA_LEARNED_ARCHIVE_OUTPUT", &output)
        .output().unwrap();
    assert!(result.status.success(), "child stdout={} stderr={}",
        String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
    let mut continuous = blueprint(); continuous.run_to_stop().unwrap();
    let expected = continuous.checkpoint(CheckpointLimits::default()).unwrap().encode_archive(ArchiveLimits::default()).unwrap();
    assert_eq!(fs::read(output).unwrap(), expected);
}
