//! Actual Unix sockets and the unchanged original helper transcript/worker loop.
use super::*;
use super::super::peer::{NativeShardFilePeerError, NativeShardProcessError, run_native_worker_from_shard_files};
use crate::action::consequence::oversight::helper_client::{ClientPhase, ClientInterest};
use crate::action::consequence::oversight::helper_client::native::peer::{NativeClientError, NativeHelperClient};
use crate::action::consequence::oversight::helper_client::native::process::{NativeProcessBudget, NativeProcessStop};
use super::super::super::super::super::tests::{frame, expected};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

#[test]
fn shard_files_peer_preserves_original_commit_reveal_and_actual_input_sensitive_votes() {
    for (prompt, verdict) in [(b"?".as_slice(), Verdict::Allow), (b"!", Verdict::Deny)] {
        let root = Directory::new(false); let (mut supervisor, socket) = UnixStream::pair().unwrap();
        supervisor.set_nonblocking(true).unwrap(); let salt = vec![27; 16];
        let (mut worker, receipt) = NativeHelperClient::from_llama_shard_files(root.request(), socket,
            salt.clone(), &mut assets(), &mut budget()).unwrap();
        assert_eq!(receipt.weights.file_bytes, root.weight_bytes());
        assert_eq!(worker.phase(), ClientPhase::ReadingRequest); assert_eq!(worker.evaluations(), 0);
        let mut byte = [0]; assert_eq!(supervisor.read(&mut byte).unwrap_err().kind(), io::ErrorKind::WouldBlock);
        supervisor.write_all(&frame(prompt, &expected())).unwrap();
        for _ in 0..256 { worker.step().unwrap(); if worker.phase() == ClientPhase::AwaitingReveal { break; } }
        assert_eq!(worker.phase(), ClientPhase::AwaitingReveal); assert_eq!(worker.evaluations(), 1);
        let mut commit = [0; 9]; supervisor.read_exact(&mut commit).unwrap();
        assert_eq!(commit, input(prompt).commitment_frame(verdict, &salt).unwrap());
        supervisor.write_all(b"R").unwrap();
        for _ in 0..128 { worker.step().unwrap(); if worker.phase() == ClientPhase::ReplySent { break; } }
        assert_eq!(worker.phase(), ClientPhase::ReplySent); assert_eq!(worker.sampled_draws(), 2);
        let expected_reveal = input(prompt).reveal_frame(verdict, &salt).unwrap();
        let mut reveal = vec![0; expected_reveal.len()]; supervisor.read_exact(&mut reveal).unwrap();
        assert_eq!(reveal, expected_reveal);
        worker.step().unwrap(); assert_eq!(worker.evaluations(), 1); assert_eq!(worker.sampled_draws(), 2);
        assert_eq!(supervisor.read(&mut byte).unwrap_err().kind(), io::ErrorKind::WouldBlock);
    }
}

#[test]
fn shard_files_peer_hold_cannot_emit_commitment_or_reroll_the_loaded_model() {
    let root = Directory::new(true); let (mut supervisor, socket) = UnixStream::pair().unwrap();
    supervisor.set_nonblocking(true).unwrap();
    let (mut worker, _) = NativeHelperClient::from_llama_shard_files(root.request(), socket,
        vec![27; 16], &mut assets(), &mut budget()).unwrap();
    supervisor.write_all(&frame(b"?", &expected())).unwrap();
    for _ in 0..256 { if worker.step().is_err() { break; } }
    assert!(matches!(worker.failure(), Some(NativeClientError::Inference(
        NativeEvaluationError::Incomplete(GenerationFinish::Held)))));
    assert_eq!(worker.interest(), ClientInterest::Finished); assert_eq!(worker.sampled_draws(), 1);
    assert_eq!(worker.evaluations(), 1); assert!(worker.step().is_err());
    assert_eq!(worker.sampled_draws(), 1);
    let mut byte = [0]; assert_eq!(supervisor.read(&mut byte).unwrap_err().kind(), io::ErrorKind::WouldBlock);
}

#[test]
fn shard_files_peer_failed_salt_or_startup_closes_without_reading_a_request_or_voting() {
    for bad_salt in [false, true] {
        let root = Directory::new(false); let (mut supervisor, socket) = UnixStream::pair().unwrap();
        supervisor.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut asset_work = assets(); let mut weight_work = budget();
        if !bad_salt { fs::write(&root.assets[1], b"not a tokenizer").unwrap(); }
        let result = NativeHelperClient::from_llama_shard_files(root.request(), socket,
            vec![27; if bad_salt { 15 } else { 16 }], &mut asset_work, &mut weight_work);
        if bad_salt {
            assert!(matches!(result, Err(NativeShardFilePeerError::Transport(_))));
            assert_eq!(asset_work.usage().read_calls, 0);
        } else { assert!(matches!(result, Err(NativeShardFilePeerError::Startup(_)))); }
        assert_eq!(weight_work.usage().read_calls, 0);
        let mut byte = [0]; assert_eq!(supervisor.read(&mut byte).unwrap(), 0);
    }
}

