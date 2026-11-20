//! Prompt preparation retains the original publication/credential requirements,
//! without obtaining either live capability. Finite numerical fixtures exercise
//! real native ownership and separate review; they are not calibration evidence.
use super::super::{functional_creation, peer_intake};
use super::*;
use crate::config::CLOCK_DOMAIN;
use fa_reference::action::{
    Scope,
    consequence::delivery::{
        persistent::observed::{
            credential::{FILE_OVERSIGHT_CREDENTIAL_PROFILE, FileCredentialPolicy},
            publication::witnesses::{
                FilePublicationInputs, FileWitnessInput,
                producer::{FilePublicationProducer, PublicationProducerProfile},
            },
            reviewer::wire::ReviewPacket,
        },
        stream::ReleaseFrame,
    },
};
use fa_reference::product_frontier::{
    FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker,
};
use fa_reference::witness::{AdapterDomainInput, DomainClosure, DomainProjection, SnapshotEntry};

const SECRET: &[u8] = b"independently-provisioned-prepared-credential-5837";

fn policy(config: &Config) -> FileCredentialPolicy {
    FileCredentialPolicy {
        family: "publication".into(),
        route: "adapter:file-oversight".into(),
        credential: "broker-token".into(),
        profile_generation: config.profile.delivery.target.contract_version,
    }
}

fn credential_envelope(root: &Root, inner: &str) -> String {
    format!(
        r#"{{"schema":"fa.learned-credential-publication/1","credential":"{}/credential.json","recipe":{inner}}}"#,
        root.0.display()
    )
}

fn publication_envelope(root: &Root, inner: &str) -> String {
    format!(
        r#"{{"schema":"fa.learned-checked-publication/1","publication":"{}/publication.json","recipe":{inner}}}"#,
        root.0.display()
    )
}

fn select_guards(root: &Root, case: &Case, publication: bool, credential: bool) {
    let scope = case.config.profile.delivery.scope;
    let mut recipe = fs::read_to_string(&case.recipe).unwrap();
    if publication {
        fs::write(root.0.join("publication.json"), format!(r#"{{"schema":"fa.supervised-witnesses/3","source":91,"producer":{{"path":"{}/producer/delivery.bin","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}}}},"feed":{{"source":41,"after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}},"limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[{{"kind":"exact_value","key":0,"role":"subject"}},{{"kind":"absent_key","key":1}},{{"kind":"empty_range","start":6,"end":9}},{{"kind":"range_members","start":2,"end":6}}]}}"#,
            root.0.display(), scope.tenant, scope.principal, scope.run, scope.branch, scope.authority)).unwrap();
        recipe = publication_envelope(root, &recipe);
    }
    if credential {
        fs::write(root.0.join("inventory.json"), format!(r#"{{"version":1,"families":[{{"scope":{{"tenant":{},"principal":{},"purpose":1}},"family":"publication","trust_domains":["actor","observation_and_analysis","enforcement","governance_and_investigation"],"credentials":[{{"credential":"broker-token","holder":"broker"}}],"routes":[{{"route":"adapter:file-oversight","effect":"file_write","profile":{{"id":"{}","generation":{}}},"trust_path":["actor","enforcement"],"threat":"direct_credential_or_egress","actor_credential":{{"kind":"broker_mediated"}},"mediation":"brokered_effects","bypass":"blocked","residual_nonclaims":["trusted host and journal sink"]}}],"residual_nonclaims":["not OS containment"]}}]}}"#,
            scope.tenant, scope.principal, FILE_OVERSIGHT_CREDENTIAL_PROFILE,
            case.config.profile.delivery.target.contract_version)).unwrap();
        fs::write(root.0.join("credential.json"), format!(r#"{{"schema":"fa.learned-file-credential/1","policy":{{"family":"publication","route":"adapter:file-oversight","credential":"broker-token","profile_generation":{}}},"epoch":{{"generation":1,"revoked":false}},"inventory":"{}/inventory.json","broker_credential":"{}/broker.bin","provider_credential":"{}/provider.bin"}}"#,
            case.config.profile.delivery.target.contract_version,
            root.0.display(), root.0.display(), root.0.display())).unwrap();
        recipe = credential_envelope(root, &recipe);
    }
    fs::write(&case.recipe, recipe).unwrap();
}

