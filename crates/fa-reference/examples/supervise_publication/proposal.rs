//! Construct actor documents without opening the publication journal or acquiring
//! authority. A retained-state proposal anticipates the owner's NEXT recovery
//! fence; it is not a reservation and concurrent changes can make it stale.
use crate::config::{Config, debug};
use fa_reference::action::{ElapsedTick, MAX_PAYLOAD_BYTES};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::credibility::CredibilityActivation;
use fa_reference::action::consequence::oversight::actor::ActorProposal;
use fa_reference::action::consequence::oversight::actor_wire::{Command, encode_command};

#[cfg(test)]
#[path = "proposal_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Basis {
    Bootstrap,
    Existing,
}

pub fn document(
    config: &Config,
    request: u64,
    payload: Vec<u8>,
    ttl: u64,
    now: ElapsedTick,
    basis: Basis,
) -> Result<Vec<u8>, String> {
    document_for(config, request, payload, ttl, now, basis, None)
}

/// Plan the original actor document for an explicit qualification in the next
/// recovered owner. This reads one historical cut and writes nothing. Matching
/// predecessors do not prove that the evidence will qualify or reserve an epoch.
pub fn document_qualified(
    config: &Config,
    request: u64,
    payload: Vec<u8>,
    ttl: u64,
    now: ElapsedTick,
    qualification: &CredibilityActivation,
) -> Result<Vec<u8>, String> {
    document_for(config, request, payload, ttl, now, Basis::Existing, Some(qualification))
}

fn document_for(
    config: &Config,
    request: u64,
    payload: Vec<u8>,
    ttl: u64,
    now: ElapsedTick,
    basis: Basis,
    qualification: Option<&CredibilityActivation>,
) -> Result<Vec<u8>, String> {
    if request == 0 {
        return Err("request must be nonzero".into());
    }
    if ttl == 0 || ttl > config.timing.runtime_ms {
        return Err("TTL must fit the configured one-shot runtime".into());
    }
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err("payload exceeds the actor limit".into());
    }
    let deadline = ElapsedTick(now.0.checked_add(ttl).ok_or("deadline overflow")?);
    let (target, expected_policy_epoch) = match basis {
        Basis::Bootstrap => (config.profile.delivery.target, 0),
        Basis::Existing => {
            // Read-only historical inspection, NOT open(): opening would fence
            // outstanding work merely to print a document. The original owner
            // will verify the target and epoch again when submit opens the store.
            let state = FileOversight::read_publication(&config.store, &config.profile)
                .map_err(debug)?;
            let epoch = state.control.ledger.epoch.checked_add(1)
                .ok_or("policy epoch exhausted")?;
            let epoch = if let Some(qualification) = qualification {
                if qualification.scope != config.profile.delivery.scope
                    || qualification.expected_control_sequence != state.control.sequence
                    || qualification.expected_epoch != epoch
                {
                    return Err("qualification does not bind the next recovered predecessor".into());
                }
                // Recovery and activation are distinct original transitions.
                // Neither is executed just to print an actor proposal.
                epoch.checked_add(1).ok_or("post-activation epoch exhausted")?
            } else { epoch };
            (state.target, epoch)
        }
    };
    let proposal = ActorProposal {
        target,
        units: u64::try_from(payload.len()).map_err(debug)?.max(1),
        payload,
        deadline,
        expected_policy_epoch,
    };
    encode_command(&Command::Submit { request, proposal }).map_err(debug)
}
