//! Whole-input oversight using the actual FA-057/FA-081 public contracts.
//! Capture, transform truth and profile qualification remain trusted inputs.

use crate::Error;
use crate::action::{FrozenAction, Purpose};
use crate::evidence_view::{EvidenceViewManifest, RedactionMetadata};
use crate::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, Omission, PartKind, SubmittedPart};
use crate::reducer::{MAX_IDENTIFIER_BYTES, MAX_VOTES};
use std::collections::BTreeMap;

pub const MAX_COMMITTEE_BYTES: usize = 1_048_576;
const MAX_QUESTION_BYTES: usize = 4_096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelperContract {
    profile: InputProfileBinding,
    projection_id: u64,
    question: Vec<u8>,
}

impl HelperContract {
    pub fn new(profile: InputProfileBinding, projection_id: u64, question: Vec<u8>) -> Result<Self, Error> {
        if projection_id == 0 || question.is_empty() { return Err(Error::InvalidInput); }
        if question.len() > MAX_QUESTION_BYTES { return Err(Error::Limit); }
        ActualHelperInput::new(question.clone(), profile.clone(), vec![SubmittedPart {
            kind: PartKind::Question, span: ByteSpan { start: 0, end: question.len() },
        }], Vec::new())?;
        Ok(Self { profile, projection_id, question })
    }

    pub fn profile_at(&self, policy_epoch: u64) -> InputProfileBinding {
        let mut profile = self.profile.clone();
        profile.policy_epoch = policy_epoch;
        profile
    }

    pub fn projection_id(&self) -> u64 { self.projection_id }
    pub fn question(&self) -> &[u8] { &self.question }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitteeContract { members: BTreeMap<String, HelperContract> }

impl CommitteeContract {
    pub fn new(members: BTreeMap<String, HelperContract>) -> Result<Self, Error> {
        if members.is_empty() { return Err(Error::InvalidInput); }
        if members.len() > MAX_VOTES { return Err(Error::Limit); }
        for name in members.keys() {
            if name.is_empty() { return Err(Error::InvalidInput); }
            if name.len() > MAX_IDENTIFIER_BYTES { return Err(Error::Limit); }
        }
        Ok(Self { members })
    }

    pub fn members(&self) -> &BTreeMap<String, HelperContract> { &self.members }
}

/// Immutable evidence, never a permit. The full local action is retained but
/// its private read witnesses are not copied into helper submissions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitteeInput {
    action: FrozenAction,
    views: BTreeMap<String, EvidenceViewManifest>,
    logical_bytes: usize,
}

impl CommitteeInput {
    pub fn capture(action: &FrozenAction, contract: &CommitteeContract, views: BTreeMap<String, EvidenceViewManifest>) -> Result<Self, Error> {
        let logical_bytes = validate(action, contract, &views)?;
        Ok(Self { action: action.clone(), views, logical_bytes })
    }

    pub fn action(&self) -> &FrozenAction { &self.action }
    pub fn views(&self) -> &BTreeMap<String, EvidenceViewManifest> { &self.views }
    pub fn logical_bytes(&self) -> usize { self.logical_bytes }

    pub fn validate_for(&self, action: &FrozenAction, contract: &CommitteeContract) -> Result<(), Error> {
        if &self.action != action { return Err(Error::Binding); }
        if validate(action, contract, &self.views)? != self.logical_bytes { return Err(Error::Binding); }
        Ok(())
    }
}

