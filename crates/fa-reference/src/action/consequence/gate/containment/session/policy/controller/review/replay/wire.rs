//! FA decision-reference wire profile v1. Big-endian fixed integers, explicit
//! enum tags and length-framed UTF-8/bytes; no normalization or unknown fields.
//! Decoding is bounded, exact-length and canonical. A decoded capsule is still
//! unauthenticated data; verify it against the independently retained anchor.

use super::*;
use crate::action::{ActionSpec, ElapsedTick, MAX_ATTEMPTS, MAX_PAYLOAD_BYTES, Purpose, ResolvedTarget, Scope};
use crate::action::consequence::Rule;
use crate::action::consequence::congress::MemberPolicy;
use crate::action::consequence::gate::containment::session::transcript::{CommitRecord, RevealRecord};
use crate::action::consequence::gate::containment::session::policy::{
    MAX_POLICY_EDGES, MAX_POLICY_LITERAL_BYTES, MAX_POLICY_NODES, Predicate, Step,
};
use crate::reducer::{Caps, MAX_IDENTIFIER_BYTES, MAX_VOTES, Outcome};
use crate::round::{MAX_FIELD_LEN, Verdict};

pub const MAX_ARCHIVE_BYTES: usize = 2_097_152;
const ARCHIVE_MAGIC: &[u8] = b"FA-DECISION-REFERENCE\0";
const ANCHOR_MAGIC: &[u8] = b"FA-REVIEW-ANCHOR\0";

impl DecisionArchive {
    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        self.verify(&self.anchor)?;
        let mut out = Writer(ARCHIVE_MAGIC.to_vec());
        out.number(u64::from(self.version));
        out.anchor(&self.anchor);
        out.transcript(&self.transcript);
        out.evaluation(&self.evaluation);
        out.0.extend_from_slice(&self.decision.consequence.encode());
        out.length(self.decision.rules.len());
        for rule in &self.decision.rules {
            out.byte(match rule {
                Rule::EmpiricalRecommendation => 1, Rule::ExactDisqualifier => 2,
                Rule::MandatoryAbsent => 3, Rule::Contradiction => 4,
            });
        }
        out.byte(match self.tally.outcome { Outcome::Empirical => 1, Outcome::Disqualified => 2 });
        out.number(self.tally.permit_weight);
        out.number(self.tally.hold_weight);
        out.weights(&self.tally.admitted_weights);
        out.weights(&self.tally.admitted_cohort_weights);
        out.names(&self.missing);
        out.names(&self.abstained);
        out.finish()
    }

    /// Checks self-consistency, not provenance. A consumer must still call
    /// verify with its separately retained pre-vote anchor, not this one's copy.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let mut input = Reader::new(bytes, ARCHIVE_MAGIC)?;
        let version = u32::try_from(input.number()?).map_err(|_| Error::InvalidInput)?;
        if version != DECISION_ARCHIVE_VERSION { return Err(Error::InvalidInput); }
        let anchor = input.anchor()?;
        let transcript = input.transcript()?;
        let evaluation = input.evaluation()?;
        let consequence = Consequence::decode(input.take(2)?)?;
        let count = input.length(4)?;
        let mut rules = Vec::with_capacity(count);
        for _ in 0..count {
            rules.push(match input.byte()? {
                1 => Rule::EmpiricalRecommendation, 2 => Rule::ExactDisqualifier,
                3 => Rule::MandatoryAbsent, 4 => Rule::Contradiction,
                _ => return Err(Error::InvalidInput),
            });
        }
        let outcome = match input.byte()? {
            1 => Outcome::Empirical, 2 => Outcome::Disqualified,
            _ => return Err(Error::InvalidInput),
        };
        let tally = Reduction {
            outcome, permit_weight: input.number()?, hold_weight: input.number()?,
            admitted_weights: input.weights()?, admitted_cohort_weights: input.weights()?,
        };
        let archive = Self {
            version, anchor, transcript, evaluation, decision: Decision { consequence, rules },
            tally, missing: input.names()?, abstained: input.names()?,
        };
        input.finish()?;
        archive.verify(&archive.anchor)?;
        if archive.to_bytes()?.as_slice() != bytes { return Err(Error::Binding); }
        Ok(archive)
    }

    pub fn verify_bytes(bytes: &[u8], expected: &ReviewAnchor) -> Result<ReplayedDecision, Error> {
        Self::from_bytes(bytes)?.verify(expected)
    }
}

