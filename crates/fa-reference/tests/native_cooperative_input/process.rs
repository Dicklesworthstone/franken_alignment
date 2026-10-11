use super::{assets, fixture::{admit, frame, input, load, prompt, tokenizer}};
use crate::process_fixture;
use fa_reference::action::consequence::oversight::helper_client::{
    ClientPhase, native::{NativeEvaluationStatus, NativeEvaluationWork,
        process::{NativeProcessBudget, NativeProcessReport, NativeProcessStop, run_native_worker}},
};
use fa_reference::round::Verdict;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

fn run(fixture: &assets::Fixture, bytes: &[u8], steps: usize, reveal: bool) -> (NativeProcessReport, Vec<u8>) {
    let (mut parent, socket) = UnixStream::pair().unwrap();
    parent.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    parent.set_write_timeout(Some(Duration::from_secs(10))).unwrap();
    parent.write_all(&frame(bytes)).unwrap();
    if reveal { parent.write_all(b"R").unwrap(); }
    let report = run_native_worker(socket, load(fixture, &fixture.policy), assets::SALT.to_vec(),
        NativeProcessBudget::new(10_000, steps).unwrap()).unwrap();
    let mut output = Vec::new();
    parent.read_to_end(&mut output).unwrap();
    (report, output)
}

#[test]
fn cooperative_native_process_charges_tokenizer_quanta_and_yields_before_prefill() {
    let fixture = assets::Fixture::new(false);
    let bytes = prompt(b'?');
    let packet = input(&bytes);
    let encoded = tokenizer(&fixture).encode(&bytes, fixture.policy.tokenization).unwrap();
    let mut baseline = load(&fixture, &fixture.policy);
    baseline.begin_cooperative(&packet).unwrap();
    let admitted = admit(&mut baseline).unwrap();
    let quanta = admitted.admission_steps.div_ceil(256);
    assert!(quanta > 2);
    // Two complete prequeued request reads. Each further process step spends
    // one fixed tokenizer quantum, including the final no-inference handoff.
    for steps in [3, 4, 2 + quanta, 3 + quanta] {
        let (report, output) = run(&fixture, &bytes, steps, false);
        assert_eq!(report.stop, NativeProcessStop::StepLimit);
        assert_eq!(report.steps, steps);
        assert_eq!(report.evaluations, 1);
        assert_eq!(report.phase, ClientPhase::NeedsInference);
        assert_eq!(report.failure, None);
        assert_eq!(report.evaluation.status, NativeEvaluationStatus::Cancelled);
        assert_eq!(report.evaluation.work.sampled_draws, 0);
        assert!(output.is_empty());
        if steps == 3 {
            assert_eq!(report.evaluation.admission_steps, 256);
            assert_eq!(report.evaluation.tokenization.pair_lookups, 256);
        } else if steps == 4 {
            assert_eq!(report.evaluation.admission_steps, 512);
            assert!(report.evaluation.tokenization.merges > 0);
        } else {
            assert_eq!(report.evaluation.admission_steps, admitted.admission_steps);
            assert_eq!(report.evaluation.tokenization, encoded.work());
        }
        if steps == 3 + quanta {
            assert_eq!(report.evaluation.work.decoder.tokens, 1);
            assert_eq!(report.evaluation.reviewed_prompt_tokens, 1);
        } else {
            assert_eq!(report.evaluation.work, NativeEvaluationWork::default());
        }
    }
}

#[test]
fn cooperative_native_process_exact_completion_budget_preserves_sent_commitment_on_one_short() {
    let fixture = assets::Fixture::new(false);
    let bytes = prompt(b'!');
    let packet = input(&bytes);
    let mut baseline = load(&fixture, &fixture.policy);
    baseline.begin_cooperative(&packet).unwrap();
    let admitted = admit(&mut baseline).unwrap();
    // Original header/body reads, commitment write, reveal-request read and
    // reveal write are five calls for these completely prequeued small frames.
    let steps = admitted.admission_steps.div_ceil(256) + admitted.requested_prompt_tokens + 2 + 5;
    let (complete, bytes_out) = run(&fixture, &bytes, steps, true);
    assert_eq!(complete.stop, NativeProcessStop::ReplySent);
    assert_eq!(complete.steps, steps);
    assert_eq!(complete.evaluation.status, NativeEvaluationStatus::Judged(Verdict::Deny));
    assert_eq!(complete.evaluation.work.sampled_draws, 2);
    assert_eq!(complete.evaluation.reviewed_prompt_tokens, admitted.requested_prompt_tokens);
    assert_eq!(complete.evaluation.tokenization, admitted.tokenization);
    let commitment = packet.commitment_frame(Verdict::Deny, assets::SALT).unwrap();
    let reveal = packet.reveal_frame(Verdict::Deny, assets::SALT).unwrap();
    assert_eq!(bytes_out, [commitment.as_slice(), reveal.as_slice()].concat());

    let (short, bytes_out) = run(&fixture, &bytes, steps - 1, true);
    assert_eq!(short.stop, NativeProcessStop::StepLimit);
    assert_eq!(short.steps, steps - 1);
    assert_eq!(short.phase, ClientPhase::SendingReveal);
    assert_eq!(short.evaluation, complete.evaluation);
    assert_eq!(bytes_out, commitment);
}

#[test]
fn actual_native_binary_uses_cooperative_input_and_preserves_legacy_manifest_and_verdicts() {
    let mut fixture = assets::Fixture::new(false);
    for (last, verdict) in [(b'?', Verdict::Allow), (b'!', Verdict::Deny)] {
        process_fixture::exchange(&fixture, &prompt(last), verdict);
    }
    // With the unchanged manifest schema, four process calls now stop amid
    // original BPE. They cannot silently compute the first native tokens.
    fixture.manifest = fixture.manifest.replace("\"steps\":10000", "\"steps\":4");
    fixture.save();
    let stderr = process_fixture::no_vote(&fixture, Some(&prompt(b'?')));
    assert!(stderr.contains("StepLimit"), "{stderr}");
    assert!(stderr.contains("position: 0"), "{stderr}");
    assert!(stderr.contains("sampled_draws: 0"), "{stderr}");
    assert!(stderr.contains("admission_steps: 512"), "{stderr}");
}
