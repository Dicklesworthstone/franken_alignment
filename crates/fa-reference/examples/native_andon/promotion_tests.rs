//! Real native source capture, joint replay, promotion, recovery and operator I/O.
use super::*;
use super::super::{Mutation, run};
use super::super::super::{Settings, command_with_elapsed};
use crate::{config::Config, profile::ResumeProfile};
use fa_reference::action::{ActionSpec, VERSION};
use fa_reference::action::consequence::delivery::persistent::{RecoveryReserve,
    observed::{credibility::FileIndependentEvaluator,
        guarded::evaluation::andon::FileNativeAndonSnapshot}};
use fa_reference::action::consequence::oversight::{ReviewWindow,
    credibility::{Assessment, GroundTruth},
    evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource}};
use fa_reference::round::{Verdict, commitment};
use fa_reference::{Snapshot, strict_json::{self, Limits}};
use std::{cell::Cell, collections::BTreeMap, fs, io, path::PathBuf,
    sync::atomic::{AtomicU64, Ordering}, time::Duration};

const SETTINGS: &[u8] = include_bytes!("../../fixtures/joint_native_andon_operator.json");
const LEGACY: &[u8] = include_bytes!("../../fixtures/native_andon_operator.json");
const CONFIG: &str = include_str!("../../fixtures/supervised_publication.json");
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-joint-promotion-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
    fn config(&self, hold: u64) -> Config {
        let text = CONFIG.replace("/operator/publication-store", self.0.join("store").to_str().unwrap())
            .replace("\"weight\": 5", "\"weight\": 10")
            .replace("\"per_member_cap\": 5", "\"per_member_cap\": 10")
            .replace("\"per_cohort_cap\": 5", "\"per_cohort_cap\": 10")
            .replace("\"continue_minimum\": 8", "\"continue_minimum\": 5")
            .replace("\"continue_hold_maximum\": 0", &format!("\"continue_hold_maximum\": {hold}"))
            .replace("\"narrow_at\": 8", "\"narrow_at\": 20")
            .replace("\"suspend_at\": 10", "\"suspend_at\": 30");
        fs::write(self.0.join("config.json"), &text).unwrap(); Config::decode(text.as_bytes()).unwrap()
    }
    fn anchor(&self) -> PathBuf { self.0.join("anchor") }
    fn save(&self, h: &FileOversight) {
        let mut out = fs::File::create(self.anchor()).unwrap();
        h.history_anchor().unwrap().write_to(&mut out,
            fa_reference::action::consequence::delivery::persistent::observed::guarded::anchored::MAX_HISTORY_ANCHOR_BYTES).unwrap();
    }
    fn resume(&self, output: Option<&str>) -> PathBuf {
        let path = self.0.join("resume.json");
        let output = output.map(|name| format!(",\"retain_history_anchor\":\"{}\"", self.0.join(name).display())).unwrap_or_default();
        fs::write(&path, format!("{{\"schema_version\":\"fa.guarded-resume/1\",\"minimum\":{{\"journal_revision\":0,\"control_sequence\":0,\"authority_epoch\":0}},\"history_anchor\":\"{}\"{output}}}", self.anchor().display())).unwrap(); path
    }
    fn bytes(&self) -> Vec<u8> { fs::read(self.0.join("store/delivery.bin")).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(e) = fs::remove_dir_all(&self.0) { eprintln!("joint operator cleanup: {e}"); } }
}
fn state(root: &Directory, c: &Config, s: &Settings) -> FileNativeAndonSnapshot {
    let r = ResumeProfile::read(&root.resume(None)).unwrap(); let custody = r.admit(c).unwrap();
    FileOversight::read_andon_guarded(&c.store, &c.profile, &s.requirements(c, r.minimum()).unwrap(),
        &s.protocol, custody.anchor()).unwrap()
}
fn observe_case(h: &mut FileOversight, c: &Config, evidence: &EvidenceSnapshot,
    e: &FileIndependentEvaluator, id: u64, a: Verdict, b: Verdict, truth: GroundTruth)
{
    let action = h.propose(h.revision(), id, ActionSpec { version: VERSION, scope: c.profile.delivery.scope,
        target: Some(h.inspect().target), payload: b"reviewed".to_vec(), required_witnesses: Vec::new(),
        units: 16, policy_epoch: h.inspect().control.ledger.epoch, deadline: ElapsedTick(100) }, evidence.snapshot().clone()).unwrap();
    let input = evidence.inputs_for(&action, &c.profile.committee).unwrap();
    h.record_inputs(h.revision(), id, 0, input.clone()).unwrap(); let round = 100 + id; let root = evidence.reference_root();
    h.begin_review(h.revision(), id, round, root, ReviewWindow { commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) }, evidence.snapshot().clone()).unwrap();
    for (name, verdict) in [("alpha", a), ("beta", b)] {
        h.commit_review(h.revision(), round, name, commitment(round, name, &root, verdict, b"s").unwrap()).unwrap();
    }
    h.open_reveals(h.revision(), round).unwrap();
    for (name, verdict) in [("alpha", a), ("beta", b)] { h.reveal_review(h.revision(), round, name, verdict, b"s".to_vec()).unwrap(); }
    h.finish_review(h.revision(), round, Some(&input), evidence.snapshot().clone()).unwrap().unwrap();
    let ticket = h.evaluation_ticket(round).unwrap(); let revision = h.revision();
    e.assess(h, revision, &ticket, Assessment { origin: id, evidence_id: [17; 32], truth }).unwrap();
}
fn prepared(hold: u64) -> (Directory, Config, Settings, FileCredibilityUpdate) {
    let root = Directory::new(); let c = root.config(hold); let s = Settings::decode(SETTINGS).unwrap();
    let (mut h, roles) = s.create(&c, Some(RecoveryReserve::terminal())).unwrap();
    h.observe_time(h.revision(), ElapsedTick(1)).unwrap();
    let evidence = EvidenceSnapshot::new(EvidenceIdentity { source: c.source_policy.source.source,
        generation: 1, scope: c.profile.delivery.scope }, Snapshot { semantic_epoch: 1, complete: true,
        values: BTreeMap::from([(7, b"ok".to_vec())]) }, ["alpha", "beta"].into_iter()
        .map(|name| (name.to_owned(), b"original context".to_vec())).collect()).unwrap();
    let path = root.0.join("evidence.json"); fs::write(&path, evidence.encode()).unwrap();
    let mut source = FileEvidenceSource::new(path.clone(), c.source_policy.source.source,
        c.profile.delivery.scope, 1048576).unwrap();
    h.refresh_file_source(h.revision(), &mut source, ElapsedTick(1)).unwrap();
    for (id, a, b, truth) in [(1, Verdict::Hold, Verdict::Allow, GroundTruth::Violation),
        (2, Verdict::Allow, Verdict::Hold, GroundTruth::Violation),
        (3, Verdict::Allow, Verdict::Allow, GroundTruth::Benign),
        (4, Verdict::Hold, Verdict::Allow, GroundTruth::Benign)] {
        observe_case(&mut h, &c, &evidence, &roles.evaluator, id, a, b, truth);
    }
    assert!(h.credibility_report().unwrap().qualified());
    assert_eq!(h.andon_ledger().unwrap().windows()[&1].false_stops, 1);
    root.save(&h); let control = h.inspect().control;
    // Explicit request for the NEXT original recovery fence, not a request
    // rewritten by the command. Time observation does not change this sequence.
    let request = FileCredibilityUpdate { operation: 71, expected_control_sequence: control.sequence,
        expected_authority_epoch: control.ledger.epoch + 1,
        expected_evaluation_revision: h.credibility_report().unwrap().revision };
    drop(h); fs::remove_file(path).unwrap(); (root, c, s, request)
}
fn document(r: &FileCredibilityUpdate) -> Vec<u8> {
    format!("{{\"schema_version\":\"fa.native-andon-promotion/1\",\"operation\":{},\"expected_control_sequence\":{},\"expected_authority_epoch\":{},\"expected_evaluation_revision\":{}}}",
        r.operation, r.expected_control_sequence, r.expected_authority_epoch, r.expected_evaluation_revision).into_bytes()
}
fn invoke(root: &Directory, c: &Config, s: &Settings, r: &FileCredibilityUpdate, output: &str,
    clock: &mut impl FnMut() -> Result<ElapsedTick, String>, out: &mut impl Write) -> Result<(), String>
{
    let mutation = Mutation::decode("promote", &document(r)).unwrap();
    run(c, s, &root.resume(Some(output)), &mutation, &mut || Ok(()), clock, out)
}
fn valid(bytes: &[u8]) {
    strict_json::parse(bytes, Limits { max_bytes: 65536, max_depth: 8, max_items: 4096, max_string_bytes: 4096 }).unwrap();
}

