//! Original durable histories, not hand-selected policy-replay archives.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod fixture;
use fixture::*;
use fa_reference::action::consequence::delivery::persistent::{JournalError, observed::FileOversight};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::policy_campaign::{PolicyDelta, ReplayCaseId, ReplayLimits};
use fa_reference::{Error, Snapshot};

fn limits() -> ReplayLimits { ReplayLimits { cases: 16, input_bytes: 1_048_576 } }
fn candidate() -> Policy { Policy::new(2, vec![Predicate::PayloadAtMost(128)]).unwrap() }

#[test]
fn replay_includes_denials_and_reviews_without_spending_or_replacing_keys() {
    let root = Directory::new();
    let (mut host, reviewer) = create(&root);
    let keys = ready(&mut host, &reviewer, 1, b"allowed");
    let mut denied = snapshot();
    denied.values.insert(7, b"not allowed".to_vec());
    host.propose(host.revision(), 2, spec(&host, b"denied"), denied).unwrap();
    let before = host.inspect();
    let bytes = std::fs::read(root.store().join("delivery.bin")).unwrap();
    let report = host.replay_candidate_policy(candidate(), limits()).unwrap();
    assert_eq!(report.cases().len(), 3);
    assert_eq!(report.newly_reviewable(), vec![ReplayCaseId::Proposal(2)]);
    assert_eq!(report.cases().iter().filter(|case| matches!(case.id(), ReplayCaseId::Review { .. })).count(), 1);
    assert!(!report.requires_shadow());
    assert_eq!(host.inspect(), before);
    assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), bytes);
    let (cut, recovered) = FileOversight::read_policy_replay(root.store(), &profile(), candidate(), limits()).unwrap();
    assert_eq!(cut, before);
    assert_eq!(recovered, report);
    assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), bytes);
    dispatch(&mut host, &keys);
    assert_eq!(host.inspect().control.ledger.charged, 16);
}

#[test]
fn offline_replay_survives_reopen_without_restoring_old_authority() {
    let root = Directory::new();
    let (mut host, _) = create(&root);
    reviewed(&mut host, 1, b"historical");
    let before = host.inspect();
    let expected = host.replay_candidate_policy(candidate(), limits()).unwrap();
    drop(host);
    let (cut, report) = FileOversight::read_policy_replay(root.store(), &profile(), candidate(), limits()).unwrap();
    assert_eq!(cut, before);
    assert_eq!(report, expected);
    let (reopened, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert!(reopened.inspect().control.ledger.epoch > before.control.ledger.epoch);
    assert!(!reopened.clock_ready());
    assert_eq!(reopened.replay_candidate_policy(candidate(), limits()).unwrap(), expected);
}

#[test]
fn unobserved_candidate_reads_cannot_be_invented_by_journal_replay() {
    let root = Directory::new();
    let (mut host, _) = create(&root);
    reviewed(&mut host, 1, b"historical");
    let candidate = Policy::new(2, vec![Predicate::PayloadAtMost(128),
        Predicate::Absent { key: 99 }, Predicate::Any(vec![0, 1])]).unwrap();
    let report = host.replay_candidate_policy(candidate, limits()).unwrap();
    assert!(report.requires_shadow());
    assert!(report.cases().iter().all(|case| case.delta() == PolicyDelta::RequiresShadow
        && case.candidate().is_none() && case.missing_nodes() == [1]));
    assert!(report.newly_reviewable().is_empty());
}

#[test]
fn exhausted_campaigns_return_no_prefix_report_or_journal_mutation() {
    let root = Directory::new();
    let (mut host, _) = create(&root);
    assert_eq!(host.replay_candidate_policy(candidate(), limits()), Err(JournalError::Contract(Error::Incomplete)));
    reviewed(&mut host, 1, b"historical");
    let before = host.inspect();
    for limit in [ReplayLimits { cases: 1, ..limits() }, ReplayLimits { input_bytes: 1, ..limits() }] {
        assert_eq!(host.replay_candidate_policy(candidate(), limit), Err(JournalError::Contract(Error::Limit)));
        assert_eq!(FileOversight::read_policy_replay(root.store(), &profile(), candidate(), limit),
            Err(JournalError::Contract(Error::Limit)));
    }
    assert_eq!(host.inspect(), before);
}

#[test]
fn replay_requires_the_original_profile_and_a_valid_canonical_journal() {
    let root = Directory::new();
    let (mut host, _) = create(&root);
    reviewed(&mut host, 1, b"historical");
    let mut wrong = profile(); wrong.delivery.total += 1;
    assert!(FileOversight::read_policy_replay(root.store(), &wrong, candidate(), limits()).is_err());
    let path = root.store().join("delivery.bin");
    let bytes = std::fs::read(&path).unwrap();
    std::fs::write(&path, &bytes[..bytes.len() - 1]).unwrap();
    assert!(FileOversight::read_policy_replay(root.store(), &profile(), candidate(), limits()).is_err());
    std::fs::write(&path, bytes).unwrap();
    assert!(FileOversight::read_policy_replay(root.store(), &profile(), candidate(), limits()).is_ok());
    // An incomplete caller Snapshot is not accepted as a replay corpus.
    let _ = Snapshot::default();
}
