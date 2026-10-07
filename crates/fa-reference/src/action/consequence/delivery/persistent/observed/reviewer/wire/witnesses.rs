//! Ordered native action dependencies for the privileged human presentation.
//! No snapshot, judgment, permit, completeness claim or helper projection is
//! reconstructed here. Use the original bounded journal framing primitives.
use super::{Error, Reader, Writer};
use crate::ReadWitness;
use crate::action::{MAX_REQUIRED_WITNESSES, MAX_WITNESS_BYTES};

pub(super) fn write(w: &mut Writer, witnesses: &[ReadWitness]) -> Result<(), Error> {
    if witnesses.len() > MAX_REQUIRED_WITNESSES { return Err(Error::Limit); }
    w.count(witnesses.len())?;
    let mut remaining = MAX_WITNESS_BYTES;
    for witness in witnesses {
        match witness {
            ReadWitness::Exact { key, value: None } => { w.u8(0)?; w.u64(*key)?; }
            ReadWitness::Exact { key, value: Some(value) } => {
                remaining = remaining.checked_sub(value.len()).ok_or(Error::Limit)?;
                w.u8(1)?; w.u64(*key)?; w.blob(value)?;
            }
            ReadWitness::EmptyRange { start, end } => {
                if start >= end { return Err(Error::InvalidInput); }
                w.u8(2)?; w.u64(*start)?; w.u64(*end)?;
            }
        }
    }
    Ok(())
}

pub(super) fn read(r: &mut Reader<'_>) -> Result<Vec<ReadWitness>, Error> {
    let count = r.count(MAX_REQUIRED_WITNESSES)?;
    let mut witnesses = Vec::new();
    witnesses.try_reserve_exact(count).map_err(|_| Error::Limit)?;
    let mut remaining = MAX_WITNESS_BYTES;
    for _ in 0..count {
        let witness = match r.u8()? {
            0 => ReadWitness::Exact { key: r.u64()?, value: None },
            1 => {
                let key = r.u64()?;
                // Check the TOTAL original value-byte limit before allocating
                // this value; many individually small values cannot evade it.
                let value = r.blob(remaining)?;
                remaining = remaining.checked_sub(value.len()).ok_or(Error::Limit)?;
                ReadWitness::Exact { key, value: Some(value.to_vec()) }
            }
            2 => {
                let start = r.u64()?; let end = r.u64()?;
                if start >= end { return Err(Error::InvalidInput); }
                ReadWitness::EmptyRange { start, end }
            }
            _ => return Err(Error::InvalidInput),
        };
        // Do not sort, deduplicate, conflate absent with empty, or turn a range
        // into a sampled key. Exact FrozenAction equality includes this order.
        witnesses.push(witness);
    }
    Ok(witnesses)
}
