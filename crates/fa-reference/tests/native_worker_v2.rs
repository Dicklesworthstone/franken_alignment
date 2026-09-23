//! Actual private worker executable over an inherited Unix socket. Synthetic
//! checkpoints test integration, not model calibration or OS containment.
#![forbid(unsafe_code)]
#![cfg(unix)]

#[path = "support/native_worker_assets.rs"]
mod assets;
#[path = "support/native_worker_v2_assets.rs"]
mod v2_assets;

use fa_reference::round::Verdict;
use std::io::{Read, Write};
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

struct ChildGuard(Option<Child>);
impl ChildGuard {
    fn start(fixture: &assets::Fixture, socket: UnixStream) -> Self {
        let child = Command::new(env!("CARGO_BIN_EXE_fa-native-helper"))
            .arg(&fixture.path).stdin(Stdio::from(OwnedFd::from(socket)))
            .stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        Self(Some(child))
    }
    fn finish(mut self) -> (ExitStatus, String) {
        let deadline = Instant::now() + Duration::from_secs(15);
        let status = loop {
            if let Some(status) = self.0.as_mut().unwrap().try_wait().unwrap() { break status; }
            assert!(Instant::now() < deadline, "native worker failed to terminate");
            std::thread::sleep(Duration::from_millis(5));
        };
        let mut child = self.0.take().unwrap();
        let mut stderr = Vec::new(); let mut stdout = Vec::new();
        child.stderr.take().unwrap().take(65_537).read_to_end(&mut stderr).unwrap();
        child.stdout.take().unwrap().take(65_537).read_to_end(&mut stdout).unwrap();
        assert!(stderr.len() <= 65_536, "bounded worker diagnostics");
        assert!(stdout.is_empty(), "no helper protocol may escape onto stdout");
        let stderr = String::from_utf8(stderr).unwrap();
        if !stderr.is_empty() { eprintln!("native-worker stderr: {stderr}"); }
        (status, stderr)
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            if !matches!(child.try_wait(), Ok(Some(_))) {
                if let Err(error) = child.kill() { eprintln!("native worker cleanup kill: {error}"); }
            }
            if let Err(error) = child.wait() { eprintln!("native worker cleanup wait: {error}"); }
        }
    }
}
fn sockets() -> (UnixStream, UnixStream) {
    let (parent, child) = UnixStream::pair().unwrap();
    parent.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
    parent.set_write_timeout(Some(Duration::from_secs(15))).unwrap();
    (parent, child)
}
fn exchange(fixture: &assets::Fixture, prompt: &[u8], verdict: Verdict) {
    let (mut parent, socket) = sockets(); let child = ChildGuard::start(fixture, socket);
    parent.write_all(&assets::frame(prompt)).unwrap();
    let input = assets::input(prompt);
    let expected = input.commitment_frame(verdict, assets::SALT).unwrap();
    let mut commit = vec![0; expected.len()]; parent.read_exact(&mut commit).unwrap();
    assert_eq!(commit, expected);
    // A real reveal request, not permission to replace the previously committed vote.
    parent.write_all(b"R").unwrap();
    let expected = input.reveal_frame(verdict, assets::SALT).unwrap();
    let mut reveal = vec![0; expected.len()]; parent.read_exact(&mut reveal).unwrap();
    assert_eq!(reveal, expected);
    let mut extra = [0]; assert_eq!(parent.read(&mut extra).unwrap(), 0);
    let (status, stderr) = child.finish(); assert!(status.success(), "{status}: {stderr}");
}
fn no_vote(fixture: &assets::Fixture, prompt: Option<&[u8]>) -> String {
    let (mut parent, socket) = sockets(); let child = ChildGuard::start(fixture, socket);
    if let Some(prompt) = prompt { parent.write_all(&assets::frame(prompt)).unwrap(); }
    let mut byte = [0];
    match parent.read(&mut byte) {
        Ok(0) => {}
        // A step-limited worker may close with part of the request unread.
        // Reset is loss of transport, not an accepted vote or a timeout success.
        Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {}
        other => panic!("no default commitment or reveal: {other:?}"),
    }
    let (status, stderr) = child.finish(); assert!(!status.success()); stderr
}

#[test]
fn worker_v2_process_runs_every_checkpoint_format_and_input_dependent_verdict() {
    for sharded in [false, true] {
        for json in [false, true] {
            for (prompt, verdict) in [(b"?".as_slice(), Verdict::Allow), (b"!", Verdict::Deny)] {
                let mut fixture = assets::Fixture::new(false); v2_assets::configure(&mut fixture, sharded, json);
                if sharded { std::fs::remove_file(fixture.root.join("weights.safetensors")).unwrap(); }
                exchange(&fixture, prompt, verdict);
            }
        }
    }
}

#[test]
fn worker_v2_process_retains_legacy_manifest_and_imported_named_prompt_controls() {
    let fixture = assets::Fixture::new(false); exchange(&fixture, b"?", Verdict::Allow);
    let mut fixture = assets::Fixture::new(false); v2_assets::configure(&mut fixture, true, true);
    exchange(&fixture, b"<eos>!", Verdict::Deny);
}

#[test]
fn worker_v2_process_holds_and_incomplete_allow_never_emit_a_vote() {
    for alarm in [false, true] {
        let mut fixture = assets::Fixture::new(alarm); v2_assets::configure(&mut fixture, true, true);
        if !alarm { fixture.manifest = fixture.manifest.replace("\"max_new_tokens\":2", "\"max_new_tokens\":1"); fixture.save(); }
        let stderr = no_vote(&fixture, Some(b"?"));
        assert!(stderr.contains(if alarm { "Held" } else { "TokenLimit" }), "{stderr}");
    }
}

#[test]
fn worker_v2_process_bad_formats_sources_and_budgets_have_no_legacy_fallback() {
    for case in 0..4 {
        let mut fixture = assets::Fixture::new(false); v2_assets::configure(&mut fixture, true, true);
        match case {
            0 => fixture.manifest = fixture.manifest.replace("huggingface-raw-bytelevel", "native-archive"),
            1 => { std::fs::remove_file(fixture.root.join("registered-1.bin")).unwrap(); }
            2 => fixture.manifest = fixture.manifest.replace("\"weight_calls\":4096", "\"weight_calls\":1"),
            _ => fixture.manifest = fixture.manifest.replace("\"tokenizer_format\":\"huggingface-raw-bytelevel\"", "\"tokenizer_format\":\"auto\""),
        }
        // The single weight file deliberately still exists. Failure may NOT
        // discard the declared sharded layout and silently load that file.
        assert!(fixture.root.join("weights.safetensors").exists()); fixture.save();
        assert!(!no_vote(&fixture, None).is_empty());
    }
}

#[test]
fn worker_v2_process_does_not_reset_the_original_step_allowance_after_loading() {
    let mut fixture = assets::Fixture::new(false); v2_assets::configure(&mut fixture, true, true);
    fixture.manifest = fixture.manifest.replace("\"steps\":10000", "\"steps\":1"); fixture.save();
    let stderr = no_vote(&fixture, Some(b"?")); assert!(stderr.contains("StepLimit"), "{stderr}");
}
