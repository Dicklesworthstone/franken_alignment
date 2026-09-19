//! Real Store barriers and original numerical controls, not an imported cursor.
use super::*;
use super::super::{StepRequest, write, read};
use crate::action::{ActionSpec, ActionState, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::activation::monitor::{MonitorOutcome, decoder::{MonitoredStep, MonitoringStatus}};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationBudget, GenerationRequest, MAX_SAMPLING_ENTRIES,
};
use crate::action::consequence::activation::tensor::kv::decoder::{
    DecoderBudget, DecoderIdentity, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS,
};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::{SampleBudget, SamplingBudget};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalLimits, codec::shared::{Reader, Writer}};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract, human::HumanReviewPolicy};
use crate::action::consequence::oversight::decoder_host::{HostedStopCause, HostedStopPolicy};
use crate::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use crate::Snapshot;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

mod data { include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/decoder_inputs.rs")); }
static NEXT: AtomicU64 = AtomicU64::new(0);
const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync];
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-progress-{}-{now}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("generation progress cleanup: {error}"); } }
}
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile { delivery: FileDeliveryProfile {
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        total: 100, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 1,
            tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
        suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
        congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".into(), MemberPolicy { cohort: "one".into(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
            narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(), retention_ticks: 1000,
        max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
    }, committee: CommitteeContract::new(BTreeMap::from([("reviewer".into(), HelperContract::new(InputProfileBinding {
        profile_id: 1, profile_bytes: b"progress-test".to_vec(), tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1,
    }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 } }
}
fn config(threshold: f32) -> FileDecoderConfig {
    let numerical = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 9, model_generation: 1,
        tokenizer_generation: 1, profile_generation: 1 }, DecoderShape { vocabulary: 2, hidden: 2,
        intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context: 4 }, 1e-5, 10000.0).unwrap();
    FileDecoderConfig::new(numerical, data::weights(), data::monitor(threshold), data::sampling(), 5, DecoderBindingLimits::default()).unwrap()
}
fn budget() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
fn request(prompt: &[u32], new: usize) -> GenerationRequest {
    GenerationRequest { prompt: prompt.to_vec(), max_new_tokens: new, stop_tokens: Vec::new(),
        budget: GenerationBudget { scalar_products: MAX_DECODER_PRODUCTS, sampling_entries: MAX_SAMPLING_ENTRIES } }
}
fn owner(root: &Directory, p: &FileOversightProfile, c: &FileDecoderConfig) -> FileOversight {
    let (mut host, _) = FileOversight::create(root.store(), p.clone()).unwrap();
    host.enable_decoder(host.revision(), c.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); host
}
fn begin(host: &mut FileOversight, r: GenerationRequest) -> FileGenerationCommand {
    let n = host.decoder_inspection().unwrap().numerical;
    let command = FileGenerationCommand::new(7, n.actor_revision, n.position, r).unwrap();
    host.begin_decoder_generation(host.revision(), command.clone()).unwrap(); command
}
fn advance(host: &mut FileOversight) -> FileGenerationProgress {
    let next = host.decoder_generation_progress(7).unwrap().generation_revision();
    host.advance_decoder_generation(host.revision(), 7, next).unwrap()
}
fn resume(host: &mut FileOversight) {
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    host.resume_decoder(host.revision(), n.actor_revision, n.position).unwrap();
}

#[test]
fn progress_cuts_match_original_stepwise_sampler_and_only_terminal_progress_has_a_receipt() {
    let root = Directory::new(); let p = profile(); let c = config(3.0);
    let mut host = owner(&root, &p, &c); let mut control = c.build().unwrap();
    let command = begin(&mut host, request(&[0], 2));
    let start = host.revision(); let authority = host.inspect().control;
    let initial = host.decoder_generation_progress(7).unwrap();
    assert_eq!(initial.generation_revision(), 0); assert!(initial.partial().is_none());
    assert!(initial.receipt().is_none());
    control.advance_forced(0, 0, budget()).unwrap();
    let first = advance(&mut host);
    assert_eq!(first.generation_revision(), 1); assert!(!first.is_complete());
    assert_eq!(first.partial().unwrap().reviewed_prompt_tokens(), 1);
    assert!(first.partial().unwrap().report().is_none());
    assert!(matches!(host.decoder_generation(7), Err(JournalError::Contract(Error::Incomplete))));
    let mut tokens = Vec::new();
    for position in 1..3 {
        let MonitoredStep::Released(step) = control.advance_sampled(position, SampleBudget {
            decoder: budget(), sampling: SamplingBudget { vocabulary: 2 },
        }).unwrap().into_monitored() else { panic!("quiet independent control"); };
        tokens.push(step.step().token);
        let actual = advance(&mut host);
        assert_eq!(actual.tokens(), tokens.as_slice());
        assert_eq!(actual.is_complete(), position == 2);
        assert_eq!(actual.generation_revision(), position + 1);
        assert_eq!(host.machine.broker.hosted_replay_bytes().unwrap(), control.replay_bytes().unwrap());
    }
    assert_eq!(host.revision(), start + 3);
    let receipt = host.decoder_generation(7).unwrap();
    assert_eq!(receipt.command(), &command);
    assert_eq!(receipt.result().unwrap().work().admitted_sampling_entries, 4);
    assert!(host.pending_decoder_generation().unwrap().is_none());
    assert_eq!(host.inspect().control, authority); assert_eq!(host.inspect().executions, 0);
    let disk = FileOversight::read_decoder_generation_progress(root.store(), &p, &c, 7).unwrap();
    assert_eq!(disk.generation.tokens(), tokens.as_slice());
    assert_eq!(disk.publication, host.inspect()); assert_eq!(disk.numerical, host.decoder_inspection().unwrap());
}

#[test]
fn recovery_preserves_progress_and_spent_request_budget_instead_of_restarting_prefill() {
    let root = Directory::new(); let p = profile(); let c = config(3.0);
    let mut host = owner(&root, &p, &c);
    let mut r = request(&[0, 0], 2); r.budget.scalar_products = 76;
    let command = begin(&mut host, r);
    let first = advance(&mut host); assert_eq!(first.partial().unwrap().work().admitted_scalar_products, 36);
    drop(host);
    let (mut host, _) = FileOversight::open_with_decoder(root.store(), p.clone(), &c).unwrap();
    assert_eq!(host.pending_decoder_generation().unwrap(), Some(command.clone()));
    assert_eq!(host.decoder_generation_progress(7).unwrap().generation_revision(), 1);
    assert!(host.decoder_inspection().unwrap().paused);
    assert!(matches!(host.advance_decoder_generation(host.revision(), 7, 1), Err(JournalError::Contract(Error::Incomplete))));
    resume(&mut host);
    assert!(matches!(host.generate_decoder(host.revision(), command), Err(JournalError::Contract(Error::WrongState))));
    let second = advance(&mut host);
    assert_eq!(second.partial().unwrap().reviewed_prompt_tokens(), 2);
    assert_eq!(second.partial().unwrap().work().admitted_scalar_products, 76);
    let third = advance(&mut host);
    assert_eq!(third.finish(), Some(Ok(GenerationFinish::BudgetExhausted)));
    assert_eq!(third.receipt().unwrap().result().unwrap().end_position(), 2);
    assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 0);
    assert_eq!(host.decoder_inspection().unwrap().numerical.numerical.tokens, 2);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn old_revision_retries_return_current_progress_without_extra_draws_even_while_paused() {
    let root = Directory::new(); let p = profile(); let c = config(3.0);
    let mut host = owner(&root, &p, &c); begin(&mut host, request(&[0], 2));
    advance(&mut host); let current = advance(&mut host);
    let before = host.inspect(); let numerical = host.decoder_inspection().unwrap();
    let retry = host.advance_decoder_generation(0, 7, 0).unwrap();
    assert_eq!(retry.generation_revision(), 2); assert_eq!(retry.tokens(), current.tokens());
    assert_eq!(host.inspect(), before); assert_eq!(host.decoder_inspection().unwrap(), numerical);
    assert!(matches!(host.advance_decoder_generation(host.revision(), 7, 3), Err(JournalError::Contract(Error::Stale))));
    assert!(matches!(host.advance_decoder_generation(0, 7, 2), Err(JournalError::Contract(Error::Stale))));
    drop(host);
    let (mut host, _) = FileOversight::open_with_decoder(root.store(), p, &c).unwrap();
    let before = host.inspect();
    assert_eq!(host.advance_decoder_generation(0, 7, 1).unwrap().tokens(), current.tokens());
    assert_eq!(host.inspect(), before); assert!(host.decoder_inspection().unwrap().paused);
    resume(&mut host); let final_progress = advance(&mut host);
    assert_eq!(final_progress.finish(), Some(Ok(GenerationFinish::TokenLimit)));
    let before = host.inspect();
    assert_eq!(host.advance_decoder_generation(0, 7, 3).unwrap().tokens(), final_progress.tokens());
    assert_eq!(host.inspect(), before);
}

#[test]
fn partial_progress_cannot_be_bypassed_by_batch_raw_token_or_a_different_command() {
    let root = Directory::new(); let mut host = owner(&root, &profile(), &config(3.0));
    let command = begin(&mut host, request(&[0, 0], 1)); advance(&mut host);
    let before = host.inspect(); let n = host.decoder_inspection().unwrap().numerical;
    assert!(matches!(host.generate_decoder(host.revision(), command.clone()), Err(JournalError::Contract(Error::WrongState))));
    assert!(matches!(host.advance_decoder_forced(host.revision(), n.actor_revision, n.position, 1, budget()),
        Err(JournalError::Contract(Error::Incomplete))));
    let mut changed = command.request().clone(); changed.prompt[1] = 1;
    let changed = FileGenerationCommand::new(7, command.actor_revision(), command.position(), changed).unwrap();
    assert!(matches!(host.begin_decoder_generation(host.revision(), changed), Err(JournalError::Contract(Error::Binding))));
    assert!(matches!(host.advance_decoder_generation(host.revision(), 8, 0), Err(JournalError::Contract(Error::Missing))));
    let spec = ActionSpec { version: VERSION, scope: profile().delivery.scope, target: Some(host.inspect().target),
        payload: b"output".to_vec(), required_witnesses: Vec::new(), policy_epoch: before.control.ledger.epoch,
        deadline: ElapsedTick(20), units: 16 };
    assert!(matches!(host.propose(host.revision(), 1, spec, Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() }),
        Err(JournalError::Contract(Error::Incomplete))));
    assert!(host.capture_decoder_checkpoint(host.revision(), 5, n.actor_revision, before.control.ledger.epoch).is_err());
    assert_eq!(host.inspect(), before); assert!(host.storage_failure().is_none());
    advance(&mut host); assert!(advance(&mut host).is_complete());
}

