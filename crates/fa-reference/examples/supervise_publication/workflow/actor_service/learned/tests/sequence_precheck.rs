//! Independently selected resident prechecks observe each original release.
//! Real generation, native/process review and fresh human permits remain the
//! effect path after query exhaustion, evidence changes and handoffs.
use super::*;
use super::super::super::precheck;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FilePrecheckOutcome,
    precheck_wire::{PrecheckRequest, PrecheckWireError, PrecheckWireResponse, client::PrecheckPeerClient},
};
use fa_reference::action::consequence::oversight::{
    actor_transport::DriveBudget, evidence_source::EvidenceIdentity,
};
use std::collections::BTreeMap;

fn document(socket: &Path, index: usize, epoch: u64) -> String {
    format!(r#"{{"schema":"fa.actor-precheck-service/1","policy_disclosure":"exact_predicates","socket":{},"expected_policy_epoch":{epoch},"disclosure_id":{},"query_limit":1,"exchange_limit":8,"candidate_limit":8,"connection_limit":8,"runtime_ms":60000}}"#,
        quote(socket.to_str().unwrap()), 81 + index)
}

fn settings(socket: &Path, index: usize, epoch: u64) -> precheck::Settings {
    precheck::Settings::decode(document(socket, index, epoch).as_bytes()).unwrap()
}

fn select(root: &Root, entries: &mut [resident::Entry]) -> [Option<PathBuf>; 3] {
    std::array::from_fn(|index| {
        let socket = root.0.join(format!("peers/precheck-{}.sock", index + 1));
        entries[index].precheck = Some(settings(&socket, index, 0));
        Some(socket)
    })
}

fn query(profile: &Profile, socket: &Path, release: LearnedTextRelease,
    index: usize, query: u64) -> PrecheckWireResponse
{
    let proposal = FileLearnedTextStreamActorPort::encode_release(
        profile.request, release, DUE[index]).unwrap();
    let mut client = PrecheckPeerClient::connect(UnixStream::connect(socket).unwrap(),
        profile.supervisor, PrecheckRequest { query, proposal }, 0).unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(25), "resident precheck response timed out");
        let report = client.drive(DriveBudget::default()).unwrap();
        assert!(report.status.failure.is_none(), "{:?}", report.status);
        if let Some(response) = client.take_response() { return response; }
        assert!(!report.status.closed(), "resident precheck closed without its original response");
        thread::sleep(Duration::from_millis(1));
    }
}

fn passed(response: PrecheckWireResponse, index: usize) {
    assert_eq!(response.query, Some(9));
    let Ok(Knowledge::Known { value, basis }) = response.result else {
        panic!("expected the independently selected original precheck: {response:?}");
    };
    assert_eq!(value, FilePrecheckOutcome::PrecheckPassed);
    assert_eq!((basis.query, basis.disclosure, basis.policy_epoch), (9, 81 + index as u64, 0));
    assert_eq!(basis.observed_at, ElapsedTick(1000));
}

fn poll(peers: &mut Peers, sockets: &[Option<PathBuf>; 3], journal: &Path,
    change_before_submit: Option<(&Path, &[u8])>, decisions: [Option<ReviewDecision>; 3])
{
    for index in 0..3 {
        if peers.actors[index].is_some() { continue; }
        let profile = peers.profiles[index].clone();
        let release = peers.release[index];
        match sockets[index].as_ref() {
            Some(socket) if socket.exists() => {
                for previous in &sockets[..index] {
                    assert!(previous.as_ref().is_none_or(|path| !path.exists()),
                        "a predecessor disclosure survived into a later original request");
                }
                let socket = socket.clone();
                let journal = journal.to_path_buf();
                let review = peers.reviewer.socket(profile.request);
                let changed = if index == 1 {
                    change_before_submit.map(|(path, bytes)| (path.to_path_buf(), bytes.to_vec()))
                } else { None };
                peers.actors[index] = Some(thread::spawn(move || {
                    let original = fs::read(&journal).unwrap();
                    passed(query(&profile, &socket, release, index, 9), index);
                    assert_eq!(fs::read(&journal).unwrap(), original,
                        "precheck cannot mutate the original numerical or publication owner");
                    assert!(!review.exists(), "precheck cannot start human review");
                    // This opens another authenticated transport. It must keep
                    // the same exhausted original grant, never re-disclose it.
                    let exhausted = query(&profile, &socket, release, index, 10);
                    assert_eq!(exhausted.query, Some(10));
                    assert_eq!(exhausted.result, Err(PrecheckWireError::Capacity));
                    assert_eq!(fs::read(&journal).unwrap(), original);
                    if let Some((path, bytes)) = changed { fs::write(path, bytes).unwrap(); }
                    peers::actor(profile, release, DUE[index], peers::Behavior::Observe).join().unwrap()
                }));
            }
            None if profile.socket.exists() => {
                peers.actors[index] = Some(peers::actor(profile, release, DUE[index],
                    peers::Behavior::Observe));
            }
            _ => {},
        }
    }
    for (index, socket) in sockets.iter().enumerate() {
        if peers.reviewer.socket(peers.profiles[index].request).exists() {
            assert!(socket.as_ref().is_none_or(|path| !path.exists()),
                "precheck grants must retire before independent human approval");
        }
    }
    peers.poll([None; 3], decisions);
}

