//! Original learned text, sidecar review, two keys and credential-guarded sink.
//! Tiny weights and scripted ballots are causal controls, not detector quality
//! or a separate external endpoint or hostile-process perimeter qualification.
#![cfg(unix)]
#![forbid(unsafe_code)]
#[allow(dead_code)]
#[path = "support/learned_text_model.rs"] mod fixture;
#[path = "support/file_mediation.rs"] mod topology;
#[path = "learned_credential_policy/support.rs"] mod support;
use support::*;

#[test]
fn selection_pins_the_complete_resolved_policy_without_activating_a_guard() {
    let original = recipe(); let selected = policy();
    assert_eq!(original.required_credential_policy(), None);
    let config = original.clone().with_required_credential_policy(selected.clone()).unwrap();
    assert_eq!(config.required_credential_policy(), Some(&selected));
    assert_ne!(config, original); assert!(config.encoded_bytes() > original.encoded_bytes());
    assert_eq!(config.clone().with_required_credential_policy(selected.clone()).err(), Some(Error::Duplicate));
    for changed in changed_policies() {
        assert_ne!(config, recipe().with_required_credential_policy(changed).unwrap());
    }
    for field in 0..4 {
        let mut invalid = selected.clone();
        match field { 0 => invalid.family.clear(), 1 => invalid.route.clear(),
            2 => invalid.credential.clear(), _ => invalid.profile_generation = 0 }
        assert_eq!(recipe().with_required_credential_policy(invalid).err(), Some(Error::InvalidInput));
    }
    let mut oversized = selected.clone();
    oversized.credential = "x".repeat(fa_reference::perimeter::MAX_TEXT_BYTES + 1);
    assert_eq!(recipe().with_required_credential_policy(oversized).err(), Some(Error::Limit));
    let continuation = config.with_text_stream_continuation().unwrap();
    assert_eq!(continuation.required_credential_policy(), Some(&selected));
    assert!(continuation.requires_sidecar()); assert_eq!(continuation.text_stream_profile(), Some(stream()));
    let reverse = original.with_text_stream_continuation().unwrap()
        .with_required_credential_policy(selected.clone()).unwrap();
    assert_ne!(continuation, reverse, "complete wrapper order remains independently selected identity");

    let mut target = profile().delivery.target; target.contract_version += 1;
    assert_eq!(FileCredentialPolicy::from_inventory(&inventory(), &binding(), profile().delivery.scope, target),
        Err(Error::Binding));
    let root = Directory::new();
    let (host, _) = FileOversight::create_with_learned_text_stream(root.store(), profile(), recipe()).unwrap();
    assert_eq!(host.credential_policy(), None, "resolving and selecting data creates no live guard");
}

#[test]
fn genuinely_registered_guard_precedes_first_learned_image_in_all_native_lanes() {
    for mediated in [false, true] { for lane in 0..3 {
        let root = Directory::new(); let config = lane_recipe(lane);
        let guards = guards(&config); let inventory = inventory(); let binding = binding();
        let registration = Some(FileCredentialRegistration { inventory: &inventory, binding: &binding });
        let host = if mediated {
            let graph = topology::graph(profile().delivery.scope, profile().delivery.target, 1, false);
            let (host, roles) = if lane == 0 {
                FileOversight::create_mediated_guarded_with_learned_generation_reserved(root.store(),
                    profile(), &guards, registration, graph, None, config.clone(), RecoveryReserve::terminal()).unwrap()
            } else {
                let prediction = if lane == 1 {
                    FileMediatedPredictor::Raw(config.required_pre_output_forecast().unwrap().clone())
                } else { FileMediatedPredictor::Owned(config.required_owned_pre_output_forecast().unwrap().clone()) };
                FileOversight::create_predictive_mediated_guarded_with_learned_generation_reserved(root.store(),
                    profile(), &guards, registration, graph, prediction, None, config.clone(), RecoveryReserve::terminal()).unwrap()
            };
            assert_eq!(roles.consistency_observer.is_some(), lane != 0); host
        } else if lane == 0 {
            FileOversight::create_guarded_with_learned_generation_reserved(root.store(), profile(),
                &guards, registration, config.clone(), RecoveryReserve::terminal()).unwrap().0
        } else if lane == 1 {
            FileOversight::create_predictive_guarded_with_learned_generation_reserved(root.store(), profile(),
                &guards, registration, config.required_pre_output_forecast().unwrap().clone(), None,
                config.clone(), RecoveryReserve::terminal()).unwrap().0
        } else {
            FileOversight::create_predictive_guarded_with_owned_learned_consistency_reserved(root.store(), profile(),
                &guards, registration, config.required_owned_pre_output_forecast().unwrap().clone(), None,
                config.clone(), RecoveryReserve::terminal()).unwrap().0
        };
        assert_eq!(host.credential_policy(), config.required_credential_policy());
        let status = host.credential_status().unwrap().unwrap();
        assert_eq!(status.generation, 1); assert!(!status.revoked);
        assert!(host.publication_guard_required()); assert_eq!(host.inspect().executions, 0);
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, 0); assert_eq!(numerical.sampled_draws, 0);
        assert_eq!(host.journal_capacity().unwrap().reserve(), Some(RecoveryReserve::terminal()));
    } }
}

