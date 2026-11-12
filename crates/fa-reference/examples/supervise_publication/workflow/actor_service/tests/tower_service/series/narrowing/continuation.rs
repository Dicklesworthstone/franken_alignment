//! Retained live work continues only after a NEW independent human decision.
//! Real socket/child-process integration; helper verdicts remain synthetic.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::{
    ReviewerClient, ReviewClientPhase,
};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::{
    ReviewPacket, ReviewReceipt,
};
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use std::os::unix::fs::MetadataExt;

fn offer(profile: &PeerProfile, actor_request: u64) -> ReviewerClient<UnixStream> {
    let mut client = profile
        .connect_client_waiting(actor_request, Duration::from_secs(10))
        .unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10));
        match client.step().unwrap() {
            ReviewClientProgress::NeedsDecision => return client,
            ReviewClientProgress::Complete => panic!("offer completed without a decision"),
            _ => pause(1),
        }
    }
}

fn decide(mut client: ReviewerClient<UnixStream>, decision: ReviewDecision) -> ReviewReceipt {
    client.respond(decision).unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10));
        match client.step().unwrap() {
            ReviewClientProgress::Complete => return client.receipt().unwrap(),
            ReviewClientProgress::NeedsDecision => panic!("explicit decision was lost"),
            _ => pause(1),
        }
    }
}

fn disconnected(mut client: ReviewerClient<UnixStream>) {
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10));
        match client.step() {
            Err(_) => {
                assert_eq!(client.phase(), ReviewClientPhase::Failed);
                assert!(client.receipt().is_none());
                return;
            }
            Ok(ReviewClientProgress::Complete | ReviewClientProgress::NeedsDecision) => {
                panic!("withdrawn or unfunded connection disclosed an active decision")
            }
            _ => pause(1),
        }
    }
}

fn narrowed(
    socket: &mut UnixStream,
    scope: Scope,
    operation: u64,
    retained: Vec<ResolvedTarget>,
) -> (Json, String) {
    let plan = raw_exchange(socket, &next(scope, operation, retained.clone()));
    let command = action(scope, &plan, retained);
    let response = raw_exchange(socket, &command);
    let receipt = field(known(&response, "narrowing"), "value");
    assert_eq!(field(receipt, "refunded_units").as_str(), Some("0"));
    assert!(field(receipt, "cancelled").as_array().unwrap().is_empty());
    (response, command)
}

fn same_review(old: &ReviewPacket, fresh: &ReviewPacket) {
    assert_ne!(fresh.binding().request, old.binding().request);
    assert_ne!(fresh.binding().session, old.binding().session);
    assert_eq!(fresh.binding().attempt, old.binding().attempt);
    assert!(fresh.binding().offer_revision > old.binding().offer_revision);
    assert_eq!(fresh.control_sequence(), old.control_sequence() + 1);
    assert_eq!(fresh.input_revision(), old.input_revision());
    assert_eq!(fresh.policy_generation(), old.policy_generation());
    assert_eq!(fresh.action(), old.action());
    assert_eq!(fresh.views(), old.views());
    assert_eq!(fresh.disposition(), HumanDisposition::Pending);
}

