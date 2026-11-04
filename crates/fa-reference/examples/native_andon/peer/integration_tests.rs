//! Real native source capture, applied congress case, peer socket and file journal.
//! Helper verdicts/independent labels are fixtures, not model-accuracy evidence.
use super::*;
use super::super::receipt;
use fa_reference::action::{ActionSpec, ElapsedTick, VERSION};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight,
    guarded::{FileRecoveryFloor, anchored::{FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES},
        evaluation::andon::FileNativeAndonSnapshot}};
use fa_reference::action::consequence::oversight::{ReviewWindow,
    evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource}};
use fa_reference::round::{Verdict, commitment};
use fa_reference::Snapshot;
use std::{io::Cursor, thread::JoinHandle};

struct Case {
    root: Directory,
    config: Config,
    settings: Settings,
    minimum: FileRecoveryFloor,
    label: Vec<u8>,
}
impl Case {
    fn new() -> Self {
        let root = Directory::new(); let config = config(&root); let settings = settings();
        let (mut host, _) = settings.create(&config, None).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let evidence = EvidenceSnapshot::new(EvidenceIdentity { source: 51, generation: 1,
            scope: config.profile.delivery.scope }, Snapshot { semantic_epoch: 1, complete: true,
            values: BTreeMap::from([(7, b"ok".to_vec())]) }, ["alpha", "beta"].into_iter()
            .map(|name| (name.into(), b"actual captured fixture".to_vec())).collect()).unwrap();
        let source_path = root.0.join("evidence.json"); fs::write(&source_path, evidence.encode()).unwrap();
        let mut source = FileEvidenceSource::new(source_path.clone(), 51, config.profile.delivery.scope, 1048576).unwrap();
        host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1)).unwrap();
        let action = host.propose(host.revision(), 1, ActionSpec { version: VERSION,
            scope: config.profile.delivery.scope, target: Some(host.inspect().target),
            payload: b"reviewed".to_vec(), required_witnesses: Vec::new(), units: 16,
            policy_epoch: 0, deadline: ElapsedTick(100) }, evidence.snapshot().clone()).unwrap();
        let inputs = evidence.inputs_for(&action, &config.profile.committee).unwrap();
        host.record_inputs(host.revision(), 1, 0, inputs.clone()).unwrap();
        let root_id = evidence.reference_root();
        host.begin_review(host.revision(), 1, 101, root_id, ReviewWindow {
            commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) }, evidence.snapshot().clone()).unwrap();
        for (name, verdict) in [("alpha", Verdict::Hold), ("beta", Verdict::Allow)] {
            host.commit_review(host.revision(), 101, name, commitment(101, name, &root_id, verdict, b"salt").unwrap()).unwrap();
        }
        host.open_reveals(host.revision(), 101).unwrap();
        for (name, verdict) in [("alpha", Verdict::Hold), ("beta", Verdict::Allow)] {
            host.reveal_review(host.revision(), 101, name, verdict, b"salt".to_vec()).unwrap();
        }
        host.finish_review(host.revision(), 101, Some(&inputs), evidence.snapshot().clone()).unwrap().unwrap();
        let ticket = host.evaluation_ticket(101).unwrap(); let case = ticket.case(); assert!(case.stopped);
        let root_hex: String = case.evidence_root.iter().map(|byte| format!("{byte:02x}")).collect();
        let label = format!(r#"{{"schema_version":"fa.native-assessment/1","round":{},"attempt":{},"policy_generation":{},"congress_generation":{},"case_evidence_root_hex":"{root_hex}","expected_evaluation_revision":{},"origin":1,"evidence_id_hex":"0202020202020202020202020202020202020202020202020202020202020202","truth":"benign"}}"#,
            case.round, case.attempt, case.policy_generation, case.congress_generation, host.credibility_report().unwrap().revision).into_bytes();
        let state = host.inspect();
        let minimum = FileRecoveryFloor { journal_revision: state.revision,
            control_sequence: state.control.sequence, authority_epoch: state.control.ledger.epoch };
        host.history_anchor().unwrap().write_to(&mut fs::File::create(root.0.join("initial.anchor")).unwrap(), MAX_HISTORY_ANCHOR_BYTES).unwrap();
        drop(host); fs::remove_file(source_path).unwrap(); // Later commands are offline.
        let template = std::str::from_utf8(include_bytes!("../../../fixtures/supervised_publication.json")).unwrap();
        fs::write(root.0.join("config.json"), template.replace("/operator/publication-store", config.store.to_str().unwrap())).unwrap();
        fs::write(root.0.join("settings.json"), include_bytes!("../../../fixtures/joint_native_andon_operator.json")).unwrap();
        let peer = String::from_utf8(profile_json(&root, &config, &settings, Some(std::process::id()))).unwrap()
            .replace("\"runtime_ms\":1000", "\"runtime_ms\":15000");
        fs::write(root.0.join("peer.json"), peer).unwrap();
        fs::write(root.0.join("label.json"), &label).unwrap();
        Self { root, config, settings, minimum, label }
    }
    fn bytes(&self) -> Vec<u8> { fs::read(self.config.store.join("delivery.bin")).unwrap() }
    fn state(&self, anchor: &str) -> FileNativeAndonSnapshot {
        let anchor = FileHistoryAnchor::read_trusted(&mut Cursor::new(fs::read(self.root.0.join(anchor)).unwrap()), MAX_HISTORY_ANCHOR_BYTES).unwrap();
        FileOversight::read_andon_guarded(&self.config.store, &self.config.profile,
            &self.settings.requirements(&self.config, self.minimum).unwrap(), &self.settings.protocol, &anchor).unwrap()
    }
    fn start(&self, input: &str, output: &str) -> JoinHandle<(Result<(), String>, Vec<u8>)> {
        let m = self.minimum;
        fs::write(self.root.0.join("resume.json"), format!(r#"{{"schema_version":"fa.guarded-resume/1","minimum":{{"journal_revision":{},"control_sequence":{},"authority_epoch":{}}},"history_anchor":{:?},"retain_history_anchor":{:?}}}"#,
            m.journal_revision, m.control_sequence, m.authority_epoch, self.root.0.join(input), self.root.0.join(output))).unwrap();
        let root = self.root.0.clone();
        let worker = std::thread::spawn(move || {
            let mut report = Vec::new();
            let result = serve(&root.join("config.json"), &root.join("settings.json"),
                &root.join("resume.json"), &root.join("peer.json"), &mut report);
            (result, report)
        });
        let started = Instant::now();
        while !self.root.0.join("eval.sock").exists() {
            assert!(started.elapsed() < Duration::from_secs(10));
            std::thread::sleep(Duration::from_millis(1));
        }
        worker
    }
}

