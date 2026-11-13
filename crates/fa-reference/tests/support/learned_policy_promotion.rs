//! Public native controls shared only by the live learned promotion target.
use super::*;
pub(super) use legacy::{Directory, snapshot};

pub(super) struct Rig {
    pub root: Directory,
    pub actor: FileLearnedTextStreamActorPort,
    pub supervisor: Supervisor,
    pub human: FileHumanReviewer,
    pub governor: FilePolicyGovernor,
}

pub(super) fn profile() -> FileOversightProfile {
    let mut p = legacy::profile();
    p.delivery.total = 4096;
    p.delivery.initial_payload.clear();
    p.delivery.actor = ActorState::new(
        RestartProfile {
            id: 1,
            generation: 1,
            host_generation: 1,
            model_generation: 3,
            tokenizer_generation: 4,
            state_schema_generation: 1,
            grade: RestartGrade::AuditOnly,
        },
        Vec::new(),
        vec![0],
        vec![0],
        0,
    )
    .unwrap();
    p.delivery.policy = policy(1);
    p.delivery.congress = CongressPolicy {
        generation: 1,
        members: BTreeMap::from([(
            "reviewer".to_owned(),
            MemberPolicy {
                cohort: "reference".to_owned(),
                weight: 1,
            },
        )]),
        caps: Caps {
            per_member: 1,
            per_cohort: 1,
        },
        continue_minimum: 1,
        continue_hold_maximum: 0,
        narrow_at: 2,
        suspend_at: 3,
        minimum_members: 1,
        minimum_cohorts: 1,
    };
    p.committee = CommitteeContract::new(BTreeMap::from([(
        "reviewer".to_owned(),
        HelperContract::new(
            InputProfileBinding {
                profile_id: 1,
                profile_bytes: b"exact-view-v1".to_vec(),
                tokenizer_epoch: 4,
                policy_epoch: 0,
                model_epoch: 3,
            },
            7,
            b"approve?".to_vec(),
        )
        .unwrap(),
    )]))
    .unwrap();
    p
}

