//! Registered fleet terminal recovery of actual learned publication histories.
//! Original numerical generation, native helper review and both publication keys
//! create the effects. Synthetic weights/graphs are not deployment or calibration
//! evidence. Every assertion observes the original coordinator/domain histories.
use super::*;
use fa_reference::action::consequence::delivery::credential_broker::{
    BrokerRouteBinding, CredentialRotationRequest,
};
use fa_reference::action::consequence::delivery::persistent::{
    FileDeliverySnapshot, MAX_JOURNAL_BYTES, Reconciliation,
    observed::{
        consistency::learned::FileLearnedConsistencySnapshot,
        credential::{FILE_OVERSIGHT_CREDENTIAL_PROFILE, FileCredentialPolicy},
        guarded::{
            FileCredentialEpoch, FileCredentialRegistration,
            predictive::{FilePredictiveRequirements, owned::FileOwnedPredictiveRequirements},
        },
        shutdown::{
            FileShutdownDomain, FileShutdownMember, FileShutdownObservation, FileShutdownPlan,
            FileShutdownSource, MAX_SHUTDOWN_HEAD_BYTES,
            coordinator::{
                FileShutdownClockBinding, FileShutdownCoordinator, FileShutdownLearnedInputs,
                FileShutdownLearnedMember, FileShutdownLearnedPass,
                FileShutdownLearnedRequirements, FileShutdownPassResult, ShutdownVisitResult,
            },
            recovery::FileStopRecoveryStatus,
        },
    },
};
use fa_reference::perimeter_inventory::LoadedPerimeterInventory;

const OPERATION: u64 = 900;
const DOMAIN: u64 = 1;

struct FleetHistory {
    root: Directory,
    lane: Lane,
    profile: FileOversightProfile,
    config: FileLearnedConfig,
    inputs: FileShutdownLearnedInputs,
    domain: FileShutdownDomain,
    before: FileDeliverySnapshot,
    numerical: FileLearnedInspection,
    owned: Option<FileLearnedConsistencySnapshot>,
    attempt: u64,
    charge: u64,
}

