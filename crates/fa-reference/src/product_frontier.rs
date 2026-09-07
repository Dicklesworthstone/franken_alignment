//! Bounded product-of-stream frontiers for the reference model.
//!
//! A contiguous prefix is positive progress only.  A negative or closed-scope
//! claim additionally needs a caller-supplied trusted closing observation.  The
//! reference model records that trust boundary; it does not authenticate it.

use std::collections::BTreeMap;

use super::{Error, Frontier};

/// One authorized projection of one source stream in one branch and epoch.
///
/// Projection is part of the identity: identical source sequence numbers from
/// two differently authorized views are not interchangeable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProjectionKey {
    pub source: u64,
    pub branch: u64,
    pub projection: u64,
    pub source_epoch: u64,
}

/// Independently tracked evidence stages.  Their order is named by a
/// requirement; this reference model does not invent a global transition order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FrontierStage {
    Captured,
    Authenticated,
    DurableRequired,
    Presented,
    Judged,
    EffectBound,
}

/// A trusted observation that its named projection ends at `final_sequence`.
///
/// This is caller-supplied reference input.  It carries no cryptographic or
/// origin proof and must not be mistaken for one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrustedClosingMarker {
    pub key: ProjectionKey,
    pub final_sequence: u64,
    pub marker_generation: u64,
}

/// A named prefix obligation.  `closure` is required for a negative or other
/// closed-scope claim and names the exact closing-marker generation required.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrontierRequirement {
    pub key: ProjectionKey,
    pub stage: FrontierStage,
    pub through: u64,
    pub closure: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StageFrontier {
    frontier: Frontier,
    greatest_accepted: u64,
}

impl StageFrontier {
    fn new(max_gap: u64) -> Result<Self, Error> {
        Ok(Self {
            frontier: Frontier::new(max_gap)?,
            greatest_accepted: 0,
        })
    }

    fn accept(&mut self, sequence: u64) -> Result<(), Error> {
        self.frontier.accept(sequence)?;
        self.greatest_accepted = self.greatest_accepted.max(sequence);
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProjectionFrontiers {
    stages: BTreeMap<FrontierStage, StageFrontier>,
    close: Option<TrustedClosingMarker>,
}

impl ProjectionFrontiers {
    fn new(max_gap: u64) -> Result<Self, Error> {
        let mut stages = BTreeMap::new();
        for stage in [
            FrontierStage::Captured,
            FrontierStage::Authenticated,
            FrontierStage::DurableRequired,
            FrontierStage::Presented,
            FrontierStage::Judged,
            FrontierStage::EffectBound,
        ] {
            stages.insert(stage, StageFrontier::new(max_gap)?);
        }
        Ok(Self {
            stages,
            close: None,
        })
    }

    fn greatest_accepted(&self) -> u64 {
        self.stages
            .values()
            .map(|stage| stage.greatest_accepted)
            .max()
            .unwrap_or(0)
    }
}

/// A bounded product of independent source/projection frontier obligations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductFrontiers {
    max_streams: usize,
    max_gap: u64,
    projections: BTreeMap<ProjectionKey, ProjectionFrontiers>,
}

impl ProductFrontiers {
    pub fn new(max_streams: usize, max_gap: u64) -> Result<Self, Error> {
        if max_streams == 0 {
            return Err(Error::InvalidInput);
        }
        // Validate this once even before a projection is admitted.
        Frontier::new(max_gap)?;
        Ok(Self {
            max_streams,
            max_gap,
            projections: BTreeMap::new(),
        })
    }

    /// Records positive evidence progress for exactly one projection and stage.
    ///
    /// After a terminal close observation, new source positions beyond its end
    /// are rejected.  Existing positions may still progress through another
    /// independently tracked stage.
    pub fn accept(
        &mut self,
        key: ProjectionKey,
        stage: FrontierStage,
        sequence: u64,
    ) -> Result<(), Error> {
        if sequence == 0 {
            return Err(Error::InvalidInput);
        }
        if let Some(existing) = self.projections.get(&key) {
            if existing
                .close
                .is_some_and(|marker| sequence > marker.final_sequence)
            {
                return Err(Error::WrongState);
            }
            let projection = self.projections.get_mut(&key).ok_or(Error::Missing)?;
            return projection
                .stages
                .get_mut(&stage)
                .ok_or(Error::Missing)?
                .accept(sequence);
        }

        if self.projections.len() >= self.max_streams {
            return Err(Error::Limit);
        }
        let mut projection = ProjectionFrontiers::new(self.max_gap)?;
        projection
            .stages
            .get_mut(&stage)
            .ok_or(Error::Missing)?
            .accept(sequence)?;
        self.projections.insert(key, projection);
        Ok(())
    }