#[test]
fn stop_token_gets_its_own_review_and_remains_in_the_actual_decoder_history() {
    let root = Directory::new(); let mut host = owner(&root, &profile(), &config(3.0));
    let mut r = request(&[0], 3); r.stop_tokens = vec![0, 1]; begin(&mut host, r);
    assert!(!advance(&mut host).is_complete());
    let result = advance(&mut host);
    assert_eq!(result.finish(), Some(Ok(GenerationFinish::StopToken))); assert!(result.tokens().is_empty());
    assert_eq!(host.decoder_inspection().unwrap().numerical.position, 2);
    assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 1);
    assert_eq!(host.machine.broker.retained_actor_state().tokens().len(), 2);
    assert_eq!(host.decoder_inspection().unwrap().numerical.monitoring.frame_reviews, 2);
}

#[test]
fn native_alarm_still_stops_authority_and_cancels_preexisting_undispatched_work() {
    let root = Directory::new(); let p = profile(); let c = config(1.5);
    let mut host = owner(&root, &p, &c);
    host.enable_decoder_stop(host.revision(), HostedStopPolicy::new(1, 1, 900).unwrap()).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    host.advance_decoder_forced(host.revision(), n.actor_revision, 0, 0, budget()).unwrap().unwrap();
    let spec = ActionSpec { version: VERSION, scope: p.delivery.scope, target: Some(host.inspect().target),
        payload: b"output".to_vec(), required_witnesses: Vec::new(), policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(20), units: 16 };
    host.propose(host.revision(), 1, spec, Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() }).unwrap();
    begin(&mut host, request(&[], 2)); let result = advance(&mut host);
    assert_eq!(result.finish(), Some(Ok(GenerationFinish::Held))); assert!(result.tokens().is_empty());
    assert!(host.inspect().control.suspended); assert!(host.decoder_inspection().unwrap().paused);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    assert_eq!(host.decoder_stop_incident().unwrap().unwrap().cause(), HostedStopCause::Monitoring(MonitorOutcome::Alarm));
    assert_eq!(host.decoder_inspection().unwrap().numerical.status, MonitoringStatus::Held);
    assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 1);
    assert_eq!(host.inspect().executions, 0);
    assert!(host.advance_decoder_generation(0, 7, 0).unwrap().is_complete());
}

