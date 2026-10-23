//! Original mediated recovery, witness validation, helper processes and human
//! sockets. Synthetic ballots/captures test enforcement, not evaluator quality.
use super::*;
use fa_reference::{ReadWitness, Snapshot, product_frontier::{FrontierStage,
    ProductFrontiers, ProjectionKey, TrustedClosingMarker}, witness::{
    AdapterDomainInput, DomainClosure, DomainProjection, SnapshotEntry}};
use fa_reference::action::{ActionSpec, FrozenAction, VERSION};
use fa_reference::action::consequence::delivery::persistent::observed::{
    guarded::{anchored::MAX_HISTORY_ANCHOR_BYTES, mediated::{FileMediatedBootstrap, FileMediatedRecovery}},
    driver::topology::FileDriverTopology,
    mediation::source::TOPOLOGY_REFRESH_READ_BYTES,
    publication::{capture::{FileCaptureIdentity, FilePublicationCapture},
        witnesses::{FilePublicationInputs, FileWitnessInput}},
    reviewer::{client::{ReviewerClient, ReviewerExpectation, ReviewClientProgress}, wire::ReviewDecision}};
use fa_reference::action::consequence::mediation::MAX_CHECK_EDGE_VISITS;
use fa_reference::action::consequence::oversight::{
    actor::{ActorProposal, ActorOutcome, Knowledge}, actor_wire::encode_command,
    evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource},
    helper_processes::HelperProgram};
