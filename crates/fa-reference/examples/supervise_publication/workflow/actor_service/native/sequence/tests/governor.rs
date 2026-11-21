//! A separate authenticated governor changes one ORIGINAL resident native
//! owner. Real source intake, numerical work, helper processes and human keys
//! still precede each cumulative Message/Finish publication.
#[path = "governed_recovery.rs"]
mod recovery_tests;

use super::*;
use crate::workflow::actor_service::{governor as live_governor, operator as live_operator};
use fa_reference::Snapshot;
use fa_reference::action::consequence::oversight::{
    evidence_source::{EvidenceIdentity, EvidenceSnapshot},
    helper_client::{ClientPhase, HelperClient},
    helper_processes::HelperProgram,
};
use fa_reference::address_resolver::Address;
use fa_reference::round::Verdict;
use fa_reference::strict_json::{self, Json, Limits};
use fa_reference::tower::{ReadArguments, TowerRequest, Verb, encode_request};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    io::Read,
    path::Path,
    process::{Child, Command as ProcessCommand},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

const GENERATION: u64 = 61;
const CAMPAIGN: u64 = 701;
const OPERATION: u64 = 702;
const CHILD_ROOT: &str = "FA_NATIVE_LIVE_GOVERNOR_ROOT";
const CHILD_TEST: &str = "workflow::actor_service::native::sequence::tests::governor_tests::independent_native_governor_process";
const HELPER_ROOT: &str = "FA_NATIVE_LIVE_HELPER_ROOT";
const HELPER_MEMBER: &str = "FA_NATIVE_LIVE_HELPER_MEMBER";
const HELPER_TEST: &str = "workflow::actor_service::native::sequence::tests::governor_tests::independent_native_helper_process";
const FRESH_SOURCE: &str = "fresh evidence after acknowledged native policy promotion";

struct GovernorChild(Child);
impl GovernorChild {
    fn start(root: &Root) -> Self {
        Self(
            ProcessCommand::new(std::env::current_exe().unwrap())
                .args(["--exact", CHILD_TEST, "--nocapture"])
                .env(CHILD_ROOT, &root.0)
                .spawn()
                .unwrap(),
        )
    }

    fn finish(&mut self) {
        let started = Instant::now();
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                assert!(status.success());
                return;
            }
            assert!(started.elapsed() < Duration::from_secs(25));
            pause(1);
        }
    }
}
impl Drop for GovernorChild {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn wait(path: &Path) {
    let started = Instant::now();
    while !path.exists() {
        assert!(
            started.elapsed() < Duration::from_secs(25),
            "missing {}",
            path.display()
        );
        pause(1);
    }
}

fn write_complete(path: &Path, bytes: impl AsRef<[u8]>) {
    let staging = path.with_extension("pending");
    fs::write(&staging, bytes).unwrap();
    fs::rename(staging, path).unwrap();
}

fn json(bytes: &[u8]) -> Json {
    strict_json::parse(
        bytes,
        Limits {
            max_bytes: 1_048_576,
            max_depth: 32,
            max_items: 32_768,
            max_string_bytes: 1_048_576,
        },
    )
    .unwrap()
}

fn field<'a>(value: &'a Json, key: &str) -> &'a Json {
    value.get(key).unwrap()
}

fn number(value: &Json, key: &str) -> u64 {
    field(value, key).as_str().unwrap().parse().unwrap()
}

fn known<'a>(response: &'a Json, phase: &str) -> &'a Json {
    assert_eq!(field(response, "phase").as_str(), Some(phase));
    let knowledge = field(response, "knowledge");
    assert_eq!(field(knowledge, "state").as_str(), Some("known"));
    field(knowledge, "value")
}

fn exchange(root: &Path, name: &str, request: String) -> Json {
    let path = root.join(name);
    write_complete(&path, request);
    let mut output = Vec::new();
    crate::command_with_output(
        vec![
            "policy-governor-exchange".into(),
            root.join("governor.json").display().to_string(),
            path.display().to_string(),
        ],
        &mut output,
    )
    .unwrap();
    json(&output)
}

fn inspect(root: &Path, scope: &str, query: u64) -> Json {
    exchange(
        root,
        &format!("inspect-{query}.json"),
        format!(
            r#"{{"schema":"fa.policy-governor-request/1","scope":{scope},"verb":"get","query":"{query}"}}"#
        ),
    )
}

