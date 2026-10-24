//! Actual option parsing, independent assets and original numerical/receipt owners.
//! Synthetic models exercise the command's enforcement, not detector calibration.
use super::super::super::{finish as finish_workflow, recovery::readonly as query};
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::anchored::MAX_HISTORY_ANCHOR_BYTES;
use fa_reference::action::consequence::oversight::{
    actor::UnknownReason, actor_wire::WireResponse,
};
use std::{collections::BTreeMap, io, path::Path};

fn files(path: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fs::read_dir(path)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (path.clone(), fs::read(path).unwrap())
        })
        .collect()
}
fn config_file(root: &Root, config: &Config) -> PathBuf {
    let path = root.0.join("readonly-config.json");
    let original = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/supervised_publication.json"
    ));
    let text = original
        .replace(
            "/operator/publication-store",
            config.store.to_str().unwrap(),
        )
        .replace(
            "/operator/evidence.json",
            root.0.join("evidence.json").to_str().unwrap(),
        )
        .replace(
            "\"initial_payload_hex\": \"696e697469616c\"",
            "\"initial_payload_hex\": \"\"",
        )
        .replace("\"events\": 256", "\"events\": 1024")
        .replace(
            "\"journal_events\": 4096",
            &format!(
                "\"journal_events\": {}",
                config.profile.delivery.limits.events
            ),
        );
    fs::write(&path, text).unwrap();
    let decoded = Config::read(&path).unwrap();
    assert_eq!(decoded.source_policy, config.source_policy);
    assert_eq!(
        decoded.profile.delivery.limits,
        config.profile.delivery.limits
    );
    path
}
fn anchor(root: &Root, loaded: &mut Loaded, driver: &FileSupervisedDriver) -> PathBuf {
    let held = driver
        .supervisor()
        .host()
        .unwrap()
        .history_anchor()
        .unwrap();
    let mut bytes = Vec::new();
    held.write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES).unwrap();
    let path = root.0.join("independent-readonly.anchor");
    fs::write(&path, bytes).unwrap();
    loaded.recovery_anchor = Some(held);
    path
}
fn arguments(config: &Path, recipe: &Path, anchor: &Path, after: Option<u64>) -> Vec<String> {
    let mut args = vec![
        "create-learned-generated".into(),
        config.to_str().unwrap().into(),
        recipe.to_str().unwrap().into(),
        "--resume".into(),
        "--receipts-only".into(),
    ];
    if let Some(after) = after {
        args.extend(["--finish-after-message".into(), after.to_string()]);
    }
    args.extend(["--history-anchor".into(), anchor.to_str().unwrap().into()]);
    args
}
fn encoded(response: &WireResponse) -> Vec<u8> {
    let mut bytes = response.encode();
    bytes.push(b'\n');
    bytes
}
struct BrokenOutput {
    remaining: usize,
}
impl io::Write for BrokenOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        let count = bytes.len().min(self.remaining);
        self.remaining -= count;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn readonly_command_recovers_denied_and_executed_messages_without_writes_or_live_assets() {
    for mode in [None, Some(false), Some(true)] {
        for receipt_case in ReceiptCase::ALL {
            let allowed = receipt_case.permitted();
            let root = Root::new();
            let (mut config, path, mut loaded, reviewer, mut driver, mut wire, deadline) =
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
            if allowed {
                let (returned, history) =
                    native::review_current(&mut config, &loaded, &reviewer, driver, &deadline);
                driver = returned;
                let human = native::approve(&mut driver, &mut config, &reviewer, 1);
                native::publish(&mut driver, &mut config, &human);
                drop(history);
            }
            let expected = response(&mut wire, 1);
            assert!(
                matches!(&expected.result, Ok(Knowledge::Known { value, .. })
                if *value == receipt_case.outcome())
            );
            let input = anchor(&root, &mut loaded, &driver);
            assert_eq!(cleanup(driver, 1000, 1), 0);
            remove_live_assets(&root);
            fs::write(config.store.join("delivery.pending"), b"read-only staging").unwrap();
            let original = files(&config.store);
            let args = arguments(&config_file(&root, &config), &path, &input, None);
            for _ in 0..2 {
                let mut output = Vec::new();
                assert_eq!(
                    query::command_with_elapsed(&args, None, &mut output, &mut || Duration::ZERO)
                        .is_ok(),
                    allowed
                );
                assert_eq!(output, encoded(&expected));
                assert_eq!(files(&config.store), original);
            }
            let failure = query::command_with_elapsed(
                &args,
                None,
                &mut BrokenOutput { remaining: 8 },
                &mut || Duration::ZERO,
            )
            .unwrap_err();
            assert!(failure.contains("original response output failed"));
            assert_eq!(files(&config.store), original);
            // Exercise the OUTER dispatcher, which otherwise intercepts this
            // flag for the baseline socket service before entering learned code.
            assert_eq!(
                crate::workflow::actor_service::command(&args, None).is_ok(),
                allowed
            );
            assert_eq!(files(&config.store), original);
        }
    }
}