use fa_reference::perimeter_inventory::topology::LoadedAuthorityTopology;
use std::{collections::BTreeMap, ffi::OsString, fs, io, os::unix::net::UnixStream,
    path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

const TEMPLATE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/supervised_publication.json"));
const INVENTORY: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/authority_topology_inventory.json"));
// Reuse the existing actual helper executable entry point and its declared
// environment protocol, not another ballot implementation or fixture endpoint.
const CHILD: &str = "workflow::continuation::guarded::mediated::tests::synthetic_helper_process";
const MEMBER: &str = "FA_MEDIATED_SUBMIT_MEMBER";
const EPOCH: &str = "FA_MEDIATED_SUBMIT_EPOCH";
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-checked-mediated-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&root).unwrap(); Self(root)
    }
    fn path(&self, name: &str) -> PathBuf { self.0.join(name) }
    fn bytes(&self) -> Vec<u8> { fs::read(self.path("store/delivery.bin")).unwrap() }
    fn install(&self, generation: u64, live: bool) {
        fs::write(self.path(if live { "live.json" } else if generation == 1 { "saved1.json" } else { "saved2.json" }),
            graph(generation)).unwrap();
        fs::write(self.path(if live { "live-inventory.json" } else { "saved-inventory.json" }), INVENTORY).unwrap();
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("checked mediated cleanup: {error}"); }
    }
}
fn graph(generation: u64) -> String {
    format!(r#"{{"schema":"fa.authority-topology/1","generation":{generation},"inventory_generation":{generation},
        "complete_inventory_generation":{generation},
        "scope":{{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5,"purpose":"effect"}},
        "target":{{"adapter":10,"object":11,"contract_version":1,"expected_version":1,"generation":1}},
        "inventory_scope":{{"tenant":1,"principal":2,"purpose":1}},"family":"file-publication",
        "nodes":[{{"id":1,"kind":"actor"}},{{"id":2,"kind":"enforcer"}},{{"id":4,"kind":"sink"}}],
        "edges":[{{"id":1,"from":1,"to":2,"channel":"dispatch","route":"publication","provenance":101}},
        {{"id":2,"from":2,"to":4,"channel":"dispatch","route":"publication","provenance":102}}],
        "enforcers":[{{"node":2,"adapter":10,"contract_version":1,"generation":1,"provenance":102}}],
        "gates":[2],"check_edge_budget":{MAX_CHECK_EDGE_VISITS},"source_limits":{{"checks":8,"read_bytes":67108864}}}}"#)
}
fn loaded(generation: u64) -> LoadedAuthorityTopology {
    LoadedAuthorityTopology::from_json_bytes(graph(generation).as_bytes(), INVENTORY.as_bytes()).unwrap()
}
fn configured(root: &Directory, epoch: u64) -> Config {
    let mut config = Config::decode(TEMPLATE.as_bytes()).unwrap(); config.store = root.path("store");
    config.source = FileEvidenceSource::new(root.path("evidence.json"), 51, config.profile.delivery.scope, 1048576).unwrap();
    config.timing.poll_ms = 1; config.timing.runtime_ms = 15000; config.timing.cleanup_ms = 2000;
    config.programs = ["alpha", "beta"].into_iter().map(|member| {
        let program = HelperProgram::new(std::env::current_exe().unwrap(), root.0.clone(),
            vec!["--exact".into(), CHILD.into(), "--nocapture".into()],
            BTreeMap::from([(OsString::from(MEMBER), member.into()), (OsString::from(EPOCH), epoch.to_string().into())])).unwrap();
        (member.into(), program)
    }).collect(); config
}
fn reviewer(config: &Config, request: u64, choice: ReviewDecision) -> std::thread::JoinHandle<()> {
    let path = config.socket(request);
    let expected = ReviewerExpectation { reviewer: config.profile.human.reviewer_id,
        scope: config.profile.delivery.scope, clock_domain: crate::config::CLOCK_DOMAIN };
    std::thread::spawn(move || {
        let started = Instant::now();
        let socket = loop {
            match UnixStream::connect(&path) {
                Ok(socket) => break socket,
                Err(error) if matches!(error.kind(), io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused) => {
                    assert!(started.elapsed() < Duration::from_secs(10)); std::thread::sleep(Duration::from_millis(1));
                }
                Err(error) => panic!("checked reviewer connection: {error}"),
            }
        };
        let mut client = ReviewerClient::from_unix(socket, expected).unwrap();
        loop {
            assert!(started.elapsed() < Duration::from_secs(12));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => client.respond(choice).unwrap(),
                ReviewClientProgress::Complete => return,
                _ => std::thread::sleep(Duration::from_millis(1)),
            }
        }
    })
}
fn document(config: &Config, request: u64, epoch: u64, version: u64, deadline: ElapsedTick) -> Vec<u8> {
    let mut target = config.profile.delivery.target; target.expected_version = version;
    let payload = if request == 1 { b"first".to_vec() } else { b"second".to_vec() };
    encode_command(&Command::Submit { request, proposal: ActorProposal { target,
        units: payload.len() as u64, payload, deadline, expected_policy_epoch: epoch } }).unwrap()
}
fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }
}
fn archive(host: &FileOversight, path: &Path) -> Result<(), String> {
    let mut bytes = Vec::new();
    host.history_anchor().map_err(debug)?.write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES).map_err(debug)?;
    fs::write(path, bytes).map_err(debug)
}
fn witness_text(root: &Directory) -> String {
    format!(r#"{{"schema":"fa.supervised-witnesses/1","source":91,"original":"{}","current":"{}",
        "limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},
        "requests":[{{"kind":"exact_value","key":0,"role":"subject"}},{{"kind":"absent_key","key":1}},
        {{"kind":"empty_range","start":6,"end":9}},{{"kind":"range_members","start":2,"end":6}}]}}"#,
        root.path("original.bin").display(), root.path("current.bin").display())
}
fn witness(root: &Directory) -> PublicationProfile {
    PublicationProfile::decode(witness_text(root).as_bytes()).unwrap()
}
fn capture(action: &FrozenAction, generation: u64, keys: &[u64]) -> FilePublicationCapture {
    let key = ProjectionKey { source: 40, branch: action.spec().scope.branch, projection: 7, source_epoch: 1 };
    let close = TrustedClosingMarker { key, final_sequence: 1, marker_generation: 1 };
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap(); frontiers.record_close(close).unwrap();
    let input = FileWitnessInput::new(generation, generation, 20,
        AdapterDomainInput::new(DomainProjection::new(40, 1, key), DomainClosure::Closed(close)),
        keys.iter().map(|key| SnapshotEntry::new(*key, 1, b"original".to_vec()).unwrap()).collect(), &frontiers).unwrap();
    FilePublicationCapture::new(1, FileCaptureIdentity { source: 91, generation }, action,
        FilePublicationInputs::new(Some(input), None)).unwrap()
}
fn write_capture(root: &Directory, name: &str, value: &FilePublicationCapture) {
    let pending = root.path("capture.pending"); fs::write(&pending, value.to_bytes().unwrap()).unwrap();
    fs::rename(pending, root.path(name)).unwrap();
}
fn captures(root: &Directory, config: &Config, bytes: &[u8]) -> FrozenAction {
    let Command::Submit { request, proposal } = decode_command(bytes).unwrap() else { unreachable!() };
    // The existing policy has exactly one observation dependency, key 7. Native
    // preparation compares this capture to the ACTUAL admitted frozen action;
    // mismatched scope/epoch/target/deadline/witnesses cannot authorize it.
    let action = FrozenAction::freeze(ActionSpec { version: VERSION, scope: config.profile.delivery.scope,
        target: Some(proposal.target), payload: proposal.payload, units: proposal.units,
        deadline: proposal.deadline, policy_epoch: proposal.expected_policy_epoch,
        required_witnesses: vec![ReadWitness::Exact { key: 7, value: Some(b"ok".to_vec()) }] }).unwrap();
    let packet = capture(&action, 2 * request - 1, &[0, 2, 4]);
    write_capture(root, "original.bin", &packet); write_capture(root, "current.bin", &packet); action
}
fn seed(root: &Directory, checked: bool) -> Vec<u8> {
    root.install(1, false); root.install(1, true); root.install(2, false);
    let config = configured(root, 0);
    let evidence = EvidenceSnapshot::new(EvidenceIdentity { source: 51, generation: 1,
        scope: config.profile.delivery.scope }, snapshot(), ["alpha", "beta"].into_iter().map(|member|
        (member.into(), format!("review this fixture: {member}").into_bytes())).collect()).unwrap();
    fs::write(root.path("evidence.json"), evidence.encode()).unwrap();
    let publication = checked.then(|| witness(root));
    let guards = FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: Some(config.source_policy),
        identity: None, campaigns: None, credential: None };
    let (host, roles) = FileOversight::create_mediated_guarded_reserved(&config.store, config.profile.clone(), &guards, None,
        FileMediatedBootstrap { topology: loaded(1).graph().clone(), reserve: RecoveryReserve::terminal(),
            publication: publication.as_ref().map(|p| p.limits) }, None, None).unwrap();
    let source = roles.topology_observer.bind_topology_source(&host, host.revision(),
        root.path("live.json"), root.path("live-inventory.json"), loaded(1)).unwrap();
    let bytes = document(&config, 1, 0, 1, ElapsedTick(100000));
    if checked { captures(root, &config, &bytes); }
    let Command::Submit { request, proposal } = decode_command(&bytes).unwrap() else { unreachable!() };
    let human = reviewer(&config, 1, ReviewDecision::Approve);
    let result = from_recovered_with_topology_retaining(config, host, roles.oversight.human, Continuation {
        document: &bytes, request, proposal, peers: None, publication: publication.as_ref(), credibility: None,
        runtime_deadline: ElapsedTick(16000), started: Instant::now(),
    }, Some(FileDriverTopology::new(source)), || ElapsedTick(1000), |host, topology| {
        let work = topology.unwrap().source().work();
        assert_eq!(work.checks, 2); assert_eq!(work.reserved_read_bytes, 2 * TOPOLOGY_REFRESH_READ_BYTES);
        archive(host, &root.path("first.anchor"))
    }).unwrap();
    human.join().unwrap(); assert!(result.failure.is_none(), "{:?}", result.failure);
    assert_eq!(result.cleanup_pending, 0); assert!(executed(&result));
    root.install(2, true); bytes
}
fn executed(result: &RunResult) -> bool {
    matches!(&result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. }))
}
fn recovery_text(root: &Directory, output: Option<&str>) -> String {
    let config = configured(root, 0); let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    let output = output.map(|name| format!(r#", "retain_history_anchor":"{}""#, root.path(name).display())).unwrap_or_default();
    format!(r#"{{"schema_version":"fa.guarded-submit/1","minimum":{{"journal_revision":{},"control_sequence":{},"authority_epoch":{}}},
        "history_anchor":"{}"{output}}}"#, state.revision, state.control.sequence, state.control.ledger.epoch, root.path("first.anchor").display())
}
fn selection_text(root: &Directory, current: u64, available: bool, checks: u64, last: u64) -> String {
    let operation = last.max(99) + 1;
    format!(r#"{{"schema_version":"fa.mediated-submit/1",
        "initial":{{"topology_path":"{}","inventory_path":"{}"}},
        "current":{{"topology_path":"{}","inventory_path":"{}"}},"available":{available},
        "next":{{"topology_path":"{}","inventory_path":"{}"}},"operation":{operation},
        "source":{{"checks":{checks},"reserved_read_bytes":{},"last_operation":{last}}}}}"#,
        root.path("saved1.json").display(), root.path("saved-inventory.json").display(),
        root.path(&format!("saved{current}.json")).display(), root.path("saved-inventory.json").display(),
        root.path("live.json").display(), root.path("live-inventory.json").display(), checks * TOPOLOGY_REFRESH_READ_BYTES)
}
fn selection(root: &Directory) -> Selection {
    Selection::decode(selection_text(root, 1, true, 2, 2).as_bytes()).unwrap()
}
fn begin(root: &Directory) -> FileMediatedRecovery {
    let config = configured(root, 0);
    let profile = SubmitProfile::decode(recovery_text(root, None).as_bytes()).unwrap();
    let custody = profile.admit(&config).unwrap();
    let history = selection(root).history(&config, &mut || Ok(())).unwrap();
    let expected = FileMediatedRequirements { oversight: FileRecoveryRequirements {
        guards: FileGuardSet { stream: None, decoder: None, decoder_stop: None,
            source: Some(config.source_policy), identity: None, campaigns: None, credential: None },
        effective_policy: config.profile.delivery.policy.clone(), credential_epoch: None,
        minimum: history.minimum(custody.anchor(), profile.minimum()).unwrap(),
    }, topology: history.requirement, prediction: None, evaluation: None };
    FileOversight::begin_open_mediated_guarded_anchored(&config.store, config.profile.clone(),
        &expected, custody.anchor()).unwrap().require_recovery_reserve(RecoveryReserve::terminal()).unwrap()
}
fn selected(publication: Option<&PublicationProfile>) -> FilePublicationRecoveryRequirements {
    FilePublicationRecoveryRequirements { validation: publication.map(|p| p.limits), feed: None, joint: None }
}

