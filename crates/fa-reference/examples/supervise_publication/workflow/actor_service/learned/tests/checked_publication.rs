//! Learned numerical custody and independent reviews composed with the ORIGINAL
//! final-publication witnesses. Real files, producers, native inference and peer
//! transport exercise enforcement; fixed models are not detector-quality data.
use super::*;
use super::continuation as native;
use super::peer_intake as peers;
use super::super::anchors as anchor_files;
use crate::config::CLOCK_DOMAIN;
use fa_reference::action::{Scope, consequence::delivery::{
    stream::ReleaseFrame,
    persistent::{FileDeliverySnapshot, observed::{
        FileOversightProfile,
        decoder::learned::FileLearnedConfig,
        publication::{witnesses::{FilePublicationInputs, FileWitnessInput,
            producer::{FilePublicationProducer, PublicationProducerProfile}}},
        reviewer::wire::{ReviewDecision, ReviewPacket},
    }}}};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker};
use fa_reference::witness::{AdapterDomainInput, DomainClosure, DomainProjection, SnapshotEntry};
use std::path::PathBuf;

#[path = "checked_publication/recipe.rs"] mod recipe_tests;
#[path = "checked_publication/recovery.rs"] mod recovery_tests;
#[path = "checked_publication/sequence.rs"] mod sequence_tests;

fn structured(scope: Scope, revision: u64, keys: &[u64]) -> FilePublicationInputs {
    let key = ProjectionKey { source: 40, branch: scope.branch, projection: 7, source_epoch: 1 };
    let close = TrustedClosingMarker { key, final_sequence: 1, marker_generation: 1 };
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap();
    frontiers.record_close(close).unwrap();
    FilePublicationInputs::new(Some(FileWitnessInput::new(revision, revision, 20,
        AdapterDomainInput::new(DomainProjection::new(40, 1, key), DomainClosure::Closed(close)),
        keys.iter().map(|&key| SnapshotEntry::new(key, 1, b"original".to_vec()).unwrap()).collect(),
        &frontiers).unwrap()), None)
}

fn opaque(model_epoch: u64) -> FilePublicationInputs {
    // This is a separately supplied producer's opaque observation contract. It
    // does not impersonate or replace either native helper's actual input view.
    FilePublicationInputs::new(None, Some(ActualHelperInput::new(b"Q".to_vec(), InputProfileBinding {
        profile_id: 1, profile_bytes: vec![], tokenizer_epoch: 1, policy_epoch: 0, model_epoch,
    }, vec![SubmittedPart { span: ByteSpan { start: 0, end: 1 }, kind: PartKind::Question }], vec![]).unwrap()))
}

fn profile(root: &Root, config: &Config, fallback: bool, whole: bool) -> String {
    let scope = config.profile.delivery.scope;
    let schema = match (whole, fallback) {
        (false, false) => "fa.supervised-witnesses/3",
        (false, true) => "fa.supervised-witnesses/5",
        (true, false) => "fa.supervised-whole-input/1",
        (true, true) => "fa.supervised-whole-input/2",
    };
    let history = if fallback { r#", "history":"exact_current_snapshot""# } else { "" };
    let requests = if whole { "" } else {
        r#"{"kind":"exact_value","key":0,"role":"subject"},{"kind":"absent_key","key":1},{"kind":"empty_range","start":6,"end":9},{"kind":"range_members","start":2,"end":6}"#
    };
    format!(r#"{{"schema":"{schema}"{history},"source":91,"producer":{{"path":"{}/producer/delivery.bin","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}}}},"feed":{{"source":41,"after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}},"limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[{requests}]}}"#,
        root.0.display(), scope.tenant, scope.principal, scope.run, scope.branch, scope.authority)
}

fn envelope(root: &Root, inner: &str) -> String {
    format!(r#"{{"schema":"fa.learned-checked-publication/1","publication":"{}/publication.json","recipe":{inner}}}"#,
        root.0.display())
}

fn provision(root: &Root, config: &Config, path: &Path, fallback: bool, whole: bool)
    -> FilePublicationProducer
{
    fs::write(root.0.join("publication.json"), profile(root, config, fallback, whole)).unwrap();
    fs::write(path, envelope(root, &fs::read_to_string(path).unwrap())).unwrap();
    let identity = PublicationProducerProfile { source: 91, scope: config.profile.delivery.scope,
        feed: 41, clock_domain: CLOCK_DOMAIN, after: 0 };
    let initial = if whole { opaque(1) } else { structured(identity.scope, 1, &[0, 2, 4]) };
    FilePublicationProducer::create(root.0.join("producer"), identity, initial, ElapsedTick(1000)).unwrap().0
}

struct Reader { store: PathBuf, profile: FileOversightProfile, generation: FileLearnedConfig }
impl Reader {
    fn new(config: &Config, loaded: &Loaded) -> Self {
        Self { store: config.store.clone(), profile: config.profile.clone(), generation: loaded.generation.clone() }
    }
    fn snapshot(&self) -> FileDeliverySnapshot {
        if let Some(predictor) = self.generation.required_owned_pre_output_forecast() {
            FileOversight::read_publication_with_owned_learned_consistency(&self.store,
                &self.profile, &self.generation, predictor).unwrap().publication
        } else {
            FileOversight::read_publication_with_learned_generation(&self.store,
                &self.profile, &self.generation).unwrap()
        }
    }
}

fn confirmed(result: &RunResult) -> bool {
    matches!(result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. }))
}

