//! The original durable journal is the publication sink. Independently supplied
//! secret capabilities add a guard; synthetic models and same-user peers do not
//! establish provider authentication or hostile-process containment.
#[path = "credential_routes.rs"]
mod route_tests;
#[path = "credential_lifecycle.rs"]
mod lifecycle_tests;

use super::*;
use super::super::super::{audit as audit_command, sequence as resident, stopped as stop_command};
use fa_reference::action::{ActionSpec, ActionState, FrozenAction, VERSION};
use fa_reference::action::consequence::delivery::{
    EndpointOutcome,
    credential_broker::{BrokerCredential, BrokerRouteBinding, CredentialRotationRequest,
        ProviderCredential},
    persistent::{JournalError, Reconciliation, observed::{
        credential::{FileCredentialPermit, FileCredentialPolicy, FILE_OVERSIGHT_CREDENTIAL_PROFILE},
        publication::capture::{FileCaptureIdentity, FilePublicationCapture,
            heartbeat::feed::PublicationFeedBatch},
    }},
    publication_gate::changes::{PublicationChange, PublicationInputCut,
        freshness::PublicationHeartbeat},
    stream::StreamView,
};
use fa_reference::perimeter_inventory::LoadedPerimeterInventory;
use fa_reference::witness::refinement::index::routing::WitnessChange;
use fa_reference::{Error, Snapshot};
use std::collections::{BTreeMap, BTreeSet};

const SECRET: &[u8] = b"independently-provisioned-disposable-credential-7619";

fn binding() -> BrokerRouteBinding {
    BrokerRouteBinding { family: "publication".into(), route: "adapter:file-oversight".into() }
}

fn policy(config: &Config) -> FileCredentialPolicy {
    FileCredentialPolicy { family: "publication".into(), route: "adapter:file-oversight".into(),
        credential: "broker-token".into(),
        profile_generation: config.profile.delivery.target.contract_version }
}

fn inventory(config: &Config) -> String {
    let scope = config.profile.delivery.scope;
    format!(r#"{{"version":1,"families":[{{"scope":{{"tenant":{},"principal":{},"purpose":1}},"family":"publication","trust_domains":["actor","observation_and_analysis","enforcement","governance_and_investigation"],"credentials":[{{"credential":"broker-token","holder":"broker"}}],"routes":[{{"route":"adapter:file-oversight","effect":"file_write","profile":{{"id":"{}","generation":{}}},"trust_path":["actor","enforcement"],"threat":"direct_credential_or_egress","actor_credential":{{"kind":"broker_mediated"}},"mediation":"brokered_effects","bypass":"blocked","residual_nonclaims":["trusted host and journal sink"]}}],"residual_nonclaims":["not OS containment"]}}]}}"#,
        scope.tenant, scope.principal, FILE_OVERSIGHT_CREDENTIAL_PROFILE,
        config.profile.delivery.target.contract_version)
}

fn credential_profile(root: &Root, config: &Config, generation: u64, revoked: bool) -> String {
    let root = root.0.display();
    format!(r#"{{"schema":"fa.learned-file-credential/1","policy":{{"family":"publication","route":"adapter:file-oversight","credential":"broker-token","profile_generation":{}}},"epoch":{{"generation":{generation},"revoked":{revoked}}},"inventory":"{root}/inventory.json","broker_credential":"{root}/broker.bin","provider_credential":"{root}/provider.bin"}}"#,
        config.profile.delivery.target.contract_version)
}

fn credential_envelope(root: &Root, inner: &str) -> String {
    format!(r#"{{"schema":"fa.learned-credential-publication/1","credential":"{}/credential.json","recipe":{inner}}}"#,
        root.0.display())
}

fn provision_credential(root: &Root, config: &Config, recipe: &Path) {
    fs::write(root.0.join("inventory.json"), inventory(config)).unwrap();
    fs::write(root.0.join("credential.json"), credential_profile(root, config, 1, false)).unwrap();
    // Separate files represent separate trusted provisioning inputs. Neither
    // expectation is constructed by reading back the other file.
    fs::write(root.0.join("broker.bin"), SECRET).unwrap();
    fs::write(root.0.join("provider.bin"), SECRET).unwrap();
    fs::write(recipe, credential_envelope(root, &fs::read_to_string(recipe).unwrap())).unwrap();
}

fn bind(host: &FileOversight, config: &Config) -> FileCredentialPermit {
    let inventory = LoadedPerimeterInventory::from_json_bytes(inventory(config).as_bytes()).unwrap();
    host.bind_credential_pair(host.revision(), &inventory, &binding(),
        BrokerCredential::new(SECRET.to_vec()).unwrap(),
        ProviderCredential::new(SECRET.to_vec()).unwrap()).unwrap()
}

fn no_secret(bytes: &[u8]) {
    assert!(!bytes.windows(SECRET.len()).any(|part| part == SECRET));
}

