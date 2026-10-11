use super::*;
use super::super::tests::{decoder, evaluator, input};
use crate::action::consequence::activation::monitor::decoder::observation::DecoderAvailability;
use crate::round::Verdict;

fn prompt() -> Vec<u8> {
    let mut bytes = b"\0\xff".to_vec();
    for _ in 0..80 { bytes.extend_from_slice(b"allow"); }
    bytes.push(b'?');
    bytes
}

#[test]
fn cooperative_native_cancel_or_unwind_during_bpe_closes_the_original_observer() {
    let packet = input(&prompt());
    let mut permitted = evaluator(b"allow", 0);
    assert_eq!(permitted.evaluate(&packet), Ok(Verdict::Allow));
    for interrupted in [false, true] {
        let (decoder, policy) = decoder(b"allow", 0);
        let observer = decoder.observation();
        let mut worker = NativeEvaluator::new(decoder, policy).unwrap();
        worker.begin_cooperative(&packet).unwrap();
        for _ in 0..2048 {
            if worker.progress().tokenization.merges > 0 { break; }
            worker.advance(0).unwrap();
        }
        assert!(worker.progress().tokenization.merges > 0);
        assert_eq!(worker.status(), NativeEvaluationStatus::Admitting);
        let before = worker.progress();
        if interrupted {
            assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                worker.advance_with(0, || panic!("after original BPE work, before native handoff"))
            })).is_err());
            assert_eq!(worker.status(), NativeEvaluationStatus::Evaluating);
            assert_eq!(worker.progress().admission_steps, before.admission_steps + 1);
            assert!(worker.progress().tokenization.heap_pops > before.tokenization.heap_pops);
        }
        let retained = worker.progress();
        assert_eq!(retained.work, NativeEvaluationWork::default());
        assert!(worker.cancel());
        assert_eq!(observer.availability(), DecoderAvailability::Closed);
        assert!(!worker.cancel());
        assert_eq!(worker.progress().tokenization, retained.tokenization);
        assert_eq!(worker.progress().admission_steps, retained.admission_steps);
        assert_eq!(worker.progress().work, retained.work);
        assert_eq!(worker.status(), if interrupted { NativeEvaluationStatus::Evaluating }
            else { NativeEvaluationStatus::Cancelled });
        let closed = worker.progress();
        assert!(worker.advance(0).is_err());
        assert!(worker.begin_cooperative(&packet).is_err());
        assert!(worker.evaluate(&packet).is_err());
        assert_eq!(worker.progress(), closed);
        assert!(worker.report().is_none());
    }
}

#[test]
fn cooperative_native_admission_cannot_bypass_held_terminal_or_complete_output_schema() {
    let packet = input(b"\0raw\xff?");
    for (word, alarm, expected) in [
        (b"allow".as_slice(), 0, Ok(Verdict::Allow)),
        (b"allow", 2, Err(NativeEvaluationError::Incomplete(GenerationFinish::Held))),
        (b"allow\n", 0, Err(NativeEvaluationError::InvalidVerdict)),
    ] {
        let mut original = evaluator(word, alarm);
        assert_eq!(original.evaluate(&packet), expected);
        let mut cooperative = evaluator(word, alarm);
        cooperative.begin_cooperative(&packet).unwrap();
        let mut result = None;
        for _ in 0..128 {
            match cooperative.advance(cooperative.position()) {
                Ok(progress) => {
                    if let NativeEvaluationStatus::Judged(verdict) = progress.status {
                        result = Some(Ok(verdict)); break;
                    }
                    assert!(cooperative.report().is_none());
                }
                Err(error) => { result = Some(Err(error)); break; }
            }
        }
        assert_eq!(result, Some(expected));
        assert_eq!(cooperative.work(), original.work());
        assert_eq!(cooperative.progress().tokenization, original.progress().tokenization);
        assert_eq!(cooperative.report().unwrap().prompt().source(), packet.actual_input().submitted_bytes());
        assert_eq!(cooperative.report().unwrap().bytes(), original.report().unwrap().bytes());
        assert_eq!(cooperative.report().unwrap().generation().finish(), original.report().unwrap().generation().finish());
        let retained = cooperative.progress();
        cooperative.cancel();
        assert_eq!(cooperative.progress(), retained);
    }
}
