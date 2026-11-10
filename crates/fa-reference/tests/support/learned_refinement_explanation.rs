//! Shared actual learned refinement/publication control for native and protocol
//! tests. Synthetic weights and explicit ballots do not qualify helper judgment.
#![allow(dead_code)]
#[path = "learned_text_model.rs"]
mod numerical;
#[path = "file_oversight_actor.rs"]
mod support;

use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::activation::probe::learned::{KvGroup, KvRow};
use fa_reference::action::consequence::activation::tensor::kv::experiment::KvSide;
use fa_reference::action::consequence::delivery::{
    EndpointOutcome,
    persistent::observed::{
        FileHumanReviewer, FileOversight, FileOversightProfile,
        decoder::learned::{
            FileLearnedConfig,
            sidecar::{FileLearnedSidecar, FileLearnedSidecarFinish},
        },
    },
    persistent::requests::actor::{FileLearnedTextStreamActorPort, LearnedTextRelease},
    stream::StreamProfile,
};
use fa_reference::action::consequence::gate::containment::{
    ActorState, RestartGrade, RestartProfile,
};
use fa_reference::action::consequence::oversight::{
    ReviewWindow,
    decoder_monitoring::LearnedDecoderBindingLimits,
    learned_host::sidecar::LearnedSidecarRequest,
    sidecar::{SidecarCongressBudget, SidecarIdentity},
};
use fa_reference::round::{Verdict, commitment};
pub use support::{Directory, Supervisor};

pub const REQUEST: u64 = 701;
pub const REFINEMENT_ROUND: u64 = 101;
pub const APPLIED_ROUND: u64 = 102;

pub fn group() -> KvGroup {
    KvGroup {
        row: KvRow {
            layer: 1,
            side: KvSide::Value,
            position: 1,
        },
        head: 0,
    }
}

pub fn profile() -> FileOversightProfile {
    let mut profile = support::profile();
    profile.delivery.total = 4096;
    profile.delivery.initial_payload.clear();
    profile.delivery.actor = ActorState::new(
        RestartProfile {
            id: 1,
            generation: 1,
            host_generation: 1,
            model_generation: 3,
            tokenizer_generation: 4,
            state_schema_generation: 1,
            grade: RestartGrade::AuditOnly,
        },
        Vec::new(),
        vec![0],
        vec![0],
        0,
    )
    .unwrap();
    profile
}

pub fn recipe() -> FileLearnedConfig {
    let model = numerical::model(&[b'O' as u32, b'K' as u32, numerical::END]);
    FileLearnedConfig::new_text_stream(
        model.clone(),
        numerical::tokenizer(&model),
        numerical::config(&model),
        LearnedDecoderBindingLimits::default(),
        StreamProfile::new(1, 1, 4, 64, 128).unwrap(),
    )
    .unwrap()
    .with_required_sidecar()
    .unwrap()
}

pub struct Prepared {
    pub actor: FileLearnedTextStreamActorPort,
    pub supervisor: Supervisor,
    pub reviewer: FileHumanReviewer,
    pub sidecar: FileLearnedSidecar,
    pub refinement_revision: u64,
}

