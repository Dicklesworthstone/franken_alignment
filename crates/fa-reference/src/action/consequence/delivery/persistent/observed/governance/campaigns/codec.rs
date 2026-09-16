//! One new journal family, retaining the original bounded policy syntax codec.
use super::CampaignEvent;
use crate::action::consequence::delivery::persistent::codec::shared::{Reader, Writer};
use crate::action::consequence::delivery::persistent::governance::codec as policy;
use crate::action::consequence::oversight::policy_governance::MAX_POLICY_CAMPAIGNS;
use crate::action::consequence::policy_campaign::{ReplayLimits, MAX_REPLAY_CASES, MAX_REPLAY_INPUT_BYTES};
use crate::Error;

fn settings(limits: ReplayLimits, maximum: usize) -> Result<(), Error> {
    limits.validate()?;
    if maximum == 0 { return Err(Error::InvalidInput); }
    if maximum > MAX_POLICY_CAMPAIGNS { return Err(Error::Limit); }
    Ok(())
}

pub(in super::super::super) fn write(w: &mut Writer, event: &CampaignEvent) -> Result<(), Error> {
    match event {
        CampaignEvent::Enable(limits, maximum) => {
            settings(*limits, *maximum)?;
            w.u8(0)?; w.count(limits.cases)?; w.count(limits.input_bytes)?; w.count(*maximum)?;
        }
        CampaignEvent::Request(update) => { w.u8(1)?; w.blob(&policy::encode(update)?)?; }
        CampaignEvent::Approve(id, accept) => { w.u8(2)?; w.u64(*id)?; w.u8(u8::from(*accept))?; }
        CampaignEvent::Reject(id) => { w.u8(3)?; w.u64(*id)?; }
        CampaignEvent::Revoke(id) => { w.u8(4)?; w.u64(*id)?; }
        CampaignEvent::Promote(id) => { w.u8(5)?; w.u64(*id)?; }
    }
    Ok(())
}

pub(in super::super::super) fn read(r: &mut Reader<'_>) -> Result<CampaignEvent, Error> {
    Ok(match r.u8()? {
        0 => {
            let limits = ReplayLimits { cases: r.count(MAX_REPLAY_CASES)?, input_bytes: r.count(MAX_REPLAY_INPUT_BYTES)? };
            let maximum = r.count(MAX_POLICY_CAMPAIGNS)?;
            settings(limits, maximum)?;
            CampaignEvent::Enable(limits, maximum)
        }
        1 => CampaignEvent::Request(policy::decode(r.blob(policy::MAX_UPDATE_BYTES)?)?),
        2 => {
            let id = r.u64()?;
            let accept = match r.u8()? { 0 => false, 1 => true, _ => return Err(Error::InvalidInput) };
            CampaignEvent::Approve(id, accept)
        }
        3 => CampaignEvent::Reject(r.u64()?),
        4 => CampaignEvent::Revoke(r.u64()?),
        5 => CampaignEvent::Promote(r.u64()?),
        _ => return Err(Error::InvalidInput),
    })
}