impl ReviewAnchor {
    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        super::super::super::validate_congress(&self.congress)?;
        validate_observations(&self.observations)?;
        if !self.complete { return Err(Error::Incomplete); }
        if self.attempt == 0 || self.round == 0 || self.evidence_root == [0; 32] {
            return Err(Error::InvalidInput);
        }
        let snapshot = reconstruct_snapshot(self)?;
        if self.policy.evaluate(&self.action, &snapshot)?.witnesses() != self.observations.as_slice() {
            return Err(Error::Binding);
        }
        let mut out = Writer(ANCHOR_MAGIC.to_vec());
        out.number(1);
        out.anchor(self);
        out.finish()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let mut input = Reader::new(bytes, ANCHOR_MAGIC)?;
        if input.number()? != 1 { return Err(Error::InvalidInput); }
        let anchor = input.anchor()?;
        input.finish()?;
        if anchor.to_bytes()?.as_slice() != bytes { return Err(Error::Binding); }
        Ok(anchor)
    }
}

struct Writer(Vec<u8>);

impl Writer {
    fn byte(&mut self, value: u8) { self.0.push(value); }
    fn number(&mut self, value: u64) { self.0.extend_from_slice(&value.to_be_bytes()); }
    fn length(&mut self, value: usize) { self.number(value as u64); }
    fn bytes(&mut self, value: &[u8]) { self.length(value.len()); self.0.extend_from_slice(value); }
    fn text(&mut self, value: &str) { self.bytes(value.as_bytes()); }
    fn flag(&mut self, value: bool) { self.byte(u8::from(value)); }
    fn names(&mut self, values: &[String]) {
        self.length(values.len());
        for value in values { self.text(value); }
    }
    fn target(&mut self, t: ResolvedTarget) {
        for field in [t.adapter, t.object, t.contract_version, t.expected_version, t.generation] {
            self.number(field);
        }
    }
    fn witnesses(&mut self, values: &[ReadWitness]) {
        self.length(values.len());
        for value in values {
            match value {
                ReadWitness::Exact { key, value } => {
                    self.byte(1); self.number(*key); self.flag(value.is_some());
                    if let Some(value) = value { self.bytes(value); }
                }
                ReadWitness::EmptyRange { start, end } => {
                    self.byte(2); self.number(*start); self.number(*end);
                }
            }
        }
    }
    fn action(&mut self, action: &FrozenAction) {
        let action = action.spec();
        self.number(u64::from(action.version));
        for field in [action.scope.tenant, action.scope.principal, action.scope.run,
            action.scope.branch, action.scope.authority]
        { self.number(field); }
        self.byte(match action.scope.purpose { Purpose::Effect => 1, Purpose::Experiment => 2 });
        self.flag(action.target.is_some());
        if let Some(target) = action.target { self.target(target); }
        self.bytes(&action.payload);
        self.witnesses(&action.required_witnesses);
        self.number(action.policy_epoch); self.number(action.deadline.0); self.number(action.units);
    }
    fn policy(&mut self, policy: &Policy) {
        self.number(policy.generation());
        self.length(policy.nodes().len());
        for node in policy.nodes() {
            match node {
                Predicate::TargetIs(value) => { self.byte(1); self.target(*value); }
                Predicate::PayloadIs(value) => { self.byte(2); self.bytes(value); }
                Predicate::PayloadAtMost(value) => { self.byte(3); self.length(*value); }
                Predicate::UnitsAtMost(value) => { self.byte(4); self.number(*value); }
                Predicate::ExactValue { key, value } => {
                    self.byte(5); self.number(*key); self.bytes(value);
                }
                Predicate::Absent { key } => { self.byte(6); self.number(*key); }
                Predicate::EmptyRange { start, end } => {
                    self.byte(7); self.number(*start); self.number(*end);
                }
                Predicate::All(children) | Predicate::Any(children) => {
                    self.byte(if matches!(node, Predicate::All(_)) { 8 } else { 9 });
                    self.length(children.len());
                    for child in children { self.length(*child); }
                }
                Predicate::Not(child) => { self.byte(10); self.length(*child); }
            }
        }
    }
    fn congress(&mut self, policy: &CongressPolicy) {
        self.number(policy.generation); self.length(policy.members.len());
        for (member, profile) in &policy.members {
            self.text(member); self.text(&profile.cohort); self.number(profile.weight);
        }
        for value in [policy.caps.per_member, policy.caps.per_cohort, policy.continue_minimum,
            policy.continue_hold_maximum, policy.narrow_at, policy.suspend_at]
        { self.number(value); }
        self.length(policy.minimum_members); self.length(policy.minimum_cohorts);
    }
    fn anchor(&mut self, anchor: &ReviewAnchor) {
        self.number(anchor.attempt); self.action(&anchor.action);
        self.number(anchor.expected_control_sequence); self.number(anchor.round);
        self.0.extend_from_slice(&anchor.evidence_root);
        self.policy(&anchor.policy); self.congress(&anchor.congress);
        self.length(anchor.narrowed_targets.0.len());
        for key in &anchor.narrowed_targets.0 { for field in key { self.number(*field); } }
        self.number(anchor.snapshot_semantic_epoch); self.witnesses(&anchor.observations);
        self.flag(anchor.complete); self.flag(anchor.contradiction);
    }
    fn transcript(&mut self, transcript: &RoundTranscript) {
        self.number(u64::from(transcript.version)); self.number(transcript.round);
        self.0.extend_from_slice(&transcript.evidence_root); self.names(&transcript.members);
        self.length(transcript.commits.len());
        for entry in &transcript.commits { self.text(&entry.member); self.number(entry.digest); }
        self.length(transcript.reveals.len());
        for entry in &transcript.reveals {
            self.text(&entry.member);
            self.byte(match entry.verdict {
                Verdict::Allow => 1, Verdict::Hold => 2, Verdict::Deny => 3, Verdict::Abstain => 4,
            });
            self.bytes(&entry.salt);
        }
    }
    fn truth(&mut self, truth: Truth) {
        self.byte(match truth { Truth::Satisfied => 1, Truth::Violated => 2, Truth::Unknown => 3 });
    }
    fn evaluation(&mut self, evaluation: &Evaluation) {
        self.number(evaluation.generation()); self.truth(evaluation.result());
        self.length(evaluation.trace().len());
        for step in evaluation.trace() {
            self.length(step.node); self.truth(step.result); self.flag(step.witness.is_some());
            if let Some(witness) = step.witness { self.length(witness); }
        }
        self.witnesses(evaluation.witnesses()); self.flag(evaluation.complete);
    }
    fn weights(&mut self, values: &BTreeMap<String, u64>) {
        self.length(values.len());
        for (name, weight) in values { self.text(name); self.number(*weight); }
    }
    fn finish(self) -> Result<Vec<u8>, Error> {
        if self.0.len() > MAX_ARCHIVE_BYTES { Err(Error::Limit) } else { Ok(self.0) }
    }
}

