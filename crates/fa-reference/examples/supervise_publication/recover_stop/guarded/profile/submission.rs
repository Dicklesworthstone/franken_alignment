//! Independent recovery expectations for an ordinary campaign-governed store.
//! The complete policy is a retained expectation, never a promotion or approval.
use super::{Config, Custody, FileRecoveryFloor, Profile, absolute, end, number, object, take};
use crate::config::{debug, policy_from_json, read_regular};
use fa_reference::action::consequence::{
    delivery::persistent::observed::guarded::{
        FileCampaignRequirement, FileGuardSet, FileRecoveryRequirements,
    },
    gate::containment::session::policy::Policy,
    oversight::policy_governance::MAX_POLICY_CAMPAIGNS,
    policy_campaign::{MAX_REPLAY_CASES, MAX_REPLAY_INPUT_BYTES, ReplayLimits},
};
use fa_reference::strict_json::{self, Json, Limits};
use std::path::{Path, PathBuf};

const PROFILE_BYTES: usize = 2 * 1_048_576;

struct Expectation {
    policy: Policy,
    campaigns: FileCampaignRequirement,
}

/// Explicit new-work recovery intent, never inferred from a stop/receipt profile.
/// Version one keeps its exact legacy grammar and size limits. Version two pins
/// the whole effective policy and the exact original campaign gate inventory.
pub(crate) struct SubmitProfile {
    original: Profile,
    expected: Option<Expectation>,
}

impl SubmitProfile {
    pub(crate) fn read(path: &Path) -> Result<Self, String> {
        Self::decode(&read_regular(path, PROFILE_BYTES)?)
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, String> {
        let legacy_error = match Profile::decode_schema(
            bytes,
            "fa.guarded-submit/1",
            "unsupported guarded submit profile",
        ) {
            Ok(original) => {
                return Ok(Self {
                    original,
                    expected: None,
                });
            }
            Err(error) => error,
        };
        let json = strict_json::parse(
            bytes,
            Limits {
                max_bytes: PROFILE_BYTES,
                max_depth: 12,
                max_items: 16_384,
                max_string_bytes: 1_048_576,
            },
        )
        .map_err(debug)?;
        if json.get("schema_version").and_then(Json::as_str) != Some("fa.guarded-submit/2") {
            return Err(legacy_error);
        }
        let mut root = object(json)?;
        take(&mut root, "schema_version")?;
        let mut floor = object(take(&mut root, "minimum")?)?;
        let minimum = FileRecoveryFloor {
            journal_revision: number(&mut floor, "journal_revision")?,
            control_sequence: number(&mut floor, "control_sequence")?,
            authority_epoch: number(&mut floor, "authority_epoch")?,
        };
        end(&floor)?;
        let input = custody_path(take(&mut root, "history_anchor")?)?;
        let output = root
            .remove("retain_history_anchor")
            .map(custody_path)
            .transpose()?;
        let policy = policy_from_json(take(&mut root, "effective_policy")?)?;
        let campaigns = campaign_requirement(take(&mut root, "campaigns")?)?;
        end(&root)?;
        Ok(Self {
            original: Profile {
                minimum,
                input,
                output,
                topology: None,
            },
            expected: Some(Expectation { policy, campaigns }),
        })
    }

    pub(crate) fn minimum(&self) -> FileRecoveryFloor {
        self.original.minimum()
    }

    pub(crate) fn admit(&self, config: &Config) -> Result<Custody, String> {
        self.original.admit(config)
    }

    pub(crate) fn campaign_requirement(&self) -> Option<FileCampaignRequirement> {
        self.expected.as_ref().map(|expected| expected.campaigns)
    }

    /// Consumers with specialised recovery contracts must not silently discard
    /// version two's independently retained policy or campaign expectations.
    pub(crate) fn require_legacy(&self) -> Result<(), String> {
        if self.expected.is_some() {
            Err("this recovery mode does not support fa.guarded-submit/2 policy campaigns".into())
        } else {
            Ok(())
        }
    }

    pub(crate) fn requirements(&self, config: &Config) -> FileRecoveryRequirements {
        FileRecoveryRequirements {
            guards: FileGuardSet {
                stream: None,
                decoder: None,
                decoder_stop: None,
                source: Some(config.source_policy),
                identity: None,
                campaigns: self.campaign_requirement(),
                credential: None,
            },
            effective_policy: self.expected.as_ref().map_or_else(
                || config.profile.delivery.policy.clone(),
                |expected| expected.policy.clone(),
            ),
            credential_epoch: None,
            minimum: self.minimum(),
        }
    }
}

fn custody_path(value: Json) -> Result<PathBuf, String> {
    if value.as_str().is_some_and(|path| path.len() > 4096) {
        return Err("anchor path exceeds the original custody bound".into());
    }
    absolute(value)
}

fn campaign_requirement(value: Json) -> Result<FileCampaignRequirement, String> {
    let mut fields = object(value)?;
    let cases = usize::try_from(number(&mut fields, "cases")?).map_err(debug)?;
    let input_bytes = usize::try_from(number(&mut fields, "input_bytes")?).map_err(debug)?;
    let max_campaigns = usize::try_from(number(&mut fields, "max_campaigns")?).map_err(debug)?;
    end(&fields)?;
    if cases == 0 || input_bytes == 0 || max_campaigns == 0 {
        return Err("guarded recovery campaign limits must be nonzero".into());
    }
    if cases > MAX_REPLAY_CASES
        || input_bytes > MAX_REPLAY_INPUT_BYTES
        || max_campaigns > MAX_POLICY_CAMPAIGNS
    {
        return Err("guarded recovery campaign limits exceed the native bounds".into());
    }
    Ok(FileCampaignRequirement {
        limits: ReplayLimits { cases, input_bytes },
        max_campaigns,
    })
}

#[cfg(test)]
#[path = "submission/tests.rs"]
mod tests;
