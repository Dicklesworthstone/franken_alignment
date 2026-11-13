//! Original checked publication survives a separately approved policy change.
//! Native committee ballots and source documents are explicit fixture inputs.
#![cfg(unix)]
#[path = "support/file_oversight.rs"]
mod fixture;

use fa_reference::Error;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::governance::PolicyUpdate;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileCampaignRequirement, FileGuardSet, FileOversightRoles,
};
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::{
    FilePublicationEvidence, FilePublicationInputs, FileWitnessInput,
};
use fa_reference::action::consequence::delivery::persistent::observed::source::FileSourcePolicy;
use fa_reference::action::consequence::delivery::persistent::{
    JournalError, Reconciliation, RecoveryReserve,
};
use fa_reference::action::consequence::delivery::publication_gate::{
    MAX_PUBLICATION_BINDINGS, PublicationLimits,
};
use fa_reference::action::consequence::delivery::stream::StreamProfile;
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::gate::containment::session::policy::Policy;
use fa_reference::action::consequence::oversight::CommitteeInput;
use fa_reference::action::consequence::oversight::evidence_source::{
    EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES,
};
use fa_reference::action::consequence::oversight::policy_state::{
    StateFreshness, StateLimits, StateSource,
};
use fa_reference::action::consequence::policy_campaign::ReplayLimits;
use fa_reference::product_frontier::{
    FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker,
};
use fa_reference::witness::refinement::RefinementBudget;
use fa_reference::witness::{
    AdapterDomainInput, DomainClosure, DomainProjection, QueryRole, SnapshotEntry, WitnessRequest,
};
use fixture::{Directory, Keys, profile, snapshot};

fn limits() -> PublicationLimits {
    PublicationLimits {
        bindings: 8,
        validation: RefinementBudget {
            steps: 10_000,
            value_bytes: 1_048_576,
        },
    }
}

fn guards() -> FileGuardSet {
    FileGuardSet {
        stream: None,
        decoder: None,
        decoder_stop: None,
        source: Some(FileSourcePolicy {
            source: StateSource {
                scope: profile().delivery.scope,
                source: 17,
                generation: 1,
            },
            limits: StateLimits::default(),
            freshness: StateFreshness::new(50).unwrap(),
        }),
        identity: None,
        campaigns: Some(FileCampaignRequirement {
            limits: ReplayLimits {
                cases: 32,
                input_bytes: 1_048_576,
            },
            max_campaigns: 8,
        }),
        credential: None,
    }
}

