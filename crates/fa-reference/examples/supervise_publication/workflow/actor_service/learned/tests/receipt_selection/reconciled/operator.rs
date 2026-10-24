//! Command routing and acknowledged-settlement failure cuts, with real owners.
//! No fabricated receipt, new executor, credential or permitting model fixture.
use super::*;
use crate::workflow::actor_service::learned::recovery::readonly as query;
use fa_reference::action::consequence::oversight::actor_wire::WireResponse;
use std::{io, path::Path};
#[path = "operator/fixture.rs"]
mod fixture;

fn encoded(response: &WireResponse) -> Vec<u8> {
    let mut bytes = response.encode();
    bytes.push(b'\n');
    bytes
}
fn state(
    case: &Sent,
) -> fa_reference::action::consequence::delivery::persistent::FileDeliverySnapshot {
    FileOversight::read_publication_with_learned_generation(
        &case.config.store,
        &case.config.profile,
        &case.loaded.generation,
    )
    .unwrap()
}
fn command_args(root: &Root, case: &Sent, after: Option<u64>) -> Vec<String> {
    fixture::args(
        &fixture::config_file(root, &case.config),
        &case.path,
        &fixture::anchor_file(root, case.loaded.recovery_anchor.as_ref().unwrap()),
        after,
    )
}

#[test]
fn explicit_reconciliation_confirms_only_original_receipts_without_repeating_effects() {
    for mode in [None, Some(false), Some(true)] {
        for published in [false, true] {
            let root = Root::new();
            let case = sent(&root, mode, published);
            let args = command_args(&root, &case, None);
            remove_live_assets(&root);
            let old_anchor = fs::read(root.0.join("independent-settlement.anchor")).unwrap();
            let before = files(&case.config.store);
            let initial =
                query::read(&case.config, &case.loaded, None, &mut || Duration::ZERO).unwrap();
            assert!(matches!(
                initial.result,
                Ok(Knowledge::Unknown {
                    reason: UnknownReason::OutcomeUnknown
                })
            ));
            assert_eq!(files(&case.config.store), before);
            let calls = Cell::new(0);
            let mut output = Vec::new();
            assert_eq!(
                query::command_reconciled_with_elapsed(
                    &args,
                    None,
                    &mut output,
                    || {
                        calls.set(calls.get() + 1);
                        ElapsedTick(1001)
                    },
                    &mut || Duration::ZERO
                )
                .is_ok(),
                published
            );
            assert_eq!(calls.get(), 1);
            let actual =
                query::read(&case.config, &case.loaded, None, &mut || Duration::ZERO).unwrap();
            assert_eq!(output, encoded(&actual));
            assert_eq!(
                matches!(
                    actual.result,
                    Ok(Knowledge::Known {
                        value: ActorOutcome::Executed,
                        ..
                    })
                ),
                published
            );
            if !published {
                assert!(matches!(
                    actual.result,
                    Ok(Knowledge::Unknown {
                        reason: UnknownReason::OutcomeUnknown
                    })
                ));
            }
            let current = state(&case);
            assert_eq!(current.revision, case.revision + 3);
            assert_eq!(current.executions, u64::from(published));
            assert_eq!(current.control.ledger.charged, case.charged);
            assert_eq!(
                fs::read(root.0.join("independent-settlement.anchor")).unwrap(),
                old_anchor
            );
            let settled = files(&case.config.store);
            for _ in 0..2 {
                assert_eq!(
                    query::read(&case.config, &case.loaded, None, &mut || Duration::ZERO)
                        .unwrap()
                        .encode(),
                    actual.encode()
                );
                assert_eq!(files(&case.config.store), settled);
            }
        }
    }
}

