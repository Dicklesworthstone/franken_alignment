//! The source contract persists across original resident requests, not cached
//! approvals or an assumption that the first message qualified the next one.
use super::*;
use super::super::super::sequence as resident;
use crate::publication::PublicationProfile;
use crate::workflow::actor_service::Profile;
use fa_reference::action::consequence::oversight::{actor::UnknownReason,
    actor_wire::{WireResponse, client::{ActorClientState, ClientProgress,
        ClientSessionLimits, drive::ClientDriveBudget}}};
use std::{collections::BTreeSet, os::unix::net::UnixStream, thread, time::{Duration, Instant}};

// A poisoned original owner can report only unavailable knowledge. Require that
// exact wire observation; a lost socket is never accepted as an equivalent reply.
fn actor_until_unavailable(profile: Profile, due: ElapsedTick) -> thread::JoinHandle<WireResponse> {
    thread::spawn(move || {
        let started = Instant::now();
        let socket = UnixStream::connect(&profile.socket).unwrap();
        profile.supervisor.verify(&socket).unwrap();
        let mut client = ActorClientState::new(ClientSessionLimits::default()).unwrap()
            .connect_unix(socket).unwrap();
        let proposal = FileLearnedTextStreamActorPort::encode_release(profile.request,
            LearnedTextRelease::Message, due).unwrap();
        client.submit(profile.request, &proposal).unwrap();
        loop {
            assert!(started.elapsed() < Duration::from_secs(25), "original unavailable receipt timed out");
            match client.drive(ClientDriveBudget::default()).unwrap().progress {
                ClientProgress::Response(response) => {
                    if matches!(&response.result, Ok(Knowledge::Unknown {
                        reason: UnknownReason::ControllerUnavailable })) { return response; }
                    assert!(matches!(&response.result, Ok(Knowledge::Pending { .. })), "{response:?}");
                    thread::sleep(Duration::from_millis(5));
                    client.poll(profile.request).unwrap();
                }
                ClientProgress::Complete => panic!("original actor exchange completed without its unavailable receipt"),
                ClientProgress::Progress | ClientProgress::Blocked => {},
            }
            thread::sleep(Duration::from_millis(1));
        }
    })
}

