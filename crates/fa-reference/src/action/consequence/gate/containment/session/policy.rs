//! Bounded exact policy predicates over a frozen action and reference snapshot.
//!
//! This is a pure reference evaluator, not a provider authentication mechanism.
//! Every state read is retained, including reads in an unselected Boolean arm.
//! Unknown evidence never produces a certificate. The trace explains the same
//! computation that supplies the action's witnesses; it is not a second scorer.

pub mod controller;

use crate::action::{FrozenAction, MAX_REQUIRED_WITNESSES, MAX_WITNESS_BYTES, ResolvedTarget};
use crate::{Error, ReadWitness, Snapshot};

pub const MAX_POLICY_NODES: usize = 128;
pub const MAX_POLICY_EDGES: usize = 512;
pub const MAX_POLICY_LITERAL_BYTES: usize = 65_536;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Truth {
    Satisfied,
    Violated,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Predicate {
    TargetIs(ResolvedTarget),
    PayloadIs(Vec<u8>),
    PayloadAtMost(usize),
    UnitsAtMost(u64),
    ExactValue { key: u64, value: Vec<u8> },
    Absent { key: u64 },
    EmptyRange { start: u64, end: u64 },
    All(Vec<usize>),
    Any(Vec<usize>),
    Not(usize),
}

impl Predicate {
    fn children(&self) -> &[usize] {
        match self {
            Self::All(children) | Self::Any(children) => children,
            Self::Not(child) => std::slice::from_ref(child),
            _ => &[],
        }
    }

    fn reads_state(&self) -> bool {
        matches!(self, Self::ExactValue { .. } | Self::Absent { .. } | Self::EmptyRange { .. })
    }
}

/// Nodes are topologically ordered; the last is the root. All nodes must be
/// reachable from that root. No recursive evaluation, callbacks, or ambient I/O.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Policy {
    generation: u64,
    nodes: Vec<Predicate>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub node: usize,
    pub result: Truth,
    pub witness: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Evaluation {
    generation: u64,
    result: Truth,
    trace: Vec<Step>,
    witnesses: Vec<ReadWitness>,
    complete: bool,
}

impl Evaluation {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn result(&self) -> Truth {
        self.result
    }

    pub fn trace(&self) -> &[Step] {
        &self.trace
    }

    pub fn witnesses(&self) -> &[ReadWitness] {
        &self.witnesses
    }

    /// A permission path must additionally validate these witnesses, scope,
    /// epoch, deadline, congress review and the exact action at its own boundary.
    pub fn certifiable(&self) -> bool {
        self.complete
            && self.result == Truth::Satisfied
            && self.trace.iter().all(|step| step.result != Truth::Unknown)
    }
}

impl Policy {
    pub fn new(generation: u64, nodes: Vec<Predicate>) -> Result<Self, Error> {
        if generation == 0 || nodes.is_empty() {
            return Err(Error::InvalidInput);
        }
        if nodes.len() > MAX_POLICY_NODES {
            return Err(Error::Limit);
        }
        let mut edges = 0_usize;
        let mut literals = 0_usize;
        let mut reads = 0_usize;
        for (index, node) in nodes.iter().enumerate() {
            let children = node.children();
            if children.iter().any(|child| *child >= index)
                || matches!(node, Predicate::All(v) | Predicate::Any(v) if v.is_empty())
            {
                return Err(Error::InvalidInput);
            }
            edges = edges.checked_add(children.len()).ok_or(Error::Limit)?;
            reads += usize::from(node.reads_state());
            match node {
                Predicate::TargetIs(target) => {
                    if [target.adapter, target.object, target.contract_version,
                        target.expected_version, target.generation].contains(&0)
                    {
                        return Err(Error::InvalidInput);
                    }
                }
                Predicate::PayloadIs(value) | Predicate::ExactValue { value, .. } => {
                    literals = literals.checked_add(value.len()).ok_or(Error::Limit)?;
                }
                Predicate::EmptyRange { start, end } if start >= end => {
                    return Err(Error::InvalidInput);
                }
                _ => {}
            }
            if edges > MAX_POLICY_EDGES || literals > MAX_POLICY_LITERAL_BYTES
                || reads > MAX_REQUIRED_WITNESSES
            {
                return Err(Error::Limit);
            }
        }
        let mut reachable = vec![false; nodes.len()];
        let mut pending = vec![nodes.len() - 1];
        while let Some(index) = pending.pop() {
            if !reachable[index] {
                reachable[index] = true;
                pending.extend_from_slice(nodes[index].children());
            }
        }
        if reachable.contains(&false) {
            return Err(Error::InvalidInput);
        }
        Ok(Self { generation, nodes })
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn nodes(&self) -> &[Predicate] {
        &self.nodes
    }

    pub fn evaluate(&self, action: &FrozenAction, snapshot: &Snapshot) -> Result<Evaluation, Error> {
        // Charge every retained occurrence before cloning any provider bytes.
        // Empty-range violations retain one actual member as a nonemptiness
        // witness, not an invalid claim that the range was empty.
        if snapshot.complete {
            let mut retained = 0_usize;
            for node in &self.nodes {
                let value = match node {
                    Predicate::ExactValue { key, .. } | Predicate::Absent { key } => {
                        snapshot.values.get(key)
                    }
                    Predicate::EmptyRange { start, end } => {
                        snapshot.values.range(*start..*end).next().map(|(_, value)| value)
                    }
                    _ => None,
                };
                retained = retained.checked_add(value.map_or(0, Vec::len)).ok_or(Error::Limit)?;
                if retained > MAX_WITNESS_BYTES {
                    return Err(Error::Limit);
                }
            }
        }
        let mut trace: Vec<Step> = Vec::with_capacity(self.nodes.len());
        let mut witnesses = Vec::new();
        for (index, node) in self.nodes.iter().enumerate() {
            let first_witness = witnesses.len();
            let result = if node.reads_state() && !snapshot.complete {
                Truth::Unknown
            } else {
                match node {
                    Predicate::TargetIs(target) => truth(action.spec().target == Some(*target)),
                    Predicate::PayloadIs(value) => truth(action.spec().payload == *value),
                    Predicate::PayloadAtMost(limit) => truth(action.spec().payload.len() <= *limit),
                    Predicate::UnitsAtMost(limit) => truth(action.spec().units <= *limit),
                    Predicate::ExactValue { key, value } => {
                        let actual = snapshot.values.get(key);
                        witnesses.push(ReadWitness::Exact { key: *key, value: actual.cloned() });
                        truth(actual == Some(value))
                    }
                    Predicate::Absent { key } => {
                        let actual = snapshot.values.get(key);
                        witnesses.push(ReadWitness::Exact { key: *key, value: actual.cloned() });
                        truth(actual.is_none())
                    }
                    Predicate::EmptyRange { start, end } => {
                        if let Some((key, value)) = snapshot.values.range(*start..*end).next() {
                            witnesses.push(ReadWitness::Exact { key: *key, value: Some(value.clone()) });
                            Truth::Violated
                        } else {
                            witnesses.push(ReadWitness::EmptyRange { start: *start, end: *end });
                            Truth::Satisfied
                        }
                    }
                    Predicate::All(children) => {
                        if children.iter().any(|child| trace[*child].result == Truth::Violated) {
                            Truth::Violated
                        } else if children.iter().any(|child| trace[*child].result == Truth::Unknown) {
                            Truth::Unknown
                        } else {
                            Truth::Satisfied
                        }
                    }
                    Predicate::Any(children) => {
                        if children.iter().any(|child| trace[*child].result == Truth::Satisfied) {
                            Truth::Satisfied
                        } else if children.iter().any(|child| trace[*child].result == Truth::Unknown) {
                            Truth::Unknown
                        } else {
                            Truth::Violated
                        }
                    }
                    Predicate::Not(child) => match trace[*child].result {
                        Truth::Satisfied => Truth::Violated,
                        Truth::Violated => Truth::Satisfied,
                        Truth::Unknown => Truth::Unknown,
                    },
                }
            };
            trace.push(Step {
                node: index,
                result,
                witness: (witnesses.len() != first_witness).then_some(first_witness),
            });
        }
        Ok(Evaluation {
            generation: self.generation,
            result: trace.last().expect("validated nonempty policy").result,
            trace,
            witnesses,
            complete: snapshot.complete,
        })
    }
}

fn truth(value: bool) -> Truth {
    if value { Truth::Satisfied } else { Truth::Violated }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{ActionSpec, ElapsedTick, Purpose, Scope, VERSION};
    use crate::Judgment;
    use std::collections::BTreeMap;

    fn action() -> FrozenAction {
        FrozenAction::freeze(ActionSpec {
            version: VERSION,
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            target: Some(ResolvedTarget { adapter: 1, object: 2, contract_version: 3, expected_version: 4, generation: 5 }),
            payload: b"publish".to_vec(), required_witnesses: Vec::new(),
            policy_epoch: 0, deadline: ElapsedTick(100), units: 4,
        }).unwrap()
    }

    fn snapshot() -> Snapshot {
        Snapshot { semantic_epoch: 3, complete: true, values: BTreeMap::from([(7, vec![9])]) }
    }

    #[test]
    fn exact_policy_trace_and_witnesses_agree() {
        let action = action();
        let policy = Policy::new(1, vec![
            Predicate::TargetIs(action.spec().target.unwrap()),
            Predicate::ExactValue { key: 7, value: vec![9] },
            Predicate::PayloadIs(b"publish".to_vec()), Predicate::UnitsAtMost(4),
            Predicate::All(vec![0, 1, 2, 3]),
        ]).unwrap();
        let evaluation = policy.evaluate(&action, &snapshot()).unwrap();
        assert!(evaluation.certifiable());
        assert_eq!(evaluation.generation(), 1);
        assert_eq!(evaluation.trace().len(), 5);
        assert_eq!(evaluation.trace()[1].witness, Some(0));
        assert_eq!(evaluation.witnesses(), &[ReadWitness::Exact { key: 7, value: Some(vec![9]) }]);
        assert_eq!(evaluation, policy.evaluate(&action, &snapshot()).unwrap());
        let mut changed = snapshot();
        changed.values.insert(7, vec![8]);
        let changed = policy.evaluate(&action, &changed).unwrap();
        assert_eq!(changed.result(), Truth::Violated);
        assert_eq!(changed.witnesses(), &[ReadWitness::Exact { key: 7, value: Some(vec![8]) }]);
    }

    #[test]
    fn negative_domains_detect_new_keys_but_allow_unrelated_changes() {
        let policy = Policy::new(1, vec![
            Predicate::Absent { key: 8 }, Predicate::EmptyRange { start: 10, end: 20 },
            Predicate::All(vec![0, 1]),
        ]).unwrap();
        let original = snapshot();
        let result = policy.evaluate(&action(), &original).unwrap();
        assert!(result.certifiable());
        let judgment = Judgment::capture(&original, result.witnesses().to_vec()).unwrap();
        for key in [8, 10, 15, 19] {
            let mut changed = original.clone();
            changed.values.insert(key, vec![1]);
            assert!(!judgment.valid_at(&changed).unwrap());
            assert_eq!(policy.evaluate(&action(), &changed).unwrap().result(), Truth::Violated);
        }
        let mut unrelated = original;
        unrelated.values.insert(20, vec![1]);
        assert!(judgment.valid_at(&unrelated).unwrap());
    }

    #[test]
    fn negated_empty_range_uses_a_real_nonempty_witness() {
        let policy = Policy::new(1, vec![
            Predicate::EmptyRange { start: 5, end: 10 }, Predicate::Not(0),
        ]).unwrap();
        let mut snapshot = snapshot();
        let result = policy.evaluate(&action(), &snapshot).unwrap();
        assert!(result.certifiable());
        assert_eq!(result.witnesses(), &[ReadWitness::Exact { key: 7, value: Some(vec![9]) }]);
        let judgment = Judgment::capture(&snapshot, result.witnesses().to_vec()).unwrap();
        snapshot.values.remove(&7);
        assert!(!judgment.valid_at(&snapshot).unwrap());
        assert_eq!(policy.evaluate(&action(), &snapshot).unwrap().result(), Truth::Violated);
    }

    #[test]
    fn any_arm_is_not_a_hidden_dependency() {
        let policy = Policy::new(1, vec![
            Predicate::PayloadAtMost(100), Predicate::Absent { key: 7 }, Predicate::Any(vec![0, 1]),
        ]).unwrap();
        let original = snapshot();
        let result = policy.evaluate(&action(), &original).unwrap();
        assert!(result.certifiable());
        assert_eq!(result.trace()[1].result, Truth::Violated);
        let judgment = Judgment::capture(&original, result.witnesses().to_vec()).unwrap();
        let mut changed = original;
        changed.values.insert(7, vec![8]);
        assert!(!judgment.valid_at(&changed).unwrap());
        assert!(policy.evaluate(&action(), &changed).unwrap().certifiable());
    }

    #[test]
    fn unknown_does_not_become_permission_through_not_or_any() {
        let mut snapshot = snapshot();
        snapshot.complete = false;
        let negated = Policy::new(1, vec![Predicate::Absent { key: 7 }, Predicate::Not(0)]).unwrap();
        let result = negated.evaluate(&action(), &snapshot).unwrap();
        assert_eq!(result.result(), Truth::Unknown);
        assert!(!result.certifiable());
        assert!(result.witnesses().is_empty());
        let alternative = Policy::new(1, vec![
            Predicate::Absent { key: 7 }, Predicate::PayloadAtMost(100), Predicate::Any(vec![0, 1]),
        ]).unwrap();
        let result = alternative.evaluate(&action(), &snapshot).unwrap();
        assert_eq!(result.result(), Truth::Satisfied);
        assert!(!result.certifiable());
    }

    #[test]
    fn graph_shape_validation_refuses_cycles_and_unreachable_decoys() {
        for nodes in [vec![], vec![Predicate::Not(0)],
            vec![Predicate::All(vec![])], vec![Predicate::Any(vec![])],
            vec![Predicate::UnitsAtMost(10), Predicate::UnitsAtMost(10)],
            vec![Predicate::EmptyRange { start: 10, end: 10 }],
        ] {
            assert_eq!(Policy::new(1, nodes), Err(Error::InvalidInput));
        }
        assert_eq!(Policy::new(0, vec![Predicate::UnitsAtMost(1)]), Err(Error::InvalidInput));
    }

    #[test]
    fn exact_and_one_over_graph_and_literal_limits() {
        let mut nodes = vec![Predicate::UnitsAtMost(10)];
        for index in 1..MAX_POLICY_NODES {
            nodes.push(Predicate::Not(index - 1));
        }
        assert!(Policy::new(1, nodes.clone()).is_ok());
        nodes.push(Predicate::Not(MAX_POLICY_NODES - 1));
        assert_eq!(Policy::new(1, nodes), Err(Error::Limit));
        let edges = |count| vec![Predicate::UnitsAtMost(10), Predicate::All(vec![0; count])];
        assert!(Policy::new(1, edges(MAX_POLICY_EDGES)).is_ok());
        assert_eq!(Policy::new(1, edges(MAX_POLICY_EDGES + 1)), Err(Error::Limit));
        assert!(Policy::new(1, vec![Predicate::PayloadIs(vec![0; MAX_POLICY_LITERAL_BYTES])]).is_ok());
        assert_eq!(Policy::new(1, vec![Predicate::PayloadIs(vec![0; MAX_POLICY_LITERAL_BYTES + 1])]), Err(Error::Limit));
    }

    #[test]
    fn read_and_retained_occurrence_limits_are_independent() {
        let nodes = |count| {
            let mut nodes: Vec<_> = (0..count).map(|_| Predicate::Absent { key: 7 }).collect();
            nodes.push(Predicate::All((0..count).collect()));
            nodes
        };
        assert!(Policy::new(1, nodes(MAX_REQUIRED_WITNESSES)).is_ok());
        assert_eq!(Policy::new(1, nodes(MAX_REQUIRED_WITNESSES + 1)), Err(Error::Limit));
        let policy = Policy::new(1, nodes(2)).unwrap();
        let mut snapshot = snapshot();
        snapshot.values.insert(7, vec![0; MAX_WITNESS_BYTES / 2]);
        assert!(policy.evaluate(&action(), &snapshot).is_ok());
        snapshot.values.get_mut(&7).unwrap().push(0);
        assert_eq!(policy.evaluate(&action(), &snapshot), Err(Error::Limit));
        snapshot.complete = false;
        assert_eq!(policy.evaluate(&action(), &snapshot).unwrap().result(), Truth::Unknown);
    }

    #[test]
    fn extreme_range_endpoint_never_uses_an_overflowing_successor() {
        let policy = Policy::new(1, vec![Predicate::EmptyRange { start: u64::MAX - 1, end: u64::MAX }]).unwrap();
        let mut snapshot = snapshot();
        snapshot.values.insert(u64::MAX, vec![1]);
        assert!(policy.evaluate(&action(), &snapshot).unwrap().certifiable());
        snapshot.values.insert(u64::MAX - 1, vec![1]);
        assert_eq!(policy.evaluate(&action(), &snapshot).unwrap().result(), Truth::Violated);
    }
}
