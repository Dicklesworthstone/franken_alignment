//! Actual learned identity and both original publication keys. These fixed
//! intervals are a causal enforcement fixture, not calibrated fingerprints or
//! authenticated manifest commitments.
use super::*;
use super::continuation as native;
use super::super::identity as check;
use fa_reference::action::consequence::activation::{
    identity::{IdentityAnchor, ModelManifest, ModelPassport, wire as identity_wire},
    tensor::kv::decoder::{DecoderModel, MAX_DECODER_PRODUCTS},
};
use fa_reference::action::consequence::delivery::persistent::observed::{
    decoder::learned::FileLearnedInspection,
    identity::decoder::transaction::ComputedIdentityStatus,
    consistency::learned::FileLearnedConsistencySnapshot,
};
use fa_reference::action::consequence::oversight::identity::{IdentityOutcome, IdentityMismatch, IdentityStatus};
use std::path::PathBuf;

#[path = "identity/recovery.rs"] mod recovery_tests;
#[path = "identity/control.rs"] mod control_tests;
#[path = "identity/recipe.rs"] mod recipe_tests;
#[path = "identity/installation.rs"] mod installation_tests;

const JOINT: &str = r#"{"id":71,"generation":1,"minimum_safe_roots":1,"minimum_violation_roots":1,"maximum_escape_ppm":0,"maximum_false_stop_ppm":0,"max_cases":2,"max_member_outcomes":8}"#;

fn manifest() -> ModelManifest {
    ModelManifest { tenant: 1, model: 9, model_generation: 1, host_generation: 1,
        tokenizer_generation: 1, weights: [1; 32], adapters: [2; 32], tokenizer: [3; 32],
        architecture: [4; 32], numeric_profile: [5; 32] }
}

fn passport(root: &Root) -> ModelPassport {
    let model = DecoderModel::from_llama_safetensors(models::identity(9), 16,
        &fs::read(root.0.join("model.json")).unwrap(),
        &fs::read(root.0.join("weights.safetensors")).unwrap()).unwrap().0;
    let capture = model.residual_contract(1).unwrap().profile();
    // Independent analytic oracle: both residual branches are zero, so the
    // final residual is the last stimulus token's registered embedding [1,0].
    ModelPassport::new(51, 1, manifest(), vec![
        IdentityAnchor::new(10, capture, 501, vec![120, 122], &[[1.0, 1.0], [0.0, 0.0]]).unwrap(),
        IdentityAnchor::new(20, capture, 502, vec![65, 120, 121], &[[1.0, 1.0], [0.0, 0.0]]).unwrap(),
    ]).unwrap()
}

