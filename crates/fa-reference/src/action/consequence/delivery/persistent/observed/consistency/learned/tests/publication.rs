//! Same-cut inspection after an ambiguous ORIGINAL two-key publication.
use super::*;

#[test]
fn ambiguous_publication_is_readable_without_reopening_or_repeating_the_effect() {
    for stage in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new();
        let (checked, _) = source([1.0, 1.0], ResidualRetention::All, 1);
        let config = configuration(&checked, vec![0.0, 1.0], LearnedMonitorBudget::default());
        let (mut host, reviewer, observer) = owner(&root, config.clone());
        let report = predict(&mut host, &observer, &checked).unwrap().unwrap();
        report.prediction().unwrap();
        let spec = proposal(&host);
        let action = host.propose_consistent(host.revision(), 1, spec, snapshot()).unwrap().unwrap();
        let input = review(&mut host, &action);
        let automatic = host.authorize(host.revision(), 1, &input, snapshot()).unwrap();
        let request = host.request_human_approval(host.revision(), 201, 1, &input, ElapsedTick(40)).unwrap();
        let revision = host.revision();
        let human = reviewer.approve(&mut host, revision, &request).unwrap();
        let before = host.inspect();
        let sources = disk_sources(&root, &checked);
        host.store.fail_once(stage);
        let result = host.complete_checked_publication(host.revision(), CheckedCompletion {
            automatic: &automatic, human: &human, action: &action, current: &input,
            snapshot: snapshot(), now: ElapsedTick(2),
        });
        let JournalError::Io(failure) = result.unwrap_err() else { panic!("missing Store failure"); };
        assert_eq!(failure.operation, stage);
        assert_eq!(host.inspect(), before);
        assert!(host.storage_failure().is_some());
        let canonical = host.store.read(host.profile.delivery.limits.bytes).unwrap();
        // Keep the poisoned owner and its exclusive lock alive throughout reads.
        let disk = FileOversight::read_publication_with_learned_action_consistency(
            root.store(), &profile(), &config, &sources).unwrap();
        let visible = stage == JournalIo::DirectorySync;
        assert_eq!(disk.publication.revision, before.revision + if visible { 4 } else { 0 });
        assert_eq!(disk.publication.revision, disk.consistency.consistency.journal_revision);
        assert_eq!(disk.publication.executions, if visible { 1 } else { 0 });
        assert_eq!(disk.publication.payload.as_slice(), if visible { &b"risk"[..] } else { &b"initial"[..] });
        assert_eq!(disk.consistency.work, report.work());
        assert_eq!(disk.consistency.consistency.evidence.samples(), 1);
        assert_eq!(disk.pending_request, None);
        assert!(!disk.consistency.consistency.coverage_lost);
        assert_eq!(FileOversight::read_publication_with_learned_action_consistency(
            root.store(), &profile(), &config, &sources).unwrap(), disk);
        assert!(FileOversight::read_publication_with_learned_action_consistency(
            root.store(), &profile(), &config, &BTreeMap::new()).is_err());
        assert_eq!(host.store.read(host.profile.delivery.limits.bytes).unwrap(), canonical);
        drop(host);
        let (mut recovered, _, _) = FileOversight::open_with_learned_action_consistency(
            root.store(), profile(), &config, &sources).unwrap();
        assert_eq!(recovered.inspect().executions, disk.publication.executions);
        assert_eq!(recovered.inspect().control.ledger.charged, disk.publication.control.ledger.charged);
        assert!(!recovered.clock_ready());
        let revision = recovered.revision();
        assert!(recovered.complete_checked_publication(revision, CheckedCompletion {
            automatic: &automatic, human: &human, action: &action, current: &input,
            snapshot: snapshot(), now: ElapsedTick(3),
        }).is_err());
        assert_eq!(recovered.inspect().executions, disk.publication.executions);
    }
}

#[test]
fn a_read_preserves_a_pending_key_without_installing_the_recovery_fence() {
    let root = Directory::new();
    let (checked, _) = source([1.0, 0.0], ResidualRetention::All, 1);
    let config = configuration(&checked, vec![1.0, 0.0], LearnedMonitorBudget::default());
    let (mut host, _, observer) = owner(&root, config.clone());
    let revision = host.revision(); let actor = host.actor_snapshot().unwrap().actor_revision;
    observer.forecast_learned_request(&mut host, revision, 42, actor, &checked, row()).unwrap().unwrap().prediction().unwrap();
    let sources = disk_sources(&root, &checked);
    let canonical = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    let disk = FileOversight::read_publication_with_learned_action_consistency(
        root.store(), &profile(), &config, &sources).unwrap();
    assert_eq!(disk.publication, host.inspect());
    assert_eq!(disk.consistency, host.learned_action_consistency_snapshot().unwrap());
    assert_eq!(disk.pending_request, Some(42));
    assert_eq!(disk.consistency.consistency.pending_attempt, Some(1));
    assert!(!disk.consistency.consistency.coverage_lost);
    assert_eq!(disk.publication.executions, 0);
    assert_eq!(host.store.read(host.profile.delivery.limits.bytes).unwrap(), canonical);
}
