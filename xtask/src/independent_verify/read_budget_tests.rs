use super::*;
use std::io::{self, Read};

fn decode_hex(value: &str) -> Vec<u8> {
    let value = value.trim();
    assert_eq!(value.len() % 2, 0);
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).unwrap())
        .collect()
}

fn golden(v2: bool) -> (Vec<u8>, ExpectedContext) {
    // These are the existing native wire-suite fixtures, not a new encoder or
    // a hand-authored success history for the read-budget implementation.
    let bytes = decode_hex(if v2 {
        include_str!("../../../crates/fa-journal/tests/fixtures/recovery_reserve_v2.hex")
    } else {
        include_str!("../../../crates/fa-journal/tests/fixtures/revoke_epoch_v1.hex")
    });
    let expected = ExpectedContext {
        run: [1; 32],
        policy: [2; 32],
        total_units: 100,
        limits: Limits {
            max_frame_bytes: 1024,
            max_records: 9,
            max_total_bytes: if v2 { 1024 } else { bytes.len() as u64 },
        },
        recovery: if v2 {
            Some(RecoveryReserve { records: 5, bytes: 160 })
        } else {
            None
        },
    };
    (bytes, expected)
}

struct Tracked<'a> {
    bytes: &'a [u8],
    offset: usize,
    chunk: usize,
    interruptions: u64,
    terminal_error: Option<io::ErrorKind>,
    calls: u64,
    terminal_calls: u64,
}

impl<'a> Tracked<'a> {
    fn one_byte(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            offset: 0,
            chunk: 1,
            interruptions: 0,
            terminal_error: None,
            calls: 0,
            terminal_calls: 0,
        }
    }
}

impl Read for Tracked<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        assert!(!out.is_empty(), "verification must not use a zero-byte EOF probe");
        self.calls += 1;
        if self.interruptions != 0 {
            self.interruptions -= 1;
            return Err(io::Error::from(io::ErrorKind::Interrupted));
        }
        if self.offset == self.bytes.len() {
            self.terminal_calls += 1;
            return match self.terminal_error {
                Some(kind) => Err(io::Error::from(kind)),
                None => Ok(0),
            };
        }
        let count = out.len().min(self.chunk).min(self.bytes.len() - self.offset);
        out[..count].copy_from_slice(&self.bytes[self.offset..self.offset + count]);
        self.offset += count;
        Ok(count)
    }
}

struct InterruptedForever {
    calls: u64,
}

impl Read for InterruptedForever {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        self.calls += 1;
        Err(io::Error::from(io::ErrorKind::Interrupted))
    }
}

struct MustNotRead;

impl Read for MustNotRead {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        panic!("this input must remain unread")
    }
}

fn assert_exhausted(error: &VerifyError, input: Input, offset: u64, sequence: Option<u64>) {
    assert_eq!(error.kind, ErrorKind::ReadBudgetExhausted);
    assert_eq!(error.kind_name(), "read_budget_exhausted");
    assert_eq!(error.location, Location { input, offset, sequence });
    assert_eq!(error.io_kind, None);
}

#[test]
fn exact_allowance_accepts_native_formats_but_one_more_interruption_refuses() {
    for v2 in [false, true] {
        let (bytes, expected) = golden(v2);
        let interruptions = 3;
        // One call per byte from each input, plus one EOF probe per input and
        // the actual Interrupted calls. This is independent of parser chunking.
        let calls = 2 * bytes.len() as u64 + 2 + interruptions;
        let mut journal = Tracked::one_byte(&bytes);
        journal.interruptions = interruptions;
        let mut anchor = Tracked::one_byte(&bytes);
        let mut budget = ReadBudget::new(calls);
        let report = verify_with_read_budget(&mut journal, &mut anchor, expected, &mut budget)
            .unwrap();
        assert_eq!(report, verify(bytes.as_slice(), bytes.as_slice(), expected).unwrap());
        assert_eq!(report.final_bytes, bytes.len() as u64);
        assert_eq!(report.prefix_bytes, bytes.len() as u64);
        assert_eq!(budget.spent_calls(), calls);
        assert_eq!(budget.remaining_calls(), 0);
        assert_eq!(journal.calls + anchor.calls, calls);
        assert_eq!(journal.terminal_calls, 1);
        assert_eq!(anchor.terminal_calls, 1);

        let mut journal = Tracked::one_byte(&bytes);
        journal.interruptions = interruptions + 1;
        let mut anchor = Tracked::one_byte(&bytes);
        let mut budget = ReadBudget::new(calls);
        let error = verify_with_read_budget(&mut journal, &mut anchor, expected, &mut budget)
            .unwrap_err();
        // All bytes have arrived, but the journal's final EOF remains unobserved.
        // A syntactically complete prefix cannot escape as a partial success.
        assert_exhausted(
            &error,
            Input::Journal,
            bytes.len() as u64,
            Some(report.final_head.sequence + 1),
        );
        assert_eq!(journal.offset, bytes.len());
        assert_eq!(journal.terminal_calls, 0);
        assert_eq!(budget.spent_calls(), journal.calls + anchor.calls);
        assert_eq!(budget.remaining_calls(), 0);
    }
}

