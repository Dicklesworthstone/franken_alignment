//! Explicit service custody repair over the original canonical receipt triplet.
//! Actual publication and Unix peers remain the positive oracle; no effect is resent.
use super::custody_tests::{self as original, TopologyCase};
use super::*;

const REPAIR: &str = "--recover-receipt-checkpoint";

fn command(setup: &TopologyCase, name: &str, repair: bool) -> (Vec<String>, PathBuf, PathBuf) {
    let case = &setup.case;
    let history = case.root.0.join(format!("{name}.anchor"));
    let source = case.root.0.join(format!("{name}.checkpoint"));
    let mut args = original::args(
        setup,
        &case.anchor,
        &setup.profile,
        Some((&history, &source)),
        Mode::Reconcile,
    );
    if repair {
        args.insert(args.len() - 1, REPAIR.into());
    }
    (args, history, source)
}

fn canonical(case: &Case) -> Vec<u8> {
    fs::read(case.config.store.join("delivery.bin")).unwrap()
}

// Force both independent create-new exports to fail after original admission.
// The native replacement still acknowledges exactly one Fence/Time/Sweep.
fn lose_exports(setup: &TopologyCase, repair: bool) -> FileDeliverySnapshot {
    let case = &setup.case;
    let (args, history, source) = command(setup, "lost-service-receipt", repair);
    let clocks = Cell::new(0);
    let error = service::command_with(
        &args,
        None,
        || {
            clocks.set(clocks.get() + 1);
            fs::write(&history, b"raced history output").unwrap();
            fs::write(&source, b"raced source output").unwrap();
            ElapsedTick(1001)
        },
        &mut || Duration::ZERO,
        &mut |_| panic!("lost custody cannot serve a receipt"),
    )
    .unwrap_err();
    assert_eq!(clocks.get(), 1);
    assert!(error.contains("reconciliation acknowledged"), "{error}");
    assert!(error.contains("history anchor retention failed"), "{error}");
    assert!(
        error.contains("topology checkpoint retention failed"),
        "{error}"
    );
    assert_eq!(fs::read(&history).unwrap(), b"raced history output");
    assert_eq!(fs::read(&source).unwrap(), b"raced source output");
    assert!(!case.actor.socket.exists());
    let after = case.inspect();
    assert_eq!(after.revision, case.before.revision + 3);
    assert_eq!(after.executions, case.before.executions);
    assert_eq!(after.payload, case.before.payload);
    assert_eq!(
        after.control.ledger.charged,
        case.before.control.ledger.charged
    );
    after
}

fn serve_repair(
    setup: &TopologyCase,
    args: &[String],
    history: &Path,
    source: &Path,
    revision: u64,
) -> Vec<WireResponse> {
    let case = &setup.case;
    let done = Cell::new(false);
    let waits = Cell::new(0);
    let mut client = None;
    let mut replies = Vec::new();
    let report = service::command_with(
        args,
        None,
        || panic!("an existing canonical receipt triplet cannot read another logical clock"),
        &mut || {
            if done.get() {
                Duration::from_millis(100)
            } else {
                Duration::ZERO
            }
        },
        &mut |_| {
            waits.set(waits.get() + 1);
            assert!(waits.get() < 128);
            if waits.get() == 1 {
                original::pair(setup, history, source, revision);
                // Neither recovery lock nor a writable owner survives into exchange.
                drop(
                    FileOversight::begin_open_with_learned_generation(
                        &case.config.store,
                        case.config.profile.clone(),
                        &case.loaded.generation,
                    )
                    .unwrap(),
                );
            }
            exchange(case, &mut client, &mut replies, &done);
        },
    )
    .unwrap();
    assert_eq!(report.frames, 3);
    assert_eq!(report.candidates, 2);
    assert_eq!(replies[1].result, Err(WireError::Withheld));
    assert_eq!(replies[0].request, replies[2].request);
    assert_eq!(replies[0].result, replies[2].result);
    assert!(!case.actor.socket.exists());
    replies
}

