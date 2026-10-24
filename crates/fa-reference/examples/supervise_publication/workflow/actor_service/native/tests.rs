//! Real inference, files, process helpers and socket clients; synthetic model only.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::ReviewClientProgress;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::{
    ReviewDecision, ReviewPacket,
};
use fa_reference::action::consequence::delivery::stream::ReleaseFrame;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::actor_wire::client::{
    ActorExchange, ClientIoBudget, ClientIoLimits, ClientProgress,
};
use fa_reference::action::consequence::oversight::actor_wire::{WireResponse, decode_command};
use std::os::unix::net::UnixStream;
use std::sync::mpsc;
#[path = "tests/checked_tests.rs"]
mod checked_tests;
#[path = "tests/fixture.rs"]
mod fixture;
#[path = "tests/recovery_tests.rs"]
mod recovery_tests;
#[path = "tests/sharded_tests.rs"]
mod sharded_tests;
#[path = "tests/tokenizer_tests.rs"]
mod tokenizer_tests;
use fixture::*;

struct ReferenceOutput {
    bytes: Vec<u8>,
    sender: Option<mpsc::Sender<Vec<u8>>>,
}
impl Write for ReferenceOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        if let Some(sender) = self.sender.take() {
            sender
                .send(self.bytes.clone())
                .map_err(|_| io::Error::from(io::ErrorKind::BrokenPipe))?;
        }
        Ok(())
    }
}
fn actor_thread(
    profile: Profile,
    receiver: mpsc::Receiver<Vec<u8>>,
) -> std::thread::JoinHandle<WireResponse> {
    std::thread::spawn(move || {
        let bytes = receiver.recv_timeout(Duration::from_secs(10)).unwrap();
        let mut command = decode_command(&bytes).unwrap();
        assert!(matches!(&command, Command::Submit { request, proposal }
            if *request == profile.request && proposal.payload.len() == FileGeneratedTextActorPort::INTENT_BYTES));
        let mut socket = UnixStream::connect(&profile.socket).unwrap();
        profile.supervisor.verify(&socket).unwrap();
        socket.set_nonblocking(true).unwrap();
        let mut budget = ClientIoBudget::new(ClientIoLimits::default()).unwrap();
        let started = Instant::now();
        loop {
            let mut exchange = ActorExchange::new(socket, command, &mut budget).unwrap();
            let response = loop {
                assert!(started.elapsed() < Duration::from_secs(12));
                match exchange.step(&mut budget).unwrap() {
                    ClientProgress::Response(response) => break response,
                    ClientProgress::Complete => panic!("missing response"),
                    _ => pause(1),
                }
            };
            if !matches!(
                response.result,
                Ok(Knowledge::Pending { .. }
                    | Knowledge::Unknown {
                        reason: UnknownReason::OutcomeUnknown
                    })
            ) {
                return response;
            }
            socket = exchange.into_stream().unwrap();
            command = Command::Poll {
                request: profile.request,
            };
            pause(1);
        }
    })
}
fn reviewing(
    profile: PeerProfile,
    store: std::path::PathBuf,
    bootstrap: fa_reference::action::consequence::delivery::persistent::observed::FileOversightProfile,
    decision: ReviewDecision,
) -> std::thread::JoinHandle<ReviewPacket> {
    std::thread::spawn(move || {
        let started = Instant::now();
        while !profile.socket(91).exists() {
            assert!(started.elapsed() < Duration::from_secs(10));
            pause(1);
        }
        let remaining = Duration::from_secs(10).saturating_sub(started.elapsed());
        let mut client = profile.connect_client_waiting(91, remaining).unwrap();
        let mut packet = None;
        loop {
            assert!(started.elapsed() < Duration::from_secs(12));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    // Actual generation and both helpers must NOT replace this key.
                    assert_eq!(
                        FileOversight::read_publication(&store, &bootstrap)
                            .unwrap()
                            .executions,
                        0
                    );
                    packet = Some(client.packet().unwrap().clone());
                    client.respond(decision).unwrap();
                }
                ReviewClientProgress::Complete => return packet.unwrap(),
                _ => pause(1),
            }
        }
    })
}