#[test]
fn every_storage_barrier_preserves_the_last_acknowledged_progress_and_never_releases_a_candidate() {
    for alarm in [false, true] {
        for barrier in BARRIERS {
            let root = Directory::new(); let p = profile(); let c = config(if alarm { 1.5 } else { 3.0 });
            let mut host = owner(&root, &p, &c);
            host.enable_decoder_stop(host.revision(), HostedStopPolicy::new(1, 1, 900).unwrap()).unwrap();
            begin(&mut host, request(&[0], 2)); advance(&mut host);
            let before = host.inspect(); let numerical = host.decoder_inspection().unwrap().numerical;
            host.store.fail_once(barrier);
            assert!(matches!(host.advance_decoder_generation(host.revision(), 7, 1), Err(JournalError::Io(_))));
            assert_eq!(host.inspect(), before);
            assert!(matches!(host.decoder_generation_progress(7), Err(JournalError::Unavailable)));
            assert!(matches!(host.advance_decoder_generation(0, 7, 0), Err(JournalError::Unavailable)));
            let disk = FileOversight::read_decoder_generation_progress(root.store(), &p, &c, 7).unwrap();
            let replaced = barrier == JournalIo::DirectorySync;
            assert_eq!(disk.generation.generation_revision(), if replaced { 2 } else { 1 });
            assert_eq!(disk.numerical.numerical.position, numerical.position + u64::from(replaced));
            assert_eq!(disk.numerical.numerical.sampled_draws, u64::from(replaced));
            assert_eq!(disk.generation.is_complete(), alarm && replaced);
            assert_eq!(disk.publication.control.suspended, alarm && replaced);
            assert_eq!(disk.publication.executions, 0);
            drop(host);
            let (mut host, _) = FileOversight::open_with_decoder(root.store(), p, &c).unwrap();
            assert!(host.decoder_inspection().unwrap().paused);
            if alarm && replaced {
                assert_eq!(host.advance_decoder_generation(0, 7, 1).unwrap().finish(), Some(Ok(GenerationFinish::Held)));
            } else {
                resume(&mut host);
                let current = advance(&mut host);
                assert_eq!(current.is_complete(), alarm || replaced);
                if !current.is_complete() { assert!(advance(&mut host).is_complete()); }
            }
            assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, if alarm { 1 } else { 2 });
            assert_eq!(host.inspect().executions, 0);
        }
    }
}