#[test]
fn receipt_service_repairs_lost_exports_and_reconnects_without_another_event_in_all_modes() {
    for mode in [None, Some(false), Some(true)] {
        let setup = original::published(mode);
        let case = &setup.case;
        let saved_anchor = fs::read(&case.anchor).unwrap();
        let saved_source = fs::read(case.root.0.join("source-1.bin")).unwrap();
        // Empty-suffix repair is also a permitted first settlement, with one clock.
        let after = lose_exports(&setup, true);
        let journal = canonical(case);
        let (legacy, legacy_history, legacy_source) = command(&setup, "legacy-stale-pair", false);
        assert!(
            service::command_with(
                &legacy,
                None,
                || panic!("ordinary recovery cannot upgrade stale source custody"),
                &mut || Duration::ZERO,
                &mut |_| panic!("stale pair cannot serve")
            )
            .is_err()
        );
        assert_eq!(canonical(case), journal);
        assert!(!legacy_history.exists());
        assert!(!legacy_source.exists());
        for retry in 0..2 {
            let (args, history, source) =
                command(&setup, &format!("repaired-service-{retry}"), true);
            let replies = serve_repair(&setup, &args, &history, &source, after.revision);
            assert!(matches!(
                replies[0].result,
                Ok(Knowledge::Known {
                    value: ActorOutcome::Executed,
                    ..
                })
            ));
            assert_eq!(canonical(case), journal);
            assert_eq!(case.inspect(), after);
            assert_eq!(fs::read(&case.anchor).unwrap(), saved_anchor);
            assert_eq!(
                fs::read(case.root.0.join("source-1.bin")).unwrap(),
                saved_source
            );
            assert!(!case.root.0.join("live-1.json").exists());
            assert!(!case.root.0.join("inventory-1.json").exists());
            assert!(!case.root.0.join("native-roster.json").exists());
        }
    }
}

#[test]
fn receipt_service_repair_attempts_both_raced_exports_without_exposing_a_port() {
    for (block_history, block_source) in [(true, false), (false, true), (true, true)] {
        let setup = original::published(Some(true));
        let case = &setup.case;
        let after = lose_exports(&setup, false);
        let journal = canonical(case);
        let (args, history, source) = command(&setup, "repair-raced", true);
        let injected = Cell::new(false);
        let error = service::command_with(
            &args,
            None,
            || panic!("existing-triplet export retry cannot read time"),
            &mut || {
                // The original socket preflight occurs after path admission and
                // before native confirmation; this is an actual filesystem race.
                if case.actor.socket.exists() && !injected.replace(true) {
                    if block_history {
                        fs::write(&history, b"occupied repair history").unwrap();
                    }
                    if block_source {
                        fs::write(&source, b"occupied repair source").unwrap();
                    }
                }
                Duration::ZERO
            },
            &mut |_| panic!("a failed selected export cannot expose a receipt"),
        )
        .unwrap_err();
        assert!(injected.get());
        assert!(error.contains("reconciliation acknowledged"), "{error}");
        assert!(
            error.contains(&format!("revision {}", after.revision)),
            "{error}"
        );
        assert!(error.contains("no receipt was served"), "{error}");
        assert!(error.contains("no effect retry was sent"), "{error}");
        if block_history {
            assert!(error.contains("history anchor retention failed"), "{error}");
            assert_eq!(fs::read(&history).unwrap(), b"occupied repair history");
        } else {
            assert_eq!(
                history_anchors::load(&history).unwrap().revision(),
                after.revision
            );
        }
        if block_source {
            assert!(
                error.contains("topology checkpoint retention failed"),
                "{error}"
            );
            assert_eq!(fs::read(&source).unwrap(), b"occupied repair source");
        } else {
            original::same_source(&setup.original, &original::source(&source), after.revision);
        }
        assert_eq!(canonical(case), journal);
        assert_eq!(case.inspect(), after);
        assert!(!case.actor.socket.exists());
    }
}

