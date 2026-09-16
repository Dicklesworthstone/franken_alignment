//! New numerical-input family; all pre-existing journal encodings stay intact.
use super::{DecoderEvent, FileDecoderConfig, StepRequest, MAX_WITNESS_BYTES};
use super::super::super::codec::shared::{Reader, Writer};
use crate::Error;
use std::rc::Rc;

pub(in super::super) fn write(w: &mut Writer, event: &DecoderEvent) -> Result<(), Error> {
    match event {
        DecoderEvent::Enable(config) => { w.u8(0)?; config.write(w)?; }
        DecoderEvent::Step(request, witness) => {
            if witness.is_empty() { return Err(Error::Incomplete); }
            if witness.len() > MAX_WITNESS_BYTES { return Err(Error::Limit); }
            match request {
                StepRequest::Forced { revision, position, token, products } => {
                    w.u8(1)?; w.u64(*revision)?; w.u64(*position)?; w.u32(*token)?; w.u64(*products)?;
                }
                StepRequest::Sampled { revision, position, products, vocabulary } => {
                    w.u8(2)?; w.u64(*revision)?; w.u64(*position)?; w.u64(*products)?; w.count(*vocabulary)?;
                }
            }
            w.blob(witness)?;
        }
        DecoderEvent::Resume { revision, position } => { w.u8(3)?; w.u64(*revision)?; w.u64(*position)?; }
    }
    Ok(())
}
pub(in super::super) fn read(r: &mut Reader<'_>) -> Result<DecoderEvent, Error> {
    let tag = r.u8()?;
    Ok(match tag {
        0 => DecoderEvent::Enable(Rc::new(FileDecoderConfig::read(r)?)),
        1 | 2 => {
            let revision = r.u64()?; let position = r.u64()?;
            let request = if tag == 1 {
                StepRequest::Forced { revision, position, token: r.u32()?, products: r.u64()? }
            } else { StepRequest::Sampled { revision, position, products: r.u64()?, vocabulary: r.count(usize::MAX)? } };
            let witness = r.blob(MAX_WITNESS_BYTES)?;
            if witness.is_empty() { return Err(Error::Incomplete); }
            DecoderEvent::Step(request, Rc::from(witness))
        }
        3 => DecoderEvent::Resume { revision: r.u64()?, position: r.u64()? },
        _ => return Err(Error::InvalidInput),
    })
}
