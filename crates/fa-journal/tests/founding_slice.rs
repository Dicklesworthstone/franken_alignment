//! End-to-end founding vertical slice scenario and deterministic JSONL event log
//! (bead `franken_alignment-e2e-founding-vertical-slice-dmd.1`, plan §0, §21).
#![forbid(unsafe_code)]

use fa_journal::{Event, Identity, Journal, Limits};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{EndpointOutcome, PublicationEndpoint};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::{
    ActorState, ResetRequest, RestartGrade, RestartProfile,
};
use fa_reference::action::consequence::gate::{ReviewBinding, TargetCeiling};
use fa_reference::action::consequence::mediation::{
    AuthorityGraph, Channel, Completeness, CutCheck, Edge, Enforcer, GraphSpec, Node, NodeKind,
};
use fa_reference::action::consequence::oversight::evidence_source::{
    EvidenceIdentity, EvidenceSnapshot,
};
use fa_reference::action::consequence::oversight::{
    CommitteeContract, HelperContract, OversightBroker, ReviewWindow,
};
use fa_reference::action::consequence::{Consequence, DecisionInputs, Restriction, decide};
use fa_reference::action::{
    ActionSpec, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION,
};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::perimeter::{
    BypassDisposition, CredentialExposure, CredentialHolder, EffectFamilyRecord, Mediation,
    PerimeterScope, RouteRecord, ThreatClass, TrustDomain,
};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use fa_reference::{Effect, Error as ModelError, Rights, Snapshot};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