#[test]
fn asserted_missing_or_different_credential_policy_cannot_create_an_owner() {
    let config = selected(); let registered = guards(&config);
    let inventory = inventory(); let binding = binding();
    let root = Directory::new();
    assert_eq!(FileOversight::create_with_learned_text_stream(root.store(), profile(), config.clone()).err(),
        Some(JournalError::Contract(Error::Binding)));
    assert!(!root.store().exists());
    assert_eq!(FileOversight::create_guarded_with_learned_generation_reserved(root.store(), profile(),
        &registered, None, config.clone(), RecoveryReserve::terminal()).err(),
        Some(JournalError::Contract(Error::InvalidInput)));
    assert!(!root.store().exists());
    let mut absent = registered.clone(); absent.credential = None;
    assert_eq!(FileOversight::create_guarded_with_learned_generation_reserved(root.store(), profile(),
        &absent, None, config, RecoveryReserve::terminal()).err(), Some(JournalError::Contract(Error::Binding)));
    assert!(!root.store().exists());
    for changed in changed_policies() {
        let wrong = recipe().with_required_credential_policy(changed).unwrap();
        assert_eq!(FileOversight::create_guarded_with_learned_generation_reserved(root.store(), profile(),
            &registered, Some(FileCredentialRegistration { inventory: &inventory, binding: &binding }),
            wrong, RecoveryReserve::terminal()).err(), Some(JournalError::Contract(Error::Binding)));
        assert!(!root.store().exists(), "mismatched learned assertion left a partial canonical owner");
    }
    let mut falsely_registered = registered;
    falsely_registered.credential.as_mut().unwrap().credential.push_str("-different");
    let wrong = recipe().with_required_credential_policy(falsely_registered.credential.clone().unwrap()).unwrap();
    assert_eq!(FileOversight::create_guarded_with_learned_generation_reserved(root.store(), profile(),
        &falsely_registered, Some(FileCredentialRegistration { inventory: &inventory, binding: &binding }),
        wrong, RecoveryReserve::terminal()).err(), Some(JournalError::Contract(Error::Binding)));
    assert!(!root.store().exists(), "matching assertions cannot replace original perimeter registration");
}

#[test]
fn actual_learned_text_requires_both_live_credentials_after_original_review_and_two_keys() {
    let root = Directory::new(); let (mut host, human) = create(&root);
    let (action, input) = dispatched(&mut host, &human);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let revision = host.revision(); let files = root.files();
    assert_eq!(host.publish_checked(revision, 1, Some(&input), snapshot(), ElapsedTick(2)).err(),
        Some(JournalError::Contract(Error::Incomplete)));
    assert_eq!(bind(&host, b"broker-only", b"different-provider").err(), Some(JournalError::Contract(Error::Binding)));
    assert_eq!(host.revision(), revision); assert_eq!(root.files(), files);
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.charged, action.spec().units);
    let secret = b"independent-live-credential-never-persisted";
    let permit = bind(&host, secret, secret).unwrap();
    assert_eq!(host.revision(), revision, "matching live credentials do not add a journal assertion");
    let published = host.publish_checked_with_credential(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(2), &permit).unwrap();
    assert_eq!(published.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(published.basis, PublicationBasis::Revalidated);
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(published.outcome));
    assert_eq!(host.stream_snapshot().unwrap().confirmed.visible(), b"OK");
    let revision = host.revision();
    let repeated = host.publish_checked(revision, 1, None, Snapshot::default(), ElapsedTick(3)).unwrap();
    assert_eq!(repeated.outcome, published.outcome); assert_eq!(repeated.basis, PublicationBasis::PreviouslyResolved);
    assert_eq!(host.revision(), revision + 1);
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().control.ledger.charged, action.spec().units);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    for bytes in root.files().values() {
        assert!(!bytes.windows(secret.len()).any(|window| window == secret));
    }
}

