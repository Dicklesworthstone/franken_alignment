//! Finite recovery through the original durable owners, tickets and wire.
#![cfg(unix)]
#[path = "support/file_oversight.rs"]
mod fixture;

use fa_reference::Error;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileHumanReviewer, FileOversight,
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileActorSupervisor, FileRequestHost, MAX_RECOVERY_REQUESTS,
};
use fa_reference::action::consequence::delivery::persistent::{FileDelivery, Reconciliation};
use fa_reference::action::consequence::oversight::actor::{
    ActorError, ActorOutcome, ActorProposal, Knowledge, UnknownReason,
};
use fa_reference::action::consequence::oversight::actor_wire::{
    ActorRequestPort, ActorWire, Command, WireError, encode_command,
};
use fixture::{Directory, snapshot};
use std::fs;

fn proposal(host: &FileOversight, payload: &[u8]) -> ActorProposal {
    ActorProposal {
        target: host.inspect().target,
        payload: payload.to_vec(),
        expected_policy_epoch: 0,
        deadline: ElapsedTick(100),
        units: 16,
    }
}
fn admit<H: FileRequestHost>(supervisor: &mut FileActorSupervisor<H>) {
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
}
fn approve(
    supervisor: &mut FileActorSupervisor<FileOversight>,
    human: &FileHumanReviewer,
) -> fixture::Keys {
    let mut host = supervisor.host_mut().unwrap();
    let action = host.request_action(700).unwrap().clone();
    let inputs = fixture::inputs(&action, b"original complete view");
    fixture::review_existing(&mut host, 1, 101, &inputs);
    let revision = host.revision();
    let automatic = host.authorize(revision, 1, &inputs, snapshot()).unwrap();
    let revision = host.revision();
    let request = host
        .request_human_approval(revision, 1001, 1, &inputs, ElapsedTick(30))
        .unwrap();
    let revision = host.revision();
    let human = human.approve(&mut host, revision, &request).unwrap();
    fixture::Keys {
        action,
        inputs,
        automatic,
        human,
        request,
    }
}

#[test]
fn multiple_exact_retries_preserve_one_snapshot_and_withhold_unselected_or_missing_keys() {
    let root = Directory::new();
    let (host, _) = fixture::create(&root);
    let first = proposal(&host, b"first");
    let second = proposal(&host, b"second");
    let missing = proposal(&host, b"unanswered");
    let (port, mut supervisor) = host.into_actor_gateway();
    admit(&mut supervisor);
    let first_ticket = port.submit(700, &first).unwrap();
    admit(&mut supervisor);
    let second_ticket = port.submit(701, &second).unwrap();
    admit(&mut supervisor);
    let outside = port.submit(702, &first).unwrap();
    let selected = port
        .clone()
        .recovery_only()
        .for_requests(&[703, 701, 700])
        .unwrap();
    assert_eq!(selected.selected_requests(), &[700, 701, 703]);
    admit(&mut supervisor);
    let before = supervisor.host().unwrap().inspect();
    let bytes = fs::read(root.store().join("delivery.bin")).unwrap();

    for (request, original, ticket) in [
        (700, &first, &first_ticket),
        (701, &second, &second_ticket),
    ] {
        let recovered = selected.submit(request, original).unwrap();
        assert_eq!(selected.poll(&recovered), port.poll(ticket));
        for field in 0..5 {
            let mut changed = original.clone();
            match field {
                0 => changed.payload.push(b'x'),
                1 => changed.deadline.0 += 1,
                2 => changed.target.expected_version += 1,
                3 => changed.expected_policy_epoch += 1,
                _ => changed.units += 1,
            }
            assert!(matches!(
                selected.submit(request, &changed),
                Err(ActorError::IdempotencyConflict)
            ));
        }
    }
    assert!(matches!(selected.submit(703, &missing), Err(ActorError::Withheld)));
    assert!(matches!(selected.submit(702, &first), Err(ActorError::Withheld)));
    assert_eq!(
        selected.poll(&outside),
        Knowledge::Withheld { authority_required: "own_request" }
    );
    assert_eq!(selected.cancel(&outside), Err(ActorError::Withheld));
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(fs::read(root.store().join("delivery.bin")).unwrap(), bytes);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 3);

    // The selected missing retry did not consume the one installed observation.
    let newly_admitted = port.submit(703, &missing).unwrap();
    let recovered = selected.clone().submit(703, &missing).unwrap();
    assert_eq!(selected.poll(&recovered), port.poll(&newly_admitted));
    assert_eq!(supervisor.host().unwrap().retained_requests(), 4);

    let other = Directory::new();
    let (other_host, _) = fixture::create(&other);
    let other_proposal = proposal(&other_host, b"first");
    let (other_port, mut other_supervisor) = other_host.into_actor_gateway();
    admit(&mut other_supervisor);
    let foreign = other_port.submit(700, &other_proposal).unwrap();
    let other_before = other_supervisor.host().unwrap().inspect();
    assert_eq!(
        selected.poll(&foreign),
        Knowledge::Withheld { authority_required: "own_request" }
    );
    assert_eq!(selected.cancel(&foreign), Err(ActorError::Withheld));
    assert_eq!(other_supervisor.host().unwrap().inspect(), other_before);
    assert_eq!(selected.poll(&first_ticket), Knowledge::Pending { request: 700 });
}

