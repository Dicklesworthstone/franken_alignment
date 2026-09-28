//! Real Unix sockets and original journal/numerical engines. Remote ballots here
//! are scripted protocol controls, not evidence of learned helper quality.
use super::*;
use super::super::learned::{FileLearnedProbeReview, FileLearnedProbeStatus};
use super::super::super::{FileHumanReviewer, FileOversightProfile};
use super::super::super::decoder::learned::FileLearnedConfig;
use crate::action::{ActionState, FrozenAction};
use crate::action::consequence::delivery::persistent::JournalIo;
use crate::action::consequence::oversight::{
    helper_client::{ClientPhase, HelperClient},
    learned_host::sidecar::workers::probes::{ProbeReviewLimits, ProbeReviewMember},
    sidecar::SidecarRefinementOutcome,
};
use crate::round::Verdict;
use std::io::{self, Read};

// Reuse the unchanged real numerical/durable fixture, not a second model or
// altered test expectation. Its in-process-driver convenience helpers are unused.
#[allow(dead_code)]
#[path = "../learned/tests/fixture.rs"]
mod fixture;
use fixture::*;

type Clients = BTreeMap<(u64, String), HelperClient<UnixStream>>;
type WorkerPeers = BTreeMap<(u64, String), UnixStream>;

fn sockets(plan: &LearnedWorkerSchedule) -> (LearnedRoundPeers, WorkerPeers) {
    let mut peers = BTreeMap::new(); let mut workers = BTreeMap::new();
    for round in &plan.rounds {
        let mut roster = BTreeMap::new();
        for member in ["alpha", "beta"] {
            let (supervisor, worker) = UnixStream::pair().unwrap();
            worker.set_nonblocking(true).unwrap();
            roster.insert(member.to_owned(), supervisor);
            workers.insert((round.round, member.to_owned()), worker);
        }
        peers.insert(round.round, roster);
    }
    (peers, workers)
}
fn clients(host: &FileOversight, sidecar: &FileLearnedSidecar, workers: WorkerPeers) -> Clients {
    workers.into_iter().map(|(key, stream)| {
        let profile = host.current_learned_sidecar(sidecar).unwrap().views()[&key.1]
            .actual_input().input_profile().clone();
        (key, HelperClient::from_unix(stream, profile).unwrap())
    }).collect()
}
fn setup(root: &Directory) -> (FileOversight, FileHumanReviewer, FrozenAction, FileLearnedSidecar) {
    let (mut host, human) = owner(root, &config());
    step(&mut host); step(&mut host);
    let (action, sidecar) = propose(&mut host);
    (host, human, action, sidecar)
}
fn launch(host: &mut FileOversight, sidecar: FileLearnedSidecar, plan: LearnedWorkerSchedule)
    -> (FileLearnedTransportReview, Clients)
{
    let (peers, workers) = sockets(&plan);
    let clients = clients(host, &sidecar, workers);
    let run = host.begin_learned_transport_review(host.revision(), sidecar, plan, peers, snapshot()).unwrap();
    (run, clients)
}
fn peer_steps(clients: &mut Clients, required: u64, alarm: bool) {
    for ((_, member), client) in clients {
        if client.phase() == ClientPhase::ReplySent { continue; }
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference {
            let input = client.input().unwrap();
            let payload = input.actual_input().part_bytes(1).unwrap();
            assert_eq!(&payload[..8], b"FASIDE\0\x01");
            let selected = u64::from_be_bytes(payload[16..24].try_into().unwrap());
            let verdict = if alarm && member == "alpha" { Verdict::Hold }
                else if selected < required { Verdict::Abstain } else { Verdict::Allow };
            client.respond(verdict, &[19; 32]).unwrap();
        }
    }
}
fn complete(host: &mut FileOversight, run: &mut FileLearnedTransportReview,
    clients: &mut Clients, required: u64, alarm: bool, supplied: Snapshot)
    -> Result<FileLearnedTransportStatus, FileHelperSetupError>
{
    for _ in 0..64 {
        let status = run.advance(host, run.revision(), ElapsedTick(1), supplied.clone())?;
        if status != FileLearnedTransportStatus::Running { return Ok(status); }
        peer_steps(clients, required, alarm);
    }
    panic!("original socket protocol exceeded the frozen poll ceiling")
}
fn empty_read(stream: &mut UnixStream, closed: bool) {
    let mut byte = [0];
    match stream.read(&mut byte) {
        Ok(0) => assert!(closed, "unexpectedly closed uncontacted peer"),
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => assert!(!closed),
        result => panic!("unexpected disclosure/transport result: {result:?}"),
    }
}