#[test]
fn finish_settlement_requires_exact_predecessor_and_preserves_denial_or_original_publication() {
    for mode in [None, Some(false), Some(true)] {
        for receipt_case in ReceiptCase::ALL {
            let allowed = receipt_case.permitted();
            let root = Root::new();
            let case = fixture::finish_sent(&root, mode, receipt_case);
            remove_live_assets(&root);
            fs::write(
                case.config.store.join("delivery.pending"),
                b"wrong selection must preserve",
            )
            .unwrap();
            let before = files(&case.config.store);
            for wrong in [None, Some(9)] {
                let args = command_args(&root, &case, wrong);
                let mut output = Vec::new();
                assert!(
                    query::command_reconciled_with_elapsed(
                        &args,
                        None,
                        &mut output,
                        || panic!("wrong finish reached clock"),
                        &mut || Duration::ZERO
                    )
                    .is_err()
                );
                assert!(output.is_empty());
                assert_eq!(files(&case.config.store), before);
            }
            let args = command_args(&root, &case, Some(1));
            let mut output = Vec::new();
            assert_eq!(
                query::command_reconciled_with_elapsed(
                    &args,
                    None,
                    &mut output,
                    || ElapsedTick(1001),
                    &mut || Duration::ZERO
                )
                .is_ok(),
                allowed
            );
            let result =
                query::read(&case.config, &case.loaded, Some(1), &mut || Duration::ZERO).unwrap();
            assert_eq!(output, encoded(&result));
            assert!(matches!(result.result, Ok(Knowledge::Known { value, .. })
                if value == receipt_case.outcome()));
            let current = state(&case);
            assert_eq!(current.revision, case.revision + 3);
            assert_eq!(current.executions, 1 + u64::from(allowed));
            assert_eq!(current.control.ledger.charged, case.charged);
            assert!(!case.config.store.join("delivery.pending").exists());
        }
    }
}

#[test]
fn every_precommit_timeout_is_nonmutating_and_late_timeout_reports_acknowledgment() {
    for (mode, receipt_case) in [None, Some(false), Some(true)]
        .into_iter()
        .flat_map(|mode| ReceiptCase::REFUSALS.map(|receipt_case| (mode, receipt_case)))
    {
        let root = Root::new();
        let case = fixture::message_denied(&root, mode, receipt_case);
        fs::write(
            case.config.store.join("delivery.pending"),
            b"time cut staging",
        )
        .unwrap();
        let before = files(&case.config.store);
        let limit = Duration::from_millis(case.config.timing.runtime_ms);
        // pre-open, every original event, and Ready all precede the clock/write.
        for expire_at in 0..=case.revision as usize + 1 {
            let mut calls = 0;
            let result = query::reconcile(
                &case.config,
                &case.loaded,
                None,
                || panic!("expired replay reached settlement clock"),
                &mut || {
                    let at = calls;
                    calls += 1;
                    assert_eq!(files(&case.config.store), before);
                    if at == expire_at {
                        limit
                    } else {
                        Duration::ZERO
                    }
                },
            );
            assert_eq!(
                result.err().as_deref(),
                Some("learned receipt recovery replay deadline reached")
            );
            assert_eq!(calls, expire_at + 1);
            assert_eq!(files(&case.config.store), before);
        }
        let mut calls = 0;
        let clock_calls = Cell::new(0);
        let result = query::reconcile(
            &case.config,
            &case.loaded,
            None,
            || {
                clock_calls.set(clock_calls.get() + 1);
                ElapsedTick(1001)
            },
            &mut || {
                let at = calls;
                calls += 1;
                if at == case.revision as usize + 2 {
                    limit
                } else {
                    Duration::ZERO
                }
            },
        );
        let error = result.unwrap_err();
        assert!(error.contains("reconciliation acknowledged"));
        assert!(error.contains("--receipts-only"));
        assert_eq!(clock_calls.get(), 1);
        let current = state(&case);
        assert_eq!(current.revision, case.revision + 3);
        assert_eq!(current.executions, 0);
        assert!(!case.config.store.join("delivery.pending").exists());
        let after = files(&case.config.store);
        assert!(
            matches!(query::read(&case.config, &case.loaded, None, &mut || Duration::ZERO).unwrap().result,
            Ok(Knowledge::Known { value, .. }) if value == receipt_case.outcome())
        );
        assert_eq!(files(&case.config.store), after);
    }
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
fn output_loss_after_settlement_recovers_readonly_without_another_effect_or_fence() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let case = sent(&root, mode, true);
        let args = command_args(&root, &case, None);
        remove_live_assets(&root);
        let calls = Cell::new(0);
        let error = query::command_reconciled_with_elapsed(
            &args,
            None,
            &mut BrokenOutput { remaining: 8 },
            || {
                calls.set(calls.get() + 1);
                ElapsedTick(1001)
            },
            &mut || Duration::ZERO,
        )
        .unwrap_err();
        assert!(error.contains("original response output failed"));
        assert_eq!(calls.get(), 1);
        let after = files(&case.config.store);
        let current = state(&case);
        assert_eq!(current.revision, case.revision + 3);
        assert_eq!(current.executions, 1);
        assert_eq!(current.control.ledger.charged, case.charged);
        let readonly = args
            .iter()
            .map(|value| {
                if value == "--reconcile-receipts" {
                    "--receipts-only".into()
                } else {
                    value.clone()
                }
            })
            .collect::<Vec<_>>();
        let mut output = Vec::new();
        query::command_with_elapsed(&readonly, None, &mut output, &mut || Duration::ZERO).unwrap();
        let result = query::read(&case.config, &case.loaded, None, &mut || Duration::ZERO).unwrap();
        assert!(matches!(
            result.result,
            Ok(Knowledge::Known {
                value: ActorOutcome::Executed,
                ..
            })
        ));
        assert_eq!(output, encoded(&result));
        assert_eq!(files(&case.config.store), after);
    }
}