#[test]
fn readonly_command_preserves_published_unknown_until_explicit_original_resume_reconciles() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let (mut config, path, mut loaded, reviewer, mut driver, mut wire, deadline) =
            generated(&root, mode);
        submit(
            &mut driver,
            &mut wire,
            &mut config,
            &loaded,
            &deadline,
            &mut || ElapsedTick(1000),
        )
        .unwrap();
        let (returned, history) =
            native::review_current(&mut config, &loaded, &reviewer, driver, &deadline);
        driver = returned;
        let human = native::approve(&mut driver, &mut config, &reviewer, 1);
        assert!(matches!(
            driver
                .step_computed_from_policy_file(
                    &mut config.source,
                    || ElapsedTick(1000),
                    Some(&human)
                )
                .result
                .unwrap(),
            FileDriverEvent::Dispatched { .. }
        ));
        assert!(matches!(
            driver
                .step_computed_from_policy_file(&mut config.source, || ElapsedTick(1000), None)
                .result
                .unwrap(),
            FileDriverEvent::PublicationChecked { .. }
        ));
        let charge = driver
            .supervisor()
            .host()
            .unwrap()
            .inspect()
            .control
            .ledger
            .charged;
        assert!(charge > 0);
        assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 1);
        assert!(
            driver
                .supervisor()
                .host()
                .unwrap()
                .request_resolution(1)
                .unwrap()
                .is_none()
        );
        let expected = response(&mut wire, 1);
        assert!(matches!(
            expected.result,
            Ok(Knowledge::Unknown {
                reason: UnknownReason::OutcomeUnknown
            })
        ));
        let input = anchor(&root, &mut loaded, &driver);
        assert_eq!(cleanup(driver, 1000, 1), 0);
        drop(history);
        remove_live_assets(&root);
        let original = files(&config.store);
        let args = arguments(&config_file(&root, &config), &path, &input, None);
        let mut output = Vec::new();
        assert!(
            query::command_with_elapsed(&args, None, &mut output, &mut || Duration::ZERO).is_err()
        );
        assert_eq!(output, encoded(&expected));
        assert_eq!(files(&config.store), original);
        // Existing --resume is deliberately NOT changed: its explicit recovery
        // fence and receipt check can confirm this already-published message.
        let store = config.store.clone();
        let bootstrap = config.profile.clone();
        let generation = loaded.generation.clone();
        let result = recovery::resume(config, loaded, || ElapsedTick(1000)).unwrap();
        assert!(result.failure.is_none(), "{:?}", result.failure);
        assert!(matches!(
            result.response.result,
            Ok(Knowledge::Known {
                value: ActorOutcome::Executed,
                ..
            })
        ));
        let state = FileOversight::read_publication_with_learned_generation(
            &store,
            &bootstrap,
            &generation,
        )
        .unwrap();
        assert_eq!(state.executions, 1);
        assert_eq!(state.control.ledger.charged, charge);
    }
}