fn structured(scope: Scope, revision: u64, keys: &[u64]) -> FilePublicationInputs {
    let key = ProjectionKey {
        source: 40,
        branch: scope.branch,
        projection: 7,
        source_epoch: 1,
    };
    let close = TrustedClosingMarker {
        key,
        final_sequence: 1,
        marker_generation: 1,
    };
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    frontiers
        .accept(key, FrontierStage::Authenticated, 1)
        .unwrap();
    frontiers.record_close(close).unwrap();
    FilePublicationInputs::new(
        Some(
            FileWitnessInput::new(
                revision,
                revision,
                20,
                AdapterDomainInput::new(
                    DomainProjection::new(40, 1, key),
                    DomainClosure::Closed(close),
                ),
                keys.iter()
                    .map(|&key| SnapshotEntry::new(key, 1, b"original".to_vec()).unwrap())
                    .collect(),
                &frontiers,
            )
            .unwrap(),
        ),
        None,
    )
}

fn producer(root: &Root, case: &Case) -> FilePublicationProducer {
    let profile = PublicationProducerProfile {
        source: 91,
        scope: case.config.profile.delivery.scope,
        feed: 41,
        clock_domain: CLOCK_DOMAIN,
        after: 0,
    };
    FilePublicationProducer::create(
        root.0.join("producer"),
        profile,
        structured(profile.scope, 1, &[0, 2, 4]),
        ElapsedTick(1000),
    )
    .unwrap()
    .0
}

fn secrets(root: &Root) {
    // Independent provisioning inputs; neither expectation is read from the other.
    fs::write(root.0.join("broker.bin"), SECRET).unwrap();
    fs::write(root.0.join("provider.bin"), SECRET).unwrap();
}

fn no_secret(bytes: &[u8]) {
    assert!(!bytes.windows(SECRET.len()).any(|part| part == SECRET));
}

fn original_packet(packet: &ReviewPacket) {
    assert_eq!(packet.views().len(), 2);
    assert_eq!(
        ReleaseFrame::decode(&packet.action().spec().payload)
            .unwrap()
            .message(),
        Some("A")
    );
    for view in packet.views().values() {
        no_secret(view.actual_input().submitted_bytes());
    }
}

fn capture_request(prepared: &Json) -> stream_checkpoint::Request {
    stream_checkpoint::Request::Capture(
        FileLearnedStreamCheckpointRequest::new(
            number(prepared, "revision") - 1,
            FileLearnedCheckpointInfo {
                checkpoint: 71,
                actor_revision: number(prepared, "actor_revision"),
                authority_epoch: number(prepared, "authority_epoch"),
                control_sequence: number(prepared, "control_sequence"),
                stream: number(prepared, "stream"),
                position: 2,
                sampled_draws: 0,
            },
        )
        .unwrap(),
    )
}

fn verify_prepared(case: &Case, prepared: &Json) {
    assert_eq!(number(prepared, "position"), 2);
    assert_eq!(number(prepared, "sampled_draws"), 0);
    assert_eq!(number(prepared, "sampling_attempts"), 0);
    assert_eq!(number(prepared, "checkpoints"), 1);
    assert_eq!(number(prepared, "restart_attempts"), 0);
    let view = case.view();
    quiet(&view.publication);
    assert_eq!(view.consistency.consistency.evidence.samples(), 0);
    assert_eq!(view.consistency.work, LearnedMonitorWork::default());
    let before = case.disk();
    no_secret(&before);
    let mut loaded = case.reader();
    loaded.recovery_anchor = Some(custody::load(&case.anchor).unwrap());
    loaded.anchor_output = Some(case.anchor.with_file_name("guarded-capture.anchor"));
    let recovered = recovery::stream_checkpoint::execute(
        &case.config,
        &loaded,
        &capture_request(prepared),
        &mut |_| Ok(()),
    )
    .unwrap();
    assert_eq!(recovered.appended_events(), 0);
    assert_eq!(recovered.numerical().position, 2);
    assert_eq!(recovered.numerical().sampled_draws, 0);
    assert_eq!(recovered.numerical().cumulative_work.sampling_attempts, 0);
    assert_eq!(recovered.numerical().cumulative_work.admitted_tokens, 2);
    assert_eq!(case.disk(), before);
}

fn continue_prepared(
    root: &Root,
    case: &Case,
    decision: ReviewDecision,
    clock: &mut impl FnMut(bool) -> ElapsedTick,
) -> (Result<(), String>, Option<ReviewPacket>, PathBuf) {
    let (peer_path, peers) = functional_creation::peers_file(root, &case.config);
    let anchor = root.0.join("continued.anchor");
    let args = vec![
        "create-learned-generated".into(),
        case.config_path.display().to_string(),
        case.recipe.display().to_string(),
        peer_path.display().to_string(),
        "--continue-generation".into(),
        "--history-anchor".into(),
        case.anchor.display().to_string(),
        "--retain-history-anchor".into(),
        anchor.display().to_string(),
    ];
    let mut human = None;
    let result = super::super::super::command_with_clock(&args, None, || {
        if human.is_none() && peers.socket(1).exists() {
            human = Some(peer_intake::human(peers.clone(), 1, decision));
        }
        clock(human.is_some())
    });
    let packet = human.map(|human| human.join().unwrap());
    (result, packet, anchor)
}