#[test]
fn changed_witness_id_or_progress_predecessor_is_rejected_by_original_replay() {
    let root = Directory::new(); let p = profile(); let mut host = owner(&root, &p, &config(3.0));
    begin(&mut host, request(&[0], 2)); advance(&mut host); advance(&mut host);
    assert!(Machine::replay(&p, &host.events).is_ok());
    let last = host.events.len() - 1;
    for mutation in 0..3 {
        let mut events = host.events.clone();
        let Event::Decoder(DecoderEvent::AdvanceGeneration { id, revision, witness }) = &mut events[last]
            else { panic!("progress event"); };
        match mutation {
            0 => { let mut bytes = witness.to_vec(); let end = bytes.len() - 1; bytes[end] ^= 1; *witness = bytes.into(); }
            1 => *id = 8,
            _ => *revision = 0,
        }
        assert!(Machine::replay(&p, &events).is_err());
    }
    let mut duplicate = host.events.clone(); duplicate.push(host.events[last].clone());
    assert!(Machine::replay(&p, &duplicate).is_err());
    let n = host.decoder_inspection().unwrap().numerical;
    assert!(host.machine.prepare_decoder_step(StepRequest::Forced { revision: n.actor_revision,
        position: n.position, token: 1, products: MAX_DECODER_PRODUCTS }).is_err());
}

#[test]
fn new_progress_wire_tag_has_an_independent_vector_and_rejects_truncation() {
    let event = DecoderEvent::AdvanceGeneration { id: 7, revision: 2, witness: Rc::from(&b"x"[..]) };
    let mut w = Writer::new(100); write(&mut w, &event).unwrap(); let bytes = w.finish();
    let mut expected = vec![8];
    for value in [7_u64, 2] { expected.extend_from_slice(&value.to_be_bytes()); }
    expected.extend_from_slice(&1_u32.to_be_bytes()); // Original journal blob length is u32.
    expected.push(b'x'); assert_eq!(bytes, expected);
    let mut r = Reader::new(&bytes); let decoded = read(&mut r).unwrap(); r.end().unwrap();
    assert!(matches!(decoded, DecoderEvent::AdvanceGeneration { id: 7, revision: 2, .. }));
    for end in 0..bytes.len() { assert!(read(&mut Reader::new(&bytes[..end])).is_err()); }
    for (id, revision, witness, error) in [
        (0, 0, Rc::from(&b"x"[..]), Error::InvalidInput),
        (7, MAX_GENERATION_TOKENS as u64, Rc::from(&b"x"[..]), Error::Limit),
        (7, 0, Rc::from(&b""[..]), Error::Incomplete),
    ] {
        assert_eq!(write(&mut Writer::new(100), &DecoderEvent::AdvanceGeneration { id, revision, witness }), Err(error));
    }
}

