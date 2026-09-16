//! Durable original-review export: exact views, application refusal, and outcome cut.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod fixture;
use fixture::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::delivery::{EndpointOutcome, persistent::{JournalError, PolicyUpdate}};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight,
    helpers::FileHelperLaunch};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::{CommitteeInput, helper_workers::HelperLimits};
use fa_reference::action::consequence::oversight::replay::ObservedReviewAnchor;
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::collections::BTreeMap;
use std::os::unix::net::UnixStream;

fn begin(host: &mut FileOversight) -> (CommitteeInput, ObservedReviewAnchor) {
    let action = host.propose(host.revision(), 1, spec(host, b"publish"), snapshot()).unwrap();
    let input = inputs(&action, b"every input byte is retained");
    host.record_inputs(host.revision(), 1, 0, input.clone()).unwrap();
    host.begin_review(host.revision(), 1, 101, ROOT, window(host), snapshot()).unwrap();
    (input, host.review_anchor(101).unwrap())
}

#[test]
fn durable_export_preserves_original_keys_and_links_review_to_real_publication() {
    let root = Directory::new(); let (mut host, reviewer) = create(&root);
    let (input, anchor) = begin(&mut host);
    let begun = host.revision();
    votes(&mut host, 101, Verdict::Allow);
    let receipt = host.finish_review(host.revision(), 101, Some(&input), snapshot()).unwrap().unwrap();
    let completed = host.inspect();
    let replay = host.review_replay(101).unwrap();
    assert_eq!(replay.anchor(), &anchor);
    assert_eq!(replay.begin_revision(), begun);
    assert_eq!(replay.application(), &Ok(receipt.clone()));
    assert_eq!(replay.completed_snapshot(), &completed);
    assert_eq!(replay.application_inputs(), Some(&input));
    assert_eq!(replay.application_snapshot(), &snapshot());
    replay.archive().verify_receipt(&anchor, &receipt).unwrap();
    assert_eq!(host.inspect(), completed);
    let automatic = host.authorize(host.revision(), 1, &input, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), 1001, 1, &input, ElapsedTick(30)).unwrap();
    let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, input.action(), &input, snapshot()).unwrap();
    assert_eq!(host.publish(host.revision(), 1).unwrap(), EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 1).unwrap();
    let before_read = host.inspect();
    let later = FileOversight::read_review_replay(root.store(), &profile(), 101).unwrap();
    assert_eq!(later.archive(), replay.archive());
    assert_eq!(later.completed_snapshot(), &completed);
    assert_eq!(later.journal_snapshot(), &before_read);
    assert_eq!(later.journal_snapshot().executions, 1);
    assert_eq!(later.journal_snapshot().control.ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(host.inspect(), before_read);
}

#[test]
fn read_only_export_does_not_lock_fence_clean_staging_or_change_the_canonical_image() {
    let root = Directory::new(); let (mut host, _) = create(&root);
    let (input, anchor) = begin(&mut host); votes(&mut host, 101, Verdict::Allow);
    host.finish_review(host.revision(), 101, Some(&input), snapshot()).unwrap().unwrap();
    let before = host.inspect(); let path = root.store().join("delivery.bin");
    let bytes = std::fs::read(&path).unwrap();
    let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"not authoritative").unwrap();
    let replay = FileOversight::read_review_replay(root.store(), &profile(), 101).unwrap();
    replay.archive().verify(&anchor).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(std::fs::read(&pending).unwrap(), b"not authoritative");
    assert_eq!(host.inspect(), before);
    // Only the test removes its own sentinel, then the SAME live owner continues.
    std::fs::remove_file(&pending).unwrap();
    assert!(host.authorize(host.revision(), 1, &input, snapshot()).is_ok());
}

#[test]
fn committed_stale_application_is_not_rewritten_as_an_applied_allow() {
    let root = Directory::new(); let (mut host, _) = create(&root);
    let (input, anchor) = begin(&mut host); votes(&mut host, 101, Verdict::Allow);
    let changed = inputs(input.action(), b"a valid but changed actual helper packet");
    assert_eq!(host.finish_review(host.revision(), 101, Some(&changed), snapshot()).unwrap(), Err(Error::Stale));
    let before = host.inspect();
    let replay = host.review_replay(101).unwrap();
    assert_eq!(replay.application(), &Err(Error::Stale));
    assert_eq!(replay.application_inputs(), Some(&changed));
    assert_eq!(replay.archive().inputs.as_ref(), &input);
    assert_eq!(replay.archive().verify(&anchor).unwrap().decision().consequence, Consequence::Continue);
    assert!(host.finish_review(host.revision(), 101, Some(&input), snapshot()).is_err());
    assert_eq!(host.inspect(), before);
    assert_eq!(before.control.ledger.reserved, 0); assert_eq!(before.executions, 0);
}

