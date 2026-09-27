use super::*;
use fa_reference::action::consequence::oversight::{helper_client::{HelperClient, ClientPhase, ClientProgress},
    helper_workers::io::WorkerIoError,
    helper_processes::{HelperChildren, HelperProgram, launch_helpers},
    learned_host::sidecar::workers::transport::{LearnedSocketReview, LearnedWorkerSockets}};
use std::io::{self, Read};
use std::os::unix::net::UnixStream;
use std::ffi::OsString;
use std::time::{Duration, Instant};

type Clients = BTreeMap<(u64, String), HelperClient<UnixStream>>;
fn sockets(owner: &OversightBroker, schedule: &LearnedWorkerSchedule) -> (LearnedWorkerSockets, Clients) {
    let mut sockets = BTreeMap::new(); let mut clients = BTreeMap::new();
    for round in &schedule.rounds {
        let mut peers = BTreeMap::new();
        for (name, helper) in owner.contracts().members() {
            let (parent, child) = UnixStream::pair().unwrap();
            peers.insert(name.clone(), parent);
            clients.insert((round.round, name.clone()), HelperClient::from_unix(child,
                helper.profile_at(owner.inspect().ledger.epoch)).unwrap());
        }
        sockets.insert(round.round, peers);
    }
    (sockets, clients)
}
fn pump(owner: &mut OversightBroker, driver: &mut LearnedSocketReview, now: u64) -> Result<LearnedWorkerStatus, WorkerIoError> {
    let revision = driver.review().revision();
    owner.pump_learned_socket_review(driver, revision, ElapsedTick(now), &snapshot())
}
fn answer(client: &mut HelperClient<UnixStream>, refine: bool) {
    if client.step().unwrap() == ClientProgress::NeedsInference {
        let payload = client.input().unwrap().actual_input().part_bytes(1).unwrap();
        assert_eq!(&payload[..8], b"FASIDE\0\x01");
        let selected = u64::from_be_bytes(payload[16..24].try_into().unwrap());
        // The control actually consumes the received representation. It asks
        // for refinement only when that packet has no selected exact residual.
        client.respond(if refine && selected == 0 { Verdict::Abstain } else { Verdict::Allow },
            b"socket-control-salt").unwrap();
    }
}

#[test]
fn original_clients_receive_richer_packets_on_fresh_sockets_and_publish_with_two_keys() {
    let (mut owner, mut endpoint, human, action, sidecar) = setup(true, SidecarCongressBudget::default());
    let selection = schedule(2); let (peers, mut clients) = sockets(&owner, &selection);
    let numerical = owner.hosted_learned_generation().unwrap();
    let mut driver = owner.begin_learned_socket_review(sidecar, selection, peers, &snapshot()).unwrap();
    for _ in 0..64 {
        for ((round, _), client) in &mut clients {
            if *round == driver.review().current_round().round { answer(client, true); }
            else if *round > driver.review().current_round().round {
                assert_eq!(client.step().unwrap(), ClientProgress::Blocked);
                assert!(client.input().is_none());
            }
        }
        if matches!(pump(&mut owner, &mut driver, 1).unwrap(), LearnedWorkerStatus::Stopped(_)) { break; }
    }
    assert_eq!(driver.review().status(), LearnedWorkerStatus::Stopped(LearnedWorkerStop::Decided));
    assert_eq!(driver.review().history().len(), 2); assert!(driver.failures().is_empty());
    assert!(driver.io_steps() > 0); assert!(driver.io_steps() <= driver.review().polls() as u64 * 2);
    assert_eq!(owner.hosted_learned_generation().unwrap(), numerical);
    let input = driver.review().input().clone();
    owner.apply_review(driver.take_review().unwrap(), Some(&input), &snapshot()).unwrap();
    let permit = owner.authorize(1, Some(&input), &snapshot()).unwrap();
    assert!(owner.dispatch(&permit, &action, Some(&input), &snapshot()).is_err());
    let request = owner.request_human_approval(901, 1, Some(&input), ElapsedTick(80)).unwrap();
    let key = human.unwrap().approve(&request, ElapsedTick(1)).unwrap();
    let message = owner.dispatch_with_human(&permit, &key, &action, Some(&input), &snapshot()).unwrap();
    owner.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.payload(), b"visible"); assert_eq!(endpoint.execution_count(), 1);
}