#[test]
fn receipt_service_repair_retains_the_existing_cut_before_deadline_or_rejected_peer() {
    for peer_failure in [false, true] {
        let setup = original::published(Some(false));
        let case = &setup.case;
        let after = lose_exports(&setup, false);
        let journal = canonical(case);
        let (args, history, source) = command(&setup, "late-repair", true);
        if peer_failure {
            let path = Path::new(&args[5]);
            let text = fs::read_to_string(path).unwrap();
            let actor = case.actor.actor;
            let old = format!(
                "\"actor\":{{\"uid\":{},\"gid\":{},\"pid\":{}}}",
                actor.uid(),
                actor.gid(),
                actor.pid().unwrap()
            );
            let new = format!(
                "\"actor\":{{\"uid\":{},\"gid\":{},\"pid\":{}}}",
                if actor.uid() == 0 { 1 } else { 0 },
                actor.gid(),
                actor.pid().unwrap()
            );
            assert!(text.contains(&old));
            fs::write(
                path,
                text.replace(&old, &new)
                    .replace("\"candidate_limit\":4", "\"candidate_limit\":1")
                    .replace("\"connection_limit\":4", "\"connection_limit\":1"),
            )
            .unwrap();
        }
        let mut peer = None;
        let error = service::command_with(
            &args,
            None,
            || panic!("an existing triplet has no trusted clock call"),
            &mut || {
                if !peer_failure && history.exists() && source.exists() {
                    Duration::from_millis(100)
                } else {
                    Duration::ZERO
                }
            },
            &mut |_| {
                assert!(peer_failure);
                assert!(peer.is_none());
                original::pair(&setup, &history, &source, after.revision);
                let socket = UnixStream::connect(&case.actor.socket).unwrap();
                socket.set_nonblocking(true).unwrap();
                peer = Some(socket);
            },
        )
        .unwrap_err();
        assert!(error.contains("reconciliation acknowledged"), "{error}");
        assert!(error.contains("no effect retry was sent"), "{error}");
        if peer_failure {
            assert!(error.contains("CredentialsRejected"), "{error}");
            assert_eq!(peer.as_mut().unwrap().read(&mut [0; 1]).unwrap(), 0);
        } else {
            assert!(error.contains("startup deadline reached"), "{error}");
        }
        original::pair(&setup, &history, &source, after.revision);
        assert_eq!(canonical(case), journal);
        assert_eq!(case.inspect(), after);
        assert!(!case.actor.socket.exists());
    }
}

#[test]
fn receipt_service_repair_refuses_malformed_options_independent_floors_and_socket_collision() {
    let setup = original::published(None);
    let case = &setup.case;
    let after = lose_exports(&setup, false);
    let journal = canonical(case);
    let (good, history, source) = command(&setup, "admitted-repair", true);
    let mut malformed = Vec::new();
    let mut passive = good.clone();
    *passive.last_mut().unwrap() = "--receipts-only".into();
    malformed.push(passive);
    let mut duplicate = good.clone();
    duplicate.insert(duplicate.len() - 1, REPAIR.into());
    malformed.push(duplicate);
    let mut trailing = good.clone();
    trailing.retain(|arg| arg != REPAIR);
    trailing.push(REPAIR.into());
    malformed.push(trailing);
    for flag in [
        "--retain-history-anchor",
        "--retain-topology-checkpoint",
        "--topology-profile",
    ] {
        let mut missing = good.clone();
        let at = missing.iter().position(|arg| arg == flag).unwrap();
        missing.drain(at..at + 2);
        malformed.push(missing);
    }
    // Removing the repair boolean must not repair a missing preceding value.
    for flag in [
        "--history-anchor",
        "--retain-history-anchor",
        "--topology-profile",
        "--retain-topology-checkpoint",
        "--serve-receipts",
    ] {
        let mut absent = good.clone();
        absent.retain(|arg| arg != REPAIR);
        let at = absent.iter().position(|arg| arg == flag).unwrap();
        absent.insert(at + 1, REPAIR.into());
        malformed.push(absent);
    }
    let mut finish = good.clone();
    finish.retain(|arg| arg != REPAIR);
    let end = finish.len() - 1;
    finish.splice(
        end..end,
        ["--finish-after-message".into(), REPAIR.into(), "1".into()],
    );
    malformed.push(finish);
    for invalid in malformed {
        assert!(
            service::command_with(
                &invalid,
                None,
                || panic!("invalid grammar cannot read a logical clock"),
                &mut || panic!("invalid repair grammar must fail before configuration intake"),
                &mut |_| panic!("invalid repair grammar cannot serve")
            )
            .is_err()
        );
        assert_eq!(canonical(case), journal);
        assert!(!case.actor.socket.exists());
        assert!(!history.exists());
        assert!(!source.exists());
    }
    let original_recipe = fs::read_to_string(&case.recipe).unwrap();
    for (name, floor) in [
        ("journal_revision", after.revision + 1),
        ("control_sequence", after.control.sequence + 1),
        ("authority_epoch", after.control.ledger.epoch + 1),
    ] {
        let old = format!("\"{name}\":0");
        assert!(original_recipe.contains(&old));
        fs::write(
            &case.recipe,
            original_recipe.replacen(&old, &format!("\"{name}\":{floor}"), 1),
        )
        .unwrap();
        assert!(
            service::command_with(
                &good,
                None,
                || panic!("independent higher floors cannot be erased by repair"),
                &mut || Duration::ZERO,
                &mut |_| panic!("unmet floor cannot serve")
            )
            .is_err()
        );
        assert_eq!(canonical(case), journal);
        assert!(!case.actor.socket.exists());
        assert!(!history.exists());
        assert!(!source.exists());
    }
    fs::write(&case.recipe, original_recipe).unwrap();
    let mut wrong_finish = good.clone();
    let end = wrong_finish.len() - 1;
    wrong_finish.splice(end..end, ["--finish-after-message".into(), "99".into()]);
    assert!(
        service::command_with(
            &wrong_finish,
            None,
            || panic!("wrong finish cannot read time"),
            &mut || Duration::ZERO,
            &mut |_| panic!("wrong finish cannot serve")
        )
        .is_err()
    );
    assert_eq!(canonical(case), journal);
    assert!(!case.actor.socket.exists());
    fs::write(&case.actor.socket, b"occupied repair endpoint").unwrap();
    assert!(
        service::command_with(
            &good,
            None,
            || panic!("occupied socket cannot confirm or replace history"),
            &mut || Duration::ZERO,
            &mut |_| panic!("occupied socket cannot serve")
        )
        .is_err()
    );
    assert_eq!(
        fs::read(&case.actor.socket).unwrap(),
        b"occupied repair endpoint"
    );
    assert_eq!(canonical(case), journal);
    assert!(!history.exists());
    assert!(!source.exists());
    fs::remove_file(&case.actor.socket).unwrap();
    assert!(matches!(
        serve_repair(&setup, &good, &history, &source, after.revision)[0].result,
        Ok(Knowledge::Known {
            value: ActorOutcome::Executed,
            ..
        })
    ));
}

