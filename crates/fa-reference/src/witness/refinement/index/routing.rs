//! Bounded reverse dependency routing for FA-061. A notification can withdraw
//! an observation; it can never establish validity or install a refinement mask.
//! The complete-snapshot index in the parent remains the only skipping oracle.
mod lookup;
#[cfg(test)]
mod tests;

use crate::witness::{DomainProjection, Witness, WitnessJudgment, MAX_WITNESSES};
use crate::Error;

pub const MAX_ROUTED_JUDGMENTS: usize = 128;
pub const MAX_ROUTED_DEPENDENCIES: usize = MAX_ROUTED_JUDGMENTS * MAX_WITNESSES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoutingLimits {
    pub judgments: usize,
    pub dependencies: usize,
}

/// Logical records visited and bytes inspected/output, NOT CPU instructions,
/// wall time, allocator usage, or the work of subsequent exact revalidation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RoutingBudget {
    pub steps: u64,
    pub bytes: u64,
}

/// Trusted change notifications, not proof of an unchanged complete tail.
/// A semantic/closure change uses Domain (or All), never a convenient changed key.
/// Ranges are half-open; Key can represent u64::MAX without overflowing an end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WitnessChange {
    Key { domain: DomainProjection, key: u64 },
    Range { domain: DomainProjection, start: u64, end: u64 },
    Domain { domain: DomainProjection },
    All,
}
impl WitnessChange {
    pub fn validate(self) -> Result<(), Error> {
        if matches!(self, Self::Range { start, end, .. } if start >= end) {
            return Err(Error::InvalidInput);
        }
        Ok(())
    }
    fn domain(self) -> Option<DomainProjection> {
        match self {
            Self::Key { domain, .. } | Self::Range { domain, .. } | Self::Domain { domain } => Some(domain),
            Self::All => None,
        }
    }
}

/// No partial candidate set escapes exhaustion. Callers must withdraw everything
/// or do a complete exact fallback on Err, never treat it as an empty answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutingReport {
    pub candidates: Result<Vec<u64>, Error>,
    pub spent: RoutingBudget,
}

// Stable domain identity deliberately excludes semantic/source epochs. A change
// under a new epoch must reach registrations made under the old epoch too.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct DomainKey([u64; 4]);
impl From<DomainProjection> for DomainKey {
    fn from(domain: DomainProjection) -> Self {
        let p = domain.projection();
        Self([domain.domain_id(), p.source, p.branch, p.projection])
    }
}
#[derive(Clone, Copy, Debug)]
struct Registration { id: u64, domain: Option<DomainProjection>, opaque: bool }
#[derive(Clone, Copy, Debug)]
struct Point { domain: DomainKey, key: u64, slot: usize }
#[derive(Clone, Copy, Debug)]
struct Interval { domain: DomainKey, start: u64, end: u64, slot: usize, prefix_end: u64 }

/// Append-only, value-free index of ORIGINAL judgments. Registration derives
/// dependencies from private captured witnesses, not caller-asserted footprints.
/// Exact and absent keys share a sorted point index. Empty/membership ranges use
/// sorted starts with a per-domain maximum-end prefix for overlap pruning.
/// Opaque judgments are conservatively notified by every change, regardless of
/// a more precise structured lane. Cancellation never reopens lifetime capacity.
#[derive(Debug)]
pub struct InvalidationIndex {
    limits: RoutingLimits,
    registrations: Vec<Registration>,
    points: Vec<Point>,
    intervals: Vec<Interval>,
}
impl InvalidationIndex {
    pub fn new(limits: RoutingLimits) -> Result<Self, Error> {
        if limits.judgments == 0 || limits.judgments > MAX_ROUTED_JUDGMENTS
            || limits.dependencies > MAX_ROUTED_DEPENDENCIES { return Err(Error::Limit); }
        Ok(Self { limits, registrations: Vec::new(), points: Vec::new(), intervals: Vec::new() })
    }
    pub fn registered(&self) -> usize { self.registrations.len() }
    pub fn dependencies(&self) -> usize { self.points.len() + self.intervals.len() }

    /// Build the entire next bounded index before changing the current one.
    /// Construction may temporarily retain two footprint indexes, never witness
    /// values. This admission work is separate from notification lookup budgets.
    pub fn register(&mut self, id: u64, structured: Option<&WitnessJudgment>, opaque: bool) -> Result<(), Error> {
        if structured.is_none() && !opaque { return Err(Error::Incomplete); }
        if self.registrations.iter().any(|entry| entry.id == id) { return Err(Error::Duplicate); }
        if self.registered() == self.limits.judgments { return Err(Error::Limit); }
        let added = structured.map_or(0, |judgment| judgment.witnesses.len());
        if self.dependencies().checked_add(added).ok_or(Error::Overflow)? > self.limits.dependencies {
            return Err(Error::Limit);
        }
        let mut points = copy_with_room(&self.points, added)?;
        let mut intervals = copy_with_room(&self.intervals, added)?;
        let slot = self.registered();
        if let Some(judgment) = structured {
            let domain = DomainKey::from(judgment.domain);
            for witness in &judgment.witnesses {
                match witness {
                    Witness::ExactValue { key, .. } | Witness::AbsentKey { key, .. } => {
                        points.push(Point { domain, key: *key, slot });
                    }
                    Witness::EmptyRange { start, end, .. } | Witness::RangeMembers { start, end, .. } => {
                        intervals.push(Interval { domain, start: *start, end: *end, slot, prefix_end: 0 });
                    }
                }
            }
        }
        points.sort_unstable_by_key(|point| (point.domain, point.key, point.slot));
        intervals.sort_unstable_by_key(|range| (range.domain, range.start, range.end, range.slot));
        let mut previous = None;
        let mut prefix = 0;
        for interval in &mut intervals {
            if previous != Some(interval.domain) { prefix = 0; }
            prefix = prefix.max(interval.end);
            interval.prefix_end = prefix;
            previous = Some(interval.domain);
        }
        self.registrations.try_reserve(1).map_err(|_| Error::Limit)?;
        self.points = points;
        self.intervals = intervals;
        self.registrations.push(Registration { id, domain: structured.map(|j| j.domain), opaque });
        Ok(())
    }

    pub fn affected(&self, change: WitnessChange, budget: RoutingBudget) -> RoutingReport {
        let mut meter = lookup::Meter::new(budget);
        let candidates = change.validate().and_then(|()| self.lookup(change, &mut meter));
        RoutingReport { candidates, spent: meter.spent() }
    }
}

fn copy_with_room<T: Copy>(old: &[T], extra: usize) -> Result<Vec<T>, Error> {
    let mut next = Vec::new();
    next.try_reserve_exact(old.len().checked_add(extra).ok_or(Error::Overflow)?)
        .map_err(|_| Error::Limit)?;
    next.extend_from_slice(old);
    Ok(next)
}