/// Complete one real original Abstain congress after exact learned text intake.
pub fn prepared(root: &Directory) -> Prepared {
    let (mut host, reviewer) =
        FileOversight::create_with_learned_text_stream(root.store(), profile(), recipe()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let (actor, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    {
        let mut host = supervisor.host_mut().unwrap();
        for _ in 0..4 {
            let state = host.learned_generation_inspection().unwrap().numerical;
            if !state.status.is_active() {
                break;
            }
            let revision = host.revision();
            host.advance_learned_generation(revision, state.actor_revision, state.position)
                .unwrap()
                .unwrap();
        }
        assert!(
            !host
                .learned_generation_inspection()
                .unwrap()
                .numerical
                .status
                .is_active()
        );
    }
    support::observe(&mut supervisor);
    actor
        .submit(REQUEST, LearnedTextRelease::Message, ElapsedTick(100))
        .unwrap();
    let sidecar = {
        let mut host = supervisor.host_mut().unwrap();
        let attempt = support::attempt(&host, REQUEST);
        let actor_revision = host
            .learned_generation_inspection()
            .unwrap()
            .numerical
            .actor_revision;
        let revision = host.revision();
        host.begin_learned_sidecar_plan(
            revision,
            attempt,
            actor_revision,
            LearnedSidecarRequest {
                identity: SidecarIdentity {
                    object_id: 91,
                    generation: 1,
                    transform_id: 7,
                },
                priority: vec![group()],
                budget: SidecarCongressBudget {
                    rounds: 2,
                    ..SidecarCongressBudget::default()
                },
            },
        )
        .unwrap()
    };
    let mut state = Prepared {
        actor,
        supervisor,
        reviewer,
        sidecar,
        refinement_revision: 0,
    };
    let sequence = state.supervisor.host().unwrap().inspect().control.sequence;
    let result = finish(&mut state, REFINEMENT_ROUND, Verdict::Abstain);
    let FileLearnedSidecarFinish::Refined {
        group: promoted,
        input_revision,
        ..
    } = result
    else {
        panic!("fixture must execute original evidence refinement");
    };
    assert_eq!(promoted, group());
    assert_eq!(input_revision, 2);
    state.refinement_revision = state.supervisor.host().unwrap().revision();
    assert_eq!(
        state.supervisor.host().unwrap().inspect().control.sequence,
        sequence
    );
    state
}

pub fn finish(state: &mut Prepared, round: u64, verdict: Verdict) -> FileLearnedSidecarFinish {
    let mut host = state.supervisor.host_mut().unwrap();
    let revision = host.revision();
    host.begin_review(
        revision,
        state.sidecar.attempt(),
        round,
        support::oversight::ROOT,
        ReviewWindow {
            commit_by: ElapsedTick(5),
            reveal_by: ElapsedTick(10),
        },
        support::snapshot(),
    )
    .unwrap();
    for member in support::oversight::MEMBERS {
        let digest = commitment(
            round,
            member,
            &support::oversight::ROOT,
            verdict,
            b"original salt",
        )
        .unwrap();
        let revision = host.revision();
        host.commit_review(revision, round, member, digest).unwrap();
    }
    let revision = host.revision();
    host.open_reveals(revision, round).unwrap();
    for member in support::oversight::MEMBERS {
        let revision = host.revision();
        host.reveal_review(revision, round, member, verdict, b"original salt".to_vec())
            .unwrap();
    }
    let revision = host.revision();
    host.finish_learned_sidecar_review(revision, &mut state.sidecar, round, support::snapshot())
        .unwrap()
}

/// A fresh original Allow congress, independent human approval and real checked
/// publication. Return the actual applying review revision for lineage checks.
pub fn publish(state: &mut Prepared) -> u64 {
    let inputs = {
        let mut host = state.supervisor.host_mut().unwrap();
        let inputs = host
            .current_learned_sidecar(&state.sidecar)
            .unwrap()
            .clone();
        let revision = host.revision();
        assert!(
            host.authorize(
                revision,
                state.sidecar.attempt(),
                &inputs,
                support::snapshot()
            )
            .is_err()
        );
        inputs
    };
    assert!(matches!(
        finish(state, APPLIED_ROUND, Verdict::Allow),
        FileLearnedSidecarFinish::Applied { receipt: Ok(_), .. }
    ));
    let mut host = state.supervisor.host_mut().unwrap();
    let applied_revision = host.revision();
    let action = host.request_action(REQUEST).unwrap().clone();
    let revision = host.revision();
    let automatic = host
        .authorize(
            revision,
            state.sidecar.attempt(),
            &inputs,
            support::snapshot(),
        )
        .unwrap();
    let revision = host.revision();
    let request = host
        .request_human_approval(
            revision,
            1001,
            state.sidecar.attempt(),
            &inputs,
            ElapsedTick(10),
        )
        .unwrap();
    let revision = host.revision();
    let human = state
        .reviewer
        .approve(&mut host, revision, &request)
        .unwrap();
    let revision = host.revision();
    host.dispatch(
        revision,
        &automatic,
        &human,
        &action,
        &inputs,
        support::snapshot(),
    )
    .unwrap();
    let revision = host.revision();
    let result = host
        .publish_checked(
            revision,
            state.sidecar.attempt(),
            Some(&inputs),
            support::snapshot(),
            ElapsedTick(2),
        )
        .unwrap();
    assert!(matches!(result.outcome, EndpointOutcome::Executed { .. }));
    let revision = host.revision();
    host.reconcile(revision, state.sidecar.attempt()).unwrap();
    assert_eq!(host.inspect().executions, 1);
    applied_revision
}

pub fn image(root: &Directory) -> Vec<u8> {
    std::fs::read(root.store().join("delivery.bin")).unwrap()
}