fn policy(generation: u64) -> Policy {
    Policy::new(
        generation,
        vec![
            Predicate::ExactValue {
                key: 7,
                value: b"ok".to_vec(),
            },
            Predicate::PayloadAtMost(4096),
            Predicate::All(vec![0, 1]),
        ],
    )
    .unwrap()
}
pub(super) fn candidate() -> Policy {
    policy(2)
}
pub(super) fn stream() -> StreamProfile {
    StreamProfile::new(1, 1, 4, 64, 128).unwrap()
}
pub(super) fn recipe() -> FileLearnedConfig {
    let model = numerical::model(&[u32::from(b'O'), u32::from(b'K'), numerical::END]);
    FileLearnedConfig::new_text_stream(
        model.clone(),
        numerical::tokenizer(&model),
        numerical::config(&model),
        LearnedDecoderBindingLimits::default(),
        stream(),
    )
    .unwrap()
    .with_required_sidecar()
    .unwrap()
}
pub(super) fn guards() -> FileGuardSet {
    FileGuardSet {
        stream: Some(stream()),
        decoder: None,
        decoder_stop: None,
        source: None,
        identity: None,
        campaigns: Some(FileCampaignRequirement {
            limits: ReplayLimits {
                cases: 16,
                input_bytes: 1_048_576,
            },
            max_campaigns: 8,
        }),
        credential: None,
    }
}
pub(super) fn ready() -> Rig {
    let root = Directory::new();
    let (mut host, roles) = FileOversight::create_guarded_with_learned_generation_reserved(
        root.store(),
        profile(),
        &guards(),
        None,
        recipe(),
        RecoveryReserve::terminal(),
    )
    .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    for _ in 0..8 {
        let current = host.learned_generation_inspection().unwrap().numerical;
        if !current.status.is_active() {
            break;
        }
        let revision = host.revision();
        host.advance_learned_generation(revision, current.actor_revision, current.position)
            .unwrap()
            .unwrap();
    }
    assert!(
        !host
            .learned_generation_inspection()
            .unwrap()
            .numerical
            .status
            .is_active()
    );
    assert_eq!(
        host.learned_text_message(LearnedEvidenceLimits::default())
            .unwrap()
            .bytes(),
        b"OK"
    );
    let mut denied = snapshot();
    denied.values.insert(7, b"denied".to_vec());
    host.propose_learned_text_stream_message(
        host.revision(),
        SEED,
        ElapsedTick(100),
        Vec::new(),
        denied,
    )
    .unwrap();
    assert_eq!(
        host.inspect().control.ledger.stages[&SEED],
        ActionState::Denied
    );
    let (actor, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    Rig {
        root,
        actor,
        supervisor,
        human: roles.human,
        governor: roles
            .policy_governor
            .expect("first-image campaign governor"),
    }
}
pub(super) fn bytes(root: &Directory) -> Vec<u8> {
    std::fs::read(root.store().join("delivery.bin")).unwrap()
}
pub(super) fn observe(supervisor: &mut Supervisor) {
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
}
pub(super) fn approve(
    supervisor: &mut Supervisor,
    governor: &FilePolicyGovernor,
) -> FilePolicyPromotionPermit {
    let mut host = supervisor.host_mut().unwrap();
    let state = host.inspect();
    let update = PolicyUpdate::new(
        CAMPAIGN,
        state.control.sequence,
        state.control.ledger.epoch,
        candidate(),
    )
    .unwrap();
    let revision = host.revision();
    let review = host.request_policy_campaign(revision, &update).unwrap();
    assert!(
        !review.report().cases().is_empty(),
        "campaign must review original retained cases"
    );
    assert!(review.report().newly_reviewable().is_empty());
    let revision = host.revision();
    governor
        .approve(&mut host, revision, &review, false)
        .unwrap()
}
pub(super) fn plan(result: FilePolicyPromotionPlanResult) -> FilePolicyPromotionPlan {
    let Ok(Knowledge::Known { value, basis }) = result else {
        panic!("expected original current campaign cut: {result:?}");
    };
    assert_eq!(value.basis(), basis);
    value
}
pub(super) fn start_review(
    mut driver: FileSupervisedDriver,
    request: u64,
    round: u64,
) -> FileNativeSupervisedDriver {
    let launch = {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let FileRequestDisposition::Admitted { attempt, .. } =
            host.request_status(request).unwrap().disposition
        else {
            panic!("native source admission");
        };
        let message = host
            .learned_text_message(LearnedEvidenceLimits::default())
            .unwrap();
        let source = message.evidence().audit().source();
        let rows: BTreeSet<_> = source.groups().map(|group| group.row).collect();
        let options = LearnedSidecarRequest {
            identity: SidecarIdentity {
                object_id: 1000 + request,
                generation: 1,
                transform_id: 7,
            },
            priority: source.groups().collect(),
            budget: SidecarCongressBudget::default(),
        };
        let current = host.learned_generation_inspection().unwrap();
        let revision = host.revision();
        let sidecar = host
            .begin_learned_sidecar_plan(
                revision,
                attempt,
                current.numerical.actor_revision,
                options,
            )
            .unwrap();
        let input = host.current_learned_sidecar(&sidecar).unwrap();
        let roster = input
            .views()
            .iter()
            .map(|(name, view)| {
                let queries = rows
                    .iter()
                    .map(|row| {
                        let (frame, heads, channels) = source.row_shape(*row).unwrap();
                        SidecarProbeQuery {
                            row: *row,
                            probe: LinearProbe::new(
                                1,
                                1,
                                frame.profile,
                                &vec![0.0; heads * channels],
                                0.0,
                                1.0,
                            )
                            .unwrap(),
                        }
                    })
                    .collect();
                (
                    name.clone(),
                    NativeReviewMember {
                        evaluator: native_fixture::evaluator(
                            view.actual_input().input_profile().clone(),
                            b"allow",
                        ),
                        queries,
                        salt: vec![16; 32],
                    },
                )
            })
            .collect();
        FileNativeDriverLaunch {
            journal_revision: host.revision(),
            request,
            sidecar,
            rounds: vec![LearnedWorkerRound {
                round,
                evidence_root: [7; 32],
                window: ReviewWindow {
                    commit_by: ElapsedTick(10),
                    reveal_by: ElapsedTick(15),
                },
            }],
            rosters: BTreeMap::from([(round, roster)]),
            limits: NativeReviewLimits::default(),
        }
    };
    driver
        .start_native_learned_sequence(launch, snapshot(), ElapsedTick(1))
        .unwrap()
}
pub(super) fn complete(run: &mut FileNativeSupervisedDriver) {
    for _ in 0..MAX_LEARNED_REVIEW_POLLS {
        if run.review().status() != NativeReviewStatus::Running {
            break;
        }
        assert!(matches!(
            run.step(|| ElapsedTick(1), || Ok(snapshot()), None)
                .unwrap(),
            FileNativeDriverEvent::Review(_)
        ));
    }
    assert_eq!(run.review().status(), NativeReviewStatus::Finished);
    let records: Vec<_> = run
        .review()
        .records()
        .values()
        .flat_map(|round| round.members.values().filter_map(Option::as_ref))
        .collect();
    assert!(!records.is_empty());
    assert!(records.iter().any(|record| record.progress.basis
        == Some(SidecarDecisionBasis::NativeModel)
        && record.progress.native.work.sampled_draws == 2));
}
pub(super) fn tick(
    run: &mut FileNativeSupervisedDriver,
    human: Option<
        &fa_reference::action::consequence::delivery::persistent::observed::FileHumanPermit,
    >,
) -> FileDriverEvent {
    match run
        .step(|| ElapsedTick(1), || Ok(snapshot()), human)
        .unwrap()
    {
        FileNativeDriverEvent::Driver(event) => event,
        event => panic!("finished native review must yield the original driver: {event:?}"),
    }
}
