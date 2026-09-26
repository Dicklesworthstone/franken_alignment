//! Native inference plus the ORIGINAL producer, witnesses, helpers and human key.
//! Files/sockets are real; weights, helper verdicts and credentials are fixtures.
use super::*;
use crate::config::CLOCK_DOMAIN;
use fa_reference::action::Scope;
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::{
    FilePublicationInputs, FileWitnessInput,
    producer::{FilePublicationProducer, PublicationProducerProfile},
};
use fa_reference::product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker};
use fa_reference::witness::{AdapterDomainInput, DomainClosure, DomainProjection, SnapshotEntry};

fn witness_inputs(scope: Scope, revision: u64, keys: &[u64]) -> FilePublicationInputs {
    let key = ProjectionKey { source: 40, branch: scope.branch, projection: 7, source_epoch: 1 };
    let close = TrustedClosingMarker { key, final_sequence: 1, marker_generation: 1 };
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap(); frontiers.record_close(close).unwrap();
    FilePublicationInputs::new(Some(FileWitnessInput::new(revision, revision, 20,
        AdapterDomainInput::new(DomainProjection::new(40, 1, key), DomainClosure::Closed(close)),
        keys.iter().map(|&key| SnapshotEntry::new(key, 1, b"original".to_vec()).unwrap()).collect(),
        &frontiers).unwrap()), None)
}
fn profile_json(root: &Root, config: &Config, fallback: bool) -> String {
    let s = config.profile.delivery.scope;
    let text = format!(r#"{{"schema":"fa.supervised-witnesses/3","source":91,"producer":{{"path":"{}/producer/delivery.bin","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}}}},"feed":{{"source":41,"after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}},"limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[{{"kind":"exact_value","key":0,"role":"subject"}},{{"kind":"absent_key","key":1}},{{"kind":"empty_range","start":6,"end":9}},{{"kind":"range_members","start":2,"end":6}}]}}"#,
        root.0.display(), s.tenant, s.principal, s.run, s.branch, s.authority);
    if fallback {
        text.replace("fa.supervised-witnesses/3", "fa.supervised-witnesses/5")
            .replacen("\"source\":91", "\"source\":91,\"history\":\"exact_current_snapshot\"", 1)
    } else { text }
}
fn publication(root: &Root, config: &Config, fallback: bool)
    -> (PublicationProfile, FilePublicationProducer)
{
    let scope = config.profile.delivery.scope;
    let identity = PublicationProducerProfile { source: 91, scope, feed: 41, clock_domain: CLOCK_DOMAIN, after: 0 };
    let (producer, _) = FilePublicationProducer::create(root.0.join("producer"), identity,
        witness_inputs(scope, 1, &[0, 2, 4]), ElapsedTick(1000)).unwrap();
    (PublicationProfile::decode(profile_json(root, config, fallback).as_bytes()).unwrap(), producer)
}
fn seed(config: &mut Config, native: &Inputs, publication: &PublicationProfile, steps: u64) {
    let selected = publication.generated_profile(&config.profile, native.stream, Some(RecoveryReserve::terminal())).unwrap();
    let (mut host, _) = FileOversight::create_generated_text_stream_checked(&config.store,
        config.profile.clone(), native.decoder.clone(), native.tokenizer.clone(), selected).unwrap();
    host.enable_file_source(host.revision(), config.source_policy).unwrap();
    host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1000)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    let command = FileTextGenerationCommand::new(native.generation, n.actor_revision, n.position, native.request.clone()).unwrap();
    host.begin_decoder_text(host.revision(), command).unwrap();
    for revision in 0..steps { host.advance_decoder_text(host.revision(), native.generation, revision).unwrap(); }
    assert_eq!(host.inspect().executions, 0);
}
fn executed(response: &WireResponse) -> bool {
    matches!(&response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. }))
}

