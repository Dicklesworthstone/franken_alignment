//! A narrowing-only native operator must retain the original generated owner.
//! Its separate replay budget never advances inference or the stream boundary.

use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileNarrowBasis, MAX_NARROW_REPLAY_EVENTS,
};
use fa_reference::action::consequence::gate::TargetCeiling;

fn narrowing_settings(
    root: &Root,
    actor: &Profile,
    events: usize,
    numerical: Option<usize>,
) -> Settings {
    let scope = actor.scope;
    let identity = actor.supervisor;
    let numerical = numerical.map_or_else(String::new, |events| {
        format!(r#", "numerical_replay_events":{events}"#)
    });
    Settings::decode(
        format!(
            r#"{{"schema":"fa.tower-operator-service/1",
      "scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},
      "socket":"{}/operator.sock",
      "supervisor":{{"uid":{},"gid":{},"pid":{}}},
      "operator":{{"uid":{},"gid":{},"pid":{}}},
      "candidate_limit":8,"connection_limit":8,"exchange_limit":32,
      "runtime_ms":15000,"poll_ms":1,"inspection_queries":0,
      "explanation_queries":0,"explanation_replay_events":0,
      "explanation_disclosure":"none",
      "narrowing":{{"queries":1,"operations":1,"max_replay_events":{events},
        "authority":"narrow_original_targets"{numerical}}}}}"#,
            scope.tenant,
            scope.principal,
            scope.run,
            scope.branch,
            scope.authority,
            root.0.display(),
            identity.uid(),
            identity.gid(),
            identity.pid().unwrap(),
            identity.uid(),
            identity.gid(),
            identity.pid().unwrap(),
        )
        .as_bytes(),
    )
    .unwrap()
}