#[test]
fn resident_prechecks_survive_bounded_exhaustion_and_complete_real_native_process_native_releases() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let (config, paths, mut entries) = selected(&root, mode, None);
        entries[1].loaded = process_second(&root, &config, &paths[1], mode);
        let sockets = select(&root, &mut entries);
        let reader = Reader::new(&root, &paths[0]);
        let journal = config.store.join("delivery.bin");
        let mut peers = Peers::new(&root, &config, &entries);
        let reviewer = peers.reviewer.clone();
        let result = resident::run(config, entries, &reviewer, false, None, None, || {
            poll(&mut peers, &sockets, &journal, None, [Some(ReviewDecision::Approve); 3]);
            ElapsedTick(1000)
        }).unwrap();
        assert_eq!(peers.seen_actor, [true; 3]);
        assert_eq!(peers.seen_human, [true; 3]);
        let (replies, packets) = peers.join();
        completed(&reader, &result, replies, packets, mode);
        for name in ["alpha", "beta"] {
            assert!(!fs::read(root.0.join(format!("process-21-{name}.input"))).unwrap().is_empty());
        }
        quiescent(&root);
    }
}

fn denied(config: &Config) -> Vec<u8> {
    EvidenceSnapshot::new(EvidenceIdentity { scope: config.profile.delivery.scope,
        source: 51, generation: 2 }, fa_reference::Snapshot { semantic_epoch: 2, complete: true,
        values: BTreeMap::from([(7, b"blocked".to_vec())]) },
        BTreeMap::from([("alpha".to_owned(), Vec::new()), ("beta".to_owned(), Vec::new())]))
        .unwrap().encode()
}

