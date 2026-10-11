use super::{assets, fixture::{frame, input, load, prompt, tokenizer}};
use fa_reference::action::consequence::oversight::helper_client::{
    ClientInterest, ClientPhase, ClientProgress,
    native::{NativeEvaluationStatus, NativeEvaluationWork, NativeEvaluator,
        peer::{NativeClientError, NativeClientProgress, NativeHelperClient}},
};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::rc::Rc;
use fa_reference::round::Verdict;

#[derive(Default)]
struct Wire {
    input: VecDeque<u8>,
    output: Vec<u8>,
    reads: usize,
    writes: usize,
}
struct Stream(Rc<RefCell<Wire>>);
impl Read for Stream {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let mut wire = self.0.borrow_mut();
        wire.reads += 1;
        if wire.input.is_empty() { return Err(io::ErrorKind::WouldBlock.into()); }
        let count = bytes.len().min(wire.input.len()).min(17);
        for byte in &mut bytes[..count] { *byte = wire.input.pop_front().unwrap(); }
        Ok(count)
    }
}
impl Write for Stream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut wire = self.0.borrow_mut();
        wire.writes += 1;
        let count = bytes.len().min(17);
        wire.output.extend_from_slice(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}
fn client(bytes: &[u8], evaluator: NativeEvaluator) -> (NativeHelperClient<Stream>, Rc<RefCell<Wire>>) {
    let wire = Rc::new(RefCell::new(Wire { input: frame(bytes).into(), ..Wire::default() }));
    let client = NativeHelperClient::new(Stream(Rc::clone(&wire)), evaluator, assets::SALT.to_vec()).unwrap();
    (client, wire)
}
fn until(client: &mut NativeHelperClient<Stream>, phase: ClientPhase) {
    for _ in 0..4096 {
        if client.phase() == phase { return; }
        client.step_cooperative().unwrap();
    }
    panic!("bounded peer did not reach {phase:?}");
}

#[test]
fn cooperative_native_peer_yields_before_prefill_and_keeps_original_commit_reveal() {
    let fixture = assets::Fixture::new(false);
    for (last, verdict) in [(b'?', Verdict::Allow), (b'!', Verdict::Deny)] {
        let bytes = prompt(last);
        let packet = input(&bytes);
        let encoded = tokenizer(&fixture).encode(&bytes, fixture.policy.tokenization).unwrap();
        let (mut worker, wire) = client(&bytes, load(&fixture, &fixture.policy));
        until(&mut worker, ClientPhase::NeedsInference);
        assert_eq!(worker.evaluations(), 0);
        assert_eq!(worker.input(), Some(&packet));
        let reads = wire.borrow().reads;
        let first = worker.step_cooperative().unwrap();
        let NativeClientProgress::Inference(first) = first else { panic!("expected original admission"); };
        assert_eq!(first.status, NativeEvaluationStatus::Admitting);
        assert_eq!(first.admission_steps, 256);
        assert_eq!(first.tokenization.pair_lookups, 256);
        assert_eq!(first.work, NativeEvaluationWork::default());
        assert_eq!(worker.interest(), ClientInterest::Inference);
        assert_eq!(worker.evaluations(), 1);

        // Mixing in the legacy pump cannot restart or synchronously drain an
        // owner already selected for cooperative admission.
        let next = worker.drive(256).unwrap();
        assert_eq!(next.steps, 1);
        assert_eq!(next.evaluations, 0);
        assert_eq!(worker.evaluation_progress().admission_steps, 512);
        assert_eq!(worker.evaluation_progress().status, NativeEvaluationStatus::Admitting);
        assert!(worker.evaluation_progress().tokenization.merges > 0);
        for _ in 0..32 {
            if worker.evaluation_progress().status == NativeEvaluationStatus::Running { break; }
            let before = worker.evaluation_progress();
            let NativeClientProgress::Inference(after) = worker.step_cooperative().unwrap() else {
                panic!("tokenization emitted an early vote");
            };
            assert!(after.admission_steps > before.admission_steps);
            assert!(after.admission_steps - before.admission_steps <= 256);
            assert_eq!(after.work, NativeEvaluationWork::default());
            assert!(worker.report().is_none());
        }
        let admitted = worker.evaluation_progress();
        assert_eq!(admitted.status, NativeEvaluationStatus::Running);
        assert_eq!(admitted.work, NativeEvaluationWork::default());
        assert_eq!(admitted.tokenization, encoded.work());
        assert_eq!(wire.borrow().reads, reads);
        assert_eq!(wire.borrow().writes, 0);
        assert!(wire.borrow().output.is_empty());

        until(&mut worker, ClientPhase::SendingCommitment);
        assert_eq!(worker.evaluation_progress().status, NativeEvaluationStatus::Judged(verdict));
        assert_eq!(worker.evaluation_progress().work.decoder.tokens, encoded.tokens().len() as u64 + 2);
        assert_eq!(worker.sampled_draws(), 2);
        assert_eq!(worker.evaluations(), 1);
        let report = worker.report().unwrap();
        assert_eq!(report.prompt().source(), bytes);
        assert_eq!(report.prompt().tokens(), encoded.tokens());
        assert_eq!(report.prompt().spans(), encoded.spans());
        assert_eq!(report.prompt().work(), encoded.work());
        assert!(wire.borrow().output.is_empty());
        until(&mut worker, ClientPhase::AwaitingReveal);
        let commitment = packet.commitment_frame(verdict, assets::SALT).unwrap();
        let reveal = packet.reveal_frame(verdict, assets::SALT).unwrap();
        assert_eq!(wire.borrow().output, commitment);
        let completed = worker.evaluation_progress();
        assert_eq!(worker.step_cooperative(), Ok(NativeClientProgress::Protocol(ClientProgress::Blocked)));
        assert_eq!(worker.evaluation_progress(), completed);
        assert_eq!(wire.borrow().output, commitment);
        wire.borrow_mut().input.push_back(b'R');
        until(&mut worker, ClientPhase::ReplySent);
        assert_eq!(wire.borrow().output, [commitment.as_slice(), reveal.as_slice()].concat());
        assert_eq!(worker.evaluation_progress(), completed);
        assert!(!worker.cancel());
    }
}

