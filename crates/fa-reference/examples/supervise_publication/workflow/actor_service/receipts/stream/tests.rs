//! Real stream owners and Unix peers; only elapsed scheduling is injected.
use super::*;
#[path = "../../../../../../tests/support/file_stream_wire.rs"]
mod fixture;
use fixture::{Directory, snapshot};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileRecoveryFloor, anchored::MAX_HISTORY_ANCHOR_BYTES,
};
use fa_reference::action::consequence::delivery::stream::StreamProfile;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::actor_peer::{PeerCredentials, PeerPolicy};
use fa_reference::action::consequence::oversight::actor_wire::{ActorRequestPort, Command, WireResponse};
use fa_reference::action::consequence::oversight::actor_wire::client::{ActorClient, ActorClientState, ClientProgress, ClientSessionLimits};
use fa_reference::action::consequence::oversight::actor_wire::client::drive::ClientDriveBudget;
use std::{cell::Cell, fs, os::unix::net::UnixStream};

fn configured(root: &Directory) -> Config {
    let mut config = Config::decode(include_bytes!("../../../../../../fixtures/supervised_publication.json")).unwrap();
    config.store = root.store(); config.profile = fixture::profile();
    config.programs.clear(); config.timing.runtime_ms = 1000; config
}
fn actor(root: &Directory, config: &Config) -> Profile {
    let (socket, _) = UnixStream::pair().unwrap(); let c = PeerCredentials::observe(&socket).unwrap();
    let policy = PeerPolicy::new(c.uid(), c.gid(), Some(c.pid())).unwrap();
    Profile { request: 7, scope: config.profile.delivery.scope, socket: root.0.join("actor.sock"),
        supervisor: policy, actor: policy, candidates: 4, connections: 4, exchanges: 128,
        runtime_ms: 1000, poll_ms: 1, reply_ms: 10 }
}
// 0 = empty, 1 = two confirmed messages and finish, 2 = published/unreconciled.
fn seeded(root: &Directory, mode: usize, source: bool) -> (Config, Profile, Settings, Vec<(u64, ActorProposal)>) {
    let config = configured(root); let actor = actor(root, &config);
    let (mut h, human) = FileOversight::create_stream(root.store(), config.profile.clone(), fixture::stream()).unwrap();
    h.enable_recovery_reserve(h.revision(), RecoveryReserve::terminal()).unwrap();
    if source { h.enable_file_source(h.revision(), config.source_policy).unwrap(); }
    h.observe_time(h.revision(), ElapsedTick(1)).unwrap();
    let (port, mut s) = h.into_stream_actor_gateway().unwrap(); let mut saved = Vec::new();
    let messages = if mode == 1 { vec![(7, 1, Some("é")), (8, 2, Some("later")), (9, 3, None)] }
        else if mode == 2 { vec![(7, 1, Some("é"))] } else { Vec::new() };
    for (key, attempt, message) in messages {
        let Command::Submit { proposal, .. } = fixture::command(&s.host().unwrap(), key, message) else { unreachable!() };
        fixture::admit(&mut s); port.submit(key, &proposal).unwrap();
        let mut h = s.host_mut().unwrap(); let keys = fixture::approve(&mut h, &human, key, attempt);
        fixture::ordinary::dispatch(&mut h, &keys);
        let r = h.revision(); h.publish_checked(r, attempt, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
        if mode == 1 { let r = h.revision(); h.reconcile(r, attempt).unwrap(); }
        saved.push((key, proposal));
    }
    let anchor = s.host().unwrap().history_anchor().unwrap(); let state = s.host().unwrap().inspect();
    let history = root.0.join("independent.history"); let mut bytes = Vec::new();
    anchor.write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES).unwrap(); fs::write(&history, bytes).unwrap();
    let settings = Settings { stream: fixture::stream(), registered_source: source,
        minimum: FileRecoveryFloor { journal_revision: state.revision,
            control_sequence: state.control.sequence, authority_epoch: state.control.ledger.epoch }, history };
    drop(s);
    (config, actor, settings, saved)
}
fn canonical(c: &Config) -> Vec<u8> { fs::read(c.store.join("delivery.bin")).unwrap() }
fn observe(c: &Config, a: &Profile, settings: &Settings, original: &ActorProposal) -> WireResponse {
    observe_with(c, a, settings, original, false)
}
fn observe_reconciled(c: &Config, a: &Profile, settings: &Settings, original: &ActorProposal) -> WireResponse {
    observe_with(c, a, settings, original, true)
}
fn observe_with(c: &Config, a: &Profile, settings: &Settings, original: &ActorProposal, reconcile: bool) -> WireResponse {
    let tick = Cell::new(0); let mut client: Option<ActorClient<UnixStream>> = None;
    let mut response = None; let mut waits = 0;
    let mut drive = |_: u64| {
        waits += 1; assert!(waits < 64, "receipt peer never completed its bounded exchange");
        if client.is_none() {
            let socket = UnixStream::connect(&a.socket).unwrap(); a.supervisor.verify(&socket).unwrap();
            let mut connected = ActorClientState::new(ClientSessionLimits::default()).unwrap().connect_unix(socket).unwrap();
            connected.submit(a.request, original).unwrap(); client = Some(connected);
        }
        let progress = client.as_mut().unwrap().drive(ClientDriveBudget::default()).unwrap();
        if let ClientProgress::Response(value) = progress.progress { response = Some(value); tick.set(1000); }
    };
    let report = if reconcile {
        serve_reconciled(c, a, settings, || ElapsedTick(2),
            &mut || Duration::from_millis(tick.get()), &mut drive)
    } else {
        serve(c, a, settings, &mut || Duration::from_millis(tick.get()), &mut drive)
    }.unwrap();
    assert!(report.frames >= 1); assert_eq!(report.candidates, 1);
    assert!(report.revision >= settings.minimum.journal_revision);
    response.unwrap()
}

