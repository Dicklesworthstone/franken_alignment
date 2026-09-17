//! Input records only: no imported prediction, likelihood, decision or permit.
use super::{ConsistencyEvent, FileConsistencyConfig};
use super::config::MAX_CONFIG_BYTES;
use super::super::super::codec::shared::{Reader, Writer};
use crate::action::consequence::activation::{MAX_BLOCK_BYTES, identity::wire};
use crate::Error;
use std::rc::Rc;

pub(in super::super) fn write(w: &mut Writer, event: &ConsistencyEvent) -> Result<(), Error> {
    match event {
        ConsistencyEvent::Enable(config) => { w.u8(0)?; w.blob(config.encoded())?; }
        ConsistencyEvent::Forecast(attempt, actor_revision, frame) => {
            w.u8(1)?; w.u64(*attempt)?; w.u64(*actor_revision)?; w.blob(&frame.encode_initial(23)?)?;
        }
        ConsistencyEvent::Unavailable => w.u8(2)?,
        ConsistencyEvent::ForecastRequest(request, actor_revision, frame) => {
            w.u8(3)?; w.u64(*request)?; w.u64(*actor_revision)?; w.blob(&frame.encode_initial(23)?)?;
        }
    }
    Ok(())
}
pub(in super::super) fn read(r: &mut Reader<'_>) -> Result<ConsistencyEvent, Error> {
    Ok(match r.u8()? {
        0 => ConsistencyEvent::Enable(Rc::new(FileConsistencyConfig::from_bytes(r.blob(MAX_CONFIG_BYTES)?)?)),
        1 => ConsistencyEvent::Forecast(r.u64()?, r.u64()?, wire::decode_frame(r.blob(MAX_BLOCK_BYTES)?)?),
        2 => ConsistencyEvent::Unavailable,
        3 => ConsistencyEvent::ForecastRequest(r.u64()?, r.u64()?, wire::decode_frame(r.blob(MAX_BLOCK_BYTES)?)?),
        _ => return Err(Error::InvalidInput),
    })
}