/// The actual service, an authenticated original actor client and a separately
/// authenticated human. The clock hook may mutate a real source, never votes,
/// keys, journal events or endpoint outcomes.
fn service(root: &Root, config: Config, loaded: Loaded, after: Option<u64>, finishing: Option<u64>,
    decision: ReviewDecision, clock: &mut impl FnMut(bool) -> ElapsedTick)
    -> (RunResult, Option<ReviewPacket>)
{
    let reviewer = fixture::peers(root, &config);
    let actor = peers::actor_profile(root, &config, loaded.request);
    let request = loaded.request;
    let release = if finishing.is_some() { LearnedTextRelease::Finish } else { LearnedTextRelease::Message };
    let mut submitted = None; let mut approved = None;
    let result = run_with_actor(config, loaded, &reviewer, after.is_some() || finishing.is_some(),
        finishing, after, None, Some(&actor), || {
            if submitted.is_none() && actor.socket.exists() {
                submitted = Some(peers::actor(actor.clone(), release, ElapsedTick(50_000), peers::Behavior::Observe));
            }
            if approved.is_none() && reviewer.socket(request).exists() {
                approved = Some(peers::human(reviewer.clone(), request, decision));
            }
            clock(approved.is_some())
        }).unwrap();
    let response = submitted.expect("the original actor submitted through its authenticated endpoint").join();
    assert!(response.is_ok(), "authenticated actor transport failed after service failure={:?}, response={:?}",
        result.failure, result.response);
    let response = response.unwrap();
    assert_eq!(response.result, result.response.result);
    assert_eq!(result.cleanup_pending, 0);
    let packet = approved.map(|human| human.join().unwrap());
    assert!(!actor.socket.exists()); assert!(!reviewer.socket(request).exists());
    (result, packet)
}

#[test]
fn checked_learned_publication_rejects_late_phantoms_but_keeps_disjoint_changes_in_all_forecast_modes() {
    for (mode, fallback) in [(None, false), (Some(false), false), (Some(true), false), (None, true)] {
        for phantom in [false, true] {
            let root = Root::new(); let config = configured(&root); let path = native::write(&root);
            if let Some(owned) = mode { super::forecast::install(&root, &path, owned, 60_000); }
            let mut producer = provision(&root, &config, &path, fallback, false);
            evidence(&root, &config, 1, true, false);
            let loaded = recipe::load(&path, &config, true).unwrap();
            let reader = Reader::new(&config, &loaded); let scope = config.profile.delivery.scope;
            let mut admitted = None;
            let (result, packet) = service(&root, config, loaded, None, None, ReviewDecision::Approve, &mut |reviewed| {
                if reviewed && admitted.is_none() {
                    let state = reader.snapshot();
                    if state.control.ledger.charged != 0 && state.executions == 0 {
                        let keys: &[u64] = if phantom { &[0, 1, 2, 4] } else { &[0, 2, 4, 99] };
                        producer.publish(1, structured(scope, 2, keys), ElapsedTick(1001)).unwrap();
                        admitted = Some(state);
                    }
                }
                ElapsedTick(if admitted.is_some() { 1001 } else { 1000 })
            });
            let admitted = admitted.expect("mutation must follow an actual charged Dispatch and precede publication");
            let packet = packet.expect("both native review and separate human key preceded the mutation");
            assert_eq!(packet.views().len(), 2);
            assert_eq!(ReleaseFrame::decode(&packet.action().spec().payload).unwrap().message(), Some("A"));
            assert_eq!(admitted.control.ledger.charged, packet.action().spec().units);
            assert_eq!(confirmed(&result), !phantom, "mode={mode:?}, fallback={fallback}: {:?}", result.failure);
            let state = reader.snapshot();
            assert_eq!(state.executions, u64::from(!phantom));
            assert_eq!(state.payload, if phantom { Vec::new() } else { b"A".to_vec() });
            assert_eq!(state.control.ledger.charged, if phantom { 0 } else { packet.action().spec().units });
            assert_eq!(state.control.ledger.reserved, 0);
            if !phantom { assert!(result.failure.is_none(), "{:?}", result.failure); }
            let config = configured(&root); let loaded = recipe::load(&path, &config, false).unwrap();
            let (host, _, _, _) = recovery::open_selected(&config, &loaded).unwrap();
            assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 2);
            assert_eq!(host.learned_generation_inspection().unwrap().numerical.position, 4);
            if mode.is_some() { assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1); }
        }
    }
}

