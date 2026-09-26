//! Actual multi-message numerical/file/socket workflow; synthetic weights and votes.
use super::*;
use crate::config::CLOCK_DOMAIN;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::{
    client::ReviewClientProgress, wire::{ReviewDecision, ReviewPacket},
};
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::{
    FilePublicationInputs, FileWitnessInput, producer::{FilePublicationProducer, PublicationProducerProfile},
};
use fa_reference::action::consequence::delivery::stream::ReleaseFrame;
use fa_reference::action::consequence::oversight::{
    actor::{ActorOutcome, Knowledge, UnknownReason},
    actor_wire::{decode_command, WireResponse, client::{ActorExchange, ClientIoBudget, ClientIoLimits, ClientProgress}},
    helper_client::{HelperClient, ClientPhase}, helper_processes::HelperProgram,
};
use fa_reference::product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker};
use fa_reference::witness::{AdapterDomainInput, DomainClosure, DomainProjection, SnapshotEntry};
use fa_reference::round::Verdict;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::os::unix::net::UnixStream;
use std::sync::mpsc;

mod fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/examples/supervise_publication/workflow/actor_service/native/tests/fixture.rs"));
}
use fixture::{Root, inputs, profiles};
const CHILD: &str = "workflow::actor_service::native::continuation::tests::synthetic_continuation_helper_process";
const MEMBER: &str = "FA_NATIVE_CONTINUATION_MEMBER";
const EPOCH: &str = "FA_NATIVE_CONTINUATION_EPOCH";

fn configured(root: &Root) -> Config {
    let mut config = fixture::configured(root);
    config.profile.delivery.total = 4096;
    let epoch = if config.store.exists() {
        FileOversight::read_publication(&config.store, &config.profile).unwrap().control.ledger.epoch + 1
    } else { 0 };
    config.programs = ["alpha", "beta"].into_iter().map(|member| (member.to_owned(),
        HelperProgram::new(std::env::current_exe().unwrap(), root.0.clone(),
            vec!["--exact".into(), CHILD.into(), "--nocapture".into()],
            BTreeMap::from([(OsString::from(MEMBER), OsString::from(member)),
                (OsString::from(EPOCH), OsString::from(epoch.to_string()))])).unwrap())).collect();
    config
}

#[test]
fn synthetic_continuation_helper_process() {
    let Ok(member) = std::env::var(MEMBER) else { return; };
    let epoch = std::env::var(EPOCH).unwrap().parse().unwrap();
    let config = Config::decode(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/fixtures/supervised_publication.json"))).unwrap();
    let mut client = HelperClient::from_process_stdin(config.profile.committee.members()[&member].profile_at(epoch)).unwrap();
    let start = Instant::now();
    loop {
        assert!(start.elapsed() < Duration::from_secs(10));
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference {
            let input = client.input().unwrap();
            assert_eq!(input.member(), member);
            let own = format!("review this fixture: {member}");
            assert!(input.actual_input().submitted_bytes().windows(own.len()).any(|p| p == own.as_bytes()));
            client.respond(Verdict::Allow, member.as_bytes()).unwrap();
        }
        if client.phase() == ClientPhase::ReplySent { return; }
        pause(1);
    }
}

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
        let document = receiver.recv_timeout(Duration::from_secs(10)).unwrap();
        let mut command = decode_command(&document).unwrap();
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
                    ClientProgress::Complete => panic!("missing actor response"),
                    _ => pause(1),
                }
            };
            if !matches!(response.result, Ok(Knowledge::Pending { .. }
                | Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown })) { return response; }
            socket = exchange.into_stream().unwrap(); command = Command::Poll { request: profile.request }; pause(1);
        }
    })
}
fn human(profile: PeerProfile, request: u64, config: &Config, decision: ReviewDecision)
    -> std::thread::JoinHandle<ReviewPacket>
{
    let store = config.store.clone(); let bootstrap = config.profile.clone();
    let prior = if store.exists() { FileOversight::read_publication(&store, &bootstrap).unwrap().executions } else { 0 };
    std::thread::spawn(move || {
        let start = Instant::now();
        while !profile.socket(request).exists() { assert!(start.elapsed() < Duration::from_secs(10)); pause(1); }
        let mut client = profile.connect_client(request).unwrap(); let mut packet = None;
        loop {
            assert!(start.elapsed() < Duration::from_secs(12));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    assert_eq!(FileOversight::read_publication(&store, &bootstrap).unwrap().executions, prior);
                    packet = Some(client.packet().unwrap().clone()); client.respond(decision).unwrap();
                }
                ReviewClientProgress::Complete => return packet.unwrap(),
                _ => pause(1),
            }
        }
    })
}
fn executed(response: &WireResponse) -> bool {
    matches!(response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. }))
}
fn cycle<F>(root: &Root, publication: Option<&PublicationProfile>, request: u64, generation: u64,
    after: Option<u64>, decision: ReviewDecision, time: F)
    -> (Result<(), String>, Vec<u8>, WireResponse, ReviewPacket)
