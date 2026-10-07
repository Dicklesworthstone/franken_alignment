//! Real native generation and durable effects; synthetic weights and decisions.
use super::*;
use super::tests::fixture::*;
use fa_reference::action::ActionState;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::ReviewClientProgress;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::{ReviewDecision, ReviewPacket};
use std::{fs, thread};

fn review_request(peers: PeerProfile, request: u64, decision: ReviewDecision)
    -> thread::JoinHandle<ReviewPacket>
{
    thread::spawn(move || {
        let started = Instant::now();
        while !peers.socket(request).exists() {
            assert!(started.elapsed() < Duration::from_secs(10), "no native human offer");
            thread::sleep(Duration::from_millis(1));
        }
        let mut client = peers.connect_client(request).unwrap();
        let mut packet = None;
        loop {
            assert!(started.elapsed() < Duration::from_secs(12));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    assert!(packet.is_none());
                    packet = Some(client.packet().unwrap().clone());
                    client.respond(decision).unwrap();
                }
                ReviewClientProgress::Complete => return packet.unwrap(),
                _ => thread::sleep(Duration::from_millis(1)),
            }
        }
    })
}
fn executed(result: &RunResult) -> bool {
    matches!(result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. }))
}
fn initial(root: &Root, decision: ReviewDecision) -> ReviewPacket {
    let config = configured(root); let peers = peers(root, &config);
    let human = review_request(peers.clone(), 1, decision);
    let result = create(config, loaded(false), &peers, || ElapsedTick(1000)).unwrap();
    let packet = human.join().unwrap();
    assert!(result.failure.is_none(), "{:?}", result.failure);
    assert_eq!(result.cleanup_pending, 0);
    assert_eq!(executed(&result), decision == ReviewDecision::Approve);
    packet
}

#[test]
fn resume_after_lost_stdout_uses_only_the_original_receipt_without_clock_or_source() {
    let root = Root::new(); initial(&root, ReviewDecision::Approve);
    let mut config = configured(&root); config.programs.clear();
    let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    let native = FileOversight::read_decoder_text_progress(&config.store, &config.profile,
        &loaded(false).decoder, &tokenizer(), 7).unwrap();
    let samples = native.text.numerical().receipt().unwrap().result().unwrap().work().attempted_samples;
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    let mut recipe = loaded(false); recipe.ttl_ms = 1; // Cannot renew an old deadline.
    let result = recovery::resume(config, recipe, || panic!("terminal receipt must not sample time")).unwrap();
    assert!(result.failure.is_none()); assert!(executed(&result)); assert_eq!(result.cleanup_pending, 0);
    let mut output = Vec::new(); emit(result, &mut output).unwrap();
    assert!(!output.windows(7).any(|bytes| bytes == b"payload"));
    let config = configured(&root);
    let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(after.executions, 1); assert_eq!(after.payload, before.payload);
    assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
    let native = FileOversight::read_decoder_text_progress(&config.store, &config.profile,
        &loaded(false).decoder, &tokenizer(), 7).unwrap();
    assert_eq!(native.text.numerical().receipt().unwrap().result().unwrap().work().attempted_samples, samples);
    assert_eq!(native.text.generation_revision(), 4);
    assert!(native.numerical.paused);
    assert_eq!(after.control.ledger.stages.get(&1), Some(&ActionState::Confirmed));
}

#[test]
fn recovery_cannot_reclassify_a_rejected_native_message_as_executed() {
    let root = Root::new(); initial(&root, ReviewDecision::Reject);
    let mut config = configured(&root); config.programs.clear();
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    let result = recovery::resume(config, loaded(false), || panic!("rejection is historical")).unwrap();
    assert!(result.failure.is_none()); assert!(!executed(&result));
    assert!(emit(result, &mut Vec::new()).is_err());
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(disk.executions, 0); assert!(disk.payload.is_empty());
    assert_eq!(disk.control.ledger.charged, 0); assert_eq!(disk.control.ledger.reserved, 0);
}