#[test]
fn finite_selections_validate_without_an_owner_and_can_only_narrow() {
    let root = Directory::new();
    let (host, _) = fixture::create(&root);
    let proposal = proposal(&host, b"retained");
    let (port, supervisor) = host.into_actor_gateway();
    let recovery = port.recovery_only();
    drop(supervisor);

    for invalid in [vec![], vec![0], vec![700, 0], vec![700, 701, 700]] {
        assert!(matches!(
            recovery.clone().for_requests(&invalid),
            Err(Error::InvalidInput)
        ));
    }
    let too_many = vec![700; MAX_RECOVERY_REQUESTS + 1];
    assert!(matches!(recovery.clone().for_requests(&too_many), Err(Error::Limit)));
    let maximum: Vec<_> = (700..700 + MAX_RECOVERY_REQUESTS as u64).collect();
    assert_eq!(
        recovery.clone().for_requests(&maximum).unwrap().selected_requests(),
        maximum.as_slice()
    );

    let single = recovery.clone().for_request(700).unwrap();
    assert_eq!(single.selected_request(), Some(700));
    assert!(matches!(single.clone().for_requests(&[701]), Err(Error::Binding)));
    assert!(matches!(single.clone().for_requests(&[700, 701]), Err(Error::Binding)));
    assert_eq!(
        single.for_requests(&[700]).unwrap().selected_requests(),
        &[700]
    );
    let selected = recovery.for_requests(&[702, 700, 701]).unwrap();
    let narrowed = selected.clone().for_requests(&[701, 700]).unwrap();
    assert_eq!(narrowed.selected_requests(), &[700, 701]);
    assert!(matches!(
        narrowed.clone().for_requests(&[700, 702]),
        Err(Error::Binding)
    ));
    let only = narrowed.for_request(701).unwrap();
    assert_eq!(only.selected_requests(), &[701]);
    assert!(matches!(only.clone().for_request(700), Err(Error::Binding)));
    assert!(matches!(only.clone().for_requests(&[]), Err(Error::InvalidInput)));
    assert!(matches!(only.clone().for_requests(&[701, 701]), Err(Error::InvalidInput)));
    assert!(matches!(only.clone().for_requests(&too_many), Err(Error::Limit)));
    assert_eq!(selected.selected_requests(), &[700, 701, 702]);

    // Selection is independent of live ownership, and an excluded key cannot
    // reveal whether the owner is unavailable or the proposal is malformed.
    let mut malformed = proposal.clone();
    malformed.units = 0;
    assert!(matches!(only.submit(700, &malformed), Err(ActorError::Withheld)));
    assert!(matches!(only.submit(701, &proposal), Err(ActorError::Unavailable)));
}

