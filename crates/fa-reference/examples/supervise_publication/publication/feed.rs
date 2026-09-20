//! Required producer freshness plus exact change coverage, consumed by the SAME
//! three-reader native driver. File readability alone is not feed liveness.
use super::{Fields, path};
use crate::config::CLOCK_DOMAIN;
use fa_reference::action::Scope;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::heartbeat::feed::PublicationFeedFile;
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::producer::PublicationProducerProfile;
use fa_reference::action::consequence::delivery::publication_gate::changes::PublicationChangePolicy;
use fa_reference::action::consequence::delivery::publication_gate::changes::freshness::PublicationFreshnessPolicy;
use fa_reference::strict_json::Json;
use fa_reference::witness::refinement::index::routing::RoutingBudget;
use std::path::PathBuf;

#[derive(Debug)]
pub(super) struct FeedProfile {
    pub(super) changes: PublicationChangePolicy,
    pub(super) freshness: PublicationFreshnessPolicy,
    pub(super) reader: PublicationFeedFile,
}
impl FeedProfile {
    pub(super) fn decode(json: Json) -> Result<Self, String> {
        let mut f = Fields::new(json)?;
        let selected = path(f.text("path")?)?;
        let (changes, freshness) = policies(&mut f)?;
        f.end()?;
        Ok(Self { changes, freshness,
            reader: PublicationFeedFile::new(selected, changes.source).map_err(super::debug)? })
    }

    /// Version 3 selects ONE path for both projections. A separate feed path is
    /// an error, not a fallback if the producer bundle becomes unavailable.
    pub(super) fn producer(json: Json, selected: PathBuf, source: u64, scope: Scope)
        -> Result<(Self, PublicationProducerProfile), String>
    {
        let mut f = Fields::new(json)?;
        let (changes, freshness) = policies(&mut f)?;
        f.end()?;
        let profile = PublicationProducerProfile { source, scope, feed: changes.source,
            clock_domain: freshness.clock_domain, after: changes.after };
        let reader = PublicationFeedFile::from_producer(selected, profile).map_err(super::debug)?;
        Ok((Self { changes, freshness, reader }, profile))
    }
}

fn policies(f: &mut Fields) -> Result<(PublicationChangePolicy, PublicationFreshnessPolicy), String> {
    let source = f.number("source")?;
    let after = f.number("after")?;
    if f.text("clock")? != "unix_milliseconds" { return Err("feed clock must be unix_milliseconds".into()); }
    let max_age_ticks = f.number("max_age_ms")?;
    if max_age_ticks == 0 { return Err("feed freshness must have a positive maximum age".into()); }
    let mut lookup = Fields::new(f.take("lookup")?)?;
    let budget = RoutingBudget { steps: lookup.number("steps")?, bytes: lookup.number("bytes")? };
    lookup.end()?;
    Ok((PublicationChangePolicy { source, after, lookup: budget },
        PublicationFreshnessPolicy { clock_domain: CLOCK_DOMAIN, max_age_ticks }))
}
