//! Real imported numerics, native terminal consumers and authenticated Unix Stop.
//! Byte equality with the original offline consumer is the state oracle. Fixtures
//! are not detector qualification, deployment authentication or latency evidence.
use super::*;
use super::super::{continuation as native, mediated_topology};
use super::super::super::{anchors, topology};
use fa_reference::action::ActionState;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::{
    FileDeliverySnapshot, Reconciliation,
    observed::{
        consistency::learned::FileLearnedConsistencySnapshot,
        driver::topology::checkpoint::{TopologyCheckpoint, MAX_TOPOLOGY_CHECKPOINT_BYTES},
        guarded::anchored::FileHistoryAnchor,
        reviewer::peer::{PeerPolicy, VerifiedReviewerSocket,
            control::{StopBinding, StopClientPhase, StopClientProgress, StopControlReceipt,
                StopRecoveryApplication, StopStatus}},
        shutdown::recovery::FileAdmissionStoppedHistory,
        stream::FileStreamSnapshot,
    },
};
use std::{io::{Cursor, Write}, path::PathBuf, thread::{self, JoinHandle}};

#[path = "durable/refusals.rs"]
mod refusals;

struct NativeCase {
    root: Root,
    config: Config,
    loaded: Loaded,
    before: FileDeliverySnapshot,
    numerical: HostedLearnedInspection,
    owned: Option<FileLearnedConsistencySnapshot>,
    stream: FileStreamSnapshot,
    anchor: PathBuf,
    archive: Vec<u8>,
    source: Option<TopologyCheckpoint>,
    source_output: Option<PathBuf>,
    reads: u64,
}

