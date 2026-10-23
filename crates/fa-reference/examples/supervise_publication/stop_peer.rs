//! Operator-facing stop uses only the independent peer profile, never Config.
//! Invoking stop-peer is the explicit restrictive decision; no stdin is needed.
#[cfg(target_os = "linux")]
pub use linux::request;
#[cfg(not(target_os = "linux"))]
pub fn request(_profile: &super::PeerProfile, _operation: u64, _out: &mut impl std::io::Write) -> Result<(), String> {
    Err("stop-peer requires Linux peer credentials; no unchecked fallback".into())
}

#[cfg(target_os = "linux")]
mod linux {
    use super::super::{PeerProfile, debug};
    use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::control::{
        StopClient, StopClientProgress, StopControlReceipt, StopStatus,
    };
    use std::io::{Read, Write};
    use std::time::{Duration, Instant};

    #[derive(Debug)]
    struct Failure { detail: String, may_have_been_sent: bool }
    #[derive(Debug)]
    struct Received { receipt: StopControlReceipt, deadline_reached: bool }

    /// The command authenticates the server on the connected socket before reading
    /// any offer. Exact audience/operation/session binding precedes request_stop.
    /// Neither a failed send nor an output failure reconnects or repeats the request.
    pub fn request(profile: &PeerProfile, operation: u64, out: &mut impl Write) -> Result<(), String> {
        let started = Instant::now();
        request_with(profile, operation, out, &mut || started.elapsed(),
            || profile.connect_stop_client(operation), &mut std::thread::sleep)
    }

    /// One original elapsed budget includes directory checks, connection and
    /// credential verification. Synchronous OS calls cannot be preempted, but
    /// their return cannot renew the budget or authorize a late request.
    pub(super) fn request_with<S: Read + Write>(profile: &PeerProfile, operation: u64,
        out: &mut impl Write, elapsed: &mut impl FnMut() -> Duration,
        connect: impl FnOnce() -> Result<StopClient<S>, String>, pause: &mut impl FnMut(Duration),
    ) -> Result<(), String> {
        if operation == 0 { return Err("stop operation must be nonzero".into()); }
        if profile.runtime_ms == 0 || profile.runtime_ms > 3_600_000
            || profile.poll_ms == 0 || profile.poll_ms > 1000 || profile.poll_ms > profile.runtime_ms {
            return Err("invalid bounded stop-client timing".into());
        }
        let runtime = Duration::from_millis(profile.runtime_ms);
        if elapsed() >= runtime {
            return finish(operation, Err(Failure { detail: "stop receipt deadline reached".into(),
                may_have_been_sent: false }), out);
        }
        let mut client = connect()?;
        let outcome = drive_with(&mut client, runtime, profile.poll_ms, elapsed, pause);
        finish(operation, outcome, out)
    }

    #[cfg(test)]
    fn drive<S: Read + Write>(client: &mut StopClient<S>, runtime: Duration, poll_ms: u64)
        -> Result<Received, Failure>
    {
        let started = Instant::now();
        drive_with(client, runtime, poll_ms, &mut || started.elapsed(), &mut std::thread::sleep)
    }

    fn drive_with<S: Read + Write>(client: &mut StopClient<S>, runtime: Duration, poll_ms: u64,
        elapsed: &mut impl FnMut() -> Duration, pause: &mut impl FnMut(Duration),
    ) -> Result<Received, Failure> {
        let result = (|| {
            loop {
                if elapsed() >= runtime { return Err("stop receipt deadline reached".to_owned()); }
                let progress = client.step();
                let after = elapsed();
                let deadline_reached = after >= runtime;
                match progress.map_err(debug)? {
                    // A valid native receipt remains an acknowledged historical
                    // fact even if reading it crossed the deadline. Report it,
                    // while refusing to classify the command as timely success.
                    StopClientProgress::Complete => return client.receipt()
                        .map(|receipt| Received { receipt, deadline_reached })
                        .ok_or_else(|| "missing native stop receipt".into()),
                    _ if deadline_reached => return Err("stop receipt deadline reached".to_owned()),
                    StopClientProgress::NeedsDecision => client.request_stop().map_err(debug)?,
                    StopClientProgress::Blocked => {
                        let delay = Duration::from_millis(poll_ms).min(runtime.saturating_sub(after));
                        if !delay.is_zero() { pause(delay); }
                    }
                    StopClientProgress::Progress => {}
                }
            }
        })();
        result.map_err(|detail| Failure { detail, may_have_been_sent: client.outcome_unknown() })
    }