#[test]
fn later_resident_precheck_pass_cannot_inherit_source_or_human_permission() {
    for source_changes in [true, false] {
        let root = Root::new();
        let (config, paths, mut entries) = selected(&root, None, None);
        let sockets = select(&root, &mut entries);
        let reader = Reader::new(&root, &paths[0]);
        let journal = config.store.join("delivery.bin");
        let source = root.0.join("evidence.json");
        let denied = denied(&config);
        let mut peers = Peers::new(&root, &config, &entries);
        let reviewer = peers.reviewer.clone();
        let decisions = [Some(ReviewDecision::Approve),
            Some(if source_changes { ReviewDecision::Approve } else { ReviewDecision::Reject }),
            Some(ReviewDecision::Approve)];
        let result = resident::run(config, entries, &reviewer, false, None, None, || {
            poll(&mut peers, &sockets, &journal,
                source_changes.then_some((source.as_path(), denied.as_slice())), decisions);
            ElapsedTick(1000)
        }).unwrap();
        assert!(result.failure.as_ref().is_some_and(|error| error.contains("without an Executed receipt")),
            "{:?}", result.failure);
        assert_eq!(result.cleanup_pending, 0);
        assert_eq!(result.responses.len(), 2);
        assert!(confirmed(&result.responses[0]));
        if source_changes {
            assert!(matches!(result.responses[1].result,
                Ok(Knowledge::Known { value: ActorOutcome::Denied, .. })));
        } else {
            assert!(matches!(result.responses[1].result,
                Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
        }
        assert_eq!(peers.seen_actor, [true, true, false]);
        assert_eq!(peers.seen_human, [true, !source_changes, false]);
        let (replies, packets) = peers.join();
        assert!(confirmed(replies[0].as_ref().unwrap()));
        assert_eq!(replies[1].as_ref().unwrap().result, result.responses[1].result);
        assert!(replies[2].is_none() && packets[2].is_none());
        let first = packets[0].as_ref().unwrap();
        if let Some(second) = packets[1].as_ref() {
            assert_ne!(first.binding().session, second.binding().session);
        }
        let state = reader.snapshot();
        assert_eq!((state.executions, state.payload.as_slice()), (1, b"A".as_slice()));
        assert_eq!(state.control.ledger.charged, first.action().spec().units);
        assert_eq!(state.control.ledger.reserved, 0);
        assert!(state.stop.is_none());
        quiescent(&root);
    }
}

fn command_inputs(root: &Root, config: &Config, paths: &[PathBuf],
    entries: &[resident::Entry]) -> Vec<String>
{
    let mut plan = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let inputs = super::super::mediated_topology::command_inputs(root, config,
            &paths[index], &entry.actor);
        let actor = root.0.join(format!("sequence-actor-{}.json", entry.loaded.request));
        fs::rename(&inputs[5], &actor).unwrap();
        let precheck = if let Some(settings) = entry.precheck.as_ref() {
            let path = root.0.join(format!("sequence-precheck-{}.json", entry.loaded.request));
            fs::write(&path, document(settings.socket_path(), index, 0)).unwrap();
            quote(path.to_str().unwrap())
        } else { "null".into() };
        plan.push(format!(r#"{{"recipe":{},"actor_profile":{},"release":"{}","precheck_profile":{precheck}}}"#,
            quote(paths[index].to_str().unwrap()), quote(actor.to_str().unwrap()),
            if matches!(entry.release, LearnedTextRelease::Finish) { "finish" } else { "message" }));
    }
    let path = root.0.join("prechecked-sequence.json");
    fs::write(&path, format!(r#"{{"schema":"fa.learned-publication-sequence/2","entries":[{}]}}"#,
        plan.join(","))).unwrap();
    vec!["serve-learned-generated".into(), root.0.join("command-host.json").display().to_string(),
        path.display().to_string(), root.0.join("command-reviewer.json").display().to_string()]
}

#[test]
fn public_resident_precheck_manifest_keeps_null_selection_and_finishes_the_original_stream() {
    let root = Root::new();
    let (config, paths, mut entries) = selected(&root, None, None);
    let mut sockets = select(&root, &mut entries);
    entries[1].precheck = None;
    sockets[1] = None;
    let args = command_inputs(&root, &config, &paths, &entries);
    let reader = Reader::new(&root, &paths[0]);
    let journal = config.store.join("delivery.bin");
    let mut peers = Peers::new(&root, &config, &entries);
    resident::command_with_clock(&args, None, || {
        assert!(!root.0.join("peers/precheck-2.sock").exists(),
            "an explicit null entry cannot inherit its predecessor's disclosure");
        poll(&mut peers, &sockets, &journal, None, [Some(ReviewDecision::Approve); 3]);
        ElapsedTick(1000)
    }).unwrap();
    assert_eq!(peers.seen_actor, [true; 3]);
    assert_eq!(peers.seen_human, [true; 3]);
    let (replies, packets) = peers.join();
    assert!(replies.iter().all(|reply| confirmed(reply.as_ref().unwrap())));
    let sessions = packets.iter().map(|packet| packet.as_ref().unwrap().binding().session)
        .collect::<BTreeSet<_>>();
    assert_eq!(sessions.len(), 3);
    let state = reader.snapshot();
    assert_eq!((state.executions, state.payload.as_slice()), (3, b"AA".as_slice()));
    assert_eq!(state.control.ledger.charged, packets.iter().map(|packet|
        packet.as_ref().unwrap().action().spec().units).sum::<u64>());
    assert_eq!(state.control.ledger.reserved, 0);
    assert_eq!(state.control.ledger.epoch, 0);
    assert!(state.stop.is_none());
    let (host, _, _, _) = recovery::open_selected(&reader.config, &reader.loaded).unwrap();
    assert!(host.stream_snapshot().unwrap().confirmed.finished());
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 4);
    quiescent(&root);
}

