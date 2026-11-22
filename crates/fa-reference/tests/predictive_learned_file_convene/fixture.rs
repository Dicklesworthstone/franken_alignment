//! The original actor, forecast and native models; no supplied verdict authority.
use super::*;
use fa_reference::action::consequence::activation::consistency::{
    BinaryForecast, ErrorBudget, ForecastRegistration,
};
use fa_reference::action::consequence::activation::monitor::learned::LearnedMonitorBudget;
use fa_reference::action::consequence::activation::probe::learned::MAX_CHECKED_KV_BYTES;
use fa_reference::action::consequence::activation::tensor::kv::experiment::KvSide;
use fa_reference::action::consequence::delivery::persistent::observed::consistency::{
    FileConsistencyConfig, FileConsistencyObserver, FileConsistencyParameters,
    FileConsistencySnapshot,
    learned::{FileLearnedConsistencyConfig, FileLearnedConsistencySnapshot},
    pre_output::FileOwnedPreOutputStart,
};
use fa_reference::action::consequence::oversight::consistency::{
    ConsistencyObservation, LearnedConsistencyObservation,
};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug)]
pub enum Mode {
    Fixed,
    Reserved,
}

fn recipe(owned: bool, crossing: bool, predictions: usize) -> FileLearnedConfig {
    let model = numerical::model(&[u32::from(b'O'), u32::from(b'K'), numerical::END]);
    let pair = if crossing {
        BinaryForecast::new(1, 32768).unwrap()
    } else {
        BinaryForecast::new(32768, 32768).unwrap()
    };
    let forecast = FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 91,
        probe_generation: 1,
        profile: if owned {
            model.cache_profile().layers()[&1].values().profile()
        } else {
            model.residual_contract(1).unwrap().profile()
        },
        weights: vec![0.0; 4],
        bias: 0.0,
        threshold: 0.0,
        forecast: ForecastRegistration {
            domain: 71,
            generation: 1,
            policy_generation: 1,
            event_prefix: b"OK".to_vec(),
            negative: pair,
            at_threshold: pair,
            positive: pair,
        },
        alpha: ErrorBudget::new(1, 2).unwrap(),
        stream: 21,
        max_predictions: predictions,
        max_prediction_age_ticks: 10,
    })
    .unwrap()
    .with_stream_messages(ordinary::stream())
    .unwrap();
    let base = ordinary::recipe();
    let pinned = if owned {
        base.with_required_owned_pre_output_forecast(
            FileLearnedConsistencyConfig::new(
                forecast,
                1,
                KvSide::Value,
                LearnedMonitorBudget::default(),
                LearnedMonitorBudget::default(),
                MAX_CHECKED_KV_BYTES,
            )
            .unwrap()
            .with_owned_generation()
            .unwrap()
            .with_pre_output_forecast()
            .unwrap(),
        )
    } else {
        base.with_required_pre_output_forecast(
            forecast
                .with_hosted_residual(1)
                .unwrap()
                .with_pre_output_forecast()
                .unwrap(),
        )
    };
    pinned
        .unwrap()
        .with_forecast_text_stream_continuation()
        .unwrap()
}

fn generate(
    host: &mut FileOversight,
    observer: &FileConsistencyObserver,
    owned: bool,
    request: u64,
) {
    let prompt = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(prompt.sampled_draws, 0);
    let revision = host.revision();
    if owned {
        let FileOwnedPreOutputStart::Ready(mut cursor) = observer
            .begin_owned_pre_output_request(
                host,
                revision,
                request,
                prompt.actor_revision,
                prompt.position,
            )
            .unwrap()
        else {
            panic!("the real owned prompt forecast must finish before the first sample");
        };
        assert!(cursor.forecast_report().prediction().is_ok());
        while cursor.numerical().status.is_active() {
            let revision = host.revision();
            cursor.advance(host, revision).unwrap().unwrap();
        }
    } else {
        let mut cursor = observer
            .begin_pre_output_request(
                host,
                revision,
                request,
                prompt.actor_revision,
                prompt.position,
            )
            .unwrap()
            .unwrap();
        while cursor.numerical().status.is_active() {
            let revision = host.revision();
            cursor.advance(host, revision).unwrap().unwrap();
        }
    }
    assert_eq!(
        host.learned_text_message(LearnedEvidenceLimits::default())
            .unwrap()
            .bytes(),
        b"OK"
    );
}