fn prepared_native(mode: Option<bool>, mediated: bool, published_unknown: bool) -> NativeCase {
    let root = Root::new();
    let (mut config, path, loaded) = if mediated {
        mediated_topology::selected(&root, mode, false)
    } else {
        let config = configured(&root);
        let path = fixture::write(&root, b"allow");
        if let Some(owned) = mode { super::super::forecast::install(&root, &path, owned, 60_000); }
        evidence(&root, &config, 1, true, false);
        let loaded = recipe::load(&path, &config, true).unwrap();
        (config, path, loaded)
    };
    let limit = deadline();
    let mut control = Control::new(&config, loaded.request, None).unwrap();
    let topology::Started { custody: (host, reviewer, observer, _identity), source, output } =
        topology::start(&config, &loaded, false, &mut control, &limit,
            &mut || ElapsedTick(1000)).unwrap();
    let mut forecast = observer.map(|observer| prediction::Session::new(loaded.request, observer));
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut wire = ActorWire::new(port);
    topology::attach(&mut driver, source).unwrap();
    topology::certify(&mut driver, &limit, &mut || ElapsedTick(1000)).unwrap();
    assert!(generate_selected(&mut driver, &reviewer, &mut config, &mut control,
        &limit, &mut || ElapsedTick(1000), forecast.as_mut()).unwrap());
    if published_unknown {
        submit(&mut driver, &mut wire, &mut config, &loaded, &limit,
            &mut || ElapsedTick(1000)).unwrap();
        let (reviewed, history) = native::review_current(
            &mut config, &loaded, &reviewer, driver, &limit);
        driver = reviewed;
        let human = native::approve(&mut driver, &mut config, &reviewer, loaded.request);
        assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
            || ElapsedTick(1000), Some(&human)).result.unwrap(), FileDriverEvent::Dispatched { .. }));
        // A real unanswered endpoint query records Unknown before the queued
        // envelope executes. Its later publication is deliberately unreconciled.
        {
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision();
            assert_eq!(host.reconcile(revision, 1).unwrap(), Reconciliation::AwaitingResolution);
            assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Unknown);
        }
        // The supervised driver correctly switches Unknown into query-only
        // reconciliation. Exercise this already queued envelope through the
        // original checked host operation instead: genuine native review input,
        // fresh policy-file capture, and the same live owner's full guard law.
        let captured = refresh(&mut driver, &mut config, &limit, &mut || ElapsedTick(1000)).unwrap();
        {
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision();
            let publication = host.publish_checked(revision, 1, Some(history.input()),
                captured.snapshot().clone(), ElapsedTick(1000)).unwrap();
            assert!(matches!(publication.outcome, EndpointOutcome::Executed { .. }));
            assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Unknown);
        }
        assert!(matches!(response(&mut wire, loaded.request).result.unwrap(), Knowledge::Unknown { .. }));
        drop(history);
    }
    let host = driver.supervisor().host().unwrap();
    let before = host.inspect();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let owned = (mode == Some(true)).then(|| host.learned_action_consistency_snapshot().unwrap());
    let stream = host.stream_snapshot().unwrap();
    let original_anchor = host.history_anchor().unwrap();
    assert_eq!(numerical.position, 4);
    assert_eq!(numerical.sampled_draws, 2);
    assert!(numerical.work.accepted_decoder.scalar_products().unwrap() > 0);
    assert!(before.stop.is_none());
    assert_eq!(before.executions, u64::from(published_unknown));
    if published_unknown {
        assert_eq!(before.control.ledger.stages[&1], ActionState::Unknown);
        assert!(before.control.ledger.charged > 0);
        assert_eq!(stream.pending, Some(1));
        assert_ne!(stream.confirmed, stream.published);
    }
    drop(host);
    let anchor = root.0.join("recovery-stop-before.anchor");
    anchors::export_acknowledged(&original_anchor, &anchor).unwrap();
    let archive = fs::read(&anchor).unwrap();
    assert!(topology::retain(output.as_ref(), &driver, None).is_none());
    assert_eq!(cleanup(driver, 1000, 1), 0);
    drop(wire);
    drop(forecast);
    let source = mediated.then(|| mediated_topology::checkpoint(&root, 1));
    if let Some(source) = &source { assert_eq!(source.history_anchor(), &original_anchor); }
    let mut loaded = recipe::load(&path, &config, false).unwrap();
    assert!(loaded.native.is_none());
    loaded.recovery_anchor = Some(anchors::load(&anchor).unwrap());
    loaded.anchor_output = Some(root.0.join("recovery-stop-after.anchor"));
    loaded.floor.journal_revision = before.revision;
    loaded.floor.control_sequence = before.control.sequence;
    loaded.floor.authority_epoch = before.control.ledger.epoch;
    let source_output = source.as_ref().map(|saved| {
        let output = root.0.join("recovery-stop-after.source");
        let profile = root.0.join("recovery-stop-topology.json");
        fs::write(&profile, format!(r#"{{"schema_version":"fa.mediated-submit/2",
            "initial":{{"topology_path":"{r}/initial.json","inventory_path":"{r}/initial-inventory.json"}},
            "next":{{"topology_path":"{r}/missing-next.json","inventory_path":"{r}/missing-next-inventory.json"}},
            "operation":{},"source_checkpoint":"{r}/source-1.bin","retain_checkpoint":"{}"}}"#,
            saved.last_operation() + 1, output.display(), r = root.0.display())).unwrap();
        topology::admit(&config, &mut loaded, Some(&profile), topology::Mode::Live).unwrap();
        output
    });
    for name in ["evidence.json", "native-roster.json", "helper-model.json", "helper-weights.safetensors",
        "helper-monitor.json", "helper-tokenizer.bbpe", "helper-sampling.json"] {
        fs::remove_file(root.0.join(name)).unwrap();
    }
    if mediated {
        fs::remove_file(root.0.join("live-1.json")).unwrap();
        fs::remove_file(root.0.join("inventory-1.json")).unwrap();
    }
    assert!(config.programs.is_empty());
    fs::write(config.store.join("delivery.pending"), b"unacknowledged recovery Stop staging").unwrap();
    let reads = config.source.status().read_attempts;
    NativeCase { root, config, loaded, before, numerical, owned, stream, anchor,
        archive, source, source_output, reads }
}

