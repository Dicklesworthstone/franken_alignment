//! Required producer freshness plus exact change coverage, consumed by the SAME
//! three-reader native driver. File readability alone is not feed liveness.
use super::{Fields, path};
use crate::config::CLOCK_DOMAIN;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::heartbeat::feed::PublicationFeedFile;
use fa_reference::action::consequence::delivery::publication_gate::changes::PublicationChangePolicy;
use fa_reference::action::consequence::delivery::publication_gate::changes::freshness::PublicationFreshnessPolicy;
use fa_reference::strict_json::Json;
use fa_reference::witness::refinement::index::routing::RoutingBudget;

#[derive(Debug)]
pub(super) struct FeedProfile {
    pub(super) changes: PublicationChangePolicy,
    pub(super) freshness: PublicationFreshnessPolicy,
    pub(super) reader: PublicationFeedFile,
}
impl FeedProfile {
    pub(super) fn decode(json: Json) -> Result<Self, String> {
        let mut f = Fields::new(json)?;
        let source = f.number("source")?;
        let selected = path(f.text("path")?)?;
        let after = f.number("after")?;
        if f.text("clock")? != "unix_milliseconds" { return Err("feed clock must be unix_milliseconds".into()); }
        let max_age_ticks = f.number("max_age_ms")?;
        if max_age_ticks == 0 { return Err("feed freshness must have a positive maximum age".into()); }
        let mut lookup = Fields::new(f.take("lookup")?)?;
        let budget = RoutingBudget { steps: lookup.number("steps")?, bytes: lookup.number("bytes")? };
        lookup.end()?; f.end()?;
        Ok(Self { changes: PublicationChangePolicy { source, after, lookup: budget },
            freshness: PublicationFreshnessPolicy { clock_domain: CLOCK_DOMAIN, max_age_ticks },
            reader: PublicationFeedFile::new(selected, source).map_err(super::debug)? })
    }
}
