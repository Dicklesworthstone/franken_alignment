use super::*;
use std::io::{self, Read};

fn context() -> ExpectedContext {
    ExpectedContext {
        run: [1; 32],
        policy: [2; 32],
        total_units: 100,
        limits: Limits {
            max_frame_bytes: MAX_FRAME_BYTES,
            max_records: 100_000,
            max_total_bytes: MAX_INPUT_BYTES,
        },
        recovery: None,
    }
}

fn hex(value: &str) -> Vec<u8> {
    let value = value.trim();
    assert_eq!(value.len() % 2, 0);
    (0..value.len()).step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).unwrap())
        .collect()
}

fn v1_golden() -> Vec<u8> {
    // Existing independently generated fixture already checked by the native
    // wire suite. No native encoder/reducer is imported into this verifier.
    hex(include_str!("../../../crates/fa-journal/tests/fixtures/revoke_epoch_v1.hex"))
}

#[test]
fn crc_matches_published_ieee_check_vector_and_chunking() {
    assert_eq!(wire::crc32(&[b"123456789"]), 0xcbf4_3926);
    assert_eq!(wire::crc32(&[b"123", b"", b"456789"]), 0xcbf4_3926);
    assert_eq!(wire::crc32(&[b""]), 0);
}

#[test]
fn existing_v1_golden_checks_prefix_suffix_and_repeat_read() {
    let bytes = v1_golden();
    let report = verify(bytes.as_slice(), &bytes[..84], context()).unwrap();
    assert_eq!(report.prefix_bytes, 84);
    assert_eq!(report.prefix_head.sequence, 0);
    assert_eq!(report.final_head.sequence, 1);
    assert_eq!(report.final_head.checksum, 0xec85_9e14);
    assert_eq!(report.final_bytes, bytes.len() as u64);
    assert_eq!(report.accounting.epoch, 1);
    assert_eq!(report.accounting.available_units, 100);
    assert_eq!(report, verify(bytes.as_slice(), &bytes[..84], context()).unwrap());
    for property in ["authentication", "full_permit_law", "external_occurrence", "capture_completeness"] {
        assert_eq!(report.assurances().iter().find(|claim| claim.property == property).unwrap().status, AssuranceStatus::Unknown);
    }
}

#[test]
fn existing_v2_golden_requires_exact_selected_capacity() {
    let bytes = hex(include_str!("../../../crates/fa-journal/tests/fixtures/recovery_reserve_v2.hex"));
    let mut expected = context();
    expected.limits = Limits { max_frame_bytes: 1024, max_records: 9, max_total_bytes: 1024 };
    expected.recovery = Some(RecoveryReserve { records: 5, bytes: 160 });
    let report = verify(bytes.as_slice(), bytes.as_slice(), expected).unwrap();
    assert_eq!(report.format_version, 2);
    assert_eq!(report.prefix_bytes, 120);
    expected.limits.max_total_bytes += 1;
    assert_eq!(verify(bytes.as_slice(), bytes.as_slice(), expected).unwrap_err().kind, ErrorKind::ContextMismatch);
}

#[test]
fn every_partial_anchor_is_rejected_except_real_record_boundaries() {
    let bytes = v1_golden();
    for end in 0..=bytes.len() {
        let result = verify(bytes.as_slice(), &bytes[..end], context());
        assert_eq!(result.is_ok(), end == 84 || end == bytes.len(), "anchor length {end}");
        if let Ok(report) = result {
            assert_eq!(report.prefix_bytes, end as u64);
        }
    }
    let error = verify(&bytes[..84], bytes.as_slice(), context()).unwrap_err();
    assert_eq!(error.kind, ErrorKind::AnchorBeyondJournal);
    assert_eq!(error.location.input, Input::Anchor);
    assert_eq!(error.location.offset, 84);
}

struct Unreadable;
impl Read for Unreadable {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        panic!("invalid configuration must fail before either input is read")
    }
}

#[test]
fn invalid_or_unsupported_configuration_is_refused_before_io() {
    let original = context();
    let mut candidates = Vec::new();
    let mut candidate = original;
    candidate.run = [0; 32];
    candidates.push(candidate);
    candidate = original;
    candidate.policy = [0; 32];
    candidates.push(candidate);
    candidate = original;
    candidate.limits.max_frame_bytes = 12;
    candidates.push(candidate);
    candidate = original;
    candidate.limits.max_frame_bytes = MAX_FRAME_BYTES + 1;
    candidates.push(candidate);
    candidate = original;
    candidate.limits.max_records = 0;
    candidates.push(candidate);
    candidate = original;
    candidate.limits.max_records = MAX_RECORDS + 1;
    candidates.push(candidate);
    candidate = original;
    candidate.limits.max_total_bytes = 83;
    candidates.push(candidate);
    candidate = original;
    candidate.recovery = Some(RecoveryReserve { records: 1, bytes: 20 });
    candidates.push(candidate);
    candidate = original;
    candidate.recovery = Some(RecoveryReserve { records: u64::MAX, bytes: u64::MAX });
    candidates.push(candidate);
    for candidate in candidates {
        let error = verify(Unreadable, Unreadable, candidate).unwrap_err();
        assert_eq!(error.kind, ErrorKind::InvalidContext);
        assert_eq!(error.location.input, Input::Context);
    }
    candidate = original;
    candidate.limits.max_total_bytes = MAX_INPUT_BYTES + 1;
    assert_eq!(verify(Unreadable, Unreadable, candidate).unwrap_err().kind, ErrorKind::UnsupportedProfile);
}

