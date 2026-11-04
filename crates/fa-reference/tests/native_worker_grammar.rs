//! The actual worker executable, checkpoint loader, private socket and original
//! commit/reveal protocol. Synthetic parameters do not establish model quality.
#![forbid(unsafe_code)]
#![cfg(unix)]

#[path = "support/native_worker_assets.rs"]
mod assets;
#[path = "support/native_worker_v2_assets.rs"]
mod v2_assets;
#[path = "support/native_worker_grammar_assets.rs"]
mod grammar_assets;

use fa_reference::round::Verdict;
use std::io::{Read, Write};
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

struct ChildGuard(Option<Child>);
impl ChildGuard {
    fn start(fixture: &assets::Fixture, socket: UnixStream) -> Self {
        Self(Some(Command::new(env!("CARGO_BIN_EXE_fa-native-helper"))
            .arg(&fixture.path).stdin(Stdio::from(OwnedFd::from(socket)))
            .stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap()))
    }
    fn finish(mut self) -> (ExitStatus, String) {
        let deadline = Instant::now() + Duration::from_secs(15);
        let status = loop {
            if let Some(status) = self.0.as_mut().unwrap().try_wait().unwrap() { break status; }
            assert!(Instant::now() < deadline, "native worker failed to terminate");
            std::thread::sleep(Duration::from_millis(5));
        };
        let mut child = self.0.take().unwrap(); let mut stderr = Vec::new(); let mut stdout = Vec::new();
        child.stderr.take().unwrap().take(65_537).read_to_end(&mut stderr).unwrap();
        child.stdout.take().unwrap().take(65_537).read_to_end(&mut stdout).unwrap();
        assert!(stderr.len() <= 65_536); assert!(stdout.is_empty());
        (status, String::from_utf8(stderr).unwrap())
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

fn exchange(fixture: &assets::Fixture, expected: Option<Verdict>) -> String {
    let (mut parent, socket) = UnixStream::pair().unwrap();
    parent.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
    parent.set_write_timeout(Some(Duration::from_secs(15))).unwrap();
    let child = ChildGuard::start(fixture, socket);
    parent.write_all(&assets::frame(b"?")).unwrap();
    if let Some(verdict) = expected {
        let input = assets::input(b"?"); let commitment = input.commitment_frame(verdict, assets::SALT).unwrap();
        let mut bytes = vec![0; commitment.len()]; parent.read_exact(&mut bytes).unwrap(); assert_eq!(bytes, commitment);
        parent.write_all(b"R").unwrap();
        let reveal = input.reveal_frame(verdict, assets::SALT).unwrap();
        let mut bytes = vec![0; reveal.len()]; parent.read_exact(&mut bytes).unwrap(); assert_eq!(bytes, reveal);
    }
    let mut byte = [0];
    match parent.read(&mut byte) {
        Ok(0) => {}
        Err(error) if expected.is_none() && error.kind() == std::io::ErrorKind::ConnectionReset => {}
        other => panic!("no extra or fabricated vote: {other:?}"),
    }
    let (status, stderr) = child.finish(); assert_eq!(status.success(), expected.is_some(), "{stderr}"); stderr
}

#[test]
fn worker_grammar_chooses_lower_scored_valid_verdict_through_real_commit_reveal() {
    for sharded in [false, true] {
        for json in [false, true] {
            let mut fixture = assets::Fixture::new(false); grammar_assets::prefer_invalid_token(&fixture);
            v2_assets::configure(&mut fixture, sharded, json);
            grammar_assets::configure(&mut fixture, "finite-words-v1");
            exchange(&fixture, Some(Verdict::Allow));
            fixture.manifest = fixture.manifest.replace("finite-words-v1", "unrestricted"); fixture.save();
            assert!(exchange(&fixture, None).contains("InvalidVerdict"));
        }
    }
}

#[test]
fn worker_grammar_does_not_emit_a_vote_for_a_held_answer_or_terminal() {
    for terminal in [false, true] {
        let mut fixture = assets::Fixture::new(!terminal);
        grammar_assets::prefer_invalid_token(&fixture);
        if terminal { grammar_assets::hold_terminal(&fixture); }
        v2_assets::configure(&mut fixture, true, true);
        grammar_assets::configure(&mut fixture, "finite-words-v1");
        assert!(exchange(&fixture, None).contains("Held"));
    }
}

#[test]
fn worker_grammar_keeps_token_and_full_vocabulary_limits_before_terminal_vote() {
    for budget in [false, true] {
        let mut fixture = assets::Fixture::new(false); grammar_assets::prefer_invalid_token(&fixture);
        v2_assets::configure(&mut fixture, false, false);
        grammar_assets::configure(&mut fixture, "finite-words-v1");
        fixture.manifest = if budget {
            fixture.manifest.replace("\"sampling_entries\":16777216", "\"sampling_entries\":264")
        } else { fixture.manifest.replace("\"max_new_tokens\":2", "\"max_new_tokens\":1") };
        fixture.save();
        assert!(exchange(&fixture, None).contains(if budget { "BudgetExhausted" } else { "TokenLimit" }));
    }
}
