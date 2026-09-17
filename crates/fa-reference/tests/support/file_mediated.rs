#![allow(dead_code)]
#[path = "file_oversight.rs"] pub mod ordinary;
#[path = "file_mediation.rs"] pub mod topology;
pub use ordinary::{Directory, profile, snapshot};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::*;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::*;
use fa_reference::action::consequence::delivery::persistent::observed::mediation::*;
use fa_reference::action::consequence::mediation::{AuthorityGraph, CutCheck, MAX_CHECK_EDGE_VISITS};
use fa_reference::action::consequence::oversight::credibility::{EvaluationProtocol, Fraction};

pub fn guards() -> FileGuardSet {
    FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None,
        identity: None, campaigns: None, credential: None }
}
pub fn graph(generation: u64, bypass: bool) -> AuthorityGraph {
    topology::graph(profile().delivery.scope, profile().delivery.target, generation, bypass)
}
pub fn protocol() -> EvaluationProtocol {
    EvaluationProtocol { domain: 1, stratum: 2, period: 3, minimum_violation_origins: 1,
        minimum_benign_origins: 1, precision_floor: Fraction { numerator: 1, denominator: 2 },
        recall_floor: Fraction { numerator: 1, denominator: 2 },
        false_positive_ceiling: Fraction { numerator: 1, denominator: 2 }, false_stop_budget: 10 }
}
pub fn requirements(host: &FileOversight, evaluation: Option<EvaluationProtocol>) -> FileMediatedRequirements {
    let state = host.inspect(); let topology = host.mediation_snapshot().unwrap();
    FileMediatedRequirements {
        oversight: FileRecoveryRequirements { guards: guards(), effective_policy: profile().delivery.policy,
            credential_epoch: None, minimum: FileRecoveryFloor { journal_revision: state.revision,
                control_sequence: state.control.sequence, authority_epoch: state.control.ledger.epoch } },
        topology: FileTopologyRequirement { initial: graph(1, false), current: topology.graph, available: topology.available },
        prediction: None, evaluation,
    }
}
pub fn certify(host: &mut FileOversight, role: &FileMediationObserver) -> CutCheck {
    let proposal = host.mediation_snapshot().unwrap().graph.propose_cut(&[2]).unwrap();
    let revision = host.revision(); let epoch = host.inspect().control.ledger.epoch;
    role.certify(host, revision, epoch, &proposal, MAX_CHECK_EDGE_VISITS).unwrap().unwrap()
}
pub fn replacement(host: &FileOversight, operation: u64, next: Option<AuthorityGraph>) -> FileMediationUpdate {
    FileMediationUpdate { operation, expected_generation: host.mediation_snapshot().unwrap().graph.spec().generation,
        expected_authority_epoch: host.inspect().control.ledger.epoch, next }
}
pub fn update(host: &mut FileOversight, role: &FileMediationObserver, operation: u64, next: Option<AuthorityGraph>) {
    let request = replacement(host, operation, next); let revision = host.revision();
    role.update(host, revision, &request).unwrap();
}
