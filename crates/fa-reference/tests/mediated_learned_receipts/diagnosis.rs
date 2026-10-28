//! Offline policy diagnosis of real source-only learned requests through every
//! original guarded cursor. Fixtures retain native guards and numerical work;
//! synthetic model weights and graphs are not detector/deployment evidence.
use super::*;
use fa_reference::action::consequence::delivery::credential_broker::{
    BrokerRouteBinding, CredentialRotationRequest,
};
use fa_reference::action::consequence::delivery::persistent::{
    FileDeliverySnapshot,
    governance::PolicyUpdate,
    observed::{
        credential::{FILE_OVERSIGHT_CREDENTIAL_PROFILE, FileCredentialPolicy},
        guarded::{
            FileCredentialEpoch, FileCredentialRegistration,
            investigation::FileProposalInvestigation,
            learned_recovery::FileGuardedLearnedRecovery,
            predictive::{
                FilePredictiveRequirements,
                learned::FilePredictiveLearnedRecovery,
                owned::{FileOwnedPredictiveRecovery, FileOwnedPredictiveRequirements},
            },
        },
    },
};
use fa_reference::action::consequence::experiment::{
    Intervention, InterventionScope, NextRequirement,
};
use fa_reference::action::consequence::gate::containment::session::policy::Truth;
use fa_reference::perimeter_inventory::LoadedPerimeterInventory;

const REQUEST: u64 = 700;

fn scope() -> InterventionScope {
    InterventionScope::new(43, true, false, false, &[]).unwrap()
}

#[derive(Debug)]
enum Cursor {
    Plain(FileGuardedLearnedRecovery),
    Raw(FilePredictiveLearnedRecovery),
    Owned(FileOwnedPredictiveRecovery),
    Mediated(FileMediatedLearnedRecovery),
    PredictiveMediated(FilePredictiveMediatedLearnedRecovery),
}

macro_rules! cursor {
    ($value:expr, $run:ident => $body:expr) => {
        match $value {
            Cursor::Plain($run) => $body,
            Cursor::Raw($run) => $body,
            Cursor::Owned($run) => $body,
            Cursor::Mediated($run) => $body,
            Cursor::PredictiveMediated($run) => $body,
        }
    };
}

impl Cursor {
    fn select(self, request: u64) -> Result<Self, JournalError> {
        match self {
            Self::Plain(run) => run
                .require_request_proposal_investigation(request, scope())
                .map(Self::Plain),
            Self::Raw(run) => run
                .require_request_proposal_investigation(request, scope())
                .map(Self::Raw),
            Self::Owned(run) => run
                .require_request_proposal_investigation(request, scope())
                .map(Self::Owned),
            Self::Mediated(run) => run
                .require_request_proposal_investigation(request, scope())
                .map(Self::Mediated),
            Self::PredictiveMediated(run) => run
                .require_request_proposal_investigation(request, scope())
                .map(Self::PredictiveMediated),
        }
    }
    fn progress(&self) -> FileLearnedRecoveryProgress {
        cursor!(self, run => run.progress())
    }
    fn advance(&mut self) -> Result<(), JournalError> {
        let completed = self.progress().replayed_events;
        cursor!(self, run => run.advance(completed, 1))?;
        Ok(())
    }
    fn ready(mut self) -> Result<Self, JournalError> {
        while self.progress().status == FileLearnedRecoveryStatus::Replaying {
            self.advance()?;
        }
        assert_eq!(self.progress().status, FileLearnedRecoveryStatus::Ready);
        Ok(self)
    }
    fn finish(self) -> Result<FileProposalInvestigation, JournalError> {
        cursor!(self, run => run.finish_proposal_investigation())
    }
    fn message(&self, request: u64) -> Result<(), JournalError> {
        cursor!(self, run => run.learned_text_stream_message_request(request))?;
        Ok(())
    }
    fn finish_request(&self, after: u64, request: u64) -> Result<(), JournalError> {
        cursor!(self, run => run.learned_text_stream_finish_request(after, request))?;
        Ok(())
    }
}

