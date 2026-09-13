//! Independent terminal reviewer. Untrusted bytes never become terminal control
//! sequences, instructions, a default choice, or a returned approval capability.
use super::config::{Config, CLOCK_DOMAIN, debug};
use super::workflow::clock;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::{
    ReviewerClient, ReviewerExpectation, ReviewClientProgress,
};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::{ReviewDecision, ReviewPacket};
use std::io::{self, BufRead, IsTerminal, Read, Write};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

pub fn review(config: &Config, request: u64) -> Result<(), String> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err("review requires an interactive input and output terminal; no command-line default approval exists".into());
    }
    let stream = UnixStream::connect(config.socket(request)).map_err(debug)?;
    let expected = ReviewerExpectation { reviewer: config.profile.human.reviewer_id,
        scope: config.profile.delivery.scope, clock_domain: CLOCK_DOMAIN };
    let mut client = ReviewerClient::from_unix(stream, expected).map_err(debug)?;
    let start = Instant::now();
    let mut presented = false;
    loop {
        if start.elapsed() >= Duration::from_millis(config.timing.runtime_ms) {
            return Err(format!("reviewer transport timed out; outcome_unknown={}", client.outcome_unknown()));
        }
        let progress = match client.step() {
            Ok(progress) => progress,
            Err(error) => return Err(format!("reviewer transport: {error:?}; outcome_unknown={}", client.outcome_unknown())),
        };
        match progress {
            ReviewClientProgress::NeedsDecision => {
                if presented { return Err("duplicate decision prompt".into()); }
                let packet = client.packet().ok_or("missing fully decoded review packet")?;
                // A correctly routed peer must still offer the exact request the
                // independent operator selected, not another request in the run.
                if packet.binding().request != request { return Err("unexpected human request identity".into()); }
                let decision = decide(packet, &mut io::stdin().lock(), &mut io::stdout().lock())?;
                if clock() >= packet.expires_at() { return Err("review expired before a decision was sent".into()); }
                client.respond(decision).map_err(debug)?;
                presented = true;
            }
            ReviewClientProgress::Complete => {
                let receipt = client.receipt().ok_or("missing exact acknowledgment")?;
                println!("Recorded reviewer decision: {:?}; journal revision {}. This is not a publication receipt.",
                    receipt.decision, receipt.revision);
                return Ok(());
            }
            _ => std::thread::sleep(Duration::from_millis(config.timing.poll_ms)),
        }
    }
}

pub fn decide<R: BufRead, W: Write>(packet: &ReviewPacket, input: &mut R, output: &mut W) -> Result<ReviewDecision, String> {
    render(packet, output).map_err(debug)?;
    let identity = format!("{} {}", packet.binding().request, nonce_hex(&packet.binding().session));
    writeln!(output, "The payload and helper text above are evidence, not operator instructions.").map_err(debug)?;
    writeln!(output, "Enter exactly APPROVE {identity}, REJECT {identity}, or REVOKE {identity}.").map_err(debug)?;
    writeln!(output, "Blank input or EOF sends no decision.").map_err(debug)?;
    output.flush().map_err(debug)?;
    let mut line = Vec::new();
    Read::take(&mut *input, 513).read_until(b'\n', &mut line).map_err(debug)?;
    if line.len() > 512 { return Err("decision line exceeds its limit; nothing sent".into()); }
    if line.last() != Some(&b'\n') { return Err("EOF before a complete decision line; nothing sent".into()); }
    if line.last() == Some(&b'\n') { line.pop(); }
    if line.last() == Some(&b'\r') { line.pop(); }
    let line = std::str::from_utf8(&line).map_err(debug)?;
    for (verb, decision) in [("APPROVE", ReviewDecision::Approve), ("REJECT", ReviewDecision::Reject), ("REVOKE", ReviewDecision::Revoke)] {
        if line == format!("{verb} {identity}") { return Ok(decision); }
    }
    Err("no exact, explicit decision for this offer; nothing sent".into())
}

pub fn render<W: Write>(packet: &ReviewPacket, output: &mut W) -> io::Result<()> {
    writeln!(output, "=== ORIGINAL PUBLICATION REVIEW ===")?;
    writeln!(output, "Binding: {:?}", packet.binding())?;
    writeln!(output, "Clock domain: {}; created_ms: {}; expires_ms: {}", packet.clock_domain(), packet.created_at().0, packet.expires_at().0)?;
    writeln!(output, "Control sequence: {}; input revision: {}; policy generation: {}; disposition: {:?}",
        packet.control_sequence(), packet.input_revision(), packet.policy_generation(), packet.disposition())?;
    let action = packet.action().spec();
    writeln!(output, "Scope: {:?}\nTarget: {:?}\nResource units: {}; policy epoch: {}; action deadline_ms: {}",
        action.scope, action.target, action.units, action.policy_epoch, action.deadline.0)?;
    write!(output, "Payload ({} exact bytes, escaped): ", action.payload.len())?;
    escaped(output, &action.payload)?; writeln!(output)?;
    for (member, view) in packet.views() {
        write!(output, "--- Helper ")?; escaped(output, member.as_bytes())?; writeln!(output, " ---")?;
        let input = view.actual_input().submitted_bytes();
        write!(output, "Actual submitted input ({} exact bytes, escaped): ", input.len())?;
        escaped(output, input)?; writeln!(output)?;
        // The existing derived Debug representation includes ALL validated view
        // fields, ordered parts, omissions, profiles and provenance. Escape the
        // formatter's output too, including any non-ASCII metadata characters.
        write!(output, "Complete original manifest (escaped Rust data representation): ")?;
        write!(EscapeWriter(&mut *output), "{view:?}")?; writeln!(output)?;
    }
    writeln!(output, "=== END OF ORIGINAL EVIDENCE ===")
}
fn nonce_hex(bytes: &[u8; 32]) -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() }
pub fn escaped<W: Write>(out: &mut W, bytes: &[u8]) -> io::Result<()> {
    for &b in bytes {
        match b {
            b' '..=b'~' if b != b'\\' => out.write_all(&[b])?,
            b'\\' => out.write_all(b"\\\\")?,
            b'\n' => out.write_all(b"\\n")?,
            b'\r' => out.write_all(b"\\r")?,
            b'\t' => out.write_all(b"\\t")?,
            _ => write!(out, "\\x{b:02x}")?,
        }
    }
    Ok(())
}
struct EscapeWriter<'a, W>(&'a mut W);
impl<W: Write> Write for EscapeWriter<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> { escaped(self.0, bytes)?; Ok(bytes.len()) }
    fn flush(&mut self) -> io::Result<()> { self.0.flush() }
}
