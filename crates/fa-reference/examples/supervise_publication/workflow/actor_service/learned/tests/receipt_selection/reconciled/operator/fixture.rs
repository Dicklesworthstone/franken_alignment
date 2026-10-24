//! Actual on-disk operator input and original native-reviewed finish histories.
use super::*;
use crate::workflow::actor_service::learned::finish as finish_workflow;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::anchored::{
    FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES,
};
use std::path::Path;

pub(super) fn config_file(root: &Root, config: &Config) -> PathBuf {
    let path = root.0.join("reconciled-config.json");
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
    let actual = Config::read(&path).unwrap();
    assert_eq!(actual.source_policy, config.source_policy);
    assert_eq!(
        actual.profile.delivery.limits,
        config.profile.delivery.limits
    );
    path
}

pub(super) fn anchor_file(root: &Root, anchor: &FileHistoryAnchor) -> PathBuf {
    let path = root.0.join("independent-settlement.anchor");
    let mut bytes = Vec::new();
    anchor
        .write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES)
        .unwrap();
    fs::write(&path, bytes).unwrap();
    path
}

pub(super) fn args(config: &Path, recipe: &Path, anchor: &Path, after: Option<u64>) -> Vec<String> {
    let mut result = vec![
        "create-learned-generated".into(),
        config.to_str().unwrap().into(),
        recipe.to_str().unwrap().into(),
        "--resume".into(),
        "--reconcile-receipts".into(),
    ];
    if let Some(after) = after {
        result.extend(["--finish-after-message".into(), after.to_string()]);
    }
    result.extend(["--history-anchor".into(), anchor.to_str().unwrap().into()]);
    result
}

pub(super) fn message_denied(root: &Root, mode: Option<bool>, receipt_case: ReceiptCase) -> Sent {
    let (mut config, path, mut loaded, _, mut driver, mut wire, deadline) = generated(root, mode);
    receipt_case.observe(root, &config);
    receipt_case.submit_release(
        &mut driver,
        &mut wire,
        &mut config,
        &loaded,
        &deadline,
        LearnedTextRelease::Message,
    );
    assert!(matches!(response(&mut wire, 1).result,
        Ok(Knowledge::Known { value, .. }) if value == receipt_case.outcome()));
    let host = driver.supervisor().host().unwrap();
    let revision = host.revision();
    let charged = host.inspect().control.ledger.charged;
    loaded.recovery_anchor = Some(host.history_anchor().unwrap());
    drop(host);
    assert_eq!(cleanup(driver, 1000, 1), 0);
    Sent {
        config,
        path,
        loaded,
        revision,
        charged,
    }
}

pub(super) fn finish_sent(root: &Root, mode: Option<bool>, receipt_case: ReceiptCase) -> Sent {
    let allowed = receipt_case.permitted();
    let (mut config, path, first, reviewer, mut driver, mut wire, deadline) = generated(root, mode);
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
    let path2 = root.0.join("reconciled-finish.json");
    fs::write(
        &path2,
        fs::read_to_string(path)
            .unwrap()
            .replacen("\"request\":1", "\"request\":2", 1)
            .replacen("\"object_id\":1001", "\"object_id\":1002", 1)
            .replacen("\"round_ids\":[1]", "\"round_ids\":[2]", 1),
    )
    .unwrap();
    let mut loaded = recipe::load(&path2, &config, true).unwrap();
    assert_eq!(first.generation, loaded.generation);
    assert_eq!(cleanup(driver, 1000, 1), 0);
    let (host, reviewer, observer, _identity) = recovery::open_selected(&config, &loaded).unwrap();
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
    receipt_case.observe(root, &config);
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
                .filter(|record| record.progress.basis == Some(SidecarDecisionBasis::NativeModel))
                .count(),
            2
        );
        let human = native::approve(&mut driver, &mut config, &reviewer, 2);
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
        drop(history);
    }
    let observation = response(&mut wire, 2).result.unwrap();
    if allowed {
        assert!(matches!(
            observation,
            Knowledge::Unknown {
                reason: UnknownReason::OutcomeUnknown
            }
        ));
    } else {
        assert!(
            matches!(observation, Knowledge::Known { value, .. } if value == receipt_case.outcome())
        );
    }
    let host = driver.supervisor().host().unwrap();
    let revision = host.revision();
    let charged = host.inspect().control.ledger.charged;
    assert_eq!(host.inspect().executions, 1 + u64::from(allowed));
    loaded.recovery_anchor = Some(host.history_anchor().unwrap());
    drop(host);
    assert_eq!(cleanup(driver, 1000, 1), 0);
    drop(prediction);
    Sent {
        config,
        path: path2,
        loaded,
        revision,
        charged,
    }
}