fn native_peers(case: &NativeCase, wrong_pid: bool, candidates: u32) -> PeerProfile {
    let (socket, _client) = UnixStream::pair().unwrap();
    let peer = PeerCredentials::observe(&socket).unwrap();
    let reviewer = if wrong_pid { peer.pid().checked_add(1).unwrap() } else { peer.pid() };
    let scope = case.config.profile.delivery.scope;
    PeerProfile::decode(format!(r#"{{"version":1,"clock":"unix_milliseconds",
        "scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},
        "reviewer_id":{},"socket_directory":"{}/peers",
        "supervisor":{{"uid":{},"gid":{},"pid":{}}},"reviewer":{{"uid":{},"gid":{},"pid":{}}},
        "candidate_limit":{candidates},"runtime_ms":10000,"poll_ms":1}}"#,
        scope.tenant, scope.principal, scope.run, scope.branch, scope.authority,
        case.config.profile.human.reviewer_id, case.root.0.display(),
        peer.uid(), peer.gid(), peer.pid(), peer.uid(), peer.gid(), reviewer).as_bytes()).unwrap()
}

fn native_observed(case: &NativeCase) -> FileDeliverySnapshot {
    if let Some(predictor) = case.loaded.generation.required_owned_pre_output_forecast() {
        let current = FileOversight::read_publication_with_owned_learned_consistency(
            &case.config.store, &case.config.profile, &case.loaded.generation, predictor).unwrap();
        let before = case.owned.as_ref().unwrap();
        assert_eq!(current.consistency.work, before.work);
        assert_eq!(current.consistency.retained_source_bytes, before.retained_source_bytes);
        assert_eq!(current.consistency.consistency.evidence, before.consistency.evidence);
        current.publication
    } else {
        FileOversight::read_publication_with_learned_generation(
            &case.config.store, &case.config.profile, &case.loaded.generation).unwrap()
    }
}

fn assert_inputs_retained(case: &NativeCase) {
    assert_eq!(case.config.source.status().read_attempts, case.reads);
    assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
    assert!(!case.root.0.join("evidence.json").exists());
    assert!(!case.root.0.join("native-roster.json").exists());
    assert_eq!(case.numerical.sampled_draws, 2);
    if let Some(source) = &case.source {
        assert_eq!(&mediated_topology::checkpoint(&case.root, 1), source);
        assert!(!case.root.0.join("live-1.json").exists());
        assert!(!case.root.0.join("missing-next.json").exists());
    }
}

// Derive the oracle through the ORIGINAL offline native consumer, then restore
// only this test fixture's exact preimage. No receipt or event bytes are forged.
fn native_oracle(case: &NativeCase) -> (FileAdmissionStoppedHistory, Vec<u8>) {
    let original = files(&case.config.store);
    let history = match recovery::terminal_selected_with_elapsed(&case.config, &case.loaded,
        recovery::StopPhase::Admission, case.loaded.request, false,
        || panic!("the original admission Stop has no effect clock"), &mut || Duration::ZERO).unwrap()
    {
        recovery::TerminalHistory::Admission(history) => history,
        recovery::TerminalHistory::Completed(_) => panic!("admission cannot fabricate a drain"),
    };
    assert_eq!(history.appended_events(), 2);
    let canonical = fs::read(case.config.store.join("delivery.bin")).unwrap();
    for (name, bytes) in &original { fs::write(case.config.store.join(name), bytes).unwrap(); }
    assert_eq!(files(&case.config.store), original);
    (history, canonical)
}