#[test]
fn old_message_and_finish_receipts_use_saved_frames_after_prefix_completion() {
    let root = Directory::new(); let (c, mut a, settings, saved) = seeded(&root, 1, false);
    let before = FileOversight::read_stream_publication(&c.store, &c.profile, fixture::stream()).unwrap();
    for (key, proposal) in &saved {
        a.request = *key;
        assert!(matches!(observe(&c, &a, &settings, proposal).result,
            Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
        let state = FileOversight::read_stream_publication(&c.store, &c.profile, fixture::stream()).unwrap();
        assert_eq!(state.confirmed, before.confirmed); assert_eq!(state.published, before.published);
        assert_eq!(state.publication.executions, 3); assert_eq!(state.publication.control.ledger.charged, before.publication.control.ledger.charged);
        assert!(!a.socket.exists());
    }
    a.request = 999;
    let bytes = canonical(&c);
    assert!(serve(&c, &a, &settings, &mut || Duration::ZERO,
        &mut |_| panic!("missing request must refuse before service")).is_err());
    let state = FileOversight::read_stream_publication(&c.store, &c.profile, fixture::stream()).unwrap();
    assert_eq!(canonical(&c), bytes);
    assert_eq!(state.confirmed, before.confirmed); assert!(state.confirmed.finished());
    assert_eq!(state.publication.executions, 3); assert!(!a.socket.exists());
}

#[test]
fn published_unreconciled_message_remains_unknown_and_charged() {
    let root = Directory::new(); let (c, a, settings, saved) = seeded(&root, 2, false);
    let before = FileOversight::read_stream_publication(&c.store, &c.profile, fixture::stream()).unwrap();
    assert_eq!(before.published.message_count(), 1); assert_eq!(before.confirmed.message_count(), 0);
    assert_eq!(observe(&c, &a, &settings, &saved[0].1).result,
        Ok(Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown }));
    let after = FileOversight::read_stream_publication(&c.store, &c.profile, fixture::stream()).unwrap();
    assert_eq!(after.published, before.published); assert_eq!(after.confirmed, before.confirmed);
    assert_eq!(after.publication.control.ledger.charged, before.publication.control.ledger.charged);
    assert_eq!(after.publication.executions, 1); assert!(!after.published.finished());
}

#[test]
fn registered_but_unavailable_source_is_not_read_and_missing_key_spends_no_fence() {
    let root = Directory::new(); let (c, a, settings, _) = seeded(&root, 0, true);
    let state = FileOversight::read_stream_publication(&c.store, &c.profile, fixture::stream()).unwrap();
    let before = canonical(&c);
    assert!(serve(&c, &a, &settings, &mut || Duration::ZERO,
        &mut |_| panic!("missing request must refuse before socket service")).is_err());
    let after = FileOversight::read_stream_publication(&c.store, &c.profile, fixture::stream()).unwrap();
    assert_eq!(canonical(&c), before);
    assert_eq!(after.publication.revision, state.publication.revision);
    assert_eq!(after.publication.executions, 0); assert_eq!(after.confirmed.message_count(), 0);
    assert!(!a.socket.exists());
}

#[test]
fn reconcile_receipts_settles_published_message_before_serving_exact_intent() {
    let root = Directory::new(); let (c, a, settings, saved) = seeded(&root, 2, false);
    let before = FileOversight::read_stream_publication(&c.store, &c.profile, fixture::stream()).unwrap();
    assert_eq!(before.published.message_count(), 1); assert_eq!(before.confirmed.message_count(), 0);
    assert!(matches!(observe_reconciled(&c, &a, &settings, &saved[0].1).result,
        Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    let after = FileOversight::read_stream_publication(&c.store, &c.profile, fixture::stream()).unwrap();
    assert_eq!(after.published.message_count(), 1); assert_eq!(after.confirmed.message_count(), 1);
    assert_eq!(after.confirmed.messages().collect::<Vec<_>>(), vec!["é"]);
    assert_eq!(after.publication.executions, 1);
    assert_eq!(after.publication.control.ledger.reserved, 0);
    assert_eq!(after.publication.control.ledger.charged, before.publication.control.ledger.charged);
    assert_eq!(after.publication.revision, before.publication.revision + 3);
}

#[test]
fn reconcile_mode_requires_existing_request_before_clock_or_recovery_write() {
    let root = Directory::new(); let (c, a, settings, _) = seeded(&root, 0, false);
    let before = canonical(&c); let clocked = Cell::new(false);
    let result = serve_reconciled(&c, &a, &settings, || {
        clocked.set(true); ElapsedTick(2)
    }, &mut || Duration::ZERO, &mut |_| panic!("missing request cannot serve"));
    assert!(result.is_err()); assert!(!clocked.get());
    assert_eq!(canonical(&c), before); assert!(!a.socket.exists());
}

#[test]
fn exact_guards_history_floors_and_socket_collisions_refuse_without_cleanup() {
    let root = Directory::new(); let (c, a, mut settings, _) = seeded(&root, 0, false);
    let before = canonical(&c); let staging = c.store.join("delivery.pending"); fs::write(&staging, b"unacknowledged").unwrap();
    let minimum = settings.minimum; let history = settings.history.clone();
    for field in 0..4 {
        settings.stream = fixture::stream(); settings.registered_source = false; settings.minimum = minimum; settings.history = history.clone();
        match field {
            0 => settings.stream = StreamProfile::new(7, 2, 4, 1024, 4096).unwrap(),
            1 => settings.registered_source = true,
            2 => settings.minimum.journal_revision += 1,
            _ => { settings.history = c.store.join("local.history"); fs::copy(&history, &settings.history).unwrap(); },
        }
        assert!(serve(&c, &a, &settings, &mut || Duration::ZERO, &mut |_| panic!("refusal must precede service")).is_err());
        assert_eq!(canonical(&c), before); assert_eq!(fs::read(&staging).unwrap(), b"unacknowledged");
    }
    settings.stream = fixture::stream(); settings.registered_source = false; settings.minimum = minimum; settings.history = history;
    fs::write(&a.socket, b"unrelated socket path").unwrap();
    assert!(serve(&c, &a, &settings, &mut || Duration::ZERO, &mut |_| panic!("no service")).is_err());
    assert_eq!(fs::read(&a.socket).unwrap(), b"unrelated socket path"); assert_eq!(canonical(&c), before);
    assert_eq!(fs::read(&staging).unwrap(), b"unacknowledged"); fs::remove_file(&a.socket).unwrap();
    let tick = Cell::new(0);
    let report = serve(&c, &a, &settings, &mut || Duration::from_millis(tick.get()), &mut |_| tick.set(1000)).unwrap();
    assert_eq!(report.frames, 0); assert_eq!(report.revision, minimum.journal_revision + 1); assert!(!staging.exists());
}

#[test]
fn every_pre_fence_deadline_preserves_history_but_late_expiry_preserves_the_fence() {
    let root = Directory::new(); let (c, a, settings, _) = seeded(&root, 0, false); let before = canonical(&c);
    let checks = Cell::new(0);
    let error = serve(&c, &a, &settings, &mut || {
        if canonical(&c) == before { checks.set(checks.get() + 1); Duration::ZERO }
        else { Duration::from_millis(1000) }
    }, &mut |_| panic!("post-fence expiry cannot serve actors")).unwrap_err();
    assert_eq!(error, EXPIRED); assert_ne!(canonical(&c), before);
    let after = FileOversight::read_stream_publication(&c.store, &c.profile, fixture::stream()).unwrap();
    assert_eq!(after.publication.revision, settings.minimum.journal_revision + 1);
    for cutoff in 0..checks.get() {
        let root = Directory::new(); let (c, a, settings, _) = seeded(&root, 0, false); let before = canonical(&c);
        let pending = c.store.join("delivery.pending"); fs::write(&pending, b"preserved").unwrap(); let n = Cell::new(0);
        let error = serve(&c, &a, &settings, &mut || { let current = n.get(); n.set(current + 1);
            Duration::from_millis(if current >= cutoff { 1000 } else { 0 })
        }, &mut |_| panic!("pre-fence expiry cannot serve actors")).unwrap_err();
        assert_eq!(error, EXPIRED); assert_eq!(canonical(&c), before);
        assert_eq!(fs::read(&pending).unwrap(), b"preserved"); assert!(!a.socket.exists());
    }
}

#[test]
fn closed_stream_document_and_command_grammar_do_not_select_a_raw_fallback() {
    let root = Directory::new(); let (c, _a, s, _) = seeded(&root, 0, false);
    let valid = format!(concat!("{{\"schema\":\"fa.stream-receipt-recovery/1\",",
        "\"stream\":{{\"id\":7,\"generation\":1,\"max_messages\":4,\"max_message_bytes\":1024,\"max_stream_bytes\":4096}},",
        "\"registered_source\":false,\"minimum\":{{\"journal_revision\":{},\"control_sequence\":{},\"authority_epoch\":{}}},",
        "\"history_anchor\":\"{}\"}}"),
        s.minimum.journal_revision, s.minimum.control_sequence, s.minimum.authority_epoch, s.history.display());
    let file = root.0.join("recovery.json"); fs::write(&file, &valid).unwrap();
    let parsed = Settings::read(&file).unwrap(); assert_eq!(parsed.stream, fixture::stream());
    assert_eq!(parsed.anchor(&c).unwrap().revision(), s.minimum.journal_revision);
    for text in [valid.replace("\"registered_source\":false", "\"registered_source\":null"),
        valid.replace("\"max_messages\":4", "\"max_messages\":0"),
        valid.replace("\"registered_source\":false", "\"registered_source\":false,\"fallback\":true"),
        valid.replace("\"registered_source\":false", "\"registered_source\":false,\"registered_source\":true")] {
        assert!(Settings::decode(text.as_bytes()).is_err());
    }
    for values in [vec!["serve-create", "missing", "missing", "--stream-recovery", "missing", "--receipts-only"],
        vec!["serve-open", "missing", "missing", "--stream-recovery", "missing", "--receipts-only", "--guarded", "missing"],
        vec!["serve-open-checked", "missing", "missing", "--stream-recovery", "missing", "--receipts-only"],
        vec!["serve-open", "missing", "missing", "--stream-recovery", "missing", "--unknown-receipt-mode"]] {
        let args = values.into_iter().map(str::to_owned).collect::<Vec<_>>();
        let error = super::super::command_with(&args, None, &mut || Duration::ZERO, &mut |_| panic!("closed options")).unwrap_err();
        assert_eq!(error, USAGE);
    }
}
