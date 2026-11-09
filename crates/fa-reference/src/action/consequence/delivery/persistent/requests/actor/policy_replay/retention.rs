//! Conservative logical charge for every retained request/report occurrence.
//! The original native builder separately bounds transient replay work. These
//! counts bound cache retention without pretending to measure allocator peaks.

use super::CachedReplay;
use crate::ReadWitness;
use crate::action::consequence::gate::containment::session::policy::{
    Evaluation, Policy, Predicate, Step,
};
use crate::action::consequence::policy_campaign::{PolicyReplayCase, PolicyReplayReport};
use std::mem::size_of;

const TREE_ENTRY_BYTES: usize = 128;
const SHARED_POLICY_BYTES: usize = 2 * size_of::<usize>();

pub(super) fn request_bytes(candidate: &Policy) -> Option<usize> {
    let mut charge = Charge(size_of::<(u64, CachedReplay)>());
    charge.add(TREE_ENTRY_BYTES)?;
    charge.policy(candidate)?;
    Some(charge.0)
}

pub(super) fn report_bytes(report: &PolicyReplayReport) -> Option<usize> {
    let mut charge = Charge(size_of::<PolicyReplayReport>());
    // The complete report is shared by legacy observations and optional pages.
    // Charge its allocation's strong/weak counters once, never per case page.
    charge.add(2 * size_of::<usize>())?;
    // Count both policies per retained report even if an Rc is shared elsewhere.
    charge.policy(report.previous_policy())?;
    charge.policy(report.candidate_policy())?;
    charge.add(2 * SHARED_POLICY_BYTES)?;
    charge.items::<PolicyReplayCase>(report.cases().len())?;
    for case in report.cases() {
        let action = case.action().spec();
        charge.add(action.payload.len())?;
        charge.witnesses(&action.required_witnesses)?;
        charge.evaluation(case.original())?;
        if let Some(evaluation) = case.candidate() {
            charge.evaluation(evaluation)?;
        }
        charge.items::<usize>(case.missing_nodes().len())?;
    }
    Some(charge.0)
}

struct Charge(usize);
impl Charge {
    fn add(&mut self, bytes: usize) -> Option<()> {
        self.0 = self.0.checked_add(bytes)?;
        Some(())
    }
    fn items<T>(&mut self, count: usize) -> Option<()> {
        self.add(count.checked_mul(size_of::<T>())?)
    }
    fn policy(&mut self, policy: &Policy) -> Option<()> {
        self.add(size_of::<Policy>())?;
        self.items::<Predicate>(policy.nodes().len())?;
        for node in policy.nodes() {
            match node {
                Predicate::PayloadIs(value) | Predicate::ExactValue { value, .. } => {
                    self.add(value.len())?;
                }
                Predicate::All(children) | Predicate::Any(children) => {
                    self.items::<usize>(children.len())?;
                }
                _ => {}
            }
        }
        Some(())
    }
    fn witnesses(&mut self, witnesses: &[ReadWitness]) -> Option<()> {
        self.items::<ReadWitness>(witnesses.len())?;
        for witness in witnesses {
            if let ReadWitness::Exact {
                value: Some(value), ..
            } = witness
            {
                self.add(value.len())?;
            }
        }
        Some(())
    }
    fn evaluation(&mut self, evaluation: &Evaluation) -> Option<()> {
        self.items::<Step>(evaluation.trace().len())?;
        self.witnesses(evaluation.witnesses())
    }
}