#[test]
fn native_source_lease_is_checked_before_the_first_numerical_step() {
    for now in [1009, 1010] {
        let root = Root::new();
        let mut config = configured(&root);
        config.source_policy.freshness =
            fa_reference::action::consequence::oversight::policy_state::StateFreshness::new(10)
                .unwrap();
        let native = inputs(&root, &config);
        let (mut host, _) = FileOversight::create_generated_text_stream_with_reserve(
            &config.store,
            config.profile.clone(),
            native.stream,
            native.decoder.clone(),
            native.tokenizer.clone(),
            RecoveryReserve::terminal(),
        )
        .unwrap();
        host.enable_file_source(host.revision(), config.source_policy)
            .unwrap();
        host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1000))
            .unwrap();
        host.observe_time(host.revision(), ElapsedTick(1000))
            .unwrap();
        let numerical = host.decoder_inspection().unwrap().numerical;
        let command = FileTextGenerationCommand::new(
            native.generation,
            numerical.actor_revision,
            numerical.position,
            native.request.clone(),
        )
        .unwrap();
        host.begin_decoder_text(host.revision(), command).unwrap();
        let deadline = Deadline {
            logical: ElapsedTick(10000),
            started: Instant::now(),
            wall: Duration::from_secs(10),
        };
        let result = stamp(&mut host, &deadline, &mut || ElapsedTick(now));
        if now == 1009 {
            result.unwrap();
            host.advance_decoder_text(host.revision(), native.generation, 0)
                .unwrap();
            assert_eq!(host.decoder_inspection().unwrap().numerical.position, 1);
            assert_eq!(
                host.decoder_text_progress(native.generation)
                    .unwrap()
                    .generation_revision(),
                1
            );
        } else {
            assert!(result.unwrap_err().contains("Stale"));
            assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
            assert_eq!(
                host.decoder_text_progress(native.generation)
                    .unwrap()
                    .generation_revision(),
                0
            );
        }
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().control.ledger.charged, 0);
        assert_eq!(host.inspect().control.ledger.reserved, 0);
    }
}

#[test]
fn native_service_computed_text_requires_original_helpers_and_separate_human_key() {
    for approve in [true, false] {
        let root = Root::new();
        let config = configured(&root);
        let (actor, reviewer) = profiles(&root, &config);
        let inputs = inputs(&root, &config);
        let store = config.store.clone();
        let bootstrap = config.profile.clone();
        let (sender, receiver) = mpsc::channel();
        let client = actor_thread(actor.clone(), receiver);
        let human = reviewing(
            reviewer.clone(),
            store.clone(),
            bootstrap.clone(),
            if approve {
                ReviewDecision::Approve
            } else {
                ReviewDecision::Reject
            },
        );
        let mut output = ReferenceOutput {
            bytes: Vec::new(),
            sender: Some(sender),
        };
        let result = serve(
            config,
            &actor,
            &reviewer,
            inputs,
            || ElapsedTick(1000),
            &mut output,
        );
        assert!(result.is_ok(), "{result:?}");
        let packet = human.join().unwrap();
        let response = client.join().unwrap();
        let frame = ReleaseFrame::decode(&packet.action().spec().payload).unwrap();
        assert_eq!(frame.message(), Some("A"));
        assert!(frame.prior_messages().is_empty());
        assert_eq!(packet.views().len(), 2);
        let stored = FileOversight::read_publication(&store, &bootstrap).unwrap();
        if approve {
            assert!(matches!(
                response.result,
                Ok(Knowledge::Known {
                    value: ActorOutcome::Executed,
                    ..
                })
            ));
            assert_eq!(stored.executions, 1);
            // The audience sees text; accounting charges the entire reviewed frame.
            assert_eq!(stored.payload, b"A");
            assert_eq!(
                stored.control.ledger.charged,
                packet.action().spec().payload.len() as u64
            );
        } else {
            assert!(matches!(
                response.result,
                Ok(Knowledge::Known {
                    value: ActorOutcome::CancelledBeforeDispatch,
                    ..
                })
            ));
            assert_eq!(stored.executions, 0);
            assert!(stored.payload.is_empty());
            assert_eq!(stored.control.ledger.charged, 0);
        }
        assert_eq!(stored.control.ledger.reserved, 0);
        assert_eq!(
            std::str::from_utf8(&output.bytes).unwrap().lines().count(),
            1
        );
        assert!(!actor.socket.exists());
        assert!(!reviewer.socket(91).exists());
        assert!(!super::super::super::control::socket_path(&reviewer, 91).exists());
    }
}

