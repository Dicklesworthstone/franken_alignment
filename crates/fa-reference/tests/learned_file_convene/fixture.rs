//! Actual learned actor output, a retained sidecar and fixed original native models.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::{
    guarded::FileGuardSet,
    source::FileSourcePolicy,
};
use fa_reference::action::consequence::gate::containment::{
    ActorState, RestartGrade, RestartProfile,
    session::policy::{Policy, Predicate},
};
use fa_reference::action::consequence::oversight::{
    CommitteeContract, HelperContract,
    policy_state::{StateFreshness, StateLimits, StateSource},
};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::reducer::Caps;
use std::path::{Path, PathBuf};

pub fn profile() -> FileOversightProfile {
    let mut profile = legacy::profile();
    profile.delivery.total = 4096;
    profile.delivery.initial_payload.clear();
    profile.delivery.actor = ActorState::new(
        RestartProfile {
            id: 1, generation: 1, host_generation: 1, model_generation: 3,
            tokenizer_generation: 4, state_schema_generation: 1,
            grade: RestartGrade::AuditOnly,
        },
        Vec::new(), vec![0], vec![0], 0,
    ).unwrap();
    profile.delivery.policy = Policy::new(1, vec![
        Predicate::ExactValue { key: 7, value: b"ok".to_vec() },
        Predicate::PayloadAtMost(4096),
        Predicate::All(vec![0, 1]),
    ]).unwrap();
    profile.delivery.congress = CongressPolicy {
        generation: 1,
        members: BTreeMap::from([("reviewer".to_owned(), MemberPolicy {
            cohort: "reference".to_owned(), weight: 1,
        })]),
        caps: Caps { per_member: 1, per_cohort: 1 },
        continue_minimum: 1, continue_hold_maximum: 0,
        narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1,
    };
    profile.committee = CommitteeContract::new(BTreeMap::from([(
        "reviewer".to_owned(),
        HelperContract::new(InputProfileBinding {
            profile_id: 1, profile_bytes: b"exact-view-v1".to_vec(),
            tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3,
        }, 7, b"approve?".to_vec()).unwrap(),
    )])).unwrap();
    profile
}

pub fn stream() -> StreamProfile { StreamProfile::new(1, 1, 4, 64, 128).unwrap() }

fn policy_source() -> FileSourcePolicy {
    FileSourcePolicy {
        source: StateSource { scope: profile().delivery.scope, source: 42, generation: 1 },
        limits: StateLimits::default(),
        freshness: StateFreshness::new(100).unwrap(),
    }
}

pub fn recipe() -> FileLearnedConfig {
    let model = numerical::model(&[u32::from(b'O'), u32::from(b'K'), numerical::END]);
    FileLearnedConfig::new_text_stream(
        model.clone(), numerical::tokenizer(&model), numerical::config(&model),
        LearnedDecoderBindingLimits::default(), stream(),
    ).unwrap().with_required_sidecar().unwrap()
        .with_required_policy_source(policy_source()).unwrap()
}

pub fn document(generation: u64, snapshot: Snapshot, context: &[u8]) -> EvidenceSnapshot {
    EvidenceSnapshot::new(
        EvidenceIdentity { source: 42, generation, scope: profile().delivery.scope },
        snapshot,
        BTreeMap::from([("reviewer".to_owned(), context.to_vec())]),
    ).unwrap()
}

pub fn replace(path: &Path, document: &EvidenceSnapshot) {
    let staged = path.with_extension("pending");
    std::fs::write(&staged, document.encode()).unwrap();
    std::fs::rename(staged, path).unwrap();
}

pub fn command(port: &FileSupervisorConvenePort, round: u64) -> FileConveneRequest {
    let Knowledge::Known { value, .. } = port.next(REQUEST, round).unwrap() else {
        panic!("expected the original learned held-review cut");
    };
    value.convene_request().expect("fixed next native roster remains eligible")
}

pub fn receipt(port: &FileSupervisorConvenePort, command: FileConveneRequest) -> FileConveneReceipt {
    let Knowledge::Known { value, .. } = port.convene(command).unwrap() else {
        panic!("expected the original finite convening receipt");
    };
    value
}