fn campaign_request(scope: &str, cut: &Json, campaign: u64) -> String {
    format!(
        r#"{{"schema":"fa.policy-governor-request/1","scope":{scope},"verb":"policy-replay","operation":"{campaign}","expected":{{"journal_revision":{},"control_sequence":{},"authority_epoch":{},"policy_generation":"1"}},"candidate":{{"schema_version":"fa.policy-replay-request/1","expected_policy_generation":1,"limits":{{"cases":16,"input_bytes":1048576,"output_bytes":65536}},"candidate":{{"generation":2,"nodes":[{{"op":"exact_value","key":7,"value_hex":"6f6b"}},{{"op":"payload_at_most","maximum":256}},{{"op":"units_at_most","maximum":512}},{{"op":"all","children":[0,1,2]}}]}}}}}}"#,
        live_governor::protocol::json(field(cut, "journal_revision")),
        live_governor::protocol::json(field(cut, "control_sequence")),
        live_governor::protocol::json(field(cut, "authority_epoch")),
    )
}

#[test]
fn independent_native_governor_process() {
    let Some(root) = std::env::var_os(CHILD_ROOT) else {
        return;
    };
    let root = PathBuf::from(root);
    wait(&root.join("ready"));
    let settings = live_governor::Settings::read(&root.join("governor.json")).unwrap();
    let scope = live_governor::protocol::scope_json(settings.scope);
    let inspected = inspect(&root, &scope, 1);
    let before = known(&inspected, "inspect");
    assert_eq!(number(before, "authority_epoch"), 0);
    assert_eq!(
        number(field(before, "numerical_replay"), "remaining_events"),
        8192
    );
    assert!(
        !root.join("evidence.json").exists(),
        "campaign replay must consume retained original closures"
    );
    let reviewed = exchange(
        &root,
        "review.json",
        campaign_request(&scope, before, CAMPAIGN),
    );
    let value = known(&reviewed, "review");
    let report = field(value, "reviewed_report").as_str().unwrap();
    let decoded = json(report.as_bytes());
    let replay = field(field(&decoded, "knowledge"), "value");
    assert_eq!(field(replay, "case_count").as_u64(), Some(2));
    assert_eq!(field(field(replay, "cut"), "executions").as_u64(), Some(1));
    assert_eq!(field(replay, "requires_shadow").as_bool(), Some(false));
    assert!(
        field(replay, "newly_reviewable")
            .as_array()
            .unwrap()
            .is_empty()
    );

    let inspected = inspect(&root, &scope, 2);
    let pending = known(&inspected, "inspect");
    assert_eq!(
        number(pending, "journal_revision"),
        number(before, "journal_revision") + 1
    );
    assert_eq!(
        number(field(pending, "numerical_replay"), "remaining_events"),
        8192 - number(before, "journal_revision") - 1,
    );
    let canonical = fs::read(root.join("store/delivery.bin")).unwrap();
    let exhausted = exchange(
        &root,
        "exhausted.json",
        campaign_request(&scope, pending, CAMPAIGN + 1),
    );
    assert_eq!(
        field(field(&exhausted, "error"), "code").as_str(),
        Some("capacity")
    );
    assert_eq!(
        fs::read(root.join("store/delivery.bin")).unwrap(),
        canonical
    );

    // The independently authenticated custodian approves the COMPLETE report;
    // neither the exchange client nor the resident supervisor makes this choice.
    let approved = exchange(
        &root,
        "approve.json",
        format!(
            r#"{{"schema":"fa.policy-governor-request/1","scope":{scope},"verb":"promote","phase":"approve","campaign":"{CAMPAIGN}","expected_journal_revision":{},"reviewed_report":{},"accept_newly_reviewable":false}}"#,
            live_governor::protocol::json(field(value, "journal_revision")),
            live_governor::protocol::quote(report),
        ),
    );
    let approval = known(&approved, "approve");
    assert_eq!(field(approval, "approval").as_str(), Some("delegated"));
    assert_eq!(field(approval, "effect_authority").as_bool(), Some(false));
    assert_eq!(field(approval, "policy_changed").as_bool(), Some(false));
    let inspected = inspect(&root, &scope, 3);
    let after = known(&inspected, "inspect");
    assert_eq!(number(after, "authority_epoch"), 0);
    assert_eq!(
        number(field(after, "numerical_replay"), "remaining_events"),
        8192 - (number(before, "journal_revision") + 1) - (number(pending, "journal_revision") + 1),
        "reconnect must not replenish either entered numerical replay",
    );
    write_complete(
        &root.join("approved"),
        live_governor::protocol::json(&approved),
    );
}

