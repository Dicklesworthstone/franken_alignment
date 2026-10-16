//! Actual imported generation, native review and peer transport, with injected
//! scheduling only. Fixture weights/forecasts are not detector qualification.
use super::*;
use super::continuation as native;
use super::super::{anchors as history_anchors, prediction,
    recovery::readonly::service::{self, Mode}};
use crate::workflow::actor_service::Profile;
use fa_reference::action::consequence::delivery::persistent::{FileDeliverySnapshot,
    observed::guarded::anchored::MAX_HISTORY_ANCHOR_BYTES};
use fa_reference::action::consequence::oversight::{
    actor_peer::{PeerCredentials, PeerPolicy},
    actor_wire::{WireError, WireResponse, client::{ActorClient, ActorClientState,
        ClientProgress, ClientSessionLimits, drive::ClientDriveBudget}},
};
use std::{cell::Cell, collections::BTreeMap, io::Read, os::unix::net::UnixStream,
    path::{Path, PathBuf}};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Cut { Pending, Cancelled, Published, Confirmed, FinishPending }
struct Case {
    root: Root, config: Config, loaded: Loaded, recipe: PathBuf, anchor: PathBuf,
    actor: Profile, before: FileDeliverySnapshot, outcome: Knowledge<ActorOutcome>, after: Option<u64>,
}
impl Case {
    fn new(mode: Option<bool>, cut: Cut) -> Self {
        let root = Root::new(); let mut config = configured(&root);
        let mut path = native::write(&root);
        if let Some(owned) = mode { super::forecast::install(&root, &path, owned, 60_000); }
        evidence(&root, &config, 1, true, false);
        let mut loaded = recipe::load(&path, &config, true).unwrap();
        let (host, reviewer, observer) = recovery::create_selected(&config, &loaded).unwrap();
        let mut prediction = observer.map(|role| prediction::Session::new(loaded.request, role));
        let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let mut driver = FileSupervisedDriver::new(supervisor); let mut wire = ActorWire::new(port);
        let deadline = deadline(); let mut control = Control::new(&config, loaded.request, None).unwrap();
        assert!(generate_selected(&mut driver, &reviewer, &mut config, &mut control, &deadline,
            &mut || ElapsedTick(1000), prediction.as_mut()).unwrap());
        assert_eq!(driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical.sampled_draws, 2);
        submit(&mut driver, &mut wire, &mut config, &loaded, &deadline, &mut || ElapsedTick(1000)).unwrap();
        if cut == Cut::Cancelled {
            assert!(wire.exchange(&encode_command(&Command::Cancel { request: loaded.request }).unwrap()).result.is_ok());
        }
        if matches!(cut, Cut::Published | Cut::Confirmed | Cut::FinishPending) {
            let (reviewed, history) = native::review_current(&mut config, &loaded, &reviewer, driver, &deadline);
            driver = reviewed;
            let human = native::approve(&mut driver, &mut config, &reviewer, loaded.request);
            assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
                || ElapsedTick(1000), Some(&human)).result.unwrap(), FileDriverEvent::Dispatched { .. }));
            assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
                || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::PublicationChecked { .. }));
            if cut != Cut::Published {
                assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
                    || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::Reconciled { .. }));
                assert!(executed(&mut wire, loaded.request));
            }
            drop(history);
        }
        let after = if cut == Cut::FinishPending {
            assert_eq!(cleanup(driver, 1000, 1), 0); drop(prediction);
            (path, loaded) = native::next(&root, &config, 2);
            let (host, reviewer, observer) = recovery::open_selected(&config, &loaded).unwrap();
            let mut prediction = observer.map(|role| prediction::Session::new(loaded.request, role));
            let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
            driver = FileSupervisedDriver::new(supervisor); wire = ActorWire::new(port);
            let mut control = Control::new(&config, 2, None).unwrap();
            assert!(super::super::finish::prepare(&mut driver, &reviewer, &mut config, &loaded,
                1, &mut control, &deadline, &mut || ElapsedTick(1000)).unwrap());
            if let Some(prediction) = prediction.as_mut() {
                prediction.finish_request(&mut driver.supervisor_mut().host_mut().unwrap()).unwrap();
            }
            submit_release(&mut driver, &mut wire, &mut config, &loaded, LearnedTextRelease::Finish,
                &deadline, &mut || ElapsedTick(1000)).unwrap();
            Some(1)
        } else { None };
        let outcome = response(&mut wire, loaded.request).result.unwrap();
        let host = driver.supervisor().host().unwrap();
        let before = host.inspect(); let mut archive = Vec::new();
        host.history_anchor().unwrap().write_to(&mut archive, MAX_HISTORY_ANCHOR_BYTES).unwrap();
        drop(host); assert_eq!(cleanup(driver, 1000, 1), 0); drop(wire);
        let anchor = root.0.join("receipt.anchor"); fs::write(&anchor, archive).unwrap();
        let mut loaded = recipe::load(&path, &config, false).unwrap(); assert!(loaded.native.is_none());
        loaded.recovery_anchor = Some(history_anchors::load(&anchor).unwrap());
        loaded.floor.journal_revision = before.revision;
        loaded.floor.control_sequence = before.control.sequence;
        loaded.floor.authority_epoch = before.control.ledger.epoch;
        for name in ["evidence.json", "native-roster.json", "helper-model.json", "helper-weights.safetensors",
            "helper-monitor.json", "helper-tokenizer.bbpe", "helper-sampling.json"] {
            fs::remove_file(root.0.join(name)).unwrap();
        }
        fs::write(config.store.join("delivery.pending"), b"unconfirmed receipt-service staging").unwrap();
        let (socket, _) = UnixStream::pair().unwrap(); let c = PeerCredentials::observe(&socket).unwrap();
        let peer = PeerPolicy::new(c.uid(), c.gid(), Some(c.pid())).unwrap();
        let actor = Profile { request: loaded.request, scope: config.profile.delivery.scope,
            socket: root.0.join("peers/receipt.sock"), supervisor: peer, actor: peer,
            candidates: 4, connections: 4, exchanges: 32, runtime_ms: 100, poll_ms: 1, reply_ms: 10 };
        Self { root, config, loaded, recipe: path, anchor, actor, before, outcome, after }
    }
    fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fs::read_dir(&self.config.store).unwrap().map(|entry| entry.unwrap().path())
            .filter(|path| path.is_file()).map(|path| {
                let bytes = fs::read(&path).unwrap(); (path, bytes)
            }).collect()
    }
    fn inspect(&self) -> FileDeliverySnapshot {
        if let Some(prediction) = self.loaded.generation.required_owned_pre_output_forecast() {
            FileOversight::read_publication_with_owned_learned_consistency(&self.config.store,
                &self.config.profile, &self.loaded.generation, prediction).unwrap().publication
        } else {
            FileOversight::read_publication_with_learned_generation(&self.config.store,
                &self.config.profile, &self.loaded.generation).unwrap()
        }
    }
}
fn open_client(case: &Case) -> ActorClient<UnixStream> {
    let socket = UnixStream::connect(&case.actor.socket).unwrap(); case.actor.supervisor.verify(&socket).unwrap();
    let mut client = ActorClientState::new(ClientSessionLimits::default()).unwrap().connect_unix(socket).unwrap();
    let release = if case.after.is_some() { LearnedTextRelease::Finish } else { LearnedTextRelease::Message };
    client.submit(case.loaded.request, &FileLearnedTextStreamActorPort::encode_release(
        case.loaded.request, release, deadline().logical).unwrap()).unwrap(); client
}
fn exchange(case: &Case, client: &mut Option<ActorClient<UnixStream>>, replies: &mut Vec<WireResponse>, done: &Cell<bool>) {
    if client.is_none() { *client = Some(open_client(case)); }
    let progress = client.as_mut().unwrap().drive(ClientDriveBudget::default()).unwrap().progress;
    if let ClientProgress::Response(reply) = progress {
        replies.push(reply);
        match replies.len() {
            1 => client.as_mut().unwrap().cancel(case.loaded.request).unwrap(),
            2 => { *client = None; } // Reconnect with a new wire ticket domain.
            3 => done.set(true),
            _ => panic!("unexpected additional response"),
        }
    }
}
fn run(case: &Case, mode: Mode) -> Vec<WireResponse> {
    let done = Cell::new(false); let count = Cell::new(0); let clocks = Cell::new(0);
    let mut client = None; let mut replies = Vec::new();
    let report = service::serve(&case.config, &case.loaded, &case.actor, case.after, mode,
        || { assert_eq!(mode, Mode::Reconcile); clocks.set(clocks.get() + 1); ElapsedTick(1001) },
        &mut || if done.get() { Duration::from_millis(100) } else { Duration::ZERO },
        &mut |_| {
            count.set(count.get() + 1); assert!(count.get() < 128);
            // No Store or numerical owner may survive into the peer wait loop.
            if count.get() == 1 {
                drop(FileOversight::begin_open_with_learned_generation(&case.config.store,
                    case.config.profile.clone(), &case.loaded.generation).unwrap());
            }
            exchange(case, &mut client, &mut replies, &done);
        }).unwrap();
    assert_eq!(clocks.get(), usize::from(mode == Mode::Reconcile));
    assert_eq!(report.frames, 3); assert_eq!(report.candidates, 2);
    assert_eq!(replies[1].result, Err(WireError::Withheld));
    assert_eq!(replies[2].request, replies[0].request);
    assert_eq!(replies[2].result, replies[0].result); assert!(!case.actor.socket.exists()); replies
}