pub fn blocked(port: &FileSupervisorConvenePort, round: u64) {
    let Knowledge::Known { value, .. } = port.next(REQUEST, round).unwrap() else {
        panic!("expected the original bounded plan");
    };
    assert!(!value.can_convene());
    assert!(value.block_reason().is_some());
    assert!(value.convene_request().is_none());
}

pub struct Case {
    pub root: legacy::Directory,
    pub actor: FileLearnedTextStreamActorPort,
    pub ticket: FileActorTicket<FileOversight>,
    pub reviewer: FileHumanReviewer,
    pub run: FileNativeSupervisedDriver,
    pub source: FileEvidenceSource,
    pub path: PathBuf,
    pub attempt: u64,
}

impl Case {
    pub fn new(spellings: &[&[u8]], polls: usize) -> Self {
        let root = legacy::Directory::new();
        let path = root.0.join("policy-evidence.json");
        let document = document(1, legacy::snapshot(), &[]);
        replace(&path, &document);
        let mut source = FileEvidenceSource::new(
            &path, 42, profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES,
        ).unwrap();
        let guards = FileGuardSet {
            stream: Some(stream()), decoder: None, decoder_stop: None,
            source: Some(policy_source()), identity: None, campaigns: None, credential: None,
        };
        let (mut host, roles) = FileOversight::create_guarded_with_learned_generation_reserved(
            root.store(), profile(), &guards, None, recipe(), RecoveryReserve::terminal(),
        ).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1)).unwrap();
        for _ in 0..8 {
            let current = host.learned_generation_inspection().unwrap().numerical;
            if !current.status.is_active() { break; }
            host.advance_learned_generation(host.revision(), current.actor_revision, current.position)
                .unwrap().unwrap();
        }
        assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"OK");
        let (actor, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let revision = supervisor.host().unwrap().revision();
        supervisor.set_snapshot(revision, Some(legacy::snapshot())).unwrap();
        let ticket = actor.submit(REQUEST, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
        let mut driver = FileSupervisedDriver::new(supervisor);
        let (attempt, launch) = {
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            let FileRequestDisposition::Admitted { attempt, stage: ActionState::Reviewing } =
                host.request_status(REQUEST).unwrap().disposition else {
                    panic!("original learned text request was not admitted");
                };
            let message = host.learned_text_message(LearnedEvidenceLimits::default()).unwrap();
            let original_source = message.evidence().audit().source();
            let rows: BTreeSet<_> = original_source.groups().map(|group| group.row).collect();
            let options = LearnedSidecarRequest {
                identity: SidecarIdentity { object_id: 1001, generation: 1, transform_id: 7 },
                priority: original_source.groups().collect(),
                budget: SidecarCongressBudget::default(),
            };
            let actor_revision = host.learned_generation_inspection().unwrap().numerical.actor_revision;
            let revision = host.revision();
            let sidecar = host.begin_learned_sidecar_plan(revision, attempt, actor_revision, options).unwrap();
            let input = host.current_learned_sidecar(&sidecar).unwrap();
            let mut rounds = Vec::new();
            let mut rosters = BTreeMap::new();
            for (index, spelling) in spellings.iter().enumerate() {
                let round = HELD_ROUND + index as u64;
                let members = input.views().iter().map(|(name, view)| {
                    let queries = rows.iter().map(|row| {
                        let (frame, heads, channels) = original_source.row_shape(*row).unwrap();
                        SidecarProbeQuery {
                            row: *row,
                            probe: LinearProbe::new(
                                1, 1, frame.profile, &vec![0.0; heads * channels], 0.0, 1.0,
                            ).unwrap(),
                        }
                    }).collect();
                    (name.clone(), NativeReviewMember {
                        evaluator: native_fixture::evaluator(
                            view.actual_input().input_profile().clone(), spelling,
                        ),
                        queries,
                        salt: vec![16 + index as u8; 32],
                    })
                }).collect();
                rounds.push(LearnedWorkerRound {
                    round, evidence_root: document.reference_root(),
                    window: ReviewWindow {
                        commit_by: ElapsedTick(10 + index as u64 * 20),
                        reveal_by: ElapsedTick(15 + index as u64 * 20),
                    },
                });
                rosters.insert(round, members);
            }
            (attempt, FileNativeDriverLaunch {
                journal_revision: host.revision(), request: REQUEST, sidecar, rounds, rosters,
                limits: NativeReviewLimits { polls, ..NativeReviewLimits::default() },
            })
        };
        let run = driver.start_native_learned_sequence(launch, legacy::snapshot(), ElapsedTick(1)).unwrap();
        Self { root, actor, ticket, reviewer: roles.human, run, source, path, attempt }
    }

    pub fn held() -> Self {
        let mut case = Self::new(&[b"hold", b"allow"], MAX_LEARNED_REVIEW_POLLS);
        case.complete(Consequence::HoldEffect);
        case
    }

    pub fn held_with_grant(
        queries: usize, operations: usize,
    ) -> (Self, FileSupervisorConvenePort, FileSupervisorConveneGrant) {
        let mut case = Self::new(&[b"hold", b"allow"], MAX_LEARNED_REVIEW_POLLS);
        let (port, grant) = case.run.supervisor().convene_port(queries, operations).unwrap();
        case.complete(Consequence::HoldEffect);
        (case, port, grant)
    }

    pub fn image(&self) -> Vec<u8> {
        std::fs::read(self.root.store().join("delivery.bin")).unwrap()
    }

    pub fn complete(&mut self, expected: Consequence) {
        for _ in 0..MAX_LEARNED_REVIEW_POLLS {
            if self.run.review().status() != NativeReviewStatus::Running { break; }
            assert!(matches!(
                self.run.step(|| ElapsedTick(1), || Ok(legacy::snapshot()), None).unwrap(),
                FileNativeDriverEvent::Review(_)
            ));
        }
        assert_eq!(self.run.review().status(), NativeReviewStatus::Finished);
        let FileLearnedSidecarFinish::Applied { receipt: Ok(receipt), .. } =
            self.run.review().history().last().unwrap() else {
                panic!("actual native model did not apply its original learned sidecar review");
            };
        assert_eq!(receipt.policy.control.decision.consequence, expected);
        let round = self.run.review().round().round;
        let record = self.run.review().records()[&round].members["reviewer"].as_ref().unwrap();
        assert_eq!(record.progress.basis, Some(SidecarDecisionBasis::NativeModel));
        assert_eq!(record.progress.native.work.sampled_draws, 2);
    }

    pub fn convene(&mut self) {
        let ticket = self.run.take_pending_convene().unwrap().unwrap();
        let report = self.run.convene_from_policy_file(&mut self.source, ticket, || ElapsedTick(1));
        assert_eq!(report.observations, vec![Ok(document(1, legacy::snapshot(), &[]).identity())]);
        report.result.unwrap();
    }

    pub fn tick(&mut self, human: Option<&FileHumanPermit>) -> FileDriverEvent {
        match self.run.step_from_policy_file(&mut self.source, || ElapsedTick(1), human).result.unwrap() {
            FileNativeDriverEvent::Driver(event) => event,
            other => panic!("completed native review did not yield the original driver: {other:?}"),
        }
    }

    pub fn approve(&mut self) -> FileHumanPermit {
        assert!(matches!(self.tick(None), FileDriverEvent::AwaitingHuman { request: REQUEST }));
        let request = self.run.request_human_approval_from_policy_file(
            &mut self.source, 9001, ElapsedTick(31), || ElapsedTick(1),
        ).result.unwrap();
        let mut host = self.run.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        self.reviewer.approve(&mut host, revision, &request).unwrap()
    }

    pub fn publish(&mut self, human: &FileHumanPermit) {
        assert!(matches!(self.tick(Some(human)), FileDriverEvent::Dispatched { request: REQUEST, .. }));
        assert_eq!(self.run.supervisor().host().unwrap().inspect().executions, 0);
        assert!(matches!(self.tick(None), FileDriverEvent::PublicationChecked {
            publication, source_failure: None, ..
        } if matches!(publication.outcome, EndpointOutcome::Executed { .. })));
        assert!(matches!(self.tick(None), FileDriverEvent::Reconciled {
            request: REQUEST, outcome: Reconciliation::Resolved(EndpointOutcome::Executed { .. }),
        }));
        assert!(matches!(self.actor.poll(&self.ticket), Knowledge::Known {
            value: ActorOutcome::Executed, ..
        }));
    }
}
