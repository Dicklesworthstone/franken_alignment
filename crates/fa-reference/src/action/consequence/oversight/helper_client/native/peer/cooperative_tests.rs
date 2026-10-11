use super::*;
use super::super::tests::{evaluator, expected, frame};
use std::cell::Cell;
use std::io::{self, Cursor};
use std::rc::Rc;

struct Stream { input: Cursor<Vec<u8>>, writes: Rc<Cell<usize>> }
impl Read for Stream {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> { self.input.read(bytes) }
}
impl Write for Stream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writes.set(self.writes.get() + bytes.len()); Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

#[test]
fn cooperative_native_peer_post_bpe_unwind_latches_attempt_and_cannot_resume() {
    let mut prompt = b"\0\xff".to_vec();
    for _ in 0..80 { prompt.extend_from_slice(b"allow"); }
    prompt.push(b'?');
    for interrupted in [false, true] {
        let writes = Rc::new(Cell::new(0));
        let mut bytes = frame(&prompt, &expected()); bytes.push(b'R');
        let mut worker = NativeHelperClient::new(Stream {
            input: Cursor::new(bytes), writes: Rc::clone(&writes),
        }, evaluator(b"allow", 0), vec![31; 16]).unwrap();
        while worker.phase() != ClientPhase::NeedsInference { worker.step_cooperative().unwrap(); }
        worker.step_cooperative().unwrap();
        assert_eq!(worker.evaluation_progress().admission_steps, 256);
        if !interrupted {
            for _ in 0..2048 {
                if worker.phase() == ClientPhase::ReplySent { break; }
                worker.step_cooperative().unwrap();
            }
            assert_eq!(worker.phase(), ClientPhase::ReplySent);
            assert_eq!(worker.sampled_draws(), 2);
            assert!(writes.get() > 0);
            continue;
        }
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            worker.step_with_mode(true, || panic!("after original bounded BPE quantum"))
        })).is_err());
        assert_eq!(worker.failure(), Some(NativeClientError::Interrupted));
        let before = worker.evaluation_progress();
        assert_eq!(before.admission_steps, 512);
        assert!(before.tokenization.merges > 0);
        assert_eq!(before.work.decoder.tokens, 0);
        assert_eq!(before.work.sampled_draws, 0);
        assert_eq!(worker.step_cooperative(), Err(NativeClientError::Interrupted));
        assert_eq!(worker.evaluation_progress().status, NativeEvaluationStatus::Cancelled);
        assert_eq!(worker.evaluation_progress().tokenization, before.tokenization);
        assert_eq!(worker.evaluation_progress().admission_steps, before.admission_steps);
        assert_eq!(worker.evaluation_progress().work, before.work);
        assert_eq!(worker.evaluations(), 1);
        assert!(worker.report().is_none());
        assert_eq!(writes.get(), 0);
        assert!(!worker.cancel());
        assert_eq!(worker.step(), Err(NativeClientError::Interrupted));
    }
}