fn envelope(root: &Root, inner: &str, id: u64, sequence: u64) -> String {
    let r = root.0.display();
    format!(r#"{{"schema":"fa.learned-identity-publication/1","identity":{{"passport":"{r}/identity.passport","policy":{{"observer_id":99,"timeout_ticks":10000,"validity_ticks":60000,"max_checks":8}},"check":{id},"measurement_sequence":{sequence},"scalar_products":{MAX_DECODER_PRODUCTS},"observed_manifest":"{r}/observed.manifest"}},"recipe":{inner}}}"#)
}

fn provision(root: &Root, path: &Path, joint: bool) {
    fs::write(root.0.join("identity.passport"), identity_wire::encode_passport(&passport(root)).unwrap()).unwrap();
    fs::write(root.0.join("observed.manifest"), identity_wire::encode_manifest(&manifest())).unwrap();
    let mut inner = fs::read_to_string(path).unwrap();
    if joint { inner = format!(r#"{{"schema":"fa.learned-joint-publication/1","joint":{JOINT},"recipe":{inner}}}"#); }
    fs::write(path, envelope(root, &inner, 901, 1)).unwrap();
}

fn substitute(root: &Root) {
    let path = root.0.join("weights.safetensors");
    let mut bytes = fs::read(&path).unwrap();
    let header = u64::from_le_bytes(bytes[..8].try_into().unwrap()) as usize;
    let parsed = fa_reference::strict_json::parse(&bytes[8..8 + header],
        fa_reference::strict_json::Limits::default()).unwrap();
    let offset = parsed.get("model.embed_tokens.weight").unwrap().get("data_offsets").unwrap()
        .as_array().unwrap()[0].as_u64().unwrap() as usize;
    // Token122 is in the identity stimulus, not the actor's xy/A/stop sequence.
    // Only its actual parameter changes. Every supplied label, fitted codec,
    // manifest, monitor, request and output-producing parameter stays fixed.
    let at = 8 + header + offset + 122 * 2 * 4;
    assert_eq!(&bytes[at..at + 4], &1.0_f32.to_le_bytes());
    bytes[at..at + 4].copy_from_slice(&(-1.0_f32).to_le_bytes());
    fs::write(path, bytes).unwrap();
}

struct Case {
    config: Config, path: PathBuf, loaded: Loaded,
    reviewer: FileHumanReviewer, driver: FileSupervisedDriver, wire: Wire,
    prediction: Option<prediction::Session>, identity: Option<check::Session>,
    histories: Vec<FileNativeSidecarSequence>,
}
impl Case {
    fn new(root: &Root, mode: Option<bool>, changed: bool, joint: bool) -> Self {
        let config = configured(root); let path = native::write(root);
        if let Some(owned) = mode { super::forecast::install(root, &path, owned, 60_000); }
        provision(root, &path, joint);
        if changed { substitute(root); }
        evidence(root, &config, 1, true, false);
        let loaded = recipe::load(&path, &config, true).unwrap();
        assert!(loaded.generation.requires_computed_identity());
        let (host, reviewer, forecast, identity) = recovery::create_selected(&config, &loaded).unwrap();
        assert!(host.identity_checks_required());
        let prediction = forecast.map(|observer| prediction::Session::new(loaded.request, observer));
        let identity = Some(check::Session::new(identity.unwrap()));
        let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        Self { config, path, loaded, reviewer, driver: FileSupervisedDriver::new(supervisor),
            wire: ActorWire::new(port), prediction, identity, histories: Vec::new() }
    }
    fn measure(&mut self) -> Result<bool, String> {
        let deadline = deadline();
        let mut control = Control::new(&self.config, self.loaded.request, None).unwrap();
        assert!(super::super::qualification::prepare(&mut self.driver, &self.config, None,
            (&mut control, &self.reviewer, &deadline), &mut || ElapsedTick(1000)).unwrap());
        check::prepare(&mut self.driver, &mut self.config, &self.loaded, &mut self.identity,
            (&mut control, &self.reviewer, &deadline), &mut || ElapsedTick(1000))
    }
    fn numerical(&self) -> FileLearnedInspection {
        self.driver.supervisor().host().unwrap().learned_generation_inspection().unwrap()
    }
    fn cost(&self) -> Option<FileLearnedConsistencySnapshot> {
        let host = self.driver.supervisor().host().unwrap();
        host.owned_learned_action_consistency_required().then(|| {
            let mut value = host.learned_action_consistency_snapshot().unwrap();
            value.consistency.journal_revision = 0; value
        })
    }
    fn publish(mut self) -> Self {
        let deadline = deadline(); let request = self.loaded.request;
        let mut control = Control::new(&self.config, request, None).unwrap();
        assert!(generate_selected(&mut self.driver, &self.reviewer, &mut self.config,
            &mut control, &deadline, &mut || ElapsedTick(1000), self.prediction.as_mut()).unwrap());
        assert!(matches!(self.driver.supervisor().host().unwrap().identity_status().unwrap(),
            IdentityStatus::Matching { check, .. } if check == self.loaded.identity.as_ref().unwrap().check));
        submit(&mut self.driver, &mut self.wire, &mut self.config, &self.loaded,
            &deadline, &mut || ElapsedTick(1000)).unwrap();
        // A matched identity alone still supplies neither original effect key.
        assert!(self.driver.request_learned_human_approval_from_policy_file(&mut self.config.source,
            700 + request, ElapsedTick(50_000), || ElapsedTick(1000)).result.is_err());
        let (driver, history) = native::review_current(&mut self.config, &self.loaded,
            &self.reviewer, self.driver, &deadline);
        self.driver = driver; self.histories.push(history);
        assert!(matches!(self.driver.step_computed_from_policy_file(&mut self.config.source,
            || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::AwaitingHuman { .. }));
        let human = native::approve(&mut self.driver, &mut self.config, &self.reviewer, request);
        native::publish(&mut self.driver, &mut self.config, &human);
        assert!(executed(&mut self.wire, request)); self
    }
    fn close(self) {
        assert_eq!(cleanup(self.driver, 1000, 1), 0);
        drop(self.histories); drop(self.identity); drop(self.prediction); drop(self.wire);
    }
}

#[test]
fn computed_identity_preserves_original_numerics_then_all_three_modes_publish_with_both_keys() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new(); let mut case = Case::new(&root, mode, false, false);
        let before = case.numerical(); let cost = case.cost();
        assert!(case.measure().unwrap());
        let current = case.numerical();
        assert_eq!(current.numerical, before.numerical);
        assert_eq!(current.pending, before.pending); assert_eq!(current.paused, before.paused);
        assert_eq!(case.cost(), cost);
        let run = &case.identity.as_ref().unwrap().history()[0];
        assert_eq!(run.status(), ComputedIdentityStatus::Committed);
        assert_eq!(run.work().planned_tokens, 5);
        assert_eq!(run.work().entered_tokens, 5); assert_eq!(run.work().completed_tokens, 5);
        assert_eq!(run.work().measured_anchors, 2);
        assert_eq!(run.result().unwrap().observation.measurement.as_ref().unwrap().outcome, IdentityOutcome::Matched);
        let case = case.publish();
        let host = case.driver.supervisor().host().unwrap();
        assert_eq!(host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), ["A"]);
        assert_eq!(host.inspect().executions, 1);
        assert_eq!(host.inspect().control.ledger.charged, host.request_action(1).unwrap().spec().units);
        assert_eq!(host.inspect().control.ledger.reserved, 0);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 2);
        if mode.is_some() { assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1); }
        drop(host); case.close();
    }
}

