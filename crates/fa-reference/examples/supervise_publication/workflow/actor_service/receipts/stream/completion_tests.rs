//! Actual original two-key effects; only elapsed scheduling is injected.
use super::*;
#[path = "../../../../../../tests/support/file_stream_wire.rs"]
mod fixture;
use fixture::{Directory, snapshot};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileRecoveryFloor, anchored::MAX_HISTORY_ANCHOR_BYTES,
};
use fa_reference::action::consequence::delivery::persistent::Reconciliation;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge};
use fa_reference::action::consequence::oversight::actor_peer::{PeerCredentials, PeerPolicy};
use fa_reference::action::consequence::oversight::actor_wire::{ActorRequestPort, Command, WireResponse};
use fa_reference::action::consequence::oversight::actor_wire::client::{ActorClientState, ClientProgress, ClientSessionLimits};
use fa_reference::action::consequence::oversight::actor_wire::client::drive::ClientDriveBudget;
use std::{cell::Cell, fs, os::unix::{fs::PermissionsExt, net::UnixStream}};

// The selected request is executed/confirmed. Another request in the SAME
// original stream owns the unresolved liability; a valid receipt must still flow.
fn seed(root: &Directory, publish_tail: bool) -> (Config, Profile, Settings, ActorProposal) {
    let mut config = Config::decode(include_bytes!("../../../../../../fixtures/supervised_publication.json")).unwrap();
    config.store = root.store(); config.profile = fixture::profile();
    config.profile.delivery.clock_domain = crate::config::CLOCK_DOMAIN;
    fs::set_permissions(&root.0, fs::Permissions::from_mode(0o700)).unwrap();
    config.programs.clear(); config.timing.runtime_ms = 1000;
    let (socket, _) = UnixStream::pair().unwrap(); let c = PeerCredentials::observe(&socket).unwrap();
    let policy = PeerPolicy::new(c.uid(), c.gid(), Some(c.pid())).unwrap();
    let actor = Profile { request: 7, scope: config.profile.delivery.scope,
        socket: root.0.join("actor.sock"), supervisor: policy, actor: policy,
        candidates: 4, connections: 4, exchanges: 128, runtime_ms: 1000, poll_ms: 1, reply_ms: 10 };
    let (mut host, human) = FileOversight::create_stream(root.store(), config.profile.clone(), fixture::stream()).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let (port, mut supervisor) = host.into_stream_actor_gateway().unwrap();
    let mut original = None;
    for (request, attempt, text) in [(7, 1, "first"), (8, 2, "second")] {
        let Command::Submit { proposal, .. } = fixture::command(&supervisor.host().unwrap(), request, Some(text)) else { unreachable!() };
        if request == 7 { original = Some(proposal.clone()); }
        fixture::admit(&mut supervisor); port.submit(request, &proposal).unwrap();
        let mut host = supervisor.host_mut().unwrap();
        let keys = fixture::approve(&mut host, &human, request, attempt);
        fixture::ordinary::dispatch(&mut host, &keys);
        if request == 7 || publish_tail {
            let r = host.revision();
            host.publish_checked(r, attempt, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
        }
        if request == 7 { let r = host.revision(); host.reconcile(r, attempt).unwrap(); }
    }
    let host = supervisor.host().unwrap(); let state = host.inspect();
    let history = root.0.join("independent.history"); let mut archive = Vec::new();
    host.history_anchor().unwrap().write_to(&mut archive, MAX_HISTORY_ANCHOR_BYTES).unwrap();
    fs::write(&history, archive).unwrap();
    let settings = Settings { stream: fixture::stream(), registered_source: false,
        minimum: FileRecoveryFloor { journal_revision: state.revision,
            control_sequence: state.control.sequence, authority_epoch: state.control.ledger.epoch }, history };
    drop(host); drop(supervisor);
    (config, actor, settings, original.unwrap())
}

fn receive(config: &Config, actor: &Profile, settings: &Settings, original: &ActorProposal,
    now: u64, reconcile: bool) -> (Result<ServiceReport, String>, WireResponse)
{
    let tick = Cell::new(0); let mut client = None; let mut response = None; let mut waits = 0;
    let mut wait = |_: u64| {
        waits += 1; assert!(waits < 64);
        if client.is_none() {
            let socket = UnixStream::connect(&actor.socket).unwrap(); actor.supervisor.verify(&socket).unwrap();
            let mut c = ActorClientState::new(ClientSessionLimits::default()).unwrap().connect_unix(socket).unwrap();
            c.submit(7, original).unwrap(); client = Some(c);
        }
        if let ClientProgress::Response(value) = client.as_mut().unwrap().drive(ClientDriveBudget::default()).unwrap().progress {
            response = Some(value); tick.set(1000);
        }
    };
    let result = if reconcile {
        serve_reconciled(config, actor, settings, || ElapsedTick(now),
            &mut || Duration::from_millis(tick.get()), &mut wait)
    } else {
        serve(config, actor, settings, &mut || Duration::from_millis(tick.get()), &mut wait)
    };
    (result, response.expect("valid selected receipt must not be withheld by another liability"))
}

#[test]
fn expired_tail_is_reported_after_selected_receipt_and_survives_empty_repeat_sweep() {
    let root = Directory::new(); let (config, actor, settings, original) = seed(&root, false);
    let before = FileOversight::read_stream_publication(&config.store, &config.profile, settings.stream).unwrap();
    let now = config.profile.delivery.retention_ticks + 2;
    for repeat in [false, true] {
        let (service, response) = receive(&config, &actor, &settings, &original, now, true);
        assert!(matches!(response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
        let error = service.unwrap_err();
        assert!(error.contains("reconciliation is incomplete"));
        assert!(error.contains("IrrecoverablyUnknown"));
        if !repeat { assert!(error.contains("RetentionExpired")); }
        else { assert!(error.contains("sweep=Some({})"), "{error}"); }
        let state = FileOversight::read_stream_publication(&config.store, &config.profile, settings.stream).unwrap();
        assert_eq!(state.publication.executions, 1);
        assert_eq!(state.publication.control.ledger.charged, before.publication.control.ledger.charged);
        assert_eq!(state.confirmed, before.confirmed); assert!(!state.published.finished());
    }
}

#[test]
fn matched_published_tail_settles_and_receipt_only_does_not_claim_a_sweep() {
    for reconcile in [false, true] {
        let root = Directory::new(); let (config, actor, settings, original) = seed(&root, true);
        let (service, response) = receive(&config, &actor, &settings, &original, 2, reconcile);
        assert!(service.is_ok(), "{service:?}");
        assert!(matches!(response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
        let state = FileOversight::read_stream_publication(&config.store, &config.profile, settings.stream).unwrap();
        assert_eq!(state.publication.executions, 2);
        assert_eq!(state.confirmed.message_count(), if reconcile { 2 } else { 1 });
        assert_eq!(state.published.message_count(), 2);
    }
}

#[test]
fn late_expiry_retains_acknowledged_sweep_instead_of_only_a_startup_error() {
    let root = Directory::new(); let (config, actor, settings, _) = seed(&root, false);
    let canonical = config.store.join("delivery.bin"); let before = fs::read(&canonical).unwrap();
    let error = serve_reconciled(&config, &actor, &settings,
        || ElapsedTick(config.profile.delivery.retention_ticks + 2),
        &mut || if fs::read(&canonical).unwrap() == before { Duration::ZERO } else { Duration::from_millis(1000) },
        &mut |_| panic!("late expiry cannot drive a peer")).unwrap_err();
    assert!(error.contains(EXPIRED)); assert!(error.contains("acknowledged stream recovery"));
    assert!(error.contains("RetentionExpired")); assert!(error.contains("IrrecoverablyUnknown"));
    assert_ne!(fs::read(canonical).unwrap(), before); assert!(!actor.socket.exists());
}

#[test]
fn native_refusal_diagnostic_cannot_become_success_when_retained_stages_are_empty() {
    // Test only the report projection with an explicit synthetic native refusal;
    // the integration tests above obtain genuine sweeps from original histories.
    use fa_reference::Error;
    use std::collections::BTreeMap;
    let root = Directory::new(); let (config, actor, settings, original) = seed(&root, true);
    let (result, _) = receive(&config, &actor, &settings, &original, 2, true); result.unwrap();
    let (host, _) = FileOversight::open_stream(&config.store, config.profile.clone(), settings.stream).unwrap();
    for failure in [false, true] {
        let sweep = BTreeMap::from([(2, if failure { Err(Error::Incomplete) } else { Ok(Reconciliation::RetentionExpired) })]);
        let report = RecoveryReport::capture(&host, Some(sweep)).unwrap();
        assert!(report.finish(Ok(ServiceReport { frames: 1, candidates: 1, revision: host.revision() })).is_err());
    }
}
