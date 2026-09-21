use super::*;
use crate::action::consequence::oversight::joint_credibility::{JointPromotionPolicy, JointReplayBudget};

fn protocol() -> EvaluationProtocol {
    EvaluationProtocol { domain: 1, stratum: 2, period: 3, minimum_violation_origins: 2,
        minimum_benign_origins: 1, precision_floor: Fraction { numerator: 1, denominator: 2 },
        recall_floor: Fraction { numerator: 1, denominator: 2 },
        false_positive_ceiling: Fraction { numerator: 1, denominator: 2 }, false_stop_budget: 100 }
}
fn encoded(event: &CredibilityEvent) -> Vec<u8> {
    let mut w = Writer::new(1024);
    write(&mut w, event).unwrap();
    w.finish()
}
fn words(tag: u8, fields: &[u64]) -> Vec<u8> {
    let mut bytes = vec![tag];
    for value in fields { bytes.extend_from_slice(&value.to_be_bytes()); }
    bytes
}

#[test]
fn joint_bootstrap_vector_and_every_truncation_reconstruct_the_original_policy() {
    let joint = JointPromotionPolicy::new(71, 1,
        Fraction { numerator: 0, denominator: 1 }, Fraction { numerator: 0, denominator: 1 },
        JointReplayBudget { cases: 3, member_outcomes: 12 }).unwrap();
    let expected = words(5, &[1, 2, 3, 2, 1, 1, 2, 1, 2, 1, 2, 100, 71, 1, 0, 1, 0, 1, 3, 12]);
    assert_eq!(encoded(&CredibilityEvent::EnableJoint(protocol(), joint)), expected);
    for end in 0..expected.len() { assert!(read(&mut Reader::new(&expected[..end])).is_err()); }
    let mut reader = Reader::new(&expected);
    let restored = read(&mut reader).unwrap();
    reader.end().unwrap();
    let CredibilityEvent::EnableJoint(p, policy) = restored else { panic!("lost joint guard"); };
    assert_eq!(p, protocol());
    assert_eq!(policy, joint);
    for (word, value) in [(12, 0_u64), (13, 0), (15, 0), (17, 0), (14, 2), (18, u64::MAX), (19, u64::MAX)] {
        let mut invalid = expected.clone();
        invalid[1 + word * 8..1 + (word + 1) * 8].copy_from_slice(&value.to_be_bytes());
        assert!(read(&mut Reader::new(&invalid)).is_err(), "word {word}");
    }
    let mut unknown = expected;
    unknown[0] = 6;
    assert!(read(&mut Reader::new(&unknown)).is_err());
}

#[test]
fn legacy_bootstrap_promotion_and_withdrawal_vectors_are_byte_identical() {
    assert_eq!(encoded(&CredibilityEvent::Enable(protocol())),
        words(0, &[1, 2, 3, 2, 1, 1, 2, 1, 2, 1, 2, 100]));
    assert_eq!(encoded(&CredibilityEvent::Promote(FileCredibilityUpdate {
        operation: 7, expected_control_sequence: 8, expected_authority_epoch: 9,
        expected_evaluation_revision: 10,
    })), words(2, &[7, 8, 9, 10]));
    assert_eq!(encoded(&CredibilityEvent::WithdrawHeldOut(CredibilityWithdrawalRequest {
        operation: 11, expected_control_sequence: 12, expected_epoch: 13,
    })), words(4, &[11, 12, 13]));
}