struct Reader<'a> { bytes: &'a [u8], offset: usize }

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8], magic: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_ARCHIVE_BYTES { return Err(Error::Limit); }
        let mut input = Self { bytes, offset: 0 };
        if input.take(magic.len())? != magic { return Err(Error::InvalidInput); }
        Ok(input)
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], Error> {
        let end = self.offset.checked_add(count).ok_or(Error::Limit)?;
        let result = self.bytes.get(self.offset..end).ok_or(Error::Incomplete)?;
        self.offset = end;
        Ok(result)
    }
    fn byte(&mut self) -> Result<u8, Error> { Ok(self.take(1)?[0]) }
    fn number(&mut self) -> Result<u64, Error> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().map_err(|_| Error::Incomplete)?))
    }
    fn length(&mut self, limit: usize) -> Result<usize, Error> {
        let value = usize::try_from(self.number()?).map_err(|_| Error::Limit)?;
        if value > limit { Err(Error::Limit) } else { Ok(value) }
    }
    fn bytes(&mut self, limit: usize) -> Result<Vec<u8>, Error> {
        let length = self.length(limit)?;
        Ok(self.take(length)?.to_vec())
    }
    fn text(&mut self) -> Result<String, Error> {
        String::from_utf8(self.bytes(MAX_IDENTIFIER_BYTES)?).map_err(|_| Error::InvalidInput)
    }
    fn flag(&mut self) -> Result<bool, Error> {
        match self.byte()? { 0 => Ok(false), 1 => Ok(true), _ => Err(Error::InvalidInput) }
    }
    fn names(&mut self) -> Result<Vec<String>, Error> {
        let count = self.length(MAX_VOTES)?;
        (0..count).map(|_| self.text()).collect()
    }
    fn target(&mut self) -> Result<ResolvedTarget, Error> {
        Ok(ResolvedTarget {
            adapter: self.number()?, object: self.number()?, contract_version: self.number()?,
            expected_version: self.number()?, generation: self.number()?,
        })
    }
    fn witnesses(&mut self) -> Result<Vec<ReadWitness>, Error> {
        let count = self.length(MAX_REQUIRED_WITNESSES)?;
        let mut values = Vec::with_capacity(count);
        let mut retained = 0_usize;
        for _ in 0..count {
            values.push(match self.byte()? {
                1 => {
                    let key = self.number()?;
                    let value = if self.flag()? {
                        let bytes = self.bytes(MAX_WITNESS_BYTES - retained)?;
                        retained += bytes.len(); Some(bytes)
                    } else { None };
                    ReadWitness::Exact { key, value }
                }
                2 => ReadWitness::EmptyRange { start: self.number()?, end: self.number()? },
                _ => return Err(Error::InvalidInput),
            });
        }
        validate_observations(&values)?;
        Ok(values)
    }
    fn action(&mut self) -> Result<FrozenAction, Error> {
        let version = u32::try_from(self.number()?).map_err(|_| Error::InvalidInput)?;
        let scope = Scope {
            tenant: self.number()?, principal: self.number()?, run: self.number()?,
            branch: self.number()?, authority: self.number()?,
            purpose: match self.byte()? {
                1 => Purpose::Effect, 2 => Purpose::Experiment, _ => return Err(Error::InvalidInput),
            },
        };
        let target = if self.flag()? { Some(self.target()?) } else { None };
        FrozenAction::freeze(ActionSpec {
            version, scope, target, payload: self.bytes(MAX_PAYLOAD_BYTES)?,
            required_witnesses: self.witnesses()?, policy_epoch: self.number()?,
            deadline: ElapsedTick(self.number()?), units: self.number()?,
        })
    }
    fn policy(&mut self) -> Result<Policy, Error> {
        let generation = self.number()?;
        let count = self.length(MAX_POLICY_NODES)?;
        let mut nodes = Vec::with_capacity(count);
        let mut literals = 0_usize;
        let mut edges = 0_usize;
        for _ in 0..count {
            let tag = self.byte()?;
            nodes.push(match tag {
                1 => Predicate::TargetIs(self.target()?),
                2 => {
                    let value = self.bytes(MAX_POLICY_LITERAL_BYTES - literals)?;
                    literals += value.len(); Predicate::PayloadIs(value)
                }
                3 => Predicate::PayloadAtMost(self.length(usize::MAX)?),
                4 => Predicate::UnitsAtMost(self.number()?),
                5 => {
                    let key = self.number()?;
                    let value = self.bytes(MAX_POLICY_LITERAL_BYTES - literals)?;
                    literals += value.len(); Predicate::ExactValue { key, value }
                }
                6 => Predicate::Absent { key: self.number()? },
                7 => Predicate::EmptyRange { start: self.number()?, end: self.number()? },
                8 | 9 => {
                    let length = self.length(MAX_POLICY_EDGES - edges)?;
                    edges += length;
                    let children: Vec<_> = (0..length).map(|_| self.length(MAX_POLICY_NODES)).collect::<Result<_, _>>()?;
                    if tag == 8 { Predicate::All(children) } else { Predicate::Any(children) }
                }
                10 => {
                    if edges == MAX_POLICY_EDGES { return Err(Error::Limit); }
                    edges += 1; Predicate::Not(self.length(MAX_POLICY_NODES)?)
                }
                _ => return Err(Error::InvalidInput),
            });
        }
        Policy::new(generation, nodes)
    }
    fn congress(&mut self) -> Result<CongressPolicy, Error> {
        let generation = self.number()?;
        let count = self.length(MAX_VOTES)?;
        let mut members = BTreeMap::new();
        for _ in 0..count {
            let member = self.text()?;
            let profile = MemberPolicy { cohort: self.text()?, weight: self.number()? };
            if members.insert(member, profile).is_some() { return Err(Error::Duplicate); }
        }
        Ok(CongressPolicy {
            generation, members,
            caps: Caps { per_member: self.number()?, per_cohort: self.number()? },
            continue_minimum: self.number()?, continue_hold_maximum: self.number()?,
            narrow_at: self.number()?, suspend_at: self.number()?,
            minimum_members: self.length(MAX_VOTES)?, minimum_cohorts: self.length(MAX_VOTES)?,
        })
    }
    fn anchor(&mut self) -> Result<ReviewAnchor, Error> {
        let attempt = self.number()?;
        let action = self.action()?;
        let expected_control_sequence = self.number()?;
        let round = self.number()?;
        let evidence_root = self.take(32)?.try_into().map_err(|_| Error::Incomplete)?;
        let policy = self.policy()?;
        let congress = self.congress()?;
        let count = self.length(MAX_ATTEMPTS)?;
        let targets: Vec<_> = (0..count).map(|_| self.target()).collect::<Result<_, _>>()?;
        Ok(ReviewAnchor {
            attempt, action, expected_control_sequence, round, evidence_root, policy, congress,
            narrowed_targets: TargetCeiling::new(&targets)?, snapshot_semantic_epoch: self.number()?,
            observations: self.witnesses()?, complete: self.flag()?, contradiction: self.flag()?,
        })
    }
    fn transcript(&mut self) -> Result<RoundTranscript, Error> {
        let version = u32::try_from(self.number()?).map_err(|_| Error::InvalidInput)?;
        let round = self.number()?;
        let evidence_root = self.take(32)?.try_into().map_err(|_| Error::Incomplete)?;
        let members = self.names()?;
        let count = self.length(MAX_VOTES)?;
        let commits = (0..count).map(|_| Ok(CommitRecord {
            member: self.text()?, digest: self.number()?,
        })).collect::<Result<_, Error>>()?;
        let count = self.length(MAX_VOTES)?;
        let mut reveals = Vec::with_capacity(count);
        for _ in 0..count {
            let member = self.text()?;
            let verdict = match self.byte()? {
                1 => Verdict::Allow, 2 => Verdict::Hold, 3 => Verdict::Deny, 4 => Verdict::Abstain,
                _ => return Err(Error::InvalidInput),
            };
            reveals.push(RevealRecord { member, verdict, salt: self.bytes(MAX_FIELD_LEN)? });
        }
        Ok(RoundTranscript { version, round, evidence_root, members, commits, reveals })
    }
    fn truth(&mut self) -> Result<Truth, Error> {
        match self.byte()? {
            1 => Ok(Truth::Satisfied), 2 => Ok(Truth::Violated), 3 => Ok(Truth::Unknown),
            _ => Err(Error::InvalidInput),
        }
    }
    fn evaluation(&mut self) -> Result<Evaluation, Error> {
        let generation = self.number()?;
        let result = self.truth()?;
        let count = self.length(MAX_POLICY_NODES)?;
        let mut trace = Vec::with_capacity(count);
        for _ in 0..count {
            let node = self.length(MAX_POLICY_NODES)?;
            let result = self.truth()?;
            let witness = if self.flag()? { Some(self.length(MAX_REQUIRED_WITNESSES)?) } else { None };
            trace.push(Step { node, result, witness });
        }
        Ok(Evaluation { generation, result, trace, witnesses: self.witnesses()?, complete: self.flag()? })
    }
    fn weights(&mut self) -> Result<BTreeMap<String, u64>, Error> {
        let count = self.length(MAX_VOTES)?;
        let mut weights = BTreeMap::new();
        for _ in 0..count {
            if weights.insert(self.text()?, self.number()?).is_some() { return Err(Error::Duplicate); }
        }
        Ok(weights)
    }
    fn finish(self) -> Result<(), Error> {
        if self.offset == self.bytes.len() { Ok(()) } else { Err(Error::InvalidInput) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_golden_bytes_have_fixed_endian_lengths_and_utf8() {
        let mut out = Writer(Vec::new());
        out.number(0x0102_0304_0506_0708); out.text("\u{00e9}"); out.flag(true);
        assert_eq!(out.0, vec![1,2,3,4,5,6,7,8, 0,0,0,0,0,0,0,2, 0xc3,0xa9,1]);
        let mut input = Reader { bytes: &out.0, offset: 0 };
        assert_eq!(input.number().unwrap(), 0x0102_0304_0506_0708);
        assert_eq!(input.text().unwrap(), "\u{00e9}");
        assert!(input.flag().unwrap()); input.finish().unwrap();
    }

    #[test]
    fn oversized_lengths_invalid_utf8_and_boolean_tags_refuse() {
        let raw = u64::MAX.to_be_bytes();
        let mut input = Reader { bytes: &raw, offset: 0 };
        assert_eq!(input.bytes(8), Err(Error::Limit));
        let raw = [0,0,0,0,0,0,0,1,0xff];
        let mut input = Reader { bytes: &raw, offset: 0 };
        assert_eq!(input.text(), Err(Error::InvalidInput));
        let mut input = Reader { bytes: &[2], offset: 0 };
        assert_eq!(input.flag(), Err(Error::InvalidInput));
    }
}
