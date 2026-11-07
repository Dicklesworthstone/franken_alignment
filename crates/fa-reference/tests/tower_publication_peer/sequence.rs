//! Scheduled Tower work uses the original journal, source and peer session.
use super::*;
use fa_reference::action::ActionState;
use fa_reference::action::consequence::delivery::persistent::abandonment::FileAbandonRequest;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorProposal};

fn frame(mount: &Address, key: u64, proposal: &ActorProposal) -> Vec<u8> {
    let mut request = TowerRequest::new("scheduled-propose", mount.clone(), Verb::Propose);
    request.arguments = ReadArguments::Propose {
        request: key,
        proposal: proposal.clone(),
    };
    request.idempotency_key = Some(key.to_string());
    format!("{}\n", encode_request(&request).unwrap()).into_bytes()
}

fn exchange(
    file: &mut FileRig,
    peer: &mut PublicationPeerSession,
    socket: &mut UnixStream,
    document: &[u8],
    current: u64,
    completed: &[u64],
    new_source: bool,
) -> Json {
    socket.write_all(document).unwrap();
    let report = file
        .rig
        .driver
        .drive_tower_sequence_from_file(
            peer,
            current,
            completed,
            &mut file.source,
            || {
                assert!(new_source, "a retry or refusal acquired source/time");
                ElapsedTick(1)
            },
            DriveBudget {
                frames: 1,
                ..DriveBudget::default()
            },
        )
        .unwrap();
    assert_eq!(report.drive.progress.frames, 1);
    assert_eq!(report.intakes.len(), usize::from(new_source));
    let mut values = replies(socket);
    assert_eq!(values.len(), 1);
    values.remove(0)
}

fn native(reply: &Json) -> &Json {
    let result = reply.get("result").unwrap();
    assert_eq!(
        result.get("kind").and_then(Json::as_str),
        Some("publication-request")
    );
    result.get("value").unwrap()
}

fn cancelled(file: &mut FileRig, request: u64, proposal: &ActorProposal) {
    let ticket = file.rig.port.submit(request, proposal).unwrap();
    file.rig.port.cancel(&ticket).unwrap();
    assert!(matches!(
        file.rig.port.poll(&ticket),
        Knowledge::Known {
            value: ActorOutcome::CancelledBeforeDispatch,
            ..
        }
    ));
    assert_eq!(file.rig.driver.retire_completed_request(request), Ok(true));
}

#[test]
fn successive_tower_keys_preserve_one_owner_session_and_acquire_fresh_source() {
    let mut file = cold(10, StateLimits::default());
    let (mut peer, mount, _) = session(&file, 8, policy());
    let mut socket = attach(&mut peer);
    let first = file.rig.proposal();
    let first_frame = frame(&mount, 1, &first);
    let reply = exchange(
        &mut file,
        &mut peer,
        &mut socket,
        &first_frame,
        1,
        &[],
        true,
    );
    assert_eq!(
        native(&reply)
            .get("knowledge")
            .unwrap()
            .get("state")
            .and_then(Json::as_str),
        Some("pending")
    );
    cancelled(&mut file, 1, &first);
    let original = file.rig.driver.supervisor().host().unwrap().inspect();
    let second = file.rig.proposal();
    let second_frame = frame(&mount, 2, &second);
    let reply = exchange(
        &mut file,
        &mut peer,
        &mut socket,
        &second_frame,
        2,
        &[1],
        true,
    );
    assert_eq!(
        native(&reply).get("request").and_then(Json::as_str),
        Some("2")
    );
    cancelled(&mut file, 2, &second);
    let current = file.rig.driver.supervisor().host().unwrap().inspect();
    assert_eq!(file.source.status().read_attempts, 2);
    assert_eq!(current.control.ledger.stages.len(), 2);
    assert_eq!(current.control.ledger.epoch, original.control.ledger.epoch);
    assert_eq!(current.dispatcher_epoch, original.dispatcher_epoch);
    assert_eq!(
        current.control.ledger.available,
        original.control.ledger.available
    );
    assert_eq!(current.control.ledger.charged, 0);
    assert_eq!(current.control.ledger.reserved, 0);
    assert_eq!(current.executions, 0);
    assert_eq!(peer.status().connections_admitted, 1);
    assert_eq!(peer.remaining_exchanges(), 6);
    assert_eq!(
        FileOversight::read_publication(file.rig.root.store(), &driver::profile()).unwrap(),
        current
    );
}