#[test]
fn identical_labels_and_manifest_cannot_hide_a_real_weight_substitution_from_the_live_operator() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new(); let mut case = Case::new(&root, mode, true, false);
        // Successful loader and native first-image creation above prove this is
        // an actual-model comparison, not a model/fit metadata refusal.
        let before = case.numerical(); let cost = case.cost();
        assert!(case.measure().unwrap_err().contains("identity refused"));
        let host = case.driver.supervisor().host().unwrap();
        let report = host.identity_report(901).unwrap();
        assert_eq!(report.manifest.as_ref(), Some(&manifest()));
        assert_eq!(report.outcome, IdentityOutcome::Mismatch(IdentityMismatch::Anchor { anchor: 10 }));
        assert!(host.identity_installation(901).unwrap().is_some());
        assert!(host.inspect().control.suspended); assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().control.ledger.charged, 0);
        assert!(host.request_status(1).is_err());
        let current = host.learned_generation_inspection().unwrap();
        assert_eq!(current.numerical, before.numerical); assert_eq!(current.pending, before.pending);
        let run = &case.identity.as_ref().unwrap().history()[0];
        assert!(run.work().completed_tokens >= 2); assert!(run.work().completed_tokens <= 5);
        assert_eq!(run.status(), ComputedIdentityStatus::Committed);
        drop(host); assert_eq!(case.cost(), cost); case.close();
    }
}

