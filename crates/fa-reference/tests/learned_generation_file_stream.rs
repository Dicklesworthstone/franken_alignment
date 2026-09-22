//! File adapters exercise the same stream encoder and original learned replay.
#[path = "support/learned_generation_fixture.rs"]
mod fixture;
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{
    monitored::GenerationTelemetryBudget,
    replay::{CheckpointLimits, ReplayBudget, archive::{ArchiveLimits,
        files::{ArchiveFileError, ArchiveFileOperation}}},
};
use fa_reference::Error;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-learned-file-stream-{}-{now}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        let mut builder = fs::DirBuilder::new();
        builder.recursive(false);
        #[cfg(unix)] {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("file stream cleanup: {error}"); }
    }
}

#[test]
fn file_transport_preserves_every_cut_and_replay_survives_path_removal() {
    let root = Directory::new();
    for cut in 0..=8 {
        let mut original = fixture::generation();
        for position in 0..cut { original.advance(position).unwrap(); }
        let checkpoint = original.checkpoint(CheckpointLimits::default()).unwrap();
        let layout = checkpoint.archive_layout(ArchiveLimits::default()).unwrap();
        let limits = ArchiveLimits { bytes: layout.encoded_bytes, recipe_bytes: layout.recipe_bytes,
            state: CheckpointLimits { positions: cut as usize, state_bytes: layout.state_bytes } };
        let path = root.0.join(format!("cut-{cut}.bin"));
        assert_eq!(checkpoint.save_archive_new(&path, limits).unwrap(), layout.encoded_bytes);
        let encoded = checkpoint.encode_archive(limits).unwrap();
        assert_eq!(fs::read(&path).unwrap(), encoded);
        assert!(matches!(checkpoint.save_archive_new(&path, limits),
            Err(ArchiveFileError::Io { operation: ArchiveFileOperation::Create, .. })));
        assert_eq!(fs::read(&path).unwrap(), encoded);
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o077, 0);
        }
        let rejected = root.0.join(format!("over-limit-{cut}.bin"));
        assert_eq!(checkpoint.save_archive_new(&rejected, ArchiveLimits { bytes: layout.encoded_bytes - 1, ..limits }),
            Err(ArchiveFileError::Format(Error::Limit)));
        assert!(!rejected.exists());
        let independent = fixture::generation();
        let mut replay = independent.begin_replay_file(&path, limits, ReplayBudget::default()).unwrap();
        fs::remove_file(path).unwrap();
        replay.advance(cut as usize).unwrap();
        let (mut resumed, receipt) = replay.finish().unwrap();
        assert_eq!(receipt.positions, cut as usize);
        fixture::equivalent(&original, &resumed);
        original.run_to_stop().unwrap();
        resumed.run_to_stop().unwrap();
        fixture::equivalent(&original, &resumed);
        assert_eq!(independent.generation().position(), 0);
    }
}

#[test]
fn file_transport_rejects_truncated_wrong_and_overlong_archives_without_advancing() {
    let root = Directory::new();
    let path = root.0.join("input.bin");
    let mut original = fixture::generation();
    for position in 0..5 { original.advance(position).unwrap(); }
    let bytes = original.checkpoint(CheckpointLimits::default()).unwrap().encode_archive(ArchiveLimits::default()).unwrap();
    let independent = fixture::generation();
    for cut in (0..=24).chain([bytes.len() / 2, bytes.len() - 1]) {
        fs::write(&path, &bytes[..cut]).unwrap();
        assert!(matches!(independent.read_archive_file(&path, ArchiveLimits::default()),
            Err(ArchiveFileError::Format(_))));
    }
    let mut extra = bytes.clone(); extra.push(0);
    fs::write(&path, extra).unwrap();
    assert!(matches!(independent.read_archive_file(&path, ArchiveLimits::default()),
        Err(ArchiveFileError::Format(Error::Binding))));
    fs::write(&path, &bytes).unwrap();
    let different = fixture::generation_with(174, GenerationTelemetryBudget::default());
    assert!(matches!(different.read_archive_file(&path, ArchiveLimits::default()),
        Err(ArchiveFileError::Format(Error::Binding))));
    assert!(matches!(independent.read_archive_file(&path, ArchiveLimits { bytes: bytes.len() - 1, ..ArchiveLimits::default() }),
        Err(ArchiveFileError::Format(Error::Limit))));
    let (resumed, _) = independent.replay_file(&path, ArchiveLimits::default(), ReplayBudget::default()).unwrap();
    fixture::equivalent(&original, &resumed);
    assert_eq!(independent.generation().position(), 0);
    assert_eq!(different.generation().position(), 0);
    assert!(matches!(independent.read_archive_file(&root.0, ArchiveLimits::default()), Err(ArchiveFileError::NotRegular)));
    #[cfg(unix)] {
        let link = root.0.join("link.bin");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(matches!(independent.read_archive_file(&link, ArchiveLimits::default()), Err(ArchiveFileError::NotRegular)));
        assert!(original.checkpoint(CheckpointLimits::default()).unwrap().save_archive_new(&link, ArchiveLimits::default()).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn file_replay_preserves_a_later_hold_and_repeats_the_original_alarm() {
    let root = Directory::new();
    let path = root.0.join("before-alarm.bin");
    let mut original = fixture::controlled_generation(true);
    original.advance(0).unwrap();
    original.checkpoint(CheckpointLimits::default()).unwrap().save_archive_new(&path, ArchiveLimits::default()).unwrap();
    let held = original.advance(1).unwrap();
    assert!(held.accepted().is_none());
    assert!(held.sample().is_none());
    let status = original.generation().status();
    let (mut resumed, _) = original.replay_file(&path, ArchiveLimits::default(), ReplayBudget::default()).unwrap();
    assert_eq!(original.generation().status(), status);
    let again = resumed.advance(1).unwrap();
    assert_eq!(again.status(), held.status());
    assert!(again.accepted().is_none());
    assert!(again.sample().is_none());
    fixture::equivalent(&original, &resumed);
    assert!(matches!(original.checkpoint(CheckpointLimits::default()), Err(Error::WrongState)));
    fs::write(&path, b"truncated").unwrap();
    assert!(original.replay_file(&path, ArchiveLimits::default(), ReplayBudget::default()).is_err());
    assert_eq!(original.generation().status(), status);
}