/// A test helper obtains its independently selected current policy profile
/// from the original acknowledged journal, never from its untrusted request.
#[test]
fn independent_native_helper_process() {
    let Some(root) = std::env::var_os(HELPER_ROOT) else {
        return;
    };
    let member = std::env::var(HELPER_MEMBER).unwrap();
    let mut config = Config::decode(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/supervised_publication.json"
    )))
    .unwrap();
    config.profile.delivery.initial_payload.clear();
    let epoch = FileOversight::read_publication(PathBuf::from(root).join("store"), &config.profile)
        .unwrap()
        .control
        .ledger
        .epoch;
    let expected = config.profile.committee.members()[&member].profile_at(epoch);
    let mut client = HelperClient::from_process_stdin(expected).unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(25));
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference {
            let input = client.input().unwrap();
            assert_eq!(input.member(), member);
            let own = format!("review this fixture: {member}");
            assert!(
                input
                    .actual_input()
                    .submitted_bytes()
                    .windows(own.len())
                    .any(|bytes| bytes == own.as_bytes())
            );
            let fresh = input
                .actual_input()
                .submitted_bytes()
                .windows(FRESH_SOURCE.len())
                .any(|bytes| bytes == FRESH_SOURCE.as_bytes());
            assert_eq!(
                fresh,
                epoch != 0,
                "helpers require newly acquired source evidence after promotion"
            );
            client.respond(Verdict::Allow, member.as_bytes()).unwrap();
        }
        if client.phase() == ClientPhase::ReplySent {
            return;
        }
        pause(1);
    }
}

fn helper_programs(root: &Root) -> BTreeMap<String, HelperProgram> {
    ["alpha", "beta"]
        .into_iter()
        .map(|member| {
            (
                member.to_owned(),
                HelperProgram::new(
                    std::env::current_exe().unwrap(),
                    root.0.clone(),
                    vec!["--exact".into(), HELPER_TEST.into(), "--nocapture".into()],
                    BTreeMap::from([
                        (OsString::from(HELPER_ROOT), root.0.clone().into_os_string()),
                        (OsString::from(HELPER_MEMBER), OsString::from(member)),
                    ]),
                )
                .unwrap(),
            )
        })
        .collect()
}

fn selections(
    root: &Root,
    actor: &Profile,
    governor_pid: u32,
) -> (live_governor::Settings, operator::Selection) {
    let scope = actor.scope;
    let scope = format!(
        r#"{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}}"#,
        scope.tenant, scope.principal, scope.run, scope.branch, scope.authority,
    );
    let identity = actor.supervisor;
    let principal = format!(
        r#"{{"uid":{},"gid":{},"pid":{}}}"#,
        identity.uid(),
        identity.gid(),
        identity.pid().unwrap(),
    );
    let governor = format!(
        r#"{{"schema":"fa.policy-governor-service/1","scope":{scope},"socket":{},"supervisor":{principal},"governor":{{"uid":{},"gid":{},"pid":{governor_pid}}},"candidate_limit":8,"connection_limit":8,"exchange_limit":8,"runtime_ms":15000,"poll_ms":1,"campaign_limit":1,"report_bytes":65536,"campaigns":{{"cases":16,"input_bytes":1048576,"max_campaigns":4}},"numerical_replay":{{"max_replay_events":4096,"events":8192}}}}"#,
        live_governor::protocol::quote(root.0.join("governor.sock").to_str().unwrap()),
        identity.uid(),
        identity.gid(),
    );
    fs::write(root.0.join("governor.json"), &governor).unwrap();
    let operator = format!(
        r#"{{"schema":"fa.tower-operator-service/1","scope":{scope},"socket":{},"supervisor":{principal},"operator":{principal},"candidate_limit":1,"connection_limit":1,"exchange_limit":8,"runtime_ms":15000,"poll_ms":1,"inspection_queries":0,"explanation_queries":0,"explanation_replay_events":0,"explanation_disclosure":"none","policy_promotion":{{"queries":2,"max_replay_events":4096,"numerical_replay_events":4096,"authority":"promote_approved_policy"}}}}"#,
        live_governor::protocol::quote(root.0.join("operator.sock").to_str().unwrap()),
    );
    (
        live_governor::Settings::decode(governor.as_bytes()).unwrap(),
        operator::Selection::new_governed(
            live_operator::Settings::decode(operator.as_bytes()).unwrap(),
            GENERATION,
        )
        .unwrap(),
    )
}