#[test]
fn default_allowance_accepts_one_byte_maximum_length_inputs_and_interruptions() {
    let (bytes, expected) = golden(false);
    assert_eq!(expected.limits.max_total_bytes, bytes.len() as u64);
    let mut journal = Tracked::one_byte(&bytes);
    journal.interruptions = DEFAULT_INTERRUPTED_READ_ALLOWANCE;
    let mut anchor = Tracked::one_byte(&bytes);
    let report = verify(&mut journal, &mut anchor, expected).unwrap();
    assert_eq!(report.final_head.sequence, 1);
    assert_eq!(report.final_head.checksum, 0xec85_9e14);
    assert_eq!(report.accounting.epoch, 1);
    assert_eq!(
        journal.calls + anchor.calls,
        2 * bytes.len() as u64 + 2 + DEFAULT_INTERRUPTED_READ_ALLOWANCE,
    );
}

#[test]
fn finite_default_and_explicit_allowances_stop_infinite_interruptions() {
    let (_, expected) = golden(false);
    let mut journal = InterruptedForever { calls: 0 };
    let error = verify(&mut journal, MustNotRead, expected).unwrap_err();
    assert_exhausted(&error, Input::Journal, 0, None);
    assert_eq!(
        journal.calls,
        2 * expected.limits.max_total_bytes + 2 + DEFAULT_INTERRUPTED_READ_ALLOWANCE,
    );

    let mut journal = InterruptedForever { calls: 0 };
    let mut budget = ReadBudget::new(7);
    let error = verify_with_read_budget(&mut journal, MustNotRead, expected, &mut budget)
        .unwrap_err();
    assert_exhausted(&error, Input::Journal, 0, None);
    assert_eq!(journal.calls, 7);
    assert_eq!(budget.spent_calls(), 7);
    assert_eq!(budget.remaining_calls(), 0);

    let (bytes, expected) = golden(false);
    let mut journal = Tracked::one_byte(&bytes);
    journal.chunk = usize::MAX;
    let mut anchor = InterruptedForever { calls: 0 };
    let mut budget = ReadBudget::new(7);
    let error = verify_with_read_budget(&mut journal, &mut anchor, expected, &mut budget)
        .unwrap_err();
    assert_exhausted(&error, Input::Anchor, 0, None);
    assert_eq!(journal.offset, 84);
    assert_eq!(journal.calls + anchor.calls, 7);
    assert!(anchor.calls > 0);
    assert_eq!(budget.spent_calls(), 7);
    assert_eq!(budget.remaining_calls(), 0);
}

#[test]
fn journal_and_anchor_share_one_allowance_including_the_anchor_eof_probe() {
    let (bytes, expected) = golden(false);
    for anchor_calls in [7, 84] {
        let mut journal = Tracked::one_byte(&bytes);
        let mut anchor = Tracked::one_byte(&bytes[..84]);
        let calls = 84 + anchor_calls;
        let mut budget = ReadBudget::new(calls);
        let error = verify_with_read_budget(&mut journal, &mut anchor, expected, &mut budget)
            .unwrap_err();
        assert_exhausted(&error, Input::Anchor, anchor_calls, None);
        assert_eq!(journal.calls, 84);
        assert_eq!(anchor.calls, anchor_calls);
        assert_eq!(anchor.terminal_calls, 0);
        assert_eq!(budget.spent_calls(), calls);
        assert_eq!(budget.remaining_calls(), 0);
    }
}

#[test]
fn record_body_exhaustion_keeps_the_actual_journal_offset_and_sequence() {
    let (bytes, expected) = golden(false);
    let mut journal = Tracked::one_byte(&bytes);
    let mut anchor = Tracked::one_byte(&bytes[..84]);
    // Both headers, the retained-prefix EOF, then the four-byte frame length.
    let calls = 2 * 84 + 1 + 4;
    let mut budget = ReadBudget::new(calls);
    let error = verify_with_read_budget(&mut journal, &mut anchor, expected, &mut budget)
        .unwrap_err();
    assert_exhausted(&error, Input::Journal, 88, Some(1));
    assert_eq!(journal.offset, 88);
    assert_eq!(anchor.terminal_calls, 1);
    assert_eq!(budget.spent_calls(), journal.calls + anchor.calls);
    assert_eq!(budget.remaining_calls(), 0);
}

