//! Real learned generation, original two-key publication and sealed receipt ports.
//! Synthetic model/probes and ballots establish contracts, not detector quality.
#![cfg(unix)]
#[path = "support/learned_text_model.rs"]
mod fixture;
use fa_reference::{Error, Snapshot};
use fa_reference::action::{ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope};
use fa_reference::action::consequence::{
    congress::{CongressPolicy, MemberPolicy},
    delivery::{EndpointOutcome, stream::StreamProfile, persistent::{
        FileDeliveryProfile, JournalError, JournalLimits,
        requests::{FileRequestDisposition, actor::{FileLearnedTextStreamActorPort,
            LearnedTextRelease, receipt::FileLearnedReceiptActorPort}},
        observed::{FileOversight, FileOversightProfile, FileHumanReviewer,
            decoder::learned::{FileLearnedConfig, FileLearnedRecovery, FileLearnedRecoveryStatus},
            guarded::{FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
                learned_recovery::FileGuardedLearnedRecovery}},
    }},
    gate::containment::{ActorState, RestartGrade, RestartProfile,
        session::policy::{Policy, Predicate}},
    oversight::{CommitteeContract, CommitteeInput, HelperContract, ReviewWindow, action_frame,
        decoder_monitoring::LearnedDecoderBindingLimits, human::HumanReviewPolicy,
        actor::{ActorError, ActorOutcome, Knowledge},
        actor_wire::{ActorWire, Command, WireError, encode_command}},
};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::reducer::Caps;
use fa_reference::round::{Verdict, commitment};
use std::{collections::BTreeMap, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-learned-receipt-port-{}-{time}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("store") }
    fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        std::fs::read_dir(self.store()).unwrap().map(|entry| entry.unwrap().path())
            .filter(|path| path.is_file()).map(|path| {
                let bytes = std::fs::read(&path).unwrap(); (path, bytes)
            }).collect()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("receipt port test cleanup: {error}"); }
    }
}
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile { delivery: FileDeliveryProfile {
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        total: 4096, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
            model_generation: 3, tokenizer_generation: 4, state_schema_generation: 1,
            grade: RestartGrade::AuditOnly }, Vec::new(), vec![0], vec![0], 0).unwrap(),
        suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap(),
        congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".to_owned(),
            MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
            narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: vec![target], target, initial_payload: Vec::new(),
        retention_ticks: 1000, max_deliveries: 8, clock_domain: 99,
        limits: JournalLimits { events: 128, ..JournalLimits::default() },
    }, committee: CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(), HelperContract::new(
        InputProfileBinding { profile_id: 1, profile_bytes: b"exact-view-v1".to_vec(),
            tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 7, b"approve?".to_vec(),
    ).unwrap())])).unwrap(), human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 } }
}
fn recipe() -> FileLearnedConfig {
    assert_ne!(fixture::END, fixture::OTHER_CONTROL);
    let model = fixture::model(&[b'O' as u32, b'K' as u32, fixture::END]);
    FileLearnedConfig::new_text_stream(model.clone(), fixture::tokenizer(&model), fixture::config(&model),
        LearnedDecoderBindingLimits::default(), StreamProfile::new(1, 1, 4, 64, 128).unwrap()).unwrap()
}
fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }
}
fn dispatch(host: &mut FileOversight, reviewer: &FileHumanReviewer, key: u64) -> (u64, CommitteeInput) {
    let FileRequestDisposition::Admitted { attempt, .. } = host.request_status(key).unwrap().disposition
        else { panic!("original request must be admitted"); };
    let action: FrozenAction = host.request_action(key).unwrap().clone();
    let contracts = profile().committee; let helper = &contracts.members()["reviewer"];
    let mut bytes = action_frame(&action); let boundary = bytes.len();
    bytes.extend_from_slice(helper.question()); let end = bytes.len();
    let actual = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
        SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
    ], Vec::new()).unwrap();
    let view = EvidenceViewManifest::new(actual, AuthorizationProjection { projection_id: 7,
        policy_epoch: action.spec().policy_epoch, projected_originals: Vec::new() }, Vec::new()).unwrap();
    let input = CommitteeInput::capture(&action, &contracts, BTreeMap::from([("reviewer".to_owned(), view)])).unwrap();
    host.record_inputs(host.revision(), attempt, 0, input.clone()).unwrap();
    let round = 100 + attempt;
    host.begin_review(host.revision(), attempt, round, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30) }, snapshot()).unwrap();
    let digest = commitment(round, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap();
    host.commit_review(host.revision(), round, "reviewer", digest).unwrap();
    host.open_reveals(host.revision(), round).unwrap();
    host.reveal_review(host.revision(), round, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), round, Some(&input), snapshot()).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), attempt, &input, snapshot()).unwrap();
    let offer = host.request_human_approval(host.revision(), 1000 + attempt, attempt, &input, ElapsedTick(40)).unwrap();
    let revision = host.revision(); let human = reviewer.approve(host, revision, &offer).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
    (attempt, input)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cut { Pending, Cancelled, Sent, Published, Confirmed, Finished }