#[test]
fn unknown_and_unfinished_rounds_refuse_without_finishing_or_replacing_them() {
    let root = Directory::new(); let (mut host, _) = create(&root);
    let (input, anchor) = begin(&mut host); let before = host.inspect();
    assert_eq!(host.review_replay(999).unwrap_err(), JournalError::Contract(Error::Missing));
    assert_eq!(host.review_replay(101).unwrap_err(), JournalError::Contract(Error::Incomplete));
    assert_eq!(host.review_replay(0).unwrap_err(), JournalError::Contract(Error::InvalidInput));
    assert_eq!(host.review_anchor(101).unwrap(), anchor); assert_eq!(host.inspect(), before);
    votes(&mut host, 101, Verdict::Allow);
    host.finish_review(host.revision(), 101, Some(&input), snapshot()).unwrap().unwrap();
    assert!(host.review_replay(101).is_ok());
}

#[test]
fn whole_process_reopen_does_not_erase_a_completed_archive_or_restore_its_keys() {
    let root = Directory::new(); let (mut host, _) = create(&root);
    let (input, anchor) = begin(&mut host); votes(&mut host, 101, Verdict::Allow);
    host.finish_review(host.revision(), 101, Some(&input), snapshot()).unwrap().unwrap();
    let old = host.review_replay(101).unwrap(); drop(host);
    let (reopened, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert!(!reopened.clock_ready());
    let recovered = reopened.review_replay(101).unwrap();
    assert_eq!(recovered.archive(), old.archive());
    assert_eq!(recovered.application(), old.application());
    recovered.archive().verify(&anchor).unwrap();
    assert_eq!(recovered.journal_snapshot().control.ledger.stages[&1], ActionState::Cancelled);
    assert!(reopened.review_anchor(101).is_err());
}

#[test]
fn later_policy_change_does_not_rebase_the_historical_review() {
    let root = Directory::new(); let (mut host, _) = create(&root);
    let (input, anchor) = begin(&mut host); votes(&mut host, 101, Verdict::Allow);
    host.finish_review(host.revision(), 101, Some(&input), snapshot()).unwrap().unwrap();
    let original = host.review_replay(101).unwrap();
    let state = host.inspect().control;
    let update = PolicyUpdate::new(42, state.sequence, state.ledger.epoch,
        Policy::new(2, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap()).unwrap();
    host.replace_policy(host.revision(), &update).unwrap();
    let replay = host.review_replay(101).unwrap();
    assert_eq!(replay.archive(), original.archive());
    assert_eq!(replay.archive().policy.anchor.policy.generation(), 1);
    assert_eq!(host.current_policy().unwrap().generation(), 2);
    assert!(replay.journal_snapshot().revision > replay.completed_snapshot().revision);
    replay.archive().verify(&anchor).unwrap();
}

#[test]
fn corrupted_suffix_wrong_profile_and_another_store_cannot_substitute_the_expected_history() {
    let root = Directory::new(); let (mut host, _) = create(&root);
    let (input, anchor) = begin(&mut host); votes(&mut host, 101, Verdict::Allow);
    host.finish_review(host.revision(), 101, Some(&input), snapshot()).unwrap().unwrap();
    let mut wrong = profile(); wrong.delivery.total += 1;
    assert!(FileOversight::read_review_replay(root.store(), &wrong, 101).is_err());
    let other_root = Directory::new(); let (mut other, _) = create(&other_root);
    reviewed(&mut other, 1, b"different");
    let foreign = other.review_replay(101).unwrap();
    assert_eq!(foreign.archive().verify(&anchor), Err(Error::Binding));
    // A selected valid review prefix does not justify ignoring malformed suffix bytes.
    let path = root.store().join("delivery.bin"); let mut bytes = std::fs::read(&path).unwrap();
    bytes.push(0); std::fs::write(&path, bytes).unwrap();
    assert!(FileOversight::read_review_replay(root.store(), &profile(), 101).is_err());
}

#[test]
fn real_helper_socket_timeout_exports_missing_votes_without_a_manual_fallback() {
    let root = Directory::new(); let (mut host, _) = create(&root);
    let action = host.propose(host.revision(), 1, spec(&host, b"publish"), snapshot()).unwrap();
    let input = inputs(&action, b"native worker input");
    host.record_inputs(host.revision(), 1, 0, input).unwrap();
    let mut peers = Vec::new(); let mut streams = BTreeMap::new();
    for member in MEMBERS {
        let (local, peer) = UnixStream::pair().unwrap(); peers.push(peer); streams.insert(member.to_owned(), local);
    }
    let window = window(&host);
    let mut pool = host.begin_helper_review(host.revision(), FileHelperLaunch {
        attempt: 1, round: 101, evidence_root: ROOT, window,
        expected_input_revision: host.input_revision(1).unwrap(), streams, limits: HelperLimits::default(),
    }, snapshot()).unwrap();
    let anchor = host.review_anchor(101).unwrap();
    pool.pump(&mut host, ElapsedTick(1)).unwrap();
    let receipt = pool.finish(&mut host, window.reveal_by, None, snapshot()).unwrap().unwrap();
    assert!(pool.is_closed());
    let replay = host.review_replay(101).unwrap();
    assert_eq!(replay.archive().verify_receipt(&anchor, &receipt).unwrap().missing().len(), MEMBERS.len());
    assert!(replay.archive().commit_times.is_empty()); assert!(replay.archive().reveal_times.is_empty());
    assert_eq!(receipt.policy.control.decision.consequence, Consequence::HoldEffect);
    assert_eq!(host.inspect().executions, 0); drop(peers);
}
