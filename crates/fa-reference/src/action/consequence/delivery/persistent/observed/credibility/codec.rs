//! Fixed-size inputs to original evaluation, not serialized scores or approvals.
use super::{CredibilityEvent, FileCredibilityUpdate};
use super::super::super::codec::shared::{Reader, Writer};
use crate::action::consequence::oversight::credibility::{Assessment, EvaluationProtocol, Fraction, GroundTruth};
use crate::Error;
use super::super::super::credibility::codec as held_out;
use super::super::super::credibility::CredibilityWithdrawalRequest;

pub(in super::super) fn write(w: &mut Writer, event: &CredibilityEvent) -> Result<(), Error> {
    match event {
        CredibilityEvent::ActivateHeldOut(request) => {
            w.u8(3)?; w.blob(&held_out::encode_activation(request)?)?;
        }
        CredibilityEvent::WithdrawHeldOut(request) => {
            w.u8(4)?; w.u64(request.operation)?;
            w.u64(request.expected_control_sequence)?; w.u64(request.expected_epoch)?;
        }
        CredibilityEvent::Enable(p) => {
            w.u8(0)?;
            for value in [p.domain, p.stratum, p.period, p.minimum_violation_origins, p.minimum_benign_origins,
                p.precision_floor.numerator, p.precision_floor.denominator, p.recall_floor.numerator,
                p.recall_floor.denominator, p.false_positive_ceiling.numerator,
                p.false_positive_ceiling.denominator, p.false_stop_budget] { w.u64(value)?; }
        }
        CredibilityEvent::Assess(round, a) => {
            w.u8(1)?; w.u64(*round)?; w.u64(a.origin)?; w.raw(&a.evidence_id)?;
            w.u8(match a.truth { GroundTruth::Benign => 0, GroundTruth::Violation => 1, GroundTruth::Censored => 2 })?;
        }
        CredibilityEvent::Promote(u) => {
            u.validate()?; w.u8(2)?;
            for value in [u.operation, u.expected_control_sequence, u.expected_authority_epoch,
                u.expected_evaluation_revision] { w.u64(value)?; }
        }
    }
    Ok(())
}
pub(in super::super) fn read(r: &mut Reader<'_>) -> Result<CredibilityEvent, Error> {
    Ok(match r.u8()? {
        3 => CredibilityEvent::ActivateHeldOut(Box::new(held_out::decode_activation(
            r.blob(held_out::MAX_ACTIVATION_BYTES)?)?)),
        4 => CredibilityEvent::WithdrawHeldOut(CredibilityWithdrawalRequest {
            operation: r.u64()?, expected_control_sequence: r.u64()?, expected_epoch: r.u64()?,
        }),
        0 => CredibilityEvent::Enable(EvaluationProtocol {
            domain: r.u64()?, stratum: r.u64()?, period: r.u64()?,
            minimum_violation_origins: r.u64()?, minimum_benign_origins: r.u64()?,
            precision_floor: Fraction { numerator: r.u64()?, denominator: r.u64()? },
            recall_floor: Fraction { numerator: r.u64()?, denominator: r.u64()? },
            false_positive_ceiling: Fraction { numerator: r.u64()?, denominator: r.u64()? },
            false_stop_budget: r.u64()?,
        }),
        1 => {
            let round = r.u64()?; let origin = r.u64()?;
            let evidence_id = r.take(32)?.try_into().map_err(|_| Error::Incomplete)?;
            let truth = match r.u8()? { 0 => GroundTruth::Benign, 1 => GroundTruth::Violation,
                2 => GroundTruth::Censored, _ => return Err(Error::InvalidInput) };
            CredibilityEvent::Assess(round, Assessment { origin, evidence_id, truth })
        }
        2 => {
            let update = FileCredibilityUpdate { operation: r.u64()?, expected_control_sequence: r.u64()?,
                expected_authority_epoch: r.u64()?, expected_evaluation_revision: r.u64()? };
            update.validate()?; CredibilityEvent::Promote(update)
        }
        _ => return Err(Error::InvalidInput),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_label_vector_and_every_truncation_use_fixed_fields() {
        let mut expected = vec![1];
        expected.extend_from_slice(&7_u64.to_be_bytes());
        expected.extend_from_slice(&13_u64.to_be_bytes());
        expected.extend_from_slice(&[29; 32]); expected.push(2);
        let event = CredibilityEvent::Assess(7, Assessment { origin: 13, evidence_id: [29; 32], truth: GroundTruth::Censored });
        let mut w = Writer::new(100); write(&mut w, &event).unwrap(); assert_eq!(w.finish(), expected);
        for end in 0..expected.len() { assert!(read(&mut Reader::new(&expected[..end])).is_err()); }
        let mut r = Reader::new(&expected); let decoded = read(&mut r).unwrap(); r.end().unwrap();
        let mut w = Writer::new(100); write(&mut w, &decoded).unwrap(); assert_eq!(w.finish(), expected);
        *expected.last_mut().unwrap() = 3; assert!(read(&mut Reader::new(&expected)).is_err());
    }
}