#[test]
fn retained_active_offer_requires_fresh_review_and_later_schedule_avoids_renewal_key_collision() {
    for with_checked in [false, true] {
        let root = Directory::new();
        let config = configured(&root);
        evidence(&root, &config);
        let (mut actor, reviewer) = profiles(&root, &config);
        actor.connections = 1;
        actor.candidates = 1;
        actor.reply_ms = 1000;
        let settings = settings(&root, &config);
        let publication = with_checked.then(|| checked(&root, &config));
        let scope = config.profile.delivery.scope;
        let initial = config.profile.delivery.target;
        let mut later = initial;
        later.expected_version += 1;
        let first = document(&config, 7, 1, "6669727374");
        let second = document(&config, 8, 2, "7365636f6e64");
        let first_poll = poll(&config, 7);
        let second_poll = poll(&config, 8);
        let store = config.store.clone();
        let bootstrap = config.profile.clone();
        let peer_store = store.clone();
        let peer_bootstrap = bootstrap.clone();
        let actor_peer = actor.clone();
        let operator_peer = settings.clone();
        let human_peer = reviewer.clone();
        let peer = std::thread::spawn(move || {
            let mut actor = connect(&actor_peer);
            let submitted = raw_exchange(&mut actor, &first);
            assert_eq!(field(knowledge(&submitted), "state").as_str(), Some("pending"));
            let mut old_client = offer(&human_peer, 7);
            let old = old_client.packet().unwrap().clone();
            assert_eq!(old.binding().request, 7);
            let socket_before = fs::symlink_metadata(human_peer.socket(7)).unwrap();
            let mut operator = connect_operator(&operator_peer);
            let before = FileOversight::read_publication(&peer_store, &peer_bootstrap).unwrap();
            assert_eq!(before.control.ledger.stages[&1], ActionState::Reviewing);
            assert_eq!((before.executions, before.control.ledger.reserved), (0, 0));
            let (response, command) = narrowed(&mut operator, scope, 811, vec![initial, later]);
            let narrowed_state = FileOversight::read_publication(&peer_store, &peer_bootstrap).unwrap();
            assert_eq!(narrowed_state.control.sequence, before.control.sequence + 1);
            assert_eq!(narrowed_state.control.ledger.epoch, before.control.ledger.epoch);
            assert_eq!(narrowed_state.control.ledger.charged, 0);
            assert_eq!(narrowed_state.executions, 0);

            // A decision sent under the withdrawn packet cannot act on the new
            // predecessor or acquire a replacement approval by itself.
            old_client.respond(ReviewDecision::Approve).unwrap();
            disconnected(old_client);
            let fresh_client = offer(&human_peer, 7);
            let fresh = fresh_client.packet().unwrap().clone();
            same_review(&old, &fresh);
            assert_eq!(fresh.binding().request, 8);
            let socket_after = fs::symlink_metadata(human_peer.socket(7)).unwrap();
            assert_eq!((socket_after.dev(), socket_after.ino()), (socket_before.dev(), socket_before.ino()));
            assert_eq!(FileOversight::read_publication(&peer_store, &peer_bootstrap).unwrap().executions, 0);
            assert_eq!(decide(fresh_client, ReviewDecision::Approve).decision, ReviewDecision::Approve);
            assert!(executed(&finish(&mut actor, &first, &first_poll)));

            // Renewal consumed human ID 8. The original schedule's actor
            // request 8 must still receive a distinct native human request.
            wait(&crate::workflow::control::socket_path(&human_peer, 8));
            assert!(!human_peer.socket(7).exists());
            let submitted = raw_exchange(&mut actor, &second);
            assert_eq!(field(knowledge(&submitted), "state").as_str(), Some("pending"));
            let second_client = offer(&human_peer, 8);
            let second_key = second_client.packet().unwrap().binding().request;
            assert_eq!(second_key, 9);
            decide(second_client, ReviewDecision::Approve);
            assert!(executed(&finish(&mut actor, &second, &second_poll)));
            // Exact operator retry retains its original receipt after both
            // publications, without a new restriction or reviewer continuation.
            let before_retry = store_files(&peer_store);
            assert_eq!(raw_exchange(&mut operator, &command), response);
            assert_eq!(store_files(&peer_store), before_retry);
            let blocked = raw_exchange(&mut operator, &next(scope, 812, vec![initial, later]));
            assert_eq!(field(field(known(&blocked, "narrowing-next"), "value"), "block_reason").as_str(), Some("operation_limit"));
            assert_eq!(store_files(&peer_store), before_retry);
            (old.binding().request, fresh.binding().request, second_key, response)
        });
        let result = service::serve_tower(
            config, &actor, &reviewer, publication.as_ref(),
            Options { requests: &[7, 8], open: false, credibility: None },
            GENERATION, Some(&settings), || ElapsedTick(1000),
        );
        let (old, fresh, second, _) = peer.join().unwrap();
        assert!(result.is_ok(), "checked={with_checked}: {result:?}");
        let after = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!((after.executions, after.payload.as_slice()), (2, b"second".as_slice()));
        assert_eq!(after.control.ledger.stages[&1], ActionState::Confirmed);
        assert_eq!(after.control.ledger.stages[&2], ActionState::Confirmed);
        assert_eq!((after.control.ledger.charged, after.control.ledger.reserved), (14, 0));
        assert_eq!(after.control.ledger.epoch, 0);
        assert!(after.stop.is_none());
        assert!(!actor.socket.exists());
        assert!(!settings.socket.exists());
        assert!(!reviewer.socket(7).exists());
        assert!(!reviewer.socket(8).exists());
        let (host, _) = FileOversight::open(&store, bootstrap).unwrap();
        assert_eq!(host.human_status(old).unwrap().disposition, HumanDisposition::Revoked);
        assert_eq!(host.human_status(fresh).unwrap().disposition, HumanDisposition::Consumed);
        assert_eq!(host.human_status(second).unwrap().disposition, HumanDisposition::Consumed);
    }
}

