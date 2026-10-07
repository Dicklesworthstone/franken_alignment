//! Restart between real native messages, then run the original gates again.
use super::*;
use fa_reference::action::consequence::delivery::stream::ReleaseFrame;

// Recovery advances the policy epoch. Pin the test helper independently to that
// epoch; never obtain its expected audience from the untrusted incoming request.
fn continuation_config(root: &Root) -> Config {
    use fa_reference::action::consequence::oversight::helper_processes::HelperProgram;
    use std::{collections::BTreeMap, ffi::OsString};
    let mut config = configured(root);
    config.programs = ["alpha", "beta"].into_iter().map(|member| {
        let program = HelperProgram::new(std::env::current_exe().unwrap(), root.0.clone(),
            vec!["--exact".into(),
                "workflow::actor_service::generated::operations::continuation::native_continuation_helper".into(),
                "--nocapture".into()],
            BTreeMap::from([(OsString::from("FA_NATIVE_CONTINUATION_MEMBER"), OsString::from(member))])).unwrap();
        (member.to_owned(), program)
    }).collect();
    config
}

#[test]
fn native_continuation_helper() {
    use fa_reference::action::consequence::oversight::helper_client::{HelperClient, ClientPhase};
    use fa_reference::round::Verdict;
    let Ok(member) = std::env::var("FA_NATIVE_CONTINUATION_MEMBER") else { return; };
    let profile = Config::decode(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/fixtures/supervised_publication.json"))).unwrap().profile;
    let expected = profile.committee.members()[&member].profile_at(1);
    let mut client = HelperClient::from_process_stdin(expected).unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10));
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference {
            assert_eq!(client.input().unwrap().member(), member);
            client.respond(Verdict::Allow, member.as_bytes()).unwrap();
        }
        if client.phase() == ClientPhase::ReplySent { return; }
        thread::sleep(Duration::from_millis(1));
    }
}

fn next() -> Loaded {
    let mut recipe = loaded(false);
    recipe.request = 2; recipe.generation = 8;
    recipe.text.prompt = b"y".to_vec();
    recipe
}

#[test]
fn a_new_native_message_continues_recovered_context_and_the_complete_reviewed_prefix() {
    let root = Root::new(); let first = initial(&root, ReviewDecision::Approve);
    let config = continuation_config(&root); let audience = peers(&root, &config);
    let human = review_request(audience.clone(), 2, ReviewDecision::Approve);
    let result = continue_after(config, next(), &audience, 1, || ElapsedTick(1001)).unwrap();
    let packet = human.join().unwrap();
    assert!(result.failure.is_none(), "{:?}", result.failure); assert!(executed(&result));
    assert_eq!(result.cleanup_pending, 0);
    let frame = ReleaseFrame::decode(&packet.action().spec().payload).unwrap();
    assert_eq!(frame.prior_messages(), &["A"]); assert_eq!(frame.message(), Some("A"));
    assert_ne!(first.binding().request, packet.binding().request);
    assert_ne!(first.binding().session, packet.binding().session);
    let config = configured(&root); let recipe = next();
    let source = FileOversight::read_decoder_text_message(&config.store, &config.profile,
        &recipe.decoder, &recipe.tokenizer, recipe.stream, 2).unwrap();
    assert_eq!(source.source.generation, 8);
    assert_eq!(source.stream.confirmed.messages().collect::<Vec<_>>(), ["A", "A"]);
    assert_eq!(source.stream.confirmed, source.stream.published);
    assert_eq!(source.stream.publication.executions, 2);
    assert_eq!(source.stream.publication.control.ledger.reserved, 0);
    assert_eq!(source.stream.publication.control.ledger.charged,
        first.action().spec().units + packet.action().spec().units);
    let progress = FileOversight::read_decoder_text_progress(&config.store, &config.profile,
        &recipe.decoder, &recipe.tokenizer, 8).unwrap();
    assert_eq!(progress.text.generation_revision(), 4);
    assert_eq!(progress.numerical.numerical.position, 8);
    assert_eq!(progress.text.numerical().receipt().unwrap().result().unwrap().work().attempted_samples, 2);
    let (host, _) = recovery::open(&config, &recipe).unwrap();
    assert_eq!(host.decoder_text_generation(8).unwrap().command().position(), 4);
    assert_eq!(host.decoder_text_generation(7).unwrap().command().position(), 0);
    drop(host);
    let recovered = recovery::resume(config, recipe, || panic!("confirmed continuation is receipt-only")).unwrap();
    assert!(executed(&recovered)); assert!(recovered.failure.is_none());
}

#[test]
fn a_second_message_needs_its_own_human_key_and_rejection_keeps_prior_disclosure() {
    let root = Root::new(); let first = initial(&root, ReviewDecision::Approve);
    let config = continuation_config(&root); let audience = peers(&root, &config);
    let human = review_request(audience.clone(), 2, ReviewDecision::Reject);
    let result = continue_after(config, next(), &audience, 1, || ElapsedTick(1001)).unwrap();
    let packet = human.join().unwrap();
    assert!(result.failure.is_none()); assert!(!executed(&result));
    assert_eq!(ReleaseFrame::decode(&packet.action().spec().payload).unwrap().prior_messages(), &["A"]);
    let config = configured(&root);
    let native = FileOversight::read_decoder_text_message(&config.store, &config.profile,
        &next().decoder, &tokenizer(), next().stream, 2).unwrap();
    assert_eq!(native.stream.publication.executions, 1);
    assert_eq!(native.stream.confirmed.messages().collect::<Vec<_>>(), ["A"]);
    assert_eq!(native.stream.publication.control.ledger.charged, first.action().spec().units);
    assert_eq!(native.stream.publication.control.ledger.reserved, 0);
    assert_eq!(native.generation.bytes().unwrap(), b"A");
    let recovered = recovery::resume(config, next(), || panic!("rejected continuation is historical")).unwrap();
    assert!(!executed(&recovered));
}