#[test]
fn completed_retry_conflict_and_reconnect_keep_source_and_session_budgets() {
    let mut file = cold(10, StateLimits::default());
    let (mut peer, mount, _) = session(&file, 5, policy());
    let mut socket = attach(&mut peer);
    let first = file.rig.proposal();
    let first_frame = frame(&mount, 1, &first);
    exchange(
        &mut file,
        &mut peer,
        &mut socket,
        &first_frame,
        1,
        &[],
        true,
    );
    cancelled(&mut file, 1, &first);
    let before = image(&file);
    std::fs::remove_file(&file.path).unwrap();
    assert!(peer.disconnect());
    drop(socket);
    let mut socket = attach(&mut peer);
    let retried = exchange(
        &mut file,
        &mut peer,
        &mut socket,
        &first_frame,
        2,
        &[1],
        false,
    );
    assert_eq!(
        native(&retried)
            .get("knowledge")
            .unwrap()
            .get("value")
            .and_then(Json::as_str),
        Some("cancelled_before_dispatch")
    );
    let mut changed = first.clone();
    changed.payload[0] ^= 1;
    let conflict = exchange(
        &mut file,
        &mut peer,
        &mut socket,
        &frame(&mount, 1, &changed),
        2,
        &[1],
        false,
    );
    assert_eq!(
        native(&conflict).get("reason").and_then(Json::as_str),
        Some("idempotency_conflict")
    );
    let current = frame(&mount, 2, &first);
    socket.write_all(&current).unwrap();
    let observed = file
        .rig
        .driver
        .drive_tower_sequence_observe(&mut peer, 2, &[1], DriveBudget::default())
        .unwrap();
    assert_eq!(observed.progress.frames, 1);
    assert_eq!(
        native(&replies(&mut socket)[0])
            .get("reason")
            .and_then(Json::as_str),
        Some("unavailable")
    );
    let future = exchange(
        &mut file,
        &mut peer,
        &mut socket,
        &frame(&mount, 3, &first),
        2,
        &[1],
        false,
    );
    assert_eq!(
        native(&future).get("reason").and_then(Json::as_str),
        Some("withheld")
    );
    assert_eq!(image(&file), before);
    assert_eq!(file.source.status().read_attempts, 1);
    assert_eq!(peer.status().connections_admitted, 2);
    assert_eq!(peer.remaining_exchanges(), 0);
    assert!(peer.disconnect());
    let (server, _client) = UnixStream::pair().unwrap();
    assert_eq!(peer.attach(server), Err(PeerRefusal::Capacity));
}

#[test]
fn pending_predecessor_cannot_admit_buffered_future_work_until_original_settlement() {
    let mut file = cold(10, StateLimits::default());
    let (mut peer, mount, _) = session(&file, 8, policy());
    let mut socket = attach(&mut peer);
    let first = file.rig.proposal();
    exchange(
        &mut file,
        &mut peer,
        &mut socket,
        &frame(&mount, 1, &first),
        1,
        &[],
        true,
    );
    let second = file.rig.proposal();
    socket.write_all(&frame(&mount, 2, &second)).unwrap();
    let before = image(&file);
    assert!(
        file.rig
            .driver
            .drive_tower_sequence_from_file(
                &mut peer,
                2,
                &[1],
                &mut file.source,
                || panic!("pending predecessor acquired source"),
                DriveBudget::default(),
            )
            .is_err()
    );
    assert_eq!(peer.remaining_exchanges(), 7);
    assert_eq!(peer.status().transport.unwrap().buffered_input_bytes, 0);
    assert_eq!(image(&file), before);
    assert_eq!(file.source.status().read_attempts, 1);
    cancelled(&mut file, 1, &first);
    // The same waiting bytes are now admitted through the same session only
    // after actual native settlement, not a caller's claimed completed prefix.
    let report = file
        .rig
        .driver
        .drive_tower_sequence_from_file(
            &mut peer,
            2,
            &[1],
            &mut file.source,
            || ElapsedTick(1),
            DriveBudget::default(),
        )
        .unwrap();
    assert_eq!(report.drive.progress.frames, 1);
    assert_eq!(report.intakes.len(), 1);
    assert_eq!(
        native(&replies(&mut socket)[0])
            .get("request")
            .and_then(Json::as_str),
        Some("2")
    );
    assert_eq!(file.source.status().read_attempts, 2);
    cancelled(&mut file, 2, &second);
}