pub fn forecast(
    host: &mut FileOversight,
    observer: &FileConsistencyObserver,
    owned: bool,
    request: u64,
) -> Result<(), fa_reference::action::consequence::delivery::persistent::JournalError> {
    let numerical = host.learned_generation_inspection()?.numerical;
    let revision = host.revision();
    if owned {
        observer
            .forecast_owned_learned_request(host, revision, request, numerical.actor_revision)?
            .and_then(|report| report.prediction().map(|_| ()))
            .map_err(Into::into)
    } else {
        observer
            .forecast_hosted_request(host, revision, request, numerical.actor_revision)?
            .map(|_| ())
            .map_err(Into::into)
    }
}

pub struct Case {
    pub root: legacy::Directory,
    pub actor: FileLearnedTextStreamActorPort,
    pub ticket: FileActorTicket<FileOversight>,
    pub reviewer: FileHumanReviewer,
    pub observer: FileConsistencyObserver,
    pub run: FileNativeSupervisedDriver,
    pub source: FileEvidenceSource,
    pub path: PathBuf,
    pub attempt: u64,
    pub request: u64,
    pub owned: bool,
    pub mode: Mode,
}

// Build every evaluator before the first Begin, including explicit reserves.
fn launch(
    mut driver: FileSupervisedDriver,
    request: u64,
    first_round: u64,
    spellings: &[&[u8]],
    mode: Mode,
) -> (u64, FileNativeSupervisedDriver) {
    let (attempt, launch, successors) = {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let FileRequestDisposition::Admitted {
            attempt,
            stage: ActionState::Reviewing,
        } = host.request_status(request).unwrap().disposition
        else {
            panic!("the actual forecast and generated release must admit the original request");
        };
        let message = host
            .learned_text_message(LearnedEvidenceLimits::default())
            .unwrap();
        let source = message.evidence().audit().source();
        let rows: BTreeSet<_> = source.groups().map(|group| group.row).collect();
        let actor_revision = host
            .learned_generation_inspection()
            .unwrap()
            .numerical
            .actor_revision;
        let revision = host.revision();
        let sidecar = host
            .begin_learned_sidecar_plan(
                revision,
                attempt,
                actor_revision,
                LearnedSidecarRequest {
                    identity: SidecarIdentity {
                        object_id: 1000 + request,
                        generation: 1,
                        transform_id: 7,
                    },
                    priority: source.groups().collect(),
                    budget: SidecarCongressBudget::default(),
                },
            )
            .unwrap();
        let input = host.current_learned_sidecar(&sidecar).unwrap();
        let mut rounds = Vec::new();
        let mut rosters = BTreeMap::new();
        let mut successors = Vec::new();
        for (index, spelling) in spellings.iter().enumerate() {
            let members = input
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
                                spelling,
                            ),
                            queries,
                            salt: vec![16 + index as u8; 32],
                        },
                    )
                })
                .collect();
            if index > 0 && matches!(mode, Mode::Reserved) {
                successors.push(members);
            } else {
                let round = first_round + index as u64;
                rounds.push(LearnedWorkerRound {
                    round,
                    evidence_root: ordinary::document(1, legacy::snapshot(), &[]).reference_root(),
                    window: ReviewWindow {
                        commit_by: ElapsedTick(10 + index as u64 * 20),
                        reveal_by: ElapsedTick(15 + index as u64 * 20),
                    },
                });
                rosters.insert(round, members);
            }
        }
        (
            attempt,
            FileNativeDriverLaunch {
                journal_revision: host.revision(),
                request,
                sidecar,
                rounds,
                rosters,
                limits: NativeReviewLimits::default(),
            },
            successors,
        )
    };
    let run = driver
        .start_native_learned_sequence_with_convening(
            launch,
            legacy::snapshot(),
            ElapsedTick(1),
            successors,
        )
        .unwrap();
    (attempt, run)
}

