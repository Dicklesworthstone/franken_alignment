//! First-image atomicity uses the original Store and native configuration laws.
use super::*;
use super::super::{GeneratedSourcePublication, journal};
use super::super::super::{GeneratedPublicationFeed, GeneratedPublicationProfile};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    MAX_SAMPLING_ENTRIES, text::TextGenerationRequest,
};
use crate::action::consequence::delivery::persistent::{JournalIo, RecoveryReserve};
use crate::action::consequence::delivery::persistent::observed::{
    credibility::{
        andon::tests as accounting,
        held_out_joint::{HeldOutJointBudget, HeldOutJointPolicy},
    },
    decoder::text::FileTextGenerationCommand,
    source::FileSourcePolicy,
};
use crate::action::consequence::delivery::publication_gate::{
    PublicationLimits,
    changes::{PublicationChangePolicy, freshness::PublicationFreshnessPolicy},
};
use crate::action::consequence::delivery::stream::StreamProfile;
use crate::action::consequence::oversight::{
    credibility::Fraction,
    joint_credibility::JointReplayBudget,
    policy_state::{StateFreshness, StateLimits, StateSource},
};
use crate::witness::refinement::{RefinementBudget, index::routing::RoutingBudget};

#[allow(dead_code)]
mod fixtures {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/action/consequence/delivery/persistent/observed/decoder/text/tests/fixtures.rs"
    ));
}
use fixtures::Directory;

fn profile() -> FileOversightProfile {
    let mut profile = fixtures::host_profile();
    profile.delivery.initial_payload.clear();
    profile
}

fn joint() -> JointPromotionPolicy {
    JointPromotionPolicy::new(
        71,
        1,
        Fraction { numerator: 0, denominator: 1 },
        Fraction { numerator: 1, denominator: 2 },
        JointReplayBudget::default(),
    )
    .unwrap()
}

fn selection(mode: usize) -> GeneratedSourceProfile {
    let stream = StreamProfile::new(9, 1, 4, 64, 256).unwrap();
    let reserve = (mode != 0).then_some(RecoveryReserve::terminal());
    let publication = GeneratedPublicationProfile {
        stream,
        reserve,
        limits: PublicationLimits {
            bindings: 4,
            validation: RefinementBudget { steps: 1000, value_bytes: 10000 },
        },
        feed: (mode >= 3).then_some(GeneratedPublicationFeed {
            changes: PublicationChangePolicy {
                source: 71,
                after: 0,
                lookup: RoutingBudget { steps: 1000, bytes: 10000 },
            },
            freshness: PublicationFreshnessPolicy { clock_domain: 99, max_age_ticks: 50 },
            snapshot_fallback: mode >= 4,
        }),
    };
    GeneratedSourceProfile {
        source: FileSourcePolicy {
            source: StateSource {
                scope: profile().delivery.scope,
                source: 51,
                generation: 1,
            },
            limits: StateLimits { events: 64, retained_bytes: 65536 },
            freshness: StateFreshness::new(50).unwrap(),
        },
        publication: match mode {
            0 | 1 => GeneratedSourcePublication::Stream { stream, reserve },
            2..=4 => GeneratedSourcePublication::Checked(publication),
            _ => GeneratedSourcePublication::Joint {
                publication,
                policy: HeldOutJointPolicy::new(
                    70, 1, 1, 1, 0, 0, HeldOutJointBudget::default(),
                )
                .unwrap(),
            },
        },
    }
}

fn evaluation(mode: usize) -> FileGeneratedEvaluationConfig {
    let mut config = FileGeneratedEvaluationConfig::new(accounting::protocol());
    if mode & 1 != 0 {
        config = config.with_joint(joint()).unwrap();
    }
    if mode & 2 != 0 {
        config = config.with_andon(accounting::config()).unwrap();
    }
    config
}

fn prepared(
    selected: GeneratedSourceProfile,
    evaluation: FileGeneratedEvaluationConfig,
) -> Result<PreparedEvaluation, JournalError> {
    Prepared::new(
        profile(),
        fixtures::config(3.0, 65),
        &fixtures::tokenizer(false),
        selected,
    )?
    .evaluated(evaluation)
}

#[test]
fn every_generated_mode_preserves_its_exact_source_and_selected_evaluation_contracts() {
    for mode in 0..6 {
        for ledger in 0..4 {
            let root = Directory::new();
            let selected = selection(mode);
            let config = evaluation(ledger);
            let (host, roles) = FileOversight::create_generated_text_stream_from_source_evaluated(
                root.store(),
                profile(),
                fixtures::config(3.0, 65),
                fixtures::tokenizer(false),
                selected,
                config.clone(),
            )
            .unwrap();
            assert_eq!(selected.check(&host.events), Ok(()));
            assert_eq!(selected.check_machine(&host.machine), Ok(()));
            assert_eq!(host.machine.credibility_contract(), Some(config.protocol()));
            assert_eq!(host.machine.broker.joint_credibility_policy(), config.joint_policy());
            assert_eq!(host.andon_config().unwrap(), config.andon());
            assert_eq!(host.held_out_joint_policy().unwrap(), selected.publication.joint());
            assert_eq!(host.publication_validation_profile().unwrap(),
                selected.publication.witnesses().map(|p| p.limits));
            assert_eq!(host.journal_capacity().unwrap().reserve(), selected.publication.reserve());
            assert_eq!(roles.evaluator.check_owner(&host), Ok(()));
            assert!(roles.oversight.identity_observer.is_none());
            assert!(roles.oversight.policy_governor.is_none());
            assert_eq!(host.credibility_report().unwrap().retained_cases, 0);
            assert_eq!(host.credibility_report().unwrap().revision, 0);
            assert!(!host.clock_ready());
            assert_eq!(host.file_source_status().unwrap().producer, None);
            assert_eq!(host.file_source_status().unwrap().capture.closed, None);
            assert_eq!(host.decoder_inspection().unwrap().numerical.position, 0);
            assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 0);
            assert_eq!(host.inspect().executions, 0);
        }
    }
}