#[test]
fn source_loss_is_checked_before_first_wire_byte_while_stale_calls_leave_transport_usable() {
    let (mut owner, endpoint, _, _, sidecar) = setup(false, SidecarCongressBudget::default());
    let selection = schedule(1); let mut peers = BTreeMap::new(); let mut readers = Vec::new();
    for name in owner.contracts().members().keys() {
        let (parent, child) = UnixStream::pair().unwrap(); child.set_nonblocking(true).unwrap();
        peers.insert(name.clone(), parent); readers.push(child);
    }
    let mut driver = owner.begin_learned_socket_review(sidecar, selection,
        BTreeMap::from([(101, peers)]), &snapshot()).unwrap();
    assert_eq!(owner.pump_learned_socket_review(&mut driver, 9, ElapsedTick(1), &snapshot()),
        Err(WorkerIoError::Protocol(Error::Stale)));
    assert_eq!(driver.io_steps(), 0); assert_eq!(driver.review().polls(), 0);
    let mut byte = [0];
    for reader in &mut readers { assert_eq!(reader.read(&mut byte).unwrap_err().kind(), io::ErrorKind::WouldBlock); }
    advance_model(&mut owner);
    assert_eq!(pump(&mut owner, &mut driver, 1), Err(WorkerIoError::Protocol(Error::Stale)));
    assert_eq!(driver.io_steps(), 0);
    assert_eq!(driver.review().status(), LearnedWorkerStatus::Failed(Error::Stale));
    for reader in &mut readers { assert_eq!(reader.read(&mut byte).unwrap(), 0); }
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn missing_socket_inventory_refuses_before_round_and_disconnected_peer_is_not_replaced() {
    let (mut owner, endpoint, _, _, sidecar) = setup(false, SidecarCongressBudget::default());
    assert!(owner.begin_learned_socket_review(sidecar, schedule(2), BTreeMap::new(), &snapshot()).is_err());
    assert!(owner.begin_review(1, 101, [7; 32], schedule(1).rounds[0].window, &snapshot()).is_ok());
    assert_eq!(endpoint.execution_count(), 0);

    let (mut owner, endpoint, _, _, sidecar) = setup(false, SidecarCongressBudget::default());
    let selection = schedule(2); let (peers, mut clients) = sockets(&owner, &selection);
    drop(clients.remove(&(101, "bob".to_owned())).unwrap());
    let mut driver = owner.begin_learned_socket_review(sidecar, selection, peers, &snapshot()).unwrap();
    for _ in 0..20 {
        answer(clients.get_mut(&(101, "alice".to_owned())).unwrap(), false);
        pump(&mut owner, &mut driver, 1).unwrap();
    }
    assert_eq!(driver.review().status(), LearnedWorkerStatus::Running);
    assert!(matches!(pump(&mut owner, &mut driver, 20).unwrap(), LearnedWorkerStatus::Stopped(LearnedWorkerStop::Missing)));
    assert!(driver.failures().contains_key(&(101, "bob".to_owned())));
    assert_eq!(driver.review().history().len(), 1); assert_eq!(driver.review().input_revision(), 1);
    assert_eq!(driver.take_review().unwrap().missing(), &["bob".to_owned()]);
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn socket_poll_limit_and_cancellation_close_unstarted_peers_without_more_io() {
    for cancel in [false, true] {
        let (mut owner, _, _, _, sidecar) = setup(false, SidecarCongressBudget::default());
        let mut selection = schedule(2); selection.polls = 1;
        let (peers, mut clients) = sockets(&owner, &selection);
        let mut driver = owner.begin_learned_socket_review(sidecar, selection, peers, &snapshot()).unwrap();
        pump(&mut owner, &mut driver, 1).unwrap(); let count = driver.io_steps();
        if cancel { driver.cancel(driver.review().revision()).unwrap(); }
        else { assert_eq!(pump(&mut owner, &mut driver, 1), Err(WorkerIoError::Protocol(Error::Limit))); }
        assert_eq!(driver.io_steps(), count); assert!(driver.take_review().is_err());
        for name in ["alice", "bob"] {
            let peer = clients.get_mut(&(102, name.to_owned())).unwrap();
            assert_eq!(peer.step(), Err(WorkerIoError::Io(io::ErrorKind::UnexpectedEof)));
            assert!(peer.input().is_none());
        }
    }
}

// The following entrypoint runs only in explicitly spawned children. Its verdicts
// still test the protocol, not real helper-model reasoning or process isolation.
#[test]
fn learned_worker_child() {
    if std::env::var_os("FA_LEARNED_WORKER_CHILD").is_none() { return; }
    let epoch = std::env::var("FA_LEARNED_WORKER_EPOCH").unwrap().parse().unwrap();
    let mut client = HelperClient::from_process_stdin(InputProfileBinding { profile_id: 1,
        profile_bytes: Vec::new(), tokenizer_epoch: 4, policy_epoch: epoch, model_epoch: 3 }).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while client.phase() != ClientPhase::ReplySent && Instant::now() < deadline {
        answer(&mut client, true);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(client.phase(), ClientPhase::ReplySent);
}
struct Reap(Vec<HelperChildren>);
impl Reap {
    fn stop_and_reap(&mut self) -> bool {
        for children in &mut self.0 { children.request_stop_all(); }
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.0.iter().any(|children| !children.all_reaped()) && Instant::now() < deadline {
            for children in &mut self.0 { children.reap(); }
            std::thread::sleep(Duration::from_millis(1));
        }
        self.0.iter().all(HelperChildren::all_reaped)
    }
}
impl Drop for Reap {
    fn drop(&mut self) {
        if !self.stop_and_reap() { eprintln!("test helper cleanup incomplete"); }
    }
}
#[test]
fn separately_launched_helpers_drive_original_sidecar_refinement_over_inherited_sockets() {
    let (mut owner, endpoint, _, _, sidecar) = setup(false, SidecarCongressBudget::default());
    let mut selection = schedule(2); selection.polls = 60_000;
    let program = HelperProgram::new(std::env::current_exe().unwrap(), std::env::current_dir().unwrap(),
        vec![OsString::from("--exact"), OsString::from("transport::learned_worker_child"),
            OsString::from("--nocapture"), OsString::from("--test-threads=1")],
        BTreeMap::from([(OsString::from("FA_LEARNED_WORKER_CHILD"), OsString::from("1")),
            (OsString::from("FA_LEARNED_WORKER_EPOCH"), OsString::from(owner.inspect().ledger.epoch.to_string()))])).unwrap();
    let programs = owner.contracts().members().keys().map(|name| (name.clone(), program.clone())).collect();
    let mut reaper = Reap(Vec::new()); let mut peers = BTreeMap::new();
    for round in &selection.rounds {
        let (streams, children) = launch_helpers(owner.contracts(), &programs).unwrap();
        peers.insert(round.round, streams); reaper.0.push(children);
    }
    let mut driver = owner.begin_learned_socket_review(sidecar, selection, peers, &snapshot()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while driver.review().status() == LearnedWorkerStatus::Running && Instant::now() < deadline {
        pump(&mut owner, &mut driver, 1).unwrap();
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(driver.review().status(), LearnedWorkerStatus::Stopped(LearnedWorkerStop::Decided));
    assert_eq!(driver.review().history().len(), 2); assert!(driver.failures().is_empty());
    assert_eq!(driver.take_review().unwrap().decision().consequence, Consequence::Continue);
    assert!(reaper.stop_and_reap(), "direct helper children not reaped");
    // Even successful original socket reviews have not yet applied authority.
    assert_eq!(endpoint.execution_count(), 0);
    assert!(owner.authorize(1, Some(driver.review().input()), &snapshot()).is_err());
}

#[test]
fn expired_uncontacted_workers_receive_no_evidence_and_remain_missing() {
    let (mut owner, endpoint, _, _, sidecar) = setup(false, SidecarCongressBudget::default());
    let selection = schedule(1); let (peers, mut clients) = sockets(&owner, &selection);
    let mut driver = owner.begin_learned_socket_review(sidecar, selection, peers, &snapshot()).unwrap();
    assert_eq!(pump(&mut owner, &mut driver, 20).unwrap(), LearnedWorkerStatus::Stopped(LearnedWorkerStop::Missing));
    assert_eq!(driver.io_steps(), 0);
    assert_eq!(driver.take_review().unwrap().missing().len(), 2);
    for client in clients.values_mut() {
        assert_eq!(client.step(), Err(WorkerIoError::Io(io::ErrorKind::UnexpectedEof)));
        assert!(client.input().is_none());
    }
    assert_eq!(endpoint.execution_count(), 0);
}