#[test]
fn external_rounds_refine_exact_packets_then_publish_with_both_original_keys() {
    let root = Directory::new(); let (mut host, human, action, sidecar) = setup(&root);
    let before = host.learned_generation_inspection().unwrap().numerical;
    let first_input = host.current_learned_sidecar(&sidecar).unwrap().clone();
    let (mut run, mut workers) = launch(&mut host, sidecar, schedule());
    assert_eq!(run.records().len(), 10);
    assert!(workers.values().all(|client| client.input().is_none()));
    assert_eq!(complete(&mut host, &mut run, &mut workers, 2, false, snapshot()).unwrap(),
        FileLearnedTransportStatus::Finished);
    assert_eq!(run.history().len(), 3);
    assert!(matches!(&run.history()[0], FileLearnedSidecarFinish::Refined { input_revision: 2, .. }));
    assert!(matches!(&run.history()[1], FileLearnedSidecarFinish::Refined { input_revision: 3, .. }));
    assert!(matches!(&run.history()[2], FileLearnedSidecarFinish::Applied { receipt: Ok(_), .. }));
    for (index, outcome) in run.history().iter().enumerate() {
        let archive = outcome.archive();
        assert_eq!(archive.input_revision, index as u64 + 1);
        for member in ["alpha", "beta"] {
            let input = workers[&(101 + index as u64, member.to_owned())].input().unwrap();
            assert_eq!(input.actual_input(), archive.inputs.views()[member].actual_input());
        }
        assert!(host.learned_sidecar_outcome(101 + index as u64).is_ok());
    }
    assert!(run.records().iter().filter(|((round, _), _)| *round >= 104)
        .all(|(_, record)| *record == LearnedTransportRecord::default()));
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, before);
    assert!(host.authorize(host.revision(), 1, &first_input, snapshot()).is_err());
    let input = run.input().clone();
    assert!(host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(1)).is_err());
    let automatic = host.authorize(host.revision(), 1, &input, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), 1001, 1, &input, ElapsedTick(80)).unwrap();
    let revision = host.revision(); let key = human.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &key, &action, &input, snapshot()).unwrap();
    host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(2)).unwrap();
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().payload, b"visible");
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert!(host.dispatch(host.revision(), &automatic, &key, &action, &input, snapshot()).is_err());
    drop(host);
    let (mut recovered, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config()).unwrap();
    assert_eq!(recovered.inspect().executions, 1);
    assert!(recovered.learned_generation_inspection().unwrap().paused);
    for round in 101..104 { assert!(recovered.learned_sidecar_outcome(round).is_ok()); }
    assert!(recovered.dispatch(recovered.revision(), &automatic, &key, &action, &input, snapshot()).is_err());
}

#[test]
fn every_future_roster_is_validated_before_begin_or_any_disclosure() {
    for kind in 0..5 {
        let root = Directory::new(); let (mut host, _, _, sidecar) = setup(&root);
        let mut plan = schedule(); let (mut peers, mut workers) = sockets(&plan);
        let before = host.revision();
        match kind {
            0 => { peers.remove(&105); }
            1 => { peers.get_mut(&105).unwrap().remove("beta"); }
            2 => { plan.rounds[4].round = 101; }
            3 => { plan.helpers.members = 0; }
            4 => { plan.polls = 0; }
            _ => unreachable!(),
        }
        assert!(host.begin_learned_transport_review(before, sidecar, plan, peers, snapshot()).is_err());
        assert_eq!(host.revision(), before);
        for worker in workers.values_mut() { empty_read(worker, true); }
        assert!(host.worker_rounds.is_empty());
    }
}

#[test]
fn expired_uncontacted_workers_never_receive_evidence_or_become_abstentions() {
    let root = Directory::new(); let (mut host, _, _, sidecar) = setup(&root);
    let plan = schedule(); let (peers, mut workers) = sockets(&plan);
    let mut run = host.begin_learned_transport_review(host.revision(), sidecar, plan, peers, snapshot()).unwrap();
    run.advance(&mut host, 0, ElapsedTick(10), snapshot()).unwrap();
    assert_eq!(run.next_deadline(), Some(ElapsedTick(15)));
    for worker in workers.values_mut() { empty_read(worker, false); }
    assert_eq!(run.advance(&mut host, 1, ElapsedTick(15), snapshot()).unwrap(), FileLearnedTransportStatus::Finished);
    assert_eq!(run.input_revision(), 1); assert_eq!(run.history().len(), 1);
    assert!(matches!(&run.history()[0], FileLearnedSidecarFinish::Applied {
        outcome: Some(SidecarRefinementOutcome::Missing { members }), .. } if members.len() == 2));
    for worker in workers.values_mut() { empty_read(worker, true); }
    assert!(host.authorize(host.revision(), 1, run.input(), snapshot()).is_err());
}

