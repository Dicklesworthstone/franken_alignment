//! Per-release prechecks and the independent operator retain separate finite
//! grants on one original owner, across actual native Message/Message/Finish.
use super::*;
use super::super::precheck_tests as exact;
use std::{os::unix::fs::MetadataExt, sync::mpsc};

#[test]
fn resident_prechecks_preserve_one_spent_operator_grant_through_messages_and_finish() {
    let root = Root::new();
    let (config, mut entries, reviewer) = selected(&root, true);
    let sockets = exact::select(&root, &mut entries, 0);
    let reader = Reader::new(&config, &entries[0]);
    let chosen = selection(&root, &entries[0].actor, 1, false);
    let settings = chosen.settings.clone();
    let actor_profile = entries[0].actor.clone();
    let profiles = entries
        .iter()
        .map(|entry| entry.actor.clone())
        .collect::<Vec<_>>();
    let store = reader.store.clone();
    let profile = reader.profile.clone();
    let mut output = exact::CheckedOutput::new(&entries, &reader, &reviewer, 0);
    let observed_sockets = sockets.clone();
    let (offered, offers) = mpsc::channel();
    let (release, decisions) = mpsc::channel();
    let human = held_reviews(reviewer.clone(), REQUESTS.to_vec(), offered, decisions);
    let observer = thread::spawn(move || {
        let mut original_inode = None;
        for (index, selected_request) in REQUESTS.into_iter().enumerate() {
            let (request_id, packet) = offers.recv_timeout(Duration::from_secs(20)).unwrap();
            assert_eq!(request_id, selected_request);
            assert_eq!(packet.action().spec().policy_epoch, 0);
            assert!(observed_sockets.iter().flatten().all(|socket| !socket.exists()));
            assert_eq!(
                FileOversight::read_publication(&store, &profile)
                    .unwrap()
                    .executions,
                index as u64,
                "neither finite disclosure supplies the independent human key"
            );
            let mut socket = connect(&settings);
            let inode = std::fs::metadata(&settings.socket).unwrap().ino();
            if let Some(original) = original_inode {
                assert_eq!(inode, original, "operator custody changed at release handoff");
            } else {
                original_inode = Some(inode);
            }
            let observed = exchange(
                &mut socket,
                &request(
                    &actor_profile,
                    &format!("with-prechecks-{index}"),
                    Verb::Situation,
                ),
                settings.response_bytes,
            );
            if index == 0 {
                known(&observed);
                assert_eq!(summary(&observed, "executions"), 0);
                assert_eq!(summary(&observed, "pending_attempts"), 1);
            } else {
                assert_eq!(
                    field(&observed, &["envelope", "reason_code"]).as_str(),
                    Some("BudgetExhausted"),
                    "a fresh actor precheck grant must not renew the original operator grant"
                );
            }
            drop(socket);
            release.send(Some(ReviewDecision::Approve)).unwrap();
        }
    });
    let result = run_with_operator(
        config,
        entries,
        &reviewer,
        None,
        (None, Some(&chosen)),
        || ElapsedTick(1000),
        &mut output,
    );
    assert!(result.is_ok(), "{result:?}");
    observer.join().unwrap();
    let packets = human.join().unwrap();
    assert_eq!(packets.len(), 3);
    assert_eq!(output.documents.len(), 3);
    let packets = std::array::from_fn(|index| Some(packets[index].clone()));
    complete(&reader, &output.join(), &packets);
    assert!(sockets.iter().flatten().all(|socket| !socket.exists()));
    assert!(!chosen.settings.socket.exists());
    closed(&profiles, &reviewer);
}
