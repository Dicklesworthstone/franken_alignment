//! Canonical, bounded original evidence inputs. Derived scores, policy weights,
//! authority balances and transition receipts never appear in this encoding.
//! Decoding rebuilds the existing ledger; activation still runs in its owner.
use super::CredibilityActivation;
use super::super::codec::shared::{Reader, Writer};
use crate::action::consequence::congress::{CredibilityBinding, CredibilityRequirements};
use crate::action::consequence::congress::credibility::{
    Campaign, CaseSpec, CredibilityLedger, CredibilitySnapshot, EvaluationLabel,
    EvaluationScope, HelperGeneration, LabelSource, LabelVerdict, Observation, MAX_CASES, MAX_STRATA,
};
use crate::action::consequence::gate::containment::{RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::controller::credibility::MAX_CREDIBILITY_OBSERVATIONS;
use crate::reducer::{MAX_IDENTIFIER_BYTES, MAX_VOTES};
use crate::Error;
use std::collections::{BTreeMap, BTreeSet};

pub(in super::super) const MAX_ACTIVATION_BYTES: usize = 4 * 1024 * 1024;
const DOMAIN: &[u8; 8] = b"FACRED\0\x01";

pub(in super::super) fn encode_activation(request: &CredibilityActivation) -> Result<Vec<u8>, Error> {
    let mut w = Writer::new(MAX_ACTIVATION_BYTES);
    w.raw(DOMAIN)?;
    w.u64(request.operation)?; w.u64(request.expected_control_sequence)?; w.u64(request.expected_epoch)?;
    w.scope(request.scope)?; w.u64(request.policy_generation)?;
    let p = request.actor_profile;
    for value in [p.id, p.generation, p.host_generation, p.model_generation,
        p.tokenizer_generation, p.state_schema_generation] { w.u64(value)?; }
    w.u8(match p.grade { RestartGrade::AuditOnly => 0, RestartGrade::FunctionalRestart => 1,
        RestartGrade::ExactRestart => 2 })?;
    write_binding(&mut w, &request.binding)?;
    text(&mut w, &request.stratum)?;
    let r = &request.requirements;
    for value in [r.minimum_safe_cases, r.minimum_violation_cases, r.minimum_precision_ppm,
        r.minimum_timely_recall_ppm, r.maximum_false_positive_ppm, r.base_weight,
        r.lead_bonus_weight, r.lead_saturation_sequences, r.maximum_evidence_age,
        r.maximum_member_share_ppm, r.maximum_cohort_share_ppm] { w.u64(value)?; }
    write_snapshot(&mut w, &request.snapshot)?;
    Ok(w.finish())
}

pub(in super::super) fn decode_activation(bytes: &[u8]) -> Result<CredibilityActivation, Error> {
    if bytes.len() > MAX_ACTIVATION_BYTES { return Err(Error::Limit); }
    let mut r = Reader::new(bytes);
    if r.take(DOMAIN.len())? != DOMAIN { return Err(Error::InvalidInput); }
    let operation = r.u64()?;
    let expected_control_sequence = r.u64()?;
    let expected_epoch = r.u64()?;
    let scope = r.scope()?;
    let policy_generation = r.u64()?;
    let actor_profile = RestartProfile { id: r.u64()?, generation: r.u64()?, host_generation: r.u64()?,
        model_generation: r.u64()?, tokenizer_generation: r.u64()?, state_schema_generation: r.u64()?,
        grade: match r.u8()? { 0 => RestartGrade::AuditOnly, 1 => RestartGrade::FunctionalRestart,
            2 => RestartGrade::ExactRestart, _ => return Err(Error::InvalidInput) } };
    let binding = read_binding(&mut r)?;
    let stratum = read_text(&mut r)?;
    let requirements = CredibilityRequirements {
        minimum_safe_cases: r.u64()?, minimum_violation_cases: r.u64()?, minimum_precision_ppm: r.u64()?,
        minimum_timely_recall_ppm: r.u64()?, maximum_false_positive_ppm: r.u64()?, base_weight: r.u64()?,
        lead_bonus_weight: r.u64()?, lead_saturation_sequences: r.u64()?, maximum_evidence_age: r.u64()?,
        maximum_member_share_ppm: r.u64()?, maximum_cohort_share_ppm: r.u64()?,
    };
    let snapshot = read_snapshot(&mut r)?;
    r.end()?;
    let request = CredibilityActivation { operation, expected_control_sequence, expected_epoch, scope,
        policy_generation, actor_profile, binding, stratum, requirements, snapshot };
    if encode_activation(&request)?.as_slice() != bytes { return Err(Error::Binding); }
    Ok(request)
}

fn text(w: &mut Writer, text: &str) -> Result<(), Error> {
    if text.len() > MAX_IDENTIFIER_BYTES { return Err(Error::Limit); }
    w.blob(text.as_bytes())
}
fn read_text(r: &mut Reader<'_>) -> Result<String, Error> {
    Ok(std::str::from_utf8(r.blob(MAX_IDENTIFIER_BYTES)?).map_err(|_| Error::InvalidInput)?.to_owned())
}
fn write_scope(w: &mut Writer, scope: &EvaluationScope) -> Result<(), Error> {
    w.u64(scope.campaign)?; w.u64(scope.model_generation)?; w.u64(scope.evaluator_generation)?;
    w.raw(&scope.held_out_manifest)
}
fn read_scope(r: &mut Reader<'_>) -> Result<EvaluationScope, Error> {
    Ok(EvaluationScope { campaign: r.u64()?, model_generation: r.u64()?, evaluator_generation: r.u64()?,
        held_out_manifest: r.take(32)?.try_into().map_err(|_| Error::Incomplete)? })
}
fn write_helpers(w: &mut Writer, helpers: &BTreeMap<String, HelperGeneration>) -> Result<(), Error> {
    if helpers.len() > MAX_VOTES { return Err(Error::Limit); }
    w.count(helpers.len())?;
    for (name, helper) in helpers {
        text(w, name)?; w.u64(helper.generation)?; text(w, &helper.cohort)?;
    }
    Ok(())
}
fn read_helpers(r: &mut Reader<'_>) -> Result<BTreeMap<String, HelperGeneration>, Error> {
    let count = r.count(MAX_VOTES)?;
    let mut helpers = BTreeMap::new();
    for _ in 0..count {
        let name = read_text(r)?;
        if helpers.last_key_value().is_some_and(|(last, _)| last >= &name) { return Err(Error::InvalidInput); }
        helpers.insert(name, HelperGeneration { generation: r.u64()?, cohort: read_text(r)? });
    }
    Ok(helpers)
}
fn write_strata(w: &mut Writer, strata: &BTreeSet<String>) -> Result<(), Error> {
    if strata.len() > MAX_STRATA { return Err(Error::Limit); }
    w.count(strata.len())?;
    for stratum in strata { text(w, stratum)?; }
    Ok(())
}
fn read_strata(r: &mut Reader<'_>) -> Result<BTreeSet<String>, Error> {
    let count = r.count(MAX_STRATA)?;
    let mut strata = BTreeSet::new();
    for _ in 0..count {
        let stratum = read_text(r)?;
        if strata.last().is_some_and(|last| last >= &stratum) { return Err(Error::InvalidInput); }
        strata.insert(stratum);
    }
    Ok(strata)
}
fn write_binding(w: &mut Writer, binding: &CredibilityBinding) -> Result<(), Error> {
    write_scope(w, &binding.scope)?; text(w, &binding.label_owner)?;
    write_helpers(w, &binding.helpers)?; write_strata(w, &binding.strata)?;
    w.u64(binding.reducer_generation)
}
fn read_binding(r: &mut Reader<'_>) -> Result<CredibilityBinding, Error> {
    Ok(CredibilityBinding { scope: read_scope(r)?, label_owner: read_text(r)?,
        helpers: read_helpers(r)?, strata: read_strata(r)?, reducer_generation: r.u64()? })
}
fn observation_bound(cases: usize, helpers: usize) -> Result<(), Error> {
    let count = cases.checked_mul(helpers).ok_or(Error::Limit)?;
    if cases == 0 || helpers == 0 { return Err(Error::InvalidInput); }
    if cases > MAX_CASES || helpers > MAX_VOTES
        || u64::try_from(count).map_err(|_| Error::Limit)? > MAX_CREDIBILITY_OBSERVATIONS
    { return Err(Error::Limit); }
    Ok(())
}

fn write_snapshot(w: &mut Writer, snapshot: &CredibilitySnapshot) -> Result<(), Error> {
    observation_bound(snapshot.case_specs().len(), snapshot.helpers().len())?;
    write_scope(w, snapshot.scope())?; text(w, snapshot.label_owner())?;
    write_helpers(w, snapshot.helpers())?; write_strata(w, snapshot.strata())?;
    w.count(snapshot.case_specs().len())?;
    for case in snapshot.case_specs() {
        w.u64(case.id)?; text(w, &case.stratum)?; w.raw(&case.evidence_root)?; w.u64(case.dispatch_sequence)?;
    }
    for case in snapshot.case_specs() {
        match snapshot.case_observations(case.id) {
            None => w.u8(0)?,
            Some(observations) => {
                if !observations.keys().eq(snapshot.helpers().keys()) { return Err(Error::Binding); }
                w.u8(1)?;
                // Identity/order comes from the complete frozen roster above.
                // No repeated per-case helper names or sparse missing-row default.
                for observation in observations.values() {
                    match observation {
                        Observation::Clear => w.u8(0)?,
                        Observation::Hold { first_sequence } => { w.u8(1)?; w.u64(*first_sequence)?; }
                        Observation::Abstain => w.u8(2)?,
                        Observation::Missing => w.u8(3)?,
                    }
                }
            }
        }
        match snapshot.case_label(case.id) {
            None => w.u8(0)?,
            Some(label) => {
                w.u8(1)?; text(w, &label.owner)?; w.u64(label.evaluator_generation)?;
                w.u8(match label.source { LabelSource::IndependentEvaluation => 0,
                    LabelSource::CommitteeConsensus => 1, LabelSource::HelperSelfReport => 2 })?;
                w.raw(&label.evidence_root)?; w.u64(label.recorded_sequence)?;
                w.u8(match label.verdict { LabelVerdict::Safe => 0, LabelVerdict::Violation => 1,
                    LabelVerdict::Censored => 2 })?;
            }
        }
    }
    w.u64(snapshot.sealed_sequence())
}

fn read_snapshot(r: &mut Reader<'_>) -> Result<CredibilitySnapshot, Error> {
    let scope = read_scope(r)?;
    let label_owner = read_text(r)?;
    let helpers = read_helpers(r)?;
    let strata = read_strata(r)?;
    let count = r.count(MAX_CASES)?;
    // Bound all logical observations BEFORE allocating records or cloning names.
    observation_bound(count, helpers.len())?;
    let mut cases: Vec<CaseSpec> = Vec::new();
    cases.try_reserve_exact(count).map_err(|_| Error::Limit)?;
    for _ in 0..count {
        let id = r.u64()?;
        if cases.last().is_some_and(|last| last.id >= id) { return Err(Error::InvalidInput); }
        cases.push(CaseSpec { id, stratum: read_text(r)?,
            evidence_root: r.take(32)?.try_into().map_err(|_| Error::Incomplete)?, dispatch_sequence: r.u64()? });
    }
    let ids: Vec<_> = cases.iter().map(|case| case.id).collect();
    let names: Vec<_> = helpers.keys().cloned().collect();
    let mut ledger = CredibilityLedger::new(Campaign { scope, label_owner, helpers, strata, cases })?;
    for id in ids {
        match r.u8()? {
            0 => {}
            1 => {
                let mut observations = BTreeMap::new();
                for name in &names {
                    let observation = match r.u8()? {
                        0 => Observation::Clear,
                        1 => Observation::Hold { first_sequence: r.u64()? },
                        2 => Observation::Abstain,
                        3 => Observation::Missing,
                        _ => return Err(Error::InvalidInput),
                    };
                    observations.insert(name.clone(), observation);
                }
                ledger.record_observations(id, observations)?;
            }
            _ => return Err(Error::InvalidInput),
        }
        match r.u8()? {
            0 => {}
            1 => {
                let label = EvaluationLabel {
                    owner: read_text(r)?, evaluator_generation: r.u64()?,
                    source: match r.u8()? { 0 => LabelSource::IndependentEvaluation,
                        1 => LabelSource::CommitteeConsensus, 2 => LabelSource::HelperSelfReport,
                        _ => return Err(Error::InvalidInput) },
                    evidence_root: r.take(32)?.try_into().map_err(|_| Error::Incomplete)?,
                    recorded_sequence: r.u64()?,
                    verdict: match r.u8()? { 0 => LabelVerdict::Safe, 1 => LabelVerdict::Violation,
                        2 => LabelVerdict::Censored, _ => return Err(Error::InvalidInput) },
                };
                ledger.record_label(id, label)?;
            }
            _ => return Err(Error::InvalidInput),
        }
    }
    // Original evidence positions and original sealing sequence, NOT replay time.
    ledger.seal(r.u64()?)
}

#[cfg(test)]
mod tests;
