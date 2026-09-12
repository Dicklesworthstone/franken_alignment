//! Frozen, origin-disjoint labelled captures for fitting an existing linear probe.
//! Labels, origins and capture provenance are supplied by the evaluation owner;
//! structural separation is not authentication or evidence of detector quality.

mod fit;
pub use fit::{FitPolicy, FittedProbe, TrainingBudget, TrainingWork, MAX_TRAINING_VISITS};

use super::{CaptureProfile, MAX_VALUES};
use super::super::SourceFrame;
use crate::Error;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;

pub const MAX_CORPUS_CASES: usize = 4096;
pub const MAX_CORPUS_COORDINATES: usize = 1_048_576;

/// One selected tap frame per original task AND attack lineage. Derivative frames
/// cannot inflate this baseline's sample count by acquiring another case number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CaseOrigin { pub task: u64, pub lineage: u64 }

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DataSplit { Training, Calibration, Evaluation }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaseLabel { Benign, Violation }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClassCounts { pub benign: usize, pub violation: usize }
impl ClassCounts {
    pub fn total(self) -> usize { self.benign + self.violation }
}

/// The complete assignment is fixed BEFORE any labelled captures are accepted.
/// It has no API to move a case, add a new origin, replace a label or delete a row.
pub struct ProbeCorpus {
    id: u64,
    generation: u64,
    profile: CaptureProfile,
    dimensions: usize,
    assignments: BTreeMap<CaseOrigin, DataSplit>,
    rows: BTreeMap<CaseOrigin, LabelledFrame>,
}
struct LabelledFrame { split: DataSplit, label: CaseLabel, source: SourceFrame }

impl fmt::Debug for ProbeCorpus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProbeCorpus").field("id", &self.id).field("generation", &self.generation)
            .field("planned", &self.assignments.len()).field("captured", &self.rows.len())
            .field("dimensions", &self.dimensions).finish_non_exhaustive()
    }
}

impl ProbeCorpus {
    pub fn new(id: u64, generation: u64, profile: CaptureProfile, dimensions: usize,
        assignments: BTreeMap<CaseOrigin, DataSplit>) -> Result<Self, Error>
    {
        if [id, generation, profile.tenant, profile.model, profile.model_generation,
            profile.tap, profile.layout_generation].contains(&0) || dimensions == 0 || assignments.is_empty()
        { return Err(Error::InvalidInput); }
        if dimensions > MAX_VALUES || assignments.len() > MAX_CORPUS_CASES
            || dimensions.checked_mul(assignments.len()).ok_or(Error::Overflow)? > MAX_CORPUS_COORDINATES
        { return Err(Error::Limit); }
        let mut tasks = BTreeSet::new();
        let mut lineages = BTreeSet::new();
        let mut splits = BTreeSet::new();
        for (origin, split) in &assignments {
            if origin.task == 0 || origin.lineage == 0 { return Err(Error::InvalidInput); }
            if !tasks.insert(origin.task) || !lineages.insert(origin.lineage) { return Err(Error::Duplicate); }
            splits.insert(*split);
        }
        if splits.len() != 3 { return Err(Error::Incomplete); }
        Ok(Self { id, generation, profile, dimensions, assignments, rows: BTreeMap::new() })
    }

    pub fn planned_cases(&self) -> usize { self.assignments.len() }
    pub fn captured_cases(&self) -> usize { self.rows.len() }

    /// Retain the actual immutable capture. No caller-supplied flattened values,
    /// aggregate statistics or classifier scores can replace its source binding.
    pub fn capture(&mut self, origin: CaseOrigin, label: CaseLabel, source: SourceFrame) -> Result<(), Error> {
        let split = *self.assignments.get(&origin).ok_or(Error::Missing)?;
        if self.rows.contains_key(&origin) { return Err(Error::Duplicate); }
        if source.identity().profile != self.profile || source.dimensions() != self.dimensions {
            return Err(Error::Binding);
        }
        if self.rows.values().any(|row| row.source.identity() == source.identity()) {
            return Err(Error::Duplicate);
        }
        self.rows.insert(origin, LabelledFrame { split, label, source });
        Ok(())
    }

    /// A missing or censored planned case prevents sealing; it is not silently
    /// removed from a denominator. Both labels are required in ALL three splits.
    /// Borrowing rather than consuming the builder permits a missing case to arrive.
    pub fn seal(&self) -> Result<SealedCorpus, Error> {
        if self.rows.len() != self.assignments.len() { return Err(Error::Incomplete); }
        let mut counts = BTreeMap::new();
        for row in self.rows.values() {
            let count: &mut ClassCounts = counts.entry(row.split).or_default();
            match row.label { CaseLabel::Benign => count.benign += 1, CaseLabel::Violation => count.violation += 1 }
        }
        if counts.values().any(|count| count.benign == 0 || count.violation == 0) { return Err(Error::Incomplete); }
        // SourceFrame clones share immutable arrays. Only bounded row metadata is copied.
        let rows = self.rows.iter().map(|(origin, row)| (*origin, LabelledFrame {
            split: row.split, label: row.label, source: row.source.clone(),
        })).collect();
        Ok(SealedCorpus { data: Rc::new(CorpusData { id: self.id, generation: self.generation,
            profile: self.profile, dimensions: self.dimensions, rows, counts }) })
    }
}

struct CorpusData {
    id: u64,
    generation: u64,
    profile: CaptureProfile,
    dimensions: usize,
    rows: BTreeMap<CaseOrigin, LabelledFrame>,
    counts: BTreeMap<DataSplit, ClassCounts>,
}

/// Immutable declared split and source closure. The owner can still lie about
/// lineage/labels or repeatedly run campaigns; this is not a governance authority.
#[derive(Clone)]
pub struct SealedCorpus { data: Rc<CorpusData> }
impl fmt::Debug for SealedCorpus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SealedCorpus").field("id", &self.id()).field("generation", &self.generation())
            .field("counts", &self.data.counts).field("dimensions", &self.dimensions()).finish_non_exhaustive()
    }
}
impl SealedCorpus {
    pub fn id(&self) -> u64 { self.data.id }
    pub fn generation(&self) -> u64 { self.data.generation }
    pub fn profile(&self) -> CaptureProfile { self.data.profile }
    pub fn dimensions(&self) -> usize { self.data.dimensions }
    pub fn counts(&self, split: DataSplit) -> ClassCounts { self.data.counts[&split] }
    pub fn assignments(&self) -> impl Iterator<Item = (CaseOrigin, DataSplit)> + '_ {
        self.data.rows.iter().map(|(origin, row)| (*origin, row.split))
    }
    fn rows(&self, split: DataSplit) -> impl Iterator<Item = (CaseOrigin, &LabelledFrame)> {
        self.data.rows.iter().filter(move |(_, row)| row.split == split).map(|(origin, row)| (*origin, row))
    }
}
