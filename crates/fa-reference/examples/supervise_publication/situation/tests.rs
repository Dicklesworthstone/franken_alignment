//! Real canonical histories consumed by the operator; output fixtures test only
//! representation. No local test is a deployment or source-authenticity proof.
use super::*;
use fa_reference::{Snapshot, strict_json::{self, Json, Limits}};
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, VERSION};
use fa_reference::action::consequence::delivery::persistent::FileAbandonRequest;
use fa_reference::action::consequence::delivery::persistent::observed::FileHumanReviewer;
use fa_reference::action::consequence::oversight::{CommitteeInput, ReviewWindow, action_frame};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, PartKind, SubmittedPart};
use fa_reference::round::{Verdict, commitment};
use std::{collections::BTreeMap, fs, io, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Root(PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("situation operator cleanup: {error}"); }
    }
}
struct Fixture {
    host: FileOversight,
    reviewer: FileHumanReviewer,
    config: Config,
    config_file: PathBuf,
    root: Root,
}
impl Fixture {
    fn new() -> Self {
        let root = Root(crate::test_directory::create(format!("fa-sit-op-{}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))));
        let json = include_str!("../../../fixtures/supervised_publication.json")
            .replace("/operator/publication-store", root.0.join("store").to_str().unwrap());
        let config_file = root.0.join("config.json"); fs::write(&config_file, &json).unwrap();
        let config = Config::decode(json.as_bytes()).unwrap();
        let (host, reviewer) = FileOversight::create(&config.store, config.profile.clone()).unwrap();
        Self { host, reviewer, config, config_file, root }
    }
    fn args(&self, size: usize) -> Vec<String> {
        vec!["situation".into(), self.config_file.to_str().unwrap().into(), "--page-size".into(), size.to_string()]
    }
    fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fs::read_dir(&self.config.store).unwrap().map(|entry| entry.unwrap().path())
            .filter(|path| path.is_file()).map(|path| {
                let bytes = fs::read(&path).unwrap(); (path, bytes)
            }).collect()
    }
    fn clock(&mut self) { self.host.observe_time(self.host.revision(), ElapsedTick(1)).unwrap(); }
    fn propose(&mut self, id: u64, allowed: bool) -> FrozenAction {
        let snapshot = snapshot(allowed);
        self.host.propose(self.host.revision(), id, ActionSpec { version: VERSION,
            scope: self.config.profile.delivery.scope, target: Some(self.host.inspect().target),
            payload: b"secret-payload".to_vec(), required_witnesses: vec![],
            policy_epoch: self.host.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 }, snapshot).unwrap()
    }
    fn dispatch(&mut self, id: u64) {
        let action = self.propose(id, true); let contracts = &self.config.profile.committee;
        let mut views = BTreeMap::new();
        for (name, projection_id) in [("alpha", 1), ("beta", 2)] {
            let helper = &contracts.members()[name]; let mut bytes = action_frame(&action);
            let boundary = bytes.len(); bytes.extend_from_slice(helper.question()); let end = bytes.len();
            let actual = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
                SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
                SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
            ], vec![]).unwrap();
            let view = EvidenceViewManifest::new(actual, AuthorizationProjection {
                projection_id, policy_epoch: action.spec().policy_epoch, projected_originals: vec![],
            }, vec![]).unwrap(); views.insert(name.to_owned(), view);
        }
        let inputs = CommitteeInput::capture(&action, contracts, views).unwrap(); let round = 100 + id;
        self.host.record_inputs(self.host.revision(), id, 0, inputs.clone()).unwrap();
        self.host.begin_review(self.host.revision(), id, round, [9; 32], ReviewWindow {
            commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8),
        }, snapshot(true)).unwrap();
        for name in ["alpha", "beta"] {
            let digest = commitment(round, name, &[9; 32], Verdict::Allow, b"salt").unwrap();
            self.host.commit_review(self.host.revision(), round, name, digest).unwrap();
        }
        self.host.open_reveals(self.host.revision(), round).unwrap();
        for name in ["alpha", "beta"] {
            self.host.reveal_review(self.host.revision(), round, name, Verdict::Allow, b"salt".to_vec()).unwrap();
        }
        self.host.finish_review(self.host.revision(), round, Some(&inputs), snapshot(true)).unwrap().unwrap();
        let automatic = self.host.authorize(self.host.revision(), id, &inputs, snapshot(true)).unwrap();
        let request = self.host.request_human_approval(self.host.revision(), 1000 + id, id, &inputs, ElapsedTick(10)).unwrap();
        let revision = self.host.revision(); let human = self.reviewer.approve(&mut self.host, revision, &request).unwrap();
        self.host.dispatch(self.host.revision(), &automatic, &human, &action, &inputs, snapshot(true)).unwrap();
    }
    fn anchor(&self) -> PathBuf {
        let anchor = self.host.history_anchor().unwrap(); let mut bytes = Vec::new();
        anchor.write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES).unwrap();
        let path = self.root.0.join("independent-anchor.bin"); fs::write(&path, bytes).unwrap(); path
    }
}
fn snapshot(allowed: bool) -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true,
        values: BTreeMap::from([(7, if allowed { b"ok".to_vec() } else { b"bad".to_vec() })]) }
}
fn lines(bytes: &[u8]) -> Vec<Json> {
    assert!(bytes.ends_with(b"\n"));
    bytes[..bytes.len()-1].split(|byte| *byte == b'\n').map(|line| {
        strict_json::parse(line, Limits { max_bytes: render::MAX_PAGE_BYTES,
            max_depth: 8, max_items: 4096, max_string_bytes: 128 }).unwrap()
    }).collect()
}
fn knowledge(page: &Json) -> &Json { page.get("knowledge").unwrap() }
fn value(page: &Json) -> &Json { knowledge(page).get("value").unwrap() }
fn summary(page: &Json) -> &Json { value(page).get("summary").unwrap() }
fn rows(page: &Json) -> &[Json] { value(page).get("attempts").unwrap().as_array().unwrap() }
fn text<'a>(json: &'a Json, key: &str) -> &'a str { json.get(key).unwrap().as_str().unwrap() }
fn complete(json: &Json) -> bool { value(json).get("enumeration_complete").unwrap().as_bool().unwrap() }

mod command_tests;
mod output_tests;
