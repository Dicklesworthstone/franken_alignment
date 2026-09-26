//! Native continuation with real source files, computation and socket peers.
//! Independent labels and helper answers are synthetic enforcement fixtures.
use super::super::*;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::ReviewClientProgress;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::ReviewDecision;
use fa_reference::action::consequence::delivery::stream::ReleaseFrame;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::actor_wire::{decode_command, WireResponse};
use fa_reference::action::consequence::oversight::actor_wire::client::{ActorExchange, ClientIoBudget, ClientIoLimits, ClientProgress};
use fa_reference::action::consequence::oversight::helper_client::{ClientPhase, HelperClient};
use fa_reference::round::Verdict;
use std::os::unix::net::UnixStream;
use std::sync::mpsc;
#[allow(dead_code)]
mod fixtures;
use fixtures::*;

struct Output { bytes: Vec<u8>, sender: Option<mpsc::Sender<Vec<u8>>> }
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> { self.bytes.extend_from_slice(bytes); Ok(bytes.len()) }
    fn flush(&mut self) -> io::Result<()> {
        if let Some(sender) = self.sender.take() {
            sender.send(self.bytes.clone()).map_err(|_| io::Error::from(io::ErrorKind::BrokenPipe))?;
        }
        Ok(())
    }
}
fn actor(profile: Profile, receiver: mpsc::Receiver<Vec<u8>>) -> std::thread::JoinHandle<WireResponse> {
    std::thread::spawn(move || {
        let bytes = receiver.recv_timeout(Duration::from_secs(10)).unwrap();
        let mut command = decode_command(&bytes).unwrap();
        assert!(matches!(&command, Command::Submit { request, proposal }
            if *request == profile.request && proposal.payload.len() == FileGeneratedTextActorPort::INTENT_BYTES));
        let mut socket = UnixStream::connect(&profile.socket).unwrap();
        profile.supervisor.verify(&socket).unwrap(); socket.set_nonblocking(true).unwrap();
        let mut budget = ClientIoBudget::new(ClientIoLimits::default()).unwrap();
        let start = Instant::now();
        loop {
            let mut exchange = ActorExchange::new(socket, command, &mut budget).unwrap();
            let response = loop {
                assert!(start.elapsed() < Duration::from_secs(12));
                match exchange.step(&mut budget).unwrap() {
                    ClientProgress::Response(response) => break response,
                    ClientProgress::Complete => panic!("missing response"), _ => pause(1),
                }
            };
            if !matches!(response.result, Ok(Knowledge::Pending { .. }
                | Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown })) { return response; }
            socket = exchange.into_stream().unwrap(); command = Command::Poll { request: profile.request }; pause(1);
        }
    })
}
fn human(profile: PeerProfile, config: &Config, approve: bool) -> std::thread::JoinHandle<()> {
    let store = config.store.clone(); let bootstrap = config.profile.clone();
    std::thread::spawn(move || {
        let start = Instant::now();
        while !profile.socket(91).exists() { assert!(start.elapsed() < Duration::from_secs(10)); pause(1); }
        let mut client = profile.connect_client(91).unwrap();
        loop {
            assert!(start.elapsed() < Duration::from_secs(12));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    assert_eq!(FileOversight::read_publication(&store, &bootstrap).unwrap().executions, 0);
                    client.respond(if approve { ReviewDecision::Approve } else { ReviewDecision::Reject }).unwrap();
                }
                ReviewClientProgress::Complete => return, _ => pause(1),
            }
        }
    })
}
fn executed(response: &WireResponse) -> bool {
    matches!(&response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. }))
}

