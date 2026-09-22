//! Public stream boundaries; these tests run the original learned generator.
#[path = "support/learned_generation_fixture.rs"]
mod fixture;
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{
    monitored::GenerationTelemetryBudget,
    replay::{
        CheckpointLimits, ReplayBudget,
        archive::{ArchiveLimits, GenerationArchive, MAX_RECIPE_BYTES, stream::ArchiveIoError},
    },
};
use fa_reference::Error;
use std::io::{self, Cursor, Read, Write};

#[derive(Default)]
struct ChoppedWriter { bytes: Vec<u8>, flushes: usize }
impl Write for ChoppedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = bytes.len().min(3);
        self.bytes.extend_from_slice(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> { self.flushes += 1; Ok(()) }
}
struct ChoppedReader<'a> { inner: Cursor<&'a [u8]>, calls: usize }
impl Read for ChoppedReader<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.calls += 1;
        if self.calls.is_multiple_of(7) { return Err(io::ErrorKind::Interrupted.into()); }
        let count = bytes.len().min(3);
        self.inner.read(&mut bytes[..count])
    }
}

#[test]
fn every_cut_streams_identical_bytes_and_resumes_original_generation() {
    let limits = ArchiveLimits::default();
    for cut in 0..=8 {
        let mut original = fixture::generation();
        for position in 0..cut { original.advance(position).unwrap(); }
        let checkpoint = original.checkpoint(CheckpointLimits::default()).unwrap();
        let expected_bytes = checkpoint.encode_archive(limits).unwrap();
        let mut output = ChoppedWriter::default();
        let written = checkpoint.write_archive_to(&mut output, limits).unwrap();
        assert_eq!(output.bytes, expected_bytes);
        assert_eq!(output.flushes, 1);
        assert_eq!(written.encoded_bytes, output.bytes.len());
        assert_eq!(written.positions, cut as usize);
        drop(checkpoint);
        // No checkpoint or recipe pointer from the original is supplied here.
        let intended = fixture::generation();
        let mut input = ChoppedReader { inner: Cursor::new(output.bytes.as_slice()), calls: 0 };
        let parsed = GenerationArchive::read_archive_from(&mut input, &intended, limits).unwrap();
        assert_eq!(parsed.positions(), cut as usize);
        assert_eq!(parsed.recipe_bytes(), written.recipe_bytes);
        assert_eq!(input.inner.position(), output.bytes.len() as u64);
        assert_eq!(intended.generation().position(), 0);
        let (mut resumed, receipt) = parsed.replay(ReplayBudget::default()).unwrap();
        assert_eq!(receipt.positions, cut as usize);
        fixture::equivalent(&original, &resumed);
        original.run_to_stop().unwrap();
        resumed.run_to_stop().unwrap();
        fixture::equivalent(&original, &resumed);
    }
}

#[test]
fn exact_caps_work_and_each_smaller_export_refuses_before_writing() {
    let mut run = fixture::generation();
    for position in 0..5 { run.advance(position).unwrap(); }
    let checkpoint = run.checkpoint(CheckpointLimits::default()).unwrap();
    let mut bytes = Vec::new();
    let written = checkpoint.write_archive_to(&mut bytes, ArchiveLimits::default()).unwrap();
    let limits = ArchiveLimits { bytes: written.encoded_bytes, recipe_bytes: written.recipe_bytes,
        state: CheckpointLimits { positions: 5, state_bytes: written.state_bytes } };
    assert_eq!(checkpoint.encode_archive(limits).unwrap(), bytes);
    let parsed = GenerationArchive::read_archive_from(&mut bytes.as_slice(), &run, limits).unwrap();
    let (restored, _) = parsed.replay(ReplayBudget::default()).unwrap();
    fixture::equivalent(&run, &restored);
    for smaller in [
        ArchiveLimits { bytes: limits.bytes - 1, ..limits },
        ArchiveLimits { recipe_bytes: limits.recipe_bytes - 1, ..limits },
        ArchiveLimits { state: CheckpointLimits { positions: 4, ..limits.state }, ..limits },
        ArchiveLimits { state: CheckpointLimits { state_bytes: limits.state.state_bytes - 1, ..limits.state }, ..limits },
    ] {
        let mut output = Vec::new();
        assert!(matches!(checkpoint.write_archive_to(&mut output, smaller), Err(ArchiveIoError::Refused(Error::Limit))));
        assert!(output.is_empty());
        assert!(matches!(GenerationArchive::read_archive_from(&mut bytes.as_slice(), &run, smaller),
            Err(ArchiveIoError::Refused(Error::Limit))));
    }
}