#[test]
fn source_or_input_loss_closes_all_peers_before_the_first_wire_byte() {
    for input_loss in [false, true] {
        let root = Directory::new(); let (mut host, _, _, sidecar) = setup(&root);
        let plan = schedule(); let (peers, mut workers) = sockets(&plan);
        let mut run = host.begin_learned_transport_review(host.revision(), sidecar, plan, peers, snapshot()).unwrap();
        if input_loss { host.inputs_unavailable(host.revision(), 1, run.input_revision()).unwrap(); }
        else { step(&mut host); }
        assert!(run.advance(&mut host, 0, ElapsedTick(1), snapshot()).is_err());
        assert_eq!(run.status(), FileLearnedTransportStatus::Failed);
        assert!(run.records().values().all(|record| record.attempted_steps == 0));
        for worker in workers.values_mut() { empty_read(worker, true); }
        assert!(run.history().is_empty()); assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn stale_foreign_and_cancelled_calls_do_not_mutate_or_release_sockets_early() {
    let root = Directory::new(); let (mut host, _, _, sidecar) = setup(&root);
    let plan = schedule(); let (peers, mut workers) = sockets(&plan);
    let mut run = host.begin_learned_transport_review(host.revision(), sidecar, plan, peers, snapshot()).unwrap();
    let revision = host.revision();
    assert_eq!(run.advance(&mut host, 1, ElapsedTick(1), snapshot()).err(), Some(Error::Stale.into()));
    assert_eq!(run.advance(&mut host, 0, ElapsedTick(0), snapshot()).err(), Some(Error::Stale.into()));
    let other = Directory::new(); let (mut foreign, _) = owner(&other, &config());
    assert_eq!(run.advance(&mut foreign, 0, ElapsedTick(1), snapshot()).err(), Some(Error::Binding.into()));
    assert_eq!(host.revision(), revision); assert_eq!(run.revision(), 0);
    for worker in workers.values_mut() { empty_read(worker, false); }
    assert_eq!(run.cancel(1), Err(Error::Stale)); run.cancel(0).unwrap();
    for worker in workers.values_mut() { empty_read(worker, true); }
    assert_eq!(run.status(), FileLearnedTransportStatus::Cancelled);
    assert_eq!(run.advance(&mut host, 1, ElapsedTick(1), snapshot()).err(), Some(Error::WrongState.into()));
    for round in 101..106 {
        assert_eq!(host.commit_review(host.revision(), round, "alpha", 0).err(), Some(Error::WrongState.into()));
        assert_eq!(host.finish_review(host.revision(), round, Some(run.input()), snapshot()).err(), Some(Error::WrongState.into()));
    }
    drop(run);
    assert_eq!(host.open_reveals(host.revision(), 105).err(), Some(Error::WrongState.into()));
}

#[test]
fn richer_packet_ceiling_failure_retains_acknowledged_refinement_without_contacting_next_peers() {
    let root = Directory::new(); let (mut host, _, _, sidecar) = setup(&root);
    let mut plan = schedule();
    plan.helpers.input_bytes = host.current_learned_sidecar(&sidecar).unwrap().logical_bytes();
    let (mut run, mut workers) = launch(&mut host, sidecar, plan);
    assert!(complete(&mut host, &mut run, &mut workers, 2, false, snapshot()).is_err());
    assert_eq!(run.status(), FileLearnedTransportStatus::Failed);
    assert_eq!(run.history().len(), 1); assert_eq!(run.input_revision(), 2);
    assert!(matches!(&run.history()[0], FileLearnedSidecarFinish::Refined { .. }));
    assert!(host.learned_sidecar_outcome(101).is_ok());
    assert!(workers.iter().filter(|((round, _), _)| *round != 101).all(|(_, client)| client.input().is_none()));
    assert!(run.records().iter().filter(|((round, _), _)| *round != 101).all(|(_, record)| record.attempted_steps == 0));
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn missing_reveal_and_alarm_remain_nonpermitting_original_outcomes() {
    for disconnected in [false, true] {
        let root = Directory::new(); let (mut host, _, _, sidecar) = setup(&root);
        let (mut run, mut workers) = launch(&mut host, sidecar, schedule());
        if disconnected {
            workers.remove(&(101, "alpha".to_owned()));
            for _ in 0..12 {
                run.advance(&mut host, run.revision(), ElapsedTick(1), snapshot()).unwrap();
                peer_steps(&mut workers, 0, false);
            }
            // Missing alpha prevents early reveal. The original coordinator
            // opens at commit expiry; beta still has time to reveal before 15.
            for _ in 0..8 {
                run.advance(&mut host, run.revision(), ElapsedTick(10), snapshot()).unwrap();
                peer_steps(&mut workers, 0, false);
            }
            run.advance(&mut host, run.revision(), ElapsedTick(15), snapshot()).unwrap();
            assert!(run.records()[&(101, "alpha".to_owned())].first_failure.is_some());
            assert!(matches!(&run.history()[0], FileLearnedSidecarFinish::Applied {
                outcome: Some(SidecarRefinementOutcome::Missing { members }), .. } if members == &["alpha".to_owned()]));
        } else {
            complete(&mut host, &mut run, &mut workers, 0, true, snapshot()).unwrap();
        }
        assert_eq!(run.status(), FileLearnedTransportStatus::Finished);
        assert_eq!(run.history().len(), 1); assert_eq!(run.input_revision(), 1);
        assert!(host.authorize(host.revision(), 1, run.input(), snapshot()).is_err());
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn last_round_and_original_application_refusal_are_retained_not_retried() {
    for policy_changed in [false, true] {
        let root = Directory::new(); let (mut host, _, _, sidecar) = setup(&root);
        let mut plan = schedule(); plan.rounds.truncate(1);
        let (mut run, mut workers) = launch(&mut host, sidecar, plan);
        let mut supplied = snapshot();
        if policy_changed { supplied.values.insert(7, b"changed".to_vec()); }
        complete(&mut host, &mut run, &mut workers, if policy_changed { 0 } else { 9 }, false, supplied).unwrap();
        assert_eq!(run.history().len(), 1); assert_eq!(run.input_revision(), 1);
        if policy_changed {
            assert!(matches!(&run.history()[0], FileLearnedSidecarFinish::Applied { receipt: Err(_), .. }));
        } else {
            assert!(matches!(&run.history()[0], FileLearnedSidecarFinish::Applied { outcome: None, .. }));
        }
        let revision = host.revision();
        assert!(run.advance(&mut host, run.revision(), ElapsedTick(1), snapshot()).is_err());
        assert_eq!(host.revision(), revision);
        assert!(host.learned_sidecar_outcome(101).is_ok());
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn every_original_storage_barrier_prevents_reveal_of_an_unacknowledged_commitment() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let (mut host, _, _, sidecar) = setup(&root);
        let (mut run, mut workers) = launch(&mut host, sidecar, schedule());
        for _ in 0..32 {
            if workers[&(101, "alpha".to_owned())].phase() == ClientPhase::AwaitingReveal { break; }
            run.advance(&mut host, run.revision(), ElapsedTick(1), snapshot()).unwrap();
            peer_steps(&mut workers, 0, false);
        }
        assert_eq!(workers[&(101, "alpha".to_owned())].phase(), ClientPhase::AwaitingReveal);
        assert!(!run.worker_statuses()["alpha"].committed);
        host.store.fail_once(barrier);
        let error = run.advance(&mut host, run.revision(), ElapsedTick(1), snapshot()).unwrap_err();
        let FileHelperSetupError::Journal(JournalError::Io(ref failure)) = error else { panic!("original injected storage barrier"); };
        assert_eq!(failure.operation, barrier); assert_eq!(run.failure(), Some(&error));
        assert_eq!(run.status(), FileLearnedTransportStatus::Failed);
        assert!(run.history().is_empty()); assert!(!host.clock_ready());
        assert!(run.records()[&(101, "alpha".to_owned())].returned_steps > 0);
        assert!(workers.get_mut(&(101, "alpha".to_owned())).unwrap().step().is_err());
        let disk = FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config()).unwrap();
        assert_eq!(disk.executions, 0);
        drop(host);
        let (recovered, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config()).unwrap();
        assert_eq!(recovered.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    }
}

#[test]
fn poll_exhaustion_and_drop_close_future_peers_without_refunding_or_manual_takeover() {
    for exhaust in [false, true] {
        let root = Directory::new(); let (mut host, _, _, sidecar) = setup(&root);
        let mut plan = schedule(); plan.polls = 1;
        let (peers, mut workers) = sockets(&plan);
        let mut run = host.begin_learned_transport_review(host.revision(), sidecar, plan, peers, snapshot()).unwrap();
        if exhaust {
            run.advance(&mut host, 0, ElapsedTick(1), snapshot()).unwrap();
            assert_eq!(run.advance(&mut host, 1, ElapsedTick(1), snapshot()).err(), Some(Error::Limit.into()));
            assert_eq!(run.status(), FileLearnedTransportStatus::Failed);
            assert!(run.history().is_empty());
        }
        drop(run);
        for ((round, _), worker) in &mut workers {
            if *round != 101 || !exhaust { empty_read(worker, true); }
        }
        assert_eq!(host.commit_review(host.revision(), 105, "alpha", 0).err(), Some(Error::WrongState.into()));
        assert_eq!(host.inspect().control.ledger.available, 100);
        assert_eq!(host.inspect().executions, 0);
    }
}
