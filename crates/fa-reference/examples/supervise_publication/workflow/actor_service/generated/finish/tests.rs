//! Actual native generation, peer-checked review and original stream closure.
//! Model weights and helper/human choices are synthetic, not calibration evidence.
use super::*;
use super::super::tests::fixture::*;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::ReviewClientProgress;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::{ReviewDecision, ReviewPacket};
use fa_reference::action::consequence::delivery::stream::{ReleaseFrame, StreamProfile};
use fa_reference::action::consequence::oversight::helper_processes::HelperProgram;
use std::{collections::BTreeMap, ffi::OsString, fs, thread};

fn closing_config(root: &Root) -> Config {
    let mut c = configured(root);
    // Reuse the existing subprocess fixture, independently pinned to epoch 1.
    // The expected policy epoch is never copied from a helper's incoming offer.
    c.programs = ["alpha", "beta"].into_iter().map(|member| {
        (member.to_owned(), HelperProgram::new(std::env::current_exe().unwrap(), root.0.clone(),
            vec!["--exact".into(),
                "workflow::actor_service::generated::operations::continuation::native_continuation_helper".into(),
                "--nocapture".into()],
            BTreeMap::from([(OsString::from("FA_NATIVE_CONTINUATION_MEMBER"), OsString::from(member))])).unwrap())
    }).collect();
    c
}
fn review(peers: PeerProfile, request: u64, choice: ReviewDecision) -> thread::JoinHandle<ReviewPacket> {
    thread::spawn(move || {
        let start = Instant::now();
        while !peers.socket(request).exists() {
            assert!(start.elapsed() < Duration::from_secs(10));
            thread::sleep(Duration::from_millis(1));
        }
        let mut client = peers.connect_client(request).unwrap(); let mut packet = None;
        loop {
            assert!(start.elapsed() < Duration::from_secs(12));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    assert!(packet.is_none()); packet = Some(client.packet().unwrap().clone());
                    client.respond(choice).unwrap();
                }
                ReviewClientProgress::Complete => return packet.unwrap(),
                _ => thread::sleep(Duration::from_millis(1)),
            }
        }
    })
}
fn recipe(full: bool) -> Loaded {
    let mut r = loaded(false);
    if full { r.stream = StreamProfile::new(9, 1, 1, 1, 1).unwrap(); r.text.max_output_bytes = 1; }
    r
}
fn initial(root: &Root, choice: ReviewDecision, full: bool) -> ReviewPacket {
    let c = configured(root); let p = peers(root, &c); let human = review(p.clone(), 1, choice);
    let result = super::super::create(c, recipe(full), &p, || ElapsedTick(1000)).unwrap();
    let packet = human.join().unwrap();
    assert!(result.failure.is_none(), "{:?}", result.failure); assert_eq!(result.cleanup_pending, 0);
    assert_eq!(executed(&result), choice == ReviewDecision::Approve); packet
}
fn executed(result: &RunResult) -> bool {
    matches!(result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. }))
}
fn native(root: &Root, full: bool)
    -> fa_reference::action::consequence::delivery::persistent::observed::stream::generated::FileTextMessageSnapshot
{
    let c = configured(root); let r = recipe(full);
    FileOversight::read_decoder_text_message(&c.store, &c.profile,
        &r.decoder, &r.tokenizer, r.stream, 1).unwrap()
}
fn position(root: &Root) -> fa_reference::action::consequence::oversight::decoder_host::HostedDecoderInspection {
    let c = configured(root); let r = loaded(false);
    FileOversight::read_decoder_text_progress(&c.store, &c.profile, &r.decoder, &r.tokenizer, 7)
        .unwrap().numerical.numerical
}

#[test]
fn finish_is_a_new_reviewed_effect_without_another_message_or_sampler_draw() {
    for full in [false, true] {
        let root = Root::new(); let first = initial(&root, ReviewDecision::Approve, full);
        let before = position(&root);
        assert!(!native(&root, full).stream.confirmed.finished()); // EOS was not closure.
        let c = closing_config(&root); let p = peers(&root, &c);
        let human = review(p.clone(), 2, ReviewDecision::Approve);
        let result = run(c, recipe(full), &p, 2, || ElapsedTick(1001)).unwrap();
        let packet = human.join().unwrap();
        assert!(result.failure.is_none(), "{:?}", result.failure); assert!(executed(&result));
        assert_eq!(result.cleanup_pending, 0);
        let frame = ReleaseFrame::decode(&packet.action().spec().payload).unwrap();
        assert!(frame.is_finish()); assert_eq!(frame.message(), None); assert_eq!(frame.prior_messages(), &["A"]);
        assert_ne!(first.binding().request, packet.binding().request);
        assert_ne!(first.binding().session, packet.binding().session);
        assert_eq!(position(&root), before);
        let state = native(&root, full).stream;
        assert!(state.confirmed.finished()); assert_eq!(state.confirmed, state.published);
        assert_eq!(state.confirmed.visible(), b"A"); assert_eq!(state.confirmed.message_count(), 1);
        assert_eq!(state.publication.executions, 2); assert_eq!(state.publication.control.ledger.reserved, 0);
        assert_eq!(state.publication.control.ledger.charged,
            first.action().spec().units + packet.action().spec().units);
        let c = configured(&root); let (host, _) = recovery::open(&c, &recipe(full)).unwrap();
        assert!(host.prepare_decoder_text_continuation(1, 8, loaded(false).text).is_err());
        assert!(host.prepare_decoder_text_finish(1, 3, ElapsedTick(5000)).is_err());
    }
}

