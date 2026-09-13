//! Preserve the original evidence-file representation and bounded primitives.
use super::{FileSourcePolicy, FileSourceReplacement, SourceEvent};
use super::super::super::codec::shared::{Reader, Writer};
use crate::action::ElapsedTick;
use crate::action::consequence::oversight::evidence_source::{EvidenceSnapshot, MAX_EVIDENCE_FILE_BYTES};
use crate::action::consequence::oversight::policy_state::{StateFreshness, StateLimits, StateSource,
    MAX_STATE_EVENTS, MAX_STATE_RETAINED_BYTES};
use crate::Error;
use std::rc::Rc;

pub(in super::super) fn write(w: &mut Writer, event: &SourceEvent) -> Result<(), Error> {
    match event {
        SourceEvent::Enable(policy) => {
            w.u8(0)?;
            // Source scope is checked against the independently supplied host
            // bootstrap during replay; no alternate scope decoder is introduced.
            let s = policy.source;
            let scope = s.scope;
            for value in [scope.tenant, scope.principal, scope.run, scope.branch, scope.authority] { w.u64(value)?; }
            w.u8(match scope.purpose { crate::action::Purpose::Effect => 0, crate::action::Purpose::Experiment => 1 })?;
            w.u64(s.source)?; w.u64(s.generation)?;
            w.count(policy.limits.events)?; w.count(policy.limits.retained_bytes)?;
            w.u64(policy.freshness.max_age_ticks())?;
        }
        SourceEvent::Observe(capture, tick) => {
            w.u8(1)?; w.u64(tick.0)?;
            let bytes = capture.encode();
            if bytes.len() > MAX_EVIDENCE_FILE_BYTES { return Err(Error::Limit); }
            w.blob(&bytes)?;
        }
        SourceEvent::Withdraw => w.u8(2)?,
        SourceEvent::Replace(request) => {
            w.u8(3)?; w.u64(request.operation)?; w.u64(request.expected_generation)?;
            w.u64(request.expected_authority_epoch)?; w.u64(request.next_generation)?;
        }
    }
    Ok(())
}

pub(in super::super) fn read(r: &mut Reader<'_>) -> Result<SourceEvent, Error> {
    Ok(match r.u8()? {
        0 => {
            let scope = crate::action::Scope { tenant: r.u64()?, principal: r.u64()?, run: r.u64()?,
                branch: r.u64()?, authority: r.u64()?, purpose: match r.u8()? {
                    0 => crate::action::Purpose::Effect, 1 => crate::action::Purpose::Experiment,
                    _ => return Err(Error::InvalidInput),
                } };
            let source = StateSource { scope, source: r.u64()?, generation: r.u64()? };
            let limits = StateLimits { events: r.count(MAX_STATE_EVENTS)?, retained_bytes: r.count(MAX_STATE_RETAINED_BYTES)? };
            SourceEvent::Enable(FileSourcePolicy { source, limits, freshness: StateFreshness::new(r.u64()?)? })
        }
        1 => {
            let tick = ElapsedTick(r.u64()?);
            let capture = EvidenceSnapshot::decode(r.blob(MAX_EVIDENCE_FILE_BYTES)?)?;
            SourceEvent::Observe(Rc::new(capture), tick)
        }
        2 => SourceEvent::Withdraw,
        3 => SourceEvent::Replace(FileSourceReplacement { operation: r.u64()?, expected_generation: r.u64()?,
            expected_authority_epoch: r.u64()?, next_generation: r.u64()? }),
        _ => return Err(Error::InvalidInput),
    })
}
