//! Operator-supplied offline evidence for the ORIGINAL continued-submission path.
//! The capsule is data, not evaluator authentication or permission to dispatch.
use crate::config::{Config, debug, read_regular};
use fa_reference::action::consequence::delivery::persistent::credibility::CredibilityActivation;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::oversight::actor::ActorProposal;
use std::path::{Path, PathBuf};

const FLAG: &str = "--credibility-activation";

/// Remove only this explicit option. Existing command/peer parsing validates
/// everything left over. Role-specific, create and query-only modes cannot gain
/// a privileged mutation merely by receiving an otherwise unused option.
pub fn take_option(args: &mut Vec<String>) -> Result<Option<PathBuf>, String> {
    let mut positions = args.iter().enumerate().filter_map(|(index, arg)| (arg == FLAG).then_some(index));
    let Some(index) = positions.next() else { return Ok(None); };
    if positions.next().is_some() { return Err("duplicate credibility activation option".into()); }
    let positional = match args.first().map(String::as_str) {
        Some("submit") => 3,
        Some("submit-checked" | "serve-open") => 4,
        Some("proposal-next" | "serve-open-checked") => 5,
        _ => return Err("credibility activation requires submit, submit-checked, proposal-next, serve-open or serve-open-checked".into()),
    };
    if index < positional || index + 1 >= args.len() || args[index + 1].is_empty()
        || args[index + 1].starts_with("--")
    {
        return Err("credibility activation requires an evidence file after the positional arguments".into());
    }
    let path = PathBuf::from(args.remove(index + 1));
    args.remove(index);
    Ok(Some(path))
}

pub fn read(path: &Path) -> Result<CredibilityActivation, String> {
    let bytes = read_regular(path, CredibilityActivation::MAX_ENCODED_BYTES)?;
    CredibilityActivation::decode_reference(&bytes).map_err(debug)
}

pub fn activate(host: &mut FileOversight, config: &Config, proposal: &ActorProposal, path: &Path)
    -> Result<(), String>
{
    let request = read(path)?;
    let before = host.inspect();
    let epoch = before.control.ledger.epoch.checked_add(1).ok_or("authority epoch exhausted")?;
    // Refuse an obviously stale ORIGINAL actor document before activating. The
    // ordinary gateway still checks every field; no frozen bytes are rewritten.
    if proposal.expected_policy_epoch != epoch || proposal.target != before.target {
        return Err("original proposal must name the post-activation epoch and current target".into());
    }
    host.activate_credibility(host.revision(), request, &config.profile.committee).map_err(debug)?;
    // A historical operation returns historical evidence, not permission. In
    // particular it cannot clear the original recovery invalidation latch.
    host.check_credibility().map_err(debug)?;
    if host.inspect().control.ledger.epoch != epoch {
        return Err("historical activation cannot admit a new request".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<String> { values.iter().map(|s| (*s).to_owned()).collect() }

    #[test]
    fn explicit_option_composes_with_both_peer_orders_and_checked_positionals() {
        for values in [
            vec!["submit", "config", "proposal", FLAG, "capsule", "--reviewer-profile", "peer"],
            vec!["submit", "config", "proposal", "--reviewer-profile", "peer", FLAG, "capsule"],
            vec!["submit-checked", "config", "proposal", "witness", FLAG, "capsule"],
            vec!["proposal-next", "config", "2", "payload", "1000", FLAG, "capsule"],
        ] {
            let mut values = args(&values);
            let before = values.clone();
            assert_eq!(take_option(&mut values).unwrap(), Some(PathBuf::from("capsule")));
            assert_eq!(values, before.into_iter().filter(|v| v != FLAG && v != "capsule").collect::<Vec<_>>());
        }
    }

    #[test]
    fn unsupported_missing_and_duplicate_options_refuse_without_argument_mutation() {
        for values in [
            vec!["resume", "config", "proposal", FLAG, "capsule"],
            vec!["create", "config", "proposal", FLAG, "capsule"],
            vec!["actor-submit", "profile", "proposal", FLAG, "capsule"],
            vec!["review-peer", "profile", "1", FLAG, "capsule"],
            vec!["submit", "config", "proposal", FLAG],
            vec!["submit", "config", "proposal", FLAG, "--reviewer-profile"],
            vec!["submit", "config", "proposal", FLAG, "capsule", FLAG, "capsule"],
            vec!["submit", FLAG, "capsule"],
        ] {
            let mut values = args(&values);
            let before = values.clone();
            assert!(take_option(&mut values).is_err());
            assert_eq!(values, before);
        }
    }

    #[test]
    fn absence_preserves_existing_arguments_including_unknown_options() {
        let mut values = args(&["submit", "config", "proposal", "--unknown", "value"]);
        let before = values.clone();
        assert_eq!(take_option(&mut values).unwrap(), None);
        assert_eq!(values, before);
    }
}