#[test]
fn receipt_service_repair_requires_canonical_confirmation_and_rejects_unrelated_suffixes() {
    let setup = original::published(Some(true));
    let case = &setup.case;
    let initial = canonical(case);
    let after = lose_exports(&setup, false);
    let journal = canonical(case);
    let (args, history, source) = command(&setup, "confirmed-repair", true);
    let pending = case.config.store.join("delivery.pending");
    // confirm_and_cleanup rejects nonregular staging. Its real filesystem path,
    // not the replacement-only fail_once seam, exercises this storage failure.
    fs::create_dir(&pending).unwrap();
    let error = service::command_with(
        &args,
        None,
        || panic!("confirmation-only repair cannot read time"),
        &mut || Duration::ZERO,
        &mut |_| panic!("unconfirmed canonical custody cannot serve"),
    )
    .unwrap_err();
    assert!(error.contains("finalization unconfirmed"), "{error}");
    assert!(pending.is_dir());
    assert_eq!(canonical(case), journal);
    assert!(!history.exists());
    assert!(!source.exists());
    assert!(!case.actor.socket.exists());
    fs::remove_dir(&pending).unwrap();
    let changed = Cell::new(false);
    let error = service::command_with(
        &args,
        None,
        || panic!("canonical conflict cannot read time"),
        &mut || {
            if case.actor.socket.exists() && !changed.replace(true) {
                fs::write(case.config.store.join("delivery.bin"), &initial).unwrap();
            }
            Duration::ZERO
        },
        &mut |_| panic!("changed canonical cut cannot issue a port"),
    )
    .unwrap_err();
    assert!(changed.get());
    assert!(error.contains("finalization unconfirmed"), "{error}");
    assert_eq!(canonical(case), initial);
    assert!(!history.exists());
    assert!(!source.exists());
    fs::write(case.config.store.join("delivery.bin"), &journal).unwrap();
    serve_repair(&setup, &args, &history, &source, after.revision);
    assert_eq!(canonical(case), journal);
    assert_eq!(case.inspect(), after);
    // Valid original extra work is not a repair suffix even though it spends no
    // topology checks. The selected old pair cannot be silently advanced over it.
    let (mut host, _) = FileOversight::open_with_learned_generation(
        &case.config.store,
        case.config.profile.clone(),
        &case.loaded.generation,
    )
    .unwrap();
    let revision = host.revision();
    host.observe_time(revision, ElapsedTick(1002)).unwrap();
    drop(host);
    let unrelated = canonical(case);
    let (invalid, later_history, later_source) = command(&setup, "unrelated-repair", true);
    fs::write(&pending, b"unrelated-suffix staging").unwrap();
    assert!(
        service::command_with(
            &invalid,
            None,
            || panic!("unrelated suffix cannot read time"),
            &mut || Duration::ZERO,
            &mut |_| panic!("unrelated suffix cannot serve")
        )
        .is_err()
    );
    assert_eq!(canonical(case), unrelated);
    assert_eq!(fs::read(&pending).unwrap(), b"unrelated-suffix staging");
    assert!(!later_history.exists());
    assert!(!later_source.exists());
    assert!(!case.actor.socket.exists());
}