fn barrier(root: &Root, entry: &mut Entry) -> PathBuf {
    let socket = root.0.join("draft.sock");
    entry.precheck = Some(
        exact_precheck::Selection::new(
            crate::workflow::actor_service::precheck::Settings::decode(
                format!(
                    r#"{{"schema":"fa.actor-precheck-service/1","policy_disclosure":"exact_predicates","socket":{},"expected_policy_epoch":0,"disclosure_id":191,"query_limit":1,"exchange_limit":1,"candidate_limit":1,"connection_limit":1,"runtime_ms":15000}}"#,
                    live_governor::protocol::quote(socket.to_str().unwrap()),
                ).as_bytes(),
            ).unwrap(),
        ).unwrap(),
    );
    socket
}

/// Hold the second original reference until the service explicitly refreshes
/// its epoch after promotion. The actor never fabricates a source identity
/// or substitutes candidate publication bytes. Causal negatives send another
/// valid release kind or deadline through the same refreshed source boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FreshChange {
    Exact,
    Finish,
    EarlierDeadline,
}

struct GovernedOutput {
    profiles: Vec<Profile>,
    pending: Vec<u8>,
    documents: Vec<Vec<u8>>,
    submitted: BTreeSet<u64>,
    clients: Vec<JoinHandle<WireResponse>>,
    released: Arc<AtomicBool>,
    change: FreshChange,
    refreshed: Arc<AtomicBool>,
}

impl GovernedOutput {
    fn new(entries: &[Entry]) -> Self {
        Self {
            profiles: entries.iter().map(|entry| entry.actor.clone()).collect(),
            pending: Vec::new(),
            documents: Vec::new(),
            submitted: BTreeSet::new(),
            clients: Vec::new(),
            released: Arc::new(AtomicBool::new(false)),
            change: FreshChange::Exact,
            refreshed: Arc::new(AtomicBool::new(false)),
        }
    }

    fn join(self) -> Vec<WireResponse> {
        assert!(self.pending.is_empty());
        self.clients
            .into_iter()
            .map(|client| client.join().unwrap())
            .collect()
    }
}

impl Write for GovernedOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.pending.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let document = std::mem::take(&mut self.pending);
        let Command::Submit { request, proposal } = decode_command(&document).unwrap() else {
            panic!("native service must emit original source-only Submit references");
        };
        let index = REQUESTS
            .iter()
            .position(|selected| *selected == request)
            .unwrap();
        assert_eq!(
            proposal.payload.len(),
            FileGeneratedTextActorPort::INTENT_BYTES
        );
        if index == 1 && proposal.expected_policy_epoch == 0 {
            assert!(!self.submitted.contains(&request));
        } else {
            assert_eq!(proposal.expected_policy_epoch, u64::from(index != 0));
            assert!(
                self.submitted.insert(request),
                "refresh must not resubmit an admitted request"
            );
            let profile = self.profiles[index].clone();
            let submitted = match (index, self.change) {
                (1, FreshChange::Finish) => {
                    let finish = FileGeneratedTextActorPort::encode_finish(
                        request,
                        proposal.target,
                        proposal.expected_policy_epoch,
                        proposal.deadline,
                    )
                    .unwrap();
                    assert_ne!(finish.payload, proposal.payload);
                    encode_command(&Command::Submit {
                        request,
                        proposal: finish,
                    })
                    .unwrap()
                }
                (1, FreshChange::EarlierDeadline) => {
                    let mut changed = proposal.clone();
                    changed.deadline = ElapsedTick(changed.deadline.0.checked_sub(1).unwrap());
                    assert!(changed.deadline > ElapsedTick(1000));
                    encode_command(&Command::Submit {
                        request,
                        proposal: changed,
                    })
                    .unwrap()
                }
                _ => document.clone(),
            };
            if index == 1 {
                self.refreshed.store(true, Ordering::Release);
            }
            if index == 0 {
                self.clients.push(actor(profile, submitted));
            } else {
                let released = self.released.clone();
                self.clients.push(thread::spawn(move || {
                    let started = Instant::now();
                    while !released.load(Ordering::Acquire) {
                        assert!(started.elapsed() < Duration::from_secs(25));
                        pause(1);
                    }
                    actor(profile, submitted).join().unwrap()
                }));
            }
        }
        self.documents.push(document);
        Ok(())
    }
}