#[derive(Clone)]
struct Contract {
    oversight: FileRecoveryRequirements,
    topology: FileTopologyRequirement,
    prediction: Option<FileMediatedPredictor>,
    evaluation: Option<EvaluationProtocol>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Admission {
    Denied,
    Reviewing,
    NotAdmitted,
}

struct DeniedHistory {
    root: Directory,
    lane: Lane,
    mediated: bool,
    profile: FileOversightProfile,
    config: FileLearnedConfig,
    expected: Contract,
    anchor: FileHistoryAnchor,
    admission: FileDeliverySnapshot,
    journal: FileDeliverySnapshot,
    attempt: Option<u64>,
}

impl DeniedHistory {
    fn new(lane: Lane, mediated: bool, admission: Admission) -> Self {
        Self::build(lane, mediated, admission, false, false, false)
    }
    fn build(
        lane: Lane,
        mediated: bool,
        admission: Admission,
        credential: bool,
        changed_policy: bool,
        full: bool,
    ) -> Self {
        let root = Directory::new();
        let config = configuration(lane);
        let mut p = receipt_profile();
        if admission == Admission::Denied {
            p.delivery.policy = Policy::new(1, vec![Predicate::PayloadAtMost(1)]).unwrap();
        }
        if admission == Admission::NotAdmitted {
            p.delivery.policy = Policy::new(
                1,
                vec![Predicate::ExactValue {
                    key: 7,
                    value: b"ok".to_vec(),
                }],
            )
            .unwrap();
        }
        let mut g = guards(&config);
        let inventory = LoadedPerimeterInventory::from_json_bytes(
            source_inventory()
                .replace("reference-publication", FILE_OVERSIGHT_CREDENTIAL_PROFILE)
                .as_bytes(),
        )
        .unwrap();
        let binding = BrokerRouteBinding {
            family: "file-publication".into(),
            route: "publication".into(),
        };
        let registration = credential.then_some(FileCredentialRegistration {
            inventory: &inventory,
            binding: &binding,
        });
        if credential {
            g.credential = Some(
                FileCredentialPolicy::from_inventory(
                    &inventory,
                    &binding,
                    p.delivery.scope,
                    p.delivery.target,
                )
                .unwrap(),
            );
        }
        let evaluation = (mediated || lane != Lane::Plain).then(protocol);
        let (mut host, oversight, consistency, topology) = if mediated {
            let (host, roles) = match lane {
                Lane::Plain => FileOversight::create_mediated_guarded_with_learned_generation_reserved(
                    root.store(), p.clone(), &g, registration, graph(1, false), evaluation.clone(),
                    config.clone(), RecoveryReserve::terminal()),
                _ => FileOversight::create_predictive_mediated_guarded_with_learned_generation_reserved(
                    root.store(), p.clone(), &g, registration, graph(1, false), prediction(&config),
                    evaluation.clone(), config.clone(), RecoveryReserve::terminal()),
            }.unwrap();
            (
                host,
                roles.oversight,
                roles.consistency_observer,
                Some(roles.topology_observer),
            )
        } else {
            match lane {
                Lane::Plain => {
                    let (host, roles) =
                        FileOversight::create_guarded_with_learned_generation_reserved(
                            root.store(),
                            p.clone(),
                            &g,
                            registration,
                            config.clone(),
                            RecoveryReserve::terminal(),
                        )
                        .unwrap();
                    (host, roles, None, None)
                }
                Lane::Raw => {
                    let (host, roles) =
                        FileOversight::create_predictive_guarded_with_learned_generation_reserved(
                            root.store(),
                            p.clone(),
                            &g,
                            registration,
                            config.required_pre_output_forecast().unwrap().clone(),
                            evaluation.clone(),
                            config.clone(),
                            RecoveryReserve::terminal(),
                        )
                        .unwrap();
                    (
                        host,
                        roles.oversight,
                        Some(roles.consistency_observer),
                        None,
                    )
                }
                Lane::Owned => {
                    let (host, roles) = FileOversight::create_predictive_guarded_with_owned_learned_consistency_reserved(
                        root.store(), p.clone(), &g, registration, config.required_owned_pre_output_forecast().unwrap().clone(),
                        evaluation.clone(), config.clone(), RecoveryReserve::terminal()).unwrap();
                    (
                        host,
                        roles.oversight,
                        Some(roles.consistency_observer),
                        None,
                    )
                }
            }
        };
        if credential {
            host.rotate_credential_guard(
                host.revision(),
                CredentialRotationRequest {
                    operation: 901,
                    expected_generation: 1,
                    next_generation: 2,
                },
            )
            .unwrap();
        }
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        assert!(step(&mut host).sample().is_none());
        assert!(step(&mut host).sample().is_none());
        if let Some(role) = topology.as_ref() {
            assert!(matches!(
                certify(&mut host, role).unwrap(),
                CutCheck::Verified(_)
            ));
        }
        if let Some(observer) = consistency.as_ref() {
            let mode = if lane == Lane::Raw {
                Mode::Raw
            } else {
                Mode::Owned
            };
            mode.forecast(&mut host, observer, REQUEST)
                .unwrap()
                .unwrap();
        }
        let (samples, _) = finish_generation(&mut host);
        assert_eq!(samples.len(), 3);
        assert_eq!(
            host.learned_text_message(LearnedEvidenceLimits::default())
                .unwrap()
                .bytes(),
            b"OK"
        );
        fresh_identity(&mut host, &oversight, 1, 1);
        let (actor, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let revision = supervisor.host().unwrap().revision();
        let mut observation = snapshot();
        if admission == Admission::NotAdmitted {
            // A complete source-only observation fits the actor snapshot bound
            // but exceeds the ORIGINAL policy's retained witness bound. The
            // request is durably refused before any FrozenAction is created.
            observation
                .values
                .insert(7, vec![b'x'; fa_reference::action::MAX_WITNESS_BYTES + 1]);
        }
        supervisor
            .set_snapshot(revision, Some(observation))
            .unwrap();
        let ticket = actor
            .submit(REQUEST, LearnedTextRelease::Message, ElapsedTick(100))
            .unwrap();
        let host = supervisor.host().unwrap();
        let attempt = match host.request_status(REQUEST).unwrap().disposition {
            FileRequestDisposition::Admitted { attempt, stage } => {
                assert_ne!(attempt, REQUEST);
                assert_eq!(
                    stage,
                    if admission == Admission::Denied {
                        ActionState::Denied
                    } else {
                        ActionState::Reviewing
                    }
                );
                assert_ne!(admission, Admission::NotAdmitted);
                Some(attempt)
            }
            FileRequestDisposition::NotAdmitted(reason) => {
                assert_eq!(admission, Admission::NotAdmitted);
                assert_eq!(reason, Error::Limit);
                assert!(host.request_action(REQUEST).is_err());
                None
            }
        };
        assert!(host.review_replay(101).is_err());
        let admitted = host.inspect();
        let anchor = host.history_anchor().unwrap();
        drop(host);
        {
            let mut host = supervisor.host_mut().unwrap();
            if changed_policy {
                let control = host.inspect().control;
                let next = Policy::new(2, vec![Predicate::PayloadAtMost(4096)]).unwrap();
                let update =
                    PolicyUpdate::new(801, control.sequence, control.ledger.epoch, next).unwrap();
                let revision = host.revision();
                host.replace_policy(revision, &update).unwrap();
            }
            let revision = host.revision();
            host.observe_time(revision, ElapsedTick(2)).unwrap();
            if full {
                while host.journal_capacity().unwrap().ordinary_remaining().events > 0 {
                    let revision = host.revision();
                    host.observe_time(revision, ElapsedTick(2)).unwrap();
                }
            }
        }
        let revision = supervisor.host().unwrap().revision();
        actor
            .submit(REQUEST, LearnedTextRelease::Message, ElapsedTick(100))
            .unwrap();
        assert_eq!(supervisor.host().unwrap().revision(), revision);
        if admission == Admission::NotAdmitted {
            assert!(matches!(
                actor.poll(&ticket),
                Knowledge::Known {
                    value: ActorOutcome::NotAdmitted,
                    ..
                }
            ));
        }
        let host = supervisor.host().unwrap();
        let journal = host.inspect();
        let topology = if mediated {
            let actual = host.mediation_snapshot().unwrap();
            FileTopologyRequirement {
                initial: graph(1, false),
                current: actual.graph,
                available: actual.available,
            }
        } else {
            FileTopologyRequirement {
                initial: graph(1, false),
                current: graph(1, false),
                available: false,
            }
        };
        let expected = Contract {
            oversight: FileRecoveryRequirements {
                guards: g,
                effective_policy: host.current_policy().unwrap().clone(),
                credential_epoch: host
                    .credential_status()
                    .unwrap()
                    .map(|s| FileCredentialEpoch {
                        generation: s.generation,
                        revoked: s.revoked,
                    }),
                minimum: FileRecoveryFloor {
                    journal_revision: journal.revision,
                    control_sequence: journal.control.sequence,
                    authority_epoch: journal.control.ledger.epoch,
                },
            },
            topology,
            prediction: (lane != Lane::Plain).then(|| prediction(&config)),
            evaluation,
        };
        drop(host);
        drop(supervisor);
        drop(actor);
        drop(oversight);
        drop(consistency);
        std::fs::write(
            root.store().join("delivery.pending"),
            b"retain diagnosis staging",
        )
        .unwrap();
        Self {
            root,
            lane,
            mediated,
            profile: p,
            config,
            expected,
            anchor,
            admission: admitted,
            journal,
            attempt,
        }
    }

    fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        std::fs::read_dir(self.root.store())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.is_file())
            .map(|path| {
                let bytes = std::fs::read(&path).unwrap();
                (path, bytes)
            })
            .collect()
    }
    fn begin_with(
        &self,
        expected: &Contract,
        config: &FileLearnedConfig,
        anchor: &FileHistoryAnchor,
    ) -> Result<Cursor, JournalError> {
        let path = self.root.store();
        let p = self.profile.clone();
        match (self.mediated, self.lane) {
            (false, Lane::Plain) => {
                FileOversight::begin_open_guarded_anchored_with_learned_generation(
                    path,
                    p,
                    &expected.oversight,
                    config,
                    anchor,
                )
                .map(Cursor::Plain)
            }
            (false, Lane::Raw) => {
                let Some(FileMediatedPredictor::Raw(prediction)) = expected.prediction.as_ref()
                else {
                    panic!("raw fixture");
                };
                let expected = FilePredictiveRequirements {
                    oversight: expected.oversight.clone(),
                    prediction: prediction.clone(),
                    evaluation: expected.evaluation.clone(),
                };
                FileOversight::begin_open_predictive_guarded_anchored_with_learned_generation(
                    path, p, &expected, config, anchor,
                )
                .map(Cursor::Raw)
            }
            (false, Lane::Owned) => {
                let Some(FileMediatedPredictor::Owned(prediction)) = expected.prediction.as_ref()
                else {
                    panic!("owned fixture");
                };
                let expected = FileOwnedPredictiveRequirements {
                    oversight: expected.oversight.clone(),
                    prediction: prediction.clone(),
                    evaluation: expected.evaluation.clone(),
                };
                FileOversight::begin_open_predictive_guarded_anchored_with_owned_learned_consistency(
                    path, p, &expected, config, anchor).map(Cursor::Owned)
            }
            (true, Lane::Plain) => {
                let expected = FileMediatedRequirements {
                    oversight: expected.oversight.clone(),
                    topology: expected.topology.clone(),
                    prediction: None,
                    evaluation: expected.evaluation.clone(),
                };
                FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
                    path, p, &expected, config, anchor,
                )
                .map(Cursor::Mediated)
            }
            (true, _) => {
                let expected = FilePredictiveMediatedRequirements {
                    oversight: expected.oversight.clone(),
                    topology: expected.topology.clone(),
                    prediction: expected.prediction.clone().unwrap(),
                    evaluation: expected.evaluation.clone(),
                };
                FileOversight::begin_open_predictive_mediated_guarded_anchored_with_learned_generation(
                    path, p, &expected, config, anchor).map(Cursor::PredictiveMediated)
            }
        }
    }
    fn begin(&self) -> Result<Cursor, JournalError> {
        self.begin_with(&self.expected, &self.config, &self.anchor)
    }
    fn investigate(&self, request: u64) -> Result<FileProposalInvestigation, JournalError> {
        self.begin()?.select(request)?.ready()?.finish()
    }
}