impl FleetHistory {
    fn new(lane: Lane, mediated: bool, published: bool, full: bool, credential: bool) -> Self {
        if mediated {
            let history = History::build_configured(
                lane,
                if published { Cut::Published } else { Cut::Sent },
                full.then_some(0),
                true,
                credential,
                true,
            );
            let registration = history.early_topology.as_ref().unwrap().history_anchor();
            let domain = FileShutdownDomain::from_history_anchor(
                FileShutdownMember {
                    id: DOMAIN,
                    directory: std::fs::canonicalize(history.root.store()).unwrap(),
                    profile: history.profile.clone(),
                },
                registration,
            )
            .unwrap();
            assert!(domain.registered_revision() < history.anchor.revision());
            let requirements = if lane == Lane::Plain {
                FileShutdownLearnedRequirements::Mediated(history.expected.clone())
            } else {
                FileShutdownLearnedRequirements::PredictiveMediated(
                    FilePredictiveMediatedRequirements {
                        oversight: history.expected.oversight.clone(),
                        topology: history.expected.topology.clone(),
                        prediction: prediction(&history.config),
                        evaluation: history.expected.evaluation.clone(),
                    },
                )
            };
            let inputs = FileShutdownLearnedInputs {
                generation: history.config.clone(),
                requirements,
                reserve: RecoveryReserve::terminal(),
                topology_checkpoint: history.topology.clone(),
            };
            let before = if lane == Lane::Owned {
                FileOversight::read_publication_with_owned_learned_consistency(
                    history.root.store(),
                    &history.profile,
                    &history.config,
                    history.config.required_owned_pre_output_forecast().unwrap(),
                )
                .unwrap()
                .publication
            } else {
                FileOversight::read_publication_with_learned_generation(
                    history.root.store(),
                    &history.profile,
                    &history.config,
                )
                .unwrap()
            };
            let owned = if lane == Lane::Owned {
                Some(
                    FileOversight::read_publication_with_owned_learned_consistency(
                        history.root.store(),
                        &history.profile,
                        &history.config,
                        history.config.required_owned_pre_output_forecast().unwrap(),
                    )
                    .unwrap()
                    .consistency,
                )
            } else {
                None
            };
            // No live source refresh is available to fleet recovery.
            let parent = history.root.store().parent().unwrap().to_path_buf();
            std::fs::remove_file(parent.join("topology.json")).unwrap();
            std::fs::remove_file(parent.join("inventory.json")).unwrap();
            return Self {
                root: history.root,
                lane,
                profile: history.profile,
                config: history.config,
                inputs,
                domain,
                before,
                numerical: history.numerical,
                owned,
                attempt: history.attempt,
                charge: history.charge,
            };
        }

        let root = Directory::new();
        let profile = receipt_profile();
        let config = configuration(lane);
        let mut guards = guards(&config);
        let inventory = LoadedPerimeterInventory::from_json_bytes(
            source_inventory()
                .replace("reference-publication", FILE_OVERSIGHT_CREDENTIAL_PROFILE)
                .as_bytes(),
        )
        .unwrap();
        let route = BrokerRouteBinding {
            family: "file-publication".into(),
            route: "publication".into(),
        };
        let registration = credential.then_some(FileCredentialRegistration {
            inventory: &inventory,
            binding: &route,
        });
        if credential {
            guards.credential = Some(
                FileCredentialPolicy::from_inventory(
                    &inventory,
                    &route,
                    profile.delivery.scope,
                    profile.delivery.target,
                )
                .unwrap(),
            );
        }
        let evaluation = (lane != Lane::Plain).then(protocol);
        let (mut host, roles, observer) = match lane {
            Lane::Plain => {
                let (host, roles) = FileOversight::create_guarded_with_learned_generation_reserved(
                    root.store(),
                    profile.clone(),
                    &guards,
                    registration,
                    config.clone(),
                    RecoveryReserve::terminal(),
                )
                .unwrap();
                (host, roles, None)
            }
            Lane::Raw => {
                let (host, roles) =
                    FileOversight::create_predictive_guarded_with_learned_generation_reserved(
                        root.store(),
                        profile.clone(),
                        &guards,
                        registration,
                        config.required_pre_output_forecast().unwrap().clone(),
                        evaluation.clone(),
                        config.clone(),
                        RecoveryReserve::terminal(),
                    )
                    .unwrap();
                (host, roles.oversight, Some(roles.consistency_observer))
            }
            Lane::Owned => {
                let (host, roles) = FileOversight::create_predictive_guarded_with_owned_learned_consistency_reserved(
                    root.store(), profile.clone(), &guards, registration,
                    config.required_owned_pre_output_forecast().unwrap().clone(), evaluation.clone(),
                    config.clone(), RecoveryReserve::terminal(),
                ).unwrap();
                (host, roles.oversight, Some(roles.consistency_observer))
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
        let domain = host.shutdown_domain(DOMAIN).unwrap();
        let registered = FileShutdownDomain::from_history_anchor(
            domain.member(),
            &host.history_anchor().unwrap(),
        )
        .unwrap();
        assert_eq!(
            registered.registered_revision(),
            domain.registered_revision()
        );
        if lane != Lane::Plain {
            let mode = if lane == Lane::Raw {
                Mode::Raw
            } else {
                Mode::Owned
            };
            mode.forecast(&mut host, observer.as_ref().unwrap(), 71)
                .unwrap()
                .unwrap();
        }
        let (samples, last) = finish_generation(&mut host);
        assert_eq!(samples.len(), 3);
        assert_eq!(
            host.learned_text_message(LearnedEvidenceLimits::default())
                .unwrap()
                .bytes(),
            b"OK"
        );
        fresh_identity(&mut host, &roles, 1, 1);
        let (actor, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let revision = supervisor.host().unwrap().revision();
        supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
        let ticket = actor
            .submit(71, LearnedTextRelease::Message, ElapsedTick(100))
            .unwrap();
        let mut host = supervisor.host_mut().unwrap();
        let FileRequestDisposition::Admitted { attempt, .. } =
            host.request_status(71).unwrap().disposition
        else {
            panic!("original generated request must be admitted");
        };
        assert_ne!(
            attempt, 71,
            "the external request is not an internal attempt"
        );
        let (action, input, automatic, offer) = support::prepared(&mut host, 71, 101, &last);
        let revision = host.revision();
        let human = roles.human.approve(&mut host, revision, &offer).unwrap();
        let revision = host.revision();
        host.dispatch(revision, &automatic, &human, &action, &input, snapshot())
            .unwrap();
        let charge = action.spec().units;
        if published {
            let revision = host.revision();
            assert_eq!(
                host.publish_checked(revision, attempt, Some(&input), snapshot(), ElapsedTick(3))
                    .unwrap()
                    .outcome,
                EndpointOutcome::Executed {
                    resulting_version: 2
                }
            );
        }
        if full {
            while host.journal_capacity().unwrap().ordinary_remaining().events > 0 {
                let revision = host.revision();
                host.observe_time(revision, ElapsedTick(4)).unwrap();
            }
            assert_eq!(
                host.journal_capacity().unwrap().remaining().events,
                RecoveryReserve::terminal().events
            );
        }
        let before = host.inspect();
        let numerical = host.learned_generation_inspection().unwrap();
        let owned =
            (lane == Lane::Owned).then(|| host.learned_action_consistency_snapshot().unwrap());
        let oversight = FileRecoveryRequirements {
            guards,
            effective_policy: profile.delivery.policy.clone(),
            credential_epoch: host
                .credential_status()
                .unwrap()
                .map(|value| FileCredentialEpoch {
                    generation: value.generation,
                    revoked: value.revoked,
                }),
            minimum: FileRecoveryFloor {
                journal_revision: before.revision,
                control_sequence: before.control.sequence,
                authority_epoch: before.control.ledger.epoch,
            },
        };
        let requirements = match lane {
            Lane::Plain => FileShutdownLearnedRequirements::Plain(oversight),
            Lane::Raw => FileShutdownLearnedRequirements::Raw(FilePredictiveRequirements {
                oversight,
                prediction: config.required_pre_output_forecast().unwrap().clone(),
                evaluation,
            }),
            Lane::Owned => {
                FileShutdownLearnedRequirements::Owned(FileOwnedPredictiveRequirements {
                    oversight,
                    prediction: config.required_owned_pre_output_forecast().unwrap().clone(),
                    evaluation,
                })
            }
        };
        let inputs = FileShutdownLearnedInputs {
            generation: config.clone(),
            requirements,
            reserve: RecoveryReserve::terminal(),
            topology_checkpoint: None,
        };
        drop(host);
        assert!(matches!(actor.poll(&ticket), Knowledge::Unknown { .. }));
        drop(supervisor);
        drop(actor);
        let result = Self {
            root,
            lane,
            profile,
            config,
            inputs,
            domain: registered,
            before,
            numerical,
            owned,
            attempt,
            charge,
        };
        result.pending();
        result
    }

    fn plan(&self) -> FileShutdownPlan {
        FileShutdownPlan::new(
            OPERATION,
            vec![self.domain.clone()],
            64,
            MAX_SHUTDOWN_HEAD_BYTES,
        )
        .unwrap()
    }
    fn binding(&self) -> FileShutdownClockBinding {
        FileShutdownClockBinding {
            domain: DOMAIN,
            clock_domain: self.profile.delivery.clock_domain,
        }
    }
    fn member(&self) -> FileShutdownLearnedMember {
        FileShutdownLearnedMember {
            domain: DOMAIN,
            inputs: Some(self.inputs.clone()),
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
    fn pending(&self) {
        std::fs::write(
            self.root.store().join("delivery.pending"),
            b"preserve fleet staging",
        )
        .unwrap();
    }
    fn observe(&self) -> FileDeliverySnapshot {
        if let Some(original) = &self.owned {
            let observed = FileOversight::read_publication_with_owned_learned_consistency(
                self.root.store(),
                &self.profile,
                &self.config,
                self.config.required_owned_pre_output_forecast().unwrap(),
            )
            .unwrap();
            assert_eq!(observed.consistency.work, original.work);
            assert_eq!(
                observed.consistency.retained_source_bytes,
                original.retained_source_bytes
            );
            assert_eq!(
                observed.consistency.consistency.evidence,
                original.consistency.evidence
            );
            observed.publication
        } else {
            FileOversight::read_publication_with_learned_generation(
                self.root.store(),
                &self.profile,
                &self.config,
            )
            .unwrap()
        }
    }
    fn assert_original_outcome(
        &self,
        result: &FileShutdownObservation,
        published: bool,
        expired: bool,
    ) {
        let stopped = result.stop.as_ref().unwrap();
        assert_eq!(stopped.receipt.request().operation, OPERATION);
        assert_eq!(stopped.reserved_units, 0);
        assert_eq!(
            stopped.charged_units,
            if published || expired { self.charge } else { 0 },
            "{:?}: original liabilities remain exact",
            self.lane
        );
        assert_eq!(stopped.drained(), !expired);
        assert_eq!(result.executions, u64::from(published));
        assert_eq!(result.dispatches_since_registration, vec![self.attempt]);
        let disk = self.observe();
        assert_eq!(disk.revision, result.journal_revision);
        assert_eq!(disk.control.ledger.charged, stopped.charged_units);
        assert_eq!(disk.executions, result.executions);
        assert_eq!(disk.stop.unwrap().request(), stopped.receipt.request());
        if expired {
            assert_eq!(stopped.unresolved, vec![self.attempt]);
            assert_eq!(
                disk.control.ledger.stages[&self.attempt],
                ActionState::Unknown
            );
        }
    }
}

fn oversight(inputs: &mut FileShutdownLearnedInputs) -> &mut FileRecoveryRequirements {
    match &mut inputs.requirements {
        FileShutdownLearnedRequirements::Plain(value) => value,
        FileShutdownLearnedRequirements::Raw(value) => &mut value.oversight,
        FileShutdownLearnedRequirements::Owned(value) => &mut value.oversight,
        FileShutdownLearnedRequirements::Mediated(value) => &mut value.oversight,
        FileShutdownLearnedRequirements::PredictiveMediated(value) => &mut value.oversight,
    }
}

fn same_native_observation(current: &FileShutdownObservation, original: &FileShutdownObservation) {
    let mut expected = original.clone();
    expected.source = FileShutdownSource::CanonicalImage;
    assert_eq!(current, &expected);
}

#[test]
fn learned_fleet_combined_and_passive_resume_preserve_the_original_drain_in_all_modes() {
    for lane in LANES {
        for mediated in [false, true] {
            let history = FleetHistory::new(lane, mediated, false, true, false);
            assert_eq!(history.numerical.numerical.sampled_draws, 3);
            let control = Directory::new();
            let plan = history.plan();
            let before = history.files();
            let mut coordinator =
                FileShutdownCoordinator::create(control.store(), &plan, MAX_JOURNAL_BYTES).unwrap();
            let calls = Cell::new(0);
            let mut polls = Vec::new();
            let observed = coordinator
                .recover_learned_and_drain_with_clock(
                    0,
                    history.binding(),
                    &history.inputs,
                    1,
                    |actual| {
                        assert_eq!(actual, history.binding());
                        calls.set(calls.get() + 1);
                        Ok(ElapsedTick(5))
                    },
                    |progress| {
                        assert_eq!(history.files(), before, "replay must not clean or fence");
                        assert_eq!(progress.attempted_events, progress.replayed_events);
                        polls.push(progress.replayed_events);
                        true
                    },
                )
                .unwrap()
                .unwrap();
            assert_eq!(calls.get(), 1);
            assert_eq!(
                polls,
                (0..=history.before.revision as usize).collect::<Vec<_>>()
            );
            assert_eq!(observed.journal_revision, history.before.revision + 3);
            assert_eq!(
                observed.last_drain.as_ref().unwrap().journal_revision,
                observed.journal_revision
            );
            history.assert_original_outcome(&observed, false, false);
            assert!(coordinator.report().all_observed_drained());
            let revision = coordinator.revision();
            let coordinator_bytes = control.bytes();
            drop(coordinator);

            // Original saved input custody is reused unchanged. The mediated
            // terminal checkpoint proves the restrictive successor itself.
            history.pending();
            let exact = history.files();
            let mut reopened =
                FileShutdownCoordinator::open(control.store(), &plan, MAX_JOURNAL_BYTES, revision)
                    .unwrap();
            assert!(!reopened.report().all_observed_drained());
            let resumed = reopened
                .resolve_learned_shutdown_visit(revision, 0, &history.inputs)
                .unwrap();
            same_native_observation(&resumed, &observed);
            assert_eq!(
                history.files(),
                exact,
                "passive resolution must not clean staging"
            );
            assert_eq!(control.bytes(), coordinator_bytes);
            assert_eq!(reopened.revision(), revision);
            assert!(reopened.report().all_observed_drained());

            let pass = reopened
                .run_registered_learned_pass(
                    revision,
                    &[history.binding()],
                    &[history.member()],
                    FileShutdownLearnedPass::Resume,
                    1,
                    |_| panic!("a matching retained drain must not acquire time"),
                    |_| true,
                )
                .unwrap();
            assert!(pass.all_observed_drained());
            assert_eq!(pass.members()[0].acquired_tick, None);
            let FileShutdownPassResult::Observed(again) = &pass.members()[0].result else {
                panic!("same acknowledged cut must resolve");
            };
            same_native_observation(again, &observed);
            assert_eq!(history.files(), exact);
            assert_eq!(control.bytes(), coordinator_bytes);
        }
    }
}

#[test]
fn learned_fleet_split_stop_and_drain_preserve_original_source_custody_and_retry_work() {
    for lane in LANES {
        let history = FleetHistory::new(lane, true, false, true, false);
        let control = Directory::new();
        let plan = history.plan();
        let mut coordinator =
            FileShutdownCoordinator::create(control.store(), &plan, MAX_JOURNAL_BYTES).unwrap();
        let admission = coordinator
            .recover_learned_admission_stop(0, history.binding(), &history.inputs, 1, |_| true)
            .unwrap()
            .unwrap();
        assert_eq!(admission.journal_revision, history.before.revision + 2);
        assert!(
            admission.last_drain.is_none(),
            "admission cannot invent a sweep"
        );
        assert_eq!(
            admission.stop.as_ref().unwrap().charged_units,
            history.charge
        );
        assert!(!admission.stop.as_ref().unwrap().drained());
        let stopped_bytes = history.root.bytes();
        let revision = coordinator.revision();
        drop(coordinator);
        let mut coordinator =
            FileShutdownCoordinator::open(control.store(), &plan, MAX_JOURNAL_BYTES, revision)
                .unwrap();
        let resume = coordinator
            .run_registered_learned_pass(
                revision,
                &[history.binding()],
                &[history.member()],
                FileShutdownLearnedPass::ResumeAdmission,
                1,
                |_| panic!("passive admission resolution has no time acquisition"),
                |_| true,
            )
            .unwrap();
        assert!(resume.all_observed_stopped());
        assert!(!resume.all_observed_drained());
        assert_eq!(history.root.bytes(), stopped_bytes);

        history.pending();
        let before_refusal = history.files();
        assert_eq!(
            coordinator
                .drain_learned_stopped_with_clock(
                    coordinator.revision(),
                    history.binding(),
                    &history.inputs,
                    1,
                    |_| Err(JournalError::Unavailable),
                    |_| true,
                )
                .unwrap(),
            Err(JournalError::Unavailable)
        );
        assert_eq!(history.files(), before_refusal);
        assert!(!coordinator.report().all_observed_drained());
        let calls = Cell::new(0);
        let drained = coordinator
            .drain_learned_stopped_with_clock(
                coordinator.revision(),
                history.binding(),
                &history.inputs,
                1,
                |_| {
                    calls.set(calls.get() + 1);
                    Ok(ElapsedTick(5))
                },
                |_| true,
            )
            .unwrap()
            .unwrap();
        assert_eq!(calls.get(), 1);
        assert_eq!(drained.journal_revision, admission.journal_revision + 1);
        history.assert_original_outcome(&drained, false, false);
        let last = drained.last_drain.as_ref().unwrap().clone();
        let canonical = history.root.bytes();
        let repeated = coordinator
            .drain_learned_stopped_with_clock(
                coordinator.revision(),
                history.binding(),
                &history.inputs,
                1,
                |_| panic!("completed drain cannot repeat work or read time"),
                |_| true,
            )
            .unwrap()
            .unwrap();
        assert_eq!(repeated.last_drain.as_ref(), Some(&last));
        assert_eq!(repeated.journal_revision, drained.journal_revision);
        assert_eq!(history.root.bytes(), canonical);
        let visit = coordinator.report().visits.len() - 1;
        let revision = coordinator.revision();
        drop(coordinator);
        let mut reopened =
            FileShutdownCoordinator::open(control.store(), &plan, MAX_JOURNAL_BYTES, revision)
                .unwrap();
        let resolved = reopened
            .resolve_learned_shutdown_visit(revision, visit, &history.inputs)
            .unwrap();
        same_native_observation(&resolved, &drained);
        assert_eq!(resolved.last_drain, Some(last));
        assert_eq!(history.root.bytes(), canonical);
    }
}

#[test]
fn learned_fleet_preserves_executed_and_unknown_liabilities_in_its_saved_sweep() {
    for lane in LANES {
        for published in [false, true] {
            let history = FleetHistory::new(lane, true, published, false, false);
            let control = Directory::new();
            let plan = history.plan();
            let mut coordinator =
                FileShutdownCoordinator::create(control.store(), &plan, MAX_JOURNAL_BYTES).unwrap();
            let expired = !published;
            let observed = coordinator
                .recover_learned_and_drain_with_clock(
                    0,
                    history.binding(),
                    &history.inputs,
                    3,
                    |_| Ok(ElapsedTick(if expired { 2000 } else { 5 })),
                    |_| true,
                )
                .unwrap()
                .unwrap();
            history.assert_original_outcome(&observed, published, expired);
            let drain = observed.last_drain.as_ref().unwrap();
            if expired {
                assert_eq!(
                    drain.sweep.outcomes[&history.attempt],
                    Ok(Reconciliation::RetentionExpired)
                );
                assert!(!coordinator.report().all_observed_drained());
            } else {
                assert!(coordinator.report().all_observed_drained());
            }
            let revision = coordinator.revision();
            let domain_bytes = history.root.bytes();
            let coordinator_bytes = control.bytes();
            drop(coordinator);
            let mut reopened =
                FileShutdownCoordinator::open(control.store(), &plan, MAX_JOURNAL_BYTES, revision)
                    .unwrap();
            let resumed = reopened
                .resolve_learned_shutdown_visit(revision, 0, &history.inputs)
                .unwrap();
            same_native_observation(&resumed, &observed);
            assert_eq!(resumed.last_drain.as_ref(), Some(drain));
            assert_eq!(history.root.bytes(), domain_bytes);
            assert_eq!(control.bytes(), coordinator_bytes);
            assert_eq!(reopened.report().all_observed_drained(), published);
        }
    }
}

#[test]
fn learned_fleet_keeps_independent_recipe_predictor_guards_floors_and_source_binding() {
    let history = FleetHistory::new(Lane::Owned, true, false, false, true);
    let control = Directory::new();
    let plan = history.plan();
    let mut coordinator =
        FileShutdownCoordinator::create(control.store(), &plan, MAX_JOURNAL_BYTES).unwrap();
    let before = history.files();
    for case in 0..11 {
        let mut wrong = history.inputs.clone();
        match case {
            0 => wrong.generation = configuration(Lane::Plain),
            1 => {
                let FileShutdownLearnedRequirements::PredictiveMediated(value) =
                    &mut wrong.requirements
                else {
                    unreachable!()
                };
                value.prediction = support::changed_owned(&history.config);
            }
            2 => oversight(&mut wrong).guards.identity = None,
            3 => {
                oversight(&mut wrong).credential_epoch = Some(FileCredentialEpoch {
                    generation: 1,
                    revoked: false,
                })
            }
            4 => oversight(&mut wrong).minimum.journal_revision += 1,
            5 => oversight(&mut wrong).minimum.control_sequence += 1,
            6 => oversight(&mut wrong).minimum.authority_epoch += 1,
            7 => wrong.topology_checkpoint = None,
            8 => {
                let FileShutdownLearnedRequirements::PredictiveMediated(value) =
                    &mut wrong.requirements
                else {
                    unreachable!()
                };
                value.evaluation = None;
            }
            9 => wrong.reserve.events += 1,
            _ => {
                oversight(&mut wrong).effective_policy =
                    Policy::new(2, vec![Predicate::PayloadAtMost(4096)]).unwrap()
            }
        }
        let result = coordinator.recover_learned_and_drain_with_clock(
            coordinator.revision(),
            history.binding(),
            &wrong,
            1,
            |_| panic!("independent binding refusal must precede time"),
            |_| true,
        );
        assert!(
            result.is_err() || result.as_ref().is_ok_and(|result| result.is_err()),
            "case {case} must refuse"
        );
        assert_eq!(
            history.files(),
            before,
            "case {case} changed the original domain"
        );
        assert!(!coordinator.report().all_observed_stopped());
    }
    // A well-formed checkpoint from another actual history cannot become the
    // independently selected source/history pair for this registration.
    let foreign = FleetHistory::new(Lane::Owned, true, false, false, false);
    let mut wrong = history.inputs.clone();
    wrong.topology_checkpoint = foreign.inputs.topology_checkpoint;
    let result = coordinator.recover_learned_and_drain_with_clock(
        coordinator.revision(),
        history.binding(),
        &wrong,
        1,
        |_| panic!("foreign source custody must precede time"),
        |_| true,
    );
    assert!(result.is_err() || result.as_ref().is_ok_and(|result| result.is_err()));
    assert_eq!(history.files(), before);
    let observed = coordinator
        .recover_learned_and_drain_with_clock(
            coordinator.revision(),
            history.binding(),
            &history.inputs,
            1,
            |_| Ok(ElapsedTick(5)),
            |_| true,
        )
        .unwrap()
        .unwrap();
    history.assert_original_outcome(&observed, false, false);
}

#[test]
fn learned_fleet_admits_only_registered_members_bound_clocks_and_the_original_stop_operation() {
    let history = FleetHistory::new(Lane::Plain, false, false, false, false);
    let control = Directory::new();
    let plan = history.plan();
    let mut coordinator =
        FileShutdownCoordinator::create(control.store(), &plan, MAX_JOURNAL_BYTES).unwrap();
    let domain_before = history.files();
    let coordinator_before = control.bytes();
    for (binding, quantum) in [
        (
            FileShutdownClockBinding {
                domain: 9,
                ..history.binding()
            },
            1,
        ),
        (
            FileShutdownClockBinding {
                clock_domain: 9000,
                ..history.binding()
            },
            1,
        ),
        (history.binding(), 0),
    ] {
        assert!(
            coordinator
                .recover_learned_and_drain_with_clock(
                    0,
                    binding,
                    &history.inputs,
                    quantum,
                    |_| panic!("invalid member inputs must precede clock"),
                    |_| panic!("invalid member inputs must precede replay"),
                )
                .is_err()
        );
        assert_eq!(coordinator.revision(), 0);
        assert_eq!(control.bytes(), coordinator_before);
        assert_eq!(history.files(), domain_before);
    }
    for members in [Vec::new(), vec![history.member(), history.member()]] {
        assert!(
            coordinator
                .run_registered_learned_pass(
                    0,
                    &[history.binding()],
                    &members,
                    FileShutdownLearnedPass::Recover,
                    1,
                    |_| panic!("incomplete or duplicate roster cannot read time"),
                    |_| panic!("incomplete or duplicate roster cannot start work"),
                )
                .is_err()
        );
        assert_eq!(control.bytes(), coordinator_before);
        assert_eq!(history.files(), domain_before);
    }

    // An independently acknowledged stop by another operation cannot be taken
    // over merely because this fleet registered the same earlier domain.
    let mut run = FileOversight::begin_open_with_learned_generation(
        history.root.store(),
        history.profile.clone(),
        &history.config,
    )
    .unwrap();
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 1).unwrap();
    }
    let request = run.stop_request(OPERATION + 1).unwrap();
    let stopped = run.finish_admission_stop(request).unwrap();
    assert_eq!(stopped.progress().receipt.request(), request);
    history.pending();
    let wrong_operation = history.files();
    assert!(
        coordinator
            .recover_learned_and_drain_with_clock(
                coordinator.revision(),
                history.binding(),
                &history.inputs,
                1,
                |_| panic!("foreign operation cannot acquire time"),
                |_| true,
            )
            .unwrap()
            .is_err()
    );
    assert_eq!(history.files(), wrong_operation);
    assert!(!coordinator.report().all_observed_stopped());

    // Near-identical positive: the registered operation still owns its own
    // independent history, and a valid explicit call does stop and drain it.
    let positive = FleetHistory::new(Lane::Plain, false, false, false, false);
    let control = Directory::new();
    let mut coordinator =
        FileShutdownCoordinator::create(control.store(), &positive.plan(), MAX_JOURNAL_BYTES)
            .unwrap();
    let result = coordinator
        .recover_learned_and_drain_with_clock(
            0,
            positive.binding(),
            &positive.inputs,
            1,
            |_| Ok(ElapsedTick(5)),
            |_| true,
        )
        .unwrap()
        .unwrap();
    positive.assert_original_outcome(&result, false, false);
}

#[test]
fn learned_fleet_replay_cancellation_and_canonical_races_cannot_report_a_stop() {
    let history = FleetHistory::new(Lane::Raw, false, false, false, false);
    let control = Directory::new();
    let plan = history.plan();
    let mut coordinator =
        FileShutdownCoordinator::create(control.store(), &plan, MAX_JOURNAL_BYTES).unwrap();
    let before = history.files();
    for cut in [0, 1, history.before.revision as usize] {
        assert_eq!(
            coordinator
                .recover_learned_and_drain_with_clock(
                    coordinator.revision(),
                    history.binding(),
                    &history.inputs,
                    1,
                    |_| panic!("cancelled replay cannot acquire time"),
                    |progress| progress.replayed_events < cut,
                )
                .unwrap(),
            Err(JournalError::Unavailable)
        );
        assert_eq!(history.files(), before);
        assert!(!coordinator.report().all_observed_stopped());
        assert!(matches!(
            coordinator.report().visits.last().unwrap().result,
            ShutdownVisitResult::Refused { .. }
        ));
    }

    let canonical = history.root.bytes();
    let mut changed = canonical.clone();
    *changed.last_mut().unwrap() ^= 1;
    let path = history.root.store().join("delivery.bin");
    for during_clock in [false, true] {
        let calls = Cell::new(0);
        let result = coordinator
            .recover_learned_and_drain_with_clock(
                coordinator.revision(),
                history.binding(),
                &history.inputs,
                1,
                |_| {
                    calls.set(calls.get() + 1);
                    if during_clock {
                        std::fs::write(&path, &changed).unwrap();
                    }
                    Ok(ElapsedTick(5))
                },
                |progress| {
                    if progress.status == FileStopRecoveryStatus::Ready && !during_clock {
                        std::fs::write(&path, &changed).unwrap();
                    }
                    true
                },
            )
            .unwrap();
        assert_eq!(result, Err(Error::Binding.into()));
        assert_eq!(calls.get(), usize::from(during_clock));
        assert_eq!(history.root.bytes(), changed);
        assert_eq!(
            std::fs::read(history.root.store().join("delivery.pending")).unwrap(),
            b"preserve fleet staging"
        );
        assert!(!coordinator.report().all_observed_stopped());
        std::fs::write(&path, &canonical).unwrap();
        assert_eq!(history.files(), before);
    }
    let observed = coordinator
        .recover_learned_and_drain_with_clock(
            coordinator.revision(),
            history.binding(),
            &history.inputs,
            1,
            |_| Ok(ElapsedTick(5)),
            |_| true,
        )
        .unwrap()
        .unwrap();
    history.assert_original_outcome(&observed, false, false);
}

#[test]
fn learned_fleet_keeps_unreachable_members_and_reaches_an_independent_ordinary_control() {
    for fault in 0..3 {
        let history = FleetHistory::new(Lane::Plain, false, false, false, false);
        let ordinary_root = Directory::new();
        let mut ordinary_profile = receipt_profile();
        ordinary_profile.delivery.scope.branch += 1;
        ordinary_profile.delivery.clock_domain = 100;
        let (mut ordinary, _) =
            FileOversight::create(ordinary_root.store(), ordinary_profile.clone()).unwrap();
        ordinary
            .observe_time(ordinary.revision(), ElapsedTick(1))
            .unwrap();
        let domain = ordinary.shutdown_domain(2).unwrap();
        drop(ordinary);
        let control = Directory::new();
        let plan = FileShutdownPlan::new(
            OPERATION,
            vec![history.domain.clone(), domain],
            64,
            MAX_SHUTDOWN_HEAD_BYTES,
        )
        .unwrap();
        let mut coordinator =
            FileShutdownCoordinator::create(control.store(), &plan, MAX_JOURNAL_BYTES).unwrap();
        let bindings = [
            history.binding(),
            FileShutdownClockBinding {
                domain: 2,
                clock_domain: 100,
            },
        ];
        let members = [
            history.member(),
            FileShutdownLearnedMember {
                domain: 2,
                inputs: None,
            },
        ];
        let canonical = history.root.bytes();
        let hidden = history
            .root
            .store()
            .parent()
            .unwrap()
            .join("offline-publication");
        let mut held = None;
        match fault {
            0 => std::fs::rename(history.root.store(), &hidden).unwrap(),
            1 => {
                let FileShutdownLearnedRequirements::Plain(expected) = &history.inputs.requirements
                else {
                    unreachable!()
                };
                held = Some(
                    FileOversight::begin_open_guarded_with_learned_generation(
                        history.root.store(),
                        history.profile.clone(),
                        expected,
                        &history.config,
                    )
                    .unwrap(),
                );
            }
            _ => {
                let mut malformed = canonical.clone();
                malformed.push(0);
                std::fs::write(history.root.store().join("delivery.bin"), malformed).unwrap();
            }
        }
        let mut clocked = Vec::new();
        let pass = coordinator
            .run_registered_learned_pass(
                0,
                &bindings,
                &members,
                FileShutdownLearnedPass::Recover,
                1,
                |binding| {
                    clocked.push(binding.domain);
                    Ok(ElapsedTick(5))
                },
                |_| true,
            )
            .unwrap();
        assert_eq!(pass.members().len(), 2);
        assert!(matches!(
            pass.members()[0].result,
            FileShutdownPassResult::Refused(_)
        ));
        assert!(matches!(
            pass.members()[1].result,
            FileShutdownPassResult::Observed(_)
        ));
        assert_eq!(pass.unobserved_stops(), vec![DOMAIN]);
        assert!(!pass.all_observed_drained());
        assert_eq!(clocked, vec![2]);
        let ordinary_bytes = ordinary_root.bytes();
        assert_eq!(
            FileOversight::read_publication(ordinary_root.store(), &ordinary_profile)
                .unwrap()
                .revision,
            4
        );
        drop(held);
        match fault {
            0 => std::fs::rename(&hidden, history.root.store()).unwrap(),
            2 => std::fs::write(history.root.store().join("delivery.bin"), &canonical).unwrap(),
            _ => {}
        }
        assert_eq!(history.root.bytes(), canonical);
        clocked.clear();
        let recovered = coordinator
            .run_registered_learned_pass(
                coordinator.revision(),
                &bindings,
                &members,
                FileShutdownLearnedPass::RecoverSelected(&[DOMAIN]),
                1,
                |binding| {
                    clocked.push(binding.domain);
                    Ok(ElapsedTick(5))
                },
                |_| true,
            )
            .unwrap();
        assert!(recovered.all_observed_drained());
        assert_eq!(clocked, vec![DOMAIN]);
        assert_eq!(
            ordinary_root.bytes(),
            ordinary_bytes,
            "the unselected success is only resolved"
        );
        let FileShutdownPassResult::Observed(observed) = &recovered.members()[0].result else {
            panic!("the explicitly selected repaired member must now progress");
        };
        history.assert_original_outcome(observed, false, false);
    }
}

#[test]
fn learned_fleet_passive_resolution_cancels_without_custody_changes_and_rejects_rollback() {
    let history = FleetHistory::new(Lane::Owned, true, true, true, false);
    let control = Directory::new();
    let plan = history.plan();
    let original = history.root.bytes();
    let mut coordinator =
        FileShutdownCoordinator::create(control.store(), &plan, MAX_JOURNAL_BYTES).unwrap();
    let observed = coordinator
        .recover_learned_and_drain_with_clock(
            0,
            history.binding(),
            &history.inputs,
            1,
            |_| Ok(ElapsedTick(5)),
            |_| true,
        )
        .unwrap()
        .unwrap();
    let revision = coordinator.revision();
    let acknowledged = history.root.bytes();
    let coordinator_bytes = control.bytes();
    drop(coordinator);
    history.pending();
    let before = history.files();
    let mut coordinator =
        FileShutdownCoordinator::open(control.store(), &plan, MAX_JOURNAL_BYTES, revision).unwrap();
    {
        let mut read = coordinator
            .begin_resolve_learned_shutdown_visit(revision, 0, &history.inputs)
            .unwrap();
        assert_eq!(read.progress().status, FileStopRecoveryStatus::Replaying);
        assert_eq!(read.progress().replayed_events, 0);
        read.advance(0, 1).unwrap();
        read.cancel().unwrap();
    }
    assert_eq!(history.files(), before);
    assert_eq!(control.bytes(), coordinator_bytes);
    assert!(!coordinator.report().all_observed_drained());
    {
        let mut read = coordinator
            .begin_resolve_learned_shutdown_visit(revision, 0, &history.inputs)
            .unwrap();
        while read.progress().status == FileStopRecoveryStatus::Replaying {
            read.advance(read.progress().replayed_events, 1).unwrap();
            assert_eq!(history.files(), before);
        }
        let mut changed = acknowledged.clone();
        *changed.last_mut().unwrap() ^= 1;
        std::fs::write(history.root.store().join("delivery.bin"), &changed).unwrap();
        assert_eq!(read.finish(), Err(Error::Binding.into()));
        assert_eq!(history.root.bytes(), changed);
    }
    assert!(!coordinator.report().all_observed_drained());
    assert_eq!(control.bytes(), coordinator_bytes);
    std::fs::write(history.root.store().join("delivery.bin"), &original).unwrap();
    assert!(
        coordinator
            .resolve_learned_shutdown_visit(revision, 0, &history.inputs)
            .is_err()
    );
    assert_eq!(
        history.root.bytes(),
        original,
        "newest acknowledged head prevents rollback acceptance"
    );
    assert_eq!(control.bytes(), coordinator_bytes);
    std::fs::write(history.root.store().join("delivery.bin"), &acknowledged).unwrap();
    {
        let mut read = coordinator
            .begin_resolve_learned_shutdown_visit(revision, 0, &history.inputs)
            .unwrap();
        read.advance(0, 1).unwrap();
        drop(read);
    }
    assert_eq!(
        coordinator
            .resolve_learned_shutdown_visit(revision, 0, &history.inputs)
            .err(),
        Some(JournalError::Unavailable)
    );
    assert_eq!(control.bytes(), coordinator_bytes);
    assert_eq!(history.files(), before);
    drop(coordinator);
    let mut reopened =
        FileShutdownCoordinator::open(control.store(), &plan, MAX_JOURNAL_BYTES, revision).unwrap();
    let result = reopened
        .resolve_learned_shutdown_visit(revision, 0, &history.inputs)
        .unwrap();
    same_native_observation(&result, &observed);
    assert_eq!(history.files(), before);
    assert_eq!(control.bytes(), coordinator_bytes);
}