#[test]
fn authenticated_service_preserves_all_three_learned_modes_without_live_assets_or_cancellation() {
    for mode in [None, Some(false), Some(true)] {
        for cut in [Cut::Pending, Cut::Cancelled] {
            let case = Case::new(mode, cut); let before = case.files();
            let reads = case.config.source.status().read_attempts;
            let replies = run(&case, Mode::Passive);
            assert_eq!(replies[0].result, Ok(case.outcome.clone()));
            assert_eq!(case.files(), before); assert_eq!(case.inspect(), case.before);
            assert_eq!(case.config.source.status().read_attempts, reads);
            assert_eq!(fs::read_dir(case.root.0.join("peers")).unwrap().count(), 0);
        }
    }
}

#[test]
fn service_can_observe_unknown_publication_or_reconcile_its_original_receipt_exactly_once() {
    for predictor in [None, Some(false), Some(true)] {
        for mode in [Mode::Passive, Mode::Reconcile] {
            let case = Case::new(predictor, Cut::Published); let before = case.files();
            assert!(matches!(case.outcome, Knowledge::Unknown { .. }));
            let reads = case.config.source.status().read_attempts;
            let replies = run(&case, mode);
            let after = case.inspect(); assert_eq!(after.executions, 1);
            assert_eq!(after.control.ledger.charged, case.before.control.ledger.charged);
            if mode == Mode::Passive {
                assert_eq!(replies[0].result, Ok(case.outcome.clone()));
                assert_eq!(after, case.before); assert_eq!(case.files(), before);
            } else {
                assert!(matches!(replies[0].result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
                assert_eq!(after.revision, case.before.revision + 3);
                assert!(!case.config.store.join("delivery.pending").exists());
            }
            assert_eq!(case.config.source.status().read_attempts, reads);
        }
    }
}

#[test]
fn service_selects_the_actual_finish_and_its_confirmed_predecessor_before_socket_binding() {
    for mode in [None, Some(false), Some(true)] {
        let case = Case::new(mode, Cut::FinishPending); let before = case.files();
        for after in [None, Some(99)] {
            assert!(service::serve(&case.config, &case.loaded, &case.actor, after, Mode::Reconcile,
                || panic!("wrong finish selection cannot read time"), &mut || Duration::ZERO,
                &mut |_| panic!("wrong finish cannot serve")).is_err());
            assert_eq!(case.files(), before); assert!(!case.actor.socket.exists());
        }
        let replies = run(&case, Mode::Passive);
        assert_eq!(replies[0].result, Ok(case.outcome.clone()));
        assert_eq!(case.files(), before); assert_eq!(case.inspect().executions, 1);
    }
}

#[test]
fn request_scope_guards_and_socket_collisions_refuse_before_reconciliation() {
    for mode in [None, Some(false), Some(true)] {
        let mut case = Case::new(mode, Cut::Cancelled); let before = case.files();
        for failure in 0..5 {
            let key = case.loaded.request; let floor = case.loaded.floor; let scope = case.actor.scope;
            match failure {
                0 => case.actor.request += 1,
                1 => { case.loaded.request = 99; case.actor.request = 99; }
                2 => case.loaded.floor.journal_revision += 1,
                3 => case.actor.scope.run += 1,
                _ => fs::write(&case.actor.socket, b"existing actor path").unwrap(),
            }
            assert!(service::serve(&case.config, &case.loaded, &case.actor, None, Mode::Reconcile,
                || panic!("failed admission cannot read reconciliation time"), &mut || Duration::ZERO,
                &mut |_| panic!("failed admission cannot serve")).is_err());
            assert_eq!(case.files(), before);
            if failure == 4 {
                assert_eq!(fs::read(&case.actor.socket).unwrap(), b"existing actor path");
                fs::remove_file(&case.actor.socket).unwrap();
            } else { assert!(!case.actor.socket.exists()); }
            case.loaded.request = key; case.actor.request = key; case.loaded.floor = floor; case.actor.scope = scope;
        }
        let anchor = case.loaded.recovery_anchor.take();
        assert!(service::serve(&case.config, &case.loaded, &case.actor, None, Mode::Passive,
            || panic!("no clock"), &mut || Duration::ZERO, &mut |_| panic!("missing anchor")).is_err());
        case.loaded.recovery_anchor = anchor;
        assert_eq!(case.files(), before); assert_eq!(run(&case, Mode::Passive)[0].result, Ok(case.outcome.clone()));
    }
}

#[test]
fn every_replay_and_pre_finalization_deadline_leaves_history_and_staging_unchanged() {
    for predictor in [None, Some(false), Some(true)] {
        let case = Case::new(predictor, Cut::Cancelled); let before = case.files();
        let events = usize::try_from(case.before.revision).unwrap();
        for mode in [Mode::Passive, Mode::Reconcile] {
            // Two pre-open checks, N events plus Ready, then post-bind check.
            for cutoff in 0..=events + 3 {
                let calls = Cell::new(0);
                let result = service::serve(&case.config, &case.loaded, &case.actor, None, mode,
                    || panic!("expired replay cannot invoke the final clock"), &mut || {
                        let now = calls.get(); calls.set(now + 1);
                        if now == cutoff { Duration::from_millis(100) } else { Duration::ZERO }
                    }, &mut |_| panic!("expired startup cannot serve"));
                assert!(result.unwrap_err().contains("startup deadline reached"));
                assert_eq!(calls.get(), cutoff + 1); assert_eq!(case.files(), before);
                assert!(!case.actor.socket.exists());
            }
        }
        // Passive expiry after the final canonical re-read still cannot write.
        let calls = Cell::new(0);
        let result = service::serve(&case.config, &case.loaded, &case.actor, None, Mode::Passive,
            || panic!("passive finalizer has no clock"), &mut || {
                let now = calls.get(); calls.set(now + 1);
                if now == events + 4 { Duration::from_millis(100) } else { Duration::ZERO }
            }, &mut |_| panic!("expired historical observation is withheld"));
        assert!(result.unwrap_err().contains("startup deadline reached"));
        assert_eq!(case.files(), before); assert!(!case.actor.socket.exists());
    }
}

#[test]
fn expiry_after_reconciliation_reports_acknowledgment_and_retains_the_original_effect() {
    for predictor in [None, Some(false), Some(true)] {
        let case = Case::new(predictor, Cut::Published); let late = Cell::new(false);
        let calls = Cell::new(0);
        let error = service::serve(&case.config, &case.loaded, &case.actor, None, Mode::Reconcile,
            || { calls.set(calls.get() + 1); late.set(true); ElapsedTick(1001) },
            &mut || if late.get() { Duration::from_millis(100) } else { Duration::ZERO },
            &mut |_| panic!("expired acknowledged recovery cannot serve")).unwrap_err();
        assert!(error.contains("reconciliation acknowledged"));
        assert!(error.contains("no effect retry was sent")); assert_eq!(calls.get(), 1);
        let after = case.inspect(); assert_eq!(after.revision, case.before.revision + 3);
        assert_eq!(after.executions, 1); assert!(!case.actor.socket.exists());
        let completed = case.files(); let replies = run(&case, Mode::Passive);
        assert!(matches!(replies[0].result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
        assert_eq!(case.files(), completed); assert_eq!(case.inspect(), after);
    }
}

#[test]
fn rejected_kernel_credentials_return_no_receipt_and_consume_the_original_candidate_quota() {
    let mut case = Case::new(Some(true), Cut::Confirmed); let before = case.files();
    case.actor.actor = PeerPolicy::new(if case.actor.actor.uid() == 0 { 1 } else { 0 },
        case.actor.actor.gid(), case.actor.actor.pid()).unwrap();
    case.actor.candidates = 1; case.actor.connections = 1;
    let mut peer = None;
    let result = service::serve(&case.config, &case.loaded, &case.actor, None, Mode::Passive,
        || panic!("passive service cannot acquire time"), &mut || Duration::ZERO, &mut |_| {
            assert!(peer.is_none()); let socket = UnixStream::connect(&case.actor.socket).unwrap();
            socket.set_nonblocking(true).unwrap(); peer = Some(socket);
        });
    assert!(result.is_err()); assert_eq!(peer.as_mut().unwrap().read(&mut [0; 1]).unwrap(), 0);
    assert_eq!(case.files(), before); assert!(!case.actor.socket.exists());
}

fn config_file(case: &Case) -> PathBuf {
    let text = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/supervised_publication.json"))
        .replace("/operator/publication-store", case.config.store.to_str().unwrap())
        .replace("/operator/evidence.json", case.root.0.join("evidence.json").to_str().unwrap())
        .replace("\"initial_payload_hex\": \"696e697469616c\"", "\"initial_payload_hex\": \"\"")
        .replace("\"events\": 256", "\"events\": 1024");
    let path = case.root.0.join("service-config.json"); fs::write(&path, text).unwrap();
    let read = Config::read(&path).unwrap(); assert_eq!(read.profile, case.config.profile);
    assert_eq!(read.source_policy, case.config.source_policy); path
}
fn actor_file(case: &Case) -> PathBuf {
    let a = &case.actor; let s = a.scope; let p = a.supervisor;
    let text = format!(r#"{{"schema":"fa.actor-service/1","clock":"unix_milliseconds","request":{},"scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},"socket":"{}","supervisor":{{"uid":{},"gid":{},"pid":{}}},"actor":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":4,"connection_limit":4,"exchange_limit":32,"runtime_ms":100,"poll_ms":1,"reply_ms":10}}"#,
        a.request, s.tenant, s.principal, s.run, s.branch, s.authority, a.socket.display(),
        p.uid(), p.gid(), p.pid().unwrap(), p.uid(), p.gid(), p.pid().unwrap());
    let path = case.root.0.join("actor.json"); fs::write(&path, text).unwrap(); path
}

#[test]
fn real_service_command_loads_only_historical_assets_and_closes_its_option_grammar_before_io() {
    for predictor in [None, Some(false), Some(true)] {
        let case = Case::new(predictor, Cut::Cancelled); let before = case.files();
        let config = config_file(&case); let actor = actor_file(&case);
        let args = vec!["create-learned-generated".into(), config.to_str().unwrap().into(),
            case.recipe.to_str().unwrap().into(), "--resume".into(), "--serve-receipts".into(),
            actor.to_str().unwrap().into(), "--history-anchor".into(), case.anchor.to_str().unwrap().into(),
            "--receipts-only".into()];
        let done = Cell::new(false); let count = Cell::new(0); let mut client = None; let mut replies = Vec::new();
        let report = service::command_with(&args, None, || panic!("passive command cannot read a logical clock"),
            &mut || if done.get() { Duration::from_millis(100) } else { Duration::ZERO }, &mut |_| {
                count.set(count.get() + 1); assert!(count.get() < 128);
                exchange(&case, &mut client, &mut replies, &done);
            }).unwrap();
        assert_eq!(report.frames, 3); assert_eq!(report.candidates, 2);
        assert_eq!(replies[0].result, Ok(case.outcome.clone())); assert_eq!(case.files(), before);
    }
    let base: Vec<String> = ["create-learned-generated", "/missing/config", "/missing/recipe",
        "--resume", "--serve-receipts", "/missing/actor", "--history-anchor", "/missing/anchor",
        "--receipts-only"].into_iter().map(str::to_owned).collect();
    let mut malformed = Vec::new();
    for index in [0, 3, 4, 6, 8] {
        let mut changed = base.clone(); changed[index] = "--unsupported".into(); malformed.push(changed);
    }
    for index in [1, 2, 5, 7] {
        let mut changed = base.clone(); changed[index] = "--reconcile-receipts".into(); malformed.push(changed);
    }
    for extra in ["--reconcile-receipts", "--continue-generation", "--recover-stop", "--retain-history-anchor"] {
        let mut changed = base.clone(); changed.push(extra.into()); malformed.push(changed);
    }
    for args in malformed {
        let error = service::command_with(&args, None, || panic!("malformed mode cannot read time"),
            &mut || panic!("malformed mode cannot start intake"), &mut |_| panic!("malformed mode cannot serve")).unwrap_err();
        assert!(error.starts_with("create-learned-generated CONFIG LEARNED_RECIPE --resume --serve-receipts"));
    }
    assert!(service::command_with(&base, Some(Path::new("/missing/capsule")),
        || panic!("no logical clock"), &mut || panic!("no intake"), &mut |_| panic!("no serving")).is_err());
    // The actual outer receipt router reaches this grammar, not live generation.
    let mut invalid = base; invalid.push("--reconcile-receipts".into());
    assert!(crate::command(invalid).unwrap_err().contains("--serve-receipts ACTOR_PROFILE"));
}