#[test]
fn denied_learned_messages_retain_original_policy_admission_without_inventing_a_congress() {
    for mediated in [false, true] {
        for lane in LANES {
            let history = DeniedHistory::new(lane, mediated, Admission::Denied);
            let before = history.files();
            let investigation = history.investigate(REQUEST).unwrap();
            assert_eq!(investigation.purpose(), Purpose::Experiment);
            assert_eq!(investigation.origin().external_request(), Some(REQUEST));
            assert_eq!(
                investigation.origin().proposal().attempt,
                history.attempt.unwrap()
            );
            assert_eq!(investigation.origin().proposal().state, ActionState::Denied);
            assert_eq!(
                investigation.origin().admission_snapshot(),
                &history.admission
            );
            assert_eq!(investigation.origin().journal_snapshot(), &history.journal);
            assert!(history.journal.revision > history.admission.revision);
            let baseline = investigation.run(&[]).unwrap();
            assert_eq!(baseline.result().baseline(), Truth::Violated);
            assert_eq!(baseline.result().counterfactual(), Truth::Violated);
            let repaired = investigation
                .run(&[Intervention::Payload(b"x".to_vec())])
                .unwrap();
            assert_eq!(repaired.result().counterfactual(), Truth::Satisfied);
            assert_eq!(
                repaired.result().next_requirement(),
                NextRequirement::FreshIndependentReview
            );
            assert_eq!(repaired.origin(), investigation.origin());
            assert_eq!(repaired.result().initial_state(), ActionState::Denied);
            assert_eq!(history.files(), before);
            assert_eq!(
                history.investigate(REQUEST).unwrap().origin(),
                investigation.origin()
            );
            assert_eq!(history.files(), before);
        }
    }
}