#[test]
fn shard_files_dedicated_worker_sends_the_original_transcript_and_releases_ownership() {
    let root = Directory::new(false); let (mut supervisor, socket) = UnixStream::pair().unwrap();
    supervisor.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    supervisor.set_write_timeout(Some(Duration::from_secs(10))).unwrap();
    let salt = vec![31; 16]; let original = frame(b"!", &expected());
    let commit = input(b"!").commitment_frame(Verdict::Deny, &salt).unwrap();
    let reveal = input(b"!").reveal_frame(Verdict::Deny, &salt).unwrap();
    // The native evaluator is not Send. It remains on this thread; only the
    // independent supervisor socket runs in the test peer thread.
    let peer = std::thread::spawn(move || {
        supervisor.write_all(&original).unwrap();
        let mut received = [0; 9]; supervisor.read_exact(&mut received).unwrap(); assert_eq!(received, commit);
        supervisor.write_all(b"R").unwrap();
        let mut received = vec![0; reveal.len()]; supervisor.read_exact(&mut received).unwrap(); assert_eq!(received, reveal);
        let mut byte = [0]; assert_eq!(supervisor.read(&mut byte).unwrap(), 0);
    });
    let lifetime = NativeProcessBudget::new(30_000, 4096).unwrap();
    let result = run_native_worker_from_shard_files(root.request(), socket, salt,
        &mut assets(), &mut budget(), lifetime);
    peer.join().unwrap();
    let (report, receipt) = result.unwrap();
    assert_eq!(report.stop, NativeProcessStop::ReplySent);
    assert_eq!(report.phase, ClientPhase::ReplySent); assert_eq!(report.evaluations, 1);
    assert!(report.failure.is_none()); assert_eq!(receipt.weights.file_bytes, root.weight_bytes());
}

#[test]
fn shard_files_expired_worker_lifetime_refuses_before_disk_without_a_new_deadline() {
    let root = Directory::new(false); let (mut supervisor, socket) = UnixStream::pair().unwrap();
    supervisor.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let lifetime = NativeProcessBudget::new(1, 64).unwrap();
    std::thread::sleep(Duration::from_millis(5));
    assert!(lifetime.expired());
    fs::remove_file(&root.assets[0]).unwrap(); // wrong ordering would report file I/O
    let mut asset_work = assets(); let mut weight_work = budget();
    assert!(matches!(run_native_worker_from_shard_files(root.request(), socket, vec![31; 16],
        &mut asset_work, &mut weight_work, lifetime), Err(NativeShardProcessError::ExpiredBeforeStartup)));
    assert_eq!(asset_work.usage().read_calls, 0); assert_eq!(weight_work.usage().read_calls, 0);
    let mut byte = [0]; assert_eq!(supervisor.read(&mut byte).unwrap(), 0);
}

#[test]
fn shard_files_dedicated_worker_retains_the_original_step_allowance_after_loading() {
    let root = Directory::new(false); let (mut supervisor, socket) = UnixStream::pair().unwrap();
    supervisor.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    supervisor.write_all(&frame(b"?", &expected())).unwrap();
    let (report, receipt) = run_native_worker_from_shard_files(root.request(), socket, vec![31; 16],
        &mut assets(), &mut budget(), NativeProcessBudget::new(30_000, 1).unwrap()).unwrap();
    assert_eq!(report.stop, NativeProcessStop::StepLimit); assert_eq!(report.steps, 1);
    assert_eq!(receipt.weights.file_bytes, root.weight_bytes());
    // Closing with unread inbound bytes can reset the Unix socket; neither EOF
    // nor reset is a commitment/reveal frame or proof of congress acceptance.
    let mut byte = [0];
    match supervisor.read(&mut byte) {
        Ok(0) => {}
        Err(error) if error.kind() == io::ErrorKind::ConnectionReset => {}
        other => panic!("limited worker must not emit a protocol byte: {other:?}"),
    }
}

#[test]
fn shard_files_non_socket_descriptor_refuses_before_any_asset_or_weight_read() {
    use std::os::fd::OwnedFd;
    let root = Directory::new(false);
    let descriptor: OwnedFd = fs::File::open(&root.assets[0]).unwrap().into();
    let socket = UnixStream::from(descriptor);
    let mut asset_work = assets(); let mut weight_work = budget();
    assert!(matches!(NativeHelperClient::from_llama_shard_files(root.request(), socket,
        vec![27; 16], &mut asset_work, &mut weight_work), Err(NativeShardFilePeerError::Transport(_))));
    assert_eq!(asset_work.usage().read_calls, 0); assert_eq!(weight_work.usage().read_calls, 0);
}
