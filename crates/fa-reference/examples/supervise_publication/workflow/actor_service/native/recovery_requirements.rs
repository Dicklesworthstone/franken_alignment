//! Independently supplied current policy, complete native guard inventory and
//! rollback floors. Original config/recipes pin source, stream, decoder and
//! tokenizer; this document cannot change bootstrap policy or mint live roles.
use super::{Config, Inputs, operator};
use crate::config::{debug, policy_from_json, read_regular};
use fa_reference::action::consequence::{
    delivery::persistent::observed::guarded::{
        FileCampaignRequirement, FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
    },
    gate::containment::session::policy::Policy,
    oversight::policy_governance::MAX_POLICY_CAMPAIGNS,
    policy_campaign::{MAX_REPLAY_CASES, MAX_REPLAY_INPUT_BYTES, ReplayLimits},
};
use fa_reference::strict_json::{self, Json, Limits};
use std::{collections::BTreeMap, path::Path};

const MAX_BYTES: usize = 64 * 1024;

/// This closed native service supports manual decoder reset and no identity or
/// credential guard. Those absences must be selected explicitly; an additional
/// guard on disk is rejected by original guarded recovery, never discarded.
pub(super) struct Selection {
    policy: Policy,
    campaigns: FileCampaignRequirement,
    minimum: FileRecoveryFloor,
}

impl Selection {
    pub(super) fn read(path: &Path, config: &Config) -> Result<Self, String> {
        let selected = operator::resolve(path)?;
        if selected.starts_with(operator::resolve(&config.store)?)
            || selected == operator::resolve(config.source.path())?
        {
            return Err("native recovery requirements need independent custody outside the authority store and evidence source".into());
        }
        Self::decode(&read_regular(path, MAX_BYTES)?)
    }

    fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut fields = Fields::new(
            strict_json::parse(
                bytes,
                Limits {
                    max_bytes: MAX_BYTES,
                    max_depth: 12,
                    max_items: 16_384,
                    max_string_bytes: MAX_BYTES,
                },
            )
            .map_err(debug)?,
        )?;
        if fields.text("schema")? != "fa.native-recovery-requirements/1" {
            return Err("unsupported native recovery requirements schema".into());
        }
        let policy = policy_from_json(fields.take("effective_policy")?)?;
        let mut guards = Fields::new(fields.take("guards")?)?;
        for name in ["decoder_stop", "identity", "credential"] {
            if guards.take(name)? != Json::Null {
                return Err(format!(
                    "native recovery requires explicit absence of {name}"
                ));
            }
        }
        let mut campaigns = Fields::new(guards.take("campaigns")?)?;
        let campaigns_value = FileCampaignRequirement {
            limits: ReplayLimits {
                cases: campaigns.size("cases")?,
                input_bytes: campaigns.size("input_bytes")?,
            },
            max_campaigns: campaigns.size("max_campaigns")?,
        };
        campaigns.end()?;
        guards.end()?;
        if campaigns_value.limits.cases == 0
            || campaigns_value.limits.cases > MAX_REPLAY_CASES
            || campaigns_value.limits.input_bytes == 0
            || campaigns_value.limits.input_bytes > MAX_REPLAY_INPUT_BYTES
            || campaigns_value.max_campaigns == 0
            || campaigns_value.max_campaigns > MAX_POLICY_CAMPAIGNS
        {
            return Err("native recovery campaign limits exceed the original finite bounds".into());
        }
        let mut minimum = Fields::new(fields.take("minimum")?)?;
        let minimum_value = FileRecoveryFloor {
            journal_revision: minimum.number("journal_revision")?,
            control_sequence: minimum.number("control_sequence")?,
            authority_epoch: minimum.number("authority_epoch")?,
        };
        minimum.end()?;
        fields.end()?;
        Ok(Self {
            policy,
            campaigns: campaigns_value,
            minimum: minimum_value,
        })
    }

    pub(super) fn requirements(
        &self,
        config: &Config,
        inputs: &Inputs,
    ) -> FileRecoveryRequirements {
        FileRecoveryRequirements {
            guards: FileGuardSet {
                stream: Some(inputs.stream),
                decoder: Some(inputs.decoder.clone()),
                decoder_stop: None,
                source: Some(config.source_policy),
                identity: None,
                campaigns: Some(self.campaigns),
                credential: None,
            },
            effective_policy: self.policy.clone(),
            credential_epoch: None,
            minimum: self.minimum,
        }
    }
}