#[test]
fn readonly_finish_command_requires_exact_predecessor_and_preserves_denied_or_confirmed_finish() {
    for mode in [None, Some(false), Some(true)] {
        for receipt_case in ReceiptCase::ALL {
            let allowed = receipt_case.permitted();
            let root = Root::new();
            let (mut config, path, first, reviewer, mut driver, mut wire, deadline) =
                generated(&root, mode);
            submit(
                &mut driver,
                &mut wire,
                &mut config,
                &first,
                &deadline,
                &mut || ElapsedTick(1000),
            )
            .unwrap();
            let (returned, history) =
                native::review_current(&mut config, &first, &reviewer, driver, &deadline);
            driver = returned;
            let human = native::approve(&mut driver, &mut config, &reviewer, 1);
            native::publish(&mut driver, &mut config, &human);
            assert!(executed(&mut wire, 1));
            drop(history);
            let finish_path = root.0.join("readonly-finish.json");
            fs::write(
                &finish_path,
                fs::read_to_string(path)
                    .unwrap()
                    .replacen("\"request\":1", "\"request\":2", 1)
                    .replacen("\"object_id\":1001", "\"object_id\":1002", 1)
                    .replacen("\"round_ids\":[1]", "\"round_ids\":[2]", 1),
            )
            .unwrap();
            let mut loaded = recipe::load(&finish_path, &config, true).unwrap();
            assert_eq!(first.generation, loaded.generation);
            assert_eq!(cleanup(driver, 1000, 1), 0);
            let (host, reviewer, observer, _identity) =
                recovery::open_selected(&config, &loaded).unwrap();
            let mut prediction = observer.map(|observer| prediction::Session::new(2, observer));
            let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
            let mut driver = FileSupervisedDriver::new(supervisor);
            let mut wire = ActorWire::new(port);
            let mut control = Control::new(&config, 2, None).unwrap();
            assert!(
                finish_workflow::prepare(
                    &mut driver,
                    &reviewer,
                    &mut config,
                    &loaded,
                    1,
                    &mut control,
                    &deadline,
                    &mut || ElapsedTick(1000)
                )
                .unwrap()
            );
            if let Some(prediction) = prediction.as_mut() {
                prediction
                    .finish_request(&mut driver.supervisor_mut().host_mut().unwrap())
                    .unwrap();
            }
            receipt_case.observe(&root, &config);
            receipt_case.submit_release(
                &mut driver,
                &mut wire,
                &mut config,
                &loaded,
                &deadline,
                LearnedTextRelease::Finish,
            );
            if allowed {
                let reviewed = review::run(
                    driver,
                    &mut config,
                    &loaded,
                    &reviewer,
                    &mut control,
                    &deadline,
                    &mut || ElapsedTick(1000),
                );
                assert!(reviewed.failure.is_none(), "{:?}", reviewed.failure);
                driver = reviewed.driver;
                let history = reviewed.history.unwrap();
                assert_eq!(history.status(), NativeReviewStatus::Finished);
                assert_eq!(
                    native_records(&history)
                        .filter(|record| record.progress.basis
                            == Some(SidecarDecisionBasis::NativeModel))
                        .count(),
                    2
                );
                let human = native::approve(&mut driver, &mut config, &reviewer, 2);
                native::publish(&mut driver, &mut config, &human);
                drop(history);
            }
            let expected = response(&mut wire, 2);
            assert!(
                matches!(&expected.result, Ok(Knowledge::Known { value, .. })
                if *value == receipt_case.outcome())
            );
            let input = anchor(&root, &mut loaded, &driver);
            assert_eq!(cleanup(driver, 1000, 1), 0);
            drop(prediction);
            remove_live_assets(&root);
            fs::write(
                config.store.join("delivery.pending"),
                b"retain finish staging",
            )
            .unwrap();
            let original = files(&config.store);
            let config_path = config_file(&root, &config);
            for wrong in [None, Some(9)] {
                let args = arguments(&config_path, &finish_path, &input, wrong);
                let mut output = Vec::new();
                assert!(
                    query::command_with_elapsed(&args, None, &mut output, &mut || Duration::ZERO)
                        .is_err()
                );
                assert!(output.is_empty());
                assert_eq!(files(&config.store), original);
            }
            let args = arguments(&config_path, &finish_path, &input, Some(1));
            let mut output = Vec::new();
            assert_eq!(
                query::command_with_elapsed(&args, None, &mut output, &mut || Duration::ZERO)
                    .is_ok(),
                allowed
            );
            assert_eq!(output, encoded(&expected));
            assert_eq!(files(&config.store), original);
        }
    }
}