#[test]
fn explicit_modes_and_mutating_options_cannot_be_silently_interchanged() {
    let base: Vec<String> = [
        "create-learned-generated",
        "/missing/config",
        "/missing/recipe",
        "--resume",
        "--reconcile-receipts",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let mut cases = Vec::new();
    for extra in [
        vec!["--receipts-only"],
        vec!["--reconcile-receipts"],
        vec!["--continue-generation"],
        vec!["--after-message", "1"],
        vec!["--recover-stop", "1"],
        vec!["--audit-review", "1", "/anchor"],
        vec!["--retain-history-anchor", "/output"],
        vec!["--credibility-activation", "/capsule"],
        vec!["--history-anchor"],
        vec!["--finish-after-message", "0"],
    ] {
        let mut args = base.clone();
        args.extend(extra.into_iter().map(str::to_owned));
        cases.push(args);
    }
    let mut missing_anchor_value = base.clone();
    missing_anchor_value.insert(4, "--history-anchor".into());
    cases.push(missing_anchor_value);
    let mut no_resume = base.clone();
    no_resume[3] = "/reviewer".into();
    cases.push(no_resume);
    let mut readonly = base.clone();
    readonly[4] = "--receipts-only".into();
    cases.push(readonly);
    for args in cases {
        let mut output = Vec::new();
        assert!(
            query::command_reconciled_with_elapsed(
                &args,
                None,
                &mut output,
                || panic!("invalid grammar reached clock"),
                &mut || panic!("invalid grammar reached intake timer")
            )
            .is_err()
        );
        assert!(output.is_empty());
    }
    assert!(
        query::command_reconciled_with_elapsed(
            &base,
            Some(Path::new("/capsule")),
            &mut Vec::new(),
            || panic!("qualification reached clock"),
            &mut || panic!("qualification reached intake")
        )
        .is_err()
    );
    assert!(
        query::command_with_elapsed(&base, None, &mut Vec::new(), &mut || panic!(
            "read-only entry must reject mutating flags"
        ))
        .is_err()
    );
}

#[test]
fn outer_dispatcher_reaches_learned_settlement_without_loading_live_assets() {
    for (mode, receipt_case) in [None, Some(false), Some(true)]
        .into_iter()
        .flat_map(|mode| ReceiptCase::REFUSALS.map(|receipt_case| (mode, receipt_case)))
    {
        // A retained refusal has no expiring endpoint receipt. The real wall
        // clock can therefore exercise the actual command without fixture time.
        let root = Root::new();
        let case = fixture::message_denied(&root, mode, receipt_case);
        let args = command_args(&root, &case, None);
        remove_live_assets(&root);
        let error = crate::workflow::actor_service::command(&args, None).unwrap_err();
        assert_eq!(
            error,
            "learned publication is not confirmed; inspect the original response"
        );
        let current = state(&case);
        assert_eq!(current.revision, case.revision + 3);
        assert_eq!(current.executions, 0);
        let after = files(&case.config.store);
        let read = query::read(&case.config, &case.loaded, None, &mut || Duration::ZERO).unwrap();
        assert!(
            matches!(read.result, Ok(Knowledge::Known { value, .. }) if value == receipt_case.outcome())
        );
        assert_eq!(files(&case.config.store), after);
    }
}

#[test]
fn outer_dispatcher_also_reaches_existing_baseline_stream_reconciliation() {
    use fa_reference::Snapshot;
    use fa_reference::action::consequence::delivery::{
        persistent::observed::stream::{FileStreamProposal, actor_wire::encode_stream_proposal},
        stream::StreamProfile,
    };
    use fa_reference::action::consequence::oversight::{
        actor_peer::PeerCredentials, actor_wire::ActorRequestPort,
    };
    use std::os::unix::net::UnixStream;
    for receipt_case in ReceiptCase::REFUSALS {
        let root = Root::new();
        let config = configured(&root);
        let stream = StreamProfile::new(7, 1, 4, 1024, 4096).unwrap();
        let (mut host, _human) =
            FileOversight::create_stream(&config.store, config.profile.clone(), stream).unwrap();
        host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal())
            .unwrap();
        host.observe_time(host.revision(), ElapsedTick(1000))
            .unwrap();
        let (port, mut supervisor) = host.into_stream_actor_gateway().unwrap();
        let revision = supervisor.host().unwrap().revision();
        let observed = Snapshot {
            semantic_epoch: 1,
            complete: true,
            values: BTreeMap::from([(7, b"blocked".to_vec())]),
        };
        supervisor
            .set_snapshot(revision, Some(observed.clone()))
            .unwrap();
        let proposal = encode_stream_proposal(
            stream,
            &FileStreamProposal {
                target: config.profile.delivery.target,
                expected_policy_epoch: 0,
                deadline: ElapsedTick(2000),
                message: Some("baseline".into()),
            },
        )
        .unwrap();
        if receipt_case == ReceiptCase::NotAdmitted {
            let mut host = supervisor.host_mut().unwrap();
            let spec = host
                .stream_message_spec("baseline", ElapsedTick(2000))
                .unwrap();
            let revision = host.revision();
            host.observe_time(revision, ElapsedTick(2000)).unwrap();
            let revision = host.revision();
            host.submit_request(revision, 7, spec, observed).unwrap();
        }
        let ticket = port.submit(7, &proposal).unwrap();
        assert!(
            matches!(port.poll(&ticket), Knowledge::Known { value, .. } if value == receipt_case.outcome())
        );
        if receipt_case == ReceiptCase::NotAdmitted {
            let host = supervisor.host().unwrap();
            assert_eq!(
                host.request_status(7).unwrap().disposition,
                FileRequestDisposition::NotAdmitted(Error::Stale)
            );
            assert_eq!(
                host.request_action(7).err(),
                Some(JournalError::Contract(Error::WrongState))
            );
        } else {
            receipt_case.assert_admission(&supervisor.host().unwrap(), 7);
        }
        let before = supervisor.host().unwrap().inspect();
        let anchor =
            fixture::anchor_file(&root, &supervisor.host().unwrap().history_anchor().unwrap());
        drop(supervisor);
        let (peer, _other) = UnixStream::pair().unwrap();
        let peer = PeerCredentials::observe(&peer).unwrap();
        let socket = root.0.join("peers/settlement.sock");
        let scope = config.profile.delivery.scope;
        let actor = root.0.join("settlement-actor.json");
        fs::write(&actor, format!(r#"{{"schema":"fa.actor-service/1","clock":"unix_milliseconds","request":7,"scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},"socket":"{}","supervisor":{{"uid":{},"gid":{},"pid":{}}},"actor":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":1,"connection_limit":1,"exchange_limit":1,"runtime_ms":2000,"poll_ms":1,"reply_ms":1}}"#,
            scope.tenant, scope.principal, scope.run, scope.branch, scope.authority, socket.display(),
            peer.uid(), peer.gid(), peer.pid(), peer.uid(), peer.gid(), peer.pid())).unwrap();
        let settings = root.0.join("stream-recovery.json");
        fs::write(&settings, format!(r#"{{"schema":"fa.stream-receipt-recovery/1","stream":{{"id":7,"generation":1,"max_messages":4,"max_message_bytes":1024,"max_stream_bytes":4096}},"registered_source":false,"minimum":{{"journal_revision":{},"control_sequence":{},"authority_epoch":{}}},"history_anchor":"{}"}}"#,
            before.revision, before.control.sequence, before.control.ledger.epoch, anchor.display())).unwrap();
        let args = vec![
            "serve-open".into(),
            fixture::config_file(&root, &config)
                .to_str()
                .unwrap()
                .into(),
            actor.to_str().unwrap().into(),
            "--stream-recovery".into(),
            settings.to_str().unwrap().into(),
            "--reconcile-receipts".into(),
        ];
        crate::workflow::actor_service::command(&args, None).unwrap();
        let after =
            FileOversight::read_stream_publication(&config.store, &config.profile, stream).unwrap();
        assert_eq!(after.publication.revision, before.revision + 3);
        assert_eq!(after.publication.executions, 0);
        assert_eq!(after.publication.control.ledger.charged, 0);
        assert!(!socket.exists());
        // A missing requested key reaches native admission but cannot spend a fence.
        fs::write(
            &actor,
            fs::read_to_string(&actor)
                .unwrap()
                .replace("\"request\":7", "\"request\":8"),
        )
        .unwrap();
        fs::write(
            config.store.join("delivery.pending"),
            b"missing key staging",
        )
        .unwrap();
        let original = files(&config.store);
        assert!(crate::workflow::actor_service::command(&args, None).is_err());
        assert_eq!(files(&config.store), original);
        assert!(!socket.exists());
    }
}