struct Fields(BTreeMap<String, Json>);
impl Fields {
    fn new(value: Json) -> Result<Self, String> {
        match value {
            Json::Object(fields) => Ok(Self(fields)),
            _ => Err("native recovery requirements need an object".into()),
        }
    }

    fn take(&mut self, name: &str) -> Result<Json, String> {
        self.0
            .remove(name)
            .ok_or_else(|| format!("missing native recovery requirement {name}"))
    }

    fn text(&mut self, name: &str) -> Result<String, String> {
        match self.take(name)? {
            Json::String(value) => Ok(value),
            _ => Err(format!("native recovery {name} must be text")),
        }
    }

    fn number(&mut self, name: &str) -> Result<u64, String> {
        self.take(name)?
            .as_u64()
            .ok_or_else(|| format!("native recovery {name} must be an unsigned integer"))
    }

    fn size(&mut self, name: &str) -> Result<usize, String> {
        usize::try_from(self.number(name)?).map_err(debug)
    }

    fn end(self) -> Result<(), String> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err("unknown native recovery requirement".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOCUMENT: &str = r#"{"schema":"fa.native-recovery-requirements/1","effective_policy":{"generation":2,"nodes":[{"op":"payload_at_most","maximum":128}]},"guards":{"campaigns":{"cases":16,"input_bytes":1048576,"max_campaigns":4},"decoder_stop":null,"identity":null,"credential":null},"minimum":{"journal_revision":70,"control_sequence":9,"authority_epoch":1}}"#;

    #[test]
    fn complete_native_requirements_keep_current_policy_and_explicit_floors() {
        let selected = Selection::decode(DOCUMENT.as_bytes()).unwrap();
        assert_eq!(
            selected.minimum,
            FileRecoveryFloor {
                journal_revision: 70,
                control_sequence: 9,
                authority_epoch: 1,
            }
        );
        assert_eq!(selected.campaigns.max_campaigns, 4);
        assert_eq!(selected.campaigns.limits.cases, 16);
        assert_eq!(
            selected.policy,
            policy_from_json(
                strict_json::parse(
                    br#"{"generation":2,"nodes":[{"op":"payload_at_most","maximum":128}]}"#,
                    Limits {
                        max_bytes: 1024,
                        max_depth: 4,
                        max_items: 32,
                        max_string_bytes: 128
                    },
                )
                .unwrap()
            )
            .unwrap()
        );
    }

    #[test]
    fn omitted_added_weakened_and_ambiguous_requirements_refuse() {
        for changed in [
            DOCUMENT.replace("\"identity\":null,", ""),
            DOCUMENT.replace("\"identity\":null", "\"identity\":{}"),
            DOCUMENT.replace("\"credential\":null", "\"credential\":{}"),
            DOCUMENT.replace("\"decoder_stop\":null", "\"decoder_stop\":{}"),
            DOCUMENT.replace("\"authority_epoch\":1", "\"authority_epoch\":1,\"extra\":0"),
            DOCUMENT.replace(",\"authority_epoch\":1", ""),
            DOCUMENT.replace("\"cases\":16", "\"cases\":0"),
            DOCUMENT.replace("\"input_bytes\":1048576", "\"input_bytes\":0"),
            DOCUMENT.replace("\"max_campaigns\":4", "\"max_campaigns\":0"),
            DOCUMENT.replace("\"schema\":", "\"unknown\":0,\"schema\":"),
            DOCUMENT.replace(
                "\"journal_revision\":70",
                "\"journal_revision\":70,\"journal_revision\":0",
            ),
            DOCUMENT.replace("\"control_sequence\":9", "\"control_sequence\":-1"),
        ] {
            assert!(Selection::decode(changed.as_bytes()).is_err(), "{changed}");
        }
    }
}