#[test]
fn exact_publication_selection_precedes_replay_and_cannot_be_changed_after_work() {
    for checked in [false, true] {
        let root = Directory::new(); seed(&root, checked); let publication = witness(&root);
        let correct = selected(checked.then_some(&publication));
        fs::write(root.path("store/delivery.pending"), b"preserve staging").unwrap(); let before = root.bytes();
        assert_eq!(begin(&root).require_publication_profile(selected((!checked).then_some(&publication))).err(),
            Some(JournalError::Contract(Error::Binding)));
        let mut recovery = begin(&root).require_publication_profile(correct).unwrap();
        let untouched = recovery.progress(); assert_eq!(untouched.attempted_events, 0);
        assert_eq!(untouched.replayed_events, 0);
        recovery.advance(0, 1).unwrap();
        assert_eq!(recovery.require_publication_profile(correct).err(), Some(JournalError::Contract(Error::WrongState)));
        assert_eq!(root.bytes(), before);
        assert_eq!(fs::read(root.path("store/delivery.pending")).unwrap(), b"preserve staging");
        // Positive on the same untouched history after the rejected late call.
        let mut recovery = begin(&root).require_publication_profile(correct).unwrap();
        while recovery.progress().status == FileGuardedRecoveryStatus::Replaying {
            recovery.advance(recovery.progress().replayed_events, 1).unwrap();
        }
        assert_eq!(recovery.progress().status, FileGuardedRecoveryStatus::Ready);
        recovery.cancel(); assert_eq!(root.bytes(), before);
    }
}