where F: FnMut() -> ElapsedTick {
    let config = configured(root); let mut native = inputs(root, &config); native.generation = generation;
    let (mut peer, reviewer) = profiles(root, &config); peer.request = request;
    let open = config.store.exists();
    let (sender, receiver) = mpsc::channel(); let client = actor(peer.clone(), receiver);
    let review = human(reviewer.clone(), request, &config, decision);
    let mut output = Output { bytes: Vec::new(), sender: Some(sender) };
    let result = serve_continued(config, &peer, &reviewer, native,
        (open, publication, None, after), time, &mut output);
    let packet = review.join().unwrap(); let response = client.join().unwrap();
    (result, output.bytes, response, packet)
}
fn witness_inputs(config: &Config, revision: u64, phantom: bool) -> FilePublicationInputs {
    let key = ProjectionKey { source: 40, branch: config.profile.delivery.scope.branch, projection: 7, source_epoch: 1 };
    let close = TrustedClosingMarker { key, final_sequence: 1, marker_generation: 1 };
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap(); frontiers.record_close(close).unwrap();
    let keys = if phantom { vec![0, 1] } else { vec![0, 99] };
    FilePublicationInputs::new(Some(FileWitnessInput::new(revision, revision, 20,
        AdapterDomainInput::new(DomainProjection::new(40, 1, key), DomainClosure::Closed(close)),
        keys.into_iter().map(|key| SnapshotEntry::new(key, 1, b"original".to_vec()).unwrap()).collect(),
        &frontiers).unwrap()), None)
}
fn publication(root: &Root, joint: bool) -> (PublicationProfile, FilePublicationProducer) {
    let config = configured(root); let scope = config.profile.delivery.scope;
    let producer_profile = PublicationProducerProfile { source: 91, scope, feed: 41, clock_domain: CLOCK_DOMAIN, after: 0 };
    let (producer, _) = FilePublicationProducer::create(root.0.join("producer"), producer_profile,
        witness_inputs(&config, 1, false), ElapsedTick(1000)).unwrap();
    let inner = format!(r#"{{"schema":"fa.supervised-witnesses/3","source":91,"producer":{{"path":"{}/producer/delivery.bin","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}}}},"feed":{{"source":41,"after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}},"limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[{{"kind":"exact_value","key":0,"role":"subject"}},{{"kind":"absent_key","key":1}}]}}"#,
        root.0.display(), scope.tenant, scope.principal, scope.run, scope.branch, scope.authority);
    let json = if joint { format!(r#"{{"schema":"fa.supervised-joint-publication/1","joint":{{"id":70,"generation":1,"minimum_safe_roots":1,"minimum_violation_roots":1,"maximum_escape_ppm":0,"maximum_false_stop_ppm":0,"max_cases":4,"max_member_outcomes":16}},"publication":{inner}}}"#) } else { inner };
    (PublicationProfile::decode(json.as_bytes()).unwrap(), producer)
}

#[test]
fn native_continuation_second_message_keeps_history_and_requires_a_new_human_key() {
    for mode in 0..3 {
        for approve in [false, true] {
            let root = Root::new();
            let selected = (mode != 0).then(|| publication(&root, mode == 2));
            let publication = selected.as_ref().map(|(profile, _)| profile);
            let (result, _, response, first) = cycle(&root, publication, 91, 7, None,
                ReviewDecision::Approve, || ElapsedTick(1000));
            assert!(result.is_ok(), "{result:?}"); assert!(executed(&response));
            let (result, _, response, second) = cycle(&root, publication, 92, 8, Some(91),
                if approve { ReviewDecision::Approve } else { ReviewDecision::Reject }, || ElapsedTick(1001));
            assert!(result.is_ok(), "{result:?}"); assert_eq!(executed(&response), approve);
            let frame = ReleaseFrame::decode(&second.action().spec().payload).unwrap();
            assert_eq!(frame.prior_messages(), &["A"]); assert_eq!(frame.message(), Some("A"));
            let config = configured(&root); let native = inputs(&root, &config);
            let state = FileOversight::read_stream_publication(&config.store, &config.profile, native.stream).unwrap();
            assert_eq!(state.published.messages().collect::<Vec<_>>(), if approve { vec!["A", "A"] } else { vec!["A"] });
            assert_eq!(state.confirmed, state.published);
            assert_eq!(state.publication.control.ledger.charged,
                first.action().spec().units + if approve { second.action().spec().units } else { 0 });
            assert_eq!(state.publication.control.ledger.reserved, 0);
            let progress = FileOversight::read_decoder_text_progress(&config.store, &config.profile,
                &native.decoder, &native.tokenizer, 8).unwrap();
            assert_eq!(progress.text.command().position(), 4);
            assert_eq!(progress.numerical.numerical.position, 8);
            assert_eq!(progress.numerical.numerical.sampled_draws, 4);
            assert_eq!(progress.text.generation_revision(), 4);
        }
    }
}

#[test]
fn native_continuation_interrupted_intent_uses_existing_recovery_without_restarting_the_prompt() {
    let root = Root::new();
    let (result, _, response, _) = cycle(&root, None, 91, 7, None, ReviewDecision::Approve, || ElapsedTick(1000));
    assert!(result.is_ok(), "{result:?}"); assert!(executed(&response));
    let mut config = configured(&root); let mut native = inputs(&root, &config); native.generation = 8;
    let (mut host, _) = FileOversight::open_generated_text_stream_with_reserve(&config.store, config.profile.clone(),
        native.stream, &native.decoder, &native.tokenizer, RecoveryReserve::terminal()).unwrap();
    let prepared = host.prepare_decoder_text_continuation(91, 8, native.request.clone()).unwrap();
    host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1001)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1001)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    host.resume_decoder(host.revision(), n.actor_revision, n.position).unwrap();
    host.begin_decoder_text_continuation(host.revision(), 91, prepared.clone()).unwrap();
    host.advance_decoder_text(host.revision(), 8, 0).unwrap(); drop(host);
    let config = configured(&root); let (mut actor, reviewer) = profiles(&root, &config); actor.request = 92;
    let mut output = Vec::new();
    assert!(serve_continued(config, &actor, &reviewer, native, (true, None, None, Some(91)),
        || ElapsedTick(1002), &mut output).is_err());
    assert!(output.is_empty()); // duplicate initiation refuses; do not choose another ID
    let (result, _, response, _) = cycle(&root, None, 92, 8, None, ReviewDecision::Approve, || ElapsedTick(1002));
    assert!(result.is_ok(), "{result:?}"); assert!(executed(&response));
    let config = configured(&root); let native = inputs(&root, &config);
    let state = FileOversight::read_decoder_text_progress(&config.store, &config.profile,
        &native.decoder, &native.tokenizer, 8).unwrap();
    assert_eq!(state.text.command(), &prepared);
    assert_eq!(state.text.generation_revision(), 4); assert_eq!(state.numerical.numerical.position, 8);
    assert_eq!(state.publication.executions, 2);
}

