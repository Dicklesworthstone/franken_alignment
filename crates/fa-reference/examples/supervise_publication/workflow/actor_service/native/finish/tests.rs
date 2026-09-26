//! Actual native generation, socket helpers, human decisions and finish receipts.
//! Synthetic model/ballots do not establish detector quality or process isolation.
use super::*;
use crate::config::CLOCK_DOMAIN;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::{
    client::ReviewClientProgress, wire::{ReviewDecision, ReviewPacket},
};
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::{
    FilePublicationInputs, FileWitnessInput, producer::{FilePublicationProducer, PublicationProducerProfile},
};
use fa_reference::action::consequence::delivery::stream::{ReleaseFrame, StreamProfile};
use fa_reference::action::consequence::oversight::{actor::{ActorOutcome, Knowledge, UnknownReason},
    actor_wire::{decode_command, WireResponse, client::{ActorExchange, ClientIoBudget, ClientIoLimits, ClientProgress}},
    helper_client::{HelperClient, ClientPhase}, helper_processes::HelperProgram};
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
use fixture::{Root, profiles};
const CHILD: &str = "workflow::actor_service::native::finish::tests::synthetic_finish_helper_process";
const MEMBER: &str = "FA_NATIVE_FINISH_MEMBER";
const EPOCH: &str = "FA_NATIVE_FINISH_EPOCH";