#[test]
fn native_checked_create_and_recovery_keep_producer_witnesses_and_independent_human_approval() {
    for steps in [None, Some(1), Some(4)] {
        for approve in [true, false] {
            let root = Root::new(); let mut config = configured(&root);
            let native = inputs(&root, &config); let (actor, reviewer) = profiles(&root, &config);
            let (publication, _producer) = publication(&root, &config, false);
            if let Some(steps) = steps { seed(&mut config, &native, &publication, steps); }
            let store = config.store.clone(); let bootstrap = config.profile.clone();
            let decoder = native.decoder.clone(); let tokenizer = native.tokenizer.clone();
            let (sender, receiver) = mpsc::channel(); let client = actor_thread(actor.clone(), receiver);
            let human = reviewing(reviewer.clone(), store.clone(), bootstrap.clone(),
                if approve { ReviewDecision::Approve } else { ReviewDecision::Reject });
            let mut output = ReferenceOutput { bytes: Vec::new(), sender: Some(sender) };
            let result = serve_selected(config, &actor, &reviewer, native,
                (steps.is_some(), Some(&publication)), || ElapsedTick(1001), &mut output);
            assert!(result.is_ok(), "steps={steps:?} approve={approve}: {result:?}");
            human.join().unwrap(); assert_eq!(executed(&client.join().unwrap()), approve);
            let image = FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 7).unwrap();
            assert_eq!(image.text.generation_revision(), 4);
            assert_eq!(image.text.finish(), Some(Ok(GenerationFinish::StopToken)));
            assert_eq!(image.text.bytes().unwrap(), b"A");
            assert_eq!(image.publication.executions, u64::from(approve));
            assert_eq!(image.publication.control.ledger.reserved, 0);
            if approve {
                assert_eq!(ReleaseFrame::decode(&image.publication.payload).unwrap().message(), Some("A"));
                assert_eq!(image.publication.control.ledger.charged, image.publication.payload.len() as u64);
            } else {
                assert!(image.publication.payload.is_empty()); assert_eq!(image.publication.control.ledger.charged, 0);
            }
            assert!(!reviewer.socket(91).exists()); assert!(!actor.socket.exists());
        }
    }
}

#[test]
fn native_checked_final_publication_rejects_a_late_phantom_but_preserves_disjoint_changes() {
    for fallback in [false, true] {
        for phantom in [false, true] {
            let root = Root::new(); let config = configured(&root);
            let native = inputs(&root, &config); let (actor, reviewer) = profiles(&root, &config);
            let (publication, mut producer) = publication(&root, &config, fallback);
            let store = config.store.clone(); let bootstrap = config.profile.clone(); let scope = config.profile.delivery.scope;
            let (sender, receiver) = mpsc::channel(); let client = actor_thread(actor.clone(), receiver);
            let human = reviewing(reviewer.clone(), store.clone(), bootstrap.clone(), ReviewDecision::Approve);
            let mut output = ReferenceOutput { bytes: Vec::new(), sender: Some(sender) };
            let mut changed = false;
            let result = serve_selected(config, &actor, &reviewer, native, (false, Some(&publication)), || {
                if !changed && let Ok(image) = FileOversight::read_publication(&store, &bootstrap)
                    && image.control.ledger.charged != 0 && image.executions == 0 {
                    let keys: &[u64] = if phantom { &[0, 1, 2, 4] } else { &[0, 2, 4, 99] };
                    producer.publish(1, witness_inputs(scope, 2, keys), ElapsedTick(1001)).unwrap();
                    changed = true;
                }
                ElapsedTick(if changed { 1001 } else { 1000 })
            }, &mut output);
            human.join().unwrap(); let response = client.join().unwrap();
            assert!(changed, "must cross actual dispatch before the mutation");
            assert_eq!(executed(&response), !phantom, "{result:?}");
            let image = FileOversight::read_publication(&store, &bootstrap).unwrap();
            assert_eq!(image.executions, u64::from(!phantom));
            assert_eq!(image.control.ledger.reserved, 0);
            assert_eq!(image.control.ledger.charged, if phantom { 0 } else { image.payload.len() as u64 });
            if phantom { assert!(image.payload.is_empty()); }
            else { assert!(result.is_ok(), "{result:?}"); }
        }
    }
}