#[test]
fn actual_not_admitted_requests_have_no_proposal_and_missing_ids_do_not_select_an_attempt() {
    for mediated in [false, true] {
        for lane in LANES {
            let history = DeniedHistory::new(lane, mediated, Admission::NotAdmitted);
            let before = history.files();
            assert_eq!(
                history.investigate(REQUEST).unwrap_err(),
                Error::Missing.into()
            );
            assert_eq!(history.investigate(1).unwrap_err(), Error::Missing.into());
            assert_eq!(
                history.investigate(REQUEST + 1).unwrap_err(),
                Error::Missing.into()
            );
            assert_eq!(history.files(), before);
        }
    }
}

#[test]
fn successful_admission_and_later_policy_replacement_preserve_the_original_cut() {
    for lane in LANES {
        let history = DeniedHistory::build(lane, true, Admission::Reviewing, false, true, false);
        let before = history.files();
        let investigation = history.investigate(REQUEST).unwrap();
        let attempt = history.attempt.unwrap();
        assert_eq!(
            investigation.origin().proposal().state,
            ActionState::Reviewing
        );
        assert_eq!(investigation.origin().proposal().policy.generation(), 1);
        assert_eq!(history.expected.oversight.effective_policy.generation(), 2);
        assert_eq!(
            investigation
                .origin()
                .journal_snapshot()
                .control
                .ledger
                .stages[&attempt],
            ActionState::Cancelled
        );
        assert_eq!(
            investigation.run(&[]).unwrap().result().counterfactual(),
            Truth::Satisfied
        );
        assert_eq!(history.files(), before);
    }
}

