use super::*;
use super::super::tests::{decoder, evaluator, expected, frame, input};
use super::super::{NativeEvaluationError, NativeEvaluationStatus, NativeEvaluationWork};
use crate::round::Verdict;
use std::io::{Read, Write};

fn prompt() -> Vec<u8> {
    let mut bytes = b"\0\xff".to_vec();
    for _ in 0..80 { bytes.extend_from_slice(b"allow"); }
    bytes.push(b'?'); bytes
}
fn queued(bytes: &[u8], reveal: bool) -> (UnixStream, UnixStream) {
    let (mut parent, socket) = UnixStream::pair().unwrap();
    parent.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    parent.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
    parent.write_all(&frame(bytes, &expected())).unwrap();
    if reveal { parent.write_all(b"R").unwrap(); }
    (parent, socket)
}

#[test]
fn cooperative_native_process_deadline_stops_real_bpe_or_handoff_before_first_token() {
    let bytes = prompt();
    let salt = vec![31; 16];
    let mut original = evaluator(b"allow", 0);
    assert_eq!(original.evaluate(&input(&bytes)), Ok(Verdict::Allow));
    let full = original.progress();
    for boundary in 0..3 {
        let (mut parent, socket) = queued(&bytes, boundary == 0);
        let budget = NativeProcessBudget::new(2000, 4096).unwrap();
        let start = budget.started; let deadline = budget.deadline;
        let report = run_with_clock(socket, evaluator(b"allow", 0), salt.clone(), budget, |_, worker| {
            let p = worker.evaluation_progress();
            let expired = match boundary {
                0 => false,
                1 => p.status == NativeEvaluationStatus::Admitting && p.tokenization.merges > 0,
                _ => p.status == NativeEvaluationStatus::Running && p.work.decoder.tokens == 0,
            };
            if expired { deadline } else { start }
        }).unwrap();
        let mut output = Vec::new(); parent.read_to_end(&mut output).unwrap();
        assert_eq!(report.evaluations, 1);
        if boundary == 0 {
            assert_eq!(report.stop, NativeProcessStop::ReplySent);
            assert_eq!(report.evaluation.work, full.work);
            assert_eq!(report.evaluation.tokenization, full.tokenization);
            let packet = input(&bytes);
            assert_eq!(output, [packet.commitment_frame(Verdict::Allow, &salt).unwrap().as_slice(),
                packet.reveal_frame(Verdict::Allow, &salt).unwrap().as_slice()].concat());
        } else {
            assert_eq!(report.stop, NativeProcessStop::Deadline);
            assert_eq!(report.phase, ClientPhase::NeedsInference);
            assert_eq!(report.failure, None);
            assert_eq!(report.evaluation.work, NativeEvaluationWork::default());
            assert_eq!(report.evaluation.status, NativeEvaluationStatus::Cancelled);
            assert_eq!(report.evaluation.tokenization.input_bytes, bytes.len());
            assert!(report.evaluation.tokenization.merges > 0);
            assert!(report.evaluation.admission_steps > 0);
            assert!(output.is_empty());
            if boundary == 1 {
                assert_eq!(report.evaluation.admission_steps, 512);
                assert!(report.evaluation.tokenization.merges < full.tokenization.merges);
            } else {
                assert_eq!(report.evaluation.tokenization, full.tokenization);
                assert_eq!(report.evaluation.requested_prompt_tokens, full.requested_prompt_tokens);
            }
        }
    }
}

#[test]
fn cooperative_native_process_coincident_deadline_retains_original_tokenizer_failure() {
    let bytes = prompt();
    let (decoder, mut policy) = decoder(b"allow", 0);
    policy.tokenization.pair_lookups = 257;
    let expected = decoder.tokenizer().encode(&bytes, policy.tokenization).unwrap_err();
    let evaluator = NativeEvaluator::new(decoder, policy).unwrap();
    let (mut parent, socket) = queued(&bytes, false);
    let budget = NativeProcessBudget::new(2000, 4096).unwrap();
    let start = budget.started; let deadline = budget.deadline;
    let report = run_with_clock(socket, evaluator, vec![31; 16], budget, |_, worker| {
        if worker.failure().is_some() { deadline } else { start }
    }).unwrap();
    let mut output = Vec::new(); parent.read_to_end(&mut output).unwrap();
    assert_eq!(report.stop, NativeProcessStop::Deadline);
    let Some(NativeClientError::Inference(NativeEvaluationError::Admission(failure))) = report.failure else {
        panic!("deadline erased original tokenizer refusal: {report:?}");
    };
    assert_eq!(failure.error, expected.error);
    assert_eq!(failure.tokenization, expected.work);
    assert_eq!(report.evaluation.tokenization, expected.work);
    assert_eq!(report.evaluation.work, NativeEvaluationWork::default());
    assert_eq!(report.evaluation.status, NativeEvaluationStatus::Failed(NativeEvaluationError::Admission(failure)));
    assert_eq!(report.evaluations, 1);
    assert!(report.evaluation.admission_steps > 256);
    assert!(output.is_empty());
}