#[derive(Debug)]
struct ClientResult {
    binding: Option<StopBinding>,
    receipt: Option<StopControlReceipt>,
    outcome_unknown: bool,
    failure: Option<String>,
}
fn stop_client(stream: UnixStream, peers: &PeerProfile, operation: u64,
    lose_reply: bool) -> JoinHandle<ClientResult>
{
    let expected = peers.expected;
    thread::spawn(move || {
        let credentials = PeerCredentials::observe(&stream).unwrap();
        let verified = VerifiedReviewerSocket::verify(stream,
            PeerPolicy::new(credentials.uid(), credentials.gid(), Some(credentials.pid())).unwrap()).unwrap();
        let mut client = verified.into_stop_client(expected, operation).unwrap();
        let started = Instant::now();
        let failure = loop {
            assert!(started.elapsed() < Duration::from_secs(15), "recovery Stop client stalled");
            match client.step() {
                Ok(StopClientProgress::NeedsDecision) => client.request_stop().unwrap(),
                Ok(StopClientProgress::Complete) => break None,
                Ok(_) => {}
                Err(error) => break Some(format!("{error:?}")),
            }
            if lose_reply && client.phase() == StopClientPhase::AwaitingReceipt { break None; }
            thread::sleep(Duration::from_millis(1));
        };
        ClientResult { binding: client.binding(), receipt: client.receipt(),
            outcome_unknown: client.outcome_unknown(), failure }
    })
}

struct Exchange {
    control: Control,
    application: Option<StopRecoveryApplication>,
    client: ClientResult,
    terminal: String,
    clock_calls: usize,
}
fn exchange(case: &NativeCase, connect_at: usize, lose_reply: bool,
    mut clock: impl FnMut(usize, Option<&StopRecoveryApplication>) -> ElapsedTick) -> Exchange
{
    let peers = native_peers(case, false, 2);
    let mut control = Control::new(&case.config, case.loaded.request, Some(&peers)).unwrap();
    let shared = control.share();
    let socket = socket_path(&peers, case.loaded.request);
    let mut client = None;
    let mut clock_calls = 0;
    let result = recovery::open_selected_live_with_topology(&case.config, &case.loaded,
        &mut control, &deadline(), &mut || {
            let cut = clock_calls;
            clock_calls += 1;
            if cut == connect_at {
                let stream = UnixStream::connect(&socket).unwrap();
                client = Some(stop_client(stream, &peers, case.loaded.request, lose_reply));
            }
            clock(cut, shared.recovery_application().as_ref())
        });
    let terminal = result.err().expect("an authenticated explicit Stop must return no live custody");
    let application = control.recovery_application();
    let client = client.expect("selected recovery cut was reached").join().unwrap();
    Exchange { control, application, client, terminal, clock_calls }
}

fn assert_stopped(case: &NativeCase, exchange: &Exchange,
    expected: &FileAdmissionStoppedHistory, canonical: &[u8])
{
    assert!(exchange.terminal.starts_with(&format!(
        "native admission Stop acknowledged at revision {}", expected.snapshot().revision)),
        "{}", exchange.terminal);
    let application = exchange.application.as_ref().expect("original application retained");
    assert_eq!(application.result.as_ref().unwrap(), expected);
    assert!(application.receipt.acknowledged());
    assert_eq!(application.receipt.binding, exchange.client.binding.unwrap());
    assert_ne!(application.receipt.binding.session, [0; 32]);
    assert_eq!(application.receipt.binding.operation, case.loaded.request);
    let receipt = expected.snapshot().stop.as_ref().unwrap();
    assert_eq!(application.receipt.control_sequence, receipt.control_sequence());
    assert_eq!(application.receipt.revocation_floor, receipt.revocation_floor());
    assert_eq!(application.receipt.dispatcher_epoch, receipt.dispatcher_epoch());
    assert_eq!(native_observed(case), *expected.snapshot());
    assert_eq!(fs::read(case.config.store.join("delivery.bin")).unwrap(), canonical);
    assert_eq!(expected.snapshot().revision, case.before.revision + 2);
    assert_eq!(expected.snapshot().control.ledger.elapsed, case.before.control.ledger.elapsed);
    assert_eq!(expected.snapshot().control.ledger.charged, case.before.control.ledger.charged);
    assert_eq!(expected.snapshot().executions, case.before.executions);
    assert_eq!(expected.snapshot().payload, case.before.payload);
    assert!(!case.config.store.join("delivery.pending").exists());
    if case.stream.pending.is_some() {
        assert_eq!(expected.snapshot().control.ledger.stages[&1], ActionState::Unknown);
        assert_eq!(expected.progress().unresolved, vec![1]);
        assert!(expected.progress().irrecoverable.is_empty());
        assert!(!application.receipt.drained());
        assert_eq!(application.receipt.status, StopStatus::StoppedPending);
    }
    assert_inputs_retained(case);
}

