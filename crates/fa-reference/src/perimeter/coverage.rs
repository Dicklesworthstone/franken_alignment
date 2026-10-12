//! Non-vacuous mediation audits over the original bounded graph oracle.
//!
//! Inputs are caller-declared topology and eligible barriers, not authenticated
//! deployment observations. A result proves a graph property under that exact
//! declaration; it is neither a permit nor proof of real-world containment.

pub mod synthesis;

use super::PerimeterScope;
use crate::{Error, Graph};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Identity of one logical projection. All components must be nonzero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoverageIdentity {
    pub scope: PerimeterScope,
    pub projection: u64,
    pub semantic_epoch: u64,
    pub topology_generation: u64,
}

/// Sources and sinks are protected endpoints, never eligible barrier sites.
/// `complete` is a trusted caller declaration, not something reachability proves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoverageDeclaration {
    pub identity: CoverageIdentity,
    pub sources: BTreeSet<usize>,
    pub sinks: BTreeSet<usize>,
    pub candidates: BTreeSet<usize>,
    pub complete: bool,
}

/// An immutable, admitted logical problem, bounded by `Graph::new`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoverageProblem {
    graph: Graph,
    declaration: CoverageDeclaration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SinkCoverage {
    /// No original path exists. This is not credited as mediated coverage.
    AlreadyUnreachable,
    /// An original path exists, and deleting the selected barriers blocks ALL
    /// paths to this sink, not just this deterministic shortest baseline path.
    Mediated { baseline_path: Vec<usize> },
    /// A deterministic shortest source-to-sink path survives every barrier.
    Bypass { path: Vec<usize> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoverageAudit {
    identity: CoverageIdentity,
    barriers: BTreeSet<usize>,
    sinks: BTreeMap<usize, SinkCoverage>,
}

impl CoverageAudit {
    pub fn identity(&self) -> CoverageIdentity {
        self.identity
    }

    pub fn barriers(&self) -> &BTreeSet<usize> {
        &self.barriers
    }

    pub fn sinks(&self) -> &BTreeMap<usize, SinkCoverage> {
        &self.sinks
    }

    pub fn active_sinks(&self) -> usize {
        self.sinks
            .values()
            .filter(|status| !matches!(status, SinkCoverage::AlreadyUnreachable))
            .count()
    }

    pub fn has_bypass(&self) -> bool {
        self.sinks
            .values()
            .any(|status| matches!(status, SinkCoverage::Bypass { .. }))
    }

    /// Non-vacuous coverage of the declared active sinks only. Unreachable
    /// sinks remain separately visible and are never counted as mediated.
    pub fn covers_active_sinks(&self) -> bool {
        self.active_sinks() != 0 && !self.has_bypass()
    }
}

/// Opaque graph-only result bound by FULL structural equality, not a short hash.
/// It contains no credentials, live authority or mutable deployment handle.
///
/// ```compile_fail,E0308
/// use fa_reference::action::Permit;
/// use fa_reference::perimeter::coverage::CoverageCertificate;
/// fn authorize(certificate: CoverageCertificate) -> Permit { certificate }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoverageCertificate {
    problem: CoverageProblem,
    audit: CoverageAudit,
}

impl CoverageCertificate {
    pub fn audit(&self) -> &CoverageAudit {
        &self.audit
    }
}

impl CoverageProblem {
    pub fn new(graph: Graph, declaration: CoverageDeclaration) -> Result<Self, Error> {
        declaration.identity.scope.validate()?;
        if [
            declaration.identity.projection,
            declaration.identity.semantic_epoch,
            declaration.identity.topology_generation,
        ]
        .contains(&0)
        {
            return Err(Error::InvalidInput);
        }
        if !declaration.complete {
            return Err(Error::Incomplete);
        }
        let nodes = graph.edges.len();
        if declaration.sources.len() > nodes
            || declaration.sinks.len() > nodes
            || declaration.candidates.len() > nodes
        {
            return Err(Error::Limit);
        }
        if declaration.sources.is_empty()
            || declaration.sinks.is_empty()
            || declaration
                .sources
                .iter()
                .chain(&declaration.sinks)
                .chain(&declaration.candidates)
                .any(|&node| node >= nodes)
        {
            return Err(Error::InvalidInput);
        }
        if !declaration.candidates.is_disjoint(&declaration.sources)
            || !declaration.candidates.is_disjoint(&declaration.sinks)
        {
            return Err(Error::Binding);
        }
        Ok(Self { graph, declaration })
    }

    pub fn declaration(&self) -> &CoverageDeclaration {
        &self.declaration
    }

    /// Two bounded breadth-first traversals, followed by at most 128 paths of
    /// at most 128 nodes. Sorted sources and graph adjacency fix tie-breaking.
    /// This operation never changes the graph, inventory or authority ledger.
    pub fn audit(&self, barriers: &BTreeSet<usize>) -> Result<CoverageAudit, Error> {
        if barriers.len() > self.graph.edges.len() {
            return Err(Error::Limit);
        }
        if !barriers.is_subset(&self.declaration.candidates) {
            return Err(Error::Binding);
        }
        let original = self.predecessors(&BTreeSet::new());
        let surviving = self.predecessors(barriers);
        let sinks = self
            .declaration
            .sinks
            .iter()
            .map(|&sink| {
                let status = if original[sink].is_none() {
                    SinkCoverage::AlreadyUnreachable
                } else if surviving[sink].is_some() {
                    SinkCoverage::Bypass {
                        path: path_to(&surviving, sink),
                    }
                } else {
                    SinkCoverage::Mediated {
                        baseline_path: path_to(&original, sink),
                    }
                };
                (sink, status)
            })
            .collect();
        Ok(CoverageAudit {
            identity: self.declaration.identity,
            barriers: barriers.clone(),
            sinks,
        })
    }

    /// Refuses a vacuous or bypassed claim. Eligibility alone is insufficient:
    /// the original, independent cut oracle also checks the selected barriers.
    pub fn certify(&self, barriers: &BTreeSet<usize>) -> Result<CoverageCertificate, Error> {
        let audit = self.audit(barriers)?;
        if !audit.covers_active_sinks() {
            return Err(Error::Incomplete);
        }
        let sources: Vec<_> = self.declaration.sources.iter().copied().collect();
        let sinks: Vec<_> = self.declaration.sinks.iter().copied().collect();
        if !self.graph.cut_disconnects(&sources, &sinks, barriers)? {
            return Err(Error::Binding);
        }
        Ok(CoverageCertificate {
            problem: self.clone(),
            audit,
        })
    }

    /// Rechecks both the full immutable problem and the original graph oracle.
    /// Equal epochs cannot hide changed edges, endpoints or candidate eligibility.
    pub fn verify(&self, certificate: &CoverageCertificate) -> Result<(), Error> {
        if self != &certificate.problem {
            return Err(Error::Binding);
        }
        let checked = self.certify(certificate.audit.barriers())?;
        if checked.audit != certificate.audit {
            return Err(Error::Binding);
        }
        Ok(())
    }

    fn predecessors(&self, barriers: &BTreeSet<usize>) -> Vec<Option<usize>> {
        let mut parents = vec![None; self.graph.edges.len()];
        let mut queue = VecDeque::new();
        for &source in &self.declaration.sources {
            parents[source] = Some(source);
            queue.push_back(source);
        }
        while let Some(node) = queue.pop_front() {
            for &next in &self.graph.edges[node] {
                if parents[next].is_none() && !barriers.contains(&next) {
                    parents[next] = Some(node);
                    queue.push_back(next);
                }
            }
        }
        parents
    }
}

fn path_to(parents: &[Option<usize>], mut node: usize) -> Vec<usize> {
    let mut path = Vec::new();
    loop {
        path.push(node);
        let parent = parents[node].expect("path requested only for reached nodes");
        if parent == node {
            break;
        }
        node = parent;
    }
    path.reverse();
    path
}
