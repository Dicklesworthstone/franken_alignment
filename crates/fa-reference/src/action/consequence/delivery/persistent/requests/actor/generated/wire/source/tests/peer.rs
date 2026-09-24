//! Actual Unix pairs and original SO_PEERCRED admission, not supplied credentials.
use super::*;
use crate::action::consequence::delivery::persistent::requests::actor::source_wire::FileActorPeerDriveError;
use crate::action::consequence::oversight::actor_peer::{PeerCredentials, PeerPolicy, PeerRefusal, PeerSession};
use crate::action::consequence::oversight::actor_transport::{DriveBudget, MAX_DRIVE_FRAMES};
use crate::action::consequence::oversight::actor_wire::WireResponse;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;

type Session = PeerSession<FileGeneratedTextActorPort>;
fn connected(port: FileGeneratedTextActorPort) -> (Session, UnixStream) {
    let (server, client) = UnixStream::pair().unwrap();
    let credentials = PeerCredentials::observe(&server).unwrap();
    let policy = PeerPolicy::new(credentials.uid(), credentials.gid(), Some(credentials.pid())).unwrap();
    let mut session = PeerSession::new(policy, ActorWire::new(port), ChannelLimits::default(), 4).unwrap();
    session.attach(server).unwrap(); client.set_nonblocking(true).unwrap();
    (session, client)
}
fn input_budget() -> DriveBudget {
    DriveBudget { write_bytes: 0, frames: 1, ..DriveBudget::default() }
}
fn send(client: &mut UnixStream, document: &[u8]) {
    let mut line = document.to_vec(); line.push(b'\n'); client.write_all(&line).unwrap();
}
fn drain(session: &mut Session, client: &mut UnixStream) -> WireResponse {
    let mut bytes = Vec::new();
    for _ in 0..64 {
        // Permit only response write/flush: no further frame can enter without
        // going through the source-aware supervisor operation under test.
        session.drive(DriveBudget { read_bytes: 0, frames: 0, ..DriveBudget::default() }).unwrap();
        let mut buffer = [0; 513];
        match client.read(&mut buffer) {
            Ok(0) => panic!("unexpected EOF before response"),
            Ok(n) => bytes.extend_from_slice(&buffer[..n]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("response read: {error}"),
        }
        if bytes.last() == Some(&b'\n') { return decode_response(&bytes[..bytes.len() - 1]).unwrap(); }
    }
    panic!("response did not drain in bounded drives");
}

#[test]
fn generated_source_peer_credential_gate_precedes_intake_and_driver_accepts_matching_peer() {
    let mut s = setup(); let (server, mut client) = UnixStream::pair().unwrap();
    let observed = PeerCredentials::observe(&server).unwrap();
    let wrong_uid = if observed.uid() == 0 { 1 } else { 0 };
    let policy = PeerPolicy::new(wrong_uid, observed.gid(), Some(observed.pid())).unwrap();
    let mut refused = PeerSession::new(policy, ActorWire::new(s.port.clone()), ChannelLimits::default(), 1).unwrap();
    send(&mut client, &document(&s.input));
    let before = disk(&s); let reads = s.source.status().read_attempts;
    assert_eq!(refused.attach(server), Err(PeerRefusal::CredentialsRejected));
    assert_eq!(refused.status().connections_admitted, 0);
    assert!(matches!(s.supervisor.drive_generated_peer_from_file(&mut refused, &mut s.source, no_clock, input_budget()),
        Err(FileActorPeerDriveError::Wire(WireError::Unavailable))));
    assert_eq!(disk(&s), before); assert_eq!(s.source.status().read_attempts, reads);
    let (mut session, mut client) = connected(s.port.clone());
    let mut driver = FileSupervisedDriver::new(s.supervisor);
    send(&mut client, &document(&s.input));
    let report = driver.drive_generated_peer_from_file(&mut session, &mut s.source, || ElapsedTick(2), input_budget()).unwrap();
    assert_eq!(report.drive.progress.frames, 1); assert_eq!(report.intakes.len(), 1);
    assert!(report.intakes[0].result.is_ok()); pending(&drain(&mut session, &mut client));
    assert_eq!(driver.phase(), crate::action::consequence::delivery::persistent::observed::driver::FileDriverPhase::Idle);
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn generated_source_peer_backpressure_then_new_file_version_changes_admission() {
    let mut s = setup(); let (mut session, mut client) = connected(s.port.clone());
    let doc = document(&s.input); let reads = s.source.status().read_attempts;
    client.write_all(&doc).unwrap();
    let report = s.supervisor.drive_generated_peer_from_file(&mut session, &mut s.source, no_clock, input_budget()).unwrap();
    assert_eq!(report.drive.progress.frames, 0); assert!(report.intakes.is_empty());
    client.write_all(b"\n").unwrap();
    let report = s.supervisor.drive_generated_peer_from_file(&mut session, &mut s.source, || ElapsedTick(2), input_budget()).unwrap();
    assert_eq!(report.intakes.len(), 1); assert!(report.intakes[0].result.is_ok());
    send(&mut client, &encode_command(&Command::Cancel { request: 91 }).unwrap());
    let blocked = s.supervisor.drive_generated_peer_from_file(&mut session, &mut s.source, no_clock, input_budget()).unwrap();
    assert_eq!(blocked.drive.progress.frames, 0); assert!(blocked.intakes.is_empty());
    pending(&drain(&mut session, &mut client));
    let cancelled = s.supervisor.drive_generated_peer_from_file(&mut session, &mut s.source, no_clock, input_budget()).unwrap();
    assert!(cancelled.intakes.is_empty());
    assert!(matches!(drain(&mut session, &mut client).result,
        Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    s.root.replace(&capture(2, true, b"deny"));
    let mut next = s.input.clone(); next.request = 92;
    send(&mut client, &document(&next));
    let report = s.supervisor.drive_generated_peer_from_file(&mut session, &mut s.source, || ElapsedTick(3), input_budget()).unwrap();
    assert_eq!(report.intakes.len(), 1);
    assert_eq!(report.intakes[0].result, Ok(capture(2, true, b"deny").identity()));
    let refused = drain(&mut session, &mut client);
    assert!(matches!(refused.result, Ok(Knowledge::Known { value: ActorOutcome::NotAdmitted, .. })));
    assert!(!refused.encode().windows(b"private reviewer context".len()).any(|w| w == b"private reviewer context"));
    s.root.replace(&capture(3, true, b"allow"));
    send(&mut client, &document(&next));
    let retry = s.supervisor.drive_generated_peer_from_file(&mut session, &mut s.source, no_clock, input_budget()).unwrap();
    assert!(retry.intakes.is_empty());
    assert_eq!(drain(&mut session, &mut client), refused); // policy refusal is immutable for this ID
    next.request = 93;
    send(&mut client, &document(&next));
    let repaired = s.supervisor.drive_generated_peer_from_file(&mut session, &mut s.source, || ElapsedTick(4), input_budget()).unwrap();
    assert_eq!(repaired.intakes[0].result, Ok(capture(3, true, b"allow").identity()));
    assert_eq!(drain(&mut session, &mut client).result, Ok(Knowledge::Pending { request: 93 }));
    assert_eq!(s.source.status().read_attempts, reads + 3);
    assert_eq!(s.supervisor.host().unwrap().inspect().executions, 0);
}

#[test]
fn generated_source_peer_lost_reply_reconnect_never_reobserves_or_readmits_the_request() {
    let mut s = setup(); let (mut session, mut client) = connected(s.port.clone()); let doc = document(&s.input);
    send(&mut client, &doc);
    let report = s.supervisor.drive_generated_peer_from_file(&mut session, &mut s.source, || ElapsedTick(2), input_budget()).unwrap();
    assert_eq!(report.intakes.len(), 1); // response deliberately has not been written
    let before = disk(&s); let reads = s.source.status().read_attempts;
    assert!(session.disconnect()); drop(client); std::fs::remove_file(s.root.source()).unwrap();
    let (server, mut client) = UnixStream::pair().unwrap();
    session.attach(server).unwrap(); client.set_nonblocking(true).unwrap();
    assert_eq!(session.status().connections_admitted, 2);
    send(&mut client, &encode_command(&Command::Poll { request: 91 }).unwrap());
    let report = s.supervisor.drive_generated_peer_from_file(&mut session, &mut s.source, no_clock, input_budget()).unwrap();
    assert!(report.intakes.is_empty()); pending(&drain(&mut session, &mut client));
    let (mut fresh, mut client) = connected(s.port.clone());
    send(&mut client, &encode_command(&Command::Poll { request: 91 }).unwrap());
    s.supervisor.drive_generated_peer_from_file(&mut fresh, &mut s.source, no_clock, input_budget()).unwrap();
    assert!(matches!(drain(&mut fresh, &mut client).result, Ok(Knowledge::Withheld { .. })));
    send(&mut client, &doc);
    let retry = s.supervisor.drive_generated_peer_from_file(&mut fresh, &mut s.source, no_clock, input_budget()).unwrap();
    assert!(retry.intakes.is_empty()); pending(&drain(&mut fresh, &mut client));
    assert_eq!(disk(&s), before); assert_eq!(s.source.status().read_attempts, reads);
}

#[test]
fn generated_source_peer_domain_budget_and_revocation_checks_precede_socket_admission() {
    let mut a = setup(); let mut b = setup(); let (mut session, mut client) = connected(b.port.clone());
    send(&mut client, &document(&b.input));
    let status = session.status(); let before_a = disk(&a); let before_b = disk(&b);
    assert!(matches!(a.supervisor.drive_generated_peer_from_file(&mut session, &mut a.source, no_clock, input_budget()),
        Err(FileActorPeerDriveError::Journal(JournalError::Contract(Error::Binding)))));
    assert_eq!(session.status(), status); assert_eq!(disk(&a), before_a); assert_eq!(disk(&b), before_b);
    assert!(matches!(b.supervisor.drive_generated_peer_from_file(&mut session, &mut b.source, no_clock,
        DriveBudget { frames: MAX_DRIVE_FRAMES + 1, ..input_budget() }),
        Err(FileActorPeerDriveError::Wire(WireError::Capacity))));
    assert_eq!(session.status(), status);
    let zero = DriveBudget { read_bytes: 0, write_bytes: 0, frames: 0, io_calls: 0 };
    let no_work = b.supervisor.drive_generated_peer_from_file(&mut session, &mut b.source, no_clock, zero).unwrap();
    assert!(no_work.intakes.is_empty()); assert_eq!(no_work.drive.progress.read_bytes, 0);
    let valid = b.supervisor.drive_generated_peer_from_file(&mut session, &mut b.source, || ElapsedTick(2), input_budget()).unwrap();
    assert_eq!(valid.intakes.len(), 1); pending(&drain(&mut session, &mut client));
    let before = disk(&b); let reads = b.source.status().read_attempts;
    assert!(session.revoke());
    assert!(matches!(b.supervisor.drive_generated_peer_from_file(&mut session, &mut b.source, no_clock, input_budget()),
        Err(FileActorPeerDriveError::Wire(WireError::Withheld))));
    assert_eq!(disk(&b), before); assert_eq!(b.source.status().read_attempts, reads);
    assert!(matches!(b.supervisor.host().unwrap().request_status(91).unwrap().disposition,
        FileRequestDisposition::Admitted { .. })); // transport revocation is not effect cancellation
}

#[test]
fn generated_source_peer_recovery_rejects_a_fresh_readers_rolled_back_producer() {
    use crate::action::consequence::delivery::persistent::observed::guarded::{
        FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
    };
    use crate::action::consequence::delivery::persistent::observed::decoder::text::FileTextGenerationCommand;
    use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
        GenerationBudget, MAX_SAMPLING_ENTRIES, text::TextGenerationRequest, tokenizer::TokenizationBudget,
    };
    use crate::action::consequence::activation::tensor::kv::decoder::MAX_DECODER_PRODUCTS;
    use crate::action::consequence::oversight::evidence_source::FileEvidenceSource;
    let mut s = setup(); s.root.replace(&capture(2, true, b"allow"));
    let (mut session, mut client) = connected(s.port.clone()); let doc = document(&s.input);
    send(&mut client, &doc);
    s.supervisor.drive_generated_peer_from_file(&mut session, &mut s.source, || ElapsedTick(2), input_budget()).unwrap();
    pending(&drain(&mut session, &mut client));
    let host = s.supervisor.host().unwrap(); let control = host.inspect().control;
    let requirements = FileRecoveryRequirements { guards: FileGuardSet {
        stream: Some(s.port.profile()), decoder: Some(s.configuration.clone()), decoder_stop: None,
        source: Some(source_policy()), identity: None, campaigns: None, credential: None,
    }, effective_policy: profile().delivery.policy, credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: host.revision(), control_sequence: control.sequence,
            authority_epoch: control.ledger.epoch } };
    let anchor = host.history_anchor().unwrap();
    let numerical = host.decoder_inspection().unwrap().numerical;
    drop(host); drop(s.supervisor); drop(session); drop(client);
    let (host, _roles) = FileOversight::open_generated_text_stream_anchored(s.root.store(), profile(),
        &requirements, &s.tokenizer, &anchor).unwrap();
    assert!(host.decoder_inspection().unwrap().paused);
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    let (port, mut supervisor) = host.into_generated_text_actor_gateway().unwrap();
    assert!(matches!(s.port.submit(&s.input), Err(ActorError::Unavailable)));
    s.root.replace(&capture(1, true, b"allow"));
    let mut source = FileEvidenceSource::new(s.root.source(), 7, profile().delivery.scope, 4096).unwrap();
    let (mut session, mut client) = connected(port);
    send(&mut client, &doc);
    let retry = supervisor.drive_generated_peer_from_file(&mut session, &mut source, no_clock, input_budget()).unwrap();
    assert!(retry.intakes.is_empty()); assert_eq!(source.status().read_attempts, 0);
    assert!(matches!(drain(&mut session, &mut client).result,
        Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    let mut next = s.input.clone(); next.request = 92;
    next.policy_epoch = supervisor.host().unwrap().inspect().control.ledger.epoch;
    send(&mut client, &document(&next));
    let refused = supervisor.drive_generated_peer_from_file(&mut session, &mut source, || ElapsedTick(3), input_budget()).unwrap();
    assert_eq!(refused.intakes[0].result, Err(JournalError::Contract(Error::Stale)));
    assert_eq!(drain(&mut session, &mut client).result, Err(WireError::Unavailable));
    assert_eq!(supervisor.host().unwrap().file_source_status().unwrap().producer.unwrap().generation, 2);
    s.root.replace(&capture(3, true, b"allow"));
    send(&mut client, &document(&next));
    let prepared = supervisor.drive_generated_peer_from_file(&mut session, &mut source, || ElapsedTick(4), input_budget()).unwrap();
    assert!(prepared.intakes[0].result.is_ok());
    assert_eq!(drain(&mut session, &mut client).result, Err(WireError::Unavailable));
    // Successful source preparation cannot silently resume recovered inference.
    assert!(supervisor.host().unwrap().decoder_inspection().unwrap().paused);
    let mut borrowed = supervisor.host_mut().unwrap(); let host = &mut *borrowed;
    let n = host.decoder_inspection().unwrap().numerical;
    host.resume_decoder(host.revision(), n.actor_revision, n.position).unwrap();
    let r = TextGenerationRequest { prompt: b"ab".to_vec(), prefix_controls: vec![256],
        max_new_tokens: 2, stop_tokens: vec![256], tokenization: TokenizationBudget::default(),
        generation: GenerationBudget { scalar_products: MAX_DECODER_PRODUCTS, sampling_entries: MAX_SAMPLING_ENTRIES },
        max_output_bytes: 4 };
    let c = FileTextGenerationCommand::new(8, n.actor_revision, n.position, r).unwrap();
    host.generate_decoder_text(host.revision(), c).unwrap();
    assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, numerical.sampled_draws + 2);
    next.generation = 8; next.generation_revision = host.decoder_generation_progress(8).unwrap().generation_revision();
    drop(borrowed);
    send(&mut client, &document(&next));
    let admitted = supervisor.drive_generated_peer_from_file(&mut session, &mut source, || ElapsedTick(5), input_budget()).unwrap();
    assert!(admitted.intakes[0].result.is_ok());
    assert_eq!(drain(&mut session, &mut client).result, Ok(Knowledge::Pending { request: 92 }));
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
}