#[test]
fn real_confirmed_message_and_finish_have_distinct_historical_request_selections() {
    for lane in LANES {
        let history = History::new(lane, Cut::FinishPending, false);
        let before = history.files();
        let begin = || match history.begin() {
            Recovery::Plain(run) => Cursor::Mediated(run),
            Recovery::Predictive(run) => Cursor::PredictiveMediated(run),
        };
        let message = begin().select(71).unwrap().ready().unwrap();
        message.message(71).unwrap();
        assert!(message.finish_request(71, 71).is_err());
        let message = message.finish().unwrap();
        assert_eq!(message.origin().external_request(), Some(71));
        assert_eq!(message.origin().proposal().state, ActionState::Reviewing);
        let finish = begin().select(72).unwrap().ready().unwrap();
        finish.finish_request(71, 72).unwrap();
        assert!(finish.finish_request(70, 72).is_err());
        assert!(finish.message(72).is_err());
        let finish = finish.finish().unwrap();
        assert_eq!(finish.origin().external_request(), Some(72));
        assert_ne!(
            finish.origin().proposal().attempt,
            message.origin().proposal().attempt
        );
        assert!(
            finish.origin().admission_snapshot().revision
                > message.origin().admission_snapshot().revision
        );
        assert_eq!(finish.origin().proposal().state, ActionState::Reviewing);
        assert_eq!(
            finish.run(&[]).unwrap().result().counterfactual(),
            Truth::Satisfied
        );
        assert_eq!(history.files(), before);
    }
}