#[test]
fn inadmissible_header_sizes_consume_no_body_and_invalid_limits_consume_nothing() {
    let run = fixture::generation();
    let bytes = run.checkpoint(CheckpointLimits::default()).unwrap().encode_archive(ArchiveLimits::default()).unwrap();
    let mut huge = bytes.clone();
    huge[8..16].copy_from_slice(&((MAX_RECIPE_BYTES as u64) + 1).to_be_bytes());
    let mut input = Cursor::new(huge);
    assert!(matches!(GenerationArchive::read_archive_from(&mut input, &run, ArchiveLimits::default()),
        Err(ArchiveIoError::Refused(Error::Limit))));
    assert_eq!(input.position(), 24);

    let mut input = Cursor::new(bytes.as_slice());
    let tight = ArchiveLimits { bytes: bytes.len() - 1, ..ArchiveLimits::default() };
    assert!(matches!(GenerationArchive::read_archive_from(&mut input, &run, tight),
        Err(ArchiveIoError::Refused(Error::Limit))));
    assert_eq!(input.position(), 24);
    let mut input = Cursor::new(bytes.as_slice());
    let tiny = ArchiveLimits { bytes: 23, ..ArchiveLimits::default() };
    assert!(matches!(GenerationArchive::read_archive_from(&mut input, &run, tiny),
        Err(ArchiveIoError::Refused(Error::Limit))));
    assert_eq!(input.position(), 0);

    let mut shorter_recipe = bytes;
    let count = u64::from_be_bytes(shorter_recipe[8..16].try_into().unwrap());
    shorter_recipe[8..16].copy_from_slice(&(count - 1).to_be_bytes());
    let mut input = Cursor::new(shorter_recipe);
    assert!(matches!(GenerationArchive::read_archive_from(&mut input, &run, ArchiveLimits::default()),
        Err(ArchiveIoError::Refused(Error::Binding))));
    assert_eq!(input.position(), 24);
}

#[test]
fn truncation_trailing_bytes_and_an_independently_different_recipe_never_admit() {
    let run = fixture::generation();
    let bytes = run.checkpoint(CheckpointLimits::default()).unwrap().encode_archive(ArchiveLimits::default()).unwrap();
    for cut in (0..=24).chain([bytes.len() / 2, bytes.len() - 1]) {
        assert!(GenerationArchive::read_archive_from(&mut &bytes[..cut], &run, ArchiveLimits::default()).is_err());
    }
    let mut extra = bytes.clone(); extra.push(0);
    assert!(matches!(GenerationArchive::read_archive_from(&mut extra.as_slice(), &run, ArchiveLimits::default()),
        Err(ArchiveIoError::Refused(Error::Binding))));
    let changed = fixture::generation_with(174, GenerationTelemetryBudget::default());
    assert!(matches!(GenerationArchive::read_archive_from(&mut bytes.as_slice(), &changed, ArchiveLimits::default()),
        Err(ArchiveIoError::Refused(Error::Binding))));
    // Unchanged, same-length control still imports and verifies.
    let parsed = GenerationArchive::read_archive_from(&mut bytes.as_slice(), &run, ArchiveLimits::default()).unwrap();
    fixture::equivalent(&run, &parsed.replay(ReplayBudget::default()).unwrap().0);
}

