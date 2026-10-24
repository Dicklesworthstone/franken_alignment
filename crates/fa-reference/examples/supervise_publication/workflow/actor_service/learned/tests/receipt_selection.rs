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

#[test]
fn native_admission_expiry_and_known_policy_denial_have_distinct_retained_receipts() {
    for mode in [None, Some(false), Some(true)] {
        for expires in [999, 1000, 1001, 61_000] {
            let root = Root::new();
            let (mut config, path, loaded, reviewer, mut driver, mut wire, deadline) =
                generated(&root, mode);
            let permitted = expires == deadline.logical.0;
            evidence(&root, &config, 2, permitted, false);
            // Exercise the original native gateway's retained refusal. The
            // operator's live deadline preflight still refuses an expired call.
            let proposal = FileLearnedTextStreamActorPort::encode_release(
                1,
                LearnedTextRelease::Message,
                ElapsedTick(expires),
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
                    || ElapsedTick(1000),
                )
                .unwrap();
            assert!(report.response.result.is_ok(), "{report:?}");
            {
                let host = driver.supervisor().host().unwrap();
                let disposition = host.request_status(1).unwrap().disposition;
                if expires <= 1000 {
                    assert_eq!(
                        disposition,
                        FileRequestDisposition::NotAdmitted(Error::Stale)
                    );
                    assert!(host.request_action(1).is_err());
                } else {
                    assert!(
                        matches!(disposition, FileRequestDisposition::Admitted { stage, .. }
                        if stage == if permitted { ActionState::Reviewing } else { ActionState::Denied })
                    );
                    assert!(host.request_action(1).is_ok());
                }
                assert_eq!(
                    host.learned_text_stream_message_request(1)
                        .unwrap()
                        .deadline,
                    ElapsedTick(expires)
                );
                assert_eq!(host.inspect().executions, 0);
                assert_eq!(host.inspect().control.ledger.charged, 0);
                assert_eq!(host.inspect().control.ledger.reserved, 0);
            }
            if permitted {
                let (returned, history) =
                    native::review_current(&mut config, &loaded, &reviewer, driver, &deadline);
                driver = returned;
                let human = native::approve(&mut driver, &mut config, &reviewer, 1);
                native::publish(&mut driver, &mut config, &human);
                drop(history);
            }
            let expected = if permitted {
                ActorOutcome::Executed
            } else if expires <= 1000 {
                ActorOutcome::NotAdmitted
            } else {
                ActorOutcome::Denied
            };
            assert!(matches!(response(&mut wire, 1).result,
                Ok(Knowledge::Known { value, .. }) if value == expected));
            assert_eq!(cleanup(driver, 1000, 1), 0);
            remove_live_assets(&root);
            let loaded = recipe::load(&path, &config, false).unwrap();
            let result = recovery::resume(config, loaded, || {
                panic!("terminal refusal/denial/execution receipt must not sample time")
            })
            .unwrap();
            assert!(result.failure.is_none(), "{:?}", result.failure);
            assert!(
                matches!(result.response.result, Ok(Knowledge::Known { value, .. }) if value == expected)
            );
        }
    }
}

#[test]
fn recorded_policy_refusal_and_confirmed_message_both_recover_the_original_receipt() {
    for mode in [None, Some(false), Some(true)] {
        for allowed in [false, true] {
            let root = Root::new();
            let (mut config, path, loaded, reviewer, mut driver, mut wire, deadline) =
                generated(&root, mode);
            evidence(&root, &config, 2, allowed, false);
            submit(
                &mut driver,
                &mut wire,
                &mut config,
                &loaded,
                &deadline,
                &mut || ElapsedTick(1000),
            )
            .unwrap();
            {
                let host = driver.supervisor().host().unwrap();
                let status = host.request_status(1).unwrap();
                assert_eq!(
                    matches!(status.disposition, FileRequestDisposition::NotAdmitted(_)),
                    !allowed
                );
                assert_eq!(host.request_action(1).is_ok(), allowed);
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
            assert_eq!(matches!(expected, ActorOutcome::Executed), allowed);
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
            recovered.ttl_ms = 1; // Cannot renew or replace the retained deadline.
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