fn candidate_limited(root: &Directory, config: &Config, maximum: u32) -> PeerProfile {
    let id = credentials();
    PeerProfile::decode(format!(
        r#"{{"version":1,"clock":"unix_milliseconds","scope":{},"reviewer_id":{},"socket_directory":"{}/reviewers","supervisor":{{"uid":{},"gid":{},"pid":{}}},"reviewer":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":{},"runtime_ms":15000,"poll_ms":1}}"#,
        scope(config), config.profile.human.reviewer_id, root.0.display(),
        id.uid(), id.gid(), id.pid(), id.uid(), id.gid(), id.pid(), maximum,
    ).as_bytes()).unwrap()
}

#[test]
fn reapproval_keeps_original_candidate_request_capacity_and_current_source_boundaries() {
    for boundary in ["candidate_limit", "human_request_limit", "changed_source"] {
        let root = Directory::new();
        let mut config = configured(&root);
        if boundary == "human_request_limit" {
            config.profile.human.max_requests = 1;
        }
        evidence(&root, &config);
        let (mut actor, _) = profiles(&root, &config);
        actor.reply_ms = 1000;
        let reviewer = candidate_limited(&root, &config, if boundary == "candidate_limit" { 1 } else { 8 });
        let settings = settings(&root, &config);
        let scope = config.profile.delivery.scope;
        let target = config.profile.delivery.target;
        let first = document(&config, 7, 1, "6669727374");
        let source = root.0.join("evidence.json");
        let store = config.store.clone();
        let bootstrap = config.profile.clone();
        let actor_peer = actor.clone();
        let operator_peer = settings.clone();
        let human_peer = reviewer.clone();
        let peer = std::thread::spawn(move || {
            let mut actor = connect(&actor_peer);
            raw_exchange(&mut actor, &first);
            let mut old = offer(&human_peer, 7);
            let mut operator = connect_operator(&operator_peer);
            let _ = narrowed(&mut operator, scope, 813, vec![target]);
            old.respond(ReviewDecision::Approve).unwrap();
            disconnected(old);
            if boundary == "changed_source" {
                let changed = EvidenceSnapshot::new(
                    EvidenceIdentity { source: 51, generation: 2, scope },
                    Snapshot { semantic_epoch: 1, complete: true,
                        values: BTreeMap::from([(7, b"ok".to_vec())]) },
                    ["alpha", "beta"].into_iter().map(|member| (
                        member.to_owned(),
                        format!("changed independent source: {member}").into_bytes(),
                    )).collect(),
                ).unwrap();
                fs::write(&source, changed.encode()).unwrap();
            }
            // This connection is independently initiated. It must fail under
            // the SAME candidate counter, original native request capacity or
            // changed source; no replacement offer or publication is permitted.
            let fresh = human_peer
                .connect_client_waiting(7, Duration::from_secs(10))
                .unwrap();
            disconnected(fresh);
        });
        let result = service::serve_tower(
            config, &actor, &reviewer, None,
            Options { requests: &[7], open: false, credibility: None },
            GENERATION, Some(&settings), || ElapsedTick(1000),
        );
        peer.join().unwrap();
        assert!(result.is_err(), "{boundary}: {result:?}");
        let after = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(after.executions, 0, "{boundary}");
        assert_eq!((after.control.ledger.charged, after.control.ledger.reserved), (0, 0), "{boundary}");
        assert!(after.stop.is_some(), "{boundary}");
        assert!(!reviewer.socket(7).exists());
        let (host, _) = FileOversight::open(&store, bootstrap).unwrap();
        assert_eq!(host.human_status(7).unwrap().disposition, HumanDisposition::Revoked);
        assert!(host.human_request(8).is_err(), "{boundary}");
    }
}


