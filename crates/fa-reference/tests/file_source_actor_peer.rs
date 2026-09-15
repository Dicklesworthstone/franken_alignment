//! Registered source intake behind the Linux kernel peer-credential boundary.
#![cfg(target_os = "linux")]
#[path = "support/file_source_intake.rs"]
mod fixture;
use fixture::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileActorPort;
use fa_reference::action::consequence::oversight::actor_peer::{PeerCredentials, PeerPolicy, PeerRefusal, PeerSession};
use fa_reference::action::consequence::oversight::actor_transport::DriveBudget;
use fa_reference::action::consequence::oversight::actor_wire::{ActorWire, ChannelLimits, Command, encode_command};
use fa_reference::action::consequence::oversight::policy_state::StateLimits;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;

type Port = FileActorPort<FileOversight>;

fn policy(socket: &UnixStream) -> PeerPolicy {
    let observed = PeerCredentials::observe(socket).unwrap();
    PeerPolicy::new(observed.uid(), observed.gid(), Some(observed.pid())).unwrap()
}
fn submit(file: &FileRig) -> Command { Command::Submit { request: 1, proposal: file.rig.proposal() } }

fn exchange(file: &mut FileRig, session: &mut PeerSession<Port>, peer: &mut UnixStream,
    command: &Command) -> (Vec<u8>, usize)
{
    peer.set_nonblocking(true).unwrap();
    let mut frame = encode_command(command).unwrap(); frame.push(b'\n'); peer.write_all(&frame).unwrap();
    let budget = DriveBudget { read_bytes: 7, write_bytes: 5, frames: 2, io_calls: 8 };
    let mut output = Vec::new(); let mut intakes = 0;
    for _ in 0..4096 {
        let report = file.rig.driver.drive_peer_from_file(session, &mut file.source, || ElapsedTick(1), budget).unwrap();
        intakes += report.intakes.len();
        for intake in report.intakes {
            assert_eq!(intake.source_updates.len(), 1);
            assert!(intake.source_updates[0].is_ok());
            assert!(intake.result.is_ok());
        }
        let mut bytes = [0; 1024];
        match peer.read(&mut bytes) {
            Ok(0) => panic!("unexpected source-peer closure"),
            Ok(count) => output.extend_from_slice(&bytes[..count]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("peer read: {error}"),
        }
        if output.ends_with(b"\n") { return (output, intakes); }
        std::thread::yield_now();
    }
    panic!("source-aware authenticated exchange did not finish");
}

#[test]
fn cold_authenticated_submit_persists_source_once_and_exact_reconnect_retry_is_read_free() {
    let mut file = cold(10, StateLimits::default());
    let (server, mut peer) = UnixStream::pair().unwrap(); let admission_policy = policy(&server);
    let mut session: PeerSession<Port> = PeerSession::new(admission_policy,
        ActorWire::new(file.rig.port.clone()), ChannelLimits::default(), 2).unwrap();
    session.attach(server).unwrap();
    let reads = file.source.status().read_attempts;
    let command = submit(&file);
    let (response, intakes) = exchange(&mut file, &mut session, &mut peer, &command);
    assert_eq!(intakes, 1);
    assert!(String::from_utf8(response).unwrap().contains("\"state\":\"pending\""));
    assert_eq!(file.source.status().read_attempts, reads + 1);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().retained_requests(), 1);
    let revision = file.rig.driver.supervisor().host().unwrap().revision();

    std::fs::remove_file(&file.path).unwrap();
    assert!(session.disconnect()); drop(peer);
    let (server, mut peer) = UnixStream::pair().unwrap(); session.attach(server).unwrap();
    let reads = file.source.status().read_attempts;
    let (response, intakes) = exchange(&mut file, &mut session, &mut peer, &command);
    assert_eq!(intakes, 0);
    assert!(String::from_utf8(response).unwrap().contains("\"state\":\"pending\""));
    assert_eq!(file.source.status().read_attempts, reads);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().revision(), revision);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().retained_requests(), 1);
}

#[test]
fn kernel_peer_rejection_precedes_source_io_and_durable_request_admission() {
    let mut file = cold(10, StateLimits::default());
    let (server, _peer) = UnixStream::pair().unwrap();
    let observed = PeerCredentials::observe(&server).unwrap();
    let wrong_pid = if observed.pid() < i32::MAX as u32 { observed.pid() + 1 } else { observed.pid() - 1 };
    let wrong = PeerPolicy::new(observed.uid(), observed.gid(), Some(wrong_pid)).unwrap();
    let mut refused: PeerSession<Port> = PeerSession::new(wrong,
        ActorWire::new(file.rig.port.clone()), ChannelLimits::default(), 1).unwrap();
    let reads = file.source.status().read_attempts;
    let revision = file.rig.driver.supervisor().host().unwrap().revision();
    assert_eq!(refused.attach(server), Err(PeerRefusal::CredentialsRejected));
    assert_eq!(file.source.status().read_attempts, reads);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().revision(), revision);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().retained_requests(), 0);

    // Positive control with the same durable owner and source.
    let (server, mut peer) = UnixStream::pair().unwrap(); let good = policy(&server);
    let mut accepted: PeerSession<Port> = PeerSession::new(good,
        ActorWire::new(file.rig.port.clone()), ChannelLimits::default(), 1).unwrap();
    accepted.attach(server).unwrap();
    let command = submit(&file);
    let (_, intakes) = exchange(&mut file, &mut accepted, &mut peer, &command);
    assert_eq!(intakes, 1);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().retained_requests(), 1);
}

#[test]
fn authenticated_poll_and_cancel_do_not_reread_or_renew_the_registered_source() {
    let mut file = cold(10, StateLimits::default());
    let (server, mut peer) = UnixStream::pair().unwrap(); let admission_policy = policy(&server);
    let mut session: PeerSession<Port> = PeerSession::new(admission_policy,
        ActorWire::new(file.rig.port.clone()), ChannelLimits::default(), 1).unwrap();
    session.attach(server).unwrap();
    let command = submit(&file); let (_, intakes) = exchange(&mut file, &mut session, &mut peer, &command);
    assert_eq!(intakes, 1);
    let reads = file.source.status().read_attempts;
    std::fs::remove_file(&file.path).unwrap();

    let (poll, intakes) = exchange(&mut file, &mut session, &mut peer, &Command::Poll { request: 1 });
    assert_eq!(intakes, 0); assert!(String::from_utf8(poll).unwrap().contains("\"state\":\"pending\""));
    assert_eq!(file.source.status().read_attempts, reads);

    let (cancel, intakes) = exchange(&mut file, &mut session, &mut peer, &Command::Cancel { request: 1 });
    assert_eq!(intakes, 0);
    assert!(String::from_utf8(cancel).unwrap().contains("cancelled_before_dispatch"));
    assert_eq!(file.source.status().read_attempts, reads);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().executions, 0);
}
