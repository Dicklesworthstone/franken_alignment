//! The resident service must retain the same pair before its first request and
//! then reuse the corrected original owner for both Message and Finish.
use super::*;
use crate::workflow::actor_service::learned::sequence as service;
use fa_reference::action::consequence::delivery::stream::ReleaseFrame;

fn entries(
    root: &Root,
    config: &Config,
    input: &Archives,
    output: &Archives,
    generation: u64,
) -> (Vec<service::Entry>, [Profile; 2], PathBuf) {
    let (_, message) = load_recovery(root, config, 2, input, output, generation);
    let (last, finish) = native::next(root, config, 3);
    let mut actors = [
        peers::actor_profile(root, config, 2),
        peers::actor_profile(root, config, 3),
    ];
    for actor in &mut actors {
        actor.socket = root.0.join(format!("peers/resident-startup-{}.sock", actor.request));
    }
    let entries = vec![
        service::Entry {
            loaded: message,
            actor: actors[0].clone(),
            release: LearnedTextRelease::Message,
            precheck: None,
        },
        service::Entry {
            loaded: finish,
            actor: actors[1].clone(),
            release: LearnedTextRelease::Finish,
            precheck: None,
        },
    ];
    (entries, actors, last)
}

#[test]
fn resident_startup_retains_rebound_custody_for_corrected_message_and_finish_in_one_owner() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let (input, before) = first(&root, mode);
        let original = input.read();
        let output = Archives::named(&root, "resident-refused");
        write_next(&root, 2);
        let config = configured(&root);
        let (selected, _, last) = entries(&root, &config, &input, &output, 2);
        let reader = recipe::load(&last, &config, false).unwrap();
        let reviewers = fixture::peers(&root, &config);
        let parked = park_live_inputs(&root);
        let result = service::run(config, selected, &reviewers, true, Some(999), None,
            || ElapsedTick(1000));
        let error = result.err().expect("first resident request must reject the nonexistent predecessor");
        assert!(error.contains("Missing"), "{error}");
        let after = observed(&configured(&root), &reader);
        assert_unchanged_effects(&before, &after, 2, 3);
        assert_retained_cut(&output, &original, &after, true);
        assert_eq!(input.read(), original);
        for (path, _) in &parked { assert!(!path.exists()); }
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
        restore_live_inputs(parked);

        // The failed startup already bound generation 2. Corrected recovery
        // selects generation 3, preserves its spent source work, and keeps ONE
        // owner through the two remaining releases instead of replaying again.
        write_next(&root, 3);
        let final_pair = Archives::named(&root, "resident-completed");
        let config = configured(&root);
        let (selected, profiles, last) = entries(&root, &config, &output, &final_pair, 3);
        let mut actors = [None, None];
        let mut humans = [None, None];
        let result = service::run(config, selected, &reviewers, true, Some(1), None, || {
            for index in 0..profiles.len() {
                let release = if index == 0 {
                    LearnedTextRelease::Message
                } else {
                    LearnedTextRelease::Finish
                };
                if profiles[index].socket.exists() && actors[index].is_none() {
                    actors[index] = Some(peers::actor(
                        profiles[index].clone(), release, ElapsedTick(50_000), peers::Behavior::Observe,
                    ));
                }
                if reviewers.socket(profiles[index].request).exists() && humans[index].is_none() {
                    humans[index] = Some(peers::human(
                        reviewers.clone(), profiles[index].request, ReviewDecision::Approve,
                    ));
                }
            }
            ElapsedTick(1000)
        }).unwrap();
        assert!(result.failure.is_none(), "{:?}", result.failure);
        assert_eq!(result.cleanup_pending, 0);
        assert_eq!(result.responses.len(), 2);
        for (index, actor) in actors.into_iter().enumerate() {
            let response = actor.expect("every remaining release needs its original authenticated actor")
                .join().unwrap();
            assert_eq!(response.result, result.responses[index].result);
            assert!(matches!(&response.result,
                Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
        }
        let mut sessions = std::collections::BTreeSet::new();
        for (index, human) in humans.into_iter().enumerate() {
            let packet = human.expect("every remaining release requires fresh independent human review")
                .join().unwrap();
            assert!(sessions.insert(packet.binding().session));
            assert_eq!(packet.views().len(), 2);
            let frame = ReleaseFrame::decode(&packet.action().spec().payload).unwrap();
            assert_eq!(frame.prior_messages(), vec!["A"; index + 1].as_slice());
            assert_eq!(frame.message(), if index == 0 { Some("A") } else { None });
        }
        let saved = final_pair.read();
        assert!(saved.work().checks >= original.work().checks + 4);
        assert_eq!(saved.expected().graph().spec().generation, 3);
        let state = observed(&configured(&root), &reader);
        assert_eq!(state.control.ledger.epoch, after.control.ledger.epoch + 3,
            "the Fence withdraws topology and revokes recovery, then rebind revokes once; the resident handoff cannot reopen");
        assert_finished(&root, &last, &final_pair);
        assert_eq!(input.read(), original);
    }
}