#[test]
fn guarded_prompt_preparation_resumes_original_numerical_state_and_requires_both_review_keys() {
    for (publication, credential, decision) in [
        (true, false, ReviewDecision::Approve),
        (false, true, ReviewDecision::Approve),
        (true, true, ReviewDecision::Approve),
        (true, true, ReviewDecision::Reject),
    ] {
        let root = Root::new();
        let case = Case::new(&root, false);
        select_guards(&root, &case, publication, credential);
        let helper = root.0.join("helper-weights.safetensors");
        let helper_weights = fs::read(&helper).unwrap();
        fs::remove_file(&helper).unwrap();
        assert!(!root.0.join("producer").exists());
        assert!(!root.0.join("broker.bin").exists());
        assert!(!root.0.join("provider.bin").exists());
        let prepared = case.prepare();
        verify_prepared(&case, &prepared);
        let loaded = case.reader();
        assert_eq!(
            loaded
                .generation
                .required_publication_validation()
                .is_some(),
            publication
        );
        assert_eq!(
            loaded.generation.required_credential_policy().is_some(),
            credential
        );
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
        fs::write(helper, helper_weights).unwrap();
        let _producer = publication.then(|| producer(&root, &case));
        if credential {
            secrets(&root);
        }
        let (result, packet, anchor) =
            continue_prepared(&root, &case, decision, &mut |_| ElapsedTick(1001));
        let packet = packet.expect("prepared guarded prompt still needs independent human review");
        original_packet(&packet);
        assert_eq!(
            result.is_ok(),
            decision == ReviewDecision::Approve,
            "{result:?}"
        );
        let view = case.view();
        assert_eq!(
            view.publication.executions,
            u64::from(decision == ReviewDecision::Approve)
        );
        assert_eq!(
            view.publication.payload,
            if decision == ReviewDecision::Approve {
                b"A".to_vec()
            } else {
                Vec::new()
            }
        );
        assert_eq!(
            view.publication.control.ledger.charged,
            if decision == ReviewDecision::Approve {
                packet.action().spec().units
            } else {
                0
            }
        );
        assert_eq!(view.publication.control.ledger.reserved, 0);
        assert_eq!(view.consistency.consistency.evidence.samples(), 1);
        assert!(view.consistency.work.probe_coordinates > 0);
        no_secret(&case.disk());
        if decision == ReviewDecision::Approve {
            let mut loaded = case.reader();
            loaded.recovery_anchor = Some(custody::load(&anchor).unwrap());
            let (host, _, _, _) = recovery::open_selected(&case.config, &loaded).unwrap();
            let numerical = host.learned_generation_inspection().unwrap().numerical;
            let actor = host.actor_snapshot().unwrap();
            assert_eq!(numerical.position, 4);
            assert_eq!(numerical.sampled_draws, 2);
            assert_eq!(numerical.cumulative_work.sampling_attempts, 2);
            assert_eq!(numerical.cumulative_work.admitted_tokens, 4);
            assert_eq!(actor.state.next_position(), numerical.position);
            assert_eq!(actor.state.cache().len(), numerical.cache_bytes);
            assert_eq!(actor.state.sampler().len(), numerical.sampler_bytes);
            assert_eq!(host.learned_recovery_usage().unwrap().checkpoints, 1);
            assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
            assert_eq!(
                host.learned_stream_checkpoint(71)
                    .unwrap()
                    .info()
                    .authority_epoch,
                0
            );
            if credential {
                let status = host.credential_status().unwrap().unwrap();
                assert_eq!(status.policy, policy(&case.config));
                assert_eq!(status.generation, 1);
                assert!(!status.revoked);
                assert_eq!(status.retained_changes, 0);
            }
        }
    }
}