    /// Records an explicit trusted closing-marker observation.
    ///
    /// Contiguity through the marker is necessary but not sufficient for a
    /// closed claim: callers must provide this observation separately.
    pub fn record_close(&mut self, marker: TrustedClosingMarker) -> Result<(), Error> {
        if marker.final_sequence == 0 || marker.marker_generation == 0 {
            return Err(Error::InvalidInput);
        }
        let projection = self
            .projections
            .get_mut(&marker.key)
            .ok_or(Error::Missing)?;
        if projection.close.is_some() {
            return Err(Error::WrongState);
        }
        if projection.greatest_accepted() > marker.final_sequence {
            return Err(Error::InvalidInput);
        }
        let authenticated = projection
            .stages
            .get(&FrontierStage::Authenticated)
            .ok_or(Error::Missing)?;
        if authenticated.frontier.contiguous() < marker.final_sequence {
            return Err(Error::Incomplete);
        }
        projection.close = Some(marker);
        Ok(())
    }

    /// Returns whether all named positive and optional closing obligations hold.
    pub fn satisfies(&self, requirement: FrontierRequirement) -> Result<bool, Error> {
        if requirement.through == 0 || requirement.closure == Some(0) {
            return Err(Error::InvalidInput);
        }
        let Some(projection) = self.projections.get(&requirement.key) else {
            return Ok(false);
        };
        let stage = projection
            .stages
            .get(&requirement.stage)
            .ok_or(Error::Missing)?;
        if stage.frontier.contiguous() < requirement.through {
            return Ok(false);
        }
        match requirement.closure {
            None => Ok(true),
            Some(generation) => Ok(projection.close.is_some_and(|marker| {
                marker.marker_generation == generation
                    && marker.final_sequence >= requirement.through
            })),
        }
    }