#[test]
fn checked_opaque_model_epoch_change_is_not_hidden_by_identical_bytes_or_human_approval() {
    for changed in [false, true] {
        let root = Root::new(); let config = configured(&root); let path = native::write(&root);
        let mut producer = provision(&root, &config, &path, true, true);
        evidence(&root, &config, 1, true, false);
        let loaded = recipe::load(&path, &config, true).unwrap();
        let reader = Reader::new(&config, &loaded); let mut dispatched = false;
        let (result, packet) = service(&root, config, loaded, None, None, ReviewDecision::Approve, &mut |reviewed| {
            if reviewed && !dispatched {
                let state = reader.snapshot();
                if state.control.ledger.charged != 0 && state.executions == 0 {
                    // Source bytes, partition, question and tokenizer stay fixed.
                    // Only the semantic model epoch differs in the negative arm.
                    producer.publish(1, opaque(if changed { 2 } else { 1 }), ElapsedTick(1001)).unwrap();
                    dispatched = true;
                }
            }
            ElapsedTick(if dispatched { 1001 } else { 1000 })
        });
        assert!(dispatched); let packet = packet.unwrap();
        assert_eq!(confirmed(&result), !changed, "{:?}", result.failure);
        let state = reader.snapshot();
        assert_eq!(state.executions, u64::from(!changed));
        assert_eq!(state.control.ledger.charged, if changed { 0 } else { packet.action().spec().units });
        assert_eq!(state.control.ledger.reserved, 0);
        if changed { assert!(state.payload.is_empty()); }
        else { assert_eq!(state.payload, b"A"); assert!(result.failure.is_none()); }
    }
}

#[test]
fn original_identity_joint_forecast_topology_and_witness_guards_compose_before_real_publication() {
    let root = Root::new();
    let (config, path, mut original) = super::mediated_topology::selected(&root, Some(true), false);
    super::identity_tests::provision(&root, &path, true);
    let _producer = provision(&root, &config, &path, false, false);
    let mut loaded = recipe::load(&path, &config, true).unwrap();
    assert!(loaded.generation.required_held_out_joint().is_some());
    assert!(loaded.generation.required_computed_identity_registration().is_some());
    assert!(loaded.generation.required_owned_pre_output_forecast().is_some());
    assert!(loaded.generation.required_publication_validation().is_some());
    loaded.topology = original.topology.take();
    loaded.anchor_output = original.anchor_output.take();
    let reader = Reader::new(&config, &loaded);
    let (result, packet) = service(&root, config, loaded, None, None, ReviewDecision::Approve,
        &mut |_| ElapsedTick(1000));
    assert!(confirmed(&result)); assert!(result.failure.is_none(), "{:?}", result.failure);
    let packet = packet.unwrap(); assert_eq!(packet.views().len(), 2);
    let current = reader.snapshot();
    assert_eq!(current.executions, 1); assert_eq!(current.payload, b"A");
    assert_eq!(current.control.ledger.charged, packet.action().spec().units);
    assert_eq!(current.control.ledger.reserved, 0);
    let source = super::mediated_topology::checkpoint(&root, 1);
    assert!(source.available()); assert!(source.work().checks >= 2);
    assert_eq!(source.history_anchor(), &anchor_files::load(&root.0.join("history-1.bin")).unwrap());
}

