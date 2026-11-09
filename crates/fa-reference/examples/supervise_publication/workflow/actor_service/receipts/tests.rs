//! Real native owners and Linux peer sessions; only scheduling time is injected.
use super::*;
#[path = "../../../../../tests/support/file_oversight.rs"]
mod fixture;
use fixture::{Directory, snapshot};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
fn directory() -> Directory {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    Directory(crate::test_directory::create(format!(
        "fa-receipt-0-{}-{stamp}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )))
}
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::oversight::actor::{
    ActorOutcome, ActorProposal, Knowledge, UnknownReason,
};
use fa_reference::action::consequence::oversight::actor_peer::{PeerCredentials, PeerPolicy};
use fa_reference::action::consequence::oversight::actor_wire::client::drive::ClientDriveBudget;
use fa_reference::action::consequence::oversight::actor_wire::client::{
    ActorClient, ActorClientState, ClientProgress, ClientSessionLimits,
};
use fa_reference::action::consequence::oversight::actor_wire::{WireError, WireResponse};
use std::{cell::Cell, fs, os::unix::net::UnixStream};

fn actor(root: &Directory, scope: fa_reference::action::Scope) -> Profile {
    let (socket, _) = UnixStream::pair().unwrap();
    let c = PeerCredentials::observe(&socket).unwrap();
    let policy = PeerPolicy::new(c.uid(), c.gid(), Some(c.pid())).unwrap();
    Profile {
        request: 700,
        scope,
        socket: root.0.join("actor.sock"),
        supervisor: policy,
        actor: policy,
        candidates: 4,
        connections: 4,
        exchanges: 32,
        runtime_ms: 100,
        poll_ms: 1,
        reply_ms: 10,
    }
}
fn proposal(h: &FileOversight) -> ActorProposal {
    ActorProposal {
        target: h.inspect().target,
        payload: b"recovery".to_vec(),
        expected_policy_epoch: 0,
        deadline: ElapsedTick(100),
        units: 16,
    }
}
fn configured(root: &Directory) -> Config {
    let mut c = Config::decode(include_bytes!(
        "../../../../../fixtures/supervised_publication.json"
    ))
    .unwrap();
    c.store = root.store();
    c.programs.clear();
    c.timing.runtime_ms = 100;
    c.timing.poll_ms = 1;
    // Deliberately absent live evidence. No receipt-serving path may acquire it.
    c.source =
        fa_reference::action::consequence::oversight::evidence_source::FileEvidenceSource::new(
            root.0.join("missing-evidence.json"),
            51,
            c.profile.delivery.scope,
            1048576,
        )
        .unwrap();
    c
}
fn prepare_client(a: &Profile, p: &ActorProposal) -> ActorClient<UnixStream> {
    let socket = UnixStream::connect(&a.socket).unwrap();
    a.supervisor.verify(&socket).unwrap();
    let mut client = ActorClientState::new(ClientSessionLimits::default())
        .unwrap()
        .connect_unix(socket)
        .unwrap();
    client.submit(a.request, p).unwrap();
    client
}
fn exercise(
    a: &Profile,
    p: &ActorProposal,
    done: &Cell<bool>,
    client: &mut Option<ActorClient<UnixStream>>,
    responses: &mut Vec<WireResponse>,
    cancel: bool,
) {
    if client.is_none() {
        *client = Some(prepare_client(a, p));
    }
    let c = client.as_mut().unwrap();
    if let ClientProgress::Response(response) =
        c.drive(ClientDriveBudget::default()).unwrap().progress
    {
        responses.push(response);
        if cancel && responses.len() == 1 {
            c.cancel(a.request).unwrap();
        } else {
            done.set(true);
        }
    }
}
fn endpoint(
    root: &Directory,
    h: FileOversight,
    a: &Profile,
    p: &ActorProposal,
    cancel: bool,
) -> Vec<WireResponse> {
    let socket = BoundSocket::bind(&a.socket, None).unwrap();
    a.secure_socket().unwrap();
    let done = Cell::new(false);
    let waits = Cell::new(0);
    let mut client = None;
    let mut responses = Vec::new();
    let report = serve_owner(
        h,
        a,
        socket,
        Duration::from_millis(100),
        &mut || {
            if done.get() {
                Duration::from_millis(100)
            } else {
                Duration::ZERO
            }
        },
        &mut |_| {
            waits.set(waits.get() + 1);
            assert!(waits.get() < 64);
            exercise(a, p, &done, &mut client, &mut responses, cancel);
        },
    )
    .unwrap();
    assert_eq!(report.frames, responses.len());
    assert_eq!(report.candidates, 1);
    assert!(!a.socket.exists());
    assert!(root.store().exists());
    responses
}
fn authorized(root: &Directory, phase: u8) -> (FileOversight, ActorProposal) {
    let (mut h, human) = fixture::create(root);
    h.enable_publication_guard(h.revision()).unwrap();
    let p = proposal(&h);
    let mut spec = fixture::spec(&h, &p.payload);
    spec.deadline = p.deadline;
    h.submit_request(h.revision(), 700, spec, snapshot())
        .unwrap();
    let action = h.request_action(700).unwrap().clone();
    let inputs = fixture::inputs(&action, b"original whole view");
    fixture::review_existing(&mut h, 1, 101, &inputs);
    let automatic = h.authorize(h.revision(), 1, &inputs, snapshot()).unwrap();
    let req = h
        .request_human_approval(h.revision(), 1001, 1, &inputs, ElapsedTick(30))
        .unwrap();
    let r = h.revision();
    let approval = human.approve(&mut h, r, &req).unwrap();
    if phase > 0 {
        h.dispatch(
            h.revision(),
            &automatic,
            &approval,
            &action,
            &inputs,
            snapshot(),
        )
        .unwrap();
    }
    if phase > 1 {
        h.publish_checked(h.revision(), 1, Some(&inputs), snapshot(), ElapsedTick(1))
            .unwrap();
    }
    if phase > 2 {
        h.reconcile(h.revision(), 1).unwrap();
    }
    (h, p)
}

#[test]
fn actual_peer_receipt_and_cancel_retain_native_unknown_and_executed_liabilities() {
    for phase in 0..4 {
        let root = directory();
        let (h, p) = authorized(&root, phase);
        let a = actor(&root, fixture::profile().delivery.scope);
        let before = h.inspect();
        let bytes = fs::read(root.store().join("delivery.bin")).unwrap();
        let replies = endpoint(&root, h, &a, &p, true);
        assert_eq!(replies.len(), 2);
        let after = FileOversight::read_publication(root.store(), &fixture::profile()).unwrap();
        if phase == 0 {
            assert!(matches!(
                replies[1].result,
                Ok(Knowledge::Known {
                    value: ActorOutcome::CancelledBeforeDispatch,
                    ..
                })
            ));
            assert_eq!(after.control.ledger.reserved, 0);
            assert_eq!(after.executions, 0);
        } else {
            assert_eq!(fs::read(root.store().join("delivery.bin")).unwrap(), bytes);
            assert_eq!(after, before);
            assert_eq!(after.control.ledger.charged, 16);
            if phase < 3 {
                assert_eq!(
                    replies[1].result,
                    Ok(Knowledge::Unknown {
                        reason: UnknownReason::OutcomeUnknown
                    })
                );
            } else {
                assert!(matches!(
                    replies[1].result,
                    Ok(Knowledge::Known {
                        value: ActorOutcome::Executed,
                        ..
                    })
                ));
            }
        }
    }
}

#[test]
fn actual_open_serves_retained_refusal_without_live_assets_or_a_replacement_request() {
    let root = directory();
    let c = configured(&root);
    let a = actor(&root, c.profile.delivery.scope);
    let (mut h, _) = prepare_host(&c, None, false).unwrap();
    h.observe_time(h.revision(), ElapsedTick(1)).unwrap();
    let p = proposal(&h);
    let spec = fa_reference::action::ActionSpec {
        version: 1,
        scope: c.profile.delivery.scope,
        target: Some(p.target),
        payload: p.payload.clone(),
        required_witnesses: Vec::new(),
        policy_epoch: 0,
        deadline: p.deadline,
        units: p.units,
    };
    // No ready source: native admission records the refusal. Recovery must
    // retain that original disposition instead of rerunning admission with data.
    h.submit_request(h.revision(), 700, spec, snapshot())
        .unwrap();
    let before = h.revision();
    drop(h);
    let done = Cell::new(false);
    let count = Cell::new(0);
    let mut client = None;
    let mut replies = Vec::new();
    let report = serve(
        &c,
        &a,
        None,
        &mut || {
            if done.get() {
                Duration::from_millis(100)
            } else {
                Duration::ZERO
            }
        },
        &mut |_| {
            count.set(count.get() + 1);
            assert!(count.get() < 64);
            exercise(&a, &p, &done, &mut client, &mut replies, false);
        },
    )
    .unwrap();
    assert_eq!(
        report.revision,
        before + 1,
        "only the original recovery fence may append"
    );
    assert!(matches!(
        replies[0].result,
        Ok(Knowledge::Known {
            value: ActorOutcome::NotAdmitted,
            ..
        })
    ));
    assert!(!root.0.join("missing-evidence.json").exists());
    assert!(!a.socket.exists());
}

#[test]
fn absent_selected_request_stays_withheld_and_never_allocates_after_open() {
    let root = directory();
    let c = configured(&root);
    let a = actor(&root, c.profile.delivery.scope);
    let (h, _) = prepare_host(&c, None, false).unwrap();
    let p = proposal(&h);
    let before = h.revision();
    drop(h);
    let done = Cell::new(false);
    let count = Cell::new(0);
    let mut client = None;
    let mut replies = Vec::new();
    let report = serve(
        &c,
        &a,
        None,
        &mut || {
            if done.get() {
                Duration::from_millis(100)
            } else {
                Duration::ZERO
            }
        },
        &mut |_| {
            count.set(count.get() + 1);
            assert!(count.get() < 64);
            exercise(&a, &p, &done, &mut client, &mut replies, false);
        },
    )
    .unwrap();
    assert_eq!(replies[0].result, Err(WireError::Withheld));
    assert_eq!(report.revision, before + 1);
    let (h, _) = FileOversight::open(&c.store, c.profile.clone()).unwrap();
    assert_eq!(h.retained_requests(), 0);
    assert_eq!(h.inspect().executions, 0);
}

#[test]
fn closed_flags_foreign_scope_and_socket_collision_refuse_without_recovery() {
    let args = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    for bad in [
        vec!["serve-create", "/missing", "/missing", "--receipts-only"],
        vec![
            "serve-open",
            "/missing",
            "/missing",
            "--receipts-only",
            "--requests",
            "700",
        ],
        vec![
            "serve-open",
            "/missing",
            "/missing",
            "--peers",
            "--receipts-only",
        ],
        vec![
            "serve-open-checked",
            "/missing",
            "/missing",
            "--receipts-only",
        ],
        vec![
            "serve-open-checked",
            "/missing",
            "/missing",
            "--guarded",
            "--receipts-only",
        ],
    ] {
        assert_eq!(super::super::command(&args(&bad), None), Err(USAGE.into()));
    }
    assert!(
        command_with(
            &args(&["serve-open", "/missing", "/missing", "--receipts-only"]),
            Some(Path::new("/missing-capsule")),
            &mut || Duration::ZERO,
            &mut |_| panic!("no I/O")
        )
        .is_err()
    );
    let root = directory();
    let c = configured(&root);
    let (h, _) = prepare_host(&c, None, false).unwrap();
    drop(h);
    let before = fs::read(c.store.join("delivery.bin")).unwrap();
    let mut a = actor(&root, c.profile.delivery.scope);
    a.scope.run += 1;
    assert!(
        serve(&c, &a, None, &mut || Duration::ZERO, &mut |_| panic!(
            "foreign profile"
        ))
        .is_err()
    );
    a.scope.run -= 1;
    fs::write(&a.socket, b"preserve existing path").unwrap();
    assert!(
        serve(&c, &a, None, &mut || Duration::ZERO, &mut |_| panic!(
            "socket collision"
        ))
        .is_err()
    );
    assert_eq!(fs::read(&a.socket).unwrap(), b"preserve existing path");
    assert_eq!(fs::read(c.store.join("delivery.bin")).unwrap(), before);
}

#[test]
fn startup_deadline_before_open_and_after_fence_are_not_conflated() {
    for cutoff in [0, 2, 3] {
        let root = directory();
        let c = configured(&root);
        let a = actor(&root, c.profile.delivery.scope);
        let (h, _) = prepare_host(&c, None, false).unwrap();
        let revision = h.revision();
        drop(h);
        let calls = Cell::new(0);
        let before = fs::read(c.store.join("delivery.bin")).unwrap();
        let result = serve(
            &c,
            &a,
            None,
            &mut || {
                let n = calls.get();
                calls.set(n + 1);
                Duration::from_millis(if n >= cutoff { 100 } else { 0 })
            },
            &mut |_| panic!("deadline prevents protocol work"),
        );
        assert!(matches!(result, Err(ref error) if error == EXPIRED));
        let after = FileOversight::read_publication(&c.store, &c.profile).unwrap();
        assert_eq!(after.revision, revision + u64::from(cutoff == 3));
        if cutoff < 3 {
            assert_eq!(fs::read(c.store.join("delivery.bin")).unwrap(), before);
        }
        assert!(!a.socket.exists());
        assert_eq!(after.executions, 0);
    }
}

#[test]
fn rejected_kernel_credentials_consume_quota_without_returning_request_data() {
    use std::io::Read;
    let root = directory();
    let (h, p) = authorized(&root, 3);
    let mut a = actor(&root, fixture::profile().delivery.scope);
    a.actor = PeerPolicy::new(
        if a.actor.uid() == 0 { 1 } else { 0 },
        a.actor.gid(),
        a.actor.pid(),
    )
    .unwrap();
    a.candidates = 1;
    a.connections = 1;
    let socket = BoundSocket::bind(&a.socket, None).unwrap();
    a.secure_socket().unwrap();
    let before = fs::read(root.store().join("delivery.bin")).unwrap();
    let mut peer = None;
    let result = serve_owner(
        h,
        &a,
        socket,
        Duration::from_millis(100),
        &mut || Duration::ZERO,
        &mut |_| {
            assert!(peer.is_none());
            let s = UnixStream::connect(&a.socket).unwrap();
            s.set_nonblocking(true).unwrap();
            peer = Some(s);
        },
    );
    assert!(result.is_err());
    assert_eq!(peer.as_mut().unwrap().read(&mut [0; 1]).unwrap(), 0);
    assert_eq!(fs::read(root.store().join("delivery.bin")).unwrap(), before);
    // Matching credential positive controls use the same executed request above.
    assert_eq!(p.payload, b"recovery");
    assert!(!a.socket.exists());
}
