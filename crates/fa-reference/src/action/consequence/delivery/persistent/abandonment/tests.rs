//! Reuse the original publication fixtures, reducer and storage failpoints.
//! These are bounded local-journal laws, not remote execution-outcome evidence.
use super::*;
use crate::action::consequence::delivery::persistent::requests::FileRequestDisposition;

fn request(host: &FileDelivery, operation: u64, attempt: u64) -> FileAbandonRequest {
    let control = host.inspect().control;
    FileAbandonRequest {
        operation,
        attempt,
        expected_control_sequence: control.sequence,
        expected_authority_epoch: control.ledger.epoch,
    }
}

fn unknown(host: &mut FileDelivery, attempt: u64) {
    dispatch(host, attempt, b"unsent");
    assert_eq!(
        host.reconcile(host.revision(), attempt).unwrap(),
        Reconciliation::AwaitingResolution,
    );
    assert_eq!(
        host.inspect().control.ledger.stages[&attempt],
        ActionState::Unknown
    );
}

fn canonical(host: &FileDelivery) -> Vec<u8> {
    std::fs::read(host.store.identity().join(storage::CANONICAL)).unwrap()
}

#[test]
fn explicit_abandonment_is_durable_and_does_not_refund_or_hide_the_liability() {
    let root = Directory::new();
    let mut host = create(&root);
    unknown(&mut host, 1);
    let before = host.inspect();
    let original = host.machine.actions[&1].clone();
    let input = request(&host, 501, 1);
    let receipt = host.abandon_unknown(host.revision(), input).unwrap();
    assert_eq!(receipt.request(), input);
    assert_eq!(receipt.action(), &original);
    assert_eq!(receipt.charged_units(), 16);
    assert_eq!(host.abandonment_receipt(501), Ok(&receipt));
    assert_eq!(host.inspect().revision, before.revision + 1);
    assert_eq!(
        host.inspect().control.ledger.stages[&1],
        ActionState::IrrecoverablyUnknown
    );
    assert_eq!(host.inspect().control.ledger.available, 84);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().control.sequence, before.control.sequence);
    assert_eq!(
        host.inspect().control.ledger.epoch,
        before.control.ledger.epoch
    );
    assert_eq!(host.inspect().payload, before.payload);
    assert_eq!(host.inspect().executions, before.executions);
    assert_eq!(
        host.inspect().control.ledger.elapsed,
        before.control.ledger.elapsed
    );

    // A lookup miss had left a live local envelope. Abandonment withdraws that
    // handle without inventing an endpoint seal or a later execution.
    let cut = host.inspect();
    let bytes = canonical(&host);
    assert_eq!(host.publish(host.revision(), 1), Err(Error::Missing.into()));
    assert_eq!(
        host.seal_unexecuted(host.revision(), 1),
        Err(Error::WrongState.into())
    );
    assert_eq!(
        host.cancel(host.revision(), 1),
        Err(Error::WrongState.into())
    );
    assert_eq!(host.inspect(), cut);
    assert_eq!(canonical(&host), bytes);
    assert!(host.reconcile_pending(host.revision()).unwrap().is_empty());
    assert_eq!(
        host.inspect().control.ledger.stages[&1],
        ActionState::IrrecoverablyUnknown
    );
    assert_eq!(host.inspect().control.ledger.charged, 16);

    let current = host.inspect().control;
    let stop = StopRequest {
        operation: 601,
        expected_control_sequence: current.sequence,
        expected_authority_epoch: current.ledger.epoch,
    };
    host.request_stop(host.revision(), stop).unwrap();
    let sweep = host.progress_stop(host.revision(), ElapsedTick(2)).unwrap();
    assert!(sweep.outcomes.is_empty());
    assert_eq!(sweep.progress.unresolved, vec![1]);
    assert_eq!(sweep.progress.irrecoverable, vec![1]);
    assert_eq!(sweep.progress.charged_units, 16);
    assert!(!sweep.progress.drained());
    assert_eq!(
        FileDelivery::read_publication(root.store(), &profile()).unwrap(),
        host.inspect()
    );

    drop(host);
    let mut recovered = FileDelivery::open(root.store(), profile()).unwrap();
    assert!(!recovered.clock_ready());
    let cut = recovered.inspect();
    assert_eq!(recovered.abandonment_receipt(501), Ok(&receipt));
    assert_eq!(
        recovered.abandon_unknown(recovered.revision(), input),
        Ok(receipt)
    );
    assert_eq!(recovered.inspect(), cut);
    assert!(!recovered.stop_progress().unwrap().drained());
    assert_eq!(recovered.stop_progress().unwrap().irrecoverable, vec![1]);
}