fn packet_is_original(packet: &ReviewPacket) {
    assert_eq!(packet.views().len(), 2);
    assert_eq!(ReleaseFrame::decode(&packet.action().spec().payload).unwrap().message(), Some("A"));
    for view in packet.views().values() { no_secret(view.actual_input().submitted_bytes()); }
}

#[test]
fn credentialed_learned_operator_publishes_with_original_native_reviews_in_all_forecast_modes() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new(); let config = configured(&root); let path = native::write(&root);
        if let Some(owned) = mode { super::super::forecast::install(&root, &path, owned, 60_000); }
        let _producer = mode.map(|_| provision(&root, &config, &path, true, false));
        let original = recipe::load(&path, &config, true).unwrap();
        provision_credential(&root, &config, &path);
        evidence(&root, &config, 1, true, false);
        let loaded = recipe::load(&path, &config, true).unwrap();
        assert_eq!(loaded.generation, original.generation.with_required_credential_policy(policy(&config)).unwrap());
        let reader = Reader::new(&config, &loaded);
        let (result, packet) = service(&root, config, loaded, None, None, ReviewDecision::Approve,
            &mut |_| ElapsedTick(1000));
        assert!(result.failure.is_none(), "mode={mode:?}: {:?}", result.failure);
        assert!(confirmed(&result)); let packet = packet.unwrap(); packet_is_original(&packet);
        let state = reader.snapshot();
        assert_eq!(state.executions, 1); assert_eq!(state.payload, b"A");
        assert_eq!(state.control.ledger.charged, packet.action().spec().units);
        assert_eq!(state.control.ledger.reserved, 0); assert_eq!(state.control.ledger.stages.len(), 1);
        no_secret(&fs::read(reader.store.join("delivery.bin")).unwrap());
        no_secret(&result.response.encode());
        let config = configured(&root); let loaded = recipe::load(&path, &config, false).unwrap();
        let (host, _, _, _) = recovery::open_selected(&config, &loaded).unwrap();
        let status = host.credential_status().unwrap().unwrap();
        assert_eq!(status.policy, policy(&config)); assert_eq!(status.generation, 1);
        assert!(!status.revoked); assert_eq!(status.retained_changes, 0);
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, 4); assert_eq!(numerical.sampled_draws, 2);
    }
}

#[test]
fn credentials_compose_with_identity_joint_owned_forecast_topology_and_cannot_override_changed_witnesses() {
    use super::super::super::topology as custody;
    use fa_reference::action::consequence::oversight::identity::IdentityOutcome;

    for phantom in [false, true] {
        let root = Root::new();
        let (config, path, mut original) = super::super::mediated_topology::selected(&root, Some(true), false);
        super::super::identity_tests::provision(&root, &path, true);
        let mut producer = provision(&root, &config, &path, false, false);
        provision_credential(&root, &config, &path);
        let mut loaded = recipe::load(&path, &config, true).unwrap();
        assert_eq!(loaded.generation.required_credential_policy(), Some(&policy(&config)));
        assert!(loaded.generation.required_held_out_joint().is_some());
        assert!(loaded.generation.required_computed_identity_registration().is_some());
        assert!(loaded.generation.required_owned_pre_output_forecast().is_some());
        assert!(loaded.generation.required_publication_validation().is_some());
        loaded.topology = original.topology.take();
        loaded.anchor_output = original.anchor_output.take();
        let reader = Reader::new(&config, &loaded); let scope = config.profile.delivery.scope;
        let mut admitted = None;
        let (result, packet) = service(&root, config, loaded, None, None, ReviewDecision::Approve,
            &mut |reviewed| {
                if reviewed && admitted.is_none() {
                    let state = reader.snapshot();
                    if state.control.ledger.charged != 0 && state.executions == 0 {
                        // Both arms retain the same independently provisioned
                        // credential, identity, qualifications and original
                        // effect keys. Only this actual queried input differs.
                        let keys: &[u64] = if phantom { &[0, 1, 2, 4] } else { &[0, 2, 4, 99] };
                        producer.publish(1, structured(scope, 2, keys), ElapsedTick(1001)).unwrap();
                        admitted = Some(state);
                    }
                }
                ElapsedTick(if admitted.is_some() { 1001 } else { 1000 })
            });
        let admitted = admitted.expect("the witness mutation must follow the original charged dispatch");
        let packet = packet.expect("both original helper reviews and human approval must precede publication");
        packet_is_original(&packet);
        assert_eq!(admitted.control.ledger.charged, packet.action().spec().units);
        assert_eq!(confirmed(&result), !phantom, "{:?}", result.failure);
        let current = reader.snapshot();
        assert_eq!(current.executions, u64::from(!phantom));
        assert_eq!(current.payload, if phantom { Vec::new() } else { b"A".to_vec() });
        assert_eq!(current.control.ledger.charged, if phantom { 0 } else { packet.action().spec().units });
        assert_eq!(current.control.ledger.reserved, 0);
        if !phantom { assert!(result.failure.is_none(), "{:?}", result.failure); }
        let source = super::super::mediated_topology::checkpoint(&root, 1);
        assert!(source.work().checks >= 2);
        if !phantom { assert!(source.available()); }
        assert_eq!(source.history_anchor(), &anchor_files::load(&root.0.join("history-1.bin")).unwrap());
        no_secret(&fs::read(reader.store.join("delivery.bin")).unwrap());

        let config = configured(&root); let mut loaded = recipe::load(&path, &config, false).unwrap();
        loaded.recovery_anchor = Some(source.history_anchor().clone());
        let historical = root.0.join("credential-topology-history.json");
        let r = root.0.display();
        fs::write(&historical, format!(r#"{{"schema_version":"fa.mediated-submit/2","initial":{{"topology_path":"{r}/initial.json","inventory_path":"{r}/initial-inventory.json"}},"next":{{"topology_path":"{r}/missing-next.json","inventory_path":"{r}/missing-next-inventory.json"}},"operation":{},"source_checkpoint":"{r}/source-1.bin"}}"#,
            source.last_operation() + 1)).unwrap();
        custody::admit(&config, &mut loaded, Some(&historical), custody::Mode::Historical).unwrap();
        let (host, _, _, _) = recovery::open_selected(&config, &loaded).unwrap();
        let identity = host.identity_report(901).unwrap();
        assert_eq!(identity.outcome, IdentityOutcome::Matched); assert_eq!(identity.observations.len(), 2);
        assert!(host.identity_installation(901).unwrap().is_some());
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, 4); assert_eq!(numerical.sampled_draws, 2);
        let status = host.credential_status().unwrap().unwrap();
        assert_eq!(status.policy, policy(&config)); assert_eq!(status.generation, 1);
        assert!(!status.revoked); assert_eq!(status.retained_changes, 0);
    }
}