#[test]
fn resumed_checked_publication_keeps_both_guards_until_after_human_dispatch() {
    for mutation in ["unrelated", "phantom", "missing-witness", "missing-topology", "both-missing"] {
        let root = Directory::new(); seed(&root, true); let config = configured(&root, 3);
        let bytes = document(&config, 2, 3, 2, ElapsedTick(100000)); let action = captures(&root, &config, &bytes);
        let publication = witness(&root); let recovery = SubmitProfile::decode(recovery_text(&root, Some("second.anchor")).as_bytes()).unwrap();
        let human = reviewer(&config, 2, ReviewDecision::Approve);
        let store = config.store.clone(); let bootstrap = config.profile.clone(); let mut changed = false;
        let result = submit_selected(config, Submission { document: &bytes, profile: &recovery, peers: None, credibility: None },
            &selection(&root), Some(&publication), || {
                let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
                if !changed && state.executions == 1 && state.control.ledger.charged > 5 {
                    match mutation {
                        "unrelated" => write_capture(&root, "current.bin", &capture(&action, 4, &[0, 2, 4, 99])),
                        "phantom" => write_capture(&root, "current.bin", &capture(&action, 4, &[0, 1, 2, 4])),
                        "missing-witness" => fs::remove_file(root.path("current.bin")).unwrap(),
                        "missing-topology" => fs::remove_file(root.path("live-inventory.json")).unwrap(),
                        "both-missing" => {
                            fs::remove_file(root.path("current.bin")).unwrap(); fs::remove_file(root.path("live-inventory.json")).unwrap();
                        }
                        _ => unreachable!(),
                    }
                    changed = true;
                }
                ElapsedTick(1001)
            }).unwrap();
        human.join().unwrap(); assert!(changed, "{mutation}");
        assert_eq!(executed(&result), mutation == "unrelated", "{mutation}: {:?}", result.failure);
        if mutation == "unrelated" { assert!(result.failure.is_none(), "{:?}", result.failure); }
        assert_eq!(result.cleanup_pending, 0);
        let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(state.executions, if mutation == "unrelated" { 2 } else { 1 });
        assert_eq!(state.payload, if mutation == "unrelated" { b"second".to_vec() } else { b"first".to_vec() });
        assert_eq!(state.control.ledger.charged, if mutation == "unrelated" { 11 } else { 5 });
        assert_eq!(state.control.ledger.reserved, 0);
        assert_eq!(state.control.ledger.available + state.control.ledger.reserved + state.control.ledger.charged, bootstrap.delivery.total);
        assert!(root.path("second.anchor").exists());
    }
}