#[test]
fn dispatch_unknown_and_abandoned_liabilities_never_count_as_completed() {
    let mut file = cold(10, StateLimits::default());
    let (mut peer, mount, _) = session(&file, 8, policy());
    let mut socket = attach(&mut peer);
    let first = file.rig.proposal();
    exchange(
        &mut file,
        &mut peer,
        &mut socket,
        &frame(&mount, 1, &first),
        1,
        &[],
        true,
    );
    file.reviewed();
    let human = file.human();
    file.dispatch(&human);
    socket.write_all(b"{}\n").unwrap();
    for stage in [
        ActionState::Dispatching,
        ActionState::Unknown,
        ActionState::IrrecoverablyUnknown,
    ] {
        let before = file.rig.driver.supervisor().host().unwrap().inspect();
        assert_eq!(before.control.ledger.stages.get(&1), Some(&stage));
        assert_eq!(before.control.ledger.charged, first.units);
        let bytes = image(&file);
        assert!(
            file.rig
                .driver
                .drive_tower_sequence_observe(&mut peer, 2, &[1], DriveBudget::default(),)
                .is_err()
        );
        assert_eq!(image(&file), bytes);
        assert_eq!(peer.remaining_exchanges(), 7);
        assert_eq!(peer.status().transport.unwrap().buffered_input_bytes, 0);
        let mut host = file.rig.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        match stage {
            ActionState::Dispatching => {
                host.fence(revision).unwrap();
            }
            ActionState::Unknown => {
                let control = host.inspect().control;
                host.abandon_unknown(
                    revision,
                    FileAbandonRequest {
                        operation: 900,
                        attempt: 1,
                        expected_control_sequence: control.sequence,
                        expected_authority_epoch: control.ledger.epoch,
                    },
                )
                .unwrap();
            }
            ActionState::IrrecoverablyUnknown => {}
            _ => unreachable!(),
        }
    }
}

#[test]
fn invalid_prefix_foreign_owner_and_drive_budget_precede_socket_or_clock_work() {
    let mut file = cold(10, StateLimits::default());
    let mut foreign = cold(10, StateLimits::default());
    let (mut peer, _, _) = session(&file, 8, policy());
    let mut socket = attach(&mut peer);
    socket.write_all(b"{}\n").unwrap();
    let before = image(&file);
    for (current, completed) in [
        (0, vec![]),
        (1, vec![0]),
        (1, vec![1]),
        (1, vec![2, 2]),
        (1, (100..164).collect()),
    ] {
        assert!(
            file.rig
                .driver
                .drive_tower_sequence_from_file(
                    &mut peer,
                    current,
                    &completed,
                    &mut file.source,
                    || panic!("invalid schedule acquired source"),
                    DriveBudget::default(),
                )
                .is_err()
        );
    }
    assert!(
        foreign
            .rig
            .driver
            .drive_tower_sequence_observe(&mut peer, 1, &[], DriveBudget::default(),)
            .is_err()
    );
    assert!(
        file.rig
            .driver
            .drive_tower_sequence_observe(
                &mut peer,
                1,
                &[],
                DriveBudget {
                    frames: usize::MAX,
                    ..DriveBudget::default()
                },
            )
            .is_err()
    );
    assert_eq!(image(&file), before);
    assert_eq!(file.source.status().read_attempts, 0);
    assert_eq!(peer.remaining_exchanges(), 8);
    assert_eq!(peer.status().transport.unwrap().buffered_input_bytes, 0);
    let read = file
        .rig
        .driver
        .drive_tower_sequence_observe(&mut peer, 1, &[], DriveBudget::default())
        .unwrap();
    assert_eq!(read.progress.frames, 1);
    assert_eq!(peer.remaining_exchanges(), 7);
    assert_eq!(replies(&mut socket).len(), 1);
}