#[test]
fn native_continuation_receipt_recovery_preserves_the_second_document_without_live_sources() {
    let root = Root::new(); let (publication, producer) = publication(&root, false);
    let (result, _, _, _) = cycle(&root, Some(&publication), 91, 7, None, ReviewDecision::Approve, || ElapsedTick(1000));
    assert!(result.is_ok(), "{result:?}");
    let (result, document, response, _) = cycle(&root, Some(&publication), 92, 8, Some(91),
        ReviewDecision::Approve, || ElapsedTick(1001));
    assert!(result.is_ok(), "{result:?}"); assert!(executed(&response)); drop(producer);
    let mut config = configured(&root); let mut native = inputs(&root, &config); native.generation = 8;
    let (mut peer, reviewer) = profiles(&root, &config); peer.request = 92;
    let store = config.store.clone(); let bootstrap = config.profile.clone();
    let decoder = native.decoder.clone(); let tokenizer = native.tokenizer.clone();
    config.programs.clear();
    std::fs::remove_file(root.0.join("producer/delivery.bin")).unwrap();
    std::fs::remove_file(root.0.join("evidence.json")).unwrap();
    let (sender, receiver) = mpsc::channel(); let client = actor(peer.clone(), receiver);
    let mut output = Output { bytes: Vec::new(), sender: Some(sender) };
    let result = serve_selected(config, &peer, &reviewer, native, (true, Some(&publication)),
        || ElapsedTick(20000), &mut output);
    assert!(result.is_ok(), "{result:?}"); assert!(executed(&client.join().unwrap()));
    assert_eq!(output.bytes, document);
    let state = FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 8).unwrap();
    assert!(state.numerical.paused); assert_eq!(state.numerical.numerical.position, 8);
    assert_eq!(state.publication.executions, 2); assert!(!reviewer.socket(92).exists());
}