#[test]
fn credential_files_refuse_missing_mismatched_oversized_and_aliased_material_before_startup() {
    for variant in 0..5 {
        let root = Root::new(); let config = configured(&root); let path = native::write(&root);
        provision_credential(&root, &config, &path); evidence(&root, &config, 1, true, false);
        let loaded = recipe::load(&path, &config, true).unwrap();
        let reader = Reader::new(&config, &loaded); let peers = fixture::peers(&root, &config);
        match variant {
            0 => {},
            1 => fs::write(root.0.join("provider.bin"), b"different-independent-material").unwrap(),
            2 => fs::remove_file(root.0.join("broker.bin")).unwrap(),
            3 => fs::write(root.0.join("provider.bin"), vec![7; 4097]).unwrap(),
            _ => {
                fs::remove_file(root.0.join("provider.bin")).unwrap();
                fs::hard_link(root.0.join("broker.bin"), root.0.join("provider.bin")).unwrap();
            }
        }
        let mut human = None; let mut clocks = 0;
        let result = run(config, loaded, &peers, false, || {
            clocks += 1;
            if variant == 0 && human.is_none() && peers.socket(1).exists() {
                human = Some(super::peers::human(peers.clone(), 1, ReviewDecision::Approve));
            }
            ElapsedTick(1000)
        });
        if variant == 0 {
            let result = result.unwrap();
            assert!(result.failure.is_none(), "{:?}", result.failure); assert!(confirmed(&result));
            packet_is_original(&human.unwrap().join().unwrap());
            let state = reader.snapshot(); assert_eq!(state.executions, 1);
            assert!(state.control.ledger.charged > 0); assert_eq!(result.cleanup_pending, 0);
            no_secret(&fs::read(reader.store.join("delivery.bin")).unwrap());
        } else {
            let failure = result.err().expect("bad live credential material must refuse startup");
            no_secret(failure.as_bytes()); assert!(human.is_none()); assert!(clocks > 0);
            assert!(!reader.store.exists());
        }
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    }
}