#[test]
fn native_service_hold_partial_output_and_source_loss_never_emit_a_reference() {
    for cause in ["hold", "token_limit", "source_missing"] {
        let root = Root::new();
        let config = configured(&root);
        let (actor, reviewer) = profiles(&root, &config);
        write_inputs(&root);
        if cause == "hold" {
            let path = root.0.join("monitor.json");
            let original = std::fs::read_to_string(&path).unwrap();
            std::fs::write(path, original.replace("3.0", "-3.0")).unwrap();
        }
        if cause == "token_limit" {
            let path = root.0.join("recipe.json");
            let original = std::fs::read_to_string(&path).unwrap();
            std::fs::write(
                path,
                original.replace("\"max_new_tokens\":2", "\"max_new_tokens\":1"),
            )
            .unwrap();
        }
        let native = recipe::load(
            &root.0.join("recipe.json"),
            1,
            config.profile.delivery.limits.bytes,
        )
        .unwrap();
        if cause == "source_missing" {
            std::fs::remove_file(root.0.join("evidence.json")).unwrap();
        }
        let store = config.store.clone();
        let bootstrap = config.profile.clone();
        let mut output = Vec::new();
        assert!(
            serve(
                config,
                &actor,
                &reviewer,
                native,
                || ElapsedTick(1000),
                &mut output
            )
            .is_err(),
            "{cause}"
        );
        assert!(output.is_empty(), "{cause}");
        let stored = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(stored.executions, 0);
        assert_eq!(stored.control.ledger.charged, 0);
        assert!(stored.payload.is_empty());
        assert!(stored.stop.is_some());
    }
}

#[test]
fn native_service_deadline_between_tokens_preserves_acknowledged_work_without_publication() {
    let root = Root::new();
    let config = configured(&root);
    let (actor, reviewer) = profiles(&root, &config);
    let native = inputs(&root, &config);
    let store = config.store.clone();
    let bootstrap = config.profile.clone();
    let decoder = native.decoder.clone();
    let tokenizer = native.tokenizer.clone();
    let mut output = Vec::new();
    let time = || {
        let progressed =
            FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 7)
                .is_ok_and(|image| image.numerical.numerical.position > 0);
        ElapsedTick(if progressed { 20000 } else { 1000 })
    };
    assert!(serve(config, &actor, &reviewer, native, time, &mut output).is_err());
    assert!(output.is_empty());
    let image =
        FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 7)
            .unwrap();
    assert_eq!(image.numerical.numerical.position, 1);
    assert_eq!(image.publication.executions, 0);
    assert!(image.publication.stop.is_some());
}

#[test]
fn native_recipe_uses_exact_aggregate_bound_and_rejects_bad_semantics_before_store_creation() {
    let root = Root::new();
    let config = configured(&root);
    write_inputs(&root);
    let recipe_path = root.0.join("recipe.json");
    let names = [
        "recipe.json",
        "model.json",
        "model.safetensors",
        "monitor.json",
        "sampling.json",
        "tokenizer.bbpe",
        "prompt.txt",
    ];
    let total = names
        .iter()
        .map(|name| std::fs::metadata(root.0.join(name)).unwrap().len() as usize)
        .sum();
    assert!(recipe::load(&recipe_path, 1, total).is_ok());
    assert!(recipe::load(&recipe_path, 1, total - 1).is_err());
    assert!(recipe::load(&recipe_path, 2, total).is_err());
    let original = std::fs::read_to_string(&recipe_path).unwrap();
    std::fs::write(
        &recipe_path,
        original.replace("\"stop_tokens\":[256]", "\"stop_tokens\":[65]"),
    )
    .unwrap();
    assert!(recipe::load(&recipe_path, 1, total + 100).is_err());
    std::fs::write(
        &recipe_path,
        original.replacen('{', "{\"output_override\":\"arbitrary\",", 1),
    )
    .unwrap();
    std::fs::remove_file(root.0.join("model.safetensors")).unwrap();
    assert_eq!(
        recipe::load(&recipe_path, 1, total + 100).err().unwrap(),
        "unknown native recipe field"
    );
    assert!(!config.store.exists());
}

#[test]
fn native_command_refuses_unsupported_modes_and_profile_collisions_without_effects() {
    for mode in ["actor-submit", "serve-open-checked", "serve-create-checked"] {
        let args = [
            mode,
            "absent",
            "absent",
            "absent",
            "--native-text",
            "absent",
        ]
        .map(str::to_owned);
        assert_eq!(command(&args, None).unwrap_err(), USAGE);
    }
    let root = Root::new();
    let config = configured(&root);
    let (mut actor, reviewer) = profiles(&root, &config);
    let native = inputs(&root, &config);
    actor.socket = reviewer.socket(91);
    assert!(
        serve(
            config,
            &actor,
            &reviewer,
            native,
            || panic!("invalid profiles sampled time"),
            &mut Vec::new()
        )
        .is_err()
    );
    assert!(!root.0.join("store").exists());
    assert!(!actor.socket.exists());
}
