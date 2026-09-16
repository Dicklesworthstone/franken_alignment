//! Input interchange tests; numerical witnesses remain comparison-only bytes.
use super::*;
use crate::action::consequence::gate::ReviewBinding;

fn request() -> CheckpointRequest {
    CheckpointRequest::Reset { checkpoint: 7, control: FileResetRequest {
        operation: 1, expected_control_sequence: 2, expected_actor_revision: 3,
        expected_authority_epoch: 4, binding: ReviewBinding {
            round: 9, evidence_root: [17; 32], reducer_generation: 1,
        }, retained_targets: Vec::new(),
    }, budget: DecoderBudget { scalar_products: 123 } }
}

#[test]
fn checkpoint_input_codec_is_lossless_and_rejects_every_truncation() {
    for original in [CheckpointRequest::Capture { checkpoint: 7, actor_revision: 8, epoch: 9 }, request()] {
        let mut w = Writer::new(4096); write_request(&mut w, &original).unwrap(); let bytes = w.finish();
        let mut r = Reader::new(&bytes); assert_eq!(read_request(&mut r).unwrap(), original); r.end().unwrap();
        for end in 0..bytes.len() { assert!(read_request(&mut Reader::new(&bytes[..end])).is_err()); }
        let event = DecoderEvent::Checkpoint(original, Rc::from(&b"comparison, never imported state"[..]));
        let mut w = Writer::new(4096); super::super::write(&mut w, &event).unwrap(); let bytes = w.finish();
        assert_eq!(bytes[0], 4); // No previous numerical subtag changes.
        let mut r = Reader::new(&bytes); let decoded = super::super::read(&mut r).unwrap(); r.end().unwrap();
        let mut w = Writer::new(4096); super::super::write(&mut w, &decoded).unwrap(); assert_eq!(w.finish(), bytes);
        for end in 0..bytes.len() { assert!(super::super::read(&mut Reader::new(&bytes[..end])).is_err()); }
    }
}

#[test]
fn malformed_checkpoint_inputs_and_missing_comparison_material_are_not_admitted() {
    let mut w = Writer::new(4096);
    assert_eq!(write_request(&mut w, &CheckpointRequest::Capture { checkpoint: 0, actor_revision: 0, epoch: 0 }), Err(Error::InvalidInput));
    let mut invalid = request();
    if let CheckpointRequest::Reset { budget, .. } = &mut invalid { budget.scalar_products = MAX_DECODER_PRODUCTS + 1; }
    assert_eq!(write_request(&mut Writer::new(4096), &invalid), Err(Error::Limit));
    let mut invalid = request();
    if let CheckpointRequest::Reset { control, .. } = &mut invalid { control.binding.evidence_root = [0; 32]; }
    assert_eq!(write_request(&mut Writer::new(4096), &invalid), Err(Error::InvalidInput));
    assert_eq!(super::super::write(&mut Writer::new(4096), &DecoderEvent::Checkpoint(request(), Rc::from(&b""[..]))), Err(Error::Incomplete));
    assert_eq!(read_request(&mut Reader::new(&[255])), Err(Error::InvalidInput));
}