struct BrokenWriter { remaining: usize, flush_failure: bool }
impl Write for BrokenWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 { return Err(io::Error::other("injected-write")); }
        let written = self.remaining.min(bytes.len());
        self.remaining -= written;
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.flush_failure { Err(io::Error::other("injected-flush")) } else { Ok(()) }
    }
}
struct MissingEof<'a>(&'a [u8]);
impl Read for MissingEof<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if self.0.is_empty() { return Err(io::ErrorKind::WouldBlock.into()); }
        self.0.read(bytes)
    }
}
#[test]
fn transport_errors_keep_their_original_cause_and_would_block_is_not_eof() {
    let run = fixture::generation();
    let checkpoint = run.checkpoint(CheckpointLimits::default()).unwrap();
    for (remaining, flush_failure, message) in [(20, false, "injected-write"), (usize::MAX, true, "injected-flush")] {
        let error = checkpoint.write_archive_to(&mut BrokenWriter { remaining, flush_failure }, ArchiveLimits::default()).unwrap_err();
        assert!(matches!(error, ArchiveIoError::Io(ref cause) if cause.to_string() == message));
    }
    let bytes = checkpoint.encode_archive(ArchiveLimits::default()).unwrap();
    let error = GenerationArchive::read_archive_from(&mut MissingEof(&bytes), &run, ArchiveLimits::default()).unwrap_err();
    assert!(matches!(error, ArchiveIoError::Io(ref cause) if cause.kind() == io::ErrorKind::WouldBlock));
    fixture::equivalent(&run, &fixture::generation());
}

#[test]
fn tampered_streamed_state_remains_comparison_material_and_fails_replay() {
    let mut run = fixture::generation();
    for position in 0..5 { run.advance(position).unwrap(); }
    let mut bytes = run.checkpoint(CheckpointLimits::default()).unwrap().encode_archive(ArchiveLimits::default()).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    let intended = fixture::generation();
    let parsed = GenerationArchive::read_archive_from(&mut bytes.as_slice(), &intended, ArchiveLimits::default()).unwrap();
    // An opaque saved cache may parse, but it is NEVER installed in the candidate.
    assert!(matches!(parsed.replay(ReplayBudget::default()), Err(Error::Binding)));
    assert_eq!(intended.generation().position(), 0);
    assert_eq!(run.generation().position(), 5);
    let valid = run.checkpoint(CheckpointLimits::default()).unwrap().encode_archive(ArchiveLimits::default()).unwrap();
    let parsed = GenerationArchive::read_archive_from(&mut valid.as_slice(), &intended, ArchiveLimits::default()).unwrap();
    fixture::equivalent(&run, &parsed.replay(ReplayBudget::default()).unwrap().0);
}

#[test]
fn streamed_recovery_cannot_clear_the_original_hold_or_skip_the_next_alarm() {
    let mut original = fixture::controlled_generation(true);
    original.advance(0).unwrap();
    let mut bytes = Vec::new();
    original.checkpoint(CheckpointLimits::default()).unwrap()
        .write_archive_to(&mut bytes, ArchiveLimits::default()).unwrap();
    let held = original.advance(1).unwrap();
    assert!(held.accepted().is_none());
    assert!(held.sample().is_none());
    let old_status = original.generation().status();
    assert!(matches!(original.checkpoint(CheckpointLimits::default()), Err(Error::WrongState)));
    let archive = GenerationArchive::read_archive_from(&mut bytes.as_slice(), &original, ArchiveLimits::default()).unwrap();
    assert_eq!(original.generation().status(), old_status);
    let (mut resumed, _) = archive.replay(ReplayBudget::default()).unwrap();
    let again = resumed.advance(1).unwrap();
    assert_eq!(again.status(), held.status());
    assert!(again.accepted().is_none());
    assert!(again.sample().is_none());
    fixture::equivalent(&original, &resumed);
}
