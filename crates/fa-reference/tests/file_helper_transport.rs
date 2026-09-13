#![cfg(unix)]
#[path = "support/file_helper.rs"] mod fixture;
use fixture::*;
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::delivery::{EndpointOutcome, StopRequest};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::helpers::{FileHelperLaunch, FileHelperSetupError};
use fa_reference::action::consequence::oversight::CommitteeInput;
use fa_reference::action::consequence::oversight::helper_workers::{HelperFailure, HelperLimits};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::collections::BTreeMap;
use std::io::Read;
use std::os::unix::net::UnixStream;

#[test]
fn actual_worker_socket_responses_reach_original_two_key_publication_and_recovery() {
    let root = Directory::new();
    let (mut host, reviewer) = create(&root);
    let (action, input) = prepare(&mut host, 1, b"published");
    let cutoffs = window(&host);
    let (mut pool, mut clients) = launch(&mut host, 1, 101, cutoffs);
    assert_eq!(pool.next_deadline(), Some(cutoffs.commit_by));
    assert!(host.authorize(host.revision(), 1, &input, snapshot()).is_err());
    assert_eq!(pool.finish(&mut host, ElapsedTick(1), Some(&input), snapshot()).unwrap_err(), JournalError::Contract(Error::Incomplete));
    assert!(!pool.is_closed());
    complete(&mut pool, &mut host, &mut clients, ElapsedTick(1), Verdict::Allow);
    assert!(pool.statuses().values().all(|status| status.committed && status.revealed));
    let receipt = pool.finish(&mut host, ElapsedTick(1), Some(&input), snapshot()).unwrap().unwrap();
    assert_eq!(receipt.policy.control.decision.consequence, Consequence::Continue);
    assert_eq!(receipt.inputs.as_ref(), &input);
    assert!(pool.is_closed());
    assert_eq!(pool.next_deadline(), None);
    let automatic = host.authorize(host.revision(), 1, &input, snapshot()).unwrap();
    assert_eq!(host.inspect().control.ledger.reserved, 16);
    assert!(host.publish(host.revision(), 1).is_err());
    let request = host.request_human_approval(host.revision(), 1001, 1, &input, ElapsedTick(31)).unwrap();
    let revision = host.revision();
    let human = reviewer.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
    assert_eq!(host.publish(host.revision(), 1).unwrap(), EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.inspect().control.ledger.charged, 16);
    drop(pool); drop(clients); drop(host); drop(reviewer);
    let (mut recovered, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(recovered.inspect().payload, b"published");
    assert_eq!(recovered.inspect().executions, 1);
    assert_eq!(recovered.inspect().control.ledger.stages[&1], ActionState::Unknown);
    recovered.observe_time(recovered.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(recovered.reconcile(recovered.revision(), 1).unwrap(),
        Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }));
    assert_eq!(recovered.inspect().control.ledger.charged, 16);
    assert!(recovered.publish(recovered.revision(), 1).is_err());
}

#[test]
fn each_socket_gets_only_its_exact_original_view_without_private_policy_witnesses() {
    let root = Directory::new();
    let (mut host, _) = create(&root);
    let action = host.propose(host.revision(), 1, spec(&host, b"x"), snapshot()).unwrap();
    let alpha = inputs(&action, b"ALPHA-ONLY-SOURCE");
    let beta = inputs(&action, b"BETA-ONLY-SOURCE");
    let input = CommitteeInput::capture(&action, &profile().committee, BTreeMap::from([
        ("alpha".to_owned(), alpha.views()["alpha"].clone()),
        ("beta".to_owned(), beta.views()["beta"].clone()),
    ])).unwrap();
    host.record_inputs(host.revision(), 1, 0, input.clone()).unwrap();
    let cutoffs = window(&host);
    let (mut pool, mut clients) = launch(&mut host, 1, 101, cutoffs);
    complete(&mut pool, &mut host, &mut clients, ElapsedTick(1), Verdict::Allow);
    for (member, worker) in &clients {
        let captured = worker.input().unwrap();
        assert_eq!(captured.member(), member);
        assert_eq!(captured.round(), 101);
        assert_eq!(captured.actual_input(), input.views()[member].actual_input());
        let other = if member == "alpha" { b"BETA-ONLY-SOURCE".as_slice() } else { b"ALPHA-ONLY-SOURCE".as_slice() };
        assert!(!captured.actual_input().submitted_bytes().windows(other.len()).any(|window| window == other));
    }
    assert!(pool.finish(&mut host, ElapsedTick(1), Some(&input), snapshot()).unwrap().is_ok());
}