/// Exact execution-bearing frame. It is the first Other part in the ordered
/// input, not an ordinal field that the underlying input type does not contain.
pub fn action_frame(action: &FrozenAction) -> Vec<u8> {
    let spec = action.spec();
    let target = spec.target.expect("frozen resolved action");
    let mut bytes = b"fa/helper-action/v1\0".to_vec();
    bytes.extend_from_slice(&spec.version.to_be_bytes());
    bytes.push(match spec.scope.purpose { Purpose::Effect => 1, Purpose::Experiment => 2 });
    for value in [spec.scope.tenant, spec.scope.principal, spec.scope.run,
        spec.scope.branch, spec.scope.authority, target.adapter, target.object,
        target.contract_version, target.expected_version, target.generation,
        spec.policy_epoch, spec.deadline.0, spec.units, spec.payload.len() as u64]
    {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    bytes.extend_from_slice(&spec.payload);
    bytes
}

fn validate(action: &FrozenAction, contract: &CommitteeContract, views: &BTreeMap<String, EvidenceViewManifest>) -> Result<usize, Error> {
    if views.len() > MAX_VOTES { return Err(Error::Limit); }
    if views.len() != contract.members.len() || !views.keys().eq(contract.members.keys()) { return Err(Error::Binding); }
    if action.spec().scope.purpose != Purpose::Effect { return Err(Error::Binding); }
    let frame = action_frame(action);
    let mut bytes = action.spec().payload.len();
    for witness in &action.spec().required_witnesses {
        if let crate::ReadWitness::Exact { value: Some(value), .. } = witness { charge(&mut bytes, value.len())?; }
    }
    for (member, manifest) in views {
        let expected = &contract.members[member];
        let input = manifest.actual_input();
        let projection = manifest.authorization();
        if input.input_profile() != &expected.profile_at(action.spec().policy_epoch)
            || projection.projection_id != expected.projection_id
            || projection.policy_epoch != action.spec().policy_epoch
            || projection.projected_originals.iter().any(|o| o.tenant_id != action.spec().scope.tenant)
            || manifest.evidence_parts().iter().any(|v| v.original.tenant_id != action.spec().scope.tenant)
        {
            return Err(Error::Binding);
        }
        if input.omissions().iter().any(|o| !matches!(o, Omission::ClosedAbsent { .. }))
            || manifest.evidence_parts().iter().any(|v| v.window.truncated || v.redaction != RedactionMetadata::None)
        {
            return Err(Error::Incomplete);
        }
        let action_part = input.ordered_parts().iter().position(|p| p.kind == PartKind::Other).ok_or(Error::Incomplete)?;
        let question = input.ordered_parts().iter().position(|p| p.kind == PartKind::Question).ok_or(Error::Incomplete)?;
        if input.part_bytes(action_part)? != frame.as_slice() || input.part_bytes(question)? != expected.question.as_slice() {
            return Err(Error::Binding);
        }
        // These are the actual variable-length buffers retained by the manifest.
        // Numeric identities and fixed metadata are not fabricated string copies.
        charge(&mut bytes, member.len())?;
        charge(&mut bytes, input.submitted_bytes().len())?;
        charge(&mut bytes, input.input_profile().profile_bytes.len())?;
    }
    Ok(bytes)
}

fn charge(total: &mut usize, bytes: usize) -> Result<(), Error> {
    *total = total.checked_add(bytes).ok_or(Error::Limit)?;
    if *total > MAX_COMMITTEE_BYTES { return Err(Error::Limit); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{ActionSpec, ElapsedTick, ResolvedTarget, Scope, VERSION};
    use crate::evidence_view::AuthorizationProjection;

    fn action() -> FrozenAction {
        FrozenAction::freeze(ActionSpec {
            version: VERSION, scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            target: Some(ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1 }),
            payload: b"publish".to_vec(), required_witnesses: Vec::new(), policy_epoch: 0, deadline: ElapsedTick(100), units: 10,
        }).unwrap()
    }
    fn contract() -> CommitteeContract {
        CommitteeContract::new(BTreeMap::from([("alice".to_owned(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: b"framed-action-v1".to_vec(), model_epoch: 1, tokenizer_epoch: 1, policy_epoch: 0 },
            7, b"approve?".to_vec(),
        ).unwrap())])).unwrap()
    }
    fn view(action: &FrozenAction, contract: &CommitteeContract) -> EvidenceViewManifest {
        let helper = &contract.members()["alice"];
        let mut bytes = action_frame(action);
        let boundary = bytes.len();
        bytes.extend_from_slice(helper.question());
        let end = bytes.len();
        let input = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
            SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
            SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
        ], Vec::new()).unwrap();
        EvidenceViewManifest::new(input, AuthorizationProjection {
            projection_id: 7, policy_epoch: action.spec().policy_epoch, projected_originals: Vec::new(),
        }, Vec::new()).unwrap()
    }
    #[test]
    fn complete_roster_action_and_question_are_bound() {
        let action = action(); let contract = contract();
        let views = BTreeMap::from([("alice".to_owned(), view(&action, &contract))]);
        let input = CommitteeInput::capture(&action, &contract, views.clone()).unwrap();
        input.validate_for(&action, &contract).unwrap();
        assert!(input.logical_bytes() > action.spec().payload.len());
        assert_eq!(CommitteeInput::capture(&action, &contract, BTreeMap::new()), Err(Error::Binding));
        let mut changed = action.spec().clone(); changed.payload.push(1);
        assert_eq!(CommitteeInput::capture(&FrozenAction::freeze(changed).unwrap(), &contract, views), Err(Error::Binding));
    }
    #[test]
    fn every_execution_field_changes_the_frame_but_private_witnesses_do_not() {
        let original = action();
        for field in 0..14 {
            let mut spec = original.spec().clone();
            match field {
                0 => spec.scope.tenant += 1, 1 => spec.scope.principal += 1, 2 => spec.scope.run += 1,
                3 => spec.scope.branch += 1, 4 => spec.scope.authority += 1, 5 => spec.scope.purpose = Purpose::Experiment,
                6 => spec.target.as_mut().unwrap().adapter += 1, 7 => spec.target.as_mut().unwrap().object += 1,
                8 => spec.target.as_mut().unwrap().contract_version += 1, 9 => spec.target.as_mut().unwrap().expected_version += 1,
                10 => spec.target.as_mut().unwrap().generation += 1, 11 => spec.policy_epoch += 1,
                12 => spec.deadline.0 += 1, _ => spec.units += 1,
            }
            assert_ne!(action_frame(&original), action_frame(&FrozenAction::freeze(spec).unwrap()));
        }
        let mut spec = original.spec().clone();
        spec.required_witnesses.push(crate::ReadWitness::Exact { key: 1, value: Some(b"local-secret".to_vec()) });
        assert_eq!(action_frame(&original), action_frame(&FrozenAction::freeze(spec).unwrap()));
    }
    #[test]
    fn unchanged_bytes_cannot_hide_a_profile_or_question_substitution() {
        let action = action(); let contract = contract(); let manifest = view(&action, &contract);
        for changed_question in [false, true] {
            let mut members = contract.members.clone();
            if changed_question { members.get_mut("alice").unwrap().question.push(b'?'); }
            else { members.get_mut("alice").unwrap().profile.model_epoch += 1; }
            let other = CommitteeContract::new(members).unwrap();
            assert_eq!(CommitteeInput::capture(&action, &other, BTreeMap::from([("alice".to_owned(), manifest.clone())])), Err(Error::Binding));
        }
    }
}