#[test]
fn native_continuation_rechecks_witnesses_after_second_dispatch() {
    for phantom in [false, true] {
        let root = Root::new(); let (publication, mut producer) = publication(&root, false);
        let (result, _, _, _) = cycle(&root, Some(&publication), 91, 7, None, ReviewDecision::Approve, || ElapsedTick(1000));
        assert!(result.is_ok(), "{result:?}");
        let config = configured(&root);
        let prior = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        let mut changed = false;
        let (result, _, response, _) = cycle(&root, Some(&publication), 92, 8, Some(91), ReviewDecision::Approve, || {
            if !changed && let Ok(state) = FileOversight::read_publication(&config.store, &config.profile)
                && state.control.ledger.charged > prior.control.ledger.charged && state.executions == 1 {
                producer.publish(1, witness_inputs(&config, 2, phantom), ElapsedTick(1001)).unwrap(); changed = true;
            }
            ElapsedTick(1001)
        });
        assert!(changed); assert_eq!(executed(&response), !phantom, "{result:?}");
        let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(state.executions, if phantom { 1 } else { 2 });
        assert_eq!(state.control.ledger.reserved, 0);
        if phantom { assert_eq!(state.payload, prior.payload); assert_eq!(state.control.ledger.charged, prior.control.ledger.charged); }
        else { assert!(result.is_ok(), "{result:?}"); }
    }
}

#[test]
fn native_continuation_missing_intent_never_implicitly_starts_and_source_loss_preserves_prior_message() {
    let root = Root::new();
    let (result, _, _, _) = cycle(&root, None, 91, 7, None, ReviewDecision::Approve, || ElapsedTick(1000));
    assert!(result.is_ok(), "{result:?}");
    let config = configured(&root); let mut native = inputs(&root, &config); native.generation = 8;
    let (mut actor, reviewer) = profiles(&root, &config); actor.request = 92;
    let mut output = Vec::new();
    assert!(serve_mode(config, &actor, &reviewer, native, true, || ElapsedTick(1001), &mut output).is_err());
    assert!(output.is_empty());
    let config = configured(&root); let mut native = inputs(&root, &config); native.generation = 8;
    let store = config.store.clone(); let bootstrap = config.profile.clone();
    let decoder = native.decoder.clone(); let tokenizer = native.tokenizer.clone();
    std::fs::remove_file(root.0.join("evidence.json")).unwrap();
    assert!(serve_continued(config, &actor, &reviewer, native, (true, None, None, Some(91)),
        || ElapsedTick(1001), &mut output).is_err());
    assert!(output.is_empty());
    let prior = FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 7).unwrap();
    assert_eq!(prior.numerical.numerical.position, 4); assert_eq!(prior.publication.executions, 1);
    assert!(prior.publication.stop.is_some()); assert!(!reviewer.socket(92).exists());
}

#[test]
fn native_continuation_option_is_explicit_strict_and_reaches_the_existing_native_parser() {
    let good: Vec<String> = ["serve-open", "config", "actor", "reviewer", "--native-text", "recipe", FLAG, "91"]
        .into_iter().map(str::to_owned).collect();
    let (prefix, id) = take_option(&good).unwrap(); assert_eq!(id, Some(91)); assert_eq!(prefix, &good[..6]);
    for value in ["0", "-1", "+1", "", "1x", "18446744073709551616"] {
        let mut bad = good.clone(); bad[7] = value.into(); assert!(take_option(&bad).is_err());
    }
    for mode in ["serve-create", "serve-create-checked", "actor-submit", "review-peer"] {
        let mut bad = good.clone(); bad[0] = mode.into(); assert!(take_option(&bad).is_err());
    }
    let mut duplicate = good.clone(); duplicate.extend([FLAG.into(), "92".into()]); assert!(take_option(&duplicate).is_err());
    assert!(take_option(&good[..7]).is_err());
    let mut reordered = good.clone(); reordered.swap(4, 6); assert!(take_option(&reordered).is_err());
    // Global qualification parsing removes only its own option and keeps the
    // explicit continuation suffix, under both original native open modes.
    for checked in [false, true] {
        let mut args: Vec<String> = if checked {
            ["serve-open-checked", "missing", "actor", "reviewer", "witness", "--native-text", "recipe"].into_iter().map(str::to_owned).collect()
        } else { good[..6].to_vec() };
        args.extend([FLAG.into(), "91".into(), "--credibility-activation".into(), "capsule".into()]);
        assert!(crate::qualification::take_option(&mut args).unwrap().is_some());
        assert_eq!(take_option(&args).unwrap().1, Some(91));
        assert_ne!(super::super::command(&args, None).unwrap_err(), USAGE); // configuration read, not accidental fallback
    }
}