#[test]
fn synthetic_helper() {
    let Ok(member) = std::env::var("FA_NATIVE_QUAL_MEMBER") else { return; };
    let epoch = std::env::var("FA_NATIVE_QUAL_EPOCH").unwrap().parse::<u64>().unwrap();
    let config = Config::decode(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/supervised_publication.json"))).unwrap();
    let mut client = HelperClient::from_process_stdin(config.profile.committee.members()[&member].profile_at(epoch)).unwrap();
    let start = Instant::now();
    loop {
        assert!(start.elapsed() < Duration::from_secs(10));
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference {
            let input = client.input().unwrap(); assert_eq!(input.member(), member);
            assert!(!input.actual_input().submitted_bytes().windows(b"independent-evaluator".len())
                .any(|part| part == b"independent-evaluator"));
            client.respond(Verdict::Allow, member.as_bytes()).unwrap();
        }
        if client.phase() == ClientPhase::ReplySent { return; }
        pause(1);
    }
}

#[test]
fn native_joint_requalification_resumes_original_work_but_never_replaces_the_human_key() {
    for steps in [1, 4] {
        for approve in [false, true] {
            let root = Root::new(); let mut config = configured(&root);
            let native = inputs(&root, &config); let (peer, reviewer) = profiles(&root, &config);
            let (publication, _producer) = publication(&root, &config);
            let (_, fresh) = seed(&mut config, &native, &publication, steps);
            let path = root.0.join("qualification.bin");
            std::fs::write(&path, fresh.encode_reference().unwrap()).unwrap();
            programs(&mut config, &root, fresh.expected_epoch + 1);
            let store = config.store.clone(); let bootstrap = config.profile.clone();
            let decoder = native.decoder.clone(); let tokenizer = native.tokenizer.clone();
            let before = FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 7).unwrap();
            let (sender, receiver) = mpsc::channel(); let actor = actor(peer.clone(), receiver);
            let human = human(reviewer.clone(), &config, approve);
            let mut output = Output { bytes: Vec::new(), sender: Some(sender) };
            let result = serve_qualified(config, &peer, &reviewer, native,
                (true, Some(&publication), Some(&path)), || ElapsedTick(1001), &mut output);
            assert!(result.is_ok(), "{result:?}"); human.join().unwrap();
            assert_eq!(executed(&actor.join().unwrap()), approve);
            let after = FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 7).unwrap();
            assert_eq!(after.text.command(), before.text.command());
            assert_eq!(after.text.generation_revision(), 4); assert_eq!(after.numerical.numerical.position, 8);
            assert_eq!(after.text.bytes().unwrap(), b"A");
            assert_eq!(after.text.finish(), Some(Ok(GenerationFinish::StopToken)));
            assert_eq!(after.publication.executions, u64::from(approve));
            assert_eq!(after.publication.control.ledger.reserved, 0);
            if approve { assert_eq!(ReleaseFrame::decode(&after.publication.payload).unwrap().message(), Some("A")); }
            else { assert!(after.publication.payload.is_empty()); assert_eq!(after.publication.control.ledger.charged, 0); }
            assert_eq!(std::str::from_utf8(&output.bytes).unwrap().lines().count(), 1);
            assert!(!peer.socket.exists()); assert!(!reviewer.socket(91).exists());
        }
    }
}

#[test]
fn native_joint_requalification_rejects_lost_historical_and_joint_insufficient_evidence_before_resume() {
    for cause in ["missing", "historical", "insufficient_joint_roots"] {
        let root = Root::new(); let mut config = configured(&root);
        let native = inputs(&root, &config); let (peer, reviewer) = profiles(&root, &config);
        let (publication, _producer) = publication(&root, &config);
        let (old, fresh) = seed(&mut config, &native, &publication, 1);
        let path = root.0.join("qualification.bin");
        if cause == "historical" { std::fs::write(&path, old.encode_reference().unwrap()).unwrap(); }
        if cause == "insufficient_joint_roots" {
            // Same individual precision/recall requirements pass with one pair;
            // ONLY the frozen joint policy requires two distinct roots per class.
            let small = activation(&config, fresh.expected_control_sequence, fresh.expected_epoch, 21, 3, 1);
            std::fs::write(&path, small.encode_reference().unwrap()).unwrap();
        }
        let store = config.store.clone(); let bootstrap = config.profile.clone();
        let decoder = native.decoder.clone(); let tokenizer = native.tokenizer.clone();
        let before = FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 7).unwrap();
        let mut output = Vec::new();
        assert!(serve_qualified(config, &peer, &reviewer, native,
            (true, Some(&publication), Some(&path)), || ElapsedTick(1001), &mut output).is_err(), "{cause}");
        assert!(output.is_empty());
        let after = FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 7).unwrap();
        assert_eq!(after.text.command(), before.text.command());
        assert_eq!(after.text.generation_revision(), 1);
        assert_eq!(after.numerical.numerical.position, before.numerical.numerical.position);
        assert_eq!(after.publication.executions, 0); assert!(after.publication.stop.is_some());
        assert!(!reviewer.socket(91).exists());
    }
}