#[test]
fn shared_profile_selects_exact_joint_bootstrap_and_rejects_legacy_downgrade() {
    for reserved in [false, true] {
        let root = Directory::new(); let c = root.config(0); let s = Settings::decode(SETTINGS).unwrap();
        let (h, _) = s.create(&c, reserved.then_some(RecoveryReserve::terminal())).unwrap();
        root.save(&h); assert_eq!(h.joint_credibility_policy().unwrap(), s.joint);
        assert_eq!(h.journal_capacity().unwrap().reserve(), reserved.then_some(RecoveryReserve::terminal()));
        let request = ResumeProfile::read(&root.resume(None)).unwrap();
        let mut marginal = Settings::decode(SETTINGS).unwrap(); marginal.joint = None;
        assert!(FileOversight::read_andon_guarded(&c.store, &c.profile,
            &marginal.requirements(&c, request.minimum()).unwrap(), &s.protocol, &h.history_anchor().unwrap()).is_err());
        assert_eq!(state(&root, &c, &s).accounting, *h.andon_ledger().unwrap());
    }
    let text = std::str::from_utf8(SETTINGS).unwrap();
    for wrong in [text.replace("operator/2", "operator/1"), text.replace("\"id\": 71", "\"id\": 0"),
        text.replace("\"cases\": 512", "\"cases\": 513"), text.replace("\"generation\": 1", "\"extra\": 0, \"generation\": 1")] {
        assert!(Settings::decode(wrong.as_bytes()).is_err());
    }
    assert!(Settings::decode(LEGACY).unwrap().joint.is_none());
}

