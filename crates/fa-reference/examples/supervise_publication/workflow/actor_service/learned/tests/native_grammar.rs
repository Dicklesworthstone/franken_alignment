//! The operator selects a grammar before input; original numerical helpers,
//! congress and the independent human key still decide every publication.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::{
    client::ReviewClientProgress, wire::{ReviewDecision, ReviewPacket},
};
use fa_reference::action::consequence::delivery::stream::ReleaseFrame;
use fa_reference::action::consequence::oversight::helper_client::native::{
    NativeEvaluationError, NativeEvaluationStatus, NativeVerdictGrammar,
};
use fa_reference::round::Verdict;
use fa_reference::strict_json::{self, Limits};
use std::thread;

fn select(root: &Root, grammar: &str) {
    let file = root.0.join("native-roster.json");
    let mut roster = fs::read_to_string(&file).unwrap();
    assert!(roster.contains("fa.learned-native-roster/1"));
    roster = roster.replace("fa.learned-native-roster/1", "fa.learned-native-roster/3");
    for name in ["alpha", "beta"] {
        let start = format!("\"{name}\":{{\"model\":");
        assert!(roster.contains(&start));
        roster = roster.replace(&start,
            &format!("\"{name}\":{{\"verdict_grammar\":\"{grammar}\",\"model\":"));
    }
    fs::write(file, roster).unwrap();
}

fn set_helper_row(root: &Root, name: &str, row: usize, values: [f32; 2]) {
    let file = root.0.join("helper-weights.safetensors");
    let mut bytes = fs::read(&file).unwrap();
    let header_bytes = u64::from_le_bytes(bytes[..8].try_into().unwrap()) as usize;
    let header = strict_json::parse(&bytes[8..8 + header_bytes], Limits::default()).unwrap();
    let tensor = header.get(name).unwrap();
    assert_eq!(tensor.get("dtype").unwrap().as_str(), Some("F32"));
    let shape = tensor.get("shape").unwrap().as_array().unwrap();
    assert_eq!(shape[1].as_u64(), Some(2));
    assert!(row < shape[0].as_u64().unwrap() as usize);
    let offset = tensor.get("data_offsets").unwrap().as_array().unwrap()[0].as_u64().unwrap() as usize;
    let position = 8 + header_bytes + offset + row * 8;
    for (index, value) in values.into_iter().enumerate() {
        bytes[position + index * 4..position + (index + 1) * 4].copy_from_slice(&value.to_le_bytes());
    }
    fs::write(file, bytes).unwrap();
}

fn prefer_invalid(root: &Root) {
    // Original logits select x over the fixture's real verdict token. Its
    // numerical successor selects stop, so unrestricted output is exactly x,
    // not a missing terminal. Finite sampling must compute the lower score.
    set_helper_row(root, "model.embed_tokens.weight", usize::from(b'x'), [0.0, 1.0]);
    set_helper_row(root, "lm_head.weight", usize::from(b'x'), [20.0, 0.0]);
}