#[test]
fn a_credential_cannot_replace_either_key_and_foreign_or_absent_permits_cannot_retry_publication() {
    for variant in 0..3 {
        let root = Root::new(); let mut config = configured(&root); let path = native::write(&root);
        provision_credential(&root, &config, &path); evidence(&root, &config, 1, true, false);
        let loaded = recipe::load(&path, &config, true).unwrap();
        let (host, reviewer, _, _) = recovery::create_selected(&config, &loaded).unwrap();
        let own = bind(&host, &config);
        let foreign_root = Root::new(); let foreign_config = configured(&foreign_root);
        let foreign_path = native::write(&foreign_root);
        provision_credential(&foreign_root, &foreign_config, &foreign_path);
        let foreign_loaded = recipe::load(&foreign_path, &foreign_config, true).unwrap();
        let (foreign_host, _, _, _) = recovery::create_selected(&foreign_config, &foreign_loaded).unwrap();
        let foreign = bind(&foreign_host, &foreign_config);
        let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let mut driver = FileSupervisedDriver::new(supervisor); let mut wire = ActorWire::new(port);
        let deadline = deadline(); let mut control = Control::new(&config, 1, None).unwrap();
        assert!(generate(&mut driver, &reviewer, &mut config, &mut control,
            &deadline, &mut || ElapsedTick(1000)).unwrap());
        submit(&mut driver, &mut wire, &mut config, &loaded, &deadline,
            &mut || ElapsedTick(1000)).unwrap();
        assert!(driver.request_learned_human_approval_from_policy_file(&mut config.source,
            701, ElapsedTick(50_000), || ElapsedTick(1000)).result.is_err());
        assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
        let (mut driver, history) = native::review_current(&mut config, &loaded, &reviewer, driver, &deadline);
        assert!(matches!(driver.step_computed_from_policy_file_with_credential(&mut config.source,
            || ElapsedTick(1000), None, &own).result.unwrap(), FileDriverEvent::AwaitingHuman { request: 1 }));
        assert_eq!(driver.supervisor().host().unwrap().inspect().control.ledger.charged, 0);
        let human = native::approve(&mut driver, &mut config, &reviewer, 1);
        assert!(matches!(driver.step_computed_from_policy_file_with_credential(&mut config.source,
            || ElapsedTick(1000), Some(&human), &own).result.unwrap(), FileDriverEvent::Dispatched { request: 1, .. }));
        let admitted = driver.supervisor().host().unwrap().inspect();
        let key = driver.supervisor().host().unwrap().human_status(701).unwrap();
        assert!(admitted.control.ledger.charged > 0); assert_eq!(admitted.executions, 0);
        let published = if variant == 1 {
            driver.step_computed_from_policy_file(&mut config.source, || ElapsedTick(1000), None).result.unwrap()
        } else {
            driver.step_computed_from_policy_file_with_credential(&mut config.source,
                || ElapsedTick(1000), None, if variant == 0 { &own } else { &foreign }).result.unwrap()
        };
        if variant == 0 {
            assert!(matches!(published, FileDriverEvent::PublicationChecked { publication, .. }
                if publication.outcome == EndpointOutcome::Executed { resulting_version: 2 }));
        } else {
            assert!(matches!(published, FileDriverEvent::PublicationUnknown { error: JournalError::Contract(error), .. }
                if error == if variant == 1 { Error::Incomplete } else { Error::Binding }));
            assert_eq!(driver.phase(), FileDriverPhase::AwaitingReconciliation { request: 1 });
            assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
        }
        // A later correct capability cannot return the original driver to its
        // retired send phase. Reconciliation is the only remaining operation.
        let resolved = driver.step_computed_from_policy_file_with_credential(&mut config.source,
            || ElapsedTick(1000), None, &own).result.unwrap();
        assert!(matches!(resolved, FileDriverEvent::Reconciled { outcome, .. }
            if outcome == if variant == 0 {
                Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 })
            } else { Reconciliation::AwaitingResolution }));
        let host = driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().executions, u64::from(variant == 0));
        assert_eq!(host.inspect().control.ledger.charged, admitted.control.ledger.charged);
        assert_eq!(host.human_status(701).unwrap(), key);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 2);
        drop(host); drop(history); drop(wire); assert_eq!(cleanup(driver, 1000, 1), 0);
    }
}

fn replace(root: &Root, name: &str, bytes: &[u8]) {
    let pending = root.0.join(format!("{name}.next"));
    fs::write(&pending, bytes).unwrap(); fs::rename(pending, root.0.join(name)).unwrap();
}

fn capture(action: &FrozenAction, generation: u64, through: u64) -> Vec<u8> {
    let keys: &[u64] = if generation == 1 { &[0, 2, 4] } else { &[0, 2, 4, 99] };
    FilePublicationCapture::new_at_cut(1, FileCaptureIdentity { source: 91, generation }, action,
        structured(action.spec().scope, generation, keys), PublicationInputCut { source: 41, through })
        .unwrap().to_bytes().unwrap()
}

fn captures(path: &Path) -> usize {
    // Only retained captured-packet events contain this header in these fixed
    // fixtures. This observes acknowledgment without numerical replay per tick.
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return 0,
        Err(error) => panic!("original capture history read failed: {error}"),
    };
    bytes.windows(8).filter(|window| *window == b"FAPCAP02").count()
}