#[test]
fn invalid_evaluation_inputs_refuse_before_creating_a_source_only_owner() {
    for changed in 0..6 {
        let root = Directory::new();
        let mut protocol = accounting::protocol();
        match changed {
            0 => protocol.domain = 0,
            1 => protocol.period = 0,
            2 => protocol.minimum_benign_origins = 0,
            3 => protocol.precision_floor.denominator = 0,
            4 => protocol.false_positive_ceiling.numerator = 3,
            _ => protocol.minimum_violation_origins = 512,
        }
        let result = FileOversight::create_generated_text_stream_from_source_evaluated(
            root.store(),
            profile(),
            fixtures::config(3.0, 65),
            fixtures::tokenizer(false),
            selection(1),
            FileGeneratedEvaluationConfig::new(protocol)
                .with_andon(accounting::config())
                .unwrap(),
        );
        assert!(matches!(result, Err(JournalError::Contract(Error::InvalidInput | Error::Limit))));
        assert!(!root.store().exists());
        let (host, roles) = prepared(selection(1), evaluation(2))
            .unwrap()
            .publish_evaluated(storage::Store::create(&root.store()).unwrap())
            .unwrap();
        assert_eq!(roles.evaluator.check_owner(&host), Ok(()));
        assert_eq!(host.andon_config().unwrap(), Some(&accounting::config()));
    }
}

#[test]
fn event_budget_covers_evaluation_and_andon_before_any_store_is_created() {
    let selected = selection(4);
    let complete = prepared(selected, evaluation(3)).unwrap().prepared.events.len();
    for limit in [complete - 2, complete - 1] {
        let root = Directory::new();
        let mut limited = profile();
        limited.delivery.limits.events = limit;
        assert!(matches!(
            FileOversight::create_generated_text_stream_from_source_evaluated(
                root.store(), limited, fixtures::config(3.0, 65),
                fixtures::tokenizer(false), selected, evaluation(3),
            ),
            Err(JournalError::Contract(Error::Limit))
        ));
        assert!(!root.store().exists());
        let mut sufficient = profile();
        sufficient.delivery.limits.events = complete + RecoveryReserve::terminal().events;
        let (host, _) = FileOversight::create_generated_text_stream_from_source_evaluated(
            root.store(), sufficient, fixtures::config(3.0, 65),
            fixtures::tokenizer(false), selected, evaluation(3),
        )
        .unwrap();
        assert_eq!(host.revision() as usize, complete);
    }
}

#[test]
fn every_original_store_failure_returns_no_partial_owner_or_evaluator_role() {
    for mode in [0, 2, 5] {
        for stage in accounting::BARRIERS {
            let root = Directory::new();
            let selected = selection(mode);
            let prepared = prepared(selected, evaluation(3)).unwrap();
            let store = storage::Store::create(&root.store()).unwrap();
            let expected = journal::encode(
                &prepared.prepared.profile,
                store.identity(),
                &prepared.prepared.events,
            )
            .unwrap();
            store.fail_once(stage);
            assert!(matches!(prepared.publish_evaluated(store),
                Err(JournalError::Io(failure)) if failure.operation == stage));
            let canonical = root.store().join(storage::CANONICAL);
            if stage == JournalIo::DirectorySync {
                assert_eq!(std::fs::read(&canonical).unwrap(), expected);
                // The uncertain image can only be the complete composition.
                // Reading it does not invent the missing acknowledgment/role.
                let (host, _) = FileOversight::open_generated_text_stream_from_source(
                    root.store(), profile(), &fixtures::config(3.0, 65),
                    &fixtures::tokenizer(false), selected,
                )
                .unwrap();
                assert_eq!(host.andon_config().unwrap(), Some(&accounting::config()));
                assert_eq!(host.credibility_report().unwrap().protocol, accounting::protocol());
                assert_eq!(host.machine.broker.joint_credibility_policy(), Some(joint()));
                assert_eq!(host.inspect().executions, 0);
                assert!(!host.clock_ready());
            } else {
                assert!(!canonical.exists(), "{stage:?}");
            }
        }
    }
}

#[test]
fn invalid_source_and_prior_payload_cannot_be_hidden_by_valid_accounting() {
    for changed in 0..2 {
        let root = Directory::new();
        let mut selected = selection(1);
        let mut p = profile();
        match changed {
            0 => selected.source.source.scope.principal += 1,
            _ => p.delivery.initial_payload = b"caller supplied native text".to_vec(),
        }
        assert!(FileOversight::create_generated_text_stream_from_source_evaluated(
            root.store(), p, fixtures::config(3.0, 65), fixtures::tokenizer(false),
            selected, evaluation(2),
        ).is_err());
        assert!(!root.store().exists());
        let (host, roles) = prepared(selection(1), evaluation(2))
            .unwrap()
            .publish_evaluated(storage::Store::create(&root.store()).unwrap())
            .unwrap();
        assert!(host.generated_text_stream_required().unwrap());
        assert_eq!(roles.evaluator.check_owner(&host), Ok(()));
    }
}