#[test]
fn readonly_operator_deadline_includes_every_replay_boundary_and_final_canonical_check() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let (mut config, _, mut loaded, _, mut driver, mut wire, deadline) = generated(&root, mode);
        evidence(&root, &config, 2, false, false);
        submit(
            &mut driver,
            &mut wire,
            &mut config,
            &loaded,
            &deadline,
            &mut || ElapsedTick(1000),
        )
        .unwrap();
        let expected = response(&mut wire, 1);
        let _input = anchor(&root, &mut loaded, &driver);
        let total = driver.supervisor().host().unwrap().revision() as usize;
        assert_eq!(cleanup(driver, 1000, 1), 0);
        fs::write(
            config.store.join("delivery.pending"),
            b"keep at all time cuts",
        )
        .unwrap();
        let original = files(&config.store);
        let limit = Duration::from_millis(config.timing.runtime_ms);
        // pre-open + N events + Ready + post-finalizer canonical re-read.
        for expire_at in 0..=total + 2 {
            let mut calls = 0;
            let result = query::read(&config, &loaded, None, &mut || {
                assert_eq!(files(&config.store), original);
                let at = calls;
                calls += 1;
                if at == expire_at {
                    limit
                } else {
                    Duration::ZERO
                }
            });
            assert_eq!(
                result.err().as_deref(),
                Some("learned receipt recovery replay deadline reached")
            );
            assert_eq!(calls, expire_at + 1);
            assert_eq!(files(&config.store), original);
        }
        let mut calls = 0;
        let actual = query::read(&config, &loaded, None, &mut || {
            calls += 1;
            Duration::ZERO
        })
        .unwrap();
        assert_eq!(calls, total + 3);
        assert_eq!(actual.encode(), expected.encode());
        assert_eq!(files(&config.store), original);
        loaded.anchor_output = Some(root.0.join("must-not-create.anchor"));
        assert!(
            query::read(&config, &loaded, None, &mut || panic!(
                "output export must refuse before replay or clock"
            ))
            .is_err()
        );
        assert!(!loaded.anchor_output.as_ref().unwrap().exists());
        assert_eq!(files(&config.store), original);
    }
}

#[test]
fn readonly_option_rejects_live_modes_duplicates_and_anchor_output_before_intake() {
    let base: Vec<String> = [
        "create-learned-generated",
        "/missing/config",
        "/missing/recipe",
        "--resume",
        "--receipts-only",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let mut cases = Vec::new();
    for extra in [
        vec!["--receipts-only"],
        vec!["--continue-generation"],
        vec!["--after-message", "1"],
        vec!["--recover-stop", "1"],
        vec!["--audit-review", "1", "/anchor"],
        vec!["--retain-history-anchor", "/must-not-create"],
        vec!["--credibility-activation", "/capsule"],
    ] {
        let mut args = base.clone();
        args.extend(extra.into_iter().map(str::to_owned));
        cases.push(args);
    }
    let mut live = base.clone();
    live[3] = "/reviewer".into();
    cases.push(live);
    for args in cases {
        let mut output = Vec::new();
        let error = query::command_with_elapsed(&args, None, &mut output, &mut || {
            panic!("invalid options must not reach intake or replay")
        })
        .unwrap_err();
        assert!(error.starts_with("create-learned-generated"));
        assert!(output.is_empty());
        assert!(crate::workflow::actor_service::command(&args, None).is_err());
    }
    let mut output = Vec::new();
    assert!(
        query::command_with_elapsed(
            &base,
            Some(Path::new("/capsule")),
            &mut output,
            &mut || panic!("a capsule cannot enter receipt-only recovery")
        )
        .is_err()
    );
    assert!(output.is_empty());
}