#[test]
fn credentialed_wait_preserves_native_and_process_reviews_and_uses_the_same_live_capability() {
    for process in [false, true] {
        let root = Root::new(); let config = configured(&root);
        let path = if process {
            let path = fixture::write(&root, b"allow");
            fs::write(&path, super::super::processes::process_recipe(&root, "allow")).unwrap(); path
        } else { native::write(&root) };
        fs::write(root.0.join("publication.json"), recipe_tests::captures(&root, 4)
            .replace("\"max_retries\":3", "\"max_retries\":1")).unwrap();
        fs::write(&path, envelope(&root, &fs::read_to_string(&path).unwrap())).unwrap();
        provision_credential(&root, &config, &path); evidence(&root, &config, 1, true, false);
        let loaded = recipe::load(&path, &config, true).unwrap();
        let payload = StreamView::empty(loaded.stream).encode_message("A").unwrap();
        let (mut producer, _) = FileOversight::create(root.0.join("capture-owner"), config.profile.clone()).unwrap();
        producer.observe_time(producer.revision(), ElapsedTick(1000)).unwrap();
        let action = producer.propose(producer.revision(), 1, ActionSpec { version: VERSION,
            scope: config.profile.delivery.scope, target: Some(config.profile.delivery.target),
            units: payload.len() as u64, payload, required_witnesses: Vec::new(), policy_epoch: 0,
            deadline: ElapsedTick(50_000) }, Snapshot { semantic_epoch: 1, complete: true,
                values: BTreeMap::from([(7, b"ok".to_vec())]) }).unwrap();
        drop(producer);
        replace(&root, "original.bin", &capture(&action, 1, 0));
        replace(&root, "current.bin", &capture(&action, 1, 0));
        let feed = PublicationFeedBatch::new(PublicationHeartbeat { source: 41,
            clock_domain: CLOCK_DOMAIN, generation: 2, through: 1, produced_at: ElapsedTick(1000) },
            0, vec![PublicationChange { source: 41, sequence: 1, change: WitnessChange::All }]).unwrap();
        replace(&root, "feed.bin", &feed.to_bytes().unwrap());
        let reader = Reader::new(&config, &loaded); let canonical = reader.store.join("delivery.bin");
        let mut before_human = None; let mut caught_up = false;
        let (result, packet) = service(&root, config, loaded, None, None, ReviewDecision::Approve,
            &mut |human| {
                if human && !caught_up {
                    let current = captures(&canonical);
                    match before_human {
                        None => before_human = Some(current),
                        Some(before) if current > before => {
                            let waiting = reader.snapshot();
                            assert_eq!(waiting.executions, 0); assert_eq!(waiting.control.ledger.charged, 0);
                            // The prepared request already owns a credential.
                            // Waiting cannot reopen the provisioning inputs.
                            fs::remove_file(root.0.join("broker.bin")).unwrap();
                            fs::remove_file(root.0.join("provider.bin")).unwrap();
                            replace(&root, "current.bin", &capture(&action, 2, 1));
                            caught_up = true;
                        }
                        _ => {},
                    }
                }
                ElapsedTick(1000)
            });
        assert!(caught_up); assert!(before_human.unwrap() > 1);
        assert!(result.failure.is_none(), "process={process}: {:?}", result.failure); assert!(confirmed(&result));
        let packet = packet.unwrap(); packet_is_original(&packet); assert_eq!(packet.action(), &action);
        let state = reader.snapshot(); assert_eq!(state.executions, 1); assert_eq!(state.payload, b"A");
        assert_eq!(state.control.ledger.charged, action.spec().units); assert_eq!(state.control.ledger.reserved, 0);
        assert_eq!(state.control.ledger.stages.len(), 1); no_secret(&fs::read(&canonical).unwrap());
        if process {
            for name in ["alpha", "beta"] {
                let input = fs::read(root.0.join(format!("{name}.input"))).unwrap(); no_secret(&input);
                assert!(input.windows(8).any(|part| part == b"FASIDE\0\x01"));
            }
        }
    }
}