#[test]
fn committed_human_receipt_remains_historical_and_restrictive_decision_stays_terminal() {
    // The exact ingress checkpoint after native application makes this boundary
    // deterministic. The separate full-service test above authenticates the
    // operator transport; this one retains actual helpers and reviewer sockets.
    for decision in [ReviewDecision::Approve, ReviewDecision::Reject] {
        let root = Directory::new();
        let mut config = configured(&root);
        evidence(&root, &config);
        let (_, reviewer_profile) = profiles(&root, &config);
        let (mut host, reviewer) = FileOversight::create(
            &config.store, config.profile.clone(),
        ).unwrap();
        host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
        host.enable_file_source(host.revision(), config.source_policy).unwrap();
        let snapshot = host
            .refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1000))
            .unwrap().snapshot().clone();
        host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
        let (actor, mut driver) = host.into_supervised_driver();
        let revision = driver.supervisor().host().unwrap().revision();
        driver.supervisor_mut().set_snapshot(revision, Some(snapshot)).unwrap();
        let Command::Submit { request, proposal } = proposal(&config) else {
            unreachable!("original submit fixture");
        };
        let ticket = actor.submit(request, &proposal).unwrap();
        assert_eq!(actor.poll(&ticket), Knowledge::Pending { request });
        let (narrow, _grant) = driver.supervisor()
            .narrow_port(1, 1, MAX_NARROW_REPLAY_EVENTS).unwrap();
        let target = config.profile.delivery.target;
        let control_applied = std::cell::Cell::new(false);
        let mut ingress = |driver: &mut FileSupervisedDriver| -> Result<bool, String> {
            let disposition = {
                let host = driver.supervisor().host().map_err(debug)?;
                host.human_status(request).ok().map(|status| status.disposition)
            };
            let wanted = if decision == ReviewDecision::Approve {
                HumanDisposition::Approved
            } else {
                HumanDisposition::Rejected
            };
            if !control_applied.get() && disposition == Some(wanted) {
                let before = driver.supervisor().host().map_err(debug)?.inspect();
                assert_eq!(before.executions, 0);
                assert_eq!(before.control.ledger.charged, 0);
                let Knowledge::Known { value: plan, .. } =
                    narrow.next(821, vec![target]).map_err(debug)?
                else {
                    return Err("original narrowing plan was unavailable".into());
                };
                let explicit = plan.request().ok_or("original narrowing was blocked")?;
                assert!(matches!(
                    narrow.narrow(explicit).map_err(debug)?,
                    Knowledge::Known { .. }
                ));
                let host = driver.supervisor().host().map_err(debug)?;
                let after = host.inspect();
                assert_eq!(after.control.sequence, before.control.sequence + 1);
                assert_eq!(after.control.ledger.epoch, before.control.ledger.epoch);
                assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
                assert_eq!(host.human_status(request).map_err(debug)?.disposition,
                    if decision == ReviewDecision::Approve {
                        HumanDisposition::Revoked
                    } else {
                        HumanDisposition::Rejected
                    });
                control_applied.set(true);
            }
            Ok(false)
        };
        let peer_profile = reviewer_profile.clone();
        let human = std::thread::spawn(move || {
            let first = offer(&peer_profile, request);
            let old = first.packet().unwrap().clone();
            // This original receipt still names the actually applied first
            // decision. Narrowing does not relabel it as a rejected/unsent vote.
            assert_eq!(decide(first, decision).decision, decision);
            if decision == ReviewDecision::Approve {
                let renewed = offer(&peer_profile, request);
                let fresh = renewed.packet().unwrap().clone();
                same_review(&old, &fresh);
                assert_eq!(decide(renewed, ReviewDecision::Approve).decision, ReviewDecision::Approve);
                Some(fresh.binding().request)
            } else {
                None
            }
        });
        let deadline = Deadline {
            logical: ElapsedTick(100000),
            started: Instant::now(),
            wall: Duration::from_secs(15),
        };
        let result = crate::workflow::execute_serviced(
            &mut driver, &reviewer, &mut config, request, &deadline,
            ExecuteServices {
                peers: Some(&reviewer_profile),
                publication: None,
                ingress: &mut ingress,
                stop: None,
            },
            &mut || ElapsedTick(1000),
        );
        let renewed_key = human.join().unwrap();
        assert!(result.is_ok(), "{decision:?}: {result:?}");
        assert!(control_applied.get());
        {
            let host = driver.supervisor().host().unwrap();
            let after = host.inspect();
            assert!(after.stop.is_none());
            assert_eq!(after.control.ledger.reserved, 0);
            match renewed_key {
                Some(key) => {
                    assert_ne!(key, request);
                    assert_eq!((after.executions, after.control.ledger.charged), (1, 7));
                    assert_eq!(host.human_status(request).unwrap().disposition, HumanDisposition::Revoked);
                    assert_eq!(host.human_status(key).unwrap().disposition, HumanDisposition::Consumed);
                }
                None => {
                    assert_eq!((after.executions, after.control.ledger.charged), (0, 0));
                    assert_eq!(after.control.ledger.stages[&1], ActionState::Cancelled);
                    assert_eq!(host.human_status(request).unwrap().disposition, HumanDisposition::Rejected);
                    assert!(host.human_request(request + 1).is_err());
                }
            }
        }
        assert_eq!(crate::workflow::cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms), 0);
        assert!(!reviewer_profile.socket(request).exists());
    }
}
