//! Same inherited private socket and actual binary as the original worker tests.
//! The test owns bounded child cleanup, stderr and the entire protocol exchange.
use super::assets;
use fa_reference::round::Verdict;
use std::io::{Read, Write};
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

struct Worker(Option<Child>);
impl Worker {
    fn start(fixture: &assets::Fixture, socket: UnixStream) -> Self {
        Self(Some(Command::new(env!("CARGO_BIN_EXE_fa-native-helper"))
            .arg(&fixture.path).stdin(Stdio::from(OwnedFd::from(socket)))
            .stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap()))
    }
    fn finish(mut self) -> (ExitStatus, String) {
        let deadline = Instant::now() + Duration::from_secs(15);
        let status = loop {
            if let Some(status) = self.0.as_mut().unwrap().try_wait().unwrap() { break status; }
            assert!(Instant::now() < deadline, "native worker did not exit");
            std::thread::sleep(Duration::from_millis(5));
        };
        let mut child = self.0.take().unwrap();
        let mut stdout = Vec::new(); let mut stderr = Vec::new();
        child.stdout.take().unwrap().take(65_537).read_to_end(&mut stdout).unwrap();
        child.stderr.take().unwrap().take(65_537).read_to_end(&mut stderr).unwrap();
        assert!(stdout.is_empty(), "protocol escaped private socket");
        assert!(stderr.len() <= 65_536, "unbounded diagnostics");
        let stderr = String::from_utf8(stderr).unwrap();
        assert!(!stderr.contains(std::str::from_utf8(assets::SALT).unwrap()));
        if !stderr.is_empty() { eprintln!("native-worker stderr: {stderr}"); }
        (status, stderr)
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            if !matches!(child.try_wait(), Ok(Some(_))) {
                if let Err(error) = child.kill() { eprintln!("worker cleanup kill: {error}"); }
            }
            if let Err(error) = child.wait() { eprintln!("worker cleanup wait: {error}"); }
        }
    }
}
fn sockets() -> (UnixStream, UnixStream) {
    let (parent, child) = UnixStream::pair().unwrap();
    parent.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
    parent.set_write_timeout(Some(Duration::from_secs(15))).unwrap();
    (parent, child)
}
pub fn exchange(fixture: &assets::Fixture, prompt: &[u8], verdict: Verdict) {
    exchange_frame(fixture, &super::fixture::frame(prompt), verdict)
}
pub fn exchange_frame(fixture: &assets::Fixture, request: &[u8], verdict: Verdict) {
    let (mut parent, socket) = sockets(); let child = Worker::start(fixture, socket);
    parent.write_all(request).unwrap();
    let input = fa_reference::action::consequence::oversight::helper_workers::wire::decode_request(request).unwrap();
    let expected = input.commitment_frame(verdict, assets::SALT).unwrap();
    let mut commit = vec![0; expected.len()]; parent.read_exact(&mut commit).unwrap();
    assert_eq!(commit, expected);
    // A commitment cannot disclose a reveal before the controller requests it.
    parent.set_read_timeout(Some(Duration::from_millis(20))).unwrap();
    let mut byte = [0];
    assert!(matches!(parent.read(&mut byte).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut));
    parent.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
    parent.write_all(b"R").unwrap();
    let expected = input.reveal_frame(verdict, assets::SALT).unwrap();
    let mut reveal = vec![0; expected.len()]; parent.read_exact(&mut reveal).unwrap();
    assert_eq!(reveal, expected);
    assert_eq!(parent.read(&mut byte).unwrap(), 0);
    let (status, stderr) = child.finish(); assert!(status.success(), "{status}: {stderr}");
}
pub fn no_vote(fixture: &assets::Fixture, prompt: Option<&[u8]>) -> String {
    let bytes = prompt.map(super::fixture::frame);
    no_vote_frame(fixture, bytes.as_deref())
}
pub fn no_vote_frame(fixture: &assets::Fixture, request: Option<&[u8]>) -> String {
    let (mut parent, socket) = sockets(); let child = Worker::start(fixture, socket);
    if let Some(bytes) = request { parent.write_all(bytes).unwrap(); }
    let mut byte = [0];
    match parent.read(&mut byte) {
        Ok(0) => {}
        Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {}
        other => panic!("refusal must not emit a commitment: {other:?}"),
    }
    let (status, stderr) = child.finish(); assert!(!status.success()); stderr
}
