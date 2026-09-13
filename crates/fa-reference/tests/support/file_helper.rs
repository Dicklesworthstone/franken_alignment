#![allow(dead_code)]
#[path = "file_oversight.rs"] pub mod oversight;
pub use oversight::{Directory, MEMBERS, ROOT, create, inputs, profile, snapshot, spec, window};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::helpers::{FileHelperLaunch, FileHelperPool};
use fa_reference::action::consequence::oversight::{CommitteeInput, ReviewWindow};
use fa_reference::action::consequence::oversight::helper_client::{ClientPhase, HelperClient};
use fa_reference::action::consequence::oversight::helper_workers::HelperLimits;
use fa_reference::action::{ElapsedTick, FrozenAction};
use fa_reference::round::Verdict;
use std::collections::BTreeMap;
use std::os::unix::net::UnixStream;

pub type Clients = BTreeMap<String, HelperClient<UnixStream>>;
pub fn sockets(epoch: u64) -> (BTreeMap<String, UnixStream>, Clients) {
    let contracts = profile().committee;
    let mut streams = BTreeMap::new();
    let mut clients = BTreeMap::new();
    for member in MEMBERS {
        let (server, worker) = UnixStream::pair().unwrap();
        streams.insert(member.to_owned(), server);
        clients.insert(member.to_owned(), HelperClient::from_unix(worker, contracts.members()[member].profile_at(epoch)).unwrap());
    }
    (streams, clients)
}
pub fn prepare(host: &mut FileOversight, id: u64, payload: &[u8]) -> (FrozenAction, CommitteeInput) {
    let action = host.propose(host.revision(), id, spec(host, payload), snapshot()).unwrap();
    let input = inputs(&action, b"complete original helper context");
    host.record_inputs(host.revision(), id, 0, input.clone()).unwrap();
    (action, input)
}
pub fn launch(host: &mut FileOversight, attempt: u64, round: u64, window: ReviewWindow) -> (FileHelperPool, Clients) {
    let (streams, clients) = sockets(host.inspect().control.ledger.epoch);
    let launch = FileHelperLaunch { attempt, round, evidence_root: ROOT, window,
        expected_input_revision: host.input_revision(attempt).unwrap(), streams, limits: HelperLimits::default() };
    let pool = host.begin_helper_review(host.revision(), launch, snapshot()).unwrap();
    (pool, clients)
}
pub fn worker_steps(clients: &mut Clients, verdict: Verdict) {
    for (member, client) in clients {
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference {
            client.respond(verdict, &oversight::salt(member)).unwrap();
        }
    }
}
pub fn complete(pool: &mut FileHelperPool, host: &mut FileOversight, clients: &mut Clients, now: ElapsedTick, verdict: Verdict) {
    for _ in 0..128 {
        pool.pump(host, now).unwrap();
        worker_steps(clients, verdict);
        if pool.ready_to_finish() { return; }
    }
    panic!("original helper protocol did not complete within the fixed pump bound");
}

/// Queue actual commitment frames but stop before the supervising pool receives
/// them. All fixture inputs fit one body read and have equal per-member lengths.
pub fn queue_commits(pool: &mut FileHelperPool, host: &mut FileOversight, clients: &mut Clients) {
    for _ in 0..32 {
        pool.pump(host, ElapsedTick(1)).unwrap();
        worker_steps(clients, Verdict::Allow);
        for client in clients.values_mut() {
            if client.phase() == ClientPhase::SendingCommitment { client.step().unwrap(); }
        }
        if clients.values().all(|client| client.phase() == ClientPhase::AwaitingReveal) {
            assert!(pool.statuses().values().all(|status| !status.committed));
            return;
        }
    }
    panic!("failed to stage worker commitments");
}