fn read_checkpoint(path: &Path) -> TopologyCheckpoint {
    TopologyCheckpoint::read_trusted(&mut Cursor::new(fs::read(path).unwrap()),
        MAX_TOPOLOGY_CHECKPOINT_BYTES).unwrap()
}
fn assert_exported(case: &NativeCase, history: &FileAdmissionStoppedHistory) {
    assert_eq!(&anchors::load(case.loaded.anchor_output.as_ref().unwrap()).unwrap(), history.history_anchor());
    if let Some(source) = &case.source {
        let successor = read_checkpoint(case.source_output.as_ref().unwrap());
        assert_eq!(successor, source.after_admission_stop(&case.config.profile,
            &fs::canonicalize(&case.config.store).unwrap(), history).unwrap());
        assert_eq!(successor.work(), source.work());
        assert_eq!(successor.last_operation(), source.last_operation());
        assert_eq!(successor.expected(), source.expected());
        assert!(!successor.available());
        assert_eq!(successor.history_anchor(), history.history_anchor());
    }
}

#[test]
fn explicit_stop_at_early_middle_and_ready_cuts_matches_all_six_original_native_histories() {
    for mediated in [false, true] {
        for mode in [None, Some(false), Some(true)] {
            for position in 0..3 {
                let case = prepared_native(mode, mediated, true);
                let total = usize::try_from(case.before.revision).unwrap();
                let cut = [0, total / 2, total + 1][position];
                let (expected, canonical) = native_oracle(&case);
                let exchange = exchange(&case, cut, false, |_, _| ElapsedTick(1000));
                assert_stopped(&case, &exchange, &expected, &canonical);
                assert!(exchange.clock_calls >= total + 4);
                assert_eq!(exchange.client.receipt, Some(exchange.application.as_ref().unwrap().receipt));
                assert!(!exchange.client.outcome_unknown);
                assert!(exchange.client.failure.is_none());
                assert_exported(&case, &expected);
            }
        }
    }
}

#[test]
fn no_peer_and_one_rejected_peer_keep_the_original_single_fence_and_separate_roles() {
    for mediated in [false, true] {
        for mode in [None, Some(false), Some(true)] {
            for rejected in [false, true] {
                let case = prepared_native(mode, mediated, false);
                let peers = native_peers(&case, rejected, 2);
                let mut control = Control::new(&case.config, case.loaded.request, Some(&peers)).unwrap();
                let mut client = rejected.then(|| UnixStream::connect(socket_path(&peers, case.loaded.request)).unwrap());
                let mut calls = 0;
                let ((host, _human, forecast, _identity), source) = recovery::open_selected_live_with_topology(
                    &case.config, &case.loaded, &mut control, &deadline(),
                    &mut || { calls += 1; ElapsedTick(1000) }).unwrap();
                assert_eq!(calls, case.before.revision as usize + 2);
                assert_eq!(host.revision(), case.before.revision + 1);
                assert_eq!(host.learned_generation_inspection().unwrap().numerical, case.numerical);
                assert!(host.learned_generation_inspection().unwrap().paused);
                assert!(!host.clock_ready());
                assert!(host.inspect().stop.is_none());
                assert_eq!(host.inspect().executions, case.before.executions);
                assert_eq!(forecast.is_some(), mode.is_some());
                assert_eq!(source.is_some(), mediated);
                assert!(control.recovery_application().is_none());
                assert!(!case.loaded.anchor_output.as_ref().unwrap().exists());
                if let Some(path) = &case.source_output { assert!(!path.exists()); }
                if let Some(client) = client.as_mut() {
                    client.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
                    assert_eq!(client.read(&mut [0; 1]).unwrap(), 0);
                }
                assert_inputs_retained(&case);
            }
        }
    }
}

