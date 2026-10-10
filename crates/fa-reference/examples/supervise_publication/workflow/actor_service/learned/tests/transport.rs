//! Actual authenticated peer transport; a denied Unix socket remains a failure.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::{
    client::ReviewClientProgress, wire::{ReviewDecision, ReviewPacket},
};
use fa_reference::action::consequence::delivery::stream::ReleaseFrame;
use std::thread;

fn reviewer(peers: PeerProfile) -> thread::JoinHandle<ReviewPacket> {
    thread::spawn(move || {
        let started = Instant::now();
        let mut client = peers.connect_client(1).unwrap(); let mut packet = None;
        loop {
            assert!(started.elapsed() < Duration::from_secs(15), "learned human review timed out");
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    assert!(packet.is_none());
                    packet = Some(client.packet().unwrap().clone());
                    client.respond(ReviewDecision::Approve).unwrap();
                }
                ReviewClientProgress::Complete => return packet.unwrap(),
                _ => thread::sleep(Duration::from_millis(1)),
            }
        }
    })
}

#[test]
fn learned_command_real_peer_review_publishes_original_native_generated_message() {
    let root = Root::new(); let config = configured(&root);
    let path = fixture::write(&root, b"allow"); evidence(&root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    let generation = loaded.generation.clone(); let profile = config.profile.clone(); let store = config.store.clone();
    let peers = fixture::peers(&root, &config); let mut human = None;
    let result = run(config, loaded, &peers, false, || {
        // Start the client only after the real human endpoint exists. If this
        // environment denies listener creation, run returns that real error
        // and no detached client thread is left waiting for an impossible offer.
        if human.is_none() && peers.socket(1).exists() { human = Some(reviewer(peers.clone())); }
        ElapsedTick(1000)
    }).unwrap();
    let packet = human.expect("actual peer reviewer connected").join().unwrap();
    assert!(result.failure.is_none(), "{:?}", result.failure); assert_eq!(result.cleanup_pending, 0);
    assert!(matches!(result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert_eq!(packet.views().len(), 2);
    assert_eq!(ReleaseFrame::decode(&packet.action().spec().payload).unwrap().message(), Some("A"));
    let state = FileOversight::read_publication_with_learned_generation(&store, &profile, &generation).unwrap();
    assert_eq!(state.executions, 1); assert_eq!(state.payload, b"A");
    assert_eq!(state.control.ledger.charged, packet.action().spec().payload.len() as u64);
    assert_eq!(state.control.ledger.reserved, 0);
}
