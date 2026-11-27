//! Native policy promotion and independent labels retain one original owner.
use super::*;
use crate::action::consequence::delivery::persistent::observed::{
    credibility::andon::tests as accounting,
    guarded::{
        FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
        evaluation::{FileEvaluatedOversightRoles, andon::FileAndonRecoveryRequirements},
    },
    stream::generated::checked::source::evaluation::FileGeneratedEvaluationConfig,
};
use crate::action::consequence::oversight::{
    credibility::{Assessment, Fraction, GroundTruth},
    joint_credibility::{JointPromotionPolicy, JointReplayBudget},
};

fn joint_policy() -> JointPromotionPolicy {
    JointPromotionPolicy::new(
        71, 1, Fraction { numerator: 0, denominator: 1 },
        Fraction { numerator: 1, denominator: 2 }, JointReplayBudget::default(),
    ).unwrap()
}

fn evaluated(
    root: &Directory,
    selected: GeneratedSourceProfile,
    joint: bool,
) -> (FileOversight, FileEvaluatedOversightRoles) {
    let mut config = FileGeneratedEvaluationConfig::new(accounting::protocol())
        .with_andon(accounting::config()).unwrap()
        .with_campaigns(campaigns()).unwrap();
    if joint {
        config = config.with_joint(joint_policy()).unwrap();
    }
    FileOversight::create_generated_text_stream_from_source_evaluated(
        root.store(), governed_profile(), fixtures::stopped_config(), tokenizer(false),
        selected, config,
    ).unwrap()
}

fn expected(
    host: &FileOversight,
    selected: GeneratedSourceProfile,
    joint: bool,
    campaigns: Option<FileCampaignRequirement>,
) -> FileAndonRecoveryRequirements {
    let snapshot = host.inspect();
    let mut expected = FileAndonRecoveryRequirements::new(
        FileRecoveryRequirements {
            guards: FileGuardSet {
                stream: Some(selected.publication.stream()),
                decoder: Some(fixtures::stopped_config()),
                decoder_stop: None,
                source: Some(selected.source),
                identity: None,
                campaigns,
                credential: None,
            },
            effective_policy: host.current_policy().unwrap().clone(),
            credential_epoch: None,
            minimum: FileRecoveryFloor {
                journal_revision: snapshot.revision,
                control_sequence: snapshot.control.sequence,
                authority_epoch: snapshot.control.ledger.epoch,
            },
        },
        accounting::config(),
        host.andon_ledger().unwrap().windows().values().map(|w| w.budget).collect(),
        host.andon_source().unwrap().revision(),
    ).unwrap();
    if joint {
        expected = expected.require_joint(joint_policy()).unwrap();
    }
    if let Some(publication) = selected.publication.witnesses() {
        expected = expected.require_publication(publication.limits).unwrap();
    }
    expected
}