#[test]
fn stale_prefix_and_reused_native_ids_refuse_before_any_additional_generation() {
    let root = Root::new(); initial(&root, ReviewDecision::Approve);
    let config = continuation_config(&root); let audience = peers(&root, &config);
    let human = review_request(audience.clone(), 2, ReviewDecision::Approve);
    let result = continue_after(config, next(), &audience, 1, || ElapsedTick(1001)).unwrap();
    human.join().unwrap(); assert!(executed(&result));
    for mutation in 0..4 {
        let mut config = configured(&root); config.programs.clear();
        let audience = peers(&root, &config); let mut recipe = next();
        recipe.request = 3; recipe.generation = 9;
        let after = match mutation {
            0 => 1, // Older confirmed request is not the current prefix.
            1 => { recipe.request = 1; 2 }, // Spent request ID, even with a new generation.
            2 => { recipe.generation = 8; 2 }, // Consumed native generation ID.
            _ => 99,
        };
        fs::remove_file(root.0.join("evidence.json")).unwrap();
        assert!(continue_after(config, recipe, &audience, after, || ElapsedTick(1002)).is_err());
        let config = configured(&root);
        let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(disk.executions, 2);
        let progress = FileOversight::read_decoder_text_progress(&config.store, &config.profile,
            &next().decoder, &tokenizer(), 8).unwrap();
        assert_eq!(progress.numerical.numerical.position, 8);
        assert!(FileOversight::read_decoder_text_progress(&config.store, &config.profile,
            &next().decoder, &tokenizer(), 9).is_err());
    }
}

#[test]
fn rejected_predecessor_and_insufficient_remaining_context_do_not_launch_work() {
    for rejected in [false, true] {
        let root = Root::new();
        initial(&root, if rejected { ReviewDecision::Reject } else { ReviewDecision::Approve });
        let mut config = configured(&root); config.programs.clear();
        let audience = peers(&root, &config); let mut recipe = next();
        if !rejected { recipe.text.max_new_tokens = 11; } // 2 + 11 > 16 - 4.
        fs::remove_file(root.0.join("evidence.json")).unwrap();
        assert!(continue_after(config, recipe, &audience, 1, || ElapsedTick(1001)).is_err());
        let config = configured(&root);
        let progress = FileOversight::read_decoder_text_progress(&config.store, &config.profile,
            &next().decoder, &tokenizer(), 7).unwrap();
        assert_eq!(progress.numerical.numerical.position, 4);
        assert_eq!(progress.publication.executions, if rejected { 0 } else { 1 });
        assert!(FileOversight::read_decoder_text_progress(&config.store, &config.profile,
            &next().decoder, &tokenizer(), 8).is_err());
    }
}

#[test]
fn source_loss_after_reopen_cannot_spend_another_token_or_refund_prior_publication() {
    let root = Root::new(); let first = initial(&root, ReviewDecision::Approve);
    let mut config = configured(&root); config.programs.clear();
    let audience = peers(&root, &config); fs::remove_file(root.0.join("evidence.json")).unwrap();
    let result = continue_after(config, next(), &audience, 1, || ElapsedTick(1001)).unwrap();
    assert!(result.failure.is_some()); assert!(!executed(&result));
    assert_eq!(result.cleanup_pending, 0); assert!(!audience.socket(2).exists());
    let config = configured(&root);
    let progress = FileOversight::read_decoder_text_progress(&config.store, &config.profile,
        &next().decoder, &tokenizer(), 7).unwrap();
    assert_eq!(progress.numerical.numerical.position, 4);
    assert_eq!(progress.publication.executions, 1);
    assert_eq!(progress.publication.control.ledger.charged, first.action().spec().units);
    assert!(FileOversight::read_decoder_text_progress(&config.store, &config.profile,
        &next().decoder, &tokenizer(), 8).is_err());
}

#[test]
fn continuation_options_reject_ambiguity_before_files_or_clock_and_never_become_resume() {
    for (option, id) in [("--after", "0"), ("--after", "-1"), ("--after", "+1"),
        ("--after", "18446744073709551616"), ("--after", ""), ("--resume", "1")] {
        let args = ["create-generated", "missing-config", "missing-recipe", "missing-peer", option, id]
            .map(str::to_owned);
        assert_eq!(command(&args, None).unwrap_err(), USAGE);
    }
    let root = Root::new(); let config = configured(&root); let audience = peers(&root, &config);
    assert!(continue_after(config, next(), &audience, 2, || panic!("same request must refuse before clock")).is_err());
    assert!(!root.0.join("store").exists());
}

#[path = "continuation/interrupted.rs"]
mod interrupted;