    fn finish(operation: u64, outcome: Result<Received, Failure>, out: &mut impl Write) -> Result<(), String> {
        match outcome {
            Ok(Received { receipt, deadline_reached }) => {
                let timing = if deadline_reached { "; stop receipt deadline reached" } else { "" };
                report(receipt, out).map_err(|error| format!("stop status {:?}{timing}; receipt output failed: {error}", receipt.status))?;
                if deadline_reached {
                    Err(format!("stop status {:?}; stop receipt deadline reached; native receipt retained; inspect native recovery state before any retry", receipt.status))
                } else if receipt.drained() { Ok(()) }
                else { Err(format!("stop status {:?}; native recovery/inspection is required", receipt.status)) }
            }
            Err(failure) => {
                let diagnostic = format!("no stop confirmation: {}; request_may_have_been_sent={}; inspect native recovery state before any retry",
                    failure.detail, failure.may_have_been_sent);
                let output = writeln!(out, "{{\"operation\":\"{operation}\",\"status\":\"unconfirmed\",\"acknowledged\":false,\"drained\":false,\"request_may_have_been_sent\":{}}}", failure.may_have_been_sent)
                    .and_then(|()| out.flush());
                match output { Ok(()) => Err(diagnostic), Err(error) => Err(format!("{diagnostic}; output: {error}")) }
            }
        }
    }
    fn report(receipt: StopControlReceipt, out: &mut impl Write) -> std::io::Result<()> {
        let status = match receipt.status {
            StopStatus::Unconfirmed => "unconfirmed", StopStatus::StoppedDrainRefused => "stopped_drain_refused",
            StopStatus::StoppedPending => "stopped_pending", StopStatus::StoppedDrained => "stopped_drained",
        };
        writeln!(out, "{{\"operation\":\"{}\",\"status\":\"{status}\",\"acknowledged\":{},\"drained\":{},\"control_sequence\":\"{}\",\"revocation_floor\":\"{}\",\"dispatcher_epoch\":\"{}\"}}",
            receipt.binding.operation, receipt.acknowledged(), receipt.drained(),
            receipt.control_sequence, receipt.revocation_floor, receipt.dispatcher_epoch)?;
        out.flush()
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use fa_reference::action::{Purpose, Scope};
        use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::ReviewerExpectation;
        use fa_reference::strict_json;
        use std::io::{self, Cursor};

        // Scripted wire parsing/output tests, NOT proof that any effect stopped.
        // Real server/actor composition lives in the workflow control tests.
        struct Wire { input: Cursor<Vec<u8>>, writes: Vec<u8>, fail_write: bool, block_tail: bool }
        impl Read for Wire {
            fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
                if self.block_tail && self.input.position() == self.input.get_ref().len() as u64 {
                    return Err(io::ErrorKind::WouldBlock.into());
                }
                self.input.read(out)
            }
        }
        impl Write for Wire {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                if self.fail_write { return Err(io::ErrorKind::BrokenPipe.into()); }
                self.writes.extend_from_slice(bytes); Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> { Ok(()) }
        }
        fn expected() -> ReviewerExpectation {
            ReviewerExpectation { reviewer: 9, clock_domain: 11,
                scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect } }
        }
        fn offer() -> Vec<u8> {
            let mut bytes = b"FASTOFF1".to_vec();
            for value in [1_u64, 2, 3, 4, 5, 11, 9, 7] { bytes.extend_from_slice(&value.to_be_bytes()); }
            bytes.extend_from_slice(&[6; 32]); bytes
        }
        fn wire(bytes: Vec<u8>) -> Wire { Wire { input: Cursor::new(bytes), writes: Vec::new(), fail_write: false, block_tail: false } }
        fn receipt(status: u8) -> Vec<u8> {
            let mut bytes = offer(); bytes[..8].copy_from_slice(b"FASTRCP1"); bytes.push(status);
            for value in if status == 0 { [0_u64; 3] } else { [20, 21, 22] } { bytes.extend_from_slice(&value.to_be_bytes()); }
            bytes
        }
        fn parsed(status: u8) -> Received {
            let mut bytes = offer(); bytes.extend_from_slice(&receipt(status));
            let mut client = StopClient::new(wire(bytes), expected(), 7).unwrap();
            drive(&mut client, Duration::from_secs(1), 1).unwrap()
        }

        #[test]
        fn reports_native_stop_and_drain_as_separate_facts() {
            for status in 0..=3 {
                let received = parsed(status); let mut out = Vec::new();
                let result = finish(7, Ok(received), &mut out);
                assert_eq!(result.is_ok(), status == 3);
                let json = strict_json::parse(&out, strict_json::Limits::default()).unwrap();
                assert_eq!(json.get("acknowledged").unwrap().as_bool(), Some(status != 0));
                assert_eq!(json.get("drained").unwrap().as_bool(), Some(status == 3));
            }
        }
        #[test]
        fn wrong_offer_and_expiry_before_sending_never_claim_transmission() {
            let mut bytes = offer(); bytes[16] ^= 1;
            let mut client = StopClient::new(wire(bytes), expected(), 7).unwrap();
            assert!(!drive(&mut client, Duration::from_secs(1), 1).unwrap_err().may_have_been_sent);
            let mut client = StopClient::new(wire(offer()), expected(), 7).unwrap();
            assert!(!drive(&mut client, Duration::ZERO, 1).unwrap_err().may_have_been_sent);
        }
        #[test]
        fn disconnect_write_failure_and_silent_reply_preserve_unknown_outcome() {
            for case in 0..3 {
                let mut stream = wire(offer()); stream.fail_write = case == 1; stream.block_tail = case == 2;
                let mut client = StopClient::new(stream, expected(), 7).unwrap();
                let error = drive(&mut client, Duration::from_secs(1), 1).unwrap_err();
                assert!(error.may_have_been_sent, "case {case}"); assert!(client.outcome_unknown());
                let mut out = Vec::new(); assert!(finish(7, Err(error), &mut out).is_err());
                let json = strict_json::parse(&out, strict_json::Limits::default()).unwrap();
                assert_eq!(json.get("request_may_have_been_sent").unwrap().as_bool(), Some(true));
                assert_eq!(json.get("acknowledged").unwrap().as_bool(), Some(false));
            }
        }
        #[test]
        fn output_failure_retains_acknowledged_status_without_another_request() {
            struct Broken;
            impl Write for Broken {
                fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
                fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
            }
            let error = finish(7, Ok(parsed(3)), &mut Broken).unwrap_err();
            assert!(error.contains("StoppedDrained")); assert!(error.contains("output failed"));
            let error = finish(7, Err(Failure { detail: "lost reply".into(), may_have_been_sent: true }), &mut Broken).unwrap_err();
            assert!(error.contains("request_may_have_been_sent=true"));
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "stop_peer/deadline_tests.rs"]
mod deadline_tests;