#[test]
fn recipe_policy_is_required_by_source_free_receipts_audits_and_guarded_recovery() {
    let root = Directory::new(); let config = selected(); let (mut host, human) = create(&root);
    let (_, input) = dispatched(&mut host, &human); let old = bind(&host, b"first-secret", b"first-secret").unwrap();
    host.publish_checked_with_credential(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(2), &old).unwrap();
    host.reconcile(host.revision(), 1).unwrap();
    let stream = host.stream_snapshot().unwrap(); let replay = host.review_replay(101).unwrap();
    let anchor = host.history_anchor().unwrap(); let expected = requirements(&host, &config);
    let numerical = host.learned_generation_inspection().unwrap().numerical; drop(host);
    std::fs::write(root.store().join("delivery.pending"), b"retain unacknowledged candidate").unwrap();
    let before = root.files();
    let mut variants = vec![recipe()];
    variants.extend(changed_policies().into_iter().map(|policy| recipe().with_required_credential_policy(policy).unwrap()));
    for wrong in variants {
        assert_eq!(FileOversight::read_stream_publication_with_learned_generation(root.store(), &profile(), &wrong).err(),
            Some(JournalError::Contract(Error::Binding)));
        assert_eq!(FileOversight::read_review_replay_with_learned_generation(root.store(), &profile(), &wrong, 101).err(),
            Some(JournalError::Contract(Error::Binding)));
        assert_eq!(FileOversight::begin_read_review_replay_anchored_with_learned_generation(
            root.store(), &profile(), &wrong, 101, &anchor).err(), Some(JournalError::Contract(Error::Binding)));
        assert_eq!(FileOversight::open_guarded_with_learned_generation(root.store(), profile(), &expected, &wrong).err(),
            Some(JournalError::Contract(Error::Binding)));
        assert_eq!(root.files(), before, "wrong independent recipe must precede cleanup and recovery writes");
    }
    assert_eq!(FileOversight::read_stream_publication_with_learned_generation(root.store(), &profile(), &config).unwrap(), stream);
    assert_eq!(FileOversight::read_review_replay_with_learned_generation(root.store(), &profile(), &config, 101).unwrap(), replay);
    let mut audit = FileOversight::begin_read_review_replay_anchored_with_learned_generation(
        root.store(), &profile(), &config, 101, &anchor).unwrap();
    while audit.progress().status == FileReviewReplayStatus::Replaying {
        audit.advance(audit.progress().replayed_events, 1).unwrap();
    }
    assert_eq!(audit.finish().unwrap(), replay); assert_eq!(root.files(), before);
    let (mut host, _) = FileOversight::open_guarded_with_learned_generation(root.store(), profile(), &expected, &config).unwrap();
    assert!(!root.store().join("delivery.pending").exists());
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert!(host.inspect().control.ledger.epoch > expected.minimum.authority_epoch);
    let revision = host.revision();
    assert_eq!(host.publish_checked_with_credential(revision, 999, None, Snapshot::default(), ElapsedTick(3), &old).err(),
        Some(JournalError::Contract(Error::Binding)));
    assert_eq!(host.revision(), revision);
    assert_eq!(host.publish_checked(revision, 1, None, Snapshot::default(), ElapsedTick(3)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.inspect().executions, 1);
}

#[test]
fn rotation_preserves_the_same_learned_action_but_requires_a_new_live_pair() {
    let root = Directory::new(); let config = selected(); let (mut host, human) = create(&root);
    let (action, input) = dispatched(&mut host, &human);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let old = bind(&host, b"first", b"first").unwrap();
    let request = CredentialRotationRequest { operation: 41, expected_generation: 1, next_generation: 2 };
    let receipt = host.rotate_credential_guard(host.revision(), request).unwrap(); assert_eq!(receipt.generation, 2);
    let revision = host.revision();
    assert_eq!(host.publish_checked_with_credential(revision, 1, Some(&input), snapshot(), ElapsedTick(2), &old).err(),
        Some(JournalError::Contract(Error::Binding)));
    assert_eq!(host.revision(), revision); assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.charged, action.spec().units);
    let current = bind(&host, b"second", b"second").unwrap();
    assert_eq!(current.generation(), 2); assert_eq!(current.policy(), config.required_credential_policy().unwrap());
    assert_eq!(host.publish_checked_with_credential(revision, 1, Some(&input), snapshot(), ElapsedTick(2), &current).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.stream_snapshot().unwrap().confirmed.visible(), b"OK");
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert_eq!(host.inspect().executions, 1);
    let expected = requirements(&host, &config); drop(host);
    let before = root.files(); let mut stale = expected.clone(); stale.credential_epoch.as_mut().unwrap().generation = 1;
    assert_eq!(FileOversight::open_guarded_with_learned_generation(root.store(), profile(), &stale, &config).err(),
        Some(JournalError::Contract(Error::Binding)));
    assert_eq!(root.files(), before);
    let (host, _) = FileOversight::open_guarded_with_learned_generation(root.store(), profile(), &expected, &config).unwrap();
    assert_eq!(host.credential_status().unwrap().unwrap().generation, 2);
    assert_eq!(host.credential_policy(), config.required_credential_policy());
    assert_eq!(host.inspect().control.ledger.charged, action.spec().units);
}