#[test]
fn native_checked_receipt_recovery_needs_no_live_producer_evidence_helpers_or_new_human() {
    let root = Root::new(); let config = configured(&root);
    let native = inputs(&root, &config); let (actor, reviewer) = profiles(&root, &config);
    let (publication, producer) = publication(&root, &config, false);
    let store = config.store.clone(); let bootstrap = config.profile.clone();
    let decoder = native.decoder.clone(); let tokenizer = native.tokenizer.clone();
    let (sender, receiver) = mpsc::channel(); let client = actor_thread(actor.clone(), receiver);
    let human = reviewing(reviewer.clone(), store.clone(), bootstrap.clone(), ReviewDecision::Approve);
    let mut original = ReferenceOutput { bytes: Vec::new(), sender: Some(sender) };
    let result = serve_selected(config, &actor, &reviewer, native, (false, Some(&publication)),
        || ElapsedTick(1000), &mut original);
    assert!(result.is_ok(), "{result:?}"); human.join().unwrap(); assert!(executed(&client.join().unwrap()));
    drop(producer);
    let mut config = configured(&root); let native = inputs(&root, &config);
    config.programs.clear();
    std::fs::remove_file(root.0.join("producer/delivery.bin")).unwrap();
    std::fs::remove_file(root.0.join("evidence.json")).unwrap();
    let (sender, receiver) = mpsc::channel(); let client = actor_thread(actor.clone(), receiver);
    let mut recovered = ReferenceOutput { bytes: Vec::new(), sender: Some(sender) };
    let result = serve_selected(config, &actor, &reviewer, native, (true, Some(&publication)),
        || ElapsedTick(20000), &mut recovered);
    assert!(result.is_ok(), "{result:?}"); assert!(executed(&client.join().unwrap()));
    assert_eq!(recovered.bytes, original.bytes); // original target, epoch and expired deadline
    let image = FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 7).unwrap();
    assert_eq!(image.text.generation_revision(), 4); assert!(image.numerical.paused);
    assert_eq!(image.publication.executions, 1); assert_eq!(image.publication.control.ledger.reserved, 0);
    assert!(!reviewer.socket(91).exists()); assert!(!actor.socket.exists());
}

#[test]
fn native_checked_missing_original_producer_never_falls_back_to_unchecked_publication() {
    let root = Root::new(); let config = configured(&root);
    let native = inputs(&root, &config); let (actor, reviewer) = profiles(&root, &config);
    let (publication, producer) = publication(&root, &config, false);
    drop(producer); std::fs::remove_file(root.0.join("producer/delivery.bin")).unwrap();
    let store = config.store.clone(); let bootstrap = config.profile.clone();
    let (sender, receiver) = mpsc::channel(); let client = actor_thread(actor.clone(), receiver);
    let mut output = ReferenceOutput { bytes: Vec::new(), sender: Some(sender) };
    let result = serve_selected(config, &actor, &reviewer, native, (false, Some(&publication)),
        || ElapsedTick(1000), &mut output);
    assert!(result.is_err()); assert!(!executed(&client.join().unwrap()));
    let image = FileOversight::read_publication(&store, &bootstrap).unwrap();
    assert_eq!(image.executions, 0); assert!(image.payload.is_empty());
    assert!(image.stop.is_some()); assert!(!reviewer.socket(91).exists());
}

#[test]
fn native_checked_scope_and_command_preflight_cannot_select_a_weaker_mode() {
    let root = Root::new(); let config = configured(&root);
    let native = inputs(&root, &config); let (actor, reviewer) = profiles(&root, &config);
    // No producer exists: converting the profile is read-free even with a feed.
    let valid = profile_json(&root, &config, true);
    let publication = PublicationProfile::decode(valid.as_bytes()).unwrap();
    let selected = publication.generated_profile(&config.profile, native.stream, Some(RecoveryReserve::terminal())).unwrap();
    assert!(selected.feed.unwrap().snapshot_fallback);
    assert_eq!(selected.feed.unwrap().freshness.clock_domain, CLOCK_DOMAIN);
    let foreign = PublicationProfile::decode(valid.replacen("\"tenant\":1", "\"tenant\":2", 1).as_bytes()).unwrap();
    assert!(serve_selected(config, &actor, &reviewer, native, (false, Some(&foreign)),
        || panic!("foreign producer sampled time"), &mut Vec::new()).is_err());
    assert!(!root.0.join("store").exists()); assert!(!actor.socket.exists());
    let absent = root.0.join("absent").display().to_string();
    for mode in ["serve-create-checked", "serve-open-checked"] {
        let complete = [mode, &absent, &absent, &absent, &absent, "--native-text", &absent].map(str::to_owned);
        assert_ne!(command(&complete, None).unwrap_err(), USAGE); // reaches the explicit config reader
        let incomplete = [mode, &absent, &absent, &absent, "--native-text", &absent].map(str::to_owned);
        assert_eq!(command(&incomplete, None).unwrap_err(), USAGE);
        let mut duplicate = complete.to_vec(); duplicate.extend(["--native-text".into(), absent.clone()]);
        assert_eq!(command(&duplicate, None).unwrap_err(), USAGE);
    }
}

mod tied_tests;