fn operator_exchange(socket: &mut UnixStream, request: &str) -> Json {
    socket.write_all(request.as_bytes()).unwrap();
    if !request.ends_with('\n') {
        socket.write_all(b"\n").unwrap();
    }
    socket.flush().unwrap();
    let mut bytes = Vec::new();
    loop {
        let mut byte = [0];
        socket.read_exact(&mut byte).unwrap();
        bytes.push(byte[0]);
        assert!(bytes.len() <= 1_048_576);
        if byte[0] == b'\n' {
            return json(&bytes);
        }
    }
}

fn operator_known<'a>(response: &'a Json, kind: &str) -> &'a Json {
    let result = field(response, "result");
    assert_eq!(field(result, "kind").as_str(), Some(kind));
    let knowledge = field(field(result, "value"), "knowledge");
    assert_eq!(field(knowledge, "state").as_str(), Some("known"));
    field(knowledge, "value")
}

#[test]
fn a_separate_governor_promotes_real_native_history_and_continues_messages_and_finish() {
    governed_sequence(ReviewDecision::Approve, FreshChange::Exact);
}

#[test]
fn native_policy_promotion_cannot_replace_the_next_independent_human_decision() {
    governed_sequence(ReviewDecision::Reject, FreshChange::Exact);
}

#[test]
fn refreshed_native_reference_does_not_authorize_a_different_release_or_deadline() {
    for change in [FreshChange::Finish, FreshChange::EarlierDeadline] {
        governed_sequence(ReviewDecision::Approve, change);
    }
}