#[test]
fn real_peer_assessment_retries_and_conflicts_preserve_original_accounting() {
    let case = Case::new();
    let worker = case.start("initial.anchor", "first.anchor");
    let mut reply = Vec::new();
    send(&case.root.0.join("peer.json"), &case.root.0.join("label.json"), &mut reply).unwrap();
    assert!(worker.join().unwrap().0.is_ok());
    let first = case.state("first.anchor");
    assert_eq!(first.accounting.windows()[&1].false_stops, 1);
    assert_eq!(first.credibility.report.pending_cases, 0); assert_eq!(first.receipts.len(), 1);
    let worker = case.start("first.anchor", "retry.anchor");
    send(&case.root.0.join("peer.json"), &case.root.0.join("label.json"), &mut Vec::new()).unwrap();
    assert!(worker.join().unwrap().0.is_ok());
    let retry = case.state("retry.anchor");
    assert_eq!(retry.accounting, first.accounting); assert_eq!(retry.receipts, first.receipts);
    assert_eq!(retry.credibility.journal.revision, first.credibility.journal.revision + 1);
    let original = strict_json::parse(&case.label, Limits::default()).unwrap();
    let before_revision = original.get("expected_evaluation_revision").unwrap().as_u64().unwrap();
    let changed = String::from_utf8(case.label.clone()).unwrap().replace("\"origin\":1", "\"origin\":2")
        .replace(&format!("\"expected_evaluation_revision\":{before_revision}"),
            &format!("\"expected_evaluation_revision\":{}", retry.credibility.report.revision));
    fs::write(case.root.0.join("label.json"), changed).unwrap();
    let worker = case.start("retry.anchor", "refusal.anchor");
    let mut reply = Vec::new();
    assert!(send(&case.root.0.join("peer.json"), &case.root.0.join("label.json"), &mut reply).unwrap_err().contains("refused"));
    assert!(worker.join().unwrap().0.is_err());
    let refused = case.state("refusal.anchor");
    assert_eq!(refused.accounting, first.accounting); assert_eq!(refused.receipts, first.receipts);
    assert_eq!(refused.credibility.journal.executions, 0);
}

#[test]
fn wrong_audience_on_a_verified_socket_cannot_fence_or_assess_the_other_domain() {
    let case = Case::new(); let before = case.bytes();
    let pending = case.config.store.join("delivery.pending"); fs::write(&pending, b"keep staging").unwrap();
    let peer = fs::read_to_string(case.root.0.join("peer.json")).unwrap()
        .replace("\"evaluator\":202", "\"evaluator\":203");
    fs::write(case.root.0.join("wrong-peer.json"), peer).unwrap();
    let worker = case.start("initial.anchor", "forbidden.anchor");
    assert!(send(&case.root.0.join("wrong-peer.json"), &case.root.0.join("label.json"), &mut Vec::new()).is_err());
    assert!(worker.join().unwrap().0.unwrap_err().contains("audience mismatch"));
    assert_eq!(case.bytes(), before); assert_eq!(fs::read(pending).unwrap(), b"keep staging");
    assert!(!case.root.0.join("forbidden.anchor").exists());
}

#[test]
fn lost_peer_reply_never_undoes_native_label_charge_or_successor_custody() {
    let case = Case::new(); let worker = case.start("initial.anchor", "retained.anchor");
    let peer = EvaluatorPeerProfile::read(&case.root.0.join("peer.json")).unwrap();
    let request = receipt::Request::prepare(&peer, &case.label).unwrap();
    let mut socket = UnixStream::connect(&peer.socket).unwrap();
    peer.supervisor.policy().unwrap().verify(&socket).unwrap();
    socket.shutdown(Shutdown::Read).unwrap();
    socket.write_all(request.wire()).unwrap(); socket.shutdown(Shutdown::Write).unwrap(); drop(socket);
    let (result, report) = worker.join().unwrap(); assert!(result.is_err());
    let text = String::from_utf8(report).unwrap();
    assert!(text.contains("\"native_completed\":true")); assert!(text.contains("\"reply_written\":false"));
    let state = case.state("retained.anchor"); assert_eq!(state.accounting.windows()[&1].false_stops, 1);
    assert_eq!(state.receipts.len(), 1); assert_eq!(state.credibility.journal.executions, 0);
}