impl Case {
    pub fn new(owned: bool, mode: Mode, finish: bool, crossing: bool, next: &[&[u8]]) -> Self {
        let root = legacy::Directory::new();
        let path = root.0.join("predictive-policy-evidence.json");
        ordinary::replace(&path, &ordinary::document(1, legacy::snapshot(), &[]));
        let mut source = FileEvidenceSource::new(
            &path,
            42,
            ordinary::profile().delivery.scope,
            MAX_EVIDENCE_FILE_BYTES,
        )
        .unwrap();
        let (mut host, reviewer, observer) =
            FileOversight::create_with_pre_output_forecast_with_reserve(
                root.store(),
                ordinary::profile(),
                recipe(owned, crossing, if finish { 2 } else { 1 }),
                RecoveryReserve::terminal(),
            )
            .unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1))
            .unwrap();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        host.advance_learned_generation(
            host.revision(),
            numerical.actor_revision,
            numerical.position,
        )
        .unwrap()
        .unwrap(); // actual prompt only
        let request = if finish { REQUEST - 1 } else { REQUEST };
        generate(&mut host, &observer, owned, request);
        let (actor, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let revision = supervisor.host().unwrap().revision();
        supervisor
            .set_snapshot(revision, Some(legacy::snapshot()))
            .unwrap();
        let ticket = actor
            .submit(request, LearnedTextRelease::Message, ElapsedTick(100))
            .unwrap();
        let driver = FileSupervisedDriver::new(supervisor);
        let spellings: Vec<&[u8]> = std::iter::once(b"hold".as_slice())
            .chain(next.iter().copied())
            .collect();
        if !finish {
            let (attempt, run) = launch(driver, REQUEST, HELD_ROUND, &spellings, mode);
            return Self {
                root,
                actor,
                ticket,
                reviewer,
                observer,
                run,
                source,
                path,
                attempt,
                request,
                owned,
                mode,
            };
        }
        // A real Message obtains both original keys and a receipt before Finish.
        let (attempt, run) = launch(driver, request, HELD_ROUND - 1, &[b"allow"], Mode::Fixed);
        let mut first = Self {
            root,
            actor,
            ticket,
            reviewer,
            observer,
            run,
            source,
            path,
            attempt,
            request,
            owned,
            mode: Mode::Fixed,
        };
        first.complete(Consequence::Continue, ElapsedTick(1));
        let human = first.approve(ElapsedTick(1));
        first.publish(&human, ElapsedTick(1));
        let Self {
            root,
            actor,
            reviewer,
            observer,
            run,
            source,
            path,
            ..
        } = first;
        let mut driver = run.into_handoff().unwrap().driver;
        {
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            let numerical = host.learned_generation_inspection().unwrap().numerical;
            forecast(&mut host, &observer, owned, REQUEST).unwrap();
            assert_eq!(
                host.learned_generation_inspection().unwrap().numerical,
                numerical
            );
        }
        let ticket = actor
            .submit(REQUEST, LearnedTextRelease::Finish, ElapsedTick(100))
            .unwrap();
        let (attempt, run) = launch(driver, REQUEST, HELD_ROUND, &spellings, mode);
        Self {
            root,
            actor,
            ticket,
            reviewer,
            observer,
            run,
            source,
            path,
            attempt,
            request: REQUEST,
            owned,
            mode,
        }
    }

    pub fn held(
        owned: bool,
        mode: Mode,
        finish: bool,
        crossing: bool,
        next: &[&[u8]],
    ) -> (Self, FileSupervisorConvenePort, FileSupervisorConveneGrant) {
        let mut case = Self::new(owned, mode, finish, crossing, next);
        let (port, grant) = case.run.supervisor().convene_port(32, 4).unwrap();
        case.complete(Consequence::HoldEffect, ElapsedTick(1));
        (case, port, grant)
    }

    pub fn image(&self) -> Vec<u8> {
        std::fs::read(self.root.store().join("delivery.bin")).unwrap()
    }

    pub fn complete(&mut self, expected: Consequence, now: ElapsedTick) {
        for _ in 0..MAX_LEARNED_REVIEW_POLLS {
            if self.run.review().status() != NativeReviewStatus::Running {
                break;
            }
            assert!(matches!(
                self.run
                    .step(|| now, || Ok(legacy::snapshot()), None)
                    .unwrap(),
                FileNativeDriverEvent::Review(_)
            ));
        }
        assert_eq!(self.run.review().status(), NativeReviewStatus::Finished);
        let FileLearnedSidecarFinish::Applied {
            receipt: Ok(receipt),
            ..
        } = self.run.review().history().last().unwrap()
        else {
            panic!("the original native model must apply its own review");
        };
        assert_eq!(receipt.policy.control.decision.consequence, expected);
        let record =
            self.run.review().records()[&self.run.review().round().round].members["reviewer"]
                .as_ref()
                .unwrap();
        assert_eq!(
            record.progress.basis,
            Some(SidecarDecisionBasis::NativeModel)
        );
        assert_eq!(record.progress.native.work.sampled_draws, 2);
    }

    pub fn convene(
        &mut self,
        ticket: FileConveneTicket,
        now: ElapsedTick,
    ) -> Result<(), fa_reference::action::consequence::delivery::persistent::JournalError> {
        let report = match self.mode {
            Mode::Fixed => self
                .run
                .convene_from_policy_file(&mut self.source, ticket, || now),
            Mode::Reserved => self.run.convene_reserved_from_policy_file(
                &mut self.source,
                ticket,
                ReviewWindow {
                    commit_by: ElapsedTick(30),
                    reveal_by: ElapsedTick(35),
                },
                || now,
            ),
        };
        report.result.map(|_| ())
    }

    pub fn tick(&mut self, human: Option<&FileHumanPermit>, now: ElapsedTick) -> FileDriverEvent {
        match self
            .run
            .step_from_policy_file(&mut self.source, || now, human)
            .result
            .unwrap()
        {
            FileNativeDriverEvent::Driver(event) => event,
            other => panic!("the completed review must yield its original driver: {other:?}"),
        }
    }

    pub fn approve(&mut self, now: ElapsedTick) -> FileHumanPermit {
        assert!(
            matches!(self.tick(None, now), FileDriverEvent::AwaitingHuman { request }
            if request == self.request)
        );
        let request = self
            .run
            .request_human_approval_from_policy_file(
                &mut self.source,
                9000 + self.request,
                ElapsedTick(40),
                || now,
            )
            .result
            .unwrap();
        let mut host = self.run.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        self.reviewer
            .approve(&mut host, revision, &request)
            .unwrap()
    }

    pub fn publish(&mut self, human: &FileHumanPermit, now: ElapsedTick) {
        assert!(
            matches!(self.tick(Some(human), now), FileDriverEvent::Dispatched { request, .. }
            if request == self.request)
        );
        assert!(
            matches!(self.tick(None, now), FileDriverEvent::PublicationChecked {
            publication, source_failure: None, ..
        } if matches!(publication.outcome, EndpointOutcome::Executed { .. }))
        );
        assert!(matches!(self.tick(None, now), FileDriverEvent::Reconciled {
            request, outcome: Reconciliation::Resolved(EndpointOutcome::Executed { .. }),
        } if request == self.request));
        assert!(matches!(
            self.actor.poll(&self.ticket),
            Knowledge::Known {
                value: ActorOutcome::Executed,
                ..
            }
        ));
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct PredictorProof {
    pub evidence: FileConsistencySnapshot,
    owned: Option<FileLearnedConsistencySnapshot>,
    raw_observation: Option<ConsistencyObservation>,
    owned_observation: Option<LearnedConsistencyObservation>,
}
impl PredictorProof {
    pub fn capture(case: &Case) -> Self {
        let host = case.run.supervisor().host().unwrap();
        // Journal progression is expected; the original predictor state is not.
        let mut evidence = host.action_consistency_snapshot().unwrap();
        evidence.journal_revision = 0;
        let mut owned = case
            .owned
            .then(|| host.learned_action_consistency_snapshot().unwrap());
        if let Some(owned) = &mut owned {
            owned.consistency.journal_revision = 0;
        }
        Self {
            evidence,
            owned,
            raw_observation: (!case.owned).then(|| {
                host.action_consistency_observation(case.attempt)
                    .unwrap()
                    .clone()
            }),
            owned_observation: case.owned.then(|| {
                host.learned_action_consistency_observation(case.attempt)
                    .unwrap()
                    .clone()
            }),
        }
    }
}