#[test]
fn executed_unknown_keeps_its_publication_and_charge_while_normal_reconciliation_still_works() {
    for abandon in [false, true] {
        let root = Directory::new();
        let mut host = create(&root);
        dispatch(&mut host, 1, b"visible");
        assert_eq!(
            host.publish(host.revision(), 1).unwrap(),
            EndpointOutcome::Executed {
                resulting_version: 2
            }
        );
        // Recovery marks the unaccepted dispatch Unknown and withdraws time.
        // Explicit trusted disposition needs no fabricated current clock.
        drop(host);
        let mut host = FileDelivery::open(root.store(), profile()).unwrap();
        assert!(!host.clock_ready());
        assert_eq!(
            host.inspect().control.ledger.stages[&1],
            ActionState::Unknown
        );
        let before = host.inspect();
        if abandon {
            let input = request(&host, 502, 1);
            let receipt = host.abandon_unknown(host.revision(), input).unwrap();
            assert_eq!(receipt.charged_units(), 16);
            assert_eq!(
                host.inspect().control.ledger.stages[&1],
                ActionState::IrrecoverablyUnknown
            );
            assert!(!host.clock_ready());
            assert_eq!(
                host.inspect().control.ledger.elapsed,
                before.control.ledger.elapsed
            );
            host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
            // Even a later real outcome cannot silently rewrite this explicit
            // terminal disposition through the ordinary receipt API.
            assert_eq!(
                host.reconcile(host.revision(), 1),
                Err(Error::WrongState.into())
            );
        } else {
            host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
            assert_eq!(
                host.reconcile(host.revision(), 1).unwrap(),
                Reconciliation::Resolved(EndpointOutcome::Executed {
                    resulting_version: 2
                })
            );
            assert_eq!(
                host.inspect().control.ledger.stages[&1],
                ActionState::Confirmed
            );
            let input = request(&host, 502, 1);
            let bytes = canonical(&host);
            assert_eq!(
                host.abandon_unknown(host.revision(), input),
                Err(Error::WrongState.into())
            );
            assert_eq!(canonical(&host), bytes);
        }
        assert_eq!(host.inspect().payload, b"visible");
        assert_eq!(host.inspect().executions, 1);
        assert_eq!(host.inspect().target, before.target);
        assert_eq!(host.inspect().control.ledger.available, 84);
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(
            FileDelivery::read_publication(root.store(), &profile()).unwrap(),
            host.inspect()
        );
    }
}

#[test]
fn exact_operation_retry_requires_a_current_journal_cut_but_not_an_old_epoch() {
    let root = Directory::new();
    let mut host = create(&root);
    unknown(&mut host, 1);
    let input = request(&host, 503, 1);
    let before = host.inspect();
    let bytes = canonical(&host);
    for (candidate, error) in [
        (
            FileAbandonRequest {
                operation: 0,
                ..input
            },
            Error::InvalidInput,
        ),
        (
            FileAbandonRequest {
                attempt: 0,
                ..input
            },
            Error::InvalidInput,
        ),
        (
            FileAbandonRequest {
                attempt: 999,
                ..input
            },
            Error::Missing,
        ),
        (
            FileAbandonRequest {
                expected_control_sequence: input.expected_control_sequence + 1,
                ..input
            },
            Error::Stale,
        ),
        (
            FileAbandonRequest {
                expected_authority_epoch: input.expected_authority_epoch + 1,
                ..input
            },
            Error::Stale,
        ),
    ] {
        assert_eq!(
            host.abandon_unknown(host.revision(), candidate),
            Err(error.into())
        );
        assert_eq!(host.inspect(), before);
        assert_eq!(canonical(&host), bytes);
    }
    assert_eq!(
        host.abandon_unknown(host.revision() - 1, input),
        Err(Error::Stale.into())
    );
    let receipt = host.abandon_unknown(host.revision(), input).unwrap();
    let after = host.inspect();
    let bytes = canonical(&host);
    assert_eq!(
        host.abandon_unknown(before.revision, input),
        Err(Error::Stale.into())
    );
    assert_eq!(
        host.abandon_unknown(host.revision(), input),
        Ok(receipt.clone())
    );
    for changed in [
        FileAbandonRequest {
            attempt: 999,
            ..input
        },
        FileAbandonRequest {
            expected_control_sequence: input.expected_control_sequence + 1,
            ..input
        },
        FileAbandonRequest {
            expected_authority_epoch: input.expected_authority_epoch + 1,
            ..input
        },
    ] {
        assert_eq!(
            host.abandon_unknown(host.revision(), changed),
            Err(Error::Binding.into())
        );
    }
    assert_eq!(
        host.abandon_unknown(
            host.revision(),
            FileAbandonRequest {
                operation: 504,
                ..input
            }
        ),
        Err(Error::WrongState.into())
    );
    assert_eq!(host.inspect(), after);
    assert_eq!(canonical(&host), bytes);
    host.fence(host.revision()).unwrap();
    assert!(host.inspect().control.ledger.epoch > input.expected_authority_epoch);
    let fenced = host.inspect();
    assert_eq!(host.abandon_unknown(host.revision(), input), Ok(receipt));
    assert_eq!(host.inspect(), fenced);
    assert_eq!(host.abandonment_receipt(999), Err(Error::Missing.into()));
}