fn configured(root: &Root) -> Config {
    let mut config = fixture::configured(root); config.profile.delivery.total = 4096;
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
fn native(root: &Root, config: &Config, full: bool) -> Inputs {
    let mut input = fixture::inputs(root, config);
    if full {
        input.stream = StreamProfile::new(9, 1, 1, 1, 1).unwrap();
        input.request.max_output_bytes = 1;
    }
    input
}
#[test]
fn synthetic_finish_helper_process() {
    let Ok(member) = std::env::var(MEMBER) else { return; };
    let epoch = std::env::var(EPOCH).unwrap().parse().unwrap();
    let config = Config::decode(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/fixtures/supervised_publication.json"))).unwrap();
    let mut client = HelperClient::from_process_stdin(config.profile.committee.members()[&member].profile_at(epoch)).unwrap();
    let start = Instant::now();
    loop {
        assert!(start.elapsed() < Duration::from_secs(10)); client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference {
            let input = client.input().unwrap(); assert_eq!(input.member(), member);
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
fn actor(profile: Profile, receiver: mpsc::Receiver<Vec<u8>>, finish: bool) -> std::thread::JoinHandle<WireResponse> {
    std::thread::spawn(move || {
        let document = receiver.recv_timeout(Duration::from_secs(10)).unwrap();
        let mut command = decode_command(&document).unwrap();
        let Command::Submit { proposal, .. } = &command else { panic!("original Submit document"); };
        assert_eq!(proposal.payload.len(), FileGeneratedTextActorPort::INTENT_BYTES);
        assert_eq!(proposal.payload[16..].iter().all(|byte| *byte == 0), finish);
        let mut socket = UnixStream::connect(&profile.socket).unwrap();
        profile.supervisor.verify(&socket).unwrap(); socket.set_nonblocking(true).unwrap();
        let mut budget = ClientIoBudget::new(ClientIoLimits::default()).unwrap(); let start = Instant::now();
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
fn first(root: &Root, publication: Option<&PublicationProfile>, full: bool) -> ReviewPacket {
    let config = configured(root); let native = native(root, &config, full);
    let (peer, reviewer) = profiles(root, &config);
    let (sender, receiver) = mpsc::channel(); let client = actor(peer.clone(), receiver, false);
    let review = human(reviewer.clone(), peer.request, &config, ReviewDecision::Approve);
    let mut output = Output { bytes: Vec::new(), sender: Some(sender) };
    let result = serve_selected(config, &peer, &reviewer, native, (false, publication),
        || ElapsedTick(1000), &mut output);
    assert!(result.is_ok(), "{result:?}"); let packet = review.join().unwrap();
    assert!(executed(&client.join().unwrap())); packet
}
fn close<F>(root: &Root, publication: Option<&PublicationProfile>, full: bool, decision: ReviewDecision,
    time: F) -> (Result<(), String>, Vec<u8>, WireResponse, ReviewPacket)
where F: FnMut() -> ElapsedTick {
    let config = configured(root); let native = native(root, &config, full);
    let (mut peer, reviewer) = profiles(root, &config); peer.request = 92;
    let (sender, receiver) = mpsc::channel(); let client = actor(peer.clone(), receiver, true);
    let review = human(reviewer.clone(), 92, &config, decision);
    let mut output = Output { bytes: Vec::new(), sender: Some(sender) };
    let result = serve(config, &peer, &reviewer, native, (publication, None, 91), time, &mut output);
    let packet = review.join().unwrap(); let response = client.join().unwrap();
    (result, output.bytes, response, packet)
}
fn witness_inputs(config: &Config, revision: u64, phantom: bool) -> FilePublicationInputs {
    let key = ProjectionKey { source: 40, branch: config.profile.delivery.scope.branch, projection: 7, source_epoch: 1 };
    let close = TrustedClosingMarker { key, final_sequence: 1, marker_generation: 1 };
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap(); frontiers.record_close(close).unwrap();
    let keys = if revision == 1 { vec![0] } else if phantom { vec![0, 1] } else { vec![0, 99] };
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
fn native_finish_service_closes_full_stream_only_after_fresh_helpers_and_human_approval() {
    for mode in 0..3 {
        for approve in [false, true] {
            let root = Root::new(); let selected = (mode != 0).then(|| publication(&root, mode == 2));
            let publication = selected.as_ref().map(|(profile, _)| profile);
            let original = first(&root, publication, true);
            let config = configured(&root); let input = native(&root, &config, true);
            let before = FileOversight::read_decoder_text_progress(&config.store, &config.profile,
                &input.decoder, &input.tokenizer, 7).unwrap();
            let (result, document, response, reviewed) = close(&root, publication, true,
                if approve { ReviewDecision::Approve } else { ReviewDecision::Reject }, || ElapsedTick(1001));
            assert!(result.is_ok(), "{result:?}"); assert_eq!(executed(&response), approve);
            let frame = ReleaseFrame::decode(&reviewed.action().spec().payload).unwrap();
            assert!(frame.is_finish()); assert_eq!(frame.prior_messages(), &["A"]);
            let after = FileOversight::read_decoder_text_progress(&config.store, &config.profile,
                &input.decoder, &input.tokenizer, 7).unwrap();
            assert_eq!(after.numerical.numerical, before.numerical.numerical);
            assert_eq!(after.text.command(), before.text.command()); assert_eq!(after.text.generation_revision(), 4);
            let state = FileOversight::read_stream_publication(&config.store, &config.profile, input.stream).unwrap();
            assert_eq!(state.confirmed, state.published); assert_eq!(state.confirmed.finished(), approve);
            assert_eq!(state.confirmed.visible(), b"A"); assert_eq!(state.confirmed.message_count(), 1);
            assert_eq!(state.publication.executions, 1 + u64::from(approve));
            assert_eq!(state.publication.control.ledger.charged,
                original.action().spec().units + if approve { reviewed.action().spec().units } else { 0 });
            assert_eq!(state.publication.control.ledger.reserved, 0);
            assert_eq!(std::str::from_utf8(&document).unwrap().lines().count(), 1);
        }
    }
}

#[test]
fn native_finish_service_expired_receipts_skip_all_live_evidence_and_never_reclose() {
    for approve in [false, true] {
        let root = Root::new(); let (publication, producer) = publication(&root, true);
        first(&root, Some(&publication), false);
        let (result, original, response, _) = close(&root, Some(&publication), false,
            if approve { ReviewDecision::Approve } else { ReviewDecision::Reject }, || ElapsedTick(1001));
        assert!(result.is_ok(), "{result:?}"); assert_eq!(executed(&response), approve); drop(producer);
        let mut config = configured(&root); let native = native(&root, &config, false);
        let (mut peer, reviewer) = profiles(&root, &config); peer.request = 92;
        let store = config.store.clone(); let bootstrap = config.profile.clone(); let stream = native.stream;
        let decoder = native.decoder.clone(); let tokenizer = native.tokenizer.clone();
        let before = FileOversight::read_stream_publication(&store, &bootstrap, stream).unwrap();
        config.programs.clear(); std::fs::remove_file(root.0.join("evidence.json")).unwrap();
        std::fs::remove_file(root.0.join("producer/delivery.bin")).unwrap();
        let missing_qualification = root.0.join("absent-qualification.bin");
        let (sender, receiver) = mpsc::channel(); let client = actor(peer.clone(), receiver, true);
        let mut output = Output { bytes: Vec::new(), sender: Some(sender) };
        let result = serve(config, &peer, &reviewer, native, (Some(&publication), Some(&missing_qualification), 91),
            || ElapsedTick(20000), &mut output);
        assert!(result.is_ok(), "{result:?}"); assert_eq!(executed(&client.join().unwrap()), approve);
        assert_eq!(output.bytes, original);
        let after = FileOversight::read_stream_publication(&store, &bootstrap, stream).unwrap();
        assert_eq!(after.confirmed, before.confirmed); assert_eq!(after.published, before.published);
        assert_eq!(after.publication.executions, before.publication.executions);
        assert_eq!(after.publication.control.ledger.charged, before.publication.control.ledger.charged);
        assert!(FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 7).unwrap().numerical.paused);
        assert!(!reviewer.socket(92).exists()); assert!(!peer.socket.exists());
    }
}

#[test]
fn native_finish_service_final_witness_change_holds_closure_without_erasing_the_visible_prefix() {
    for phantom in [false, true] {
        let root = Root::new(); let (publication, mut producer) = publication(&root, false);
        let original = first(&root, Some(&publication), false);
        let config = configured(&root); let native = native(&root, &config, false);
        let mut changed = false;
        let (result, _, response, _) = close(&root, Some(&publication), false, ReviewDecision::Approve, || {
            if !changed && let Ok(state) = FileOversight::read_publication(&config.store, &config.profile)
                && state.control.ledger.charged > original.action().spec().units && state.executions == 1 {
                producer.publish(1, witness_inputs(&config, 2, phantom), ElapsedTick(1002)).unwrap();
                changed = true;
            }
            ElapsedTick(if changed { 1002 } else { 1001 })
        });
        assert!(changed); assert_eq!(executed(&response), !phantom, "{result:?}");
        let state = FileOversight::read_stream_publication(&config.store, &config.profile, native.stream).unwrap();
        assert_eq!(state.published.visible(), b"A"); assert_eq!(state.published.message_count(), 1);
        assert_eq!(state.published.finished(), !phantom);
        if phantom { assert_eq!(state.publication.control.ledger.charged, original.action().spec().units); }
        else { assert!(result.is_ok(), "{result:?}"); }
    }
}

#[test]
fn native_finish_service_missing_source_or_changed_original_intent_cannot_close_or_generate() {
    for cause in ["source", "prompt", "generation", "previous"] {
        let root = Root::new(); first(&root, None, false);
        let config = configured(&root); let mut native = native(&root, &config, false);
        let (mut peer, reviewer) = profiles(&root, &config); peer.request = 92;
        let store = config.store.clone(); let bootstrap = config.profile.clone(); let stream = native.stream;
        let decoder = native.decoder.clone(); let tokenizer = native.tokenizer.clone();
        match cause {
            "source" => std::fs::remove_file(root.0.join("evidence.json")).unwrap(),
            "prompt" => native.request.prompt = b"ba".to_vec(),
            "generation" => native.generation = 8,
            _ => {},
        }
        let mut output = Vec::new();
        assert!(serve(config, &peer, &reviewer, native, (None, None, if cause == "previous" { 90 } else { 91 }),
            || ElapsedTick(1001), &mut output).is_err());
        assert!(output.is_empty());
        let state = FileOversight::read_stream_publication(&store, &bootstrap, stream).unwrap();
        assert_eq!(state.publication.executions, 1); assert_eq!(state.published.visible(), b"A");
        assert!(!state.published.finished());
        let progress = FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 7).unwrap();
        assert_eq!(progress.numerical.numerical.position, 4); assert_eq!(progress.numerical.numerical.sampled_draws, 2);
        assert!(!reviewer.socket(92).exists()); assert!(!peer.socket.exists());
    }
}

#[test]
fn native_finish_service_broken_receipt_output_does_not_stop_or_repeat_an_existing_effect() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
        fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
    }
    let root = Root::new(); first(&root, None, false);
    let (result, _, response, _) = close(&root, None, false, ReviewDecision::Approve, || ElapsedTick(1001));
    assert!(result.is_ok(), "{result:?}"); assert!(executed(&response));
    let mut config = configured(&root); let native = native(&root, &config, false);
    let (mut peer, reviewer) = profiles(&root, &config); peer.request = 92;
    let store = config.store.clone(); let bootstrap = config.profile.clone(); let stream = native.stream;
    let before = FileOversight::read_stream_publication(&store, &bootstrap, stream).unwrap();
    config.programs.clear(); std::fs::remove_file(root.0.join("evidence.json")).unwrap();
    let result = serve(config, &peer, &reviewer, native, (None, None, 91), || ElapsedTick(20000), &mut Broken);
    assert!(result.unwrap_err().contains("source reference output failed"));
    let after = FileOversight::read_stream_publication(&store, &bootstrap, stream).unwrap();
    assert_eq!(after.published, before.published); assert_eq!(after.publication.executions, 2);
    assert_eq!(after.publication.stop.is_some(), before.publication.stop.is_some());
    assert_eq!(after.publication.control.ledger.charged, before.publication.control.ledger.charged);
}

#[test]
fn native_finish_service_option_is_explicit_exclusive_and_preserves_qualification_routing() {
    for mode in ["serve-open", "serve-open-checked"] {
        let mut args: Vec<String> = [mode, "config", "actor", "reviewer"].map(str::to_owned).to_vec();
        if mode.ends_with("checked") { args.push("witness".into()); }
        args.extend(["--native-text", "recipe", FLAG, "91"].map(str::to_owned));
        let (plain, id) = take_option(&args).unwrap(); assert_eq!(id, Some(91));
        assert_eq!(plain, &args[..args.len() - 2]);
        args.extend(["--credibility-activation", "activation"].map(str::to_owned));
        assert_eq!(crate::qualification::take_option(&mut args).unwrap(), Some("activation".into()));
        assert_eq!(take_option(&args).unwrap().1, Some(91));
        let complete = args.clone();
        for suffix in [vec![FLAG, "91"], vec!["--after-message", "91"]] {
            let mut invalid = complete.clone(); invalid.extend(suffix.into_iter().map(str::to_owned));
            assert!(take_option(&invalid).is_err());
        }
        for value in ["0", "-1", "+1", "", "18446744073709551616", "1x"] {
            let mut invalid = complete.clone(); *invalid.last_mut().unwrap() = value.into();
            assert!(take_option(&invalid).is_err());
        }
        let mut absent = complete.clone(); absent.pop(); assert!(take_option(&absent).is_err());
        for other in ["serve-create", "serve-create-checked", "actor-submit", "review-peer"] {
            let mut invalid = complete.clone(); invalid[0] = other.into(); assert!(take_option(&invalid).is_err());
        }
        // Both flags refuse in either order, rather than selecting the last mode.
        let mut invalid = complete.clone(); let split = invalid.len() - 2;
        invalid.splice(split..split, ["--after-message".into(), "91".into()]);
        assert!(take_option(&invalid).is_err());
    }
    assert!(Intent::Finish(91).check(false, 92).is_err());
    assert!(Intent::Finish(91).check(true, 91).is_err());
    assert!(Intent::Finish(91).check(true, 92).is_ok());
}