#[test]
fn selected_cancellation_preserves_executed_liability_and_only_releases_unsent_work() {
    let root = Directory::new();
    let (mut host, human) = fixture::create(&root);
    host.enable_publication_guard(host.revision()).unwrap();
    let first = proposal(&host, b"published");
    let second = proposal(&host, b"unsent");
    let (port, mut supervisor) = host.into_actor_gateway();
    admit(&mut supervisor);
    port.submit(700, &first).unwrap();
    admit(&mut supervisor);
    port.submit(701, &second).unwrap();
    let keys = approve(&mut supervisor, &human);
    fixture::dispatch(&mut supervisor.host_mut().unwrap(), &keys);
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.publish_checked(revision, 1, Some(&keys.inputs), snapshot(), ElapsedTick(1))
            .unwrap();
        let revision = host.revision();
        assert!(matches!(host.reconcile(revision, 1).unwrap(), Reconciliation::Resolved(_)));
    }
    let selected = port.recovery_only().for_requests(&[701, 700]).unwrap();
    let before = supervisor.host().unwrap().inspect();
    let executed = selected.submit(700, &first).unwrap();
    let pending = selected.submit(701, &second).unwrap();
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert!(matches!(
        selected.poll(&executed),
        Knowledge::Known { value: ActorOutcome::Executed, .. }
    ));
    assert_eq!(selected.poll(&pending), Knowledge::Pending { request: 701 });
    assert_eq!(before.executions, 1);
    assert_eq!(before.control.ledger.charged, 16);
    assert_eq!(before.control.ledger.reserved, 16);

    selected.cancel(&executed).unwrap();
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    selected.clone().cancel(&pending).unwrap();
    let after = supervisor.host().unwrap().inspect();
    assert_eq!(after.executions, 1);
    assert_eq!(after.control.ledger.charged, 16);
    assert_eq!(after.control.ledger.reserved, 0);
    assert!(matches!(
        selected.poll(&pending),
        Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }
    ));
    assert_eq!(supervisor.host().unwrap().retained_requests(), 2);

    drop(supervisor);
    assert!(matches!(selected.submit(700, &first), Err(ActorError::Unavailable)));
    assert_eq!(selected.cancel(&pending), Err(ActorError::Unavailable));
    assert_eq!(
        selected.poll(&executed),
        Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable }
    );
}

#[test]
fn original_wire_recovers_multiple_refusals_without_importing_tickets_or_retaining_the_lock() {
    let root = Directory::new();
    let profile = fixture::profile().delivery;
    let mut host = FileDelivery::create(root.store(), profile.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let first = ActorProposal {
        target: profile.target,
        payload: b"first".to_vec(),
        expected_policy_epoch: 99,
        deadline: ElapsedTick(100),
        units: 16,
    };
    let mut second = first.clone();
    second.payload = b"second".to_vec();
    let (port, mut supervisor) = host.into_actor_gateway();
    admit(&mut supervisor);
    let first_ticket = port.submit(700, &first).unwrap();
    admit(&mut supervisor);
    let second_ticket = port.submit(701, &second).unwrap();
    assert!(matches!(
        port.poll(&first_ticket),
        Knowledge::Known { value: ActorOutcome::NotAdmitted, .. }
    ));
    let selected = port.clone().recovery_only().for_requests(&[701, 700]).unwrap();
    let before = fs::read(root.store().join("delivery.bin")).unwrap();
    let mut wire = ActorWire::new(selected.clone());
    for (request, original, ticket) in [
        (700, &first, &first_ticket),
        (701, &second, &second_ticket),
    ] {
        assert_eq!(
            wire.execute(Command::Poll { request }).result,
            Ok(Knowledge::Withheld { authority_required: "own_request" })
        );
        let frame = encode_command(&Command::Submit {
            request,
            proposal: original.clone(),
        })
        .unwrap();
        assert_eq!(wire.exchange(&frame).result, Ok(port.poll(ticket)));
        assert_eq!(wire.execute(Command::Poll { request }).result, Ok(port.poll(ticket)));
    }
    assert_eq!(
        wire.execute(Command::Submit { request: 702, proposal: first.clone() }).result,
        Err(WireError::Withheld)
    );
    assert_eq!(fs::read(root.store().join("delivery.bin")).unwrap(), before);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 2);
    let mut fresh = ActorWire::new(selected.clone());
    assert_eq!(
        fresh.execute(Command::Poll { request: 700 }).result,
        Ok(Knowledge::Withheld { authority_required: "own_request" })
    );
    let borrowed = supervisor.host_mut().unwrap();
    assert!(matches!(selected.submit(700, &first), Err(ActorError::Unavailable)));
    assert!(matches!(selected.submit(702, &first), Err(ActorError::Withheld)));
    drop(borrowed);
    drop(supervisor);
    assert!(matches!(selected.submit(701, &second), Err(ActorError::Unavailable)));
    assert_eq!(
        selected.poll(&first_ticket),
        Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable }
    );
    // Both wire sessions and all cloned selections have only weak custody.
    let reopened = FileDelivery::open(root.store(), profile).unwrap();
    drop(reopened);
}