#[test]
fn prepared_guards_do_not_replace_live_credentials_or_consume_custody_on_failed_preflight() {
    let root = Root::new();
    let case = Case::new(&root, false);
    select_guards(&root, &case, true, true);
    let prepared = case.prepare();
    verify_prepared(&case, &prepared);
    let _producer = producer(&root, &case);
    let original = case.disk();
    let anchor = fs::read(&case.anchor).unwrap();
    for missing in [true, false] {
        if !missing {
            fs::write(root.0.join("broker.bin"), SECRET).unwrap();
            fs::write(
                root.0.join("provider.bin"),
                b"independently-different-credential",
            )
            .unwrap();
        }
        let (result, packet, output) =
            continue_prepared(&root, &case, ReviewDecision::Approve, &mut |_| {
                ElapsedTick(1001)
            });
        assert!(result.is_err());
        assert!(packet.is_none());
        assert!(!output.exists());
        assert_eq!(case.disk(), original);
        assert_eq!(fs::read(&case.anchor).unwrap(), anchor);
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
        quiet(&case.view().publication);
    }
    secrets(&root);
    let (result, packet, _) = continue_prepared(&root, &case, ReviewDecision::Approve, &mut |_| {
        ElapsedTick(1001)
    });
    result.unwrap();
    original_packet(&packet.unwrap());
    assert_eq!(case.view().publication.executions, 1);
    no_secret(&case.disk());
}

#[test]
fn prepared_checked_publication_still_rechecks_changed_or_absent_final_witnesses() {
    for mutation in 0..3 {
        let root = Root::new();
        let case = Case::new(&root, false);
        select_guards(&root, &case, true, true);
        case.prepare();
        secrets(&root);
        let mut producer = producer(&root, &case);
        let mut dispatched = None;
        let (result, packet, _) =
            continue_prepared(&root, &case, ReviewDecision::Approve, &mut |reviewed| {
                if reviewed && dispatched.is_none() {
                    let view = case.view();
                    if view.publication.control.ledger.charged != 0
                        && view.publication.executions == 0
                    {
                        if mutation == 2 {
                            fs::rename(
                                root.0.join("producer/delivery.bin"),
                                root.0.join("producer/unavailable-delivery.bin"),
                            )
                            .unwrap();
                        } else {
                            let keys: &[u64] = if mutation == 1 {
                                &[0, 1, 2, 4]
                            } else {
                                &[0, 2, 4, 99]
                            };
                            producer
                                .publish(
                                    1,
                                    structured(case.config.profile.delivery.scope, 2, keys),
                                    ElapsedTick(1001),
                                )
                                .unwrap();
                        }
                        dispatched = Some(view.publication);
                    }
                }
                ElapsedTick(1001)
            });
        let dispatched =
            dispatched.expect("mutate only after real charged dispatch and both reviews");
        let packet = packet.unwrap();
        original_packet(&packet);
        assert_eq!(
            dispatched.control.ledger.charged,
            packet.action().spec().units
        );
        assert_eq!(
            result.is_ok(),
            mutation == 0,
            "mutation={mutation}: {result:?}"
        );
        let view = case.view();
        assert_eq!(view.publication.executions, u64::from(mutation == 0));
        assert_eq!(
            view.publication.payload,
            if mutation == 0 {
                b"A".to_vec()
            } else {
                Vec::new()
            }
        );
        assert_eq!(
            view.publication.control.ledger.charged,
            if mutation == 0 {
                packet.action().spec().units
            } else {
                0
            }
        );
        assert_eq!(view.publication.control.ledger.reserved, 0);
        no_secret(&case.disk());
    }
}

#[test]
fn guarded_preparation_rejects_missing_or_changed_inventory_and_invalid_profiles_before_creation() {
    for variant in 0..6 {
        let root = Root::new();
        let case = Case::new(&root, false);
        select_guards(&root, &case, true, true);
        let inventory = fs::read_to_string(root.0.join("inventory.json")).unwrap();
        let credential = fs::read_to_string(root.0.join("credential.json")).unwrap();
        let publication = fs::read_to_string(root.0.join("publication.json")).unwrap();
        match variant {
            0 => fs::remove_file(root.0.join("inventory.json")).unwrap(),
            1 => fs::write(
                root.0.join("inventory.json"),
                inventory.replace("\"holder\":\"broker\"", "\"holder\":\"actor\""),
            )
            .unwrap(),
            2 => fs::write(
                root.0.join("credential.json"),
                credential.replace("\"generation\":1", "\"generation\":2"),
            )
            .unwrap(),
            3 => fs::write(
                root.0.join("credential.json"),
                credential.replace("\"revoked\":false", "\"revoked\":true"),
            )
            .unwrap(),
            4 => fs::write(
                root.0.join("publication.json"),
                publication.replace("\"max_age_ms\":5000", "\"max_age_ms\":0"),
            )
            .unwrap(),
            _ => fs::remove_file(root.0.join("publication.json")).unwrap(),
        }
        let mut output = Vec::new();
        assert!(
            prepare::command(
                &case.args(),
                None,
                || panic!("invalid guarded preparation read the clock: {variant}"),
                &mut output
            )
            .is_err()
        );
        assert!(output.is_empty());
        assert!(!case.config.store.exists());
        assert!(!case.anchor.exists());
        fs::write(root.0.join("inventory.json"), inventory).unwrap();
        fs::write(root.0.join("credential.json"), credential).unwrap();
        fs::write(root.0.join("publication.json"), publication).unwrap();
        let prepared = case.prepare();
        verify_prepared(&case, &prepared);
    }
}

