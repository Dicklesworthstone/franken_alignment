use fa_journal::{Error, Event, Identity, Journal, Limits};

fn identity() -> Identity {
    Identity { run: [1; 32], policy: [2; 32], total_units: 100 }
}

// Test-only framing of adversarial payloads. These deliberately carry valid
// checksums so that rejection exercises the decoder, not merely CRC checking.
fn frame(payload: &[u8], previous: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend(((12 + payload.len()) as u32).to_le_bytes());
    bytes.extend(1_u64.to_le_bytes());
    bytes.extend(previous.to_le_bytes());
    bytes.extend(payload);
    let mut crc = !0_u32;
    for byte in &bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 0 { crc >> 1 } else { (crc >> 1) ^ 0xedb8_8320 };
        }
    }
    bytes.extend((!crc).to_le_bytes());
    bytes
}

#[test]
fn encoder_matches_independently_generated_v1_wire_fixture() {
    // Generated with Python struct.pack (little endian) and zlib.crc32, rather
    // than the implementation's encoder or CRC helper.
    let hex = include_str!("fixtures/revoke_epoch_v1.hex").trim();
    let expected: Vec<u8> = (0..hex.len()).step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect();
    let mut journal = Journal::new(identity(), Limits::default()).unwrap();
    let mut actual = journal.header();
    actual.extend(journal.append(&Event::RevokeEpoch).unwrap());
    assert_eq!(actual, expected);
    let replay = Journal::replay(expected.as_slice(), identity(), journal.head(), Limits::default()).unwrap();
    assert_eq!(replay.rights().epoch(), 1);
}

#[test]
fn checksummed_noncanonical_events_are_rejected() {
    let journal = Journal::new(identity(), Limits::default()).unwrap();
    let mut invalid_bool = vec![5];
    invalid_bool.extend(1_u64.to_le_bytes());
    invalid_bool.push(2);
    let mut duplicate_set = vec![7];
    duplicate_set.extend(2_u32.to_le_bytes());
    duplicate_set.extend(1_u64.to_le_bytes());
    duplicate_set.extend(1_u64.to_le_bytes());
    let mut descending_set = vec![7];
    descending_set.extend(2_u32.to_le_bytes());
    descending_set.extend(2_u64.to_le_bytes());
    descending_set.extend(1_u64.to_le_bytes());
    let mut huge_set = vec![7];
    huge_set.extend(u32::MAX.to_le_bytes());
    for payload in [vec![99], vec![6, 0], invalid_bool, duplicate_set, descending_set, huge_set] {
        let mut bytes = journal.header();
        bytes.extend(frame(&payload, journal.head().checksum));
        assert!(matches!(Journal::replay(bytes.as_slice(), identity(), journal.head(), Limits::default()), Err(Error::Corrupt(_))));
    }
}

#[test]
fn valid_checksums_do_not_authorize_invalid_state_transitions() {
    let journal = Journal::new(identity(), Limits::default()).unwrap();
    let mut payload = vec![3]; // mark unknown without a prior dispatch
    payload.extend(1_u64.to_le_bytes());
    let mut bytes = journal.header();
    bytes.extend(frame(&payload, journal.head().checksum));
    assert!(matches!(Journal::replay(bytes.as_slice(), identity(), journal.head(), Limits::default()), Err(Error::Model(_))));
}

#[test]
fn smaller_recovery_limits_refuse_an_otherwise_valid_journal() {
    let mut journal = Journal::new(identity(), Limits::default()).unwrap();
    let genesis = journal.head();
    let mut bytes = journal.header();
    bytes.extend(journal.append(&Event::RevokeEpoch).unwrap());
    bytes.extend(journal.append(&Event::RevokeEpoch).unwrap());
    for limits in [
        Limits { max_records: 1, ..Limits::default() },
        Limits { max_total_bytes: bytes.len() as u64 - 1, ..Limits::default() },
    ] {
        assert!(matches!(Journal::replay(bytes.as_slice(), identity(), genesis, limits), Err(Error::Limit)));
    }
}
