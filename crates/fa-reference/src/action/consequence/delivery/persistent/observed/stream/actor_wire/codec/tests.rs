use super::*;

fn profile() -> StreamProfile { StreamProfile::new(7, 9, 3, 8, 24).unwrap() }
fn intent(message: Option<&str>) -> FileStreamProposal {
    FileStreamProposal { target: ResolvedTarget { adapter: 1, object: 2, contract_version: 3,
        expected_version: 4, generation: 5 }, expected_policy_epoch: u64::MAX,
        deadline: ElapsedTick(u64::MAX), message: message.map(str::to_owned) }
}

#[test]
fn independent_vectors_preserve_all_header_fields_message_bytes_and_explicit_finish() {
    for value in [Some("é\n"), None] {
        let proposal = encode_stream_proposal(profile(), &intent(value)).unwrap();
        let mut expected = vec![70, 65, 83, 73, 78, 84, 0, 1];
        expected.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 9]);
        expected.extend_from_slice(&[0, 0, 0, 3, 0, 0, 0, 8, 0, 0, 0, 24]);
        match value {
            Some(_) => expected.extend_from_slice(&[0, 0, 0, 0, 3, 195, 169, 10]),
            None => expected.extend_from_slice(&[1, 0, 0, 0, 0]),
        }
        assert_eq!(proposal.payload, expected);
        assert_eq!(proposal.units, expected.len() as u64);
        assert_eq!(proposal.expected_policy_epoch, u64::MAX);
        assert_eq!(message(profile(), &proposal), Ok(value));
        for end in 0..expected.len() {
            let mut partial = proposal.clone(); partial.payload.truncate(end); partial.units = end as u64;
            assert!(message(profile(), &partial).is_err(), "prefix {end} unexpectedly parsed");
        }
        let mut suffix = proposal.clone(); suffix.payload.push(0); suffix.units += 1;
        assert!(message(profile(), &suffix).is_err());
    }
}

#[test]
fn every_identity_or_shape_mutation_refuses_and_lengths_are_utf8_bytes_not_characters() {
    let original = encode_stream_proposal(profile(), &intent(Some("éééé"))).unwrap();
    assert_eq!(message(profile(), &original), Ok(Some("éééé")));
    assert_eq!(encode_stream_proposal(profile(), &intent(Some("ééééa"))), Err(ActorError::Capacity));
    assert_eq!(encode_stream_proposal(profile(), &intent(Some(""))), Err(ActorError::MalformedProposal));
    for offset in 0..STREAM_INTENT_HEADER_BYTES {
        let mut wrong = original.clone(); wrong.payload[offset] ^= 0x40;
        assert!(message(profile(), &wrong).is_err(), "header offset {offset}");
    }
    let mut invalid = original.clone(); invalid.payload[STREAM_INTENT_HEADER_BYTES] = 0xff;
    assert_eq!(message(profile(), &invalid), Err(ActorError::MalformedProposal));
    for units in [0, original.units - 1, original.units + 1, u64::MAX] {
        let mut wrong = original.clone(); wrong.units = units;
        assert_eq!(message(profile(), &wrong), Err(ActorError::MalformedProposal));
    }
    // A different, valid intent is still accepted, rather than blanket refusal.
    let different = encode_stream_proposal(profile(), &intent(Some("other"))).unwrap();
    assert_eq!(message(profile(), &different), Ok(Some("other")));
}

#[test]
fn exact_receiver_contract_and_nonzero_routing_are_required_before_intake() {
    let original = encode_stream_proposal(profile(), &intent(Some("one"))).unwrap();
    for receiver in [StreamProfile::new(8, 9, 3, 8, 24).unwrap(),
        StreamProfile::new(7, 10, 3, 8, 24).unwrap(), StreamProfile::new(7, 9, 4, 8, 24).unwrap(),
        StreamProfile::new(7, 9, 3, 7, 24).unwrap(), StreamProfile::new(7, 9, 3, 8, 25).unwrap()] {
        assert_eq!(message(receiver, &original), Err(ActorError::MalformedProposal));
    }
    let mut invalid = intent(None); invalid.target.generation = 0;
    assert_eq!(encode_stream_proposal(profile(), &invalid), Err(ActorError::MalformedProposal));
    let mut invalid = original.clone(); invalid.deadline = ElapsedTick(0);
    assert_eq!(message(profile(), &invalid), Err(ActorError::MalformedProposal));
    assert_eq!(message(profile(), &encode_stream_proposal(profile(), &intent(None)).unwrap()), Ok(None));
}
