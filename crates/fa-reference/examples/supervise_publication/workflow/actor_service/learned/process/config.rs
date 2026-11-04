//! Closed, independently selected per-round child configurations.
use super::super::fields::{Fields, path};
use crate::config::{Config, debug};
use fa_reference::action::consequence::delivery::persistent::observed::helpers::learned_sockets::processes::sequence::{
    LearnedProcessLimits, MAX_LEARNED_PROCESS_STARTS,
};
use fa_reference::action::consequence::oversight::{
    helper_processes::{HelperProgram, MAX_PROGRAM_ARGUMENTS, MAX_PROGRAM_ENVIRONMENT},
    helper_workers::HelperLimits,
    learned_host::sidecar::workers::MAX_LEARNED_REVIEW_POLLS,
    sidecar::MAX_SIDECAR_REFINEMENT_ROUNDS,
};
use std::{collections::BTreeMap, ffi::OsString, path::{Component, PathBuf}};

#[derive(Clone)]
pub(in super::super) struct ProcessRecipe {
    rounds: Vec<u64>,
    pub programs: BTreeMap<u64, BTreeMap<String, HelperProgram>>,
    pub helpers: HelperLimits,
    pub polls: usize,
    pub limits: LearnedProcessLimits,
}

impl ProcessRecipe {
    pub(in super::super) fn decode(mut fields: Fields) -> Result<Self, String> {
        let mut programs = BTreeMap::new();
        let mut rounds = Vec::new();
        let mut members_count = None;
        for round in fields.array("rounds", MAX_SIDECAR_REFINEMENT_ROUNDS)? {
            let mut round = Fields::new(round)?;
            let id = round.number("id")?;
            let members = round.object("members")?.entries();
            round.end()?;
            if id == 0 || programs.contains_key(&id) {
                return Err("process round IDs must be nonzero and unique".into());
            }
            if members.is_empty() || members.len() > HelperLimits::default().members
                || members_count.is_some_and(|count| count != members.len()) {
                return Err("every process round requires the same bounded nonempty congress".into());
            }
            members_count = Some(members.len());
            let members = members.into_iter().map(|(name, value)| {
                Ok((name, program(Fields::new(value)?)?))
            }).collect::<Result<BTreeMap<_, _>, String>>()?;
            programs.insert(id, members);
            rounds.push(id);
        }
        let helpers = HelperLimits { members: members_count.unwrap_or(0),
            input_bytes: fields.size("input_bytes")?, salt_bytes: fields.size("salt_bytes")? };
        let polls = fields.size("polls")?;
        let limits = LearnedProcessLimits { starts: fields.size("starts")? };
        fields.end()?;
        let caps = HelperLimits::default();
        let starts = helpers.members.checked_mul(rounds.len()).ok_or("process start reservation overflow")?;
        if rounds.is_empty() || polls == 0 || polls > MAX_LEARNED_REVIEW_POLLS
            || helpers.input_bytes == 0 || helpers.input_bytes > caps.input_bytes
            || helpers.salt_bytes == 0 || helpers.salt_bytes > caps.salt_bytes
            || limits.starts == 0 || limits.starts > MAX_LEARNED_PROCESS_STARTS
            || starts > limits.starts {
            return Err("process review requires finite input, salt, poll and complete-schedule start bounds".into());
        }
        Ok(Self { rounds, programs, helpers, polls, limits })
    }

    pub(in super::super) fn rounds(&self) -> Vec<u64> { self.rounds.clone() }

    /// Pure roster comparison, also safe for receipt-only loading. Program and
    /// directory I/O belongs to the original live launch preflight.
    pub(in super::super) fn check_roster(&self, config: &Config) -> Result<(), String> {
        for members in self.programs.values() {
            if !members.keys().eq(config.profile.committee.members().keys()) {
                return Err("process roster must cover the exact independently configured congress in every round".into());
            }
        }
        Ok(())
    }
}

fn program(mut fields: Fields) -> Result<HelperProgram, String> {
    let executable = path(&fields.text("executable")?)?;
    let directory = PathBuf::from(fields.text("directory")?);
    if !directory.is_absolute()
        || directory.components().any(|part| !matches!(part, Component::RootDir | Component::Normal(_))) {
        return Err("process directory must be absolute and normalized".into());
    }
    let arguments = fields.array("arguments", MAX_PROGRAM_ARGUMENTS)?.into_iter()
        .map(|value| value.as_str().map(OsString::from).ok_or_else(|| "process arguments must be text".to_owned()))
        .collect::<Result<Vec<_>, _>>()?;
    let environment = fields.object("environment")?.entries();
    if environment.len() > MAX_PROGRAM_ENVIRONMENT { return Err("process environment exceeds its bound".into()); }
    let environment = environment.into_iter().map(|(name, value)| {
        Ok((OsString::from(name), OsString::from(value.as_str().ok_or("process environment values must be text")?)))
    }).collect::<Result<BTreeMap<_, _>, String>>()?;
    fields.end()?;
    HelperProgram::new(executable, directory, arguments, environment).map_err(debug)
}