fn prepare(root: &Root, spelling: &[u8], grammar: Option<&str>)
    -> (Config, Loaded, FileHumanReviewer, FileSupervisedDriver, Wire, Deadline)
{
    let mut config = configured(root);
    let path = fixture::write(root, spelling);
    if let Some(grammar) = grammar { select(root, grammar); }
    prefer_invalid(root);
    evidence(root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    let (host, reviewer) = recovery::create(&config, &loaded).unwrap();
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut wire = ActorWire::new(port);
    let deadline = deadline();
    let mut control = Control::new(&config, loaded.request, None).unwrap();
    assert!(generate(&mut driver, &reviewer, &mut config, &mut control,
        &deadline, &mut || ElapsedTick(1000)).unwrap());
    submit(&mut driver, &mut wire, &mut config, &loaded, &deadline,
        &mut || ElapsedTick(1000)).unwrap();
    (config, loaded, reviewer, driver, wire, deadline)
}

fn human(peers: PeerProfile) -> thread::JoinHandle<ReviewPacket> {
    thread::spawn(move || {
        let started = Instant::now();
        let mut client = peers.connect_client(1).unwrap();
        let mut packet = None;
        loop {
            assert!(started.elapsed() < Duration::from_secs(15), "finite-grammar human review timed out");
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
fn native_roster_grammar_reaches_real_peer_review_and_original_publication() {
    let root = Root::new(); let config = configured(&root);
    let path = fixture::write(&root, b"allow");
    select(&root, "finite-words-v1"); prefer_invalid(&root);
    evidence(&root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    let generation = loaded.generation.clone();
    let profile = config.profile.clone(); let store = config.store.clone();
    let peers = fixture::peers(&root, &config); let mut reviewer = None;
    let result = run(config, loaded, &peers, false, || {
        if reviewer.is_none() && peers.socket(1).exists() {
            reviewer = Some(human(peers.clone()));
        }
        ElapsedTick(1000)
    }).unwrap();
    let packet = reviewer.expect("independent authenticated human endpoint").join().unwrap();
    assert!(result.failure.is_none(), "{:?}", result.failure);
    assert_eq!(result.cleanup_pending, 0);
    assert!(matches!(result.response.result,
        Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert_eq!(packet.views().len(), 2);
    assert_eq!(ReleaseFrame::decode(&packet.action().spec().payload).unwrap().message(), Some("A"));
    let state = FileOversight::read_publication_with_learned_generation(&store, &profile, &generation).unwrap();
    assert_eq!(state.executions, 1); assert_eq!(state.payload, b"A");
    assert_eq!(state.control.ledger.charged, packet.action().spec().payload.len() as u64);
    assert_eq!(state.control.ledger.reserved, 0);
}

#[test]
fn only_explicit_finite_native_roster_filters_the_higher_scored_invalid_output() {
    for selection in [None, Some("unrestricted"), Some("finite-words-v1")] {
        let root = Root::new();
        let (mut config, loaded, _reviewer, driver, _wire, deadline) = prepare(&root, b"allow", selection);
        let mut run = review::start(driver, &mut config, &loaded, &deadline,
            &mut || ElapsedTick(1000)).unwrap();
        for _ in 0..loaded.native_limits.polls {
            if native_records(run.review()).count() == 2 && native_records(run.review()).all(|record|
                matches!(record.progress.native.status,
                    NativeEvaluationStatus::Judged(_) | NativeEvaluationStatus::Failed(_))) { break; }
            run.step_from_policy_file(&mut config.source, || ElapsedTick(1000), None).result.unwrap();
        }
        let finite = selection == Some("finite-words-v1");
        assert_eq!(native_records(run.review()).count(), 2);
        for record in native_records(run.review()) {
            assert_eq!(record.progress.native.status, if finite {
                NativeEvaluationStatus::Judged(Verdict::Allow)
            } else { NativeEvaluationStatus::Failed(NativeEvaluationError::InvalidVerdict) });
            assert_eq!(record.progress.native.work.sampled_draws, 2);
            assert_eq!(record.progress.native.reviewed_prompt_tokens,
                record.progress.native.requested_prompt_tokens);
            assert!(record.progress.native.requested_prompt_tokens > 0);
            assert_eq!(record.progress.completed_probes, 2);
        }
        // A computed categorical answer alone has no human key or effect.
        assert_eq!(run.supervisor().host().unwrap().inspect().executions, 0);
        if run.review().status() == NativeReviewStatus::Running {
            run.cancel(run.review().revision()).unwrap();
        }
        let handoff = run.into_handoff().unwrap();
        assert_eq!(handoff.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 0);
        assert_eq!(cleanup(handoff.driver, 1000, 1), 0);
    }
}

#[test]
fn finite_native_verdicts_preserve_deny_hold_abstain_and_the_separate_human_key() {
    for (spelling, verdict) in [(b"hold".as_slice(), Verdict::Hold),
        (b"deny".as_slice(), Verdict::Deny), (b"abstain".as_slice(), Verdict::Abstain),
        (b"allow".as_slice(), Verdict::Allow)]
    {
        let root = Root::new();
        let (mut config, loaded, reviewer, driver, mut wire, deadline) =
            prepare(&root, spelling, Some("finite-words-v1"));
        let mut control = Control::new(&config, 1, None).unwrap();
        let reviewed = review::run(driver, &mut config, &loaded, &reviewer, &mut control,
            &deadline, &mut || ElapsedTick(1000));
        assert!(reviewed.failure.is_none(), "{:?}", reviewed.failure);
        let mut driver = reviewed.driver; let history = reviewed.history.unwrap();
        assert_native(&history);
        assert!(native_records(&history).all(|record|
            record.progress.native.status == NativeEvaluationStatus::Judged(verdict)));
        if verdict == Verdict::Allow {
            assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
                || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::AwaitingHuman { request: 1 }));
            let offer = driver.request_learned_human_approval_from_policy_file(&mut config.source,
                701, ElapsedTick(50_000), || ElapsedTick(1000)).result.unwrap();
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision(); reviewer.reject(&mut host, revision, &offer).unwrap();
            let revision = host.revision(); host.cancel_request(revision, 1).unwrap();
        } else {
            assert!(!matches!(driver.phase(), FileDriverPhase::AwaitingDispatch { .. }));
            assert!(driver.request_learned_human_approval_from_policy_file(&mut config.source,
                701, ElapsedTick(50_000), || ElapsedTick(1000)).result.is_err());
        }
        assert!(!executed(&mut wire, 1));
        let state = driver.supervisor().host().unwrap().inspect();
        assert_eq!(state.executions, 0); assert!(state.payload.is_empty());
        assert_eq!(state.control.ledger.charged, 0);
        assert_eq!(cleanup(driver, 1000, 1), 0);
    }
}

#[test]
fn roster_grammar_selection_is_frozen_for_each_fresh_member_and_input_epoch() {
    let root = Root::new(); let config = configured(&root);
    fixture::write(&root, b"allow");
    let file = root.0.join("native-roster.json");
    let legacy = fs::read_to_string(&file).unwrap();
    for schema in ["fa.learned-native-roster/1", "fa.learned-native-roster/2"] {
        fs::write(&file, legacy.replace("fa.learned-native-roster/1", schema)).unwrap();
        let mut assets = fields::Assets::new(fields::MAX_ASSET_BYTES).unwrap();
        let roster = native::NativeRoster::load(&file, &config, &mut assets).unwrap();
        for (name, member) in config.profile.committee.members() {
            assert_eq!(roster.evaluator(name, member.profile_at(0)).unwrap().verdict_grammar(),
                NativeVerdictGrammar::Unrestricted);
        }
    }
    fs::write(&file, &legacy).unwrap(); select(&root, "finite-words-v1");
    let selected = fs::read_to_string(&file).unwrap();
    fs::write(&file, selected.replace("\"beta\":{\"verdict_grammar\":\"finite-words-v1\"",
        "\"beta\":{\"verdict_grammar\":\"unrestricted\"")).unwrap();
    let mut assets = fields::Assets::new(fields::MAX_ASSET_BYTES).unwrap();
    let roster = native::NativeRoster::load(&file, &config, &mut assets).unwrap();
    // Round/continuation evaluator creation uses only the admitted original
    // model and selection, never rereads mutable roster files after a vote.
    fs::remove_file(&file).unwrap();
    fs::remove_file(root.0.join("helper-weights.safetensors")).unwrap();
    for epoch in [0, 7, 19] {
        for (name, member) in config.profile.committee.members() {
            let input = member.profile_at(epoch);
            let evaluator = roster.evaluator(name, input.clone()).unwrap();
            assert_eq!(evaluator.policy().input_profile, input);
            assert_eq!(evaluator.position(), 0); assert_eq!(evaluator.sampled_draws(), 0);
            assert_eq!(evaluator.verdict_grammar(), if name == "alpha" {
                NativeVerdictGrammar::FiniteWordsV1
            } else { NativeVerdictGrammar::Unrestricted });
        }
    }
    assert!(!config.store.exists());
}

#[test]
fn all_member_grammar_declarations_are_admitted_before_any_model_read_or_store() {
    let root = Root::new(); let config = configured(&root);
    fixture::write(&root, b"allow"); select(&root, "finite-words-v1");
    let file = root.0.join("native-roster.json"); let valid = fs::read_to_string(&file).unwrap();
    let mut assets = fields::Assets::new(fields::MAX_ASSET_BYTES).unwrap();
    native::NativeRoster::load(&file, &config, &mut assets).unwrap();
    assert!(assets.consumed() > valid.len()); assert!(!config.store.exists());
    fs::remove_file(root.0.join("helper-model.json")).unwrap();
    let beta = "\"beta\":{\"verdict_grammar\":\"finite-words-v1\",";
    assert!(valid.contains(beta));
    for (invalid, message) in [
        (valid.replace(beta, "\"beta\":{"), "missing learned recipe field verdict_grammar"),
        (valid.replace(beta, "\"beta\":{\"verdict_grammar\":\"guess\","), "unsupported native verdict grammar"),
        (valid.replace(beta, "\"beta\":{\"verdict_grammar\":true,"), "verdict_grammar must be text"),
        (valid.replace(beta, "\"beta\":{\"verdict_grammar\":\"finite-words-v1\",\"verdict_grammar\":\"unrestricted\","), "Duplicate"),
        (valid.replace("fa.learned-native-roster/3", "fa.learned-native-roster/1"), "unsupported learned recipe field"),
        (valid.replace("fa.learned-native-roster/3", "fa.learned-native-roster/2"), "unsupported learned recipe field"),
    ] {
        fs::write(&file, &invalid).unwrap();
        let mut assets = fields::Assets::new(fields::MAX_ASSET_BYTES).unwrap();
        let failure = native::NativeRoster::load(&file, &config, &mut assets).err().unwrap();
        assert!(failure.contains(message), "{failure}");
        assert_eq!(assets.consumed(), invalid.len());
        assert!(!config.store.exists());
    }
}