#[test]
fn revoked_learned_send_stays_charged_until_original_restrictive_receipt() {
    let root = Directory::new(); let config = selected(); let (mut host, human) = create(&root);
    let (action, input) = dispatched(&mut host, &human); let old = bind(&host, b"first", b"first").unwrap();
    host.revoke_credential_guard(host.revision(), CredentialRevocationRequest { operation: 77, expected_generation: 1 }).unwrap();
    let revision = host.revision();
    assert_eq!(host.publish_checked_with_credential(revision, 1, Some(&input), snapshot(), ElapsedTick(2), &old).err(),
        Some(JournalError::Contract(Error::WrongState)));
    assert_eq!(bind(&host, b"new", b"new").err(), Some(JournalError::Contract(Error::WrongState)));
    assert_eq!(host.revision(), revision); assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.charged, action.spec().units);
    let expected = requirements(&host, &config); drop(host);
    std::fs::write(root.store().join("delivery.pending"), b"retain candidate").unwrap(); let before = root.files();
    let mut active = expected.clone(); active.credential_epoch.as_mut().unwrap().revoked = false;
    let mut absent = expected.clone(); absent.credential_epoch = None;
    for (wrong, error) in [(&active, Error::Binding), (&absent, Error::InvalidInput)] {
        assert_eq!(FileOversight::open_guarded_with_learned_generation(root.store(), profile(), wrong, &config).err(),
            Some(JournalError::Contract(error)));
        assert_eq!(root.files(), before);
    }
    let (mut host, _) = FileOversight::open_guarded_with_learned_generation(root.store(), profile(), &expected, &config).unwrap();
    assert!(host.credential_status().unwrap().unwrap().revoked);
    assert_eq!(host.inspect().control.ledger.charged, action.spec().units, "recovery is not nonexecution evidence");
    assert_eq!(host.inspect().executions, 0); assert!(host.stream_snapshot().unwrap().confirmed.visible().is_empty());
    host.observe_time(host.revision(), ElapsedTick(3)).unwrap();
    assert_eq!(host.seal_unexecuted(host.revision(), 1).unwrap(),
        Reconciliation::Resolved(EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed }));
    assert_eq!(host.inspect().control.ledger.charged, 0);
    assert_eq!(host.inspect().control.ledger.available, profile().delivery.total);
}