#[test]
fn lost_reply_and_late_workflow_deadline_keep_native_stop_and_paired_archives() {
    for mediated in [false, true] {
        for mode in [None, Some(false), Some(true)] {
            for lose_reply in [false, true] {
                let case = prepared_native(mode, mediated, true);
                let (expected, canonical) = native_oracle(&case);
                let expired = deadline().logical;
                let mut after_ack = 0;
                let mut exchange = exchange(&case, 0, lose_reply, |_, application| {
                    if application.is_some() {
                        after_ack += 1;
                        if !lose_reply { return expired; }
                    }
                    ElapsedTick(1000)
                });
                assert_eq!(after_ack, 1);
                assert_stopped(&case, &exchange, &expected, &canonical);
                assert_exported(&case, &expected);
                if lose_reply {
                    assert!(exchange.client.receipt.is_none());
                    assert!(exchange.client.outcome_unknown);
                } else {
                    assert!(exchange.terminal.contains("deadline reached after native recovery Stop application"));
                    assert_eq!(exchange.client.receipt, Some(exchange.application.as_ref().unwrap().receipt));
                }
                let stopped = files(&case.config.store);
                let mut retried = 0;
                let error = recovery::open_selected_live_with_topology(&case.config, &case.loaded,
                    &mut exchange.control, &deadline(), &mut || { retried += 1; ElapsedTick(1000) })
                    .err().unwrap();
                assert!(error.starts_with("native admission Stop acknowledged at revision"));
                assert_eq!(retried, 1);
                assert_eq!(files(&case.config.store), stopped);
                assert_eq!(exchange.control.recovery_application(), exchange.application);
            }
        }
    }
}

#[test]
fn independent_output_races_preserve_acknowledgment_and_attempt_both_archives() {
    for mode in [None, Some(false), Some(true)] {
        for collision in [1, 2, 3] {
            let case = prepared_native(mode, true, true);
            let (expected, canonical) = native_oracle(&case);
            let mut injected = false;
            let exchange = exchange(&case, 0, false, |_, application| {
                if application.is_some() {
                    assert!(!injected);
                    injected = true;
                    if collision & 1 != 0 { fs::write(case.loaded.anchor_output.as_ref().unwrap(), b"independent history owner").unwrap(); }
                    if collision & 2 != 0 { fs::write(case.source_output.as_ref().unwrap(), b"independent source owner").unwrap(); }
                }
                ElapsedTick(1000)
            });
            assert!(injected);
            assert_stopped(&case, &exchange, &expected, &canonical);
            assert_eq!(exchange.client.receipt, Some(exchange.application.as_ref().unwrap().receipt));
            if collision & 1 != 0 {
                assert!(exchange.terminal.contains("history anchor retention failed"));
                assert_eq!(fs::read(case.loaded.anchor_output.as_ref().unwrap()).unwrap(), b"independent history owner");
            } else {
                assert_eq!(&anchors::load(case.loaded.anchor_output.as_ref().unwrap()).unwrap(), expected.history_anchor());
            }
            if collision & 2 != 0 {
                assert!(exchange.terminal.contains("topology checkpoint retention failed"));
                assert_eq!(fs::read(case.source_output.as_ref().unwrap()).unwrap(), b"independent source owner");
            } else {
                let successor = read_checkpoint(case.source_output.as_ref().unwrap());
                assert_eq!(successor.history_anchor(), expected.history_anchor());
                assert_eq!(successor.work(), case.source.as_ref().unwrap().work());
            }
        }
    }
}