#[test]
fn checked_receipt_retry_needs_no_live_witness_topology_helper_or_reviewer() {
    let root = Directory::new(); seed(&root, true); let config = configured(&root, 3);
    let bytes = document(&config, 2, 3, 2, ElapsedTick(100000)); captures(&root, &config, &bytes);
    let publication = witness(&root); let recovery = SubmitProfile::decode(recovery_text(&root, Some("second.anchor")).as_bytes()).unwrap();
    let human = reviewer(&config, 2, ReviewDecision::Approve);
    let result = submit_selected(config, Submission { document: &bytes, profile: &recovery, peers: None, credibility: None },
        &selection(&root), Some(&publication), || ElapsedTick(1001)).unwrap();
    human.join().unwrap(); assert!(executed(&result)); assert!(result.failure.is_none());
    for name in ["original.bin", "current.bin", "evidence.json", "live.json", "live-inventory.json"] {
        fs::remove_file(root.path(name)).unwrap();
    }
    let mut config = configured(&root, 3); config.programs.clear();
    let recovery = SubmitProfile::decode(recovery_text(&root, None).as_bytes()).unwrap();
    let receipt = Selection::decode(selection_text(&root, 2, true, 4, 102).as_bytes()).unwrap();
    let capsule = root.path("absent-credibility");
    let retried = submit_selected(config, Submission { document: &bytes, profile: &recovery, peers: None,
        credibility: Some(&capsule) }, &receipt, Some(&publication), || ElapsedTick(1000000)).unwrap();
    assert!(retried.failure.is_none()); assert_eq!(retried.response.result, result.response.result);
    assert_eq!(retried.cleanup_pending, 0);
    let mut changed = decode_command(&bytes).unwrap();
    let Command::Submit { proposal, .. } = &mut changed else { unreachable!() };
    proposal.payload = b"other".to_vec();
    let changed = encode_command(&changed).unwrap();
    let mut config = configured(&root, 3); config.programs.clear();
    let recovery = SubmitProfile::decode(recovery_text(&root, None).as_bytes()).unwrap();
    let receipt = Selection::decode(selection_text(&root, 2, false, 4, 102).as_bytes()).unwrap();
    let conflict = submit_selected(config, Submission { document: &changed, profile: &recovery, peers: None,
        credibility: Some(&capsule) }, &receipt, Some(&publication), || ElapsedTick(1000001)).unwrap();
    assert!(conflict.response.result.is_err()); assert_eq!(conflict.cleanup_pending, 0);
    let config = configured(&root, 3); let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(state.executions, 2); assert_eq!(state.control.ledger.charged, 11);
    assert!(!config.socket(2).exists());
}