#[test]
fn recovery_refuses_wrong_source_prompt_and_budget_without_generation_or_effect_retry() {
    let root = Root::new(); initial(&root, ReviewDecision::Approve);
    for mutation in 0..4 {
        let mut config = configured(&root); config.programs.clear();
        let mut recipe = loaded(false);
        match mutation {
            0 => recipe.request = 99,
            1 => recipe.generation = 99,
            2 => recipe.text.prompt.push(b'y'),
            _ => recipe.text.generation.scalar_products -= 1,
        }
        fs::remove_file(root.0.join("evidence.json")).unwrap();
        assert!(recovery::resume(config, recipe, || panic!("identity refusal must not sample time")).is_err());
        let config = configured(&root);
        let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(disk.executions, 1);
        let native = FileOversight::read_decoder_text_progress(&config.store, &config.profile,
            &loaded(false).decoder, &tokenizer(), 7).unwrap();
        assert_eq!(native.text.generation_revision(), 4);
        assert_eq!(native.numerical.numerical.position, 4);
    }
}

#[test]
fn recovery_requires_the_original_native_bootstrap_and_never_creates_a_missing_store() {
    let root = Root::new(); let config = configured(&root);
    assert!(recovery::resume(config, loaded(false), || panic!("missing store" )).is_err());
    assert!(!root.0.join("store").exists());
    initial(&root, ReviewDecision::Approve);
    let config = configured(&root); let bytes = fs::read(config.store.join("delivery.bin")).unwrap();
    assert!(recovery::resume(config, loaded(true), || panic!("wrong monitor")).is_err());
    assert_eq!(fs::read(root.0.join("store/delivery.bin")).unwrap(), bytes);
}

#[test]
fn recovery_output_failure_does_not_change_the_canonical_publication() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> { Err(std::io::ErrorKind::BrokenPipe.into()) }
        fn flush(&mut self) -> std::io::Result<()> { panic!("failed output cannot flush") }
    }
    let root = Root::new(); initial(&root, ReviewDecision::Approve);
    let config = configured(&root);
    let result = recovery::resume(config, loaded(false), || panic!("terminal recovery")).unwrap();
    let before = fs::read(root.0.join("store/delivery.bin")).unwrap();
    assert!(emit(result, &mut Broken).unwrap_err().contains("no effect retry"));
    assert_eq!(fs::read(root.0.join("store/delivery.bin")).unwrap(), before);
}

#[test]
fn recovery_option_has_no_review_profile_or_qualification_fallback() {
    for args in [vec!["create-generated", "config", "recipe", "--resume", "reviewer"],
        vec!["create-generated", "config", "recipe", "--resume", "--after", "1"]] {
        assert_eq!(command(&args.into_iter().map(str::to_owned).collect::<Vec<_>>(), None).unwrap_err(), USAGE);
    }
    let args = ["create-generated", "config", "recipe", "--resume"].map(str::to_owned);
    assert_eq!(command(&args, Some(Path::new("qualification"))).unwrap_err(), USAGE);
}

#[test]
fn exact_source_retry_restores_only_a_local_ticket_without_a_journal_transition() {
    let root = Root::new(); initial(&root, ReviewDecision::Approve);
    let config = configured(&root); let (host, _) = recovery::open(&config, &loaded(false)).unwrap();
    let source = host.decoder_text_message_request(1).unwrap().clone();
    let revision = host.revision();
    let (port, supervisor) = host.into_generated_text_actor_gateway().unwrap();
    let driver = FileSupervisedDriver::new(supervisor);
    let mut refused = ActorWire::new(port.clone());
    let mut wire = ActorWire::new(port);
    let poll = encode_command(&Command::Poll { request: 1 }).unwrap();
    assert!(wire.exchange(&poll).result.is_err()); // A naked numeric ID is not a ticket.
    let mut wrong = source.clone(); wrong.generation += 1;
    let wrong = encode_command(&Command::Submit { request: 1,
        proposal: FileGeneratedTextActorPort::encode_message(&wrong).unwrap() }).unwrap();
    assert!(refused.exchange(&wrong).result.is_err());
    assert!(refused.exchange(&poll).result.is_err());
    let original = encode_command(&Command::Submit { request: 1,
        proposal: FileGeneratedTextActorPort::encode_message(&source).unwrap() }).unwrap();
    assert!(wire.exchange(&original).result.is_ok());
    assert!(matches!(wire.exchange(&poll).result,
        Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert_eq!(driver.supervisor().host().unwrap().revision(), revision);
    assert_eq!(driver.supervisor().host().unwrap().decoder_text_message_request(1).unwrap(), &source);
    assert_eq!(cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms), 0);
}

#[path = "operations/continuation.rs"]
mod continuation;
