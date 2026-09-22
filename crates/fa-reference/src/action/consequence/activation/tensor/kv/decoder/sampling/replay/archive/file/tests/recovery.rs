//! The stepwise opener uses the same real generator, lock, files and verifier.
use super::*;
use super::super::super::super::{CheckpointLimits, ReplayStatus};

#[test]
fn bounded_recovery_keeps_one_lock_and_never_exposes_partial_state() {
    for quantum in [1, 2, usize::MAX] {
        let root = Directory::new(); let mut host = create(&root); let mut oracle = quiet();
        for position in 0..3 { host.advance(position, position).unwrap(); oracle.advance(position).unwrap(); }
        let floor = host.last_commit().floor(); drop(host); pending(&root); let bytes = root.bytes();
        let mut recovery = FileGeneration::begin_open(&root.0, &quiet(), ArchiveLimits::default(), ReplayBudget::default(), floor).unwrap();
        assert_eq!(recovery.advance(0).unwrap(), ReplayStatus::Pending { compared: 0, remaining: 3 });
        assert!(recovery.receipt().is_none());
        while recovery.status() != ReplayStatus::Verified {
            assert!(matches!(FileGeneration::open(&root.0, &quiet(), ArchiveLimits::default(), ReplayBudget::default(), floor), Err(GenerationFileError::Busy)));
            recovery.advance(quantum).unwrap();
            assert_eq!(root.bytes(), bytes); assert!(root.0.join(storage::PENDING).exists());
        }
        let receipt = *recovery.receipt().unwrap();
        assert_eq!(receipt.positions, 3);
        assert_eq!(recovery.advance(usize::MAX).unwrap(), ReplayStatus::Verified);
        assert_eq!(recovery.receipt(), Some(&receipt));
        let (mut host, finished) = recovery.finish().unwrap(); assert_eq!(finished, receipt);
        same(host.generation().unwrap(), oracle.generation());
        assert!(!root.0.join(storage::PENDING).exists()); assert_eq!(root.bytes(), bytes);
        host.advance(3, 3).unwrap(); oracle.advance(3).unwrap();
        same(host.generation().unwrap(), oracle.generation());
    }
}

#[test]
fn abandoned_or_premature_recovery_does_not_clean_or_publish_a_candidate() {
    let root = Directory::new(); let mut host = create(&root); host.advance(0, 0).unwrap(); drop(host);
    pending(&root); let bytes = root.bytes();
    let recovery = FileGeneration::begin_open(&root.0, &quiet(), ArchiveLimits::default(), ReplayBudget::default(), ZERO).unwrap();
    assert!(matches!(recovery.finish(), Err(GenerationFileError::Contract(Error::Incomplete))));
    assert_eq!(root.bytes(), bytes); assert!(root.0.join(storage::PENDING).exists());
    let recovery = FileGeneration::begin_open(&root.0, &quiet(), ArchiveLimits::default(), ReplayBudget::default(), ZERO).unwrap();
    drop(recovery);
    assert_eq!(root.bytes(), bytes); assert!(root.0.join(storage::PENDING).exists());
    let (host, receipt) = FileGeneration::open(&root.0, &quiet(), ArchiveLimits::default(), ReplayBudget::default(), ZERO).unwrap();
    assert_eq!(receipt.positions, 1); assert_eq!(host.last_commit().position, 1);
}

#[test]
fn even_an_empty_prefix_requires_verification_before_returning_its_owner() {
    let root = Directory::new(); let host = create(&root); drop(host);
    let recovery = FileGeneration::begin_open(&root.0, &quiet(), ArchiveLimits::default(), ReplayBudget::default(), ZERO).unwrap();
    assert_eq!(recovery.status(), ReplayStatus::Pending { compared: 0, remaining: 0 });
    assert!(recovery.receipt().is_none());
    assert!(matches!(recovery.finish(), Err(GenerationFileError::Contract(Error::Incomplete))));
    let mut recovery = FileGeneration::begin_open(&root.0, &quiet(), ArchiveLimits::default(), ReplayBudget::default(), ZERO).unwrap();
    assert_eq!(recovery.advance(0).unwrap(), ReplayStatus::Verified);
    let (mut host, receipt) = recovery.finish().unwrap(); assert_eq!(receipt.positions, 0);
    host.advance(0, 0).unwrap(); assert_eq!(host.last_commit().position, 1);
}

#[test]
fn a_changed_file_during_replay_never_becomes_a_successful_recovered_owner() {
    let root = Directory::new(); let mut host = create(&root); host.advance(0, 0).unwrap(); drop(host);
    pending(&root); let bytes = root.bytes();
    let mut recovery = FileGeneration::begin_open(&root.0, &quiet(), ArchiveLimits::default(), ReplayBudget::default(), ZERO).unwrap();
    assert_eq!(recovery.advance(1).unwrap(), ReplayStatus::Verified);
    // Deliberate noncooperating mutation: passing an old in-memory replay is not
    // permission to acknowledge different canonical bytes or remove staging.
    let mut changed = bytes.clone(); *changed.last_mut().unwrap() ^= 1;
    fs::write(root.0.join(storage::CANONICAL), &changed).unwrap();
    assert!(matches!(recovery.finish(), Err(GenerationFileError::Contract(Error::Binding))));
    assert_eq!(root.bytes(), changed); assert!(root.0.join(storage::PENDING).exists());
    fs::write(root.0.join(storage::CANONICAL), &bytes).unwrap();
    let (host, _) = FileGeneration::open(&root.0, &quiet(), ArchiveLimits::default(), ReplayBudget::default(), ZERO).unwrap();
    assert_eq!(host.last_commit().position, 1); assert!(!root.0.join(storage::PENDING).exists());
}

#[test]
fn whole_replay_bounds_are_checked_once_and_not_refilled_by_small_quanta() {
    let root = Directory::new(); let mut host = create(&root); let mut oracle = quiet();
    for position in 0..3 { host.advance(position, position).unwrap(); oracle.advance(position).unwrap(); }
    drop(host); pending(&root); let bytes = root.bytes();
    let checkpoint = oracle.checkpoint(CheckpointLimits::default()).unwrap();
    let exact = ReplayBudget { positions: 3, state_bytes: checkpoint.state_bytes(),
        decoder_products: checkpoint.work().reserved_decoder_products,
        vocabulary_scores: checkpoint.work().reserved_vocabulary_scores };
    for limited in [ReplayBudget { positions: 2, ..exact },
        ReplayBudget { state_bytes: exact.state_bytes - 1, ..exact },
        ReplayBudget { decoder_products: exact.decoder_products - 1, ..exact },
        ReplayBudget { vocabulary_scores: exact.vocabulary_scores - 1, ..exact }] {
        assert!(matches!(FileGeneration::begin_open(&root.0, &quiet(), ArchiveLimits::default(), limited, ZERO), Err(GenerationFileError::Contract(Error::Limit))));
        assert_eq!(root.bytes(), bytes); assert!(root.0.join(storage::PENDING).exists());
    }
    let mut recovery = FileGeneration::begin_open(&root.0, &quiet(), ArchiveLimits::default(), exact, ZERO).unwrap();
    for _ in 0..3 { recovery.advance(1).unwrap(); }
    let (host, receipt) = recovery.finish().unwrap();
    assert_eq!(receipt.recomputation, oracle.generation().work());
    assert_eq!(receipt.telemetry_recomputation, oracle.generation().telemetry_work());
    same(host.generation().unwrap(), oracle.generation());
}