#[test]
fn absent_or_changed_checked_mode_refuses_before_fencing_or_staging_cleanup() {
    for checked in [false, true] {
        let root = Directory::new(); let bytes = seed(&root, checked);
        let mut publication = witness(&root); let profile = SubmitProfile::decode(recovery_text(&root, None).as_bytes()).unwrap();
        fs::write(root.path("store/delivery.pending"), b"keep").unwrap(); let before = root.bytes();
        for mismatched in 0..(if checked { 2 } else { 1 }) {
            let candidate = if checked && mismatched == 0 { None } else { Some(&publication) };
            let mut config = configured(&root, 3); config.programs.clear(); let mut clock_calls = 0;
            let result = submit_selected(config, Submission { document: &bytes, profile: &profile, peers: None, credibility: None },
                &selection(&root), candidate, || { clock_calls += 1; ElapsedTick(1001) });
            assert!(result.is_err()); assert_eq!(clock_calls, 1, "only the invocation clock precedes bootstrap admission");
            assert_eq!(root.bytes(), before); assert_eq!(fs::read(root.path("store/delivery.pending")).unwrap(), b"keep");
            publication.limits.validation.steps += 1;
        }
        let config = configured(&root, 0); let publication = witness(&root);
        let receipt = submit_selected(config, Submission { document: &bytes, profile: &profile, peers: None, credibility: None },
            &selection(&root), checked.then_some(&publication), || ElapsedTick(1001)).unwrap();
        assert!(executed(&receipt)); assert!(receipt.failure.is_none());
    }
}

#[test]
fn richer_profiles_are_not_downgraded_and_invalid_flags_never_open_configuration() {
    let root = Directory::new(); let bytes = seed(&root, true);
    let profile = SubmitProfile::decode(recovery_text(&root, None).as_bytes()).unwrap(); let before = root.bytes();
    for waiting in [false, true] {
        let schema = if waiting { "fa.supervised-witnesses/4" } else { "fa.supervised-witnesses/2" };
        let wait = if waiting { ",\"max_retries\":2" } else { "" };
        let text = witness_text(&root).replace("fa.supervised-witnesses/1", schema)
            .replace("\"source\":91", &format!(r#""source":91{wait},"feed":{{"path":"{}","source":41,"after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}}"#,
                root.path("missing-feed.bin").display()));
        let publication = PublicationProfile::decode(text.as_bytes()).unwrap();
        let error = submit_selected(configured(&root, 3), Submission { document: &bytes, profile: &profile, peers: None, credibility: None },
            &selection(&root), Some(&publication), || panic!("stronger profile must refuse before the invocation clock")).err().unwrap();
        assert!(error.contains("not downgraded")); assert_eq!(root.bytes(), before);
    }
    let plain: Vec<String> = ["submit", "/config", "/document", "--guarded", "/recovery", "--topology-profile", "/topology"].map(str::to_owned).to_vec();
    let mut valid = plain.clone(); valid.extend(["--publication-profile".into(), "/witness".into()]);
    let (rest, path) = publication_option(&valid).unwrap(); assert_eq!(rest, plain); assert_eq!(path, Some(Path::new("/witness")));
    let mut both = valid.clone(); both.extend(["--reviewer-profile".into(), "/reviewer".into()]);
    let (rest, _) = publication_option(&both).unwrap(); assert_eq!(arguments(&rest).unwrap().2, Some(Path::new("/reviewer")));
    let mut malformed = vec![valid[..8].to_vec()];
    for index in [1, 2, 4, 6, 8] {
        let mut changed = valid.clone(); changed[index] = "--publication-profile".into(); malformed.push(changed);
    }
    for flag in ["--publication-profile", "--guarded", "--topology-profile", "--receipts-only", "--peers"] {
        let mut changed = valid.clone(); changed.extend([flag.into(), "/extra".into()]); malformed.push(changed);
    }
    for args in malformed {
        assert_eq!(command(&args, None, &mut Vec::new()).unwrap_err(), USAGE);
    }
}

