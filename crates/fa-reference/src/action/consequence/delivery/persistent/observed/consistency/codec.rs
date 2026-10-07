//! Input records only: no imported prediction, likelihood, decision or permit.
use super::{ConsistencyEvent, FileConsistencyConfig, learned};
use super::config::MAX_CONFIG_BYTES;
use super::super::super::codec::shared::{Reader, Writer};
use crate::action::consequence::activation::{MAX_BLOCK_BYTES, identity::wire};
use crate::Error;
use crate::action::ElapsedTick;
use crate::action::consequence::oversight::consistency::ConsistencyDeadline;
use std::rc::Rc;

pub(in super::super) fn write(w: &mut Writer, event: &ConsistencyEvent) -> Result<(), Error> {
    match event {
        ConsistencyEvent::Enable(config) => { w.u8(0)?; w.blob(config.encoded())?; }
        ConsistencyEvent::Forecast(attempt, actor_revision, frame) => {
            w.u8(1)?; w.u64(*attempt)?; w.u64(*actor_revision)?; w.blob(&frame.encode_initial(23)?)?;
        }
        ConsistencyEvent::ForecastHosted(attempt, revision) => {
            w.u8(4)?; w.u64(*attempt)?; w.u64(*revision)?;
        }
        ConsistencyEvent::ForecastHostedRequest(request, revision) => {
            w.u8(5)?; w.u64(*request)?; w.u64(*revision)?;
        }
        ConsistencyEvent::Expire(deadline, at) => {
            w.u8(6)?;
            for value in [deadline.attempt, deadline.actor_revision, deadline.authority_epoch,
                deadline.source_sequence, deadline.created_at.0, deadline.expires_at.0, at.0] {
                w.u64(value)?;
            }
        }
        ConsistencyEvent::EnableLearned(config) => { w.u8(7)?; config.write(w)?; }
        ConsistencyEvent::ForecastLearned(attempt, revision, row, source) => {
            w.u8(8)?; w.u64(*attempt)?; w.u64(*revision)?; learned::write_row(w, *row)?; source.write(w)?;
        }
        ConsistencyEvent::ForecastLearnedRequest(request, revision, row, source) => {
            w.u8(9)?; w.u64(*request)?; w.u64(*revision)?; learned::write_row(w, *row)?; source.write(w)?;
        }
        ConsistencyEvent::ForecastOwnedLearned(attempt, revision, witness) => {
            w.u8(10)?; w.u64(*attempt)?; w.u64(*revision)?;
            w.blob(witness.as_deref().unwrap_or(&[]))?;
        }
        ConsistencyEvent::ForecastOwnedLearnedRequest(request, revision, witness) => {
            w.u8(11)?; w.u64(*request)?; w.u64(*revision)?;
            w.blob(witness.as_deref().unwrap_or(&[]))?;
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
        4 => ConsistencyEvent::ForecastHosted(r.u64()?, r.u64()?),
        5 => ConsistencyEvent::ForecastHostedRequest(r.u64()?, r.u64()?),
        6 => ConsistencyEvent::Expire(ConsistencyDeadline {
            attempt: r.u64()?, actor_revision: r.u64()?, authority_epoch: r.u64()?,
            source_sequence: r.u64()?, created_at: ElapsedTick(r.u64()?), expires_at: ElapsedTick(r.u64()?),
        }, ElapsedTick(r.u64()?)),
        7 => ConsistencyEvent::EnableLearned(learned::Configuration::read(r)?),
        8 => ConsistencyEvent::ForecastLearned(r.u64()?, r.u64()?, learned::read_row(r)?, learned::Capture::read(r)?),
        9 => ConsistencyEvent::ForecastLearnedRequest(r.u64()?, r.u64()?, learned::read_row(r)?, learned::Capture::read(r)?),
        10 => ConsistencyEvent::ForecastOwnedLearned(r.u64()?, r.u64()?, learned::owned::read_witness(r)?),
        11 => ConsistencyEvent::ForecastOwnedLearnedRequest(r.u64()?, r.u64()?, learned::owned::read_witness(r)?),
        _ => return Err(Error::InvalidInput),
    })
}
