//! Bounded inputs to the native model-identity gate, not serialized verdicts.
use super::IdentityEvent;
use crate::action::ElapsedTick;
use crate::action::consequence::activation::{MAX_BLOCK_BYTES, identity::wire};
use crate::action::consequence::delivery::persistent::codec::shared::{Reader, Writer};
use crate::action::consequence::oversight::identity::{IdentityPolicy, MAX_IDENTITY_CHECKS};
use crate::Error;
use std::rc::Rc;

fn policy(policy: IdentityPolicy) -> Result<(), Error> {
    if policy.observer_id == 0 || policy.timeout_ticks == 0 || policy.max_checks == 0
        || policy.timeout_ticks > policy.validity_ticks { return Err(Error::InvalidInput); }
    if policy.max_checks > MAX_IDENTITY_CHECKS { return Err(Error::Limit); }
    Ok(())
}

pub(in super::super) fn write(w: &mut Writer, event: &IdentityEvent) -> Result<(), Error> {
    match event {
        IdentityEvent::Enable(passport, settings) => {
            policy(*settings)?;
            w.u8(0)?; w.blob(&wire::encode_passport(passport)?)?;
            w.u64(settings.observer_id)?; w.u64(settings.timeout_ticks)?;
            w.u64(settings.validity_ticks)?; w.count(settings.max_checks)?;
        }
        IdentityEvent::Begin(id, sequence, actor) => { w.u8(1)?; w.u64(*id)?; w.u64(*sequence)?; w.u64(*actor)?; }
        IdentityEvent::Manifest(id, manifest, at) => {
            w.u8(2)?; w.u64(*id)?; w.blob(&wire::encode_manifest(manifest))?; w.u64(at.0)?;
        }
        IdentityEvent::Anchor(id, anchor, frame, at) => {
            w.u8(3)?; w.u64(*id)?; w.u64(*anchor)?; w.blob(&frame.encode_initial(23)?)?; w.u64(at.0)?;
        }
        IdentityEvent::Apply(id, sequence, epoch) => { w.u8(4)?; w.u64(*id)?; w.u64(*sequence)?; w.u64(*epoch)?; }
        IdentityEvent::Unavailable(basis) => { w.u8(5)?; w.u64(*basis)?; }
    }
    Ok(())
}

pub(in super::super) fn read(r: &mut Reader<'_>) -> Result<IdentityEvent, Error> {
    Ok(match r.u8()? {
        0 => {
            let passport = Rc::new(wire::decode_passport(r.blob(wire::MAX_PASSPORT_BYTES)?)?);
            let settings = IdentityPolicy { observer_id: r.u64()?, timeout_ticks: r.u64()?,
                validity_ticks: r.u64()?, max_checks: r.count(MAX_IDENTITY_CHECKS)? };
            policy(settings)?;
            IdentityEvent::Enable(passport, settings)
        }
        1 => IdentityEvent::Begin(r.u64()?, r.u64()?, r.u64()?),
        2 => IdentityEvent::Manifest(r.u64()?, wire::decode_manifest(r.blob(wire::MANIFEST_BYTES)?)?, ElapsedTick(r.u64()?)),
        3 => IdentityEvent::Anchor(r.u64()?, r.u64()?, wire::decode_frame(r.blob(MAX_BLOCK_BYTES)?)?, ElapsedTick(r.u64()?)),
        4 => IdentityEvent::Apply(r.u64()?, r.u64()?, r.u64()?),
        5 => IdentityEvent::Unavailable(r.u64()?),
        _ => return Err(Error::InvalidInput),
    })
}