#[test]
fn a_dispatch_needs_an_original_unknown_transition_before_abandonment() {
    let root = Directory::new();
    let mut host = create(&root);
    dispatch(&mut host, 1, b"visible");
    let input = request(&host, 505, 1);
    let before = host.inspect();
    let bytes = canonical(&host);
    assert_eq!(
        host.abandon_unknown(host.revision(), input),
        Err(Error::WrongState.into())
    );
    assert_eq!(host.inspect(), before);
    assert_eq!(canonical(&host), bytes);
    assert_eq!(
        host.reconcile(host.revision(), 1).unwrap(),
        Reconciliation::AwaitingResolution
    );
    assert!(host.abandon_unknown(host.revision(), input).is_ok());
}

#[test]
fn durable_request_projection_keeps_unknown_and_refuses_cancellation_after_abandonment() {
    let root = Directory::new();
    let mut host = create(&root);
    let spec = ActionSpec {
        version: VERSION,
        scope: profile().scope,
        target: Some(host.inspect().target),
        payload: b"visible".to_vec(),
        required_witnesses: Vec::new(),
        policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(100),
        units: 16,
    };
    let status = host
        .submit_request(host.revision(), 700, spec.clone(), snapshot())
        .unwrap();
    let FileRequestDisposition::Admitted { attempt, .. } = status.disposition else {
        panic!("positive request admission");
    };
    let action = host.request_action(700).unwrap().clone();
    host.review(
        host.revision(),
        ReferenceReview {
            attempt,
            round: 100,
            evidence_root: [9; 32],
            snapshot: snapshot(),
            ballots: BTreeMap::from([(
                "reviewer".to_owned(),
                ReferenceBallot {
                    verdict: Verdict::Allow,
                    salt: b"reference-salt".to_vec(),
                },
            )]),
        },
    )
    .unwrap();
    let key = host
        .authorize(host.revision(), attempt, snapshot())
        .unwrap();
    host.dispatch(host.revision(), &key, &action, snapshot())
        .unwrap();
    host.reconcile(host.revision(), attempt).unwrap();
    let unknown = host.request_status(700).unwrap();
    let input = request(&host, 506, attempt);
    host.abandon_unknown(host.revision(), input).unwrap();
    let abandoned = host.request_status(700).unwrap();
    assert_eq!(
        abandoned.disposition,
        FileRequestDisposition::Admitted {
            attempt,
            stage: ActionState::IrrecoverablyUnknown
        }
    );
    // Both stages stay in the actor's existing Unknown class. No new actor
    // terminal success or cancellation is manufactured by a trusted disposition.
    assert_eq!(abandoned.generation, unknown.generation);
    assert_eq!(host.request_resolution(700), Ok(None));
    let cut = host.inspect();
    let bytes = canonical(&host);
    assert_eq!(
        host.cancel_and_resolve_request(host.revision(), 700, ElapsedTick(2)),
        Err(Error::WrongState.into())
    );
    assert_eq!(host.inspect(), cut);
    assert_eq!(canonical(&host), bytes);
    assert_eq!(host.submit_request(0, 700, spec, snapshot()), Ok(abandoned));
    drop(host);
    let recovered = FileDelivery::open(root.store(), profile()).unwrap();
    assert_eq!(recovered.request_status(700), Ok(abandoned));
    assert_eq!(recovered.request_resolution(700), Ok(None));
    assert_eq!(recovered.inspect().control.ledger.charged, 16);
}