#[test]
fn original_promotion_and_exact_retry_preserve_native_incidents_and_receipts_offline() {
    let (root, c, s, request) = prepared(0); let before = state(&root, &c, &s); let mut out = Vec::new();
    invoke(&root, &c, &s, &request, "first", &mut || Ok(ElapsedTick(2)), &mut out).unwrap(); valid(&out);
    assert!(String::from_utf8(out).unwrap().contains("\"joint_qualified\":true"));
    let after = state(&root, &c, &s); assert_eq!(after.accounting, before.accounting); assert_eq!(after.receipts, before.receipts);
    assert_eq!(after.credibility.promotions.len(), 1);
    assert_eq!(after.credibility.promotions[0].change.current.members["alpha"].weight, 3);
    assert_eq!(after.credibility.promotions[0].change.current.members["beta"].weight, 5);
    assert_eq!(after.credibility.journal.revision, before.credibility.journal.revision + 3);
    invoke(&root, &c, &s, &request, "repeat", &mut || panic!("exact promotion sampled time"), &mut Vec::new()).unwrap();
    let repeated = state(&root, &c, &s);
    assert_eq!(repeated.credibility.promotions, after.credibility.promotions);
    assert_eq!(repeated.credibility.journal.revision, after.credibility.journal.revision + 1);
    assert_eq!(repeated.accounting, before.accounting); assert_eq!(repeated.credibility.journal.executions, 0);
    assert!(root.0.join("first").is_file()); assert!(root.0.join("repeat").is_file());
}

#[test]
fn common_mode_escape_refuses_before_time_and_preserves_the_acknowledged_recovery_cut() {
    let (root, c, s, request) = prepared(5); let before = state(&root, &c, &s); let mut out = Vec::new();
    let error = invoke(&root, &c, &s, &request, "refused", &mut || panic!("unsafe candidate read time"), &mut out).unwrap_err();
    assert!(error.contains("RegressedOrigin")); valid(&out);
    let after = state(&root, &c, &s); assert!(after.credibility.promotions.is_empty());
    assert_eq!(after.credibility.journal.revision, before.credibility.journal.revision + 1);
    assert_eq!(after.accounting, before.accounting); assert!(root.0.join("refused").is_file());
}

