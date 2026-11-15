//! Installing a weak disclosure never recreates the authenticated peer session.

use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileActorPrecheckPort, FilePrecheckDisclosure,
};

fn disclose(file: &mut FileRig) -> (FileActorPrecheckPort, FilePrecheckDisclosure) {
    file.rig
        .driver
        .prepare_file_intake(&mut file.source, || ElapsedTick(1))
        .result
        .unwrap();
    let supervisor = file.rig.driver.supervisor();
    let revision = supervisor.host().unwrap().revision();
    supervisor.disclose_exact_precheck(revision, 81, 2).unwrap()
}

fn check(file: &FileRig, mount: &Address) -> TowerRequest {
    let mut request = TowerRequest::new("peer-precheck", mount.clone(), Verb::Precheck);
    request.arguments = ReadArguments::Precheck {
        query: 9001,
        proposal: file.rig.proposal(),
    };
    request
}

fn send(socket: &mut UnixStream, request: &TowerRequest) {
    socket
        .write_all(format!("{}\n", encode_request(request).unwrap()).as_bytes())
        .unwrap();
}

fn drive(
    file: &mut FileRig,
    peer: &mut PublicationPeerSession,
    budget: DriveBudget,
) -> fa_reference::action::consequence::oversight::actor_transport::DriveReport {
    let report = file
        .rig
        .driver
        .drive_tower_request_from_file(
            peer,
            1,
            &mut file.source,
            || panic!("disclosure query acquired source or clock"),
            budget,
        )
        .unwrap();
    assert!(report.intakes.is_empty());
    report.drive
}

fn exchange(
    file: &mut FileRig,
    peer: &mut PublicationPeerSession,
    socket: &mut UnixStream,
    request: &TowerRequest,
) -> Json {
    send(socket, request);
    assert_eq!(drive(file, peer, DriveBudget::default()).progress.frames, 1);
    let mut responses = replies(socket);
    assert_eq!(responses.len(), 1);
    responses.remove(0)
}

fn native(response: &Json) -> &Json {
    let result = response.get("result").unwrap();
    assert_eq!(result.get("kind").and_then(Json::as_str), Some("precheck"));
    result.get("value").unwrap().get("knowledge").unwrap()
}

fn unsupported(response: &Json) {
    assert_eq!(
        response
            .get("envelope")
            .unwrap()
            .get("reason_code")
            .and_then(Json::as_str),
        Some("CapabilityUnsupported")
    );
}

#[test]
fn installation_preserves_pending_reply_and_shared_work_through_withdrawal_and_revocation() {
    let mut file = cold(10, StateLimits::default());
    let (port, custodian) = disclose(&mut file);
    let (mut peer, mount, _) = session(&file, 5, policy());
    let request = check(&file, &mount);
    let mut socket = attach(&mut peer);
    let original = image(&file);

    send(&mut socket, &request);
    let pending = drive(
        &mut file,
        &mut peer,
        DriveBudget {
            write_bytes: 0,
            ..DriveBudget::default()
        },
    );
    assert_eq!(pending.progress.frames, 1);
    assert_eq!(pending.status.channel, ChannelState::ReplyReady);
    let status = peer.status();
    assert_eq!(peer.remaining_exchanges(), 4);
    peer.set_precheck(Some(port.clone())).unwrap();
    assert_eq!(peer.status(), status);
    assert_eq!(peer.remaining_exchanges(), 4);
    assert_eq!(custodian.remaining_queries(), 2);
    assert_eq!(
        drive(&mut file, &mut peer, DriveBudget::default())
            .progress
            .frames,
        0
    );
    let response = replies(&mut socket);
    assert_eq!(response.len(), 1);
    unsupported(&response[0]);

    let response = exchange(&mut file, &mut peer, &mut socket, &request);
    assert_eq!(
        native(&response).get("value").and_then(Json::as_str),
        Some("precheck_passed")
    );
    assert_eq!(custodian.remaining_queries(), 1);
    peer.set_precheck(None).unwrap();
    let response = exchange(&mut file, &mut peer, &mut socket, &request);
    unsupported(&response);
    assert_eq!(custodian.remaining_queries(), 1);
    peer.set_precheck(Some(port)).unwrap();
    let response = exchange(&mut file, &mut peer, &mut socket, &request);
    assert_eq!(
        native(&response).get("value").and_then(Json::as_str),
        Some("precheck_passed")
    );
    assert_eq!(custodian.remaining_queries(), 0);

    custodian.revoke();
    let response = exchange(&mut file, &mut peer, &mut socket, &request);
    assert_eq!(
        native(&response).get("state").and_then(Json::as_str),
        Some("withheld")
    );
    assert_eq!(peer.remaining_exchanges(), 0);
    assert_eq!(file.source.status().read_attempts, 1);
    assert_eq!(image(&file), original);
}

#[test]
fn foreign_installation_withdraws_old_port_without_reviving_or_regranting_the_session() {
    let mut file = cold(10, StateLimits::default());
    let (port, custodian) = disclose(&mut file);
    let mut foreign = cold(10, StateLimits::default());
    let (foreign_port, foreign_custodian) = disclose(&mut foreign);
    let (mut peer, mount, _) = session(&file, 4, policy());
    let request = check(&file, &mount);
    let mut socket = attach(&mut peer);
    peer.set_precheck(Some(port.clone())).unwrap();
    let response = exchange(&mut file, &mut peer, &mut socket, &request);
    assert_eq!(
        native(&response).get("value").and_then(Json::as_str),
        Some("precheck_passed")
    );
    assert_eq!(custodian.remaining_queries(), 1);
    let status = peer.status();
    let exchanges = peer.remaining_exchanges();
    assert_eq!(peer.set_precheck(Some(foreign_port)), Err(Error::Binding));
    assert_eq!(peer.status(), status);
    assert_eq!(peer.remaining_exchanges(), exchanges);
    unsupported(&exchange(&mut file, &mut peer, &mut socket, &request));
    assert_eq!(custodian.remaining_queries(), 1);
    assert_eq!(foreign_custodian.remaining_queries(), 2);

    // Rejection removed only this facade's installation, not the original
    // custodian or the shared allowance behind other weak copies.
    assert!(matches!(
        port.check(9001, &file.rig.proposal()),
        Ok(Knowledge::Known { .. })
    ));
    assert_eq!(custodian.remaining_queries(), 0);
    assert!(peer.revoke());
    let exchanges = peer.remaining_exchanges();
    assert_eq!(peer.set_precheck(Some(port)), Err(Error::WrongState));
    assert_eq!(peer.remaining_exchanges(), exchanges);
    assert!(peer.status().revoked);
}