#[test]
fn resident_messages_and_finish_reacquire_witnesses_and_preserve_the_first_effect_when_the_next_basis_fails() {
    for phantom in [false, true] {
        let root = Root::new(); let config = configured(&root); let first = native::write(&root);
        super::super::forecast::install(&root, &first, true, 60_000);
        let mut producer = provision(&root, &config, &first, true, false);
        evidence(&root, &config, 1, true, false);
        let initial = recipe::load(&first, &config, true).unwrap();
        let (_, second) = native::next(&root, &config, 2);
        let (_, last) = native::next(&root, &config, 3);
        let reader = Reader::new(&config, &initial);
        assert_eq!(initial.generation, second.generation); assert_eq!(initial.generation, last.generation);
        let entries = [initial, second, last].into_iter().enumerate().map(|(index, loaded)| {
            let mut actor = peers::actor_profile(&root, &config, loaded.request);
            actor.socket = root.0.join(format!("peers/checked-actor-{}.sock", loaded.request));
            resident::Entry { loaded, actor,
                release: if index == 2 { LearnedTextRelease::Finish } else { LearnedTextRelease::Message } }
        }).collect::<Vec<_>>();
        let actors = entries.iter().map(|entry| entry.actor.clone()).collect::<Vec<_>>();
        let reviewer = fixture::peers(&root, &config); let scope = config.profile.delivery.scope;
        let mut clients = [None, None, None]; let mut humans = [None, None, None];
        let mut first_cut = None;
        let started = std::time::Instant::now();
        let result = resident::run(config, entries, &reviewer, false, None, None, || {
            // A real second actor listener is reachable only after the first
            // confirmed receipt. Its next basis is selected before its review.
            if first_cut.is_none() && actors[1].socket.exists() {
                let state = reader.snapshot();
                assert_eq!(state.executions, 1); assert_eq!(state.payload, b"A");
                let keys: &[u64] = if phantom { &[0, 1, 2, 4] } else { &[0, 2, 4, 99] };
                producer.publish(1, structured(scope, 2, keys), ElapsedTick(1001)).unwrap();
                first_cut = Some(state);
            }
            for index in 0..3 {
                if clients[index].is_none() && actors[index].socket.exists() {
                    eprintln!("checked resident actor {} ready after {:?}; phantom={phantom}", index + 1, started.elapsed());
                    let due = ElapsedTick(50_000 - index as u64 * 1000);
                    clients[index] = Some(if phantom && index == 1 {
                        actor_until_unavailable(actors[index].clone(), due)
                    } else {
                        peers::actor(actors[index].clone(),
                            if index == 2 { LearnedTextRelease::Finish } else { LearnedTextRelease::Message },
                            due, peers::Behavior::Observe)
                    });
                }
                if humans[index].is_none() && reviewer.socket(index as u64 + 1).exists() {
                    eprintln!("checked resident reviewer {} ready after {:?}; phantom={phantom}", index + 1, started.elapsed());
                    humans[index] = Some(peers::human(reviewer.clone(), index as u64 + 1, ReviewDecision::Approve));
                }
            }
            ElapsedTick(if first_cut.is_some() { 1001 } else { 1000 })
        }).unwrap();
        eprintln!("checked resident completed after {:?}; phantom={phantom}; failure={:?}; responses={:?}",
            started.elapsed(), result.failure, result.responses);
        let first_cut = first_cut.expect("the first checked message was durably acknowledged");
        let responses = clients.map(|client| client.map(|client| client.join().unwrap()));
        let packets = humans.map(|human| human.map(|human| human.join().unwrap()));
        let first_packet = packets[0].as_ref().unwrap();
        assert_eq!(first_cut.control.ledger.charged, first_packet.action().spec().units);
        assert_eq!(result.cleanup_pending, 0);
        assert!(matches!(responses[0].as_ref().unwrap().result,
            Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
        assert_eq!(responses[0].as_ref().unwrap().result, result.responses[0].result);
        let state = reader.snapshot();
        assert_eq!(state.control.ledger.reserved, 0);
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
        if phantom {
            assert!(result.failure.is_some()); assert_eq!(result.responses.len(), 2);
            assert!(!result.failure.as_ref().unwrap().contains("actor response service failed"), "{:?}", result.failure);
            assert!(packets[1].is_none() && packets[2].is_none());
            assert!(responses[2].is_none());
            assert_eq!(responses[1].as_ref().unwrap().request, Some(2));
            assert_eq!(responses[1].as_ref().unwrap().result, result.responses[1].result);
            assert!(matches!(responses[1].as_ref().unwrap().result,
                Ok(Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable })));
            assert!(!matches!(responses[1].as_ref().unwrap().result,
                Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
            assert_eq!(state.executions, 1); assert_eq!(state.payload, first_cut.payload);
            assert_eq!(state.control.ledger.charged, first_cut.control.ledger.charged);
        } else {
            assert!(result.failure.is_none(), "{:?}", result.failure);
            assert_eq!(result.responses.len(), 3); assert_eq!(state.executions, 3);
            assert_eq!(state.payload, b"AA");
            let mut sessions = BTreeSet::new(); let mut charged = 0;
            for (index, (response, packet)) in responses.into_iter().zip(packets).enumerate() {
                let response = response.unwrap(); let packet = packet.unwrap();
                assert_eq!(response.result, result.responses[index].result);
                assert!(matches!(response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
                assert!(sessions.insert(packet.binding().session));
                let frame = ReleaseFrame::decode(&packet.action().spec().payload).unwrap();
                assert_eq!(frame.prior_messages(), vec!["A"; index.min(2)]);
                assert_eq!(frame.message(), if index < 2 { Some("A") } else { None });
                charged += packet.action().spec().units;
            }
            assert_eq!(state.control.ledger.charged, charged);
            let config = configured(&root); let loaded = recipe::load(&first, &config, false).unwrap();
            let (host, _, _, _) = recovery::open_selected(&config, &loaded).unwrap();
            assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 4);
            assert_eq!(host.learned_generation_inspection().unwrap().numerical.position, 6);
            let stream = host.stream_snapshot().unwrap();
            assert!(stream.confirmed.finished()); assert_eq!(stream.confirmed, stream.published);
            assert_eq!(stream.confirmed.messages().collect::<Vec<_>>(), ["A", "A"]);
            assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 3);
        }
    }
}

#[test]
fn resident_publication_selection_cannot_be_removed_or_replaced_before_startup() {
    let root = Root::new(); let config = configured(&root); let path = native::write(&root);
    let _producer = provision(&root, &config, &path, false, false);
    let (next, _) = native::next(&root, &config, 2);
    let profile_path = root.0.join("publication.json"); let original = fs::read_to_string(&profile_path).unwrap();
    let modified = original.replacen("\"bindings\":8", "\"bindings\":9", 1);
    for variant in 0..3 {
        fs::write(&profile_path, &original).unwrap();
        let first = recipe::load(&path, &config, true).unwrap();
        let mut second = recipe::load(&next, &config, true).unwrap();
        match variant {
            0 => second.publication = None,
            1 => second.publication = Some(PublicationProfile::decode(modified.as_bytes()).unwrap()),
            2 => {
                fs::write(&profile_path, &modified).unwrap();
                second = recipe::load(&next, &config, true).unwrap();
            }
            _ => unreachable!(),
        }
        let entries = [first, second].into_iter().map(|loaded| {
            let mut actor = peers::actor_profile(&root, &config, loaded.request);
            actor.socket = root.0.join(format!("peers/checked-actor-{}.sock", loaded.request));
            resident::Entry { loaded, actor, release: LearnedTextRelease::Message }
        }).collect();
        let reviewer = fixture::peers(&root, &config);
        let failure = resident::run(configured(&root), entries, &reviewer, false, None, None,
            || panic!("inconsistent checked sequence must refuse before sampling time")).err().unwrap();
        assert!(failure.contains(if variant == 2 { "exact original finite generation" }
            else { "differs from the exact numerical recipe" }), "{failure}");
        assert!(!config.store.exists()); assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    }
}

#[test]
fn checked_process_review_keeps_original_sidecar_inputs_and_cannot_replace_either_publication_key() {
    for decision in ["allow", "deny"] {
        let root = Root::new(); let config = configured(&root);
        let path = fixture::write(&root, b"allow");
        fs::write(&path, super::super::processes::process_recipe(&root, decision)).unwrap();
        let _producer = provision(&root, &config, &path, false, false);
        evidence(&root, &config, 1, true, false);
        let loaded = recipe::load(&path, &config, true).unwrap();
        assert!(loaded.process.is_some()); assert!(loaded.native.is_none());
        let reader = Reader::new(&config, &loaded);
        let (result, packet) = service(&root, config, loaded, None, None, ReviewDecision::Approve,
            &mut |_| ElapsedTick(1000));
        assert_eq!(confirmed(&result), decision == "allow", "{:?}", result.failure);
        assert_eq!(packet.is_some(), decision == "allow");
        for name in ["alpha", "beta"] {
            let actual = fs::read(root.0.join(format!("{name}.input"))).unwrap();
            assert!(actual.windows(8).any(|window| window == b"FASIDE\0\x01"),
                "publication capture must not replace the original sidecar helper input");
        }
        let state = reader.snapshot();
        assert_eq!(state.executions, u64::from(decision == "allow"));
        assert_eq!(state.control.ledger.reserved, 0);
        if let Some(packet) = packet { assert_eq!(state.control.ledger.charged, packet.action().spec().units); }
        else { assert_eq!(state.control.ledger.charged, 0); assert!(state.payload.is_empty()); }
    }
}