struct History {
    root: Directory, recipe: FileLearnedConfig, expected: FileRecoveryRequirements,
    key: u64, after: Option<u64>, outcome: Knowledge<ActorOutcome>,
}
impl History {
    fn new(cut: Cut, full: bool) -> Self {
        let root = Directory::new(); let recipe = recipe();
        let (mut host, reviewer) = FileOversight::create_with_learned_text_stream(
            root.store(), profile(), recipe.clone()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        for _ in 0..4 {
            let n = host.learned_generation_inspection().unwrap().numerical;
            if !n.status.is_active() { break; }
            host.advance_learned_generation(host.revision(), n.actor_revision, n.position).unwrap().unwrap();
        }
        assert_eq!(host.learned_text_message(Default::default()).unwrap().bytes(), b"OK");
        let (port, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let revision = supervisor.host().unwrap().revision();
        supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
        let mut ticket = port.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
        if cut == Cut::Cancelled { port.cancel(&ticket).unwrap(); }
        if matches!(cut, Cut::Sent | Cut::Published | Cut::Confirmed | Cut::Finished) {
            let mut host = supervisor.host_mut().unwrap();
            let (attempt, input) = dispatch(&mut host, &reviewer, 71);
            if cut != Cut::Sent {
                let revision = host.revision();
                let receipt = host.publish_checked(revision, attempt, Some(&input), snapshot(), ElapsedTick(2)).unwrap();
                assert!(matches!(receipt.outcome, EndpointOutcome::Executed { .. }));
                if matches!(cut, Cut::Confirmed | Cut::Finished) {
                    let revision = host.revision(); host.reconcile(revision, attempt).unwrap();
                }
            }
        }
        let (key, after) = if cut == Cut::Finished {
            let revision = supervisor.host().unwrap().revision();
            supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
            ticket = port.submit(72, LearnedTextRelease::Finish, ElapsedTick(100)).unwrap();
            let mut host = supervisor.host_mut().unwrap();
            let (attempt, input) = dispatch(&mut host, &reviewer, 72);
            let revision = host.revision();
            let receipt = host.publish_checked(revision, attempt, Some(&input), snapshot(), ElapsedTick(3)).unwrap();
            assert!(matches!(receipt.outcome, EndpointOutcome::Executed { .. }));
            let revision = host.revision(); host.reconcile(revision, attempt).unwrap();
            (72, Some(71))
        } else { (71, None) };
        if full {
            let mut host = supervisor.host_mut().unwrap();
            while host.journal_capacity().unwrap().remaining().events > 0 {
                let revision = host.revision(); host.observe_time(revision, ElapsedTick(4)).unwrap();
            }
            assert_eq!(host.revision(), 128);
        }
        let outcome = port.poll(&ticket);
        let host = supervisor.host().unwrap(); let state = host.inspect();
        let expected = FileRecoveryRequirements {
            guards: FileGuardSet { stream: recipe.text_stream_profile(), decoder: None, decoder_stop: None,
                source: None, identity: None, campaigns: None, credential: None },
            effective_policy: profile().delivery.policy, credential_epoch: None,
            minimum: FileRecoveryFloor { journal_revision: state.revision,
                control_sequence: state.control.sequence, authority_epoch: state.control.ledger.epoch },
        };
        drop(host); drop(supervisor); drop(port);
        std::fs::write(root.store().join("delivery.pending"), b"preserve passive staging").unwrap();
        Self { root, recipe, expected, key, after, outcome }
    }
    fn begin(&self, guarded: bool) -> Recovery {
        if guarded {
            Recovery::Guarded(FileOversight::begin_open_guarded_with_learned_generation(
                self.root.store(), profile(), &self.expected, &self.recipe).unwrap())
        } else {
            Recovery::Plain(FileOversight::begin_open_with_learned_generation(
                self.root.store(), profile(), &self.recipe).unwrap())
        }
    }
    fn ready(&self, guarded: bool) -> Recovery {
        let mut run = self.begin(guarded);
        match &mut run {
            Recovery::Plain(run) => while run.progress().status == FileLearnedRecoveryStatus::Replaying {
                run.advance(run.progress().replayed_events, 1).unwrap();
            },
            Recovery::Guarded(run) => while run.progress().status == FileLearnedRecoveryStatus::Replaying {
                run.advance(run.progress().replayed_events, 1).unwrap();
            },
        }
        run
    }
}
enum Recovery { Plain(FileLearnedRecovery), Guarded(FileGuardedLearnedRecovery) }
impl Recovery {
    fn receipt(self, key: u64, after: Option<u64>) -> Result<FileLearnedReceiptActorPort, JournalError> {
        match self { Self::Plain(run) => run.finish_receipt_port(key, after),
            Self::Guarded(run) => run.finish_receipt_port(key, after) }
    }
    fn reconcile(self, key: u64, after: Option<u64>, clock: impl FnOnce() -> ElapsedTick)
        -> Result<FileLearnedReceiptActorPort, JournalError>
    {
        match self { Self::Plain(run) => run.finish_reconciled_receipt_port(key, after, clock),
            Self::Guarded(run) => run.finish_reconciled_receipt_port(key, after, clock) }
    }
}
fn observed(port: &FileLearnedReceiptActorPort, key: u64, after: Option<u64>) -> Knowledge<ActorOutcome> {
    let release = if after.is_some() { LearnedTextRelease::Finish } else { LearnedTextRelease::Message };
    let proposal = FileLearnedTextStreamActorPort::encode_release(key, release, ElapsedTick(100)).unwrap();
    let ticket = port.submit(key, &proposal).unwrap();
    assert_eq!(port.cancel(&ticket), Err(ActorError::Withheld));
    port.poll(&ticket)
}

#[test]
fn passive_ports_preserve_actual_request_outcomes_and_need_no_journal_capacity_or_lock() {
    for cut in [Cut::Pending, Cut::Cancelled, Cut::Sent, Cut::Published, Cut::Confirmed, Cut::Finished] {
        for guarded in [false, true] {
            let history = History::new(cut, true); let before = history.root.files();
            let port = history.ready(guarded).receipt(history.key, history.after).unwrap();
            assert_eq!(observed(&port, history.key, history.after), history.outcome);
            assert_eq!(history.root.files(), before);
            // A second original exclusive cursor opens WHILE the receipt is
            // served, proving that no numerical owner/Store was retained.
            drop(history.begin(guarded));
            assert_eq!(observed(&port, history.key, history.after), history.outcome);
            assert_eq!(history.root.files(), before);
        }
    }
}

#[test]
fn missing_wrong_kind_and_unverified_selections_return_no_port_or_recovery_mutation() {
    for guarded in [false, true] {
        let history = History::new(Cut::Confirmed, false); let before = history.root.files();
        assert_eq!(history.begin(guarded).receipt(71, None).err(), Some(Error::Incomplete.into()));
        assert!(history.ready(guarded).receipt(99, None).is_err());
        assert!(history.ready(guarded).receipt(71, Some(70)).is_err());
        assert!(history.ready(guarded).reconcile(99, None, || panic!("bad selection cannot read time")).is_err());
        assert_eq!(history.root.files(), before);
        let port = history.ready(guarded).receipt(71, None).unwrap();
        assert_eq!(observed(&port, 71, None), history.outcome);
    }
}

#[test]
fn explicit_reconciliation_confirms_original_publication_before_issuing_an_immutable_port() {
    for guarded in [false, true] {
        let history = History::new(Cut::Published, false); let before = history.root.files();
        assert!(matches!(history.outcome, Knowledge::Unknown { .. }));
        assert!(history.ready(guarded).reconcile(71, None, || ElapsedTick(0)).is_err());
        assert_eq!(history.root.files(), before);
        let mut clocks = 0;
        let port = history.ready(guarded).reconcile(71, None, || { clocks += 1; ElapsedTick(3) }).unwrap();
        assert_eq!(clocks, 1);
        let outcome = observed(&port, 71, None);
        assert!(matches!(outcome, Knowledge::Known { value: ActorOutcome::Executed, .. }));
        let state = FileOversight::read_publication_with_learned_generation(history.root.store(), &profile(), &history.recipe).unwrap();
        assert_eq!(state.executions, 1); assert_eq!(state.revision, history.expected.minimum.journal_revision + 3);
        let committed = history.root.files();
        let mut wire = ActorWire::new(port);
        let proposal = FileLearnedTextStreamActorPort::encode_release(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
        for _ in 0..3 {
            assert_eq!(wire.exchange(&encode_command(&Command::Submit { request: 71, proposal: proposal.clone() }).unwrap()).result, Ok(outcome.clone()));
            assert_eq!(wire.exchange(&encode_command(&Command::Cancel { request: 71 }).unwrap()).result, Err(WireError::Withheld));
        }
        assert_eq!(history.root.files(), committed); drop(history.begin(guarded));
    }
}

#[test]
fn finish_receipts_bind_the_accepted_predecessor_but_keep_older_message_receipts_available() {
    for guarded in [false, true] {
        let history = History::new(Cut::Finished, false); let before = history.root.files();
        assert!(history.ready(guarded).receipt(72, None).is_err());
        assert!(history.ready(guarded).receipt(72, Some(70)).is_err());
        assert_eq!(history.root.files(), before);
        let finish = history.ready(guarded).receipt(72, Some(71)).unwrap();
        let message = history.ready(guarded).receipt(71, None).unwrap();
        assert!(matches!(observed(&finish, 72, Some(71)), Knowledge::Known { value: ActorOutcome::Executed, .. }));
        assert!(matches!(observed(&message, 71, None), Knowledge::Known { value: ActorOutcome::Executed, .. }));
        assert_eq!(history.root.files(), before);
    }
}

#[test]
fn guarded_floor_and_changed_canonical_cut_cannot_escape_as_a_receipt_port() {
    let mut history = History::new(Cut::Confirmed, false); let before = history.root.files();
    history.expected.minimum.journal_revision += 1;
    let mut run = match history.begin(true) { Recovery::Guarded(run) => run, _ => unreachable!() };
    assert!(run.advance(0, run.progress().total_events).is_err());
    assert!(run.finish_receipt_port(71, None).is_err());
    assert_eq!(history.root.files(), before);
    history.expected.minimum.journal_revision -= 1;
    let run = history.ready(true);
    let path = history.root.store().join("delivery.bin"); let original = std::fs::read(&path).unwrap();
    let mut changed = original.clone(); let last = changed.len() - 1; changed[last] ^= 1;
    std::fs::write(&path, &changed).unwrap();
    assert_eq!(run.receipt(71, None).err(), Some(Error::Binding.into()));
    assert_eq!(std::fs::read(&path).unwrap(), changed);
    assert_eq!(std::fs::read(history.root.store().join("delivery.pending")).unwrap(), b"preserve passive staging");
    std::fs::write(&path, original).unwrap();
    let port = history.ready(true).receipt(71, None).unwrap();
    assert_eq!(observed(&port, 71, None), history.outcome);
}
