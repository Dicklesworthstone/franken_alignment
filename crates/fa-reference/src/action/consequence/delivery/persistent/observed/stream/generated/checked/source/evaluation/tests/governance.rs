//! Governed evaluated construction is a single original Store transaction.
use super::*;
use crate::action::consequence::delivery::persistent::observed::{
    Machine, governance::campaigns::CampaignEvent, guarded::FileCampaignRequirement,
};
use crate::action::consequence::oversight::policy_governance::MAX_POLICY_CAMPAIGNS;
use crate::action::consequence::policy_campaign::{MAX_REPLAY_CASES, ReplayLimits};

fn campaigns() -> FileCampaignRequirement {
    FileCampaignRequirement {
        limits: ReplayLimits {
            cases: 16,
            input_bytes: 1_048_576,
        },
        max_campaigns: 4,
    }
}

fn configured(ledger: usize) -> FileGeneratedEvaluationConfig {
    evaluation(ledger).with_campaigns(campaigns()).unwrap()
}

fn prepared_governed(
    selected: GeneratedSourceProfile,
    config: FileGeneratedEvaluationConfig,
) -> Result<PreparedEvaluation, JournalError> {
    Prepared::with_campaigns(
        profile(),
        fixtures::config(3.0, 65),
        &fixtures::tokenizer(false),
        selected,
        config.campaign_requirement(),
    )?
    .evaluated(config)
}

#[test]
fn independent_governance_and_evaluation_share_every_compatible_first_image() {
    for mode in 0..5 {
        for ledger in 0..4 {
            let root = Directory::new();
            let config = configured(ledger);
            let (host, roles) = FileOversight::create_generated_text_stream_from_source_evaluated(
                root.store(), profile(), fixtures::config(3.0, 65),
                fixtures::tokenizer(false), selection(mode), config.clone(),
            ).unwrap();
            assert!(host.policy_campaigns_required());
            assert!(roles.oversight.policy_governor.is_some());
            assert_eq!(roles.evaluator.check_owner(&host), Ok(()));
            assert_eq!(host.credibility_protocol().unwrap(), Some(config.protocol()));
            assert_eq!(host.andon_config().unwrap(), config.andon());
            assert_eq!(host.joint_credibility_policy().unwrap(), config.joint_policy());
            assert_eq!(host.journal_capacity().unwrap().reserve(), selection(mode).publication.reserve());
            let guards = host.events.iter().filter_map(|event| match event {
                Event::Campaign(CampaignEvent::Enable(limits, maximum)) => Some((*limits, *maximum)),
                _ => None,
            }).collect::<Vec<_>>();
            assert_eq!(guards, vec![(campaigns().limits, campaigns().max_campaigns)]);
            assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 0);
            assert_eq!(host.inspect().executions, 0);
            assert!(!host.clock_ready());
        }
    }
}

#[test]
fn invalid_or_duplicate_governance_cannot_escape_as_an_unaccounted_owner() {
    assert_eq!(
        configured(3).with_campaigns(campaigns()),
        Err(Error::Duplicate),
    );
    for changed in 0..6 {
        let root = Directory::new();
        let mut requirement = campaigns();
        let mut selected = selection(1);
        match changed {
            0 => requirement.limits.cases = 0,
            1 => requirement.limits.input_bytes = 0,
            2 => requirement.max_campaigns = 0,
            3 => requirement.limits.cases = MAX_REPLAY_CASES + 1,
            4 => requirement.max_campaigns = MAX_POLICY_CAMPAIGNS + 1,
            _ => selected = selection(5),
        }
        let result = FileOversight::create_generated_text_stream_from_source_evaluated(
            root.store(), profile(), fixtures::config(3.0, 65),
            fixtures::tokenizer(false), selected,
            evaluation(3).with_campaigns(requirement).unwrap(),
        );
        assert!(result.is_err(), "case {changed}");
        assert!(!root.store().exists(), "case {changed}");
        let (host, roles) = FileOversight::create_generated_text_stream_from_source_evaluated(
            root.store(), profile(), fixtures::config(3.0, 65),
            fixtures::tokenizer(false), selection(1), configured(3),
        ).unwrap();
        assert!(roles.oversight.policy_governor.is_some());
        assert_eq!(roles.evaluator.check_owner(&host), Ok(()));
    }
}

#[test]
fn combined_governance_and_evaluation_charge_the_complete_image_and_terminal_reserve() {
    for mode in [1, 4] {
        let selected = selection(mode);
        let config = configured(3);
        let complete = prepared_governed(selected, config.clone()).unwrap();
        let count = complete.prepared.events.len();
        let root = Directory::new();
        let path = root.store();
        let identity = std::fs::canonicalize(path.parent().unwrap()).unwrap()
            .join(path.file_name().unwrap());
        let create = |p| FileOversight::create_generated_text_stream_from_source_evaluated(
            &path, p, fixtures::config(3.0, 65), fixtures::tokenizer(false),
            selected, config.clone(),
        );
        let mut limited = profile();
        limited.delivery.limits.events = count + RecoveryReserve::terminal().events - 1;
        assert!(matches!(create(limited), Err(JournalError::Contract(Error::Limit))));
        assert!(!path.exists());

        let mut exact = profile();
        exact.delivery.limits.events = count + RecoveryReserve::terminal().events;
        let bytes = journal::encode(&exact, &identity, &complete.prepared.events).unwrap().len();
        exact.delivery.limits.bytes = bytes + RecoveryReserve::terminal().bytes - 1;
        assert!(matches!(create(exact.clone()), Err(JournalError::Contract(Error::Limit))));
        assert!(!path.exists());
        exact.delivery.limits.bytes += 1;
        let (host, roles) = create(exact).unwrap();
        assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events, 0);
        assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().bytes, 0);
        assert!(roles.oversight.policy_governor.is_some());
        assert_eq!(roles.evaluator.check_owner(&host), Ok(()));
    }
}

#[test]
fn every_storage_barrier_withholds_both_roles_and_preserves_only_the_complete_image() {
    for mode in [1, 4] {
        for stage in accounting::BARRIERS {
            let root = Directory::new();
            let initial = prepared_governed(selection(mode), configured(3)).unwrap();
            let store = storage::Store::create(&root.store()).unwrap();
            let expected = journal::encode(
                &initial.prepared.profile, store.identity(), &initial.prepared.events,
            ).unwrap();
            let identity = store.identity().to_path_buf();
            store.fail_once(stage);
            assert!(matches!(
                initial.publish_evaluated(InitialStore::Injected(store)),
                Err(JournalError::Io(failure)) if failure.operation == stage
            ));
            let canonical = root.store().join(storage::CANONICAL);
            if stage == JournalIo::DirectorySync {
                assert_eq!(std::fs::read(&canonical).unwrap(), expected);
                let events = journal::decode(&profile(), &identity, &expected).unwrap();
                let machine = Machine::replay(&profile(), &events).unwrap();
                assert!(machine.broker.policy_campaigns_required());
                assert_eq!(machine.credibility_contract(), Some(&accounting::protocol()));
                assert_eq!(machine.andon_contract(), Some(&accounting::config()));
                assert_eq!(machine.broker.joint_credibility_policy(), Some(joint()));
            } else {
                assert!(!canonical.exists(), "{stage:?}");
            }
        }
    }
}