#[test]
fn continuation_entry_acknowledges_before_duplicate_request_or_missing_live_inputs() {
    for mediated in [false, true] {
        for mode in [None, Some(false), Some(true)] {
            let case = prepared_native(mode, mediated, true);
            let (expected, canonical) = native_oracle(&case);
            let peers = native_peers(&case, false, 2);
            let operation = case.loaded.request;
            let socket = socket_path(&peers, operation);
            let store = case.config.store.clone();
            let profile = case.config.profile.clone();
            let generation = case.loaded.generation.clone();
            let owned = case.owned.clone();
            let anchor_output = case.loaded.anchor_output.clone().unwrap();
            let source_output = case.source_output.clone();
            let expected_source = case.source.as_ref().map(|source| {
                source.after_admission_stop(&profile, &fs::canonicalize(&store).unwrap(),
                    &expected).unwrap()
            });
            let old_anchor = case.anchor.clone();
            let archive = case.archive.clone();
            let old_source = case.source.clone();
            let NativeCase { root, config, loaded, .. } = case;
            let mut calls = 0;
            let mut client = None;
            let result = run(config, loaded, &peers, true, || {
                calls += 1;
                if calls == 2 {
                    // The actual continuation command has established its
                    // deadline and stop socket but has not replayed any event.
                    let stream = UnixStream::connect(&socket).unwrap();
                    client = Some(stop_client(stream, &peers, operation, false));
                }
                ElapsedTick(1000)
            });
            let terminal = result.err().expect("terminal Stop returns no actor response or live custody");
            assert!(terminal.starts_with(&format!("native admission Stop acknowledged at revision {}",
                expected.snapshot().revision)), "{terminal}");
            let client = client.unwrap().join().unwrap();
            let receipt = client.receipt.unwrap();
            assert!(receipt.acknowledged());
            assert!(!receipt.drained());
            assert_eq!(receipt.status, StopStatus::StoppedPending);
            assert_eq!(receipt.binding.operation, operation);
            assert_eq!(receipt.binding, client.binding.unwrap());
            assert_ne!(receipt.binding.session, [0; 32]);
            assert!(!client.outcome_unknown);
            assert!(client.failure.is_none());
            let native_receipt = expected.snapshot().stop.as_ref().unwrap();
            assert_eq!(receipt.control_sequence, native_receipt.control_sequence());
            assert_eq!(receipt.revocation_floor, native_receipt.revocation_floor());
            assert_eq!(receipt.dispatcher_epoch, native_receipt.dispatcher_epoch());
            assert_eq!(fs::read(store.join("delivery.bin")).unwrap(), canonical);
            let current = if let Some(predictor) = generation.required_owned_pre_output_forecast() {
                let current = FileOversight::read_publication_with_owned_learned_consistency(
                    &store, &profile, &generation, predictor).unwrap();
                let before = owned.as_ref().unwrap();
                assert_eq!(current.consistency.work, before.work);
                assert_eq!(current.consistency.retained_source_bytes, before.retained_source_bytes);
                current.publication
            } else {
                FileOversight::read_publication_with_learned_generation(&store, &profile, &generation).unwrap()
            };
            assert_eq!(&current, expected.snapshot());
            assert_eq!(current.control.ledger.stages[&1], ActionState::Unknown);
            assert_eq!(&anchors::load(&anchor_output).unwrap(), expected.history_anchor());
            if let Some(source) = expected_source {
                assert_eq!(read_checkpoint(source_output.as_ref().unwrap()), source);
                assert_eq!(Some(mediated_topology::checkpoint(&root, 1)), old_source);
                assert!(!root.0.join("missing-next.json").exists());
            }
            assert_eq!(fs::read(old_anchor).unwrap(), archive);
            assert!(!store.join("delivery.pending").exists());
            assert!(!socket.exists());
            assert!(!root.0.join("evidence.json").exists());
            assert!(!root.0.join("native-roster.json").exists());
            // Duplicate-request preflight would reject this same operation.
            // Missing helper/source inputs would reject any live execution.
            // The native terminal return above precedes both, with only the
            // original admission Stop/Fence suffix retained on disk.
            assert!(calls > expected.snapshot().revision as usize);
        }
    }
}