#[test]
fn rejecting_closure_keeps_the_previous_disclosure_and_never_calls_it_finished() {
    let root = Root::new(); let first = initial(&root, ReviewDecision::Approve, false);
    let before = position(&root); let c = closing_config(&root); let p = peers(&root, &c);
    let human = review(p.clone(), 2, ReviewDecision::Reject);
    let result = run(c, recipe(false), &p, 2, || ElapsedTick(1001)).unwrap();
    let packet = human.join().unwrap();
    assert!(ReleaseFrame::decode(&packet.action().spec().payload).unwrap().is_finish());
    assert!(result.failure.is_none()); assert!(!executed(&result)); assert_eq!(result.cleanup_pending, 0);
    assert!(emit(result, &mut Vec::new()).is_err());
    let state = native(&root, false).stream;
    assert!(!state.confirmed.finished()); assert!(!state.published.finished());
    assert_eq!(state.confirmed.visible(), b"A"); assert_eq!(state.publication.executions, 1);
    assert_eq!(state.publication.control.ledger.charged, first.action().spec().units);
    assert_eq!(state.publication.control.ledger.reserved, 0); assert_eq!(position(&root), before);
}

#[test]
fn finish_requires_the_exact_source_and_never_accepts_a_nearby_recipe_or_used_id() {
    for mutation in 0..5 {
        let root = Root::new(); initial(&root, ReviewDecision::Approve, false);
        let before = position(&root); let mut c = configured(&root); c.programs.clear();
        let p = peers(&root, &c); let mut r = recipe(false); let mut id = 2;
        match mutation { 0 => r.generation += 1, 1 => r.text.prompt.push(b'y'),
            2 => r.text.generation.scalar_products -= 1, 3 => r.request = 99, _ => id = 1 }
        fs::remove_file(root.0.join("evidence.json")).unwrap();
        assert!(run(c, r, &p, id, || ElapsedTick(1001)).is_err());
        assert!(!p.socket(id).exists()); assert_eq!(position(&root), before);
        let state = native(&root, false).stream;
        assert_eq!(state.publication.executions, 1); assert!(!state.confirmed.finished());
    }
}

#[test]
fn missing_source_cannot_turn_existing_messages_into_an_unreviewed_finish() {
    let root = Root::new(); let first = initial(&root, ReviewDecision::Approve, false);
    let before = position(&root); let mut c = closing_config(&root); c.programs.clear();
    let p = peers(&root, &c); fs::remove_file(root.0.join("evidence.json")).unwrap();
    let result = run(c, recipe(false), &p, 2, || ElapsedTick(1001)).unwrap();
    assert!(result.failure.is_some()); assert!(!executed(&result)); assert_eq!(result.cleanup_pending, 0);
    assert!(!p.socket(2).exists()); assert_eq!(position(&root), before);
    let state = native(&root, false).stream;
    assert!(state.publication.stop.is_some()); // Stop is distinct from a finish frame.
    assert!(!state.published.finished()); assert!(!state.confirmed.finished());
    assert_eq!(state.publication.executions, 1);
    assert_eq!(state.publication.control.ledger.charged, first.action().spec().units);
}

#[test]
fn rejected_predecessor_cannot_be_closed_as_a_confirmed_native_message() {
    let root = Root::new(); initial(&root, ReviewDecision::Reject, false);
    let before = position(&root); let mut c = configured(&root); c.programs.clear(); let p = peers(&root, &c);
    assert!(run(c, recipe(false), &p, 2, || ElapsedTick(1001)).is_err());
    assert_eq!(position(&root), before); let state = native(&root, false).stream;
    assert_eq!(state.publication.executions, 0); assert!(state.confirmed.visible().is_empty());
    assert!(!state.confirmed.finished());
}

#[test]
fn finish_option_is_explicit_and_cannot_mix_with_generation_or_receipt_modes() {
    for id in ["0", "-1", "+1", "", "18446744073709551616"] {
        let args = ["create-generated", "missing-config", "missing-recipe", "missing-peer", "--finish", id]
            .map(str::to_owned);
        assert_eq!(super::super::command(&args, None).unwrap_err(), USAGE);
    }
    for args in [vec!["create-generated", "config", "recipe", "--resume", "--finish", "2"],
        vec!["create-generated", "config", "recipe", "peer", "--finish", "2", "--continue-generation"],
        vec!["create-generated", "config", "recipe", "peer", "--after", "1", "--finish", "2"]] {
        assert_eq!(super::super::command(&args.into_iter().map(str::to_owned).collect::<Vec<_>>(), None).unwrap_err(), USAGE);
    }
    let args = ["create-generated", "config", "recipe", "peer", "--finish", "2"].map(str::to_owned);
    assert_eq!(super::super::command(&args, Some(Path::new("qualification"))).unwrap_err(), USAGE);
    // Valid syntax reaches file loading, not the create/continue mode fallback.
    assert_ne!(super::super::command(&args, None).unwrap_err(), USAGE);
}