#[test]
fn preparation_credential_inventory_and_publication_profile_share_the_original_asset_allowance() {
    let root = Root::new();
    let case = Case::new(&root, false);
    select_guards(&root, &case, true, true);
    let names = [
        "credential.json",
        "inventory.json",
        "publication.json",
        "fit-binding.json",
        "learned-monitor.json",
        "forecast.json",
        "model.json",
        "tokenizer.json",
        "sampling.json",
        "prompt.txt",
        "weights.safetensors",
        "fit.fakv",
    ];
    let total: u64 = names
        .into_iter()
        .map(|name| fs::metadata(root.0.join(name)).unwrap().len())
        .sum();
    let original = fs::read_to_string(&case.recipe).unwrap();
    fs::remove_file(root.0.join("helper-weights.safetensors")).unwrap();
    fs::write(
        &case.recipe,
        original.replace(
            "\"asset_bytes\":4194304",
            &format!("\"asset_bytes\":{}", total - 1),
        ),
    )
    .unwrap();
    assert!(
        prepare::command(
            &case.args(),
            None,
            || panic!("exhausted original asset allowance read time"),
            &mut Vec::new()
        )
        .is_err()
    );
    assert!(!case.config.store.exists());
    assert!(!case.anchor.exists());
    fs::write(
        &case.recipe,
        original.replace(
            "\"asset_bytes\":4194304",
            &format!("\"asset_bytes\":{total}"),
        ),
    )
    .unwrap();
    let loaded = recipe::load_for_preparation(&case.recipe, &case.config).unwrap();
    assert!(loaded.native.is_none());
    let prepared = case.prepare();
    verify_prepared(&case, &prepared);
    // Historical replay keeps its existing asset scope and needs no live inventory.
    fs::remove_file(root.0.join("inventory.json")).unwrap();
    verify_prepared(&case, &prepared);
}

#[test]
fn prepared_guarded_history_rejects_omitted_or_changed_original_requirements() {
    let root = Root::new();
    let case = Case::new(&root, false);
    let bare = fs::read_to_string(&case.recipe).unwrap();
    select_guards(&root, &case, true, true);
    let guarded = fs::read_to_string(&case.recipe).unwrap();
    let publication = fs::read_to_string(root.0.join("publication.json")).unwrap();
    let credential = fs::read_to_string(root.0.join("credential.json")).unwrap();
    let prepared = case.prepare();
    let original = case.disk();
    let anchor = fs::read(&case.anchor).unwrap();
    for variant in 0..6 {
        match variant {
            0 => fs::write(&case.recipe, &bare).unwrap(),
            1 => fs::write(&case.recipe, publication_envelope(&root, &bare)).unwrap(),
            2 => fs::write(&case.recipe, credential_envelope(&root, &bare)).unwrap(),
            3 => fs::write(
                root.0.join("publication.json"),
                publication.replace("\"max_age_ms\":5000", "\"max_age_ms\":4999"),
            )
            .unwrap(),
            4 => fs::write(
                root.0.join("credential.json"),
                credential.replace("\"generation\":1", "\"generation\":2"),
            )
            .unwrap(),
            _ => fs::write(
                root.0.join("credential.json"),
                credential.replace("broker-token", "other-token"),
            )
            .unwrap(),
        }
        let mut loaded = case.reader();
        loaded.recovery_anchor = Some(custody::load(&case.anchor).unwrap());
        loaded.anchor_output = Some(case.anchor.with_file_name("guarded-capture.anchor"));
        assert!(
            recovery::stream_checkpoint::execute(
                &case.config,
                &loaded,
                &capture_request(&prepared),
                &mut |_| Ok(())
            )
            .is_err(),
            "variant={variant}"
        );
        assert_eq!(case.disk(), original);
        assert_eq!(fs::read(&case.anchor).unwrap(), anchor);
        fs::write(&case.recipe, &guarded).unwrap();
        fs::write(root.0.join("publication.json"), &publication).unwrap();
        fs::write(root.0.join("credential.json"), &credential).unwrap();
    }
    verify_prepared(&case, &prepared);
}