#[test]
fn registration_is_before_replay_once_only_and_partial_drop_has_no_storage_effect() {
    for mediated in [false, true] {
        for lane in LANES {
            let history = DeniedHistory::new(lane, mediated, Admission::Denied);
            let before = history.files();
            assert_eq!(
                history.begin().unwrap().select(0).unwrap_err(),
                Error::InvalidInput.into()
            );
            assert_eq!(
                history
                    .begin()
                    .unwrap()
                    .select(REQUEST)
                    .unwrap()
                    .select(REQUEST)
                    .unwrap_err(),
                Error::Duplicate.into()
            );
            assert_eq!(
                history
                    .begin()
                    .unwrap()
                    .select(REQUEST)
                    .unwrap()
                    .finish()
                    .unwrap_err(),
                Error::Incomplete.into()
            );
            assert_eq!(
                history
                    .begin()
                    .unwrap()
                    .ready()
                    .unwrap()
                    .finish()
                    .unwrap_err(),
                Error::Incomplete.into()
            );
            let mut partial = history.begin().unwrap();
            partial.advance().unwrap();
            assert_eq!(
                partial.select(REQUEST).unwrap_err(),
                Error::WrongState.into()
            );
            let mut partial = history.begin().unwrap().select(REQUEST).unwrap();
            partial.advance().unwrap();
            assert!(
                history.begin().is_err(),
                "original exclusive recovery lock is still held"
            );
            drop(partial);
            assert_eq!(history.files(), before);
            assert_eq!(
                history
                    .begin()
                    .unwrap()
                    .ready()
                    .unwrap()
                    .select(REQUEST)
                    .unwrap_err(),
                Error::WrongState.into()
            );
            history.investigate(REQUEST).unwrap();
            assert_eq!(history.files(), before);
        }
    }
}

fn refuses(history: &DeniedHistory, expected: &Contract) {
    let before = history.files();
    let result = history
        .begin_with(expected, &history.config, &history.anchor)
        .and_then(|run| run.select(REQUEST))
        .and_then(Cursor::ready)
        .and_then(Cursor::finish);
    assert!(result.is_err());
    assert_eq!(history.files(), before);
}

#[test]
fn every_independent_guard_floor_and_current_credential_epoch_is_enforced() {
    for mediated in [false, true] {
        for lane in LANES {
            let history =
                DeniedHistory::build(lane, mediated, Admission::Denied, true, false, false);
            assert_eq!(
                history.expected.oversight.credential_epoch,
                Some(FileCredentialEpoch {
                    generation: 2,
                    revoked: false
                })
            );
            let before = history.files();
            history.investigate(REQUEST).unwrap();
            for field in 0..12 {
                let mut wrong = history.expected.clone();
                match field {
                    0 => wrong.oversight.minimum.journal_revision += 1,
                    1 => wrong.oversight.minimum.control_sequence += 1,
                    2 => wrong.oversight.minimum.authority_epoch += 1,
                    3 => {
                        wrong.oversight.credential_epoch = Some(FileCredentialEpoch {
                            generation: 1,
                            revoked: false,
                        })
                    }
                    4 => {
                        wrong.oversight.credential_epoch = Some(FileCredentialEpoch {
                            generation: 2,
                            revoked: true,
                        })
                    }
                    5 => wrong.oversight.credential_epoch = None,
                    6 => wrong.oversight.guards.credential = None,
                    7 => wrong.oversight.guards.identity = None,
                    8 => wrong.oversight.guards.campaigns = None,
                    9 => wrong.oversight.guards.source = None,
                    10 => wrong.oversight.guards.stream = None,
                    _ => {
                        wrong.oversight.effective_policy =
                            Policy::new(2, vec![Predicate::PayloadAtMost(1)]).unwrap()
                    }
                }
                refuses(&history, &wrong);
            }
            if history.expected.evaluation.is_some() {
                let mut wrong = history.expected.clone();
                wrong.evaluation = None;
                refuses(&history, &wrong);
            }
            if mediated {
                let mut wrong = history.expected.clone();
                wrong.topology.available = !wrong.topology.available;
                refuses(&history, &wrong);
                let mut wrong = history.expected.clone();
                wrong.topology.initial = graph(2, false);
                refuses(&history, &wrong);
                let mut wrong = history.expected.clone();
                wrong.topology.current = graph(2, false);
                refuses(&history, &wrong);
            }
            if lane != Lane::Plain {
                let model = numerical::model(&[u32::from(b'O'), u32::from(b'K'), numerical::END]);
                let mode = if lane == Lane::Raw {
                    Mode::Raw
                } else {
                    Mode::Owned
                };
                let other = recipe(&model, source(&model), mode, false);
                let mut wrong = history.expected.clone();
                wrong.prediction = Some(prediction(&other));
                refuses(&history, &wrong);
            }
            assert_eq!(history.files(), before);
        }
    }
}