#[test]
fn successful_verification_and_failed_retries_never_refund_read_work() {
    let (bytes, expected) = golden(false);
    let needed = 2 * bytes.len() as u64 + 2;
    let mut budget = ReadBudget::new(2 * needed - 1);
    let mut journal = Tracked::one_byte(&bytes);
    let mut anchor = Tracked::one_byte(&bytes);
    verify_with_read_budget(&mut journal, &mut anchor, expected, &mut budget).unwrap();
    assert_eq!(budget.spent_calls(), needed);
    assert_eq!(budget.remaining_calls(), needed - 1);

    let mut journal = Tracked::one_byte(&bytes);
    let mut anchor = Tracked::one_byte(&bytes);
    let error = verify_with_read_budget(&mut journal, &mut anchor, expected, &mut budget)
        .unwrap_err();
    assert_exhausted(&error, Input::Journal, bytes.len() as u64, Some(2));
    assert_eq!(journal.calls + anchor.calls, needed - 1);
    assert_eq!(budget.spent_calls(), 2 * needed - 1);
    assert_eq!(budget.remaining_calls(), 0);

    let error = verify_with_read_budget(MustNotRead, MustNotRead, expected, &mut budget)
        .unwrap_err();
    assert_exhausted(&error, Input::Journal, 0, None);
    assert_eq!(budget.spent_calls(), 2 * needed - 1);
    assert_eq!(budget.remaining_calls(), 0);
}

#[test]
fn semantic_refusal_retains_its_spent_allowance_for_a_corrected_retry() {
    let (bytes, expected) = golden(false);
    let mut changed = bytes.clone();
    changed[10] ^= 1;
    let mut journal = Tracked::one_byte(&bytes);
    let mut anchor = Tracked::one_byte(&changed);
    let mut budget = ReadBudget::new(1000);
    let error = verify_with_read_budget(&mut journal, &mut anchor, expected, &mut budget)
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::AnchorMismatch);
    assert_eq!(error.location.offset, 10);
    let spent_before_retry = journal.calls + anchor.calls;
    assert!(spent_before_retry > 0);
    assert_eq!(budget.spent_calls(), spent_before_retry);

    let mut journal = Tracked::one_byte(&bytes);
    let mut anchor = Tracked::one_byte(&bytes);
    let report = verify_with_read_budget(&mut journal, &mut anchor, expected, &mut budget)
        .unwrap();
    assert_eq!(report.final_head.sequence, 1);
    let spent = spent_before_retry + journal.calls + anchor.calls;
    assert_eq!(budget.spent_calls(), spent);
    assert_eq!(budget.remaining_calls(), 1000 - spent);
}

#[test]
fn io_errors_and_truncated_eof_are_charged_before_returning_failure() {
    let (bytes, expected) = golden(false);
    let mut budget = ReadBudget::new(20);
    for terminal_error in [Some(io::ErrorKind::PermissionDenied), None] {
        let mut journal = Tracked::one_byte(&bytes[..7]);
        journal.terminal_error = terminal_error;
        let before = budget.spent_calls();
        let error = verify_with_read_budget(&mut journal, MustNotRead, expected, &mut budget)
            .unwrap_err();
        assert_eq!(
            error.kind,
            if terminal_error.is_some() {
                ErrorKind::Io
            } else {
                ErrorKind::TruncatedInput
            },
        );
        assert_eq!(
            error.location,
            Location { input: Input::Journal, offset: 7, sequence: None },
        );
        assert_eq!(error.io_kind, terminal_error);
        assert_eq!(journal.calls, 8);
        assert_eq!(journal.terminal_calls, 1);
        assert_eq!(budget.spent_calls(), before + 8);
        assert_eq!(budget.remaining_calls(), 20 - budget.spent_calls());
    }
    assert_eq!(budget.spent_calls(), 16);
    assert_eq!(budget.remaining_calls(), 4);
}

#[test]
fn invalid_context_spends_no_calls_and_precedes_exhaustion() {
    let (_, original) = golden(false);
    let mut invalid = original;
    invalid.run = [0; 32];
    let mut unsupported = original;
    unsupported.limits.max_total_bytes = u64::MAX;
    for calls in [0, 11] {
        let mut budget = ReadBudget::new(calls);
        for (expected, kind) in [
            (invalid, ErrorKind::InvalidContext),
            (unsupported, ErrorKind::UnsupportedProfile),
        ] {
            let error = verify_with_read_budget(MustNotRead, MustNotRead, expected, &mut budget)
                .unwrap_err();
            assert_eq!(error.kind, kind);
            assert_eq!(
                error.location,
                Location { input: Input::Context, offset: 0, sequence: None },
            );
            assert_eq!(budget.spent_calls(), 0);
            assert_eq!(budget.remaining_calls(), calls);
        }
    }
    let mut budget = ReadBudget::new(0);
    let error = verify_with_read_budget(MustNotRead, MustNotRead, original, &mut budget)
        .unwrap_err();
    assert_exhausted(&error, Input::Journal, 0, None);
    assert_eq!(budget.spent_calls(), 0);
}
