//! Recover original learned receipts, not only successfully frozen actions.
//! Real loaded numerical/native owners; fixture models are not calibration.
use super::continuation as native;
use super::*;
use std::path::PathBuf;

fn generated(
    root: &Root,
    mode: Option<bool>,
) -> (
    Config,
    PathBuf,
    Loaded,
    FileHumanReviewer,
    FileSupervisedDriver,
    Wire,
    Deadline,
) {
    generated_with_config(root, mode, configured(root))
}

fn generated_with_config(
    root: &Root,
    mode: Option<bool>,
    mut config: Config,
) -> (
    Config,
    PathBuf,
    Loaded,
    FileHumanReviewer,
    FileSupervisedDriver,
    Wire,
    Deadline,
) {
    let path = fixture::write(root, b"allow");
    if let Some(owned) = mode {
        super::forecast::install(root, &path, owned, 60_000);
    }
    evidence(root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    let (host, reviewer, observer, _identity) =
        recovery::create_selected(&config, &loaded).unwrap();
    let mut prediction = observer.map(|role| prediction::Session::new(loaded.request, role));
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let wire = ActorWire::new(port);
    let deadline = deadline();
    let mut control = Control::new(&config, 1, None).unwrap();
    assert!(
        generate_selected(
            &mut driver,
            &reviewer,
            &mut config,
            &mut control,
            &deadline,
            &mut || ElapsedTick(1000),
            prediction.as_mut()
        )
        .unwrap()
    );
    (config, path, loaded, reviewer, driver, wire, deadline)
}

fn remove_live_assets(root: &Root) {
    for file in [
        "evidence.json",
        "native-roster.json",
        "helper-model.json",
        "helper-weights.safetensors",
        "helper-monitor.json",
        "helper-tokenizer.bbpe",
        "helper-sampling.json",
    ] {
        fs::remove_file(root.0.join(file)).unwrap();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReceiptCase {
    NotAdmitted,
    Denied,
    Executed,
}
impl ReceiptCase {
    const ALL: [Self; 3] = [Self::NotAdmitted, Self::Denied, Self::Executed];
    const REFUSALS: [Self; 2] = [Self::NotAdmitted, Self::Denied];

    fn permitted(self) -> bool {
        self == Self::Executed
    }
    fn outcome(self) -> ActorOutcome {
        match self {
            Self::NotAdmitted => ActorOutcome::NotAdmitted,
            Self::Denied => ActorOutcome::Denied,
            Self::Executed => ActorOutcome::Executed,
        }
    }
    fn observe(self, root: &Root, config: &Config) {
        evidence(root, config, 2, self.permitted(), false);
    }
    fn assert_admission(self, host: &FileOversight, request: u64) {
        let disposition = host.request_status(request).unwrap().disposition;
        if self == Self::NotAdmitted {
            assert_eq!(
                disposition,
                FileRequestDisposition::NotAdmitted(Error::Binding)
            );
            assert_eq!(
                host.request_action(request).err(),
                Some(JournalError::Contract(Error::WrongState))
            );
        } else {
            assert!(
                matches!(disposition, FileRequestDisposition::Admitted { stage, .. }
                if stage == if self.permitted() { ActionState::Reviewing } else { ActionState::Denied })
            );
            assert!(host.request_action(request).is_ok());
        }
    }
    fn submit_release(
        self,
        driver: &mut FileSupervisedDriver,
        wire: &mut Wire,
        config: &mut Config,
        loaded: &Loaded,
        deadline: &Deadline,
        release: LearnedTextRelease,
    ) {
        if self == Self::NotAdmitted {
            let expires = deadline.logical;
            submit_with_captured_input(driver, config, loaded, release, expires, true);
            let proposal =
                FileLearnedTextStreamActorPort::encode_release(loaded.request, release, expires)
                    .unwrap();
            // This is an exact historical lookup, not an expired live intake.
            assert!(
                wire.exchange(
                    &encode_command(&Command::Submit {
                        request: loaded.request,
                        proposal
                    })
                    .unwrap()
                )
                .result
                .is_ok()
            );
        } else {
            submit_release(driver, wire, config, loaded, release, deadline, &mut || {
                ElapsedTick(1000)
            })
            .unwrap();
        }
        self.assert_admission(&driver.supervisor().host().unwrap(), loaded.request);
    }
}

// The original native broker retains a refused input binding without an
// action. The only changed field is the supplied snapshot semantic epoch; the
// actual captured file, model output and source-only envelope stay original.
fn submit_with_captured_input(
    driver: &mut FileSupervisedDriver,
    config: &mut Config,
    loaded: &Loaded,
    release: LearnedTextRelease,
    expires: ElapsedTick,
    mismatched_input: bool,
) {
    let mut host = driver.supervisor_mut().host_mut().unwrap();
    let revision = host.revision();
    let captured = host
        .refresh_file_source(revision, &mut config.source, ElapsedTick(1000))
        .unwrap();
    let revision = host.revision();
    host.observe_time(revision, ElapsedTick(1000)).unwrap();
    let spec = match release {
        LearnedTextRelease::Message => {
            let message = host.learned_text_message(
                fa_reference::action::consequence::oversight::learned_source::LearnedEvidenceLimits::default()).unwrap();
            host.stream_message_spec(std::str::from_utf8(message.bytes()).unwrap(), expires)
                .unwrap()
        }
        LearnedTextRelease::Finish => host.stream_finish_spec(expires).unwrap(),
    };
    let mut supplied = captured.snapshot().clone();
    if mismatched_input {
        supplied.semantic_epoch = supplied.semantic_epoch.checked_add(1).unwrap();
    }
    let revision = host.revision();
    host.submit_request(revision, loaded.request, spec, supplied)
        .unwrap();
}

#[test]
fn native_captured_input_binding_and_known_policy_denial_have_distinct_retained_receipts() {
    for mode in [None, Some(false), Some(true)] {
        for receipt_case in ReceiptCase::ALL {
            let root = Root::new();
            let (mut config, path, loaded, reviewer, mut driver, mut wire, deadline) =
                generated(&root, mode);
            receipt_case.observe(&root, &config);
            // Already expired source-only intake must still refuse BEFORE
            // source I/O or a retained request. Do not weaken this gate.
            let reads = config.source.status().read_attempts;
            let proposal = FileLearnedTextStreamActorPort::encode_release(
                1,
                LearnedTextRelease::Message,
                ElapsedTick(1000),
            )
            .unwrap();
            let document = encode_command(&Command::Submit {
                request: 1,
                proposal,
            })
            .unwrap();
            let report = driver
                .supervisor_mut()
                .exchange_learned_text_stream_actor_from_policy_file(
                    &mut wire,
                    &document,
                    &mut config.source,
                    || panic!("expired preflight must not read time"),
                )
                .unwrap();
            assert_eq!(report.response.result, Err(fa_reference::action::consequence::oversight::actor_wire::WireError::Unavailable));
            assert!(report.intake.is_none());
            assert_eq!(config.source.status().read_attempts, reads);
            assert_eq!(driver.supervisor().host().unwrap().retained_requests(), 0);
            if receipt_case.permitted() {
                submit(
                    &mut driver,
                    &mut wire,
                    &mut config,
                    &loaded,
                    &deadline,
                    &mut || ElapsedTick(1000),
                )
                .unwrap();
            } else {
                submit_with_captured_input(
                    &mut driver,
                    &mut config,
                    &loaded,
                    LearnedTextRelease::Message,
                    deadline.logical,
                    receipt_case == ReceiptCase::NotAdmitted,
                );
            }
            {
                let host = driver.supervisor().host().unwrap();
                receipt_case.assert_admission(&host, 1);
                assert_eq!(
                    host.learned_text_stream_message_request(1)
                        .unwrap()
                        .deadline,
                    deadline.logical
                );
                assert_eq!(host.inspect().executions, 0);
                assert_eq!(host.inspect().control.ledger.charged, 0);
                assert_eq!(host.inspect().control.ledger.reserved, 0);
            }
            if receipt_case.permitted() {
                let (returned, history) =
                    native::review_current(&mut config, &loaded, &reviewer, driver, &deadline);
                driver = returned;
                let human = native::approve(&mut driver, &mut config, &reviewer, 1);
                native::publish(&mut driver, &mut config, &human);
                drop(history);
            }
            let expected = receipt_case.outcome();
            let proposal = FileLearnedTextStreamActorPort::encode_release(
                1,
                LearnedTextRelease::Message,
                deadline.logical,
            )
            .unwrap();
            assert!(matches!(wire.exchange(&encode_command(&Command::Submit {
                request: 1, proposal }).unwrap()).result,
                Ok(Knowledge::Known { value, .. }) if value == expected));
            assert_eq!(cleanup(driver, 1000, 1), 0);
            remove_live_assets(&root);
            let loaded = recipe::load(&path, &config, false).unwrap();
            let result = recovery::resume(config, loaded, || {
                panic!("terminal refusal/denial/execution receipt must not sample time")
            })
            .unwrap();
            assert!(result.failure.is_none(), "{:?}", result.failure);
            assert!(matches!(result.response.result,
                Ok(Knowledge::Known { value, .. }) if value == expected));
        }
    }
}

#[test]
fn recorded_policy_refusal_and_confirmed_message_both_recover_the_original_receipt() {
    for mode in [None, Some(false), Some(true)] {
        for receipt_case in ReceiptCase::ALL {
            let allowed = receipt_case.permitted();
            let root = Root::new();
            let (mut config, path, loaded, reviewer, mut driver, mut wire, deadline) =
                generated(&root, mode);
            receipt_case.observe(&root, &config);
            receipt_case.submit_release(
                &mut driver,
                &mut wire,
                &mut config,
                &loaded,
                &deadline,
                LearnedTextRelease::Message,
            );
            {
                let host = driver.supervisor().host().unwrap();
                receipt_case.assert_admission(&host, 1);
                let original = host.learned_text_stream_message_request(1).unwrap();
                assert_eq!(original.message.as_deref(), Some("A"));
                assert_eq!(original.deadline, deadline.logical);
                assert_eq!(original.target, config.profile.delivery.target);
                assert_eq!(host.inspect().executions, 0);
            }
            let mut history = None;
            if allowed {
                let (returned, reviewed) =
                    native::review_current(&mut config, &loaded, &reviewer, driver, &deadline);
                driver = returned;
                history = Some(reviewed);
                let human = native::approve(&mut driver, &mut config, &reviewer, 1);
                native::publish(&mut driver, &mut config, &human);
            }
            let Ok(Knowledge::Known {
                value: expected, ..
            }) = response(&mut wire, 1).result
            else {
                panic!("original terminal request must have a known outcome");
            };
            assert_eq!(expected, receipt_case.outcome());
            let numerical = driver
                .supervisor()
                .host()
                .unwrap()
                .learned_generation_inspection()
                .unwrap()
                .numerical;
            let status = driver
                .supervisor()
                .host()
                .unwrap()
                .request_status(1)
                .unwrap();
            assert_eq!(cleanup(driver, 1000, 1), 0);
            drop(history);
            remove_live_assets(&root);
            let mut recovered = recipe::load(&path, &config, false).unwrap();
            recovered.ttl_ms = 1;
            let result = recovery::resume(config, recovered, || {
                panic!("terminal receipt recovery must not consult a new clock")
            })
            .unwrap();
            assert!(result.failure.is_none(), "{:?}", result.failure);
            assert_eq!(result.cleanup_pending, 0);
            match &result.response.result {
                Ok(Knowledge::Known { value, .. }) => assert_eq!(value, &expected),
                other => panic!("lost original terminal result: {other:?}"),
            }
            let mut output = Vec::new();
            assert_eq!(emit(result, &mut output).is_ok(), allowed);
            assert!(!output.windows(7).any(|word| word == b"payload"));
            let config = configured(&root);
            let recovered = recipe::load(&path, &config, false).unwrap();
            let (host, _, _, _identity) = recovery::open_selected(&config, &recovered).unwrap();
            assert_eq!(host.request_status(1).unwrap(), status);
            assert_eq!(
                host.learned_generation_inspection().unwrap().numerical,
                numerical
            );
            assert_eq!(host.inspect().executions, u64::from(allowed));
            assert_eq!(
                host.learned_text_stream_message_request(1)
                    .unwrap()
                    .deadline,
                deadline.logical
            );
            if mode.is_some() {
                assert_eq!(
                    host.action_consistency_snapshot()
                        .unwrap()
                        .evidence
                        .samples(),
                    1
                );
            }
        }
    }
}

#[test]
fn historical_message_lookup_cannot_mint_a_request_from_ready_output() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let (mut config, _, loaded, _, mut driver, mut wire, deadline) = generated(&root, mode);
        let disk = fs::read(config.store.join("delivery.bin")).unwrap();
        {
            let host = driver.supervisor().host().unwrap();
            assert_eq!(
                host.learned_text_stream_message_request(0).err(),
                Some(JournalError::Contract(Error::InvalidInput))
            );
            assert_eq!(
                host.learned_text_stream_message_request(1).err(),
                Some(JournalError::Contract(Error::Missing))
            );
            assert_eq!(host.retained_requests(), 0);
            assert_eq!(host.inspect().executions, 0);
        }
        assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), disk);
        submit(
            &mut driver,
            &mut wire,
            &mut config,
            &loaded,
            &deadline,
            &mut || ElapsedTick(1000),
        )
        .unwrap();
        let disk = fs::read(config.store.join("delivery.bin")).unwrap();
        {
            let host = driver.supervisor().host().unwrap();
            assert_eq!(
                host.learned_text_stream_message_request(1)
                    .unwrap()
                    .message
                    .as_deref(),
                Some("A")
            );
            assert_eq!(
                host.learned_text_stream_message_request(2).err(),
                Some(JournalError::Contract(Error::Missing))
            );
            assert_eq!(host.retained_requests(), 1);
            assert_eq!(host.inspect().executions, 0);
        }
        assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), disk);
        assert_eq!(cleanup(driver, 1000, 1), 0);
    }
}

#[path = "receipt_selection/preflight.rs"]
mod preflight;
#[path = "receipt_selection/readonly.rs"]
mod readonly;
#[path = "receipt_selection/readonly_operator.rs"]
mod readonly_operator;
#[path = "receipt_selection/reconciled.rs"]
mod reconciled;