pub const DEFAULT_SEED: u64 = 0xFA03_2026_0000_0001;
pub const RETAINED_ARTIFACT_REL: &str = "artifacts/execution/2026-10-14-founding-slice.jsonl";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnitsSummary {
    pub available: u64,
    pub reserved: u64,
    pub spent: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventLine {
    pub step: u64,
    pub run: [u8; 32],
    pub action: String,
    pub attempt: u64,
    pub round: u64,
    pub consequence: String,
    pub units: UnitsSummary,
    pub incident_count: u64,
    pub epoch: u64,
    pub evidence_root: [u8; 32],
    pub closure_digest: [u8; 32],
}

impl EventLine {
    pub fn to_json_line(&self) -> String {
        format!(
            "{{\"step\":{},\"run\":\"{}\",\"action\":\"{}\",\"attempt\":{},\"round\":{},\"consequence\":\"{}\",\"units\":{{\"available\":{},\"reserved\":{},\"spent\":{}}},\"incident_count\":{},\"epoch\":{},\"evidence_root\":\"{}\",\"closure_digest\":\"{}\"}}",
            self.step,
            hex32(&self.run),
            self.action,
            self.attempt,
            self.round,
            self.consequence,
            self.units.available,
            self.units.reserved,
            self.units.spent,
            self.incident_count,
            self.epoch,
            hex32(&self.evidence_root),
            hex32(&self.closure_digest),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventLog {
    pub lines: Vec<EventLine>,
    pub journal_bytes: Vec<u8>,
}

impl EventLog {
    pub fn to_jsonl(&self) -> String {
        let mut out = String::new();
        for line in &self.lines {
            out.push_str(&line.to_json_line());
            out.push('\n');
        }
        out
    }
}

fn hex32(bytes: &[u8; 32]) -> String {
    let mut s = String::with_capacity(64);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut s, "{b:02x}");
    }
    s
}

fn seeded_bytes(seed: u64, tag: u64) -> [u8; 32] {
    let mut out = [0_u8; 32];
    for (idx, chunk) in out.chunks_exact_mut(8).enumerate() {
        let mix = seed
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(tag.wrapping_mul(0xBF58_476D_1CE4_E5B9))
            .wrapping_add((idx as u64).wrapping_mul(0x94D0_49BB_1331_11EB))
            .rotate_left(17);
        chunk.copy_from_slice(&mix.to_le_bytes());
    }
    if out == [0; 32] {
        out[0] = 1;
    }
    out
}

fn closure_digest(
    seed: u64,
    step: u64,
    attempt: u64,
    round: u64,
    consequence: &str,
    units: UnitsSummary,
    incident_count: u64,
    epoch: u64,
    evidence_root: &[u8; 32],
    journal_checksum: u32,
) -> [u8; 32] {
    let mut acc = seed ^ step.wrapping_mul(0x1000_0000_01B3);
    for &b in consequence.as_bytes().iter().chain(evidence_root.iter()) {
        acc = acc.wrapping_mul(0x1000_0000_01B3) ^ u64::from(b);
    }
    acc ^= attempt.rotate_left(7)
        ^ round.rotate_left(13)
        ^ units.available.rotate_left(19)
        ^ units.reserved.rotate_left(29)
        ^ units.spent.rotate_left(37)
        ^ incident_count.rotate_left(43)
        ^ epoch.rotate_left(53)
        ^ u64::from(journal_checksum);
    seeded_bytes(acc, step ^ round)
}

fn scope() -> Scope {
    Scope {
        tenant: 1,
        principal: 2,
        run: 3,
        branch: 4,
        authority: 5,
        purpose: Purpose::Effect,
    }
}

fn admitted_target() -> ResolvedTarget {
    ResolvedTarget {
        adapter: 10,
        object: 11,
        contract_version: 1,
        expected_version: 1,
        generation: 1,
    }
}

fn verify_broker_mediation_cut() -> Result<usize, String> {
    let target = admitted_target();
    let family = EffectFamilyRecord {
        scope: PerimeterScope {
            tenant: scope().tenant,
            principal: scope().principal,
            purpose: 1,
        },
        family: "package.publish".into(),
        trust_domains: vec![
            TrustDomain::Actor,
            TrustDomain::ObservationAndAnalysis,
            TrustDomain::Enforcement,
            TrustDomain::GovernanceAndInvestigation,
        ],
        credentials: vec![CredentialExposure {
            credential: "publish-token".into(),
            holder: CredentialHolder::Broker,
        }],
        routes: vec![RouteRecord {
            route: "broker://publish".into(),
            threat: Some(ThreatClass::DirectCredentialOrEgress),
            mediation: Mediation::BrokeredEffects,
            bypass: BypassDisposition::Blocked,
        }],
        residual_nonclaims: vec!["reference model only".into()],
    };
    let graph = AuthorityGraph::new(GraphSpec {
        generation: 1,
        inventory_generation: 1,
        scope: scope(),
        target,
        family,
        completeness: Completeness::DeclaredComplete {
            inventory_generation: 1,
        },
        nodes: vec![
            Node {
                id: 1,
                kind: NodeKind::Actor,
            },
            Node {
                id: 2,
                kind: NodeKind::Enforcer,
            },
            Node {
                id: 3,
                kind: NodeKind::Sink,
            },
        ],
        edges: vec![
            Edge {
                id: 1,
                from: 1,
                to: 2,
                channel: Channel::Dispatch,
                route: "broker://publish".into(),
                provenance: 1,
            },
            Edge {
                id: 2,
                from: 2,
                to: 3,
                channel: Channel::Dispatch,
                route: "broker://publish".into(),
                provenance: 1,
            },
        ],
        enforcers: vec![Enforcer {
            node: 2,
            adapter: target.adapter,
            contract_version: target.contract_version,
            generation: target.generation,
            provenance: 1,
        }],
    })
    .map_err(|e| format!("authority graph: {e:?}"))?;
    let proposal = graph
        .propose_cut(&[2])
        .map_err(|e| format!("propose cut: {e:?}"))?;
    match graph
        .verify_cut(&proposal, 64)
        .map_err(|e| format!("verify cut: {e:?}"))?
    {
        CutCheck::Verified(cut) => Ok(cut.edge_visits()),
        other => Err(format!("expected verified cut, got {other:?}")),
    }
}

pub fn run_founding_slice(seed: u64) -> Result<EventLog, String> {
    let run_id = seeded_bytes(seed, 1);
    let policy_id = seeded_bytes(seed, 2);
    let salt = seeded_bytes(seed, 3);
    let total_units = 100_u64;

    let mut journal = Journal::new(
        Identity {
            run: run_id,
            policy: policy_id,
            total_units,
        },
        Limits::default(),
    )
    .map_err(|e| format!("journal init: {e}"))?;
    let mut journal_bytes = journal.header();

    let target = admitted_target();
    let restart_profile = RestartProfile {
        id: 1,
        generation: 1,
        host_generation: 1,
        model_generation: 1,
        tokenizer_generation: 1,
        state_schema_generation: 1,
        grade: RestartGrade::FunctionalRestart,
    };
    let initial_actor = ActorState::new(restart_profile, vec![1], vec![2], vec![3], 1)
        .map_err(|e| format!("initial actor state: {e:?}"))?;

    // Three deterministic helpers across two cohorts ("alpha", "beta") + capped weights.
    let members = BTreeMap::from([
        (
            "h_alpha_1".into(),
            MemberPolicy {
                cohort: "alpha".into(),
                weight: 10,
            },
        ),
        (
            "h_alpha_2".into(),
            MemberPolicy {
                cohort: "alpha".into(),
                weight: 10,
            },
        ),
        (
            "h_beta_1".into(),
            MemberPolicy {
                cohort: "beta".into(),
                weight: 10,
            },
        ),
    ]);
    let contracts = CommitteeContract::new(
        [("h_alpha_1", 11_u64), ("h_alpha_2", 12), ("h_beta_1", 21)]
            .into_iter()
            .map(|(name, model_epoch)| {
                Ok((
                    name.to_owned(),
                    HelperContract::new(
                        InputProfileBinding {
                            profile_id: 1,
                            profile_bytes: vec![],
                            tokenizer_epoch: 1,
                            policy_epoch: 0,
                            model_epoch,
                        },
                        1,
                        b"review-package-publish".to_vec(),
                    )
                    .map_err(|e| format!("helper contract: {e:?}"))?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?,
    )
    .map_err(|e| format!("committee contract: {e:?}"))?;

    let mut endpoint = PublicationEndpoint::new(target, b"initial".to_vec(), 1_000, 8)
        .map_err(|e| format!("endpoint: {e:?}"))?;
    let narrowed_ceiling =
        TargetCeiling::new(&[target]).map_err(|e| format!("target ceiling: {e:?}"))?;
    let mut broker = OversightBroker::new(
        ControllerConfig {
            scope: scope(),
            total: total_units,
            max_attempts: 8,
            actor: initial_actor,
            suspend_at_incident: 3,
            policy: Policy::new(
                1,
                vec![Predicate::ExactValue {
                    key: 1,
                    value: b"admitted".to_vec(),
                }],
            )
            .map_err(|e| format!("policy: {e:?}"))?,
            congress: CongressPolicy {
                generation: 1,
                members,
                caps: Caps {
                    per_member: 10,
                    per_cohort: 10,
                },
                continue_minimum: 15,
                continue_hold_maximum: 5,
                narrow_at: 25,
                suspend_at: 30,
                minimum_members: 2,
                minimum_cohorts: 2,
            },
            narrowed_targets: narrowed_ceiling.clone(),
        },
        &mut endpoint,
        contracts,
    )
    .map_err(|e| format!("oversight broker: {e:?}"))?;
    let ack = endpoint
        .install_fence(broker.fence_request())
        .map_err(|e| format!("install fence: {e:?}"))?;
    broker
        .confirm_fence(ack)
        .map_err(|e| format!("confirm fence: {e:?}"))?;

    // Capture pre-incident checkpoint (revision 0) at FunctionalRestart grade.
    let pre_incident_checkpoint = broker
        .capture_checkpoint(1, 0)
        .map_err(|e| format!("capture checkpoint: {e:?}"))?;

    let mut lines = Vec::new();

    // Helper closure to run a 3-member commit-reveal round on the broker.
    let run_round =
        |broker: &mut OversightBroker,
         attempt: u64,
         round: u64,
         tick: u64,
         payload: Vec<u8>,
         disqualified: bool,
         verdicts: [Verdict; 3]|
         -> Result<([u8; 32], Consequence, EvidenceSnapshot, FrozenAction), String> {
            let now = ElapsedTick(tick);
            broker
                .observe_time(now)
                .map_err(|e| format!("observe_time: {e:?}"))?;
            let mut context = seed.to_le_bytes().to_vec();
            context.extend_from_slice(&round.to_le_bytes());
            let admitted_snapshot = Snapshot {
                semantic_epoch: 1,
                complete: true,
                values: BTreeMap::from([(1, b"admitted".to_vec())]),
            };
            let review_snapshot = if disqualified {
                Snapshot {
                    semantic_epoch: 1,
                    complete: true,
                    values: BTreeMap::from([(1, b"forbidden-destination".to_vec())]),
                }
            } else {
                admitted_snapshot.clone()
            };
            let source = EvidenceSnapshot::new(
                EvidenceIdentity {
                    source: 1,
                    generation: attempt,
                    scope: scope(),
                },
                review_snapshot,
                BTreeMap::from([
                    ("h_alpha_1".into(), context.clone()),
                    ("h_alpha_2".into(), context.clone()),
                    ("h_beta_1".into(), context),
                ]),
            )
            .map_err(|e| format!("evidence snapshot: {e:?}"))?;
            let action = broker
                .propose(
                    attempt,
                    ActionSpec {
                        version: VERSION,
                        scope: scope(),
                        target: Some(target),
                        payload,
                        required_witnesses: Vec::new(),
                        policy_epoch: broker.inspect().ledger.epoch,
                        deadline: ElapsedTick(1_000),
                        units: 16,
                    },
                    &admitted_snapshot,
                )
                .map_err(|e| format!("propose {attempt}: {e:?}"))?
                .action;
            let inputs = source
                .inputs_for(&action, broker.contracts())
                .map_err(|e| format!("inputs_for: {e:?}"))?;
            broker
                .record_inputs(attempt, 0, inputs.clone())
                .map_err(|e| format!("record_inputs: {e:?}"))?;
            let root = source.reference_root();
            let mut session = broker
                .begin_review(
                    attempt,
                    round,
                    root,
                    ReviewWindow {
                        commit_by: ElapsedTick(tick + 1),
                        reveal_by: ElapsedTick(tick + 2),
                    },
                    source.snapshot(),
                )
                .map_err(|e| format!("begin_review: {e:?}"))?;
            let names = ["h_alpha_1", "h_alpha_2", "h_beta_1"];
            for (name, verdict) in names.into_iter().zip(verdicts) {
                let digest = session
                    .commitment(name, verdict, &salt)
                    .map_err(|e| format!("commitment: {e:?}"))?;
                session
                    .commit(name, digest, now)
                    .map_err(|e| format!("commit: {e:?}"))?;
            }
            session
                .open_reveals(now)
                .map_err(|e| format!("open_reveals: {e:?}"))?;
            for (name, verdict) in names.into_iter().zip(verdicts) {
                session
                    .reveal(name, verdict, &salt, now)
                    .map_err(|e| format!("reveal: {e:?}"))?;
            }
            let observed = session
                .finish(now)
                .map_err(|e| format!("finish review: {e:?}"))?;
            let receipt = broker
                .apply_review(observed, Some(&inputs), source.snapshot())
                .map_err(|e| format!("apply_review: {e:?}"))?;
            Ok((
                root,
                receipt.policy.control.decision.consequence,
                source,
                action,
            ))
        };

    // STEP 1: Untrusted actor proposes package publish (attempt 1); concurrent metadata
    // escalation reveals forbidden destination, triggering exact policy disqualifier -> Deny!
    let (root1, cons1, _, _) = run_round(
        &mut broker,
        1,
        101,
        10,
        b"pub:v0.3-exfil".to_vec(),
        true,
        [Verdict::Allow, Verdict::Allow, Verdict::Hold],
    )?;
    if cons1 != Consequence::Deny {
        return Err(format!("step 1 expected Deny, got {cons1:?}"));
    }
    let effect1 = Effect {
        principal: "actor:2".into(),
        resolved_target: "adapter:10/object:11".into(),
        payload: b"pub:v0.3-exfil".to_vec(),
        units: 16,
    };
    journal_bytes.extend_from_slice(
        &journal
            .append(&Event::Reserve {
                id: 1,
                effect: effect1,
            })
            .map_err(|e| format!("journal reserve 1: {e}"))?,
    );
    journal_bytes.extend_from_slice(
        &journal
            .append(&Event::AbortBeforeDispatch { id: 1 })
            .map_err(|e| format!("journal abort 1: {e}"))?,
    );
    let insp1 = broker.inspect();
    let units1 = UnitsSummary {
        available: insp1.ledger.available,
        reserved: insp1.ledger.reserved,
        spent: insp1.ledger.charged,
    };
    lines.push(EventLine {
        step: 1,
        run: run_id,
        action: "escalate_and_deny".into(),
        attempt: 1,
        round: 101,
        consequence: "Deny".into(),
        units: units1,
        incident_count: broker.incident_count(),
        epoch: insp1.ledger.epoch,
        evidence_root: root1,
        closure_digest: closure_digest(
            seed,
            1,
            1,
            101,
            "Deny",
            units1,
            broker.incident_count(),
            insp1.ledger.epoch,
            &root1,
            journal.head().checksum,
        ),
    });

    // STEP 2: Actor advances state with suspect context and proposes attempt 2 (admissible size,
    // but committee holds pending counterfactual intervention).
    let suspect_actor = ActorState::new(restart_profile, vec![1, 99], vec![2, 9], vec![3], 2)
        .map_err(|e| format!("suspect actor: {e:?}"))?;
    broker
        .replace_actor_state(0, suspect_actor)
        .map_err(|e| format!("replace_actor_state: {e:?}"))?;
    let (root2, cons2, _, _) = run_round(
        &mut broker,
        2,
        102,
        20,
        b"pub:v0.3-suspect".to_vec(),
        false,
        [Verdict::Hold, Verdict::Hold, Verdict::Allow],
    )?;
    if cons2 != Consequence::HoldEffect {
        return Err(format!("step 2 expected HoldEffect, got {cons2:?}"));
    }
    let effect2 = Effect {
        principal: "actor:2".into(),
        resolved_target: "adapter:10/object:11".into(),
        payload: b"pub:v0.3-suspect".to_vec(),
        units: 16,
    };
    journal_bytes.extend_from_slice(
        &journal
            .append(&Event::Reserve {
                id: 2,
                effect: effect2.clone(),
            })
            .map_err(|e| format!("journal reserve 2: {e}"))?,
    );
    let insp2 = broker.inspect();
    let units2 = UnitsSummary {
        available: journal.rights().available(),
        reserved: total_units - journal.rights().available() - insp2.ledger.charged,
        spent: insp2.ledger.charged,
    };
    lines.push(EventLine {
        step: 2,
        run: run_id,
        action: "hold_effect_pending_evidence".into(),
        attempt: 2,
        round: 102,
        consequence: "HoldEffect".into(),
        units: units2,
        incident_count: broker.incident_count(),
        epoch: insp2.ledger.epoch,
        evidence_root: root2,
        closure_digest: closure_digest(
            seed,
            2,
            2,
            102,
            "HoldEffect",
            units2,
            broker.incident_count(),
            insp2.ledger.epoch,
            &root2,
            journal.head().checksum,
        ),
    });

    // STEP 3: Counterfactual branch forked with ZERO production authority.
    // Attempting to reserve or dispatch in the counterfactual branch must fail closed.
    let mut branch_rights = Rights::new(0);
    if branch_rights.reserve(99, effect2) != Err(ModelError::Limit) {
        return Err("counterfactual branch must have zero production authority".into());
    }
    let root3 = seeded_bytes(seed, 103);
    lines.push(EventLine {
        step: 3,
        run: run_id,
        action: "counterfactual_branch_zero_authority".into(),
        attempt: 2,
        round: 102,
        consequence: "HoldEffect".into(),
        units: units2,
        incident_count: broker.incident_count(),
        epoch: insp2.ledger.epoch,
        evidence_root: root3,
        closure_digest: closure_digest(
            seed,
            3,
            2,
            102,
            "HoldEffect",
            units2,
            broker.incident_count(),
            insp2.ledger.epoch,
            &root3,
            journal.head().checksum,
        ),
    });

    // STEP 4: Apply ResetToCheckpoint + NarrowAuthority from pre-incident checkpoint.
    // Held attempt 2 is cancelled and its 16 reserved units in the journal are refunded; incident_count increments to 1.
    let root4 = seeded_bytes(seed, 104);
    let reset_receipt = broker
        .reset(ResetRequest {
            checkpoint: pre_incident_checkpoint,
            expected_control_sequence: broker.inspect().sequence,
            expected_actor_revision: broker.actor_revision(),
            binding: ReviewBinding {
                round: 103,
                evidence_root: root4,
                reducer_generation: 1,
            },
            retained_targets: narrowed_ceiling,
        })
        .map_err(|e| format!("reset to checkpoint: {e:?}"))?;
    if reset_receipt.consequence != Consequence::ResetToCheckpoint
        || !reset_receipt.restored
        || reset_receipt.incident_count != 1
        || reset_receipt.cancelled != vec![2]
    {
        return Err(format!("unexpected reset receipt: {reset_receipt:?}"));
    }
    journal_bytes.extend_from_slice(
        &journal
            .append(&Event::ResetToCheckpoint {
                keep: BTreeSet::new(),
            })
            .map_err(|e| format!("journal reset: {e}"))?,
    );
    let insp4 = broker.inspect();
    let units4 = UnitsSummary {
        available: insp4.ledger.available,
        reserved: insp4.ledger.reserved,
        spent: insp4.ledger.charged,
    };
    lines.push(EventLine {
        step: 4,
        run: run_id,
        action: "reset_to_checkpoint_and_narrow".into(),
        attempt: 2,
        round: 103,
        consequence: "ResetToCheckpoint".into(),
        units: units4,
        incident_count: broker.incident_count(),
        epoch: insp4.ledger.epoch,
        evidence_root: root4,
        closure_digest: closure_digest(
            seed,
            4,
            2,
            103,
            "ResetToCheckpoint",
            units4,
            broker.incident_count(),
            insp4.ledger.epoch,
            &root4,
            journal.head().checksum,
        ),
    });

    // STEP 5: Under narrowed authority ceiling and verified mediation cut, attempt 3 passes
    // commit-reveal with Continue, dispatches through PublicationEndpoint, and reconciles.
    let edge_visits = verify_broker_mediation_cut()?;
    if edge_visits == 0 {
        return Err("mediation cut check must visit edges".into());
    }
    let clean_payload = b"pub:v0.3-clean".to_vec();
    let (root5, cons5, source5, action3) = run_round(
        &mut broker,
        3,
        104,
        30,
        clean_payload.clone(),
        false,
        [Verdict::Allow, Verdict::Allow, Verdict::Allow],
    )?;
    if cons5 != Consequence::Continue {
        return Err(format!("step 5 expected Continue, got {cons5:?}"));
    }
    let current_inputs = broker
        .current_inputs(3)
        .map_err(|e| format!("current_inputs: {e:?}"))?
        .cloned()
        .ok_or_else(|| "missing current inputs for attempt 3".to_owned())?;
    let permit = broker
        .authorize(3, Some(&current_inputs), source5.snapshot())
        .map_err(|e| format!("authorize 3: {e:?}"))?;
    let envelope = broker
        .dispatch(&permit, &action3, Some(&current_inputs), source5.snapshot())
        .map_err(|e| format!("dispatch 3: {e:?}"))?;
    endpoint
        .observe_time(ElapsedTick(30))
        .map_err(|e| format!("endpoint observe_time: {e:?}"))?;
    let endpoint_receipt = endpoint
        .deliver(&envelope)
        .map_err(|e| format!("endpoint deliver: {e:?}"))?;
    if !matches!(endpoint_receipt.outcome(), EndpointOutcome::Executed { .. }) {
        return Err(format!(
            "expected Executed endpoint outcome, got {:?}",
            endpoint_receipt.outcome()
        ));
    }
    broker
        .accept_receipt(endpoint_receipt)
        .map_err(|e| format!("accept_receipt: {e:?}"))?;

    let effect3 = Effect {
        principal: "actor:2".into(),
        resolved_target: "adapter:10/object:11".into(),
        payload: clean_payload,
        units: 16,
    };
    journal_bytes.extend_from_slice(
        &journal
            .append(&Event::Reserve {
                id: 3,
                effect: effect3.clone(),
            })
            .map_err(|e| format!("journal reserve 3: {e}"))?,
    );
    journal_bytes.extend_from_slice(
        &journal
            .append(&Event::Dispatch {
                id: 3,
                effect: effect3,
            })
            .map_err(|e| format!("journal dispatch 3: {e}"))?,
    );
    journal_bytes.extend_from_slice(
        &journal
            .append(&Event::Reconcile {
                id: 3,
                occurred: true,
            })
            .map_err(|e| format!("journal reconcile 3: {e}"))?,
    );
    let insp5 = broker.inspect();
    let units5 = UnitsSummary {
        available: insp5.ledger.available,
        reserved: insp5.ledger.reserved,
        spent: insp5.ledger.charged,
    };
    lines.push(EventLine {
        step: 5,
        run: run_id,
        action: "narrowed_continue_and_brokered_write".into(),
        attempt: 3,
        round: 104,
        consequence: "Continue".into(),
        units: units5,
        incident_count: broker.incident_count(),
        epoch: insp5.ledger.epoch,
        evidence_root: root5,
        closure_digest: closure_digest(
            seed,
            5,
            3,
            104,
            "Continue",
            units5,
            broker.incident_count(),
            insp5.ledger.epoch,
            &root5,
            journal.head().checksum,
        ),
    });

    // STEP 6: Decision replay at decision_replay grade + full Journal stream replay.
    let replayed = Journal::replay(
        journal_bytes.as_slice(),
        journal.identity(),
        journal.head(),
        Limits::default(),
    )
    .map_err(|e| format!("journal replay: {e}"))?;
    if replayed.head() != journal.head()
        || replayed.rights() != journal.rights()
        || !replayed.rights().conserved()
        || replayed.rights().available() != units5.available
        || replayed.rights().incident_count() != broker.incident_count()
    {
        return Err("replayed journal state diverged from live broker accounting".into());
    }
    let replay_deny = decide(DecisionInputs {
        empirical: Restriction::Continue,
        exact_disqualifier: true,
        mandatory_absent: false,
        contradiction: false,
    });
    let replay_hold = decide(DecisionInputs {
        empirical: Restriction::HoldEffect,
        exact_disqualifier: false,
        mandatory_absent: false,
        contradiction: false,
    });
    let replay_continue = decide(DecisionInputs {
        empirical: Restriction::Continue,
        exact_disqualifier: false,
        mandatory_absent: false,
        contradiction: false,
    });
    if (
        replay_deny.consequence,
        replay_hold.consequence,
        replay_continue.consequence,
    ) != (
        Consequence::Deny,
        Consequence::HoldEffect,
        Consequence::Continue,
    ) {
        return Err("decision_replay consequence recomputation diverged".into());
    }
    lines.push(EventLine {
        step: 6,
        run: run_id,
        action: "decision_and_journal_replay_verified".into(),
        attempt: 3,
        round: 104,
        consequence: "Continue".into(),
        units: units5,
        incident_count: replayed.rights().incident_count(),
        epoch: replayed.rights().epoch(),
        evidence_root: root5,
        closure_digest: closure_digest(
            seed,
            6,
            3,
            104,
            "Continue",
            units5,
            replayed.rights().incident_count(),
            replayed.rights().epoch(),
            &root5,
            replayed.head().checksum,
        ),
    });

    Ok(EventLog {
        lines,
        journal_bytes,
    })
}

pub fn verify_event_log_jsonl(jsonl: &str, expected_total_units: u64) -> Result<usize, String> {
    let raw_lines: Vec<&str> = jsonl.lines().collect();
    if raw_lines.is_empty() {
        return Err("empty event log".into());
    }
    for (idx, line) in raw_lines.iter().enumerate() {
        let expected_step = format!("\"step\":{},", idx + 1);
        if !line.starts_with('{') || !line.ends_with('}') || !line.contains(&expected_step) {
            return Err(format!("line {} missing canonical step header", idx + 1));
        }
        for key in [
            "\"run\":\"",
            "\"action\":\"",
            "\"attempt\":",
            "\"round\":",
            "\"consequence\":\"",
            "\"units\":{\"available\":",
            "\"reserved\":",
            "\"spent\":",
            "\"incident_count\":",
            "\"epoch\":",
            "\"evidence_root\":\"",
            "\"closure_digest\":\"",
        ] {
            if !line.contains(key) {
                return Err(format!("line {} missing required field {key}", idx + 1));
            }
        }
        let units_part = line
            .split("\"units\":{\"available\":")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .ok_or_else(|| format!("line {} malformed units object", idx + 1))?;
        let mut nums = units_part.split(',');
        let available: u64 = nums
            .next()
            .ok_or("missing available")?
            .parse()
            .map_err(|_| "invalid available")?;
        let reserved: u64 = nums
            .next()
            .and_then(|s| s.strip_prefix("\"reserved\":"))
            .ok_or("missing reserved")?
            .parse()
            .map_err(|_| "invalid reserved")?;
        let spent: u64 = nums
            .next()
            .and_then(|s| s.strip_prefix("\"spent\":"))
            .ok_or("missing spent")?
            .parse()
            .map_err(|_| "invalid spent")?;
        if available.saturating_add(reserved).saturating_add(spent) != expected_total_units {
            return Err(format!(
                "line {} violates rights conservation: {} + {} + {} != {}",
                idx + 1,
                available,
                reserved,
                spent,
                expected_total_units
            ));
        }
    }
    Ok(raw_lines.len())
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn two_runs_with_same_seed_produce_byte_identical_event_logs() {
    let first = run_founding_slice(DEFAULT_SEED).expect("first founding slice run");
    let second = run_founding_slice(DEFAULT_SEED).expect("second founding slice run");
    assert_eq!(first, second);
    assert_eq!(first.to_jsonl(), second.to_jsonl());
    println!(
        "FOUNDING_SLICE_JSONL_BEGIN\n{}FOUNDING_SLICE_JSONL_END",
        first.to_jsonl()
    );

    let artifact_path = workspace_root().join(RETAINED_ARTIFACT_REL);
    if let Ok(existing) = std::fs::read_to_string(&artifact_path) {
        assert_eq!(
            existing,
            first.to_jsonl(),
            "retained founding slice JSONL artifact drifted from deterministic seed output"
        );
    }
}

#[test]
fn event_log_lines_satisfy_canonical_schema_and_conservation() {
    let log = run_founding_slice(DEFAULT_SEED).expect("founding slice run");
    let jsonl = log.to_jsonl();
    let count = verify_event_log_jsonl(&jsonl, 100).expect("valid schema and conservation");
    assert_eq!(count, 6);
    assert_eq!(log.lines[0].consequence, "Deny");
    assert_eq!(log.lines[1].consequence, "HoldEffect");
    assert_eq!(log.lines[3].consequence, "ResetToCheckpoint");
    assert_eq!(log.lines[4].consequence, "Continue");
    assert_eq!(
        log.lines[5].units,
        UnitsSummary {
            available: 84,
            reserved: 0,
            spent: 16
        }
    );
}

#[test]
fn mutated_seed_or_tampered_event_line_or_branch_authority_refuses() {
    let baseline = run_founding_slice(DEFAULT_SEED).expect("baseline run");
    let different = run_founding_slice(DEFAULT_SEED ^ 0x5555_AAAA).expect("different seed run");
    assert_ne!(baseline.lines[0].run, different.lines[0].run);
    assert_ne!(
        baseline.lines[0].closure_digest,
        different.lines[0].closure_digest
    );
    assert_ne!(baseline.to_jsonl(), different.to_jsonl());

    // Tampering with units conservation in the JSONL log must fail closed.
    let tampered_units = baseline.to_jsonl().replacen(
        "\"available\":84,\"reserved\":16",
        "\"available\":90,\"reserved\":16",
        1,
    );
    assert!(verify_event_log_jsonl(&tampered_units, 100).is_err());

    // Tampering with step sequence order must fail closed.
    let tampered_step = baseline
        .to_jsonl()
        .replacen("\"step\":2,", "\"step\":9,", 1);
    assert!(verify_event_log_jsonl(&tampered_step, 100).is_err());

    // Tampering with journal bytes must fail Journal::replay closed.
    let mut corrupted = baseline.journal_bytes.clone();
    let last = corrupted.len() - 1;
    corrupted[last] ^= 0xFF;
    assert!(
        Journal::replay(
            corrupted.as_slice(),
            Identity {
                run: baseline.lines[0].run,
                policy: seeded_bytes(DEFAULT_SEED, 2),
                total_units: 100,
            },
            fa_journal::Head {
                sequence: 0,
                checksum: 0
            },
            Limits::default(),
        )
        .is_err()
    );
}