#[test]
fn recipe_prefix_and_entire_suffix_qualify_before_any_diagnosis_can_escape() {
    for mediated in [false, true] {
        for lane in LANES {
            let history = DeniedHistory::new(lane, mediated, Admission::Denied);
            let before = history.files();
            let other = DeniedHistory::new(lane, mediated, Admission::Denied);
            assert!(
                history
                    .begin_with(&history.expected, &history.config, &other.anchor)
                    .is_err()
            );
            let model = numerical::model(&[u32::from(b'N'), u32::from(b'O'), numerical::END]);
            let other_config = FileLearnedConfig::new_text_stream(
                model.clone(),
                numerical::tokenizer(&model),
                source(&model),
                LearnedDecoderBindingLimits::default(),
                stream_contract(),
            )
            .unwrap()
            .with_required_sidecar()
            .unwrap();
            let other_config = match lane {
                Lane::Plain => other_config,
                Lane::Raw => other_config
                    .with_required_pre_output_forecast(
                        history
                            .config
                            .required_pre_output_forecast()
                            .unwrap()
                            .clone(),
                    )
                    .unwrap(),
                Lane::Owned => other_config
                    .with_required_owned_pre_output_forecast(
                        history
                            .config
                            .required_owned_pre_output_forecast()
                            .unwrap()
                            .clone(),
                    )
                    .unwrap(),
            };
            assert!(
                history
                    .begin_with(&history.expected, &other_config, &history.anchor)
                    .is_err()
            );
            assert_eq!(history.files(), before);
            let path = history.root.store().join("delivery.bin");
            let canonical = history.root.bytes();
            let mut corrupt = canonical.clone();
            corrupt.push(0);
            std::fs::write(&path, &corrupt).unwrap();
            assert!(history.investigate(REQUEST).is_err());
            assert_eq!(history.root.bytes(), corrupt);
            std::fs::write(&path, &canonical).unwrap();
            // The independently retained admission prefix still matches. This
            // is a decodable ORIGINAL Time event after that cut whose rollback
            // must fail semantic replay before any captured admission escapes.
            assert_eq!(&canonical[canonical.len() - 8..], &2_u64.to_le_bytes());
            let mut invalid_suffix = canonical.clone();
            let end = invalid_suffix.len();
            invalid_suffix[end - 8..].copy_from_slice(&0_u64.to_le_bytes());
            std::fs::write(&path, &invalid_suffix).unwrap();
            let mut failed = history.begin().unwrap().select(REQUEST).unwrap();
            while failed.progress().status == FileLearnedRecoveryStatus::Replaying {
                if failed.advance().is_err() {
                    break;
                }
            }
            assert_eq!(
                failed.progress().status,
                FileLearnedRecoveryStatus::Failed(Error::Stale)
            );
            assert_eq!(failed.finish().unwrap_err(), Error::Stale.into());
            assert_eq!(history.root.bytes(), invalid_suffix);
            std::fs::write(&path, &canonical).unwrap();
            let run = history
                .begin()
                .unwrap()
                .select(REQUEST)
                .unwrap()
                .ready()
                .unwrap();
            let mut changed = canonical.clone();
            *changed.last_mut().unwrap() ^= 1;
            std::fs::write(&path, &changed).unwrap();
            assert_eq!(run.finish().unwrap_err(), Error::Binding.into());
            assert_eq!(history.root.bytes(), changed);
            std::fs::write(&path, &canonical).unwrap();
            history.investigate(REQUEST).unwrap();
            assert_eq!(history.files(), before);
        }
    }
}

#[test]
fn full_ordinary_capacity_and_unusable_staging_do_not_block_immutable_diagnosis() {
    let history = DeniedHistory::build(Lane::Plain, true, Admission::Denied, false, false, true);
    let before = history.files();
    let investigation = history.investigate(REQUEST).unwrap();
    assert_eq!(investigation.origin().journal_snapshot(), &history.journal);
    assert_eq!(history.files(), before);
    let staging = history.root.store().join("delivery.pending");
    std::fs::remove_file(&staging).unwrap();
    std::fs::create_dir(&staging).unwrap();
    let before = history.files();
    history.investigate(REQUEST).unwrap();
    assert!(staging.is_dir());
    assert_eq!(history.files(), before);
}