#[test]
fn real_native_publications_policy_changes_and_delayed_labels_survive_the_same_recovery() {
    for joint in [false, true] {
        let root = Directory::new();
        let selected = selection(1);
        let (mut host, roles) = evaluated(&root, selected, joint);
        let old_evaluator = roles.evaluator;
        let old_governor = roles.oversight.policy_governor.unwrap();
        let mut source = evidence(&root);
        let captured = generate(&mut host, &mut source, 7);
        release(&mut host, &roles.oversight.human, &captured, 91, 7, true);
        let ticket = old_evaluator.evaluation_case(&host, 191).unwrap();
        let numerical = host.decoder_inspection().unwrap().numerical;
        let ledger = host.inspect().control.ledger;
        let campaign = campaign(&mut host, 701);
        assert_eq!(campaign.report().cases().len(), 2);
        let revision = host.revision();
        let permit = old_governor.approve(&mut host, revision, &campaign, false).unwrap();
        host.promote_policy_campaign(host.revision(), &permit).unwrap();
        assert_eq!(host.current_policy().unwrap().generation(), 2);
        assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
        assert_eq!(host.inspect().control.ledger.charged, ledger.charged);
        assert_eq!(host.inspect().executions, 1);
        assert!(accounting::assess(&mut host, &old_evaluator, 91, GroundTruth::Censored).unwrap());
        let requirements = expected(&host, selected, joint, Some(campaigns()));
        let anchor = host.history_anchor().unwrap();
        let revision = host.revision();
        drop(host);

        let (mut host, fresh) =
            FileOversight::open_andon_guarded_generated_text_stream_from_source_anchored(
                root.store(), governed_profile(), &requirements, &fixtures::stopped_config(),
                &tokenizer(false), selected, &accounting::protocol(), &anchor,
            ).unwrap();
        assert_eq!(host.revision(), revision + 1);
        assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
        assert_eq!(host.current_policy().unwrap().generation(), 2);
        assert_eq!(host.inspect().control.ledger.charged, ledger.charged);
        assert!(fresh.oversight.policy_governor.is_some());
        assert_eq!(old_evaluator.check_owner(&host), Err(Error::Binding.into()));
        let revision = host.revision();
        assert!(matches!(
            old_governor.approve(&mut host, revision, &campaign, false),
            Err(JournalError::Contract(Error::Binding))
        ));
        assert_eq!(
            fresh.evaluator.assess(&mut host, revision, &ticket, Assessment {
                origin: 91, evidence_id: [17; 32], truth: GroundTruth::Benign,
            }),
            Err(Error::Binding.into())
        );
        assert!(!host.clock_ready());
        assert!(host.capture_file_policy_state().is_err());
        host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1)).unwrap();
        assert!(accounting::assess(&mut host, &fresh.evaluator, 91, GroundTruth::Benign).unwrap());
        assert_eq!(host.evaluation_history(191).unwrap().len(), 2);
        assert_eq!(host.credibility_report().unwrap().retained_cases, 1);
        assert_eq!(host.inspect().executions, 1, "a label is not a publication key");
        assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 0);

        let captured = host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1)).unwrap();
        let command = host.prepare_decoder_text_continuation(91, 8, request(b"ab", 2)).unwrap();
        host.begin_decoder_text_continuation(host.revision(), 91, command).unwrap();
        while host.decoder_text_progress(8).unwrap().finish().is_none() {
            let step = host.decoder_text_progress(8).unwrap().generation_revision();
            host.advance_decoder_text(host.revision(), 8, step).unwrap();
        }
        release(&mut host, &fresh.oversight.human, &captured, 92, 8, true);
        assert_eq!(host.inspect().executions, 2);
        assert_eq!(
            host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(),
            ["A", "A"],
        );
        assert_eq!(host.credibility_report().unwrap().retained_cases, 2);
        assert!(host.inspect().control.ledger.charged > ledger.charged);
    }
}

#[test]
fn accounted_recovery_rejects_missing_or_changed_governance_without_a_fence() {
    for mode in [1, 2] {
        for joint in [false, true] {
            let root = Directory::new();
            let selected = selection(mode);
            let (mut host, roles) = evaluated(&root, selected, joint);
            let mut source = evidence(&root);
            generate(&mut host, &mut source, 7);
            let numerical = host.decoder_inspection().unwrap().numerical;
            let exact = expected(&host, selected, joint, Some(campaigns()));
            let mut changed = campaigns();
            changed.max_campaigns += 1;
            let wrong = [
                expected(&host, selected, joint, None),
                expected(&host, selected, joint, Some(changed)),
            ];
            let anchor = host.history_anchor().unwrap();
            let before = bytes(&host);
            drop(host);
            for required in wrong {
                let opened =
                    FileOversight::begin_open_andon_guarded_generated_text_stream_from_source_anchored(
                        root.store(), governed_profile(), &required, &fixtures::stopped_config(),
                        &tokenizer(false), selected, &accounting::protocol(), &anchor,
                    );
                if let Ok(mut run) = opened {
                    while run.progress().status == FileGeneratedSourceRecoveryStatus::Replaying {
                        if run.advance(run.progress().replayed_events, 1).is_err() {
                            break;
                        }
                    }
                    assert!(run.finish().is_err());
                }
                assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), before);
            }
            let (host, fresh) =
                FileOversight::open_andon_guarded_generated_text_stream_from_source_anchored(
                    root.store(), governed_profile(), &exact, &fixtures::stopped_config(),
                    &tokenizer(false), selected, &accounting::protocol(), &anchor,
                ).unwrap();
            assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
            assert!(fresh.oversight.policy_governor.is_some());
            assert_eq!(fresh.evaluator.check_owner(&host), Ok(()));
            assert_eq!(roles.evaluator.check_owner(&host), Err(Error::Binding.into()));
        }
    }
}