#[test]
fn abandonment_cannot_spend_the_existing_terminal_event_reserve() {
    for ordinary_slot in [false, true] {
        let root = Directory::new();
        let mut p = profile();
        p.limits.events = if ordinary_slot { 13 } else { 12 };
        let mut host = FileDelivery::create(root.store(), p.clone()).unwrap();
        host.enable_recovery_reserve(0, RecoveryReserve::terminal())
            .unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        unknown(&mut host, 1);
        while host.revision() < 9 {
            host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        }
        let input = request(&host, 507, 1);
        let before = host.inspect();
        let bytes = canonical(&host);
        if ordinary_slot {
            assert!(host.abandon_unknown(host.revision(), input).is_ok());
            assert_eq!(host.journal_capacity().unwrap().remaining().events, 3);
        } else {
            assert_eq!(
                host.abandon_unknown(host.revision(), input),
                Err(Error::Limit.into())
            );
            assert_eq!(host.inspect(), before);
            assert_eq!(canonical(&host), bytes);
        }
        // Existing terminal work still fits; only the near-identical branch
        // without explicit abandonment may settle the original endpoint key.
        let current = host.inspect().control;
        let stop = StopRequest {
            operation: 607,
            expected_control_sequence: current.sequence,
            expected_authority_epoch: current.ledger.epoch,
        };
        host.request_stop(host.revision(), stop).unwrap();
        host.fence(host.revision()).unwrap();
        let sweep = host.progress_stop(host.revision(), ElapsedTick(2)).unwrap();
        assert_eq!(host.journal_capacity().unwrap().remaining().events, 0);
        assert_eq!(sweep.progress.drained(), !ordinary_slot);
        assert_eq!(
            sweep.progress.charged_units,
            if ordinary_slot { 16 } else { 0 }
        );
        assert_eq!(
            FileDelivery::read_publication(root.store(), &p).unwrap(),
            host.inspect()
        );
    }
}

#[test]
fn canonical_extension_preserves_legacy_bytes_and_rejects_duplicates_and_malformed_inputs() {
    let root = Directory::new();
    let mut host = create(&root);
    unknown(&mut host, 1);
    let input = request(&host, 508, 1);
    let legacy = canonical(&host);
    let parsed_legacy = codec::decode(&host.profile, host.store.identity(), &legacy).unwrap();
    assert_eq!(
        codec::encode(&host.profile, host.store.identity(), &parsed_legacy).unwrap(),
        legacy
    );
    assert_eq!(&legacy[..8], b"FADLGR\0\x01");
    host.abandon_unknown(host.revision(), input).unwrap();
    let bytes = canonical(&host);
    let mut expected_record = vec![0, 0, 0, 33, 18];
    for value in [
        input.operation,
        input.attempt,
        input.expected_control_sequence,
        input.expected_authority_epoch,
    ] {
        expected_record.extend_from_slice(&value.to_be_bytes());
    }
    assert_eq!(
        &bytes[bytes.len() - expected_record.len()..],
        &expected_record
    );
    assert_eq!(bytes.len(), legacy.len() + 37);
    let parsed = codec::decode(&host.profile, host.store.identity(), &bytes).unwrap();
    let replayed = Machine::replay(&host.profile, &parsed).unwrap();
    assert_eq!(
        replayed.abandonments.receipt(input.operation),
        host.machine.abandonments.receipt(input.operation)
    );

    // Framing alone is insufficient: the original reducer rejects another
    // canonical copy, even though the public exact-retry API is read-only.
    let mut repeated = parsed;
    repeated.push(Event::AbandonUnknown(input));
    let repeated_bytes = codec::encode(&host.profile, host.store.identity(), &repeated).unwrap();
    let repeated = codec::decode(&host.profile, host.store.identity(), &repeated_bytes).unwrap();
    assert!(matches!(
        Machine::replay(&host.profile, &repeated),
        Err(Error::Duplicate)
    ));
    let mut impossible = parsed_legacy;
    impossible.push(Event::AbandonUnknown(FileAbandonRequest {
        expected_authority_epoch: input.expected_authority_epoch + 1,
        ..input
    }));
    let impossible_bytes =
        codec::encode(&host.profile, host.store.identity(), &impossible).unwrap();
    let impossible =
        codec::decode(&host.profile, host.store.identity(), &impossible_bytes).unwrap();
    assert!(matches!(
        Machine::replay(&host.profile, &impossible),
        Err(Error::Stale)
    ));

    for cut in bytes.len() - 37..bytes.len() {
        assert!(codec::decode(&host.profile, host.store.identity(), &bytes[..cut]).is_err());
    }
    let mut unknown_tag = bytes.clone();
    let tag_offset = bytes.len() - 33;
    unknown_tag[tag_offset] = 255;
    assert!(matches!(
        codec::decode(&host.profile, host.store.identity(), &unknown_tag),
        Err(Error::InvalidInput)
    ));
    let mut zero_operation = bytes;
    zero_operation[tag_offset + 1..tag_offset + 9].fill(0);
    assert!(matches!(
        codec::decode(&host.profile, host.store.identity(), &zero_operation),
        Err(Error::InvalidInput)
    ));
}