fn governed_sequence(decision: ReviewDecision, change: FreshChange) {
    let changed = change != FreshChange::Exact;
    let root = Root::new();
    let (mut config, mut entries, reviewer) = selected(&root, true);
    config.programs = helper_programs(&root);
    let reader = Reader::new(&config, &entries[0]);
    let peer_store = config.store.clone();
    let peer_profile = config.profile.clone();
    let barrier = barrier(&root, &mut entries[1]);
    let mut child = GovernorChild::start(&root);
    assert_ne!(child.0.id(), std::process::id());
    let (governor, selected_operator) = selections(&root, &entries[0].actor, child.0.id());
    let mut output = GovernedOutput::new(&entries);
    output.change = change;
    let release_actor = output.released.clone();
    let refreshed = output.refreshed.clone();
    let profiles = output.profiles.clone();
    let mut humans = Humans::new(
        &reviewer,
        &reader,
        [
            Some(ReviewDecision::Approve),
            None,
            Some(ReviewDecision::Approve),
        ],
    );
    let human_ready = Arc::new(AtomicBool::new(false));
    let release_human = human_ready.clone();
    let directory = root.0.clone();
    let source = config.source.path().to_owned();
    let operator_settings = selected_operator.settings.clone();
    let next_human = reviewer.socket(REQUESTS[1]);
    let scope = entries[0].actor.scope;
    let fresh_source = EvidenceSnapshot::new(
        EvidenceIdentity {
            scope,
            source: 51,
            generation: 2,
        },
        Snapshot {
            semantic_epoch: 2,
            complete: true,
            values: BTreeMap::from([(7, b"ok".to_vec())]),
        },
        ["alpha", "beta"]
            .into_iter()
            .map(|member| {
                (
                    member.to_owned(),
                    format!("review this fixture: {member}; {FRESH_SOURCE}").into_bytes(),
                )
            })
            .collect(),
    )
    .unwrap()
    .encode();
    let operator_peer = thread::spawn(move || {
        // Decoder assets use local Rc custody. Reconstruct the same immutable
        // recipe inside this peer instead of sending its reader across threads.
        let inputs = recipe::load(
            &directory.join("recipe.json"),
            peer_profile.delivery.scope.tenant,
            peer_profile.delivery.limits.bytes,
        )
        .unwrap();
        let peer_reader = Reader {
            store: peer_store,
            profile: peer_profile,
            decoder: inputs.decoder,
            tokenizer: inputs.tokenizer,
            stream: inputs.stream,
        };
        wait(&barrier);
        let before = peer_reader.publication();
        assert_eq!(before.executions, 1);
        assert_eq!(before.control.ledger.epoch, 0);
        assert_eq!(before.payload, b"A");
        let numerical = peer_reader.progress(8);
        assert_eq!(
            (
                numerical.numerical.numerical.position,
                numerical.numerical.numerical.sampled_draws
            ),
            (8, 4)
        );
        let retained = fs::read(&source).unwrap();
        assert_ne!(retained, fresh_source);
        fs::remove_file(&source).unwrap();
        let mut socket = operator_tests::connect(&operator_settings);
        let mut request = TowerRequest::new(
            "native-campaign-promotion",
            Address::parse(&format!(
                "fa://{}/run/{}@{GENERATION}",
                scope.tenant, scope.run
            ))
            .unwrap(),
            Verb::Next,
        );
        request.arguments = ReadArguments::PolicyPromotionNext {
            campaign: CAMPAIGN,
            operation: OPERATION,
        };
        let next = encode_request(&request).unwrap();
        let unavailable = operator_exchange(&mut socket, &next);
        assert_eq!(
            field(field(&unavailable, "envelope"), "reason_code").as_str(),
            Some("CapabilityUnsupported")
        );
        write_complete(
            &directory.join("ready"),
            b"second original generation is complete",
        );
        wait(&directory.join("approved"));
        let approved = peer_reader.publication();
        assert_eq!(approved.revision, before.revision + 2);
        assert_eq!(approved.control.ledger.epoch, 0);
        let planned = operator_exchange(&mut socket, &next);
        let plan = operator_known(&planned, "policy-promotion-next");
        assert_eq!(field(plan, "can_promote").as_bool(), Some(true));
        let command = live_governor::protocol::json(field(plan, "action"));
        let result = operator_exchange(&mut socket, &command);
        operator_known(&result, "policy-promotion");
        let after = peer_reader.publication();
        assert_eq!(after.revision, approved.revision + 1);
        assert_eq!(after.control.ledger.epoch, 1);
        assert_eq!(
            (after.executions, after.control.ledger.charged),
            (before.executions, before.control.ledger.charged)
        );
        assert!(
            !source.exists(),
            "promotion must not acquire current source"
        );
        let started = Instant::now();
        while !refreshed.load(Ordering::Acquire) {
            assert!(
                started.elapsed() < Duration::from_secs(25),
                "promotion failed to refresh the ready source reference"
            );
            pause(1);
        }
        assert!(
            !barrier.exists(),
            "the old exact-predicate disclosure survived the epoch change"
        );
        write_complete(&source, fresh_source);
        if changed {
            // Any wrongly offered review is approved, so this negative cannot
            // pass merely because its independent human withheld an effect key.
            release_human.store(true, Ordering::Release);
            release_actor.store(true, Ordering::Release);
            return;
        }
        release_actor.store(true, Ordering::Release);
        wait(&next_human);
        let awaiting = peer_reader.publication();
        assert_eq!(
            awaiting.executions, 1,
            "governor approval is not a human effect key"
        );
        let canonical = fs::read(directory.join("store/delivery.bin")).unwrap();
        assert_eq!(operator_exchange(&mut socket, &command), result);
        assert_eq!(
            fs::read(directory.join("store/delivery.bin")).unwrap(),
            canonical,
            "promotion retry disturbed the fresh review"
        );
        release_human.store(true, Ordering::Release);
    });
    let result = run_with_governor(
        config,
        entries,
        &reviewer,
        None,
        (None, Some(&selected_operator)),
        None,
        None,
        Some(&governor),
        || {
            if human_ready.load(Ordering::Acquire) {
                humans.decisions[1] = Some(decision);
            }
            humans.poll();
            ElapsedTick(1000)
        },
        &mut output,
    );
    operator_peer.join().unwrap();
    child.finish();
    let references = output
        .documents
        .iter()
        .map(|document| {
            let Command::Submit { request, proposal } = decode_command(document).unwrap() else {
                unreachable!()
            };
            (request, proposal)
        })
        .collect::<Vec<_>>();
    let old = references
        .iter()
        .find(|(request, proposal)| *request == REQUESTS[1] && proposal.expected_policy_epoch == 0)
        .unwrap();
    let fresh = references
        .iter()
        .find(|(request, proposal)| *request == REQUESTS[1] && proposal.expected_policy_epoch == 1)
        .unwrap();
    assert_eq!(old.1.payload, fresh.1.payload);
    assert_eq!(
        (old.1.target, old.1.units, old.1.deadline),
        (fresh.1.target, fresh.1.units, fresh.1.deadline)
    );
    let packets = humans.join();
    let replies = output.join();
    assert!(executed(&replies[0]));
    let first = packets[0].as_ref().unwrap();
    assert_eq!(first.action().spec().policy_epoch, 0);
    if changed {
        assert!(
            packets[1].is_none(),
            "changed {change:?} source reference reached human review"
        );
        assert!(matches!(
            &replies[1].result,
            Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, basis })
                if basis.source == BasisSource::ControlLedger
        ));
    } else {
        let second = packets[1].as_ref().unwrap();
        assert_ne!(first.binding().session, second.binding().session);
        assert_eq!(second.action().spec().policy_epoch, 1);
        assert_eq!(second.policy_generation(), 2);
        assert_eq!(second.views().len(), 2);
        let second_frame = ReleaseFrame::decode(&second.action().spec().payload).unwrap();
        assert_eq!(second_frame.prior_messages(), &["A"]);
        assert_eq!(second_frame.message(), Some("A"));
    }
    let stream = reader.stream();
    if decision == ReviewDecision::Approve && !changed {
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(replies.len(), 3);
        assert!(replies.iter().all(executed));
        assert_eq!(
            (
                stream.publication.executions,
                stream.publication.payload.as_slice()
            ),
            (3, b"AA".as_slice())
        );
        assert!(stream.confirmed.finished());
        assert_eq!(
            stream.publication.control.ledger.charged,
            packets
                .iter()
                .map(|packet| packet.as_ref().unwrap().action().spec().units)
                .sum::<u64>()
        );
        let finish = packets[2].as_ref().unwrap();
        assert_eq!(finish.action().spec().policy_epoch, 1);
        assert_eq!(finish.policy_generation(), 2);
        assert!(
            ReleaseFrame::decode(&finish.action().spec().payload)
                .unwrap()
                .is_finish()
        );
    } else {
        assert!(result.is_err());
        assert_eq!(replies.len(), 2);
        assert!(!executed(&replies[1]));
        assert!(packets[2].is_none());
        assert_eq!(
            (
                stream.publication.executions,
                stream.publication.payload.as_slice()
            ),
            (1, b"A".as_slice())
        );
        assert!(!stream.confirmed.finished());
        assert_eq!(
            stream.publication.control.ledger.charged,
            first.action().spec().units
        );
    }
    assert_eq!(stream.publication.control.ledger.epoch, 1);
    assert_eq!(stream.publication.control.ledger.reserved, 0);
    assert_eq!(
        stream.publication.control.ledger.available + stream.publication.control.ledger.charged,
        reader.profile.delivery.total
    );
    assert_eq!(stream.confirmed, stream.published);
    assert!(stream.pending.is_none());
    let numerical = reader.progress(8);
    assert_eq!(
        (
            numerical.numerical.numerical.position,
            numerical.numerical.numerical.sampled_draws
        ),
        (8, 4),
        "policy change must not regenerate a ready Message or replenish work"
    );
    reader.no_generation(9);
    assert!(!governor.socket.exists());
    assert!(!selected_operator.settings.socket.exists());
    closed(&profiles, &reviewer);
}