#[test]
fn retained_receipt_service_refuses_archive_socket_collisions_before_recovery_in_all_modes() {
    for predictor in [None, Some(false), Some(true)] {
        let setup = original::published(predictor);
        let case = &setup.case;
        let before = case.files();
        let journal = canonical(case);
        let pending = case.config.store.join("delivery.pending");
        let staged = fs::read(&pending).unwrap();
        let alias = case.root.0.join("socket-alias");
        std::os::unix::fs::symlink(&case.root.0, &alias).unwrap();
        let aliased_socket = alias.join("peers/receipt.sock");
        for repair in [false, true] {
            for (output, flag) in ["--retain-history-anchor", "--retain-topology-checkpoint"]
                .into_iter()
                .enumerate()
            {
                // Test identical spelling, an aliased archive parent, and an
                // actor socket with an aliased ancestor but a real secure parent.
                for spelling in 0..3 {
                    let name = format!("socket-collision-{repair}-{output}-{spelling}");
                    let (mut args, history, source) = command(&setup, &name, repair);
                    let destination = if spelling == 1 {
                        &aliased_socket
                    } else {
                        &case.actor.socket
                    };
                    let at = args.iter().position(|arg| arg == flag).unwrap();
                    args[at + 1] = destination.display().to_string();
                    if spelling == 2 {
                        let path = Path::new(&args[5]);
                        let profile = fs::read_to_string(path).unwrap();
                        let original_socket =
                            format!("\"socket\":\"{}\"", case.actor.socket.display());
                        assert!(profile.contains(&original_socket));
                        fs::write(
                            path,
                            profile.replace(
                                &original_socket,
                                &format!("\"socket\":\"{}\"", aliased_socket.display()),
                            ),
                        )
                        .unwrap();
                    }
                    let error = service::command_with(
                        &args,
                        None,
                        || panic!("archive/socket collision cannot consume a recovery clock"),
                        &mut || Duration::ZERO,
                        &mut |_| panic!("archive/socket collision cannot serve a receipt"),
                    )
                    .unwrap_err();
                    assert!(error.contains("socket"), "{error}");
                    assert_eq!(case.files(), before);
                    assert_eq!(canonical(case), journal);
                    assert_eq!(fs::read(&pending).unwrap(), staged);
                    assert!(!case.actor.socket.exists());
                    assert!(!aliased_socket.exists());
                    assert!(!history.exists());
                    assert!(!source.exists());
                }
            }
        }
        // None of the refused paths spent the original cut. Distinct outputs
        // can still settle it exactly once and serve the authentic saved reply.
        let (args, history, source) = command(&setup, "distinct-service-custody", true);
        let replies = original::serve_command(
            &setup,
            &args,
            Mode::Reconcile,
            &history,
            &source,
            case.before.revision + 3,
        );
        assert!(matches!(
            replies[0].result,
            Ok(Knowledge::Known {
                value: ActorOutcome::Executed,
                ..
            })
        ));
        let after = case.inspect();
        assert_eq!(after.revision, case.before.revision + 3);
        assert_eq!(after.executions, case.before.executions);
        assert_eq!(after.payload, case.before.payload);
        assert_eq!(
            after.control.ledger.charged,
            case.before.control.ledger.charged
        );
        assert!(!pending.exists());
    }
}