#[test]
fn undersized_incremental_journal_refuses_before_work_but_the_same_frozen_batch_still_fits() {
    let root = Directory::new(); let mut p = profile(); p.delivery.limits.events = 4;
    let mut host = owner(&root, &p, &config(3.0)); let command = begin(&mut host, request(&[0], 2));
    let before = host.inspect(); let numerical = host.decoder_inspection().unwrap();
    assert!(matches!(host.advance_decoder_generation(host.revision(), 7, 0), Err(JournalError::Contract(Error::Limit))));
    assert_eq!(host.inspect(), before); assert_eq!(host.decoder_inspection().unwrap(), numerical);
    assert!(host.storage_failure().is_none()); assert!(host.pending_decoder_generation().unwrap().is_some());
    assert_eq!(host.decoder_generation_progress(7).unwrap().generation_revision(), 0);
    let receipt = host.generate_decoder(host.revision(), command).unwrap();
    assert_eq!(receipt.result().unwrap().finish(), GenerationFinish::TokenLimit);
    assert_eq!(receipt.result().unwrap().tokens().len(), 2); assert_eq!(host.revision(), 4);
}

#[test]
fn intervening_control_work_cannot_make_capacity_failure_discard_acknowledged_tokens() {
    let root = Directory::new(); let mut p = profile(); p.delivery.limits.events = 7;
    let mut host = owner(&root, &p, &config(3.0)); begin(&mut host, request(&[0], 2));
    advance(&mut host);
    for tick in 2..5 { host.observe_time(host.revision(), ElapsedTick(tick)).unwrap(); }
    assert_eq!(host.revision(), 7);
    let before = host.inspect(); let numerical = host.decoder_inspection().unwrap();
    assert!(matches!(host.advance_decoder_generation(host.revision(), 7, 1), Err(JournalError::Contract(Error::Limit))));
    assert_eq!(host.inspect(), before); assert_eq!(host.decoder_inspection().unwrap(), numerical);
    assert!(host.storage_failure().is_none());
    assert_eq!(host.decoder_generation_progress(7).unwrap().partial().unwrap().reviewed_prompt_tokens(), 1);
}

#[test]
fn invalid_or_underfunded_complete_prompt_records_a_refusal_without_any_partial_prefill() {
    for (r, error) in [
        (request(&[0, 2], 1), Error::InvalidInput),
        ({ let mut r = request(&[0, 0], 1); r.budget.scalar_products = 75; r }, Error::Limit),
    ] {
        let root = Directory::new(); let mut host = owner(&root, &profile(), &config(3.0));
        begin(&mut host, r); let before = host.decoder_inspection().unwrap().numerical;
        let progress = advance(&mut host);
        assert_eq!(progress.finish(), Some(Err(error))); assert!(progress.tokens().is_empty());
        assert_eq!(host.decoder_inspection().unwrap().numerical, before);
        assert_eq!(progress.generation_revision(), 1);
        let revision = host.revision(); advance(&mut host); assert_eq!(host.revision(), revision);
    }
}

#[test]
fn canonical_inspection_requires_exact_model_bytes_and_validates_even_later_progress() {
    let root = Directory::new(); let p = profile(); let c = config(3.0);
    let mut host = owner(&root, &p, &c); begin(&mut host, request(&[0], 2)); advance(&mut host);
    assert!(matches!(FileOversight::read_decoder_generation_progress(root.store(), &p, &config(4.0), 7),
        Err(JournalError::Contract(Error::Binding))));
    let before = host.inspect();
    assert!(FileOversight::read_decoder_generation_progress(root.store(), &p, &c, 7).is_ok());
    assert_eq!(host.inspect(), before);
    advance(&mut host);
    let mut events = host.events.clone();
    let Event::Decoder(DecoderEvent::AdvanceGeneration { witness, .. }) = events.last_mut().unwrap()
        else { panic!("progress event"); };
    let mut bytes = witness.to_vec(); bytes[0] ^= 1; *witness = bytes.into();
    let corrupt = journal::encode(&p, host.store.identity(), &events).unwrap();
    std::fs::write(host.store.identity().join(storage::CANONICAL), corrupt).unwrap();
    assert!(FileOversight::read_decoder_generation_progress(root.store(), &p, &c, 7).is_err());
}
