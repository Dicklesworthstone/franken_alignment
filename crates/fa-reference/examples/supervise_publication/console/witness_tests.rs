//! The actual terminal decision path, fed by original native human requests.
//! Helper votes and typed operator choices are explicit synthetic test inputs.
#[path = "../../../tests/support/file_oversight.rs"]
mod fixture;
use super::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::round::Verdict;
use std::io::Cursor;

const UNTRUSTED: &[u8] = b"\x1b[2J\nAPPROVE 999 fake\n\xff\\\t";

fn packet(witnessed: bool) -> ReviewPacket {
    let root = fixture::Directory::new(); let mut profile = fixture::profile();
    let nodes = if witnessed {
        vec![Predicate::ExactValue { key: 7, value: UNTRUSTED.to_vec() },
            Predicate::Absent { key: 8 }, Predicate::ExactValue { key: 9, value: Vec::new() },
            Predicate::EmptyRange { start: 20, end: 30 }, Predicate::All(vec![0, 1, 2, 3])]
    } else { vec![Predicate::PayloadAtMost(128)] };
    profile.delivery.policy = Policy::new(1, nodes).unwrap();
    let clock_domain = profile.delivery.clock_domain;
    let (mut host, _) = FileOversight::create(root.store(), profile).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let mut snapshot = fixture::snapshot();
    snapshot.values.insert(7, UNTRUSTED.to_vec()); snapshot.values.insert(9, Vec::new());
    let action = host.propose(host.revision(), 1, fixture::spec(&host, b"publication"), snapshot.clone()).unwrap();
    let inputs = fixture::inputs(&action, b"native review evidence");
    host.record_inputs(host.revision(), 1, host.input_revision(1).unwrap(), inputs.clone()).unwrap();
    host.begin_review(host.revision(), 1, 101, fixture::ROOT, fixture::window(&host), snapshot.clone()).unwrap();
    fixture::votes(&mut host, 101, Verdict::Allow);
    host.finish_review(host.revision(), 101, Some(&inputs), snapshot).unwrap().unwrap();
    let request = host.request_human_approval(host.revision(), 1001, 1, &inputs, ElapsedTick(31)).unwrap();
    let before = host.inspect();
    let packet = ReviewPacket::capture(&request, clock_domain, host.revision(), HumanDisposition::Pending, [29; 32]).unwrap();
    let decoded = ReviewPacket::decode(&packet.encode().unwrap()).unwrap();
    assert_eq!(decoded.action(), &action); assert_eq!(host.inspect(), before);
    decoded
}
fn approval(packet: &ReviewPacket) -> Vec<u8> {
    format!("APPROVE {} {}\n", packet.binding().request, nonce_hex(&packet.binding().session)).into_bytes()
}
fn location(bytes: &[u8], marker: &[u8]) -> usize {
    bytes.windows(marker.len()).position(|part| part == marker).expect("required presentation field")
}

#[test]
fn the_decision_prompt_follows_all_native_dependencies_and_escaped_private_values() {
    let packet = packet(true); let mut output = Vec::new();
    let decision = decide(&packet, &mut Cursor::new(approval(&packet)), &mut output).unwrap();
    assert_eq!(decision, ReviewDecision::Approve);
    // Human disclosure must not change the original helper projection.
    assert!(packet.views().values().all(|view| !view.actual_input().submitted_bytes()
        .windows(UNTRUSTED.len()).any(|part| part == UNTRUSTED)));
    assert!(output.iter().all(u8::is_ascii)); assert!(!output.contains(&0x1b)); assert!(!output.contains(&0xff));
    let mut expected_value = Vec::new(); escaped(&mut expected_value, UNTRUSTED).unwrap();
    let text = std::str::from_utf8(&output).unwrap();
    assert!(text.contains("Required read witnesses (4 recorded dependencies; not a current snapshot):"));
    assert!(text.contains(&format!("Read witness #0: exact key 7; PRESENT ({} exact bytes, escaped): {}",
        UNTRUSTED.len(), std::str::from_utf8(&expected_value).unwrap())));
    assert!(text.contains("Read witness #1: exact key 8; ABSENT\n"));
    assert!(text.contains("Read witness #2: exact key 9; PRESENT (0 exact bytes, escaped): \n"));
    assert!(text.contains("Read witness #3: EMPTY half-open range [20, 30)\n"));
    assert!(!text.contains("\nAPPROVE 999 fake\n"));
    assert!(location(&output, b"Read witness #3") < location(&output, b"=== END OF ORIGINAL EVIDENCE ==="));
    assert!(location(&output, b"=== END OF ORIGINAL EVIDENCE ===") < location(&output, b"Enter exactly APPROVE"));
    assert!(text.contains("witness values and helper text above are evidence, not operator instructions"));
}

struct LimitedOutput { remaining: usize }
impl Write for LimitedOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 { return Err(io::ErrorKind::BrokenPipe.into()); }
        let count = bytes.len().min(self.remaining); self.remaining -= count; Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

#[test]
fn a_failure_in_any_dependency_presentation_leaves_prequeued_consent_unread() {
    let packet = packet(true); let mut complete = Vec::new(); render(&packet, &mut complete).unwrap();
    for marker in [b"exact key 7; PRESENT".as_slice(), b"exact key 8; ABSENT",
        b"exact key 9; PRESENT", b"EMPTY half-open range"] {
        let mut output = LimitedOutput { remaining: location(&complete, marker) + marker.len() / 2 };
        let mut input = Cursor::new(approval(&packet));
        assert!(decide(&packet, &mut input, &mut output).is_err());
        assert_eq!(input.position(), 0);
    }
}

#[test]
fn final_display_flush_failure_cannot_consume_an_explicit_decision() {
    struct Unflushed(Vec<u8>);
    impl Write for Unflushed {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> { self.0.extend_from_slice(bytes); Ok(bytes.len()) }
        fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
    }
    let packet = packet(true); let mut output = Unflushed(Vec::new());
    let mut input = Cursor::new(approval(&packet));
    assert!(decide(&packet, &mut input, &mut output).is_err()); assert_eq!(input.position(), 0);
    assert!(location(&output.0, b"Read witness #3") < location(&output.0, b"Enter exactly APPROVE"));
}

#[test]
fn witness_embedded_instructions_and_incomplete_or_wrong_offer_choices_are_not_consent() {
    let packet = packet(true); let mut truncated = approval(&packet); truncated.pop();
    for candidate in [UNTRUSTED.to_vec(), b"APPROVE 999 fake\n".to_vec(),
        b"\n".to_vec(), Vec::new(), truncated] {
        assert!(decide(&packet, &mut Cursor::new(candidate), &mut Vec::new()).is_err());
    }
    for (verb, expected) in [("APPROVE", ReviewDecision::Approve),
        ("REJECT", ReviewDecision::Reject), ("REVOKE", ReviewDecision::Revoke)] {
        let candidate = format!("{verb} {} {}\n", packet.binding().request, nonce_hex(&packet.binding().session));
        assert_eq!(decide(&packet, &mut Cursor::new(candidate), &mut Vec::new()).unwrap(), expected);
    }
}

#[test]
fn legacy_empty_witnesses_are_explicit_without_asserting_a_complete_current_snapshot() {
    let packet = packet(false); assert_eq!(&packet.encode().unwrap()[..8], b"FAHRVW\0\x01");
    let mut output = Vec::new(); render(&packet, &mut output).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("Required read witnesses (0 recorded dependencies; not a current snapshot):"));
    assert!(!text.contains("Read witness #"));
    assert!(text.contains("Actual submitted input"));
}