#[test]
fn disconnected_helper_remains_missing_until_the_original_reveal_deadline() {
    let root = Directory::new();
    let (mut host, _) = create(&root);
    let (_, input) = prepare(&mut host, 1, b"held");
    let cutoffs = window(&host);
    let (mut pool, mut clients) = launch(&mut host, 1, 101, cutoffs);
    clients.remove("beta");
    for _ in 0..16 {
        pool.pump(&mut host, ElapsedTick(1)).unwrap();
        worker_steps(&mut clients, Verdict::Allow);
    }
    assert!(pool.statuses()["alpha"].committed);
    assert!(!pool.statuses()["beta"].committed);
    assert_eq!(pool.statuses()["beta"].failure, Some(HelperFailure::Disconnected));
    assert!(!pool.ready_to_finish());
    assert_eq!(pool.finish(&mut host, ElapsedTick(1), Some(&input), snapshot()).unwrap_err(), JournalError::Contract(Error::Incomplete));
    for _ in 0..16 {
        pool.pump(&mut host, cutoffs.commit_by).unwrap();
        worker_steps(&mut clients, Verdict::Allow);
    }
    assert!(pool.statuses()["alpha"].revealed);
    assert!(!pool.ready_to_finish());
    let receipt = pool.finish(&mut host, cutoffs.reveal_by, Some(&input), snapshot()).unwrap().unwrap();
    assert_eq!(receipt.policy.control.decision.consequence, Consequence::HoldEffect);
    assert!(host.authorize(host.revision(), 1, &input, snapshot()).is_err());
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn changed_current_input_consumes_the_finished_round_but_a_new_round_can_succeed() {
    let root = Directory::new();
    let (mut host, _) = create(&root);
    let (action, original) = prepare(&mut host, 1, b"updated");
    let cutoffs = window(&host);
    let (mut pool, mut clients) = launch(&mut host, 1, 101, cutoffs);
    complete(&mut pool, &mut host, &mut clients, ElapsedTick(1), Verdict::Allow);
    let changed = inputs(&action, b"new complete observation");
    host.record_inputs(host.revision(), 1, 1, changed.clone()).unwrap();
    let before = host.revision();
    assert_eq!(pool.finish(&mut host, ElapsedTick(1), Some(&changed), snapshot()).unwrap().unwrap_err(), Error::Stale);
    assert_eq!(host.revision(), before + 1);
    assert!(pool.is_closed());
    assert!(pool.finish(&mut host, ElapsedTick(1), Some(&original), snapshot()).is_err());
    assert!(host.begin_review(host.revision(), 1, 101, ROOT, cutoffs, snapshot()).is_err());
    assert!(host.authorize(host.revision(), 1, &changed, snapshot()).is_err());
    let (mut fresh, mut peers) = launch(&mut host, 1, 102, cutoffs);
    complete(&mut fresh, &mut host, &mut peers, ElapsedTick(1), Verdict::Allow);
    fresh.finish(&mut host, ElapsedTick(1), Some(&changed), snapshot()).unwrap().unwrap();
    assert!(host.authorize(host.revision(), 1, &changed, snapshot()).is_ok());
}

#[test]
fn worker_round_cannot_mix_manual_votes_or_reissue_ports_after_drop() {
    let root = Directory::new();
    let (mut host, _) = create(&root);
    let (_, input) = prepare(&mut host, 1, b"x");
    let cutoffs = window(&host);
    let (pool, clients) = launch(&mut host, 1, 101, cutoffs);
    let revision = host.revision();
    for after_drop in [false, true] {
        assert_eq!(host.commit_review(host.revision(), 101, "alpha", 0).unwrap_err(), JournalError::Contract(Error::WrongState));
        assert_eq!(host.open_reveals(host.revision(), 101).unwrap_err(), JournalError::Contract(Error::WrongState));
        assert_eq!(host.reveal_review(host.revision(), 101, "alpha", Verdict::Allow, vec![]).unwrap_err(), JournalError::Contract(Error::WrongState));
        assert_eq!(host.finish_review(host.revision(), 101, Some(&input), snapshot()).unwrap_err(), JournalError::Contract(Error::WrongState));
        assert_eq!(host.revision(), revision);
        if after_drop { break; }
    }
    drop(pool); drop(clients);
    assert_eq!(host.commit_review(host.revision(), 101, "alpha", 0).unwrap_err(), JournalError::Contract(Error::WrongState));
    let (streams, _) = sockets(0);
    let repeated = FileHelperLaunch { attempt: 1, round: 101, evidence_root: ROOT, window: cutoffs,
        expected_input_revision: 1, streams, limits: HelperLimits::default() };
    assert!(host.begin_helper_review(host.revision(), repeated, snapshot()).is_err());
    let (mut fresh, mut peers) = launch(&mut host, 1, 102, cutoffs);
    complete(&mut fresh, &mut host, &mut peers, ElapsedTick(1), Verdict::Allow);
    fresh.finish(&mut host, ElapsedTick(1), Some(&input), snapshot()).unwrap().unwrap();
}

#[test]
fn bad_launch_sends_no_bytes_and_does_not_burn_a_round_or_weaken_the_roster() {
    let root = Directory::new();
    let (mut host, _) = create(&root);
    let (_, input) = prepare(&mut host, 1, b"x");
    let cutoffs = window(&host);
    let before = host.inspect();
    let (server, mut peer) = UnixStream::pair().unwrap();
    peer.set_nonblocking(true).unwrap();
    let incomplete = FileHelperLaunch { attempt: 1, round: 101, evidence_root: ROOT, window: cutoffs,
        expected_input_revision: 1, streams: BTreeMap::from([("alpha".to_owned(), server)]), limits: HelperLimits::default() };
    assert_eq!(host.begin_helper_review(host.revision(), incomplete, snapshot()).unwrap_err(), FileHelperSetupError::Journal(JournalError::Contract(Error::Binding)));
    assert_eq!(peer.read(&mut [0_u8; 1]).unwrap(), 0);
    assert_eq!(host.inspect(), before);
    let (streams, _) = sockets(0);
    let limited = FileHelperLaunch { attempt: 1, round: 101, evidence_root: ROOT, window: cutoffs,
        expected_input_revision: 1, streams, limits: HelperLimits { input_bytes: input.logical_bytes() - 1, ..HelperLimits::default() } };
    assert_eq!(host.begin_helper_review(host.revision(), limited, snapshot()).unwrap_err(), FileHelperSetupError::Journal(JournalError::Contract(Error::Limit)));
    assert_eq!(host.inspect(), before);
    let (mut pool, mut clients) = launch(&mut host, 1, 101, cutoffs);
    complete(&mut pool, &mut host, &mut clients, ElapsedTick(1), Verdict::Allow);
    assert!(pool.finish(&mut host, ElapsedTick(1), Some(&input), snapshot()).unwrap().is_ok());
}

#[test]
fn post_read_clock_cutoff_cannot_backdate_queued_commitments() {
    let root = Directory::new();
    let (mut host, _) = create(&root);
    let (_, input) = prepare(&mut host, 1, b"late");
    let cutoffs = window(&host);
    let (mut pool, mut clients) = launch(&mut host, 1, 101, cutoffs);
    queue_commits(&mut pool, &mut host, &mut clients);
    let mut calls = 0;
    pool.pump_with_clock(&mut host, || {
        calls += 1;
        if calls <= 2 { ElapsedTick(1) } else { cutoffs.commit_by }
    }).unwrap();
    assert!(pool.statuses().values().all(|status| !status.committed));
    assert!(pool.statuses().values().all(|status| status.failure == Some(HelperFailure::CommitDeadline)));
    let receipt = pool.finish(&mut host, cutoffs.reveal_by, Some(&input), snapshot()).unwrap().unwrap();
    assert_eq!(receipt.policy.control.decision.consequence, Consequence::HoldEffect);
    assert!(host.authorize(host.revision(), 1, &input, snapshot()).is_err());
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn foreign_owner_cannot_drive_a_pool_and_reopening_never_recovers_a_live_worker_lease() {
    let first = Directory::new(); let second = Directory::new();
    let (mut host, _) = create(&first); let (mut other, _) = create(&second);
    let (_, input) = prepare(&mut host, 1, b"x");
    let cutoffs = window(&host);
    let (mut pool, mut clients) = launch(&mut host, 1, 101, cutoffs);
    assert_eq!(pool.pump(&mut other, ElapsedTick(1)).unwrap_err().error, JournalError::Contract(Error::Binding));
    assert!(!pool.is_closed());
    complete(&mut pool, &mut host, &mut clients, ElapsedTick(1), Verdict::Allow);
    drop(host);
    let (mut restored, _) = FileOversight::open(first.store(), profile()).unwrap();
    assert_eq!(restored.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    assert_eq!(pool.finish(&mut restored, ElapsedTick(2), Some(&input), snapshot()).unwrap_err(), JournalError::Contract(Error::Binding));
    restored.observe_time(restored.revision(), ElapsedTick(2)).unwrap();
    assert!(restored.commit_review(restored.revision(), 101, "alpha", 0).is_err());
    let (_, new_input) = prepare(&mut restored, 2, b"fresh");
    let new_window = window(&restored);
    let (mut fresh, mut peers) = launch(&mut restored, 2, 102, new_window);
    complete(&mut fresh, &mut restored, &mut peers, ElapsedTick(2), Verdict::Allow);
    fresh.finish(&mut restored, ElapsedTick(2), Some(&new_input), snapshot()).unwrap().unwrap();
}

#[test]
fn stop_closes_worker_progress_without_erasing_an_already_published_unknown_effect() {
    let root = Directory::new();
    let (mut host, reviewer) = create(&root);
    let keys = oversight::ready(&mut host, &reviewer, 1, b"already seen");
    oversight::dispatch(&mut host, &keys);
    host.publish(host.revision(), 1).unwrap();
    let _ = prepare(&mut host, 2, b"not published");
    let cutoffs = window(&host);
    let (mut pool, _clients) = launch(&mut host, 2, 102, cutoffs);
    let control = host.inspect().control;
    host.request_stop(host.revision(), StopRequest { operation: 19,
        expected_control_sequence: control.sequence, expected_authority_epoch: control.ledger.epoch }).unwrap();
    let failure = pool.pump(&mut host, ElapsedTick(1)).unwrap_err();
    assert_eq!(failure.error, JournalError::Contract(Error::Missing));
    assert!(failure.progress.io.is_empty());
    assert!(pool.is_closed());
    assert_eq!(host.inspect().control.ledger.charged, 16);
    let sweep = host.progress_stop(host.revision(), ElapsedTick(2)).unwrap();
    assert!(sweep.progress.drained());
    assert_eq!(sweep.progress.charged_units, 16);
    assert_eq!(host.inspect().payload, b"already seen");
    assert_eq!(host.inspect().executions, 1);
}
