use super::*;

// Fixed logical record widths include the key, owner slot and search metadata.
// Charges occur BEFORE inspecting a record. They are architecture-independent.
const REGISTRATION_BYTES: u64 = 80;
const POINT_BYTES: u64 = 48;
const INTERVAL_BYTES: u64 = 64;
pub(super) struct Meter { left: RoutingBudget, spent: RoutingBudget }
impl Meter {
    pub(super) fn new(left: RoutingBudget) -> Self { Self { left, spent: RoutingBudget::default() } }
    pub(super) fn spent(&self) -> RoutingBudget { self.spent }
    fn charge(&mut self, bytes: u64) -> Result<(), Error> {
        if self.left.steps == 0 || self.left.bytes < bytes { return Err(Error::Incomplete); }
        self.left.steps -= 1;
        self.left.bytes -= bytes;
        self.spent.steps += 1;
        self.spent.bytes += bytes;
        Ok(())
    }
}
impl InvalidationIndex {
    pub(super) fn lookup(&self, change: WitnessChange, meter: &mut Meter) -> Result<Vec<u64>, Error> {
        meter.charge(72)?;
        let mut affected = [false; MAX_ROUTED_JUDGMENTS];
        let domain = change.domain();
        for (slot, entry) in self.registrations.iter().enumerate() {
            meter.charge(REGISTRATION_BYTES)?;
            affected[slot] = entry.opaque || matches!(change, WitnessChange::All)
                || entry.domain.zip(domain).is_some_and(|(old, new)| {
                    DomainKey::from(old) == DomainKey::from(new)
                        && (old != new || matches!(change, WitnessChange::Domain { .. }))
                });
        }
        match change {
            WitnessChange::Key { domain, key } => {
                self.points_in(DomainKey::from(domain), key, None, &mut affected, meter)?;
                self.intervals_in(DomainKey::from(domain), key, None, &mut affected, meter)?;
            }
            WitnessChange::Range { domain, start, end } => {
                self.points_in(DomainKey::from(domain), start, Some(end), &mut affected, meter)?;
                self.intervals_in(DomainKey::from(domain), start, Some(end), &mut affected, meter)?;
            }
            WitnessChange::Domain { .. } | WitnessChange::All => {}
        }
        let mut ids = Vec::new();
        ids.try_reserve_exact(self.registered()).map_err(|_| Error::Limit)?;
        // Stable registration order, no duplicate IDs despite overlapping ranges.
        for (slot, entry) in self.registrations.iter().enumerate() {
            meter.charge(REGISTRATION_BYTES)?;
            if affected[slot] { meter.charge(8)?; ids.push(entry.id); }
        }
        Ok(ids)
    }

    fn points_in(&self, domain: DomainKey, low: u64, high: Option<u64>,
        affected: &mut [bool; MAX_ROUTED_JUDGMENTS], meter: &mut Meter) -> Result<(), Error>
    {
        let (mut lo, mut hi) = (0, self.points.len());
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            meter.charge(POINT_BYTES)?;
            let point = &self.points[mid];
            if (point.domain, point.key) < (domain, low) { lo = mid + 1; } else { hi = mid; }
        }
        for point in &self.points[lo..] {
            meter.charge(POINT_BYTES)?;
            if point.domain != domain || high.map_or(point.key != low, |end| point.key >= end) { break; }
            affected[point.slot] = true;
        }
        Ok(())
    }

    fn intervals_in(&self, domain: DomainKey, low: u64, high: Option<u64>,
        affected: &mut [bool; MAX_ROUTED_JUDGMENTS], meter: &mut Meter) -> Result<(), Error>
    {
        let (mut lo, mut hi) = (0, self.intervals.len());
        // Upper bound on start: <= key for a point, < end for a half-open range.
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            meter.charge(INTERVAL_BYTES)?;
            let range = &self.intervals[mid];
            let before = range.domain < domain || (range.domain == domain
                && high.map_or(range.start <= low, |end| range.start < end));
            if before { lo = mid + 1; } else { hi = mid; }
        }
        while lo > 0 {
            lo -= 1;
            meter.charge(INTERVAL_BYTES)?;
            let range = &self.intervals[lo];
            // All earlier starts are already small enough. The maximum end
            // proves disjointness of this entire remaining per-domain prefix.
            if range.domain != domain || range.prefix_end <= low { break; }
            if range.end > low { affected[range.slot] = true; }
        }
        Ok(())
    }
}