#[test]
fn native_joint_recorded_receipt_never_reads_a_supplied_qualification_or_reissues_the_effect() {
    let root = Root::new(); let mut config = configured(&root);
    let native = inputs(&root, &config); let (peer, reviewer) = profiles(&root, &config);
    let (publication, producer) = publication(&root, &config);
    let (_, fresh) = seed(&mut config, &native, &publication, 1);
    let path = root.0.join("qualification.bin");
    std::fs::write(&path, fresh.encode_reference().unwrap()).unwrap();
    programs(&mut config, &root, fresh.expected_epoch + 1);
    let store = config.store.clone(); let bootstrap = config.profile.clone();
    let decoder = native.decoder.clone(); let tokenizer = native.tokenizer.clone();
    let (sender, receiver) = mpsc::channel(); let first_actor = actor(peer.clone(), receiver);
    let human = human(reviewer.clone(), &config, true);
    let mut original = Output { bytes: Vec::new(), sender: Some(sender) };
    let result = serve_qualified(config, &peer, &reviewer, native,
        (true, Some(&publication), Some(&path)), || ElapsedTick(1001), &mut original);
    assert!(result.is_ok(), "{result:?}"); human.join().unwrap(); assert!(executed(&first_actor.join().unwrap()));
    drop(producer);
    let mut config = configured(&root); let native = inputs(&root, &config);
    config.programs.clear();
    for name in ["qualification.bin", "producer/delivery.bin", "evidence.json"] {
        std::fs::remove_file(root.0.join(name)).unwrap();
    }
    let (sender, receiver) = mpsc::channel(); let second_actor = actor(peer.clone(), receiver);
    let mut recovered = Output { bytes: Vec::new(), sender: Some(sender) };
    let result = serve_qualified(config, &peer, &reviewer, native,
        (true, Some(&publication), Some(&path)), || ElapsedTick(20000), &mut recovered);
    assert!(result.is_ok(), "{result:?}"); assert!(executed(&second_actor.join().unwrap()));
    assert_eq!(recovered.bytes, original.bytes);
    let after = FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 7).unwrap();
    assert_eq!(after.publication.executions, 1); assert_eq!(after.text.generation_revision(), 4);
    assert!(after.numerical.paused); assert!(!reviewer.socket(91).exists());
}

#[test]
fn native_joint_profile_recovery_cannot_drop_or_substitute_the_original_contract() {
    let root = Root::new(); let mut config = configured(&root);
    let native = inputs(&root, &config); let (peer, reviewer) = profiles(&root, &config);
    let (publication, _producer) = publication(&root, &config);
    seed(&mut config, &native, &publication, 1);
    // The old non-joint projection must still refuse rather than discard policy.
    assert!(publication.generated_profile(&config.profile, native.stream, Some(RecoveryReserve::terminal())).is_err());
    let before = std::fs::read(config.store.join("delivery.bin")).unwrap();
    let changed = PublicationProfile::decode(publication_json(&root, &config, 2).as_bytes()).unwrap();
    let store = config.store.clone(); let missing = root.0.join("must-not-be-read");
    let mut output = Vec::new();
    let result = serve_qualified(config, &peer, &reviewer, native,
        (true, Some(&changed), Some(&missing)), || ElapsedTick(1001), &mut output);
    assert!(result.is_err()); assert!(output.is_empty());
    assert_eq!(std::fs::read(store.join("delivery.bin")).unwrap(), before);
    assert!(!peer.socket.exists()); assert!(!reviewer.socket(91).exists());
    let absent = root.0.join("absent").display().to_string();
    for mode in ["serve-open", "serve-create"] {
        let args = [mode, &absent, &absent, &absent, "--native-text", &absent].map(str::to_owned);
        assert_ne!(command(&args, Some(&missing)).unwrap_err(), USAGE);
    }
}