#[test]
fn governed_native_selection_refuses_missing_custody_or_work_before_time_or_store() {
    for case in 0..8 {
        let root = Root::new();
        let (config, entries, reviewer) = selected(&root, true);
        let (mut governor, selected_operator) = selections(
            &root,
            &entries[0].actor,
            std::process::id().checked_add(10_000).unwrap(),
        );
        let mut after = None;
        let mut credibility = None;
        let mut supplied_governor = true;
        let mut supplied_operator = true;
        let absent = root.0.join("absent-qualification.json");
        let expected = match case {
            0 => {
                supplied_governor = false;
                "both independent"
            }
            1 => {
                supplied_operator = false;
                "both independent"
            }
            2 => {
                after = Some(REQUESTS[0]);
                "new serve-native-generated"
            }
            3 => {
                credibility = Some(absent.as_path());
                "new serve-native-generated"
            }
            4 => {
                governor.numerical_replay = None;
                "numerical"
            }
            5 => {
                governor.governor = entries[2].actor.actor;
                "overlapping actor credentials"
            }
            6 => {
                governor.socket = entries[2].actor.socket.clone();
                "conflict"
            }
            7 => {
                governor.socket = reviewer.socket(REQUESTS[2]);
                "conflict"
            }
            _ => unreachable!(),
        };
        let store = config.store.clone();
        let profiles = entries
            .iter()
            .map(|entry| entry.actor.clone())
            .collect::<Vec<_>>();
        let error = run_with_governor(
            config,
            entries,
            &reviewer,
            None,
            (after, supplied_operator.then_some(&selected_operator)),
            credibility,
            None,
            supplied_governor.then_some(&governor),
            || panic!("invalid governed selection reached the clock"),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(error.contains(expected), "case {case}: {error}");
        assert!(!store.exists());
        assert!(!governor.socket.exists());
        assert!(!selected_operator.settings.socket.exists());
        closed(&profiles, &reviewer);
    }
}

#[test]
fn native_governance_requires_explicit_numerical_promotion_authority() {
    let root = Root::new();
    let (config, entries, _) = selected(&root, true);
    let (_, selected_operator) = selections(
        &root,
        &entries[0].actor,
        std::process::id().checked_add(10_000).unwrap(),
    );
    let mut settings = selected_operator.settings.clone();
    assert!(operator::Selection::new(settings.clone(), GENERATION).is_err());
    settings
        .policy_promotion
        .as_mut()
        .unwrap()
        .numerical_replay_events = None;
    assert!(operator::Selection::new_governed(settings, GENERATION).is_err());
    assert!(!config.store.exists());
}

#[test]
fn original_stop_remains_live_while_a_governed_native_release_waits_for_actor() {
    let root = Root::new();
    let (config, mut entries, reviewer) = selected(&root, true);
    let reader = Reader::new(&config, &entries[0]);
    let barrier = barrier(&root, &mut entries[1]);
    let child = GovernorChild::start(&root);
    let (governor, selected_operator) = selections(&root, &entries[0].actor, child.0.id());
    let mut output = GovernedOutput::new(&entries);
    let profiles = output.profiles.clone();
    let mut humans = Humans::new(
        &reviewer,
        &reader,
        [Some(ReviewDecision::Approve), None, None],
    );
    let mut controller = None;
    let result = run_with_governor(
        config,
        entries,
        &reviewer,
        None,
        (None, Some(&selected_operator)),
        None,
        None,
        Some(&governor),
        || {
            humans.poll();
            if controller.is_none() && barrier.exists() {
                assert!(governor.socket.exists());
                assert!(selected_operator.settings.socket.exists());
                assert_eq!(reader.publication().executions, 1);
                controller = Some(stopping(reviewer.clone()));
            }
            ElapsedTick(1000)
        },
        &mut output,
    );
    assert!(result.is_err());
    let receipt = controller
        .expect("completed native draft must reach its original actor boundary")
        .join()
        .unwrap();
    assert!(receipt.acknowledged() && receipt.drained());
    assert_eq!(receipt.binding.operation, REQUESTS[0]);
    let packets = humans.join();
    let replies = output.join();
    assert_eq!(replies.len(), 1);
    assert!(executed(&replies[0]));
    assert!(packets[1].is_none() && packets[2].is_none());
    let after = reader.publication();
    assert_eq!(after.stop.unwrap().request().operation, REQUESTS[0]);
    assert_eq!(after.executions, 1);
    assert_eq!(after.payload, b"A");
    assert_eq!(after.control.ledger.reserved, 0);
    assert_eq!(reader.progress(8).numerical.numerical.sampled_draws, 4);
    assert!(!governor.socket.exists());
    assert!(!selected_operator.settings.socket.exists());
    closed(&profiles, &reviewer);
}