#[test]
fn plain_action_bound_capture_publishes_after_both_keys_and_seals_a_late_phantom_without_a_feed() {
    use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason,
        persistent::{requests::FileRequestDisposition, observed::publication::{
            PublicationBasis, capture::{FileCaptureIdentity, FilePublicationCapture},
        }}};
    for phantom in [false, true] {
        let root = Root::new(); let mut config = configured(&root); let path = native::write(&root);
        fs::write(root.0.join("publication.json"), recipe_tests::captures(&root, 1)).unwrap();
        fs::write(&path, envelope(&root, &fs::read_to_string(&path).unwrap())).unwrap();
        evidence(&root, &config, 1, true, false);
        let loaded = recipe::load(&path, &config, true).unwrap();
        assert!(loaded.generation.required_publication_validation().unwrap().feed.is_none());
        let (host, reviewer, forecast, identity) = recovery::create_selected(&config, &loaded).unwrap();
        assert!(forecast.is_none() && identity.is_none());
        let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let mut driver = FileSupervisedDriver::new(supervisor); let mut wire = ActorWire::new(port);
        let deadline = deadline(); let mut control = Control::new(&config, 1, None).unwrap();
        assert!(generate(&mut driver, &reviewer, &mut config, &mut control,
            &deadline, &mut || ElapsedTick(1000)).unwrap());
        submit(&mut driver, &mut wire, &mut config, &loaded, &deadline, &mut || ElapsedTick(1000)).unwrap();
        let (attempt, action, numerical) = {
            let host = driver.supervisor().host().unwrap();
            let FileRequestDisposition::Admitted { attempt, .. } = host.request_status(1).unwrap().disposition
                else { panic!("original learned request was not admitted"); };
            (attempt, host.request_action(1).unwrap().clone(), host.learned_generation_inspection().unwrap().numerical)
        };
        let capture = |generation, keys: &[u64]| FilePublicationCapture::new(attempt,
            FileCaptureIdentity { source: 91, generation }, &action,
            structured(config.profile.delivery.scope, generation, keys)).unwrap().to_bytes().unwrap();
        let original = capture(1, &[0, 2, 4]);
        fs::write(root.0.join("original.bin"), &original).unwrap();
        fs::write(root.0.join("current.bin"), &original).unwrap();
        let publication = {
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            loaded.publication.as_ref().unwrap().prepare_with_clock(&mut host, attempt, || ElapsedTick(1000)).unwrap()
        };
        let reviewed = review::run_checked(driver, &mut config, &loaded, &reviewer,
            &mut control, &deadline, &mut || ElapsedTick(1000), Some(&publication));
        assert!(reviewed.failure.is_none(), "{:?}", reviewed.failure);
        let history = reviewed.history.unwrap(); assert_eq!(history.status(), NativeReviewStatus::Finished);
        assert_eq!(native_records(&history).count(), 2);
        for record in native_records(&history) {
            assert_eq!(record.progress.basis, Some(SidecarDecisionBasis::NativeModel));
            assert_eq!(record.progress.native.reviewed_prompt_tokens, record.progress.native.requested_prompt_tokens);
            assert_eq!(record.progress.native.work.sampled_draws, 2);
        }
        let mut driver = reviewed.driver;
        assert!(matches!(publication.step_learned(&mut driver, &mut config.source,
            &mut || ElapsedTick(1000), None).unwrap(), FileDriverEvent::AwaitingHuman { request: 1 }));
        let human = native::approve(&mut driver, &mut config, &reviewer, 1);
        assert!(matches!(publication.step_learned(&mut driver, &mut config.source,
            &mut || ElapsedTick(1000), Some(&human)).unwrap(), FileDriverEvent::Dispatched { request: 1, .. }));
        let dispatched = driver.supervisor().host().unwrap().inspect();
        assert_eq!(dispatched.executions, 0); assert_eq!(dispatched.control.ledger.charged, action.spec().units);
        // Change only the producer's current capture after actual dispatch.
        // The original action, capture, native decisions and human key remain.
        let keys: &[u64] = if phantom { &[0, 1, 2, 4] } else { &[0, 2, 4, 99] };
        fs::write(root.0.join("current.bin"), FilePublicationCapture::new(attempt,
            FileCaptureIdentity { source: 91, generation: 2 }, &action,
            structured(config.profile.delivery.scope, 2, keys)).unwrap().to_bytes().unwrap()).unwrap();
        let FileDriverEvent::PublicationChecked { publication: checked, source_failure, .. } =
            publication.step_learned(&mut driver, &mut config.source,
                &mut || ElapsedTick(1001), None).unwrap()
            else { panic!("missing original checked publication result"); };
        assert!(source_failure.is_none());
        assert_eq!(checked.basis, if phantom { PublicationBasis::Rejected(Error::Stale) }
            else { PublicationBasis::Revalidated });
        assert_eq!(checked.outcome, if phantom { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }
            else { EndpointOutcome::Executed { resulting_version: 2 } });
        // Sealing is not a refund: only the next original receipt transition
        // can discharge the known nonexecution liability.
        assert_eq!(driver.supervisor().host().unwrap().inspect().control.ledger.charged, action.spec().units);
        assert!(matches!(publication.step_learned(&mut driver, &mut config.source,
            &mut || ElapsedTick(1001), None).unwrap(), FileDriverEvent::Reconciled { request: 1, .. }));
        assert_eq!(executed(&mut wire, 1), !phantom);
        let host = driver.supervisor().host().unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(host.inspect().executions, u64::from(!phantom));
        assert_eq!(host.inspect().control.ledger.charged, if phantom { 0 } else { action.spec().units });
        assert_eq!(host.inspect().control.ledger.reserved, 0);
        assert_eq!(fs::read(root.0.join("original.bin")).unwrap(), original);
        drop(host); assert_eq!(cleanup(driver, 1000, 1), 0); drop(history);
    }
}