#[test]
fn abandonment_bytes_obey_the_ordinary_limit_before_spending_a_terminal_tail() {
    let root = Directory::new();
    let mut host = FileDelivery::create(root.store(), profile()).unwrap();
    host.enable_recovery_reserve(0, RecoveryReserve::terminal())
        .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    unknown(&mut host, 1);
    let input = request(&host, 509, 1);
    let mut p = host.profile.clone();
    let legacy_len = canonical(&host).len();
    p.limits.bytes = legacy_len + RecoveryReserve::terminal().bytes + 36;
    // The changed numeric byte ceiling has the same fixed-width bootstrap
    // encoding. The new 37-byte record misses ordinary capacity by one byte.
    assert_eq!(
        codec::encode(&p, host.store.identity(), &host.events)
            .unwrap()
            .len(),
        legacy_len
    );
    assert_eq!(
        codec::encode_appended(
            &p,
            host.store.identity(),
            &host.events,
            &Event::AbandonUnknown(input)
        ),
        Err(Error::Limit)
    );
    p.limits.bytes += 1;
    let exact = codec::encode_appended(
        &p,
        host.store.identity(),
        &host.events,
        &Event::AbandonUnknown(input),
    )
    .unwrap();
    assert_eq!(
        exact.len(),
        p.limits.bytes - RecoveryReserve::terminal().bytes
    );
    let decoded = codec::decode(&p, host.store.identity(), &exact).unwrap();
    assert!(Machine::replay(&p, &decoded).is_ok());
}

#[test]
fn storage_faults_never_return_a_candidate_disposition_and_recovery_uses_actual_bytes() {
    for barrier in BARRIERS {
        let root = Directory::new();
        let mut host = create(&root);
        unknown(&mut host, 1);
        let input = request(&host, 510, 1);
        let before = host.inspect();
        host.store.fail_once(barrier);
        let error = host.abandon_unknown(host.revision(), input).unwrap_err();
        let JournalError::Io(failure) = error else {
            panic!("expected storage failure");
        };
        assert_eq!(failure.operation, barrier);
        assert_eq!(host.inspect(), before);
        assert_eq!(
            host.abandon_unknown(host.revision(), input),
            Err(JournalError::Unavailable)
        );
        assert_eq!(
            host.abandonment_receipt(input.operation),
            Err(JournalError::Unavailable)
        );
        let disk = FileDelivery::read_publication(root.store(), &profile()).unwrap();
        let replaced = barrier == JournalIo::DirectorySync;
        assert_eq!(disk.revision, before.revision + u64::from(replaced));
        assert_eq!(
            disk.control.ledger.stages[&1],
            if replaced {
                ActionState::IrrecoverablyUnknown
            } else {
                ActionState::Unknown
            }
        );
        assert_eq!(disk.control.ledger.charged, 16);
        assert_eq!(disk.control.ledger.available, 84);
        assert_eq!(disk.payload, before.payload);
        assert_eq!(disk.executions, 0);
        drop(host);
        let mut recovered = FileDelivery::open(root.store(), profile()).unwrap();
        let cut = recovered.inspect();
        if replaced {
            let receipt = recovered
                .abandonment_receipt(input.operation)
                .unwrap()
                .clone();
            assert_eq!(receipt.request(), input);
            assert_eq!(
                recovered.abandon_unknown(recovered.revision(), input),
                Ok(receipt)
            );
            assert_eq!(recovered.inspect(), cut);
        } else {
            assert_eq!(
                recovered.abandonment_receipt(input.operation),
                Err(Error::Missing.into())
            );
            assert_eq!(
                recovered.abandon_unknown(recovered.revision(), input),
                Err(Error::Stale.into())
            );
            assert_eq!(recovered.inspect(), cut);
            // The verified uncommitted branch can still follow its original
            // positive endpoint reconciliation instead of being auto-abandoned.
            recovered
                .observe_time(recovered.revision(), ElapsedTick(2))
                .unwrap();
            assert_eq!(
                recovered.seal_unexecuted(recovered.revision(), 1).unwrap(),
                Reconciliation::Resolved(EndpointOutcome::NotExecuted {
                    reason: NonExecutionReason::Sealed
                })
            );
            assert_eq!(recovered.inspect().control.ledger.charged, 0);
        }
    }
}