#[test]
fn generated_narrowing_only_operator_pays_once_without_advancing_inference_or_stream() {
    for mode in ["default", "short", "exact"] {
        let root = Root::new();
        let mut config = configured(&root);
        let (actor, reviewer_profile) = profiles(&root, &config);
        let source_bytes = std::fs::read(config.source.path()).unwrap();
        if mode == "default" {
            let source_work = config.source.status();
            let chosen = narrowing_settings(&root, &actor, MAX_NARROW_REPLAY_EVENTS, None);
            let socket = chosen.socket.clone();
            let error = native_operator::Selection::new(chosen, REGISTRATION)
                .err()
                .expect("native narrowing needs a separate numerical allowance");
            assert!(error.contains("explicit numerical replay budget"), "{error}");
            assert!(!config.store.exists());
            assert!(!socket.exists());
            assert_eq!(config.source.status(), source_work);
            assert_eq!(std::fs::read(config.source.path()).unwrap(), source_bytes);
            continue;
        }

        let native = inputs(&root, &config);
        let selected = bootstrap::selection(&config, &native, None).unwrap();
        let (host, _reviewer) = bootstrap::prepare(&config, &native, selected, false).unwrap();
        let (port, mut driver) = host.into_generated_text_supervised_driver().unwrap();
        let limit = Deadline {
            logical: ElapsedTick(16000),
            started: Instant::now(),
            wall: Duration::from_secs(15),
        };
        refresh(&mut driver, &mut config, &mut || ElapsedTick(1000)).unwrap();
        {
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            let numerical = host.decoder_inspection().unwrap().numerical;
            let command = FileTextGenerationCommand::new(
                native.generation,
                numerical.actor_revision,
                numerical.position,
                native.request.clone(),
            )
            .unwrap();
            let revision = host.revision();
            host.begin_decoder_text(revision, command).unwrap();
            for generation_revision in 0..4 {
                let revision = host.revision();
                host.advance_decoder_text(revision, native.generation, generation_revision)
                    .unwrap();
            }
        }
        let (before, numerical, stream, text, source) = {
            let host = driver.supervisor().host().unwrap();
            (
                host.inspect(),
                host.decoder_inspection().unwrap(),
                host.stream_snapshot().unwrap(),
                host.decoder_text_progress(native.generation).unwrap(),
                host.file_source_status().unwrap(),
            )
        };
        assert_eq!(numerical.numerical.position, 4);
        assert_eq!(text.generation_revision(), 4);
        assert!(text.is_complete());
        assert_eq!(text.bytes().unwrap(), b"A");
        assert!(before.control.ceiling.is_none());
        assert_eq!(before.executions, 0);
        let source_work = config.source.status();
        let journal = std::fs::read(config.store.join("delivery.bin")).unwrap();
        let events = usize::try_from(before.revision).unwrap() + 1;
        let paid = mode == "exact";
        let chosen = narrowing_settings(
            &root,
            &actor,
            events,
            Some(if paid { events } else { events - 1 }),
        );
        let selection = native_operator::Selection::new(chosen, REGISTRATION).unwrap();
        selection.check(&config, &actor, &reviewer_profile).unwrap();
        // No inspection, fence, cancellation or abandonment selects a control
        // mount incidentally. Narrowing alone must choose that original mount.
        let mut service = selection.start(&actor, port, &driver, &limit).unwrap();
        let operation = 1501;
        let mut next = request(&actor, "generated-narrow-plan", Verb::Next);
        next.arguments = ReadArguments::NarrowingNext {
            operation,
            retained_targets: vec![before.target],
        };
        let mut command = request(&actor, "narrow-authority-1501", Verb::Narrow);
        command.idempotency_key = Some(operation.to_string());
        command.arguments = ReadArguments::Narrow {
            operation,
            expected: FileNarrowBasis {
                scope: actor.scope,
                journal_revision: before.revision,
                control_sequence: before.control.sequence,
                authority_epoch: before.control.ledger.epoch,
                recorded_at: before.control.ledger.elapsed,
            },
            retained_targets: vec![before.target],
        };
        let settings = selection.settings.clone();
        let client = std::thread::spawn(move || {
            let mut socket = connect(&settings);
            let plan = exchange(&mut socket, &next, settings.response_bytes);
            if paid {
                let encoded = encode_request(&command).unwrap();
                assert_eq!(
                    field(&plan, &["result", "value", "knowledge", "value", "action"]),
                    &strict_json::parse(encoded.as_bytes(), KNOWLEDGE_JSON_LIMITS).unwrap(),
                );
            }
            let response = exchange(&mut socket, &command, settings.response_bytes);
            // Retry through a new authenticated connection with the same grant.
            // The exact paid allowance and operation are already exhausted.
            drop(socket);
            let mut socket = connect(&settings);
            let retry = exchange(&mut socket, &command, settings.response_bytes);
            (plan, response, retry)
        });
        let started = Instant::now();
        while !client.is_finished() {
            service.checkpoint();
            assert!(started.elapsed() < Duration::from_secs(20));
            pause(1);
        }
        let (plan, response, retry) = client.join().unwrap();
        assert_eq!(field(&plan, &["result", "kind"]).as_str(), Some("narrowing-next"));
        assert_eq!(
            field(&plan, &["result", "value", "knowledge", "state"]).as_str(),
            Some("known"),
        );
        assert_eq!(field(&response, &["result", "kind"]).as_str(), Some("narrowing"));
        assert_eq!(retry, response);
        let plan_value = field(&plan, &["result", "value", "knowledge", "value"]);
        assert_eq!(plan_value.get("can_narrow").unwrap().as_bool(), Some(paid));
        let mut expected = before.clone();
        if paid {
            assert!(plan_value.get("block_reason").unwrap().is_null());
            assert_eq!(
                field(&response, &["result", "value", "knowledge", "state"]).as_str(),
                Some("known"),
            );
            expected.revision += 1;
            expected.control.sequence += 1;
            expected.control.ceiling = Some(TargetCeiling::new(&[before.target]).unwrap());
            assert_eq!(
                number(&response, &["result", "value", "knowledge", "basis", "journal_revision"]),
                expected.revision,
            );
            assert_eq!(
                number(&response, &["result", "value", "knowledge", "value", "refunded_units"]),
                0,
            );
        } else {
            assert_eq!(
                plan_value.get("block_reason").unwrap().as_str(),
                Some("numerical_replay_limit"),
            );
            assert!(plan_value.get("action").unwrap().is_null());
            assert_eq!(
                field(&response, &["envelope", "reason_code"]).as_str(),
                Some("BudgetExhausted"),
            );
            assert_eq!(
                field(&response, &["result", "value", "error", "code"]).as_str(),
                Some("capacity"),
            );
            assert_eq!(std::fs::read(config.store.join("delivery.bin")).unwrap(), journal);
        }
        {
            let host = driver.supervisor().host().unwrap();
            assert_eq!(host.inspect(), expected);
            let mut expected_numerical = numerical;
            expected_numerical.journal_revision = expected.revision;
            assert_eq!(host.decoder_inspection().unwrap(), expected_numerical);
            let mut expected_stream = stream;
            expected_stream.publication = expected;
            assert_eq!(host.stream_snapshot().unwrap(), expected_stream);
            assert_eq!(host.file_source_status().unwrap(), source);
            let after = host.decoder_text_progress(native.generation).unwrap();
            assert_eq!(after.command(), text.command());
            assert_eq!(after.generation_revision(), text.generation_revision());
            assert_eq!(after.finish(), text.finish());
            assert_eq!(after.bytes().unwrap(), text.bytes().unwrap());
            assert_eq!(after.numerical().tokens(), text.numerical().tokens());
        }
        assert_eq!(config.source.status(), source_work);
        assert_eq!(std::fs::read(config.source.path()).unwrap(), source_bytes);
        service.retire();
        assert!(!selection.settings.socket.exists());
        assert_eq!(
            cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms),
            0,
        );
    }
}