struct ExactPrefixOnly {
    bytes: Vec<u8>,
    offset: usize,
}
impl Read for ExactPrefixOnly {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        assert!(self.offset < self.bytes.len(), "oversize frame must be rejected before its body is read");
        let count = out.len().min(self.bytes.len() - self.offset);
        out[..count].copy_from_slice(&self.bytes[self.offset..self.offset + count]);
        self.offset += count;
        Ok(count)
    }
}

#[test]
fn attacker_frame_length_is_checked_before_payload_read_or_allocation() {
    let header = v1_golden()[..84].to_vec();
    for length in [0_u32, 12, MAX_FRAME_BYTES + 1, u32::MAX] {
        let mut bytes = header.clone();
        bytes.extend_from_slice(&length.to_le_bytes());
        let input = ExactPrefixOnly { bytes, offset: 0 };
        let error = verify(input, header.as_slice(), context()).unwrap_err();
        assert_eq!(error.kind, ErrorKind::LimitExceeded);
        assert_eq!(error.location.offset, 84);
        assert_eq!(error.location.sequence, Some(1));
    }
}

struct FailureAfter<'a> {
    bytes: &'a [u8],
}
impl Read for FailureAfter<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.bytes.is_empty() {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        let count = out.len().min(self.bytes.len());
        out[..count].copy_from_slice(&self.bytes[..count]);
        self.bytes = &self.bytes[count..];
        Ok(count)
    }
}

#[test]
fn input_errors_preserve_origin_offset_and_never_return_a_partial_report() {
    let bytes = v1_golden();
    let header = &bytes[..84];
    let journal_error = verify(FailureAfter { bytes: header }, header, context()).unwrap_err();
    assert_eq!(journal_error.kind, ErrorKind::Io);
    assert_eq!(journal_error.location.input, Input::Journal);
    assert_eq!(journal_error.location.offset, 84);
    assert_eq!(journal_error.io_kind, Some(io::ErrorKind::PermissionDenied));
    let anchor_error = verify(bytes.as_slice(), FailureAfter { bytes: header }, context()).unwrap_err();
    assert_eq!(anchor_error.kind, ErrorKind::Io);
    assert_eq!(anchor_error.location.input, Input::Anchor);
    assert_eq!(anchor_error.location.offset, 84);
}

fn effect(tag: u8, id: u64, units: u64, payload: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    out.extend_from_slice(&id.to_le_bytes());
    out.extend_from_slice(&units.to_le_bytes());
    for field in [b"principal".as_slice(), b"resolved-target".as_slice(), payload] {
        out.extend_from_slice(&(field.len() as u32).to_le_bytes());
        out.extend_from_slice(field);
    }
    out
}

#[test]
fn reducer_releases_effect_bytes_but_keeps_unknown_liability_and_used_ids() {
    let at = Location { input: Input::Journal, offset: 100, sequence: Some(1) };
    let mut ledger = ledger::Ledger::new(10);
    let reserve = effect(1, 1, 5, &[42; 4096]);
    ledger.apply(wire::decode_event(&reserve, 100, 1).unwrap(), at).unwrap();
    assert!(ledger.retained_effect_bytes() >= 4096);
    let dispatch = effect(2, 1, 5, &[42; 4096]);
    ledger.apply(wire::decode_event(&dispatch, 100, 2).unwrap(), at).unwrap();
    assert_eq!(ledger.retained_effect_bytes(), 0);
    ledger.apply(wire::Event::MarkUnknown { id: 1 }, at).unwrap();
    let reserve_two = effect(1, 2, 2, &[19; 4096]);
    ledger.apply(wire::decode_event(&reserve_two, 100, 4).unwrap(), at).unwrap();
    assert!(ledger.retained_effect_bytes() >= 4096);
    ledger.apply(wire::Event::Reset { keep: vec![] }, at).unwrap();
    assert_eq!(ledger.retained_effect_bytes(), 0);
    let duplicate = effect(1, 2, 1, &[]);
    assert_eq!(ledger.apply(wire::decode_event(&duplicate, 100, 6).unwrap(), at).unwrap_err().kind, ErrorKind::DuplicateReservation);
    ledger.apply(wire::Event::Revoke, at).unwrap();
    let accounting = ledger.accounting();
    assert_eq!(accounting.available_units, 5);
    assert_eq!(accounting.unknown_units, 5);
    assert_eq!(accounting.unknown_count, 1);
    assert_eq!(accounting.aborted_count, 1);
    assert_eq!(accounting.reservations, 2);
    assert_eq!(accounting.incident_count, 1);
    assert_eq!(accounting.epoch, 1);
}