    /// The contiguous prefix at one named independent stage.
    pub fn frontier(&self, key: ProjectionKey, stage: FrontierStage) -> Result<u64, Error> {
        self.projections
            .get(&key)
            .and_then(|projection| projection.stages.get(&stage))
            .map(|stage| stage.frontier.contiguous())
            .ok_or(Error::Missing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(projection: u64, source_epoch: u64) -> ProjectionKey {
        ProjectionKey {
            source: 7,
            branch: 11,
            projection,
            source_epoch,
        }
    }

    fn marker(key: ProjectionKey, final_sequence: u64, generation: u64) -> TrustedClosingMarker {
        TrustedClosingMarker {
            key,
            final_sequence,
            marker_generation: generation,
        }
    }

    fn requirement(key: ProjectionKey, closure: Option<u64>) -> FrontierRequirement {
        FrontierRequirement {
            key,
            stage: FrontierStage::Judged,
            through: 2,
            closure,
        }
    }

    fn complete_prefix(frontiers: &mut ProductFrontiers, key: ProjectionKey) {
        for sequence in 1..=2 {
            frontiers
                .accept(key, FrontierStage::Authenticated, sequence)
                .unwrap();
            frontiers
                .accept(key, FrontierStage::Judged, sequence)
                .unwrap();
        }
    }

    #[test]
    fn contiguous_progress_cannot_manufacture_closure() {
        let first = key(1, 1);
        let mut frontiers = ProductFrontiers::new(2, 8).unwrap();
        complete_prefix(&mut frontiers, first);
        assert_eq!(frontiers.satisfies(requirement(first, None)), Ok(true));
        assert_eq!(frontiers.satisfies(requirement(first, Some(9))), Ok(false));
    }

    #[test]
    fn marker_requires_authenticated_prefix_and_its_own_observation() {
        let first = key(1, 1);
        let mut frontiers = ProductFrontiers::new(2, 8).unwrap();
        frontiers
            .accept(first, FrontierStage::Authenticated, 1)
            .unwrap();
        assert_eq!(
            frontiers.record_close(marker(first, 2, 9)),
            Err(Error::Incomplete)
        );
        frontiers
            .accept(first, FrontierStage::Authenticated, 2)
            .unwrap();
        frontiers.accept(first, FrontierStage::Judged, 1).unwrap();
        frontiers.accept(first, FrontierStage::Judged, 2).unwrap();
        frontiers.record_close(marker(first, 2, 9)).unwrap();
        assert_eq!(frontiers.satisfies(requirement(first, Some(9))), Ok(true));
    }

    #[test]
    fn closing_marker_cannot_cross_projection_epoch_or_generation() {
        let first = key(1, 1);
        let other_projection = key(2, 1);
        let other_epoch = key(1, 2);
        let other_source = ProjectionKey { source: 8, ..first };
        let other_branch = ProjectionKey {
            branch: 12,
            ..first
        };
        let mut frontiers = ProductFrontiers::new(5, 8).unwrap();
        complete_prefix(&mut frontiers, first);
        complete_prefix(&mut frontiers, other_projection);
        complete_prefix(&mut frontiers, other_epoch);
        complete_prefix(&mut frontiers, other_source);
        complete_prefix(&mut frontiers, other_branch);
        frontiers.record_close(marker(first, 2, 9)).unwrap();
        assert_eq!(
            frontiers.satisfies(requirement(other_projection, Some(9))),
            Ok(false)
        );
        assert_eq!(
            frontiers.satisfies(requirement(other_epoch, Some(9))),
            Ok(false)
        );
        assert_eq!(
            frontiers.satisfies(requirement(other_source, Some(9))),
            Ok(false)
        );
        assert_eq!(
            frontiers.satisfies(requirement(other_branch, Some(9))),
            Ok(false)
        );
        assert_eq!(frontiers.satisfies(requirement(first, Some(10))), Ok(false));
    }

    #[test]
    fn close_rejects_future_observation_and_terminal_close_rejects_extension() {
        let first = key(1, 1);
        let mut frontiers = ProductFrontiers::new(1, 8).unwrap();
        frontiers
            .accept(first, FrontierStage::Authenticated, 1)
            .unwrap();
        frontiers
            .accept(first, FrontierStage::Authenticated, 2)
            .unwrap();
        frontiers.accept(first, FrontierStage::Captured, 3).unwrap();
        let before_failed_close = frontiers.clone();
        assert_eq!(
            frontiers.record_close(marker(first, 2, 9)),
            Err(Error::InvalidInput)
        );
        assert_eq!(frontiers, before_failed_close);
        assert_eq!(frontiers.frontier(first, FrontierStage::Captured), Ok(0));

        let second = key(2, 1);
        let mut terminal = ProductFrontiers::new(1, 8).unwrap();
        complete_prefix(&mut terminal, second);
        terminal.record_close(marker(second, 2, 9)).unwrap();
        terminal.accept(second, FrontierStage::Captured, 2).unwrap();
        assert_eq!(terminal.frontier(second, FrontierStage::Captured), Ok(0));
        terminal.accept(second, FrontierStage::Captured, 1).unwrap();
        assert_eq!(terminal.frontier(second, FrontierStage::Captured), Ok(2));
        assert_eq!(
            terminal.accept(second, FrontierStage::Captured, 3),
            Err(Error::WrongState)
        );
        assert_eq!(
            terminal.record_close(marker(second, 2, 10)),
            Err(Error::WrongState)
        );
    }

    #[test]
    fn bounds_and_invalid_inputs_fail_without_admitting_a_projection() {
        let first = key(1, 1);
        let second = key(2, 1);
        assert_eq!(ProductFrontiers::new(0, 8), Err(Error::InvalidInput));

        let mut frontiers = ProductFrontiers::new(1, 2).unwrap();
        assert_eq!(
            frontiers.accept(first, FrontierStage::Captured, 3),
            Err(Error::Limit)
        );
        assert_eq!(
            frontiers.frontier(first, FrontierStage::Captured),
            Err(Error::Missing)
        );
        assert_eq!(
            frontiers.accept(first, FrontierStage::Captured, 0),
            Err(Error::InvalidInput)
        );
        frontiers.accept(first, FrontierStage::Captured, 1).unwrap();
        assert_eq!(
            frontiers.accept(second, FrontierStage::Captured, 1),
            Err(Error::Limit)
        );
        assert_eq!(frontiers.frontier(first, FrontierStage::Captured), Ok(1));
    }
}
