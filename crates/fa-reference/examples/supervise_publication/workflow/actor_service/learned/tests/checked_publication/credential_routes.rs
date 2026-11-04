//! Named v2 routes reach the original native reviewers and durable publication
//! sink with independent provisioning inputs. These fixed credentials establish
//! route selection and isolation, not external-provider authentication.
use super::*;

const ROUTES: [&str; 2] = ["adapter:file-a", "adapter:file-b"];
const CREDENTIALS: [&str; 2] = ["provider-a-token", "provider-b-token"];
const ROUTE_SECRETS: [&[u8]; 2] = [
    b"independently-provisioned-route-a-credential-7619",
    b"independently-provisioned-route-b-credential-8933",
];

fn named_policy(config: &Config, route: usize, credential: usize) -> FileCredentialPolicy {
    FileCredentialPolicy {
        family: "publication".into(),
        route: ROUTES[route].into(),
        credential: CREDENTIALS[credential].into(),
        profile_generation: config.profile.delivery.target.contract_version,
    }
}

fn named_inventory(config: &Config, version: u64) -> String {
    let scope = config.profile.delivery.scope;
    let routes = (0..2).map(|index| {
        let selected = if version == 2 {
            format!(r#", "credential":"{}""#, CREDENTIALS[index])
        } else { String::new() };
        format!(r#"{{"route":"{}","effect":"file_write","profile":{{"id":"{}","generation":{}}},"trust_path":["actor","enforcement"],"threat":"direct_credential_or_egress","actor_credential":{{"kind":"broker_mediated"{}}},"mediation":"brokered_effects","bypass":"blocked","residual_nonclaims":["trusted host and journal sink"]}}"#,
            ROUTES[index], FILE_OVERSIGHT_CREDENTIAL_PROFILE,
            config.profile.delivery.target.contract_version, selected)
    }).collect::<Vec<_>>().join(",");
    format!(
        r#"{{"version":{version},"families":[{{"scope":{{"tenant":{},"principal":{},"purpose":1}},"family":"publication","trust_domains":["actor","observation_and_analysis","enforcement","governance_and_investigation"],"credentials":[{{"credential":"{}","holder":"broker"}},{{"credential":"{}","holder":"broker"}}],"routes":[{routes}],"residual_nonclaims":["not OS containment"]}}]}}"#,
        scope.tenant, scope.principal, CREDENTIALS[0], CREDENTIALS[1]
    )
}

fn selected_profile(root: &Root, config: &Config, route: usize, credential: usize) -> String {
    credential_profile(root, config, 1, false)
        .replace("adapter:file-oversight", ROUTES[route])
        .replace("broker-token", CREDENTIALS[credential])
}

fn provision_named(root: &Root, config: &Config, path: &Path, index: usize) {
    provision_credential(root, config, path);
    fs::write(root.0.join("inventory.json"), named_inventory(config, 2)).unwrap();
    fs::write(
        root.0.join("credential.json"),
        selected_profile(root, config, index, index),
    )
    .unwrap();
    // Neither independently supplied expectation is obtained from the other
    // credential file; the two declared routes have different secret bytes.
    fs::write(root.0.join("broker.bin"), ROUTE_SECRETS[index]).unwrap();
    fs::write(root.0.join("provider.bin"), ROUTE_SECRETS[index]).unwrap();
}

fn no_route_secret(bytes: &[u8]) {
    no_secret(bytes);
    for secret in ROUTE_SECRETS {
        assert!(!bytes.windows(secret.len()).any(|part| part == secret));
    }
}

#[test]
fn both_named_routes_publish_with_their_original_credentials_in_all_forecast_modes() {
    for index in 0..2 {
        for mode in [None, Some(false), Some(true)] {
            let root = Root::new();
            let config = configured(&root);
            let path = native::write(&root);
            if let Some(owned) = mode {
                super::super::super::forecast::install(&root, &path, owned, 60_000);
            }
            if mode == Some(true) {
                super::super::super::identity_tests::provision(&root, &path, true);
            }
            let _producer = provision(&root, &config, &path, true, false);
            let original = recipe::load(&path, &config, true).unwrap();
            provision_named(&root, &config, &path, index);
            evidence(&root, &config, 1, true, false);
            let loaded = recipe::load(&path, &config, true).unwrap();
            let expected = named_policy(&config, index, index);
            assert_eq!(
                loaded.generation,
                original
                    .generation
                    .with_required_credential_policy(expected.clone())
                    .unwrap()
            );
            if mode == Some(true) {
                assert!(loaded.generation.required_held_out_joint().is_some());
                assert!(
                    loaded
                        .generation
                        .required_computed_identity_registration()
                        .is_some()
                );
            }
            let reader = Reader::new(&config, &loaded);
            let (result, packet) = service(
                &root,
                config,
                loaded,
                None,
                None,
                ReviewDecision::Approve,
                &mut |_| ElapsedTick(1000),
            );
            assert!(
                result.failure.is_none(),
                "route={index}, mode={mode:?}: {:?}",
                result.failure
            );
            assert!(confirmed(&result));
            let packet = packet.unwrap();
            packet_is_original(&packet);
            for view in packet.views().values() {
                no_route_secret(view.actual_input().submitted_bytes());
            }
            let state = reader.snapshot();
            assert_eq!(state.executions, 1);
            assert_eq!(state.payload, b"A");
            assert_eq!(state.control.ledger.charged, packet.action().spec().units);
            assert_eq!(state.control.ledger.reserved, 0);
            assert_eq!(state.control.ledger.stages.len(), 1);
            no_route_secret(&result.response.encode());
            no_route_secret(&fs::read(reader.store.join("delivery.bin")).unwrap());
            let config = configured(&root);
            let historical = recipe::load(&path, &config, false).unwrap();
            let (host, _, _, _) = recovery::open_selected(&config, &historical).unwrap();
            let status = host.credential_status().unwrap().unwrap();
            assert_eq!(status.policy, expected);
            assert_eq!(status.generation, 1);
            assert!(!status.revoked);
            assert_eq!(status.retained_changes, 0);
            let numerical = host.learned_generation_inspection().unwrap().numerical;
            assert_eq!(numerical.position, 4);
            assert_eq!(numerical.sampled_draws, 2);
        }
    }
}

#[test]
fn cross_route_secrets_wrong_selection_and_legacy_ambiguity_refuse_before_owner_creation() {
    for variant in 0..5 {
        let root = Root::new();
        let config = configured(&root);
        let path = native::write(&root);
        provision_named(&root, &config, &path, 1);
        evidence(&root, &config, 1, true, false);
        match variant {
            0 | 1 => {}
            2 => fs::write(
                root.0.join("credential.json"),
                selected_profile(&root, &config, 1, 0),
            )
            .unwrap(),
            3 => fs::write(
                root.0.join("credential.json"),
                selected_profile(&root, &config, 0, 1),
            )
            .unwrap(),
            _ => {
                let inventory = named_inventory(&config, 1);
                // The original v1 inventory remains a valid declaration, but
                // its two broker credentials cannot resolve either route.
                LoadedPerimeterInventory::from_json_bytes(inventory.as_bytes()).unwrap();
                fs::write(root.0.join("inventory.json"), inventory).unwrap();
            }
        }
        if variant >= 2 {
            fs::remove_file(root.0.join("weights.safetensors")).unwrap();
            let failure = recipe::load(&path, &config, true).err().unwrap();
            assert!(
                !failure.contains("No such file"),
                "route selection must precede model intake: {failure}"
            );
            no_route_secret(failure.as_bytes());
            assert!(!config.store.exists());
        } else {
            let loaded = recipe::load(&path, &config, true).unwrap();
            let reader = Reader::new(&config, &loaded);
            if variant == 0 {
                let (result, packet) = service(
                    &root,
                    config,
                    loaded,
                    None,
                    None,
                    ReviewDecision::Approve,
                    &mut |_| ElapsedTick(1000),
                );
                assert!(result.failure.is_none());
                assert!(confirmed(&result));
                packet_is_original(&packet.unwrap());
                assert_eq!(reader.snapshot().payload, b"A");
            } else {
                fs::write(root.0.join("provider.bin"), ROUTE_SECRETS[0]).unwrap();
                let peers = fixture::peers(&root, &config);
                let failure = run(config, loaded, &peers, false, || ElapsedTick(1000))
                    .err()
                    .unwrap();
                assert!(failure.contains(
                    "independently supplied broker and provider credentials do not match"
                ));
                no_route_secret(failure.as_bytes());
                assert!(!reader.store.exists());
            }
        }
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    }
}

#[test]
fn exact_named_route_and_credential_remain_bound_to_source_free_receipts_and_recovery() {
    let root = Root::new();
    let config = configured(&root);
    let path = native::write(&root);
    super::super::super::forecast::install(&root, &path, true, 60_000);
    let producer = provision(&root, &config, &path, true, false);
    provision_named(&root, &config, &path, 0);
    evidence(&root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    let reader = Reader::new(&config, &loaded);
    let (result, packet) = service(
        &root,
        config,
        loaded,
        None,
        None,
        ReviewDecision::Approve,
        &mut |_| ElapsedTick(1000),
    );
    assert!(result.failure.is_none());
    assert!(confirmed(&result));
    packet_is_original(&packet.unwrap());
    let config = configured(&root);
    let exact = recipe::load(&path, &config, false).unwrap();
    let (host, _, _, _) = recovery::open_selected(&config, &exact).unwrap();
    let anchor = host.history_anchor().unwrap();
    let paid = host.inspect();
    drop(host);
    let canonical = config.store.join("delivery.bin");
    let journal = fs::read(&canonical).unwrap();
    let pending = config.store.join("delivery.pending");
    fs::write(&pending, b"unacknowledged named-route staging").unwrap();
    drop(producer);
    fs::remove_file(root.0.join("producer/delivery.bin")).unwrap();
    for name in [
        "inventory.json",
        "broker.bin",
        "provider.bin",
        "evidence.json",
        "native-roster.json",
        "helper-model.json",
        "helper-weights.safetensors",
        "helper-monitor.json",
        "helper-tokenizer.bbpe",
        "helper-sampling.json",
    ] {
        fs::remove_file(root.0.join(name)).unwrap();
    }
    fs::remove_dir(root.0.join("peers")).unwrap();
    let source_reads = config.source.status().read_attempts;
    for (route, credential) in [(0, 1), (1, 0), (1, 1)] {
        fs::write(
            root.0.join("credential.json"),
            selected_profile(&root, &config, route, credential),
        )
        .unwrap();
        let mut changed = recipe::load(&path, &config, false).unwrap();
        changed.recovery_anchor = Some(anchor.clone());
        assert!(recovery::open_selected(&config, &changed).is_err());
        assert!(recovery::readonly::read(&config, &changed, None, &mut || Duration::ZERO).is_err());
        let mut report = Vec::new();
        assert!(
            audit_command::run(&config, &changed, 1, &anchor, &mut report, &mut || Ok(())).is_err()
        );
        assert!(report.is_empty());
        assert_eq!(fs::read(&canonical).unwrap(), journal);
        assert_eq!(
            fs::read(&pending).unwrap(),
            b"unacknowledged named-route staging"
        );
    }
    fs::write(
        root.0.join("credential.json"),
        selected_profile(&root, &config, 0, 0),
    )
    .unwrap();
    let mut exact = recipe::load(&path, &config, false).unwrap();
    exact.recovery_anchor = Some(anchor.clone());
    let receipt = recovery::readonly::read(&config, &exact, None, &mut || Duration::ZERO).unwrap();
    assert_eq!(receipt.result, result.response.result);
    let mut report = Vec::new();
    audit_command::run(&config, &exact, 1, &anchor, &mut report, &mut || Ok(())).unwrap();
    assert!(!report.is_empty());
    no_route_secret(&report);
    assert_eq!(fs::read(&canonical).unwrap(), journal);
    assert_eq!(
        fs::read(&pending).unwrap(),
        b"unacknowledged named-route staging"
    );
    let (host, _, _, _) = recovery::open_selected(&config, &exact).unwrap();
    assert_eq!(
        host.credential_status().unwrap().unwrap().policy,
        named_policy(&config, 0, 0)
    );
    assert_eq!(host.inspect().executions, paid.executions);
    assert_eq!(host.inspect().payload, paid.payload);
    assert_eq!(
        host.inspect().control.ledger.charged,
        paid.control.ledger.charged
    );
    assert_eq!(
        host.learned_generation_inspection()
            .unwrap()
            .numerical
            .sampled_draws,
        2
    );
    drop(host);
    assert_eq!(reader.snapshot().executions, 1);
    assert_eq!(config.source.status().read_attempts, source_reads);
    assert!(!pending.exists());
    no_route_secret(&fs::read(&canonical).unwrap());
}