#[test]
fn credential_history_requires_exact_policy_and_rotated_epoch_before_cleanup_without_live_secrets() {
    let root = Root::new(); let config = configured(&root); let path = native::write(&root);
    super::super::forecast::install(&root, &path, true, 60_000);
    let producer = provision(&root, &config, &path, true, false);
    provision_credential(&root, &config, &path); evidence(&root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    let reader = Reader::new(&config, &loaded);
    let (result, packet) = service(&root, config, loaded, None, None, ReviewDecision::Approve,
        &mut |_| ElapsedTick(1000));
    assert!(result.failure.is_none()); assert!(confirmed(&result)); packet_is_original(&packet.unwrap());
    let config = configured(&root); let loaded = recipe::load(&path, &config, false).unwrap();
    let (mut host, _, _, _) = recovery::open_selected(&config, &loaded).unwrap();
    let paid = host.inspect();
    host.rotate_credential_guard(host.revision(), CredentialRotationRequest {
        operation: 501, expected_generation: 1, next_generation: 2 }).unwrap();
    assert_eq!(host.credential_status().unwrap().unwrap().generation, 2);
    assert_eq!(host.inspect().control.ledger.charged, paid.control.ledger.charged);
    let anchor = host.history_anchor().unwrap(); drop(host);
    let selected = credential_profile(&root, &config, 2, false);
    fs::write(root.0.join("credential.json"), &selected).unwrap();
    let original = fs::read_to_string(&path).unwrap();
    let exact = recipe::load(&path, &config, false).unwrap();
    assert_eq!(exact.generation, loaded.generation, "rotation is not a new numerical recipe");
    let predictor = exact.generation.required_owned_pre_output_forecast().unwrap();
    let cost = FileOversight::read_publication_with_owned_learned_consistency(&config.store,
        &config.profile, &exact.generation, predictor).unwrap().consistency;
    let canonical = config.store.join("delivery.bin"); let journal = fs::read(&canonical).unwrap();
    let pending = config.store.join("delivery.pending");
    fs::write(&pending, b"preserve unacknowledged credential staging").unwrap();
    drop(producer); fs::remove_file(root.0.join("producer/delivery.bin")).unwrap();
    for name in ["broker.bin", "provider.bin", "inventory.json", "evidence.json", "native-roster.json",
        "helper-model.json", "helper-weights.safetensors", "helper-monitor.json",
        "helper-tokenizer.bbpe", "helper-sampling.json"] {
        fs::remove_file(root.0.join(name)).unwrap();
    }
    fs::remove_dir(root.0.join("peers")).unwrap();
    let reads = config.source.status().read_attempts;
    let mut historical_audit = Vec::new();
    audit_command::run(&config, &exact, 1, &anchor, &mut historical_audit, &mut || Ok(())).unwrap();
    let omitted = original.split_once("\"recipe\":").unwrap().1.strip_suffix('}').unwrap().to_owned();
    let wrongs = [
        (original.clone(), selected.replacen("\"credential\":\"broker-token\"",
            "\"credential\":\"different-token\"", 1), true),
        (original.clone(), selected.replacen("\"family\":\"publication\"",
            "\"family\":\"different-family\"", 1), true),
        (original.clone(), selected.replacen("\"route\":\"adapter:file-oversight\"",
            "\"route\":\"adapter:other-route\"", 1), true),
        (original.clone(), selected.replacen("\"profile_generation\":1",
            "\"profile_generation\":2", 1), true),
        (original.clone(), credential_profile(&root, &config, 1, false), false),
        (original.clone(), credential_profile(&root, &config, 2, true), false),
        (omitted, selected.clone(), true),
    ];
    for (document, profile, changed_policy) in wrongs {
        fs::write(&path, document).unwrap(); fs::write(root.0.join("credential.json"), profile).unwrap();
        let mut wrong = recipe::load(&path, &config, false).unwrap();
        wrong.recovery_anchor = Some(anchor.clone());
        assert!(recovery::open_selected(&config, &wrong).is_err());
        assert!(recovery::readonly::read(&config, &wrong, None, &mut || Duration::ZERO).is_err());
        let mut output = Vec::new();
        let audit = audit_command::run(&config, &wrong, 1, &anchor, &mut output, &mut || Ok(()));
        if changed_policy {
            assert!(audit.is_err()); assert!(output.is_empty());
        } else {
            // An anchored historical review makes no current credential claim.
            // Rotation/revocation selection cannot rewrite that original audit,
            // while all authority/receipt/Stop recovery below requires its epoch.
            audit.unwrap(); assert_eq!(output, historical_audit);
        }
        assert!(stop_command::run(&config, &wrong, 801,
            || panic!("wrong policy or credential epoch reached terminal completion"), &mut Vec::new()).is_err());
        assert_eq!(fs::read(&canonical).unwrap(), journal);
        assert_eq!(fs::read(&pending).unwrap(), b"preserve unacknowledged credential staging");
    }
    fs::write(&path, original).unwrap(); fs::write(root.0.join("credential.json"), selected).unwrap();
    let mut exact = recipe::load(&path, &config, false).unwrap();
    exact.recovery_anchor = Some(anchor.clone());
    assert!(exact.native.is_none()); assert_eq!(exact.generation, loaded.generation);
    let receipt = recovery::readonly::read(&config, &exact, None, &mut || Duration::ZERO).unwrap();
    assert_eq!(receipt.result, result.response.result);
    let mut report = Vec::new();
    audit_command::run(&config, &exact, 1, &anchor, &mut report, &mut || Ok(())).unwrap();
    assert_eq!(report, historical_audit);
    no_secret(&report);
    let decoded = fa_reference::strict_json::parse(&report, fa_reference::strict_json::Limits::default()).unwrap();
    let value = decoded.get("knowledge").unwrap().get("value").unwrap();
    assert_eq!(value.get("application").unwrap().get("status").unwrap().as_str(), Some("applied"));
    assert_eq!(value.get("application_inputs").unwrap().as_str(), Some("equal_to_reviewed"));
    assert_eq!(value.get("historical_effects").unwrap().get("executions_at_journal_cut").unwrap().as_str(), Some("1"));
    assert_eq!(fs::read(&canonical).unwrap(), journal);
    assert_eq!(fs::read(&pending).unwrap(), b"preserve unacknowledged credential staging");
    let mut clocks = 0;
    stop_command::run(&config, &exact, 801, || { clocks += 1; ElapsedTick(1001) }, &mut Vec::new()).unwrap();
    assert_eq!(clocks, 1); assert!(!pending.exists());
    let stopped = reader.snapshot();
    assert_eq!(stopped.executions, paid.executions); assert_eq!(stopped.payload, paid.payload);
    assert_eq!(stopped.control.ledger.charged, paid.control.ledger.charged);
    assert_eq!(stopped.control.ledger.reserved, 0);
    let current = FileOversight::read_publication_with_owned_learned_consistency(&config.store,
        &config.profile, &exact.generation, exact.generation.required_owned_pre_output_forecast().unwrap())
        .unwrap().consistency;
    assert_eq!(current.work, cost.work); assert_eq!(current.retained_source_bytes, cost.retained_source_bytes);
    assert_eq!(current.consistency.evidence, cost.consistency.evidence);
    let completed = fs::read(&canonical).unwrap();
    stop_command::run(&config, &exact, 801,
        || panic!("completed credentialed Stop needs no new clock or secret"), &mut Vec::new()).unwrap();
    assert_eq!(fs::read(&canonical).unwrap(), completed); no_secret(&completed);
    assert_eq!(config.source.status().read_attempts, reads); assert!(!root.0.join("peers").exists());
}

#[test]
fn resident_credentialed_messages_and_finish_keep_one_capability_after_provisioning_files_disappear() {
    let root = Root::new(); let config = configured(&root); let first = native::write(&root);
    super::super::forecast::install(&root, &first, true, 60_000);
    provision_credential(&root, &config, &first); evidence(&root, &config, 1, true, false);
    let initial = recipe::load(&first, &config, true).unwrap();
    let (_, second) = native::next(&root, &config, 2);
    let (_, last) = native::next(&root, &config, 3);
    assert_eq!(initial.generation, second.generation); assert_eq!(initial.generation, last.generation);
    let reader = Reader::new(&config, &initial);
    let entries = [initial, second, last].into_iter().enumerate().map(|(index, loaded)| {
        let mut actor = peers::actor_profile(&root, &config, loaded.request);
        actor.socket = root.0.join(format!("peers/credential-actor-{}.sock", loaded.request));
        resident::Entry { loaded, actor,
            release: if index == 2 { LearnedTextRelease::Finish } else { LearnedTextRelease::Message } }
    }).collect::<Vec<_>>();
    let actors = entries.iter().map(|entry| entry.actor.clone()).collect::<Vec<_>>();
    let reviewer = fixture::peers(&root, &config);
    let mut clients = [None, None, None]; let mut humans = [None, None, None];
    let mut first_cut = None;
    let result = resident::run(config, entries, &reviewer, false, None, None, || {
        if first_cut.is_none() && actors[1].socket.exists() {
            let state = reader.snapshot();
            assert_eq!(state.executions, 1); assert_eq!(state.payload, b"A");
            assert_eq!(state.control.ledger.stages[&1], ActionState::Confirmed);
            for name in ["broker.bin", "provider.bin", "inventory.json"] {
                fs::remove_file(root.0.join(name)).unwrap();
            }
            first_cut = Some(state);
        }
        for index in 0..3 {
            if clients[index].is_none() && actors[index].socket.exists() {
                clients[index] = Some(peers::actor(actors[index].clone(),
                    if index == 2 { LearnedTextRelease::Finish } else { LearnedTextRelease::Message },
                    ElapsedTick(50_000 - index as u64 * 1000), peers::Behavior::Observe));
            }
            if humans[index].is_none() && reviewer.socket(index as u64 + 1).exists() {
                humans[index] = Some(peers::human(reviewer.clone(), index as u64 + 1, ReviewDecision::Approve));
            }
        }
        ElapsedTick(1000)
    }).unwrap();
    let first_cut = first_cut.expect("first original receipt must precede second admission");
    assert!(result.failure.is_none(), "{:?}", result.failure); assert_eq!(result.cleanup_pending, 0);
    assert_eq!(result.responses.len(), 3);
    let responses = clients.map(|client| client.unwrap().join().unwrap());
    let packets = humans.map(|human| human.unwrap().join().unwrap());
    let mut sessions = BTreeSet::new(); let mut charged = 0;
    for (index, (response, packet)) in responses.into_iter().zip(packets).enumerate() {
        assert_eq!(response.result, result.responses[index].result);
        assert!(matches!(response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
        assert!(sessions.insert(packet.binding().session)); assert_eq!(packet.views().len(), 2);
        for view in packet.views().values() { no_secret(view.actual_input().submitted_bytes()); }
        let frame = ReleaseFrame::decode(&packet.action().spec().payload).unwrap();
        assert_eq!(frame.prior_messages(), vec!["A"; index.min(2)]);
        assert_eq!(frame.message(), if index < 2 { Some("A") } else { None });
        if index == 0 { assert_eq!(first_cut.control.ledger.charged, packet.action().spec().units); }
        charged += packet.action().spec().units;
    }
    let state = reader.snapshot(); assert_eq!(state.executions, 3); assert_eq!(state.payload, b"AA");
    assert_eq!(state.control.ledger.charged, charged); assert_eq!(state.control.ledger.reserved, 0);
    assert_eq!(state.control.ledger.epoch, 0); assert!(state.stop.is_none());
    assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    no_secret(&fs::read(reader.store.join("delivery.bin")).unwrap());
    let config = configured(&root); let loaded = recipe::load(&first, &config, false).unwrap();
    let (host, _, _, _) = recovery::open_selected(&config, &loaded).unwrap();
    let status = host.credential_status().unwrap().unwrap();
    assert_eq!(status.policy, policy(&config)); assert_eq!(status.generation, 1);
    assert!(!status.revoked); assert_eq!(status.retained_changes, 0);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(numerical.position, 6); assert_eq!(numerical.sampled_draws, 4);
    let stream = host.stream_snapshot().unwrap();
    assert!(stream.confirmed.finished()); assert_eq!(stream.confirmed, stream.published);
    assert_eq!(stream.confirmed.messages().collect::<Vec<_>>(), ["A", "A"]);
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 3);
}

#[test]
fn resident_credential_selection_cannot_change_policy_epoch_or_provisioning_path_between_requests() {
    let root = Root::new(); let config = configured(&root); let first = native::write(&root);
    provision_credential(&root, &config, &first);
    let (second_path, _) = native::next(&root, &config, 2);
    let original = fs::read_to_string(&second_path).unwrap();
    let selected = fs::read_to_string(root.0.join("credential.json")).unwrap();
    let declared = fs::read_to_string(root.0.join("inventory.json")).unwrap();
    for variant in 0..4 {
        fs::write(&second_path, &original).unwrap();
        fs::write(root.0.join("credential.json"), &selected).unwrap();
        fs::write(root.0.join("inventory.json"), &declared).unwrap();
        let initial = recipe::load(&first, &config, true).unwrap();
        match variant {
            0 => fs::write(&second_path,
                original.split_once("\"recipe\":").unwrap().1.strip_suffix('}').unwrap()).unwrap(),
            1 => {
                fs::write(root.0.join("credential.json"), selected.replace("broker-token", "other-token")).unwrap();
                fs::write(root.0.join("inventory.json"), declared.replace("broker-token", "other-token")).unwrap();
            }
            2 => fs::write(root.0.join("credential.json"), credential_profile(&root, &config, 2, false)).unwrap(),
            _ => fs::write(root.0.join("credential.json"), selected.replace("/provider.bin", "/other-provider.bin")).unwrap(),
        }
        let second = recipe::load(&second_path, &config, true).unwrap();
        let entries = [initial, second].into_iter().map(|loaded| {
            let mut actor = peers::actor_profile(&root, &config, loaded.request);
            actor.socket = root.0.join(format!("peers/credential-actor-{}.sock", loaded.request));
            resident::Entry { loaded, actor, release: LearnedTextRelease::Message }
        }).collect::<Vec<_>>();
        let reviewer = fixture::peers(&root, &config);
        let result = resident::run(configured(&root), entries, &reviewer, false, None, None,
            || panic!("changed resident credential selection reached live startup"));
        assert!(result.is_err(), "variant={variant}");
        assert!(!config.store.exists()); assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    }
}

#[test]
fn credential_recipe_is_closed_and_route_mismatch_refuses_before_loading_model_assets() {
    let root = Root::new(); let config = configured(&root); let path = native::write(&root);
    let original = recipe::load(&path, &config, true).unwrap();
    provision_credential(&root, &config, &path);
    let valid = fs::read_to_string(&path).unwrap();
    let selected = fs::read_to_string(root.0.join("credential.json")).unwrap();
    let loaded = recipe::load(&path, &config, true).unwrap();
    assert_eq!(loaded.generation, original.generation.with_required_credential_policy(policy(&config)).unwrap());
    assert_eq!(loaded.request, original.request); assert_eq!(loaded.rounds, original.rounds);
    assert!(!config.store.exists());
    fs::remove_file(root.0.join("weights.safetensors")).unwrap();
    let invalid = [
        (credential_envelope(&root, &valid), selected.clone()),
        (envelope(&root, &valid), selected.clone()),
        (valid.replace("fa.learned-credential-publication/1", "fa.learned-credential-publication/2"), selected.clone()),
        (valid.replacen("\"credential\":", "\"credential_bypass\":true,\"credential\":", 1), selected.clone()),
        (valid.replace(root.0.join("credential.json").to_str().unwrap(), "relative.json"), selected.clone()),
        (valid.clone(), selected.replacen("\"policy\":", "\"raw_secret\":\"forbidden\",\"policy\":", 1)),
        (valid.clone(), selected.replace("\"epoch\":{\"generation\":1", "\"epoch\":{\"generation\":0")),
        (valid.clone(), selected.replace("broker-token", "unregistered-token")),
        (valid.clone(), selected.replace("/provider.bin", "/broker.bin")),
    ];
    for (recipe, profile) in invalid {
        fs::write(&path, recipe).unwrap(); fs::write(root.0.join("credential.json"), profile).unwrap();
        let failure = recipe::load(&path, &config, true).err().expect("invalid credential selection must refuse");
        assert!(!failure.contains("No such file"), "invalid selection reached model intake: {failure}");
        no_secret(failure.as_bytes()); assert!(!config.store.exists());
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    }
}