#[test]
fn invalid_later_resident_precheck_profile_refuses_before_clock_owner_or_any_listener() {
    let root = Root::new();
    let (config, paths, mut entries) = selected(&root, None, None);
    select(&root, &mut entries);
    let args = command_inputs(&root, &config, &paths, &entries);
    let path = root.0.join("sequence-precheck-3.json");
    let valid = fs::read_to_string(&path).unwrap();
    for invalid in [
        valid.replace("service/1", "service/2").replace(r#""runtime_ms":60000"#,
            r#""runtime_ms":60000,"cache_entries":8,"cache_bytes":65536"#),
        valid.replace(r#""query_limit":1"#, r#""query_limit":0"#),
        valid.replace(r#""policy_disclosure":"exact_predicates""#, r#""policy_disclosure":"full""#),
        valid.replace(r#""runtime_ms":60000"#, r#""runtime_ms":60000,"reuse_previous_disclosure":true"#),
    ] {
        fs::write(&path, invalid).unwrap();
        assert!(resident::command_with_clock(&args, None,
            || panic!("a later invalid selection must refuse before time")).is_err());
        assert!(!config.store.exists());
        quiescent(&root);
    }
}

#[test]
fn resident_prechecks_admit_all_future_canonical_names_and_selected_audiences_before_startup() {
    for conflict in 0..10 {
        let root = Root::new();
        let (mut config, _, mut entries) = selected(&root, None, None);
        let sockets = select(&root, &mut entries);
        let mut reviewer = fixture::peers(&root, &config);
        let alias = root.0.join("alias");
        std::os::unix::fs::symlink(&root.0, &alias).unwrap();
        match conflict {
            // A first disclosure must reserve a not-yet-bound later actor;
            // canonical ancestors must not hide that collision.
            0 => entries[0].precheck = Some(settings(&alias.join("peers/actor-3.sock"), 0, 0)),
            1 => entries[2].precheck = Some(settings(&alias.join("peers/precheck-1.sock"), 2, 0)),
            2 => entries[2].precheck = Some(settings(&reviewer.socket(1), 2, 0)),
            3 => entries[0].precheck = Some(settings(&socket_path(&reviewer, 3), 0, 0)),
            4 => entries[0].loaded.anchor_output = Some(alias.join("peers/precheck-3.sock")),
            5 => {
                let (_, _, selected) = super::super::mediated_topology::selected(&root, None, false);
                let path = root.0.join("create-topology.json");
                let original = fs::read_to_string(&path).unwrap();
                fs::write(&path, original.replace(root.0.join("source-1.bin").to_str().unwrap(),
                    alias.join("peers/precheck-3.sock").to_str().unwrap())).unwrap();
                entries[0].loaded.anchor_output = selected.anchor_output;
                super::super::super::topology::admit(&config, &mut entries[0].loaded,
                    Some(&path), super::super::super::topology::Mode::Create).unwrap();
            }
            6 => entries[2].actor.scope.branch += 1,
            7 => reviewer.expected.reviewer += 1,
            8 => config.store = root.0.join("peers"),
            _ => {
                let original = entries[2].actor.supervisor;
                entries[2].actor.supervisor =
                    fa_reference::action::consequence::oversight::actor_peer::PeerPolicy::new(
                        original.uid() + 1, original.gid(), original.pid()).unwrap();
            }
        }
        let store = config.store.clone();
        let result = resident::run(config, entries, &reviewer, false, None, None,
            || panic!("future disclosure conflict {conflict} reached time"));
        assert!(result.is_err(), "future disclosure conflict {conflict} was admitted");
        assert!(!store.join("delivery.bin").exists());
        if conflict != 8 { assert!(!store.exists()); }
        assert!(sockets.iter().flatten().all(|socket| !socket.exists()));
        quiescent(&root);
    }
}

#[test]
fn a_preexisting_later_precheck_path_is_not_replaced_and_cannot_start_earlier_releases() {
    let root = Root::new();
    let (config, _, mut entries) = selected(&root, None, None);
    let sockets = select(&root, &mut entries);
    let path = sockets[2].as_ref().unwrap();
    fs::write(path, b"unrelated operator file").unwrap();
    let reviewer = fixture::peers(&root, &config);
    let store = config.store.clone();
    let result = resident::run(config, entries, &reviewer, false, None, None,
        || panic!("existing later precheck path reached time"));
    assert!(result.is_err());
    assert_eq!(fs::read(path).unwrap(), b"unrelated operator file");
    assert!(!store.exists());
    assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 1);
}

#[test]
fn a_resident_operator_cannot_claim_a_later_selected_precheck_socket() {
    use super::super::super::operator as live_operator;
    let root = Root::new();
    let (config, _, mut entries) = selected(&root, None, None);
    let sockets = select(&root, &mut entries);
    let reviewer = fixture::peers(&root, &config);
    let scope = entries[0].actor.scope;
    let identity = entries[0].actor.supervisor;
    let pid = identity.pid().unwrap();
    let path = root.0.join("operator.json");
    fs::write(&path, format!(
        r#"{{"schema":"fa.tower-operator-service/1","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},"socket":{},"supervisor":{{"uid":{},"gid":{},"pid":{pid}}},"operator":{{"uid":{},"gid":{},"pid":{pid}}},"candidate_limit":4,"connection_limit":4,"exchange_limit":16,"runtime_ms":60000,"poll_ms":1,"inspection_queries":3,"explanation_queries":2,"explanation_replay_events":16,"explanation_disclosure":"policy_and_selected_witnesses"}}"#,
        scope.tenant, scope.principal, scope.run, scope.branch, scope.authority,
        quote(sockets[2].as_ref().unwrap().to_str().unwrap()),
        identity.uid(), identity.gid(), identity.uid(), identity.gid())).unwrap();
    let selected = live_operator::Selection::read(&path, 41).unwrap();
    let store = config.store.clone();
    let result = resident::run_with_operator(config, entries, &reviewer, false, None, None,
        Some(&selected), || panic!("operator/precheck collision reached time"));
    assert!(result.is_err());
    assert!(!store.exists());
    quiescent(&root);
}

#[test]
fn resident_precheck_epoch_is_checked_before_later_numerical_work() {
    let root = Root::new();
    let (config, paths, mut entries) = selected(&root, None, None);
    let sockets = select(&root, &mut entries);
    entries[1].precheck = Some(settings(sockets[1].as_ref().unwrap(), 1, 1));
    let reader = Reader::new(&root, &paths[0]);
    let journal = config.store.join("delivery.bin");
    let mut peers = Peers::new(&root, &config, &entries);
    let reviewer = peers.reviewer.clone();
    let result = resident::run(config, entries, &reviewer, false, None, None, || {
        assert!(!sockets[1].as_ref().unwrap().exists() && !sockets[2].as_ref().unwrap().exists(),
            "wrong epoch must refuse before disclosure bootstrap");
        poll(&mut peers, &sockets, &journal, None, [Some(ReviewDecision::Approve); 3]);
        ElapsedTick(1000)
    }).unwrap();
    assert!(result.failure.as_ref().is_some_and(|error| error.contains("policy epoch")),
        "{:?}", result.failure);
    assert_eq!(result.cleanup_pending, 0);
    assert!(confirmed(&result.responses[0]));
    assert!(!peers.seen_human[1] && !peers.seen_actor[2]);
    let (replies, packets) = peers.join();
    assert!(confirmed(replies[0].as_ref().unwrap()));
    assert!(replies[1].is_none() && replies[2].is_none());
    let state = reader.snapshot();
    assert_eq!(state.executions, 1);
    assert_eq!(state.control.ledger.charged, packets[0].as_ref().unwrap().action().spec().units);
    assert_eq!(state.control.ledger.reserved, 0);
    let (host, _, _, _) = recovery::open_selected(&reader.config, &reader.loaded).unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(numerical.sampled_draws, 2, "invalid next epoch paid for another generation");
    assert_eq!(numerical.position, 4);
    quiescent(&root);
}