fn source(root: &Directory) -> FileEvidenceSource {
    let observation = EvidenceSnapshot::new(
        EvidenceIdentity {
            source: 17,
            generation: 1,
            scope: profile().delivery.scope,
        },
        snapshot(),
        fixture::MEMBERS
            .into_iter()
            .map(|member| (member.into(), b"complete source view".to_vec()))
            .collect(),
    )
    .unwrap();
    let path = root.0.join("evidence.json");
    std::fs::write(&path, observation.encode()).unwrap();
    FileEvidenceSource::new(path, 17, profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap()
}

fn observations(inputs: &CommitteeInput, revision: u64, keys: &[u64]) -> FilePublicationInputs {
    let key = ProjectionKey {
        source: 40,
        branch: 4,
        projection: 7,
        source_epoch: 1,
    };
    let marker = TrustedClosingMarker {
        key,
        final_sequence: 1,
        marker_generation: 1,
    };
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    frontiers
        .accept(key, FrontierStage::Authenticated, 1)
        .unwrap();
    frontiers.record_close(marker).unwrap();
    let structured = FileWitnessInput::new(
        revision,
        revision,
        20,
        AdapterDomainInput::new(
            DomainProjection::new(40, 1, key),
            DomainClosure::Closed(marker),
        ),
        keys.iter()
            .map(|key| SnapshotEntry::new(*key, 1, b"original".to_vec()).unwrap())
            .collect(),
        &frontiers,
    )
    .unwrap();
    FilePublicationInputs::new(
        Some(structured),
        Some(inputs.views()["alpha"].actual_input().clone()),
    )
}

fn ready(
    host: &mut FileOversight,
    roles: &FileOversightRoles,
    capture: &EvidenceSnapshot,
    id: u64,
    payload: &[u8],
) -> Keys {
    let action = host
        .propose(
            host.revision(),
            id,
            fixture::spec(host, payload),
            capture.snapshot().clone(),
        )
        .unwrap();
    let inputs = capture.inputs_for(&action, &profile().committee).unwrap();
    fixture::review_existing(host, id, id + 100, &inputs);
    let original = observations(&inputs, 10, &[0, 2, 4]);
    let evidence = FilePublicationEvidence::new(
        original.clone(),
        vec![
            WitnessRequest::ExactValue {
                key: 0,
                role: QueryRole::Subject,
            },
            WitnessRequest::AbsentKey { key: 1 },
            WitnessRequest::RangeMembers { start: 2, end: 6 },
        ],
    )
    .unwrap();
    host.bind_publication_evidence(host.revision(), id, evidence)
        .unwrap();
    host.record_publication_inputs(host.revision(), id, 0, Some(original))
        .unwrap();
    let automatic = host
        .authorize(host.revision(), id, &inputs, snapshot())
        .unwrap();
    let now = host.inspect().control.ledger.elapsed.unwrap();
    let request = host
        .request_human_approval(
            host.revision(),
            id + 1000,
            id,
            &inputs,
            ElapsedTick(now.0 + 30),
        )
        .unwrap();
    let revision = host.revision();
    let human = roles.human.approve(host, revision, &request).unwrap();
    Keys {
        action,
        inputs,
        automatic,
        human,
        request,
    }
}

#[test]
fn original_checked_guard_and_reserve_survive_promotion_and_final_witness_changes() {
    for changed in [false, true] {
        let root = Directory::new();
        let declared = guards();
        let (mut host, roles) = FileOversight::create_guarded_publication_reserved(
            root.store(),
            profile(),
            &declared,
            None,
            RecoveryReserve::terminal(),
            limits(),
        )
        .unwrap();
        assert_eq!(host.revision(), 4); // Witness, reserve, source, campaigns in the sole first image.
        assert_eq!(
            host.publication_validation_profile().unwrap(),
            Some(limits())
        );
        assert_eq!(
            host.journal_capacity().unwrap().reserve(),
            Some(RecoveryReserve::terminal())
        );
        assert!(host.file_source_required());
        assert!(host.policy_campaigns_required());
        assert!(!host.clock_ready());
        assert!(roles.policy_governor.is_some());
        let mut source = source(&root);
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let capture = host
            .refresh_file_source(host.revision(), &mut source, ElapsedTick(1))
            .unwrap();
        let first = ready(&mut host, &roles, &capture, 1, b"first");
        fixture::dispatch(&mut host, &first);
        let published = host
            .publish_checked(
                host.revision(),
                1,
                Some(&first.inputs),
                snapshot(),
                ElapsedTick(2),
            )
            .unwrap();
        assert_eq!(
            published.outcome,
            EndpointOutcome::Executed {
                resulting_version: 2
            }
        );
        assert_eq!(
            host.reconcile(host.revision(), 1).unwrap(),
            Reconciliation::Resolved(published.outcome)
        );

        let state = host.inspect();
        let policy = Policy::new(2, host.current_policy().unwrap().nodes().to_vec()).unwrap();
        let update = PolicyUpdate::new(
            801,
            state.control.sequence,
            state.control.ledger.epoch,
            policy,
        )
        .unwrap();
        let report = host
            .request_policy_campaign(host.revision(), &update)
            .unwrap();
        assert_eq!(report.report().cases().len(), 2);
        let revision = host.revision();
        let key = roles
            .policy_governor
            .as_ref()
            .unwrap()
            .approve(&mut host, revision, &report, false)
            .unwrap();
        host.promote_policy_campaign(host.revision(), &key).unwrap();
        assert_eq!(host.current_policy().unwrap().generation(), 2);
        assert_eq!(host.inspect().control.ledger.epoch, 1);
        assert_eq!(
            host.publication_validation_profile().unwrap(),
            Some(limits())
        );
        assert_eq!(
            host.journal_capacity().unwrap().reserve(),
            Some(RecoveryReserve::terminal())
        );

        host.observe_time(host.revision(), ElapsedTick(3)).unwrap();
        let capture = host
            .refresh_file_source(host.revision(), &mut source, ElapsedTick(3))
            .unwrap();
        let second = ready(&mut host, &roles, &capture, 2, b"second");
        assert_eq!(second.action.spec().policy_epoch, 1);
        let before = host.inspect();
        assert!(
            host.dispatch(
                host.revision(),
                &second.automatic,
                &first.human,
                &second.action,
                &second.inputs,
                snapshot()
            )
            .is_err()
        );
        assert_eq!(host.inspect(), before);
        fixture::dispatch(&mut host, &second);
        assert_eq!(host.inspect().control.ledger.charged, 32);
        host.record_publication_inputs(
            host.revision(),
            2,
            1,
            Some(observations(
                &second.inputs,
                11,
                if changed {
                    &[0, 1, 2, 4]
                } else {
                    &[0, 2, 4, 99]
                },
            )),
        )
        .unwrap();
        assert_eq!(
            host.publish(host.revision(), 2),
            Err(JournalError::Contract(Error::Incomplete))
        );
        let publication = host
            .publish_checked(
                host.revision(),
                2,
                Some(&second.inputs),
                snapshot(),
                ElapsedTick(4),
            )
            .unwrap();
        let expected = if changed {
            assert_eq!(publication.basis, PublicationBasis::Rejected(Error::Stale));
            EndpointOutcome::NotExecuted {
                reason: NonExecutionReason::Sealed,
            }
        } else {
            assert_eq!(publication.basis, PublicationBasis::Revalidated);
            EndpointOutcome::Executed {
                resulting_version: 3,
            }
        };
        assert_eq!(publication.outcome, expected);
        // A refusal after dispatch still remains charged until the original
        // authoritative receipt resolves it; policy promotion never refunds it.
        assert_eq!(host.inspect().control.ledger.charged, 32);
        assert_eq!(
            host.reconcile(host.revision(), 2).unwrap(),
            Reconciliation::Resolved(expected)
        );
        let state = host.inspect();
        assert_eq!(state.executions, if changed { 1 } else { 2 });
        assert_eq!(state.control.ledger.charged, if changed { 16 } else { 32 });
        assert_eq!(state.control.ledger.reserved, 0);
        assert_eq!(
            state.payload,
            if changed {
                b"first".to_vec()
            } else {
                b"second".to_vec()
            }
        );
        assert_eq!(
            FileOversight::read_publication(root.store(), &profile()).unwrap(),
            state
        );
    }
}

#[test]
fn incompatible_checked_bootstrap_inputs_refuse_before_any_store_and_existing_state_is_preserved() {
    for variant in 0..6 {
        let root = Directory::new();
        let mut selected = guards();
        let mut publication = limits();
        let mut reserve = RecoveryReserve::terminal();
        match variant {
            0 => publication.bindings = 0,
            1 => publication.bindings = MAX_PUBLICATION_BINDINGS + 1,
            2 => selected.stream = Some(StreamProfile::new(7, 1, 4, 1024, 4096).unwrap()),
            3 => reserve.events = 0,
            4 => selected.campaigns.as_mut().unwrap().max_campaigns = 0,
            _ => selected.source.as_mut().unwrap().source.scope.run += 1,
        }
        assert!(
            FileOversight::create_guarded_publication_reserved(
                root.store(),
                profile(),
                &selected,
                None,
                reserve,
                publication
            )
            .is_err()
        );
        assert!(!root.store().exists());
    }
    let root = Directory::new();
    let (host, _) = FileOversight::create_guarded_publication_reserved(
        root.store(),
        profile(),
        &guards(),
        None,
        RecoveryReserve::terminal(),
        limits(),
    )
    .unwrap();
    let before = host.inspect();
    drop(host);
    assert!(
        FileOversight::create_guarded_publication_reserved(
            root.store(),
            profile(),
            &guards(),
            None,
            RecoveryReserve::terminal(),
            limits()
        )
        .is_err()
    );
    assert_eq!(
        FileOversight::read_publication(root.store(), &profile()).unwrap(),
        before
    );
}
