//! Strict, bounded operator intent; policy predicates use Config's parser.
use crate::config::{debug, policy_from_json, read_regular};
use fa_reference::{
    action::consequence::{
        gate::containment::session::policy::Policy,
        policy_campaign::{MAX_REPLAY_CASES, MAX_REPLAY_INPUT_BYTES, ReplayLimits},
    },
    strict_json::{self, Json, Limits},
};
use std::{collections::BTreeMap, path::Path};

pub(super) const CANDIDATE_BYTES: usize = 2 * 1_048_576;
pub(super) const MAX_OUTPUT_BYTES: usize = 64 * 1_048_576;

pub(crate) struct Request {
    pub expected_policy_generation: u64,
    pub candidate: Policy,
    pub limits: ReplayLimits,
    pub output_bytes: usize,
}
impl Request {
    pub(crate) fn read(path: &Path) -> Result<Self, String> {
        Self::decode(&read_regular(path, CANDIDATE_BYTES)?)
    }
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, String> {
        let value = strict_json::parse(
            bytes,
            Limits {
                max_bytes: CANDIDATE_BYTES,
                max_depth: 12,
                max_items: 16_384,
                max_string_bytes: 1_048_576,
            },
        )
        .map_err(debug)?;
        let mut fields = Fields::new(value)?;
        if fields.take("schema_version")?.as_str() != Some("fa.policy-replay-request/1") {
            return Err("unsupported policy replay request schema_version".into());
        }
        let expected_policy_generation = fields.u64("expected_policy_generation")?;
        let mut limits = Fields::new(fields.take("limits")?)?;
        let cases = limits.bounded("cases", MAX_REPLAY_CASES)?;
        let input_bytes = limits.bounded("input_bytes", MAX_REPLAY_INPUT_BYTES)?;
        let output_bytes = limits.bounded("output_bytes", MAX_OUTPUT_BYTES)?;
        limits.end()?;
        let candidate = policy_from_json(fields.take("candidate")?)?;
        fields.end()?;
        if candidate.generation() <= expected_policy_generation {
            return Err("candidate generation must advance expected_policy_generation".into());
        }
        Ok(Self {
            expected_policy_generation,
            candidate,
            limits: ReplayLimits { cases, input_bytes },
            output_bytes,
        })
    }
}

struct Fields(BTreeMap<String, Json>);
impl Fields {
    fn new(value: Json) -> Result<Self, String> {
        match value {
            Json::Object(fields) => Ok(Self(fields)),
            _ => Err("policy replay request requires an object".into()),
        }
    }
    fn take(&mut self, name: &str) -> Result<Json, String> {
        self.0
            .remove(name)
            .ok_or_else(|| format!("missing policy replay field {name}"))
    }
    fn u64(&mut self, name: &str) -> Result<u64, String> {
        self.take(name)?
            .as_u64()
            .ok_or_else(|| format!("policy replay {name} must be an unsigned integer"))
    }
    fn bounded(&mut self, name: &str, maximum: usize) -> Result<usize, String> {
        let value = usize::try_from(self.u64(name)?).map_err(debug)?;
        if value == 0 || value > maximum {
            return Err(format!("policy replay {name} must be in 1..={maximum}"));
        }
        Ok(value)
    }
    fn end(self) -> Result<(), String> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "unknown policy replay fields: {:?}",
                self.0.keys().collect::<Vec<_>>()
            ))
        }
    }
}