#[test]
fn actual_command_entry_pairs_manifest_mismatch_with_computed_two_key_publication() {
    use fa_reference::action::consequence::delivery::persistent::observed::reviewer::{
        client::ReviewClientProgress, wire::ReviewDecision,
    };
    use fa_reference::action::consequence::delivery::stream::ReleaseFrame;
    for mismatch in [true, false] {
        let root = Root::new(); let config = configured(&root); let path = native::write(&root);
        provision(&root, &path, false); evidence(&root, &config, 1, true, false);
        let mut observed = manifest();
        if mismatch { observed.weights = [9; 32]; }
        fs::write(root.0.join("observed.manifest"), identity_wire::encode_manifest(&observed)).unwrap();
        let loaded = recipe::load(&path, &config, true).unwrap();
        let peers = fixture::peers(&root, &config);
        let mut human = None;
        let result = run(config, loaded, &peers, false, || {
            // Start the independent reviewer only when the original human
            // endpoint exists. Identity refusal must never reach that offer.
            if human.is_none() && peers.socket(1).exists() {
                let peers = peers.clone();
                human = Some(std::thread::spawn(move || {
                    let started = Instant::now();
                    let mut client = peers.connect_client(1).unwrap();
                    let mut packet = None;
                    loop {
                        assert!(started.elapsed() < Duration::from_secs(15),
                            "computed identity publication review timed out");
                        match client.step().unwrap() {
                            ReviewClientProgress::NeedsDecision => {
                                assert!(packet.is_none());
                                packet = Some(client.packet().unwrap().clone());
                                client.respond(ReviewDecision::Approve).unwrap();
                            }
                            ReviewClientProgress::Complete => return packet.unwrap(),
                            _ => std::thread::sleep(Duration::from_millis(1)),
                        }
                    }
                }));
            }
            ElapsedTick(1000)
        }).unwrap();
        let packet = human.map(|reviewer| reviewer.join().unwrap());
        assert_eq!(result.cleanup_pending, 0);
        let config = configured(&root); let loaded = recipe::load(&path, &config, true).unwrap();
        let (host, _, _, _) = recovery::open_selected(&config, &loaded).unwrap();
        let report = host.identity_report(901).unwrap();
        assert_eq!(report.manifest.as_ref(), Some(&observed));
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        if mismatch {
            assert!(result.failure.unwrap().contains("identity refused"));
            assert!(packet.is_none());
            assert_eq!(report.outcome, IdentityOutcome::Mismatch(IdentityMismatch::Manifest));
            assert!(report.observations.is_empty());
            assert_eq!(numerical.sampled_draws, 0); assert_eq!(numerical.position, 0);
            assert!(host.inspect().control.suspended); assert_eq!(host.inspect().executions, 0);
            assert_eq!(host.inspect().control.ledger.charged, 0);
            assert!(host.request_status(1).is_err());
        } else {
            assert!(result.failure.is_none(), "{:?}", result.failure);
            assert!(matches!(result.response.result,
                Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
            let packet = packet.expect("actual authenticated reviewer approved the original offer");
            assert_eq!(packet.views().len(), 2);
            assert_eq!(ReleaseFrame::decode(&packet.action().spec().payload).unwrap().message(), Some("A"));
            assert_eq!(report.outcome, IdentityOutcome::Matched);
            assert_eq!(report.observations.len(), 2);
            assert!(host.identity_installation(901).unwrap().is_some());
            assert_eq!(numerical.sampled_draws, 2); assert_eq!(numerical.position, 4);
            assert_eq!(host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), ["A"]);
            assert_eq!(host.inspect().executions, 1);
            assert_eq!(host.inspect().control.ledger.charged, packet.action().spec().units);
            assert_eq!(host.inspect().control.ledger.reserved, 0);
        }
    }
}

#[test]
fn mediated_creation_preserves_both_identity_and_forecast_custody_through_real_publication() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let (config, path, mut original) = super::mediated_topology::selected(&root, mode, false);
        provision(&root, &path, false);
        let mut loaded = recipe::load(&path, &config, true).unwrap();
        loaded.topology = original.topology.take();
        let deadline = deadline(); let mut control = Control::new(&config, 1, None).unwrap();
        let topology::Started { custody: (host, reviewer, forecast, observer), source, .. } =
            topology::start(&config, &loaded, false, &mut control, &deadline, &mut || ElapsedTick(1000)).unwrap();
        let prediction = forecast.map(|observer| prediction::Session::new(1, observer));
        let identity = Some(check::Session::new(observer.unwrap()));
        let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let mut driver = FileSupervisedDriver::new(supervisor);
        topology::attach(&mut driver, source).unwrap();
        topology::certify(&mut driver, &deadline, &mut || ElapsedTick(1000)).unwrap();
        let mut case = Case { config, path, loaded, reviewer, driver, wire: ActorWire::new(port),
            prediction, identity, histories: Vec::new() };
        assert!(case.measure().unwrap()); case.publish().close();
    }
}
