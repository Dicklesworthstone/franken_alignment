//! Bounded untrusted decoding plus byte-stable legacy presentation.
use super::*;
use crate::ReadWitness;
use crate::action::{MAX_REQUIRED_WITNESSES, MAX_WITNESS_BYTES};

fn encode(witnesses: &[ReadWitness]) -> Result<Vec<u8>, Error> {
    let mut writer = Writer::new(MAX_REVIEW_BYTES);
    super::witnesses::write(&mut writer, witnesses)?;
    Ok(writer.finish())
}
fn decode(bytes: &[u8]) -> Result<Vec<ReadWitness>, Error> {
    let mut reader = Reader::new(bytes);
    let witnesses = super::witnesses::read(&mut reader)?;
    reader.end()?;
    Ok(witnesses)
}
fn exact(key: u64, bytes: Vec<u8>) -> ReadWitness {
    ReadWitness::Exact { key, value: Some(bytes) }
}

#[test]
fn ordered_binary_values_absence_empty_values_and_negative_domains_are_distinct() {
    let witnesses = vec![exact(9, vec![0, 255, b'\n']),
        ReadWitness::Exact { key: 4, value: None }, exact(4, Vec::new()),
        ReadWitness::EmptyRange { start: 20, end: 30 }, exact(9, vec![0, 255, b'\n'])];
    let bytes = encode(&witnesses).unwrap();
    assert_eq!(decode(&bytes).unwrap(), witnesses);
    assert_eq!(encode(&decode(&bytes).unwrap()).unwrap(), bytes);
    assert_ne!(encode(&[ReadWitness::Exact { key: 4, value: None }]).unwrap(),
        encode(&[exact(4, Vec::new())]).unwrap());
}

#[test]
fn witness_count_and_total_value_bytes_have_inclusive_native_limits() {
    let at_count = vec![ReadWitness::Exact { key: 0, value: None }; MAX_REQUIRED_WITNESSES];
    assert_eq!(decode(&encode(&at_count).unwrap()).unwrap(), at_count);
    let too_many = vec![ReadWitness::Exact { key: 0, value: None }; MAX_REQUIRED_WITNESSES + 1];
    assert_eq!(encode(&too_many), Err(Error::Limit));
    let at_bytes = vec![exact(1, vec![0; MAX_WITNESS_BYTES / 2]),
        exact(2, vec![255; MAX_WITNESS_BYTES / 2])];
    assert_eq!(decode(&encode(&at_bytes).unwrap()).unwrap(), at_bytes);
    let mut over = at_bytes.clone(); over.push(exact(3, vec![1]));
    assert_eq!(encode(&over), Err(Error::Limit));
    let mut bytes = encode(&at_bytes).unwrap();
    bytes[..4].copy_from_slice(&3_u32.to_be_bytes());
    // Each value is individually admissible. Only the cumulative limit fails.
    bytes.push(1); bytes.extend_from_slice(&3_u64.to_be_bytes());
    bytes.extend_from_slice(&1_u32.to_be_bytes()); bytes.push(1);
    assert_eq!(decode(&bytes), Err(Error::Limit));
}

#[test]
fn untrusted_counts_lengths_tags_and_ranges_refuse_without_normalization() {
    assert_eq!(decode(&u32::MAX.to_be_bytes()), Err(Error::Limit));
    let mut bytes = 1_u32.to_be_bytes().to_vec(); bytes.push(3);
    assert_eq!(decode(&bytes), Err(Error::InvalidInput));
    bytes[4] = 1; bytes.extend_from_slice(&7_u64.to_be_bytes());
    bytes.extend_from_slice(&u32::MAX.to_be_bytes());
    assert_eq!(decode(&bytes), Err(Error::Limit));
    for (start, end) in [(10_u64, 10_u64), (11, 10)] {
        assert_eq!(encode(&[ReadWitness::EmptyRange { start, end }]), Err(Error::InvalidInput));
        let mut bytes = 1_u32.to_be_bytes().to_vec(); bytes.push(2);
        bytes.extend_from_slice(&start.to_be_bytes()); bytes.extend_from_slice(&end.to_be_bytes());
        assert_eq!(decode(&bytes), Err(Error::InvalidInput));
    }
}

#[test]
fn every_truncated_witness_record_and_trailing_byte_is_rejected() {
    let bytes = encode(&[exact(1, vec![0, 255]), ReadWitness::Exact { key: 2, value: None },
        ReadWitness::EmptyRange { start: 10, end: 20 }]).unwrap();
    for end in 0..bytes.len() { assert!(decode(&bytes[..end]).is_err(), "prefix {end}"); }
    assert!(decode(&bytes).is_ok());
    let mut trailing = bytes; trailing.push(0);
    assert_eq!(decode(&trailing), Err(Error::InvalidInput));
}

#[test]
fn both_offer_versions_obey_the_same_frame_ceiling_and_unknown_versions_refuse() {
    for version in [OFFER, WITNESSED_OFFER] {
        let mut header = [0; OFFER_HEADER_BYTES]; header[..8].copy_from_slice(version);
        header[8..].copy_from_slice(&(MAX_REVIEW_BYTES as u64).to_be_bytes());
        assert_eq!(offer_frame_len(&header), Ok(MAX_REVIEW_BYTES));
        header[8..].copy_from_slice(&((MAX_REVIEW_BYTES + 1) as u64).to_be_bytes());
        assert_eq!(offer_frame_len(&header), Err(Error::Limit));
        header[7] = 3;
        assert_eq!(offer_frame_len(&header), Err(Error::InvalidInput));
    }
}