#[test]
fn cooperative_native_peer_cancel_retains_real_merges_without_io_or_reroll() {
    let fixture = assets::Fixture::new(false);
    let bytes = prompt(b'?');
    let (mut worker, wire) = client(&bytes, load(&fixture, &fixture.policy));
    until(&mut worker, ClientPhase::NeedsInference);
    worker.step_cooperative().unwrap();
    worker.step_cooperative().unwrap();
    let before = worker.evaluation_progress();
    assert_eq!(before.status, NativeEvaluationStatus::Admitting);
    assert!(before.tokenization.merges > 0);
    assert_eq!(before.work, NativeEvaluationWork::default());
    let reads = wire.borrow().reads;
    assert!(worker.cancel());
    assert!(!worker.cancel());
    let cancelled = worker.evaluation_progress();
    assert_eq!(cancelled.status, NativeEvaluationStatus::Cancelled);
    assert_eq!(cancelled.tokenization, before.tokenization);
    assert_eq!(cancelled.admission_steps, before.admission_steps);
    wire.borrow_mut().input.push_back(b'R');
    assert_eq!(worker.step_cooperative(), Err(NativeClientError::Cancelled));
    assert_eq!(worker.step(), Err(NativeClientError::Cancelled));
    assert_eq!(worker.drive(256).unwrap().steps, 0);
    assert_eq!(worker.evaluations(), 1);
    assert_eq!(worker.evaluation_progress(), cancelled);
    assert_eq!(wire.borrow().reads, reads);
    assert_eq!(wire.borrow().writes, 0);
    assert!(wire.borrow().output.is_empty());
    assert!(worker.report().is_none());

    // The same complete request and independently loaded model do finish when
    // the owner is allowed to continue.
    let (mut permitted, _) = client(&bytes, load(&fixture, &fixture.policy));
    until(&mut permitted, ClientPhase::SendingCommitment);
    assert_eq!(permitted.evaluation_progress().status, NativeEvaluationStatus::Judged(Verdict::Allow));
}

#[test]
fn synchronous_native_peer_entry_point_keeps_its_original_first_token_sequence() {
    let fixture = assets::Fixture::new(false);
    let (mut worker, _) = client(&prompt(b'?'), load(&fixture, &fixture.policy));
    until(&mut worker, ClientPhase::NeedsInference);
    let NativeClientProgress::Inference(first) = worker.step().unwrap() else {
        panic!("expected original first native token");
    };
    assert_eq!(first.status, NativeEvaluationStatus::Running);
    assert_eq!(first.admission_steps, 0);
    assert_eq!(first.work.decoder.tokens, 1);
    assert_eq!(first.reviewed_prompt_tokens, 1);
    assert_eq!(first.tokenization.merges, 320);
    let second = worker.drive(256).unwrap();
    assert_eq!(second.steps, 1);
    assert_eq!(worker.evaluation_progress().work.decoder.tokens, 2);
    assert_eq!(worker.evaluation_progress().admission_steps, 0);
}