#[test]
fn checked_replay_expiry_keeps_original_charges_and_pending_storage_untouched() {
    let root = Directory::new(); let bytes = seed(&root, true); let publication = witness(&root);
    let profile = SubmitProfile::decode(recovery_text(&root, None).as_bytes()).unwrap();
    let config = configured(&root, 0); let count = FileOversight::read_publication(&config.store, &config.profile).unwrap().revision as usize;
    fs::write(root.path("store/delivery.pending"), b"preserve checked staging").unwrap(); let before = root.bytes();
    // Includes the final event and Ready: no successful profile check permits
    // skipping the same cancellation boundary that guards the original replay.
    for boundary in [1, 2, 10, count + 10, count + 11] {
        let mut calls = 0;
        let error = submit_selected_with_elapsed(configured(&root, 3), Submission { document: &bytes,
            profile: &profile, peers: None, credibility: None }, &selection(&root), Some(&publication),
            || ElapsedTick(1001), Instant::now(), &mut || {
                calls += 1; if calls == boundary { Duration::from_secs(1000) } else { Duration::ZERO }
            }).err().unwrap();
        assert!(error.contains("replay deadline"), "boundary {boundary}: {error}");
        assert_eq!(calls, boundary); assert_eq!(root.bytes(), before);
        assert_eq!(fs::read(root.path("store/delivery.pending")).unwrap(), b"preserve checked staging");
    }
    let receipt = submit_selected(configured(&root, 3), Submission { document: &bytes, profile: &profile, peers: None, credibility: None },
        &selection(&root), Some(&publication), || ElapsedTick(1001)).unwrap();
    assert!(executed(&receipt)); assert!(receipt.failure.is_none());
}

#[test]
fn checked_continuation_still_requires_a_new_independent_human_approval() {
    let root = Directory::new(); seed(&root, true); let config = configured(&root, 3);
    let bytes = document(&config, 2, 3, 2, ElapsedTick(100000)); captures(&root, &config, &bytes);
    let publication = witness(&root); let recovery = SubmitProfile::decode(recovery_text(&root, None).as_bytes()).unwrap();
    let human = reviewer(&config, 2, ReviewDecision::Reject);
    let store = config.store.clone(); let bootstrap = config.profile.clone();
    let result = submit_selected(config, Submission { document: &bytes, profile: &recovery, peers: None, credibility: None },
        &selection(&root), Some(&publication), || ElapsedTick(1001)).unwrap();
    human.join().unwrap(); assert!(!executed(&result)); assert_eq!(result.cleanup_pending, 0);
    let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
    assert_eq!(state.executions, 1); assert_eq!(state.control.ledger.charged, 5); assert_eq!(state.control.ledger.reserved, 0);
}

#[test]
fn actual_command_selects_checked_topology_recovery_without_changing_actor_bytes() {
    let root = Directory::new(); seed(&root, true); let config = configured(&root, 3);
    let now = crate::workflow::clock();
    let bytes = document(&config, 2, 3, 2, ElapsedTick(now.0 + 60000)); captures(&root, &config, &bytes);
    fs::write(root.path("submit.json"), &bytes).unwrap();
    fs::write(root.path("recovery.json"), recovery_text(&root, Some("cli.anchor"))).unwrap();
    fs::write(root.path("topology-profile.json"), selection_text(&root, 1, true, 2, 2)).unwrap();
    fs::write(root.path("witness.json"), witness_text(&root)).unwrap();
    let mut text = TEMPLATE.replace("/operator/publication-store", root.path("store").to_str().unwrap())
        .replace("/operator/evidence.json", root.path("evidence.json").to_str().unwrap())
        .replace("/operator/helpers/alpha", std::env::current_exe().unwrap().to_str().unwrap())
        .replace("/operator/helpers/beta", std::env::current_exe().unwrap().to_str().unwrap())
        .replace("/operator/helpers", root.0.to_str().unwrap())
        .replace("\"arguments\": []", &format!(r#""arguments": ["--exact", "{CHILD}", "--nocapture"]"#));
    for member in ["alpha", "beta"] {
        text = text.replacen("\"environment\": {}", &format!(r#""environment": {{"{MEMBER}":"{member}","{EPOCH}":"3"}}"#), 1);
    }
    let read = Config::decode(text.as_bytes()).unwrap();
    crate::config::assert_oversight_profiles_eq(&read.profile, &config.profile);
    fs::write(root.path("config.json"), text).unwrap();
    let human = reviewer(&config, 2, ReviewDecision::Approve);
    let result = crate::command(vec!["submit".into(), root.path("config.json").display().to_string(), root.path("submit.json").display().to_string(),
        "--publication-profile".into(), root.path("witness.json").display().to_string(),
        "--guarded".into(), root.path("recovery.json").display().to_string(),
        "--topology-profile".into(), root.path("topology-profile.json").display().to_string()]);
    human.join().unwrap(); result.unwrap(); assert_eq!(fs::read(root.path("submit.json")).unwrap(), bytes);
    let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(state.executions, 2); assert_eq!(state.control.ledger.charged, 11);
    assert!(root.path("cli.anchor").exists());
}
