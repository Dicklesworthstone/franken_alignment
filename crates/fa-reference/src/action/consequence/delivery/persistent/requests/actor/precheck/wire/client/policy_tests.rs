//! Scripted large responses exercise the actual shared I/O engine, not truth.
use super::*;
use super::super::policy::{decode_policy_response, PolicyRequest};
use std::io::Cursor;

struct Script { input: Cursor<Vec<u8>>, output: Vec<u8>, flushed: bool, calls: usize }
impl Read for Script {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        assert!(self.flushed); self.calls += 1; self.input.read(bytes)
    }
}
impl Write for Script {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        assert!(!self.flushed); self.calls += 1;
        let count = bytes.len().min(3); self.output.extend_from_slice(&bytes[..count]); Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> { self.calls += 1; self.flushed = true; Ok(()) }
}
fn body() -> Vec<u8> {
    format!(concat!("{{\"schema\":\"fa.actor-policy-response/1\",\"informational\":true,",
        "\"query\":\"7\",\"status\":\"ok\",\"knowledge\":{{\"state\":\"known\",\"value\":{{",
        "\"policy_disclosure\":\"full\",\"scope\":\"exact_policy_definition\",",
        "\"source_values\":\"withheld\",\"helper_feedback\":\"withheld\",",
        "\"consequences\":\"withheld\",\"effect_authority\":false,\"review_required\":true,",
        "\"definition\":{{\"generation\":\"1\",\"nodes\":[{{\"op\":\"payload_is\",\"value_hex\":\"{}\"}}]}}}},",
        "\"basis\":{{\"disclosure\":\"82\",\"query\":\"7\",\"policy_epoch\":\"0\",\"observed_at\":\"1\"}}}}}}"),
        "ff".repeat(65_536)).into_bytes()
}
fn exchange() -> Exchange<PolicyResponse> {
    let request = PolicyRequest { query: 7, expected_policy_epoch: 0 };
    Exchange::from_document_bounded(request.encode().unwrap(), 7, 0,
        MAX_POLICY_REQUEST_BYTES, MAX_POLICY_RESPONSE_BYTES, decode_policy_response).unwrap()
}
fn run(bytes: Vec<u8>) -> (Exchange<PolicyResponse>, Script) {
    let mut exchange = exchange();
    let mut script = Script { input: Cursor::new(bytes), output: Vec::new(), flushed: false, calls: 0 };
    let budget = DriveBudget { read_bytes: 97, write_bytes: 5, frames: 1, io_calls: 2 };
    for _ in 0..10_000 {
        let report = exchange.drive_io(&mut script, budget).unwrap();
        assert!(report.progress.read_bytes <= 97 && report.progress.written_bytes <= 5);
        assert!(report.progress.io_calls <= 2 && report.progress.frames <= 1);
        if report.status.closed() { break; }
    }
    assert!(exchange.status().closed());
    let calls = script.calls;
    assert_eq!(exchange.drive_io(&mut script, budget).unwrap().progress, DriveProgress::default());
    assert_eq!(script.calls, calls);
    (exchange, script)
}

#[test]
fn full_size_policy_and_exact_response_ceiling_survive_partial_io_without_retransmit() {
    let body = body(); let expected = decode_policy_response(&body, 7, 0).unwrap();
    assert!(body.len() > MAX_PRECHECK_RESPONSE_BYTES);
    for padded in [false, true] {
        let mut line = body.clone();
        if padded { line.resize(MAX_POLICY_RESPONSE_BYTES, b' '); }
        line.push(b'\n');
        let (mut exchange, script) = run(line);
        assert_eq!(exchange.response.take(), Some(expected.clone()));
        let mut request = PolicyRequest { query: 7, expected_policy_epoch: 0 }.encode().unwrap(); request.push(b'\n');
        assert_eq!(script.output, request);
    }
}

#[test]
fn oversized_truncated_and_trailing_policy_responses_latch_without_a_definition() {
    let body = body(); let mut over = body.clone();
    over.resize(MAX_POLICY_RESPONSE_BYTES + 1, b' '); over.push(b'\n');
    for (bytes, error) in [
        (body.clone(), PrecheckClientError::TruncatedResponse),
        (over, PrecheckClientError::ResponseTooLarge),
        ([body, b"\nx".to_vec()].concat(), PrecheckClientError::TrailingResponseData),
    ] {
        let (exchange, _) = run(bytes);
        assert_eq!(exchange.status().failure, Some(error)); assert!(exchange.response.is_none());
    }
}

#[test]
fn full_codec_selection_does_not_enlarge_the_old_clients_or_accept_wrong_response_types() {
    let request = PolicyRequest { query: 7, expected_policy_epoch: 0 }.encode().unwrap();
    let old = Exchange::from_document(request, 7, 0, MAX_POLICY_REQUEST_BYTES, decode_precheck_response).unwrap();
    assert_eq!(old.input.len(), MAX_PRECHECK_RESPONSE_BYTES);
    let wrong = b"{\"schema\":\"fa.actor-precheck-response/1\",\"precheck\":true,\"query\":\"7\",\"status\":\"error\",\"reason\":\"capacity\"}\n".to_vec();
    let (exchange, _) = run(wrong);
    assert_eq!(exchange.status().failure, Some(PrecheckClientError::Response(PrecheckResponseError::UnsupportedVersion)));
    assert!(exchange.response.is_none());
}