#[test]
fn stale_requests_and_marginal_only_intent_cannot_reach_promotion_or_a_clock() {
    for field in 0..3 {
        let (root, c, s, mut request) = prepared(0);
        match field { 0 => request.expected_authority_epoch += 1, 1 => request.expected_control_sequence += 1,
            _ => request.expected_evaluation_revision += 1 }
        assert!(invoke(&root, &c, &s, &request, "stale", &mut || panic!("stale request read time"), &mut Vec::new()).is_err());
        assert!(state(&root, &c, &s).credibility.promotions.is_empty());
    }
    let (root, c, mut s, request) = prepared(0); s.joint = None;
    let before = root.bytes();
    assert!(invoke(&root, &c, &s, &request, "no-joint", &mut || panic!("unselected joint"), &mut Vec::new()).is_err());
    assert_eq!(root.bytes(), before); assert!(!root.0.join("no-joint").exists());
}

#[test]
fn unavailable_or_slow_clock_cannot_promote_but_retains_acknowledged_recovery() {
    for slow in [false, true] {
        let (root, c, s, request) = prepared(0); let before = state(&root, &c, &s);
        let expired = Cell::new(false); let mutation = Mutation::Promote(request);
        let result = run(&c, &s, &root.resume(Some("clock-failure")), &mutation,
            &mut || if expired.get() { Err("deadline".into()) } else { Ok(()) },
            &mut || if slow { expired.set(true); Ok(ElapsedTick(2)) } else { Err("clock unavailable".into()) },
            &mut Vec::new());
        assert!(result.is_err()); let after = state(&root, &c, &s);
        assert_eq!(after.accounting, before.accounting); assert!(after.credibility.promotions.is_empty());
        assert_eq!(after.credibility.journal.revision, before.credibility.journal.revision + 1);
        assert!(root.0.join("clock-failure").is_file());
    }
}

struct Broken(bool);
impl Write for Broken {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> { if self.0 { Ok(bytes.len()) } else { Err(io::ErrorKind::BrokenPipe.into()) } }
    fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
}
#[test]
fn output_failure_does_not_undo_reweighting_or_replace_the_retained_anchor() {
    for flush in [false, true] {
        let (root, c, s, request) = prepared(0);
        assert!(invoke(&root, &c, &s, &request, "committed", &mut || Ok(ElapsedTick(2)), &mut Broken(flush)).is_err());
        let original = fs::read(root.0.join("committed")).unwrap();
        assert_eq!(state(&root, &c, &s).credibility.promotions.len(), 1);
        invoke(&root, &c, &s, &request, "repeated", &mut || panic!("retry sampled clock"), &mut Vec::new()).unwrap();
        assert_eq!(fs::read(root.0.join("committed")).unwrap(), original);
        assert_eq!(state(&root, &c, &s).credibility.promotions.len(), 1);
    }
}

#[test]
fn promotion_schema_is_closed_and_actual_command_routes_to_original_native_operation() {
    let (root, c, s, request) = prepared(0); let bytes = document(&request);
    let text = std::str::from_utf8(&bytes).unwrap();
    for wrong in [text.replace("\"operation\":71", "\"operation\":0"),
        text.replace("\"operation\":71", "\"weights\":{},\"operation\":71"),
        text.replace("promotion/1", "assessment/1")] { assert!(Mutation::decode("promote", wrong.as_bytes()).is_err()); }
    let settings = root.0.join("joint.json"); let mutation = root.0.join("promotion.json");
    fs::write(&settings, SETTINGS).unwrap(); fs::write(&mutation, bytes).unwrap();
    let resume = root.resume(Some("command"));
    let args = vec!["promote".into(), root.0.join("config.json").to_str().unwrap().into(),
        settings.to_str().unwrap().into(), resume.to_str().unwrap().into(), mutation.to_str().unwrap().into()];
    let mut extra = args.clone(); extra.push("--weights".into());
    assert!(command_with_elapsed(&extra, &mut || Duration::ZERO, &mut Vec::new()).is_err());
    let mut out = Vec::new(); command_with_elapsed(&args, &mut || Duration::ZERO, &mut out).unwrap(); valid(&out);
    assert_eq!(state(&root, &c, &s).credibility.promotions.len(), 1);
    assert!(root.0.join("command").is_file());
}
