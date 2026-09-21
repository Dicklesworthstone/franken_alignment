//! An implicit balanced tree over the already sorted interval postings.
//! Summaries contain exact unions/maxima, never an approximate disjointness test.
use super::{DomainKey, Interval, InvalidationIndex, MAX_ROUTED_JUDGMENTS};
use super::lookup::{INTERVAL_BYTES, Meter};
use crate::Error;

// Logical payload: one u64 endpoint and 128 registration bits. Padding, CPU,
// build work and allocator costs are NOT claimed by this byte counter.
const SUMMARY_BYTES: u64 = 24;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Summary { max_end: u64, owners: u128 }

pub(super) fn build(intervals: &[Interval]) -> Result<Vec<Summary>, Error> {
    let mut summaries = Vec::new();
    summaries.try_reserve_exact(intervals.len()).map_err(|_| Error::Limit)?;
    summaries.resize(intervals.len(), Summary::default());
    summarize(intervals, &mut summaries);
    Ok(summaries)
}

fn summarize(intervals: &[Interval], summaries: &mut [Summary]) -> Summary {
    if intervals.is_empty() { return Summary::default(); }
    let mid = intervals.len() / 2;
    let (left, rest) = summaries.split_at_mut(mid);
    let (root, right) = rest.split_first_mut().expect("nonempty sorted subtree");
    let a = summarize(&intervals[..mid], left);
    let b = summarize(&intervals[mid + 1..], right);
    *root = Summary {
        max_end: intervals[mid].end.max(a.max_end).max(b.max_end),
        owners: a.owners | b.owners | (1_u128 << intervals[mid].slot),
    };
    *root
}

pub(super) fn lookup(index: &InvalidationIndex, domain: DomainKey, low: u64,
    high: Option<u64>, affected: &mut [bool; MAX_ROUTED_JUDGMENTS], meter: &mut Meter)
    -> Result<(), Error>
{
    // Local result bookkeeping only. Registration/point records that selected
    // these bits were already charged by the shared lookup before this call.
    let mut selected = affected.iter().enumerate().fold(0_u128, |bits, (slot, hit)| {
        if *hit { bits | (1_u128 << slot) } else { bits }
    });
    visit(&index.intervals, &index.subtrees, domain, low, high, &mut selected, meter)?;
    for (slot, hit) in affected.iter_mut().enumerate() { *hit = selected & (1_u128 << slot) != 0; }
    Ok(())
}

fn visit(intervals: &[Interval], summaries: &[Summary], domain: DomainKey,
    low: u64, high: Option<u64>, selected: &mut u128, meter: &mut Meter) -> Result<(), Error>
{
    if intervals.is_empty() { return Ok(()); }
    // Registration caps the tree at MAX_ROUTED_DEPENDENCIES (8192 today).
    // Each recursive call halves the slice: at most 14 nonempty stack frames,
    // no traversal allocation, and no attacker-chosen tree depth.
    let mid = intervals.len() / 2;
    meter.charge(SUMMARY_BYTES)?;
    let summary = summaries[mid];
    if summary.max_end <= low || summary.owners & !*selected == 0 { return Ok(()); }
    meter.charge(INTERVAL_BYTES)?;
    let interval = intervals[mid];
    // Domain ordering narrows the same sorted tree. Maxima from other domains
    // can cause extra work, but cannot hide an intersecting interval.
    if interval.domain > domain
        || (interval.domain == domain && high.map_or(interval.start > low, |end| interval.start >= end)) {
        return visit(&intervals[..mid], &summaries[..mid], domain, low, high, selected, meter);
    }
    if interval.domain < domain {
        return visit(&intervals[mid + 1..], &summaries[mid + 1..], domain, low, high, selected, meter);
    }
    if interval.end > low { *selected |= 1_u128 << interval.slot; }
    // Root first lets an existing hit skip remaining postings of that owner.
    // A different owner's overlapping support is never removed by this pruning.
    visit(&intervals[..mid], &summaries[..mid], domain, low, high, selected, meter)?;
    visit(&intervals[mid + 1..], &summaries[mid + 1..], domain, low, high, selected, meter)
}

#[cfg(test)]
mod tests;
