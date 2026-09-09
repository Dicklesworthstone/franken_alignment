//! Compose existing full-input and view manifests with the reviewed action.
//! Equality includes every submitted byte, part, source and profile identity.
//! The complete-view profile refuses declared truncation or unavailable inputs.

use crate::Error;
use crate::action::{FrozenAction, Purpose};
use crate::evidence_view::{EvidenceViewManifest, RedactionBinding};
use crate::full_input::{ActualHelperInput, InputPart, InputProfileBinding, OmissionKind, PartKind};
use crate::reducer::{MAX_IDENTIFIER_BYTES, MAX_VOTES};
use std::collections::BTreeMap;

pub const MAX_COMMITTEE_BYTES: usize = 1_048_576;
const MAX_QUESTION_BYTES: usize = 4_096;

/// Trusted helper identity and question. Only the input's policy epoch is
/// instantiated from the current frozen action; model/tokenizer/profile epochs
/// are never silently advanced. This is a declared contract, not authentication.
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
        ActualHelperInput::new(profile.clone(), question.clone(), vec![InputPart {
            kind: PartKind::Question, ordinal: 0, start: 0, end: question.len(),
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
pub struct CommitteeContract {
    members: BTreeMap<String, HelperContract>,
}

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

/// Immutable data: callers cannot edit a previously checked committee view.
/// The full local action is retained, but its private read witnesses are NOT
/// copied into helper submissions by action_frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitteeInput {
    action: FrozenAction,
    views: BTreeMap<String, EvidenceViewManifest>,
    logical_bytes: usize,
}

impl CommitteeInput {
    pub fn capture(
        action: &FrozenAction,
        contract: &CommitteeContract,
        views: BTreeMap<String, EvidenceViewManifest>,
    ) -> Result<Self, Error> {
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

/// Unambiguous reference input contract: fixed field order, big-endian integers,
/// explicit purpose and a length-framed payload. Insert these exact bytes as
/// Other/ordinal 0 in ActualHelperInput. No unchecked text name stands for the
/// action. This is not a cryptographic commitment or an inference tokenizer.
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

fn validate(
    action: &FrozenAction,
    contract: &CommitteeContract,
    views: &BTreeMap<String, EvidenceViewManifest>,
) -> Result<usize, Error> {
    if views.len() > MAX_VOTES { return Err(Error::Limit); }
    if views.len() != contract.members.len() || !views.keys().eq(contract.members.keys()) {
        return Err(Error::Binding);
    }
    if action.spec().scope.purpose != Purpose::Effect { return Err(Error::Binding); }
    let frame = action_frame(action);
    let mut bytes = action.spec().payload.len();
    for witness in &action.spec().required_witnesses {
        if let crate::ReadWitness::Exact { value: Some(value), .. } = witness {
            charge(&mut bytes, value.len())?;
        }
    }
    for (member, manifest) in views {
        let expected = &contract.members[member];
        let input = manifest.input();
        let spec = manifest.spec();
        if input.profile() != &expected.profile_at(action.spec().policy_epoch)
            || spec.projection.tenant != action.spec().scope.tenant
            || spec.projection.projection_id != expected.projection_id
        {
            return Err(Error::Binding);
        }
        if input.omissions().iter().any(|o| o.status != OmissionKind::ClosedAbsent)
            || spec.views.iter().any(|v| v.window.is_truncated() || v.redaction != RedactionBinding::None)
        {
            return Err(Error::Incomplete);
        }
        let action_part = input.parts().iter().position(|p| p.kind == PartKind::Other && p.ordinal == 0)
            .ok_or(Error::Incomplete)?;
        let question = input.parts().iter().position(|p| p.kind == PartKind::Question)
            .ok_or(Error::Incomplete)?;
        if input.part_bytes(action_part) != Some(frame.as_slice())
            || input.part_bytes(question) != Some(expected.question.as_slice())
        {
            return Err(Error::Binding);
        }
        charge(&mut bytes, member.len())?;
        charge(&mut bytes, input.submitted().len())?;
        charge(&mut bytes, spec.exact_submitted.len())?;
        let profile = input.profile();
        for label in [&profile.profile_id, &profile.model_space, &profile.tokenizer, &profile.input_contract] {
            charge(&mut bytes, label.len())?;
        }
        for original in &spec.projection.allowed_originals { charge(&mut bytes, original.object_id.len())?; }
        for view in &spec.views {
            charge(&mut bytes, view.original.object_id.len())?;
            charge(&mut bytes, view.transform.contract_id.len())?;
        }
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
    use crate::evidence_view::{ProjectionBinding, ViewSpec};

    fn action() -> FrozenAction {
        FrozenAction::freeze(ActionSpec {
            version: VERSION,
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            target: Some(ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1 }),
            payload: b"publish".to_vec(), required_witnesses: Vec::new(),
            policy_epoch: 0, deadline: ElapsedTick(100), units: 10,
        }).unwrap()
    }

    fn contract() -> CommitteeContract {
        CommitteeContract::new(BTreeMap::from([("alice".to_owned(), HelperContract::new(
            InputProfileBinding { profile_id: "p".to_owned(), generation: 1, model_space: "m".to_owned(),
                model_epoch: 1, tokenizer: "t".to_owned(), tokenizer_epoch: 1, policy_epoch: 0,
                input_contract: "framed-action-v1".to_owned() }, 7, b"approve?".to_vec(),
        ).unwrap())])).unwrap()
    }

    fn view(action: &FrozenAction, contract: &CommitteeContract) -> EvidenceViewManifest {
        let helper = &contract.members()["alice"];
        let mut bytes = action_frame(action);
        let boundary = bytes.len();
        bytes.extend_from_slice(helper.question());
        let input = ActualHelperInput::new(helper.profile_at(action.spec().policy_epoch), bytes.clone(), vec![
            InputPart { kind: PartKind::Other, ordinal: 0, start: 0, end: boundary },
            InputPart { kind: PartKind::Question, ordinal: 0, start: boundary, end: bytes.len() },
        ], Vec::new()).unwrap();
        EvidenceViewManifest::capture(&input, ViewSpec {
            projection: ProjectionBinding { tenant: 1, projection_id: 7, allowed_originals: Vec::new() },
            views: Vec::new(), exact_submitted: bytes,
        }).unwrap()
    }

    #[test]
    fn complete_roster_action_and_question_are_bound() {
        let action = action();
        let contract = contract();
        let views = BTreeMap::from([("alice".to_owned(), view(&action, &contract))]);
        let input = CommitteeInput::capture(&action, &contract, views.clone()).unwrap();
        input.validate_for(&action, &contract).unwrap();
        assert!(input.logical_bytes() > action.spec().payload.len());
        assert_eq!(CommitteeInput::capture(&action, &contract, BTreeMap::new()), Err(Error::Binding));
        let mut changed = action.spec().clone();
        changed.payload.push(1);
        assert_eq!(CommitteeInput::capture(&FrozenAction::freeze(changed).unwrap(), &contract, views), Err(Error::Binding));
    }

    #[test]
    fn every_execution_field_changes_the_frame_but_private_witnesses_do_not() {
        let original = action();
        for field in 0..14 {
            let mut spec = original.spec().clone();
            match field {
                0 => spec.scope.tenant += 1, 1 => spec.scope.principal += 1,
                2 => spec.scope.run += 1, 3 => spec.scope.branch += 1,
                4 => spec.scope.authority += 1, 5 => spec.scope.purpose = Purpose::Experiment,
                6 => spec.target.as_mut().unwrap().adapter += 1,
                7 => spec.target.as_mut().unwrap().object += 1,
                8 => spec.target.as_mut().unwrap().contract_version += 1,
                9 => spec.target.as_mut().unwrap().expected_version += 1,
                10 => spec.target.as_mut().unwrap().generation += 1,
                11 => spec.policy_epoch += 1, 12 => spec.deadline.0 += 1, _ => spec.units += 1,
            }
            assert_ne!(action_frame(&original), action_frame(&FrozenAction::freeze(spec).unwrap()));
        }
        let mut spec = original.spec().clone();
        spec.required_witnesses.push(crate::ReadWitness::Exact { key: 1, value: Some(b"local-secret".to_vec()) });
        assert_eq!(action_frame(&original), action_frame(&FrozenAction::freeze(spec).unwrap()));
    }

    #[test]
    fn unchanged_bytes_cannot_hide_a_profile_or_question_substitution() {
        let action = action();
        let contract = contract();
        let manifest = view(&action, &contract);
        for changed_question in [false, true] {
            let mut members = contract.members.clone();
            if changed_question { members.get_mut("alice").unwrap().question.push(b'?'); }
            else { members.get_mut("alice").unwrap().profile.model_epoch += 1; }
            let other = CommitteeContract::new(members).unwrap();
            assert_eq!(CommitteeInput::capture(&action, &other, BTreeMap::from([
                ("alice".to_owned(), manifest.clone()),
            ])), Err(Error::Binding));
        }
    }
}
