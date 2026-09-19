//! Whole-request execution and recovery using the original real Store/decoder.
use super::*;
use super::super::generation::{FileGenerationCommand, MAX_FILE_GENERATIONS};
use crate::action::{ElapsedTick, Purpose, ResolvedTarget, Scope};
use crate::action::consequence::activation::monitor::decoder::MonitoringStatus;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationBudget, GenerationFinish, GenerationRequest, MAX_SAMPLING_ENTRIES,
};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalLimits};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract, human::HumanReviewPolicy};
use crate::action::consequence::oversight::decoder_host::{HostedStopCause, HostedStopPolicy};
use crate::action::consequence::activation::monitor::MonitorOutcome;
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-generation-{}-{now}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("generation fixture cleanup: {error}"); }
    }
}
fn host_profile() -> FileOversightProfile {
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
        profile_id: 1, profile_bytes: b"generation-test".to_vec(), tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1,
    }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 } }
}
fn owner(root: &Directory, configuration: &FileDecoderConfig) -> FileOversight {
    let (mut host, _) = FileOversight::create(root.store(), host_profile()).unwrap();
    host.enable_decoder(host.revision(), configuration.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host
}
fn request(prompt: &[u32], max_new_tokens: usize) -> GenerationRequest {
    GenerationRequest { prompt: prompt.to_vec(), max_new_tokens, stop_tokens: Vec::new(),
        budget: GenerationBudget { scalar_products: MAX_DECODER_PRODUCTS, sampling_entries: MAX_SAMPLING_ENTRIES } }
}
fn command(host: &FileOversight, id: u64, input: GenerationRequest) -> FileGenerationCommand {
    let n = host.decoder_inspection().unwrap().numerical;
    FileGenerationCommand::new(id, n.actor_revision, n.position, input).unwrap()
}

#[test]
fn durable_request_matches_original_generation_and_commits_intent_then_result_without_an_effect() {
    let root = Directory::new(); let config = config(3.0); let mut host = owner(&root, &config);
    let input = request(&[0], 2); let c = command(&host, 7, input.clone());
    let before = host.inspect();
    let mut control = config.build().unwrap(); let expected = control.generate(0, input).unwrap();
    let receipt = host.generate_decoder(host.revision(), c.clone()).unwrap();
    let report = receipt.result().unwrap();
    assert_eq!(report.tokens(), expected.tokens()); assert_eq!(report.work(), expected.work());
    assert_eq!(report.finish(), GenerationFinish::TokenLimit);
    assert_eq!(report.reviewed_prompt_tokens(), 1); assert_eq!(report.end_position(), 3);
    assert_eq!(host.revision(), before.revision + 2);
    assert!(matches!(&host.events[before.revision as usize], Event::Decoder(DecoderEvent::BeginGeneration(value)) if value.as_ref() == &c));
    assert!(host.pending_decoder_generation().unwrap().is_none());
    assert_eq!(host.machine.broker.hosted_replay_bytes().unwrap(), control.replay_bytes().unwrap());
    assert_eq!(host.inspect().control, before.control);
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().payload, b"initial");
    let replay = Machine::replay(&host_profile(), &host.events).unwrap();
    assert_eq!(replay.broker.hosted_replay_bytes().unwrap(), control.replay_bytes().unwrap());
    assert_eq!(replay.recorded_decoder_generation(7).unwrap().command(), &c);
}

#[test]
fn exact_id_retry_is_read_only_while_any_changed_command_conflicts() {
    let root = Directory::new(); let mut host = owner(&root, &config(3.0));
    let c = command(&host, 7, request(&[0], 1));
    let receipt = host.generate_decoder(host.revision(), c.clone()).unwrap();
    let before = host.inspect(); let numerical = host.decoder_inspection().unwrap();
    let repeated = host.generate_decoder(0, c.clone()).unwrap();
    assert_eq!(repeated.result().unwrap().tokens(), receipt.result().unwrap().tokens());
    for changed in [
        { let mut r = c.request().clone(); r.budget.sampling_entries -= 1; r },
        { let mut r = c.request().clone(); r.prompt = vec![1]; r },
        { let mut r = c.request().clone(); r.stop_tokens = vec![0]; r },
    ] {
        let changed = FileGenerationCommand::new(7, c.actor_revision(), c.position(), changed).unwrap();
        assert!(matches!(host.generate_decoder(host.revision(), changed), Err(JournalError::Contract(Error::Binding))));
    }
    let changed = FileGenerationCommand::new(7, c.actor_revision() + 1, c.position(), c.request().clone()).unwrap();
    assert!(matches!(host.generate_decoder(0, changed), Err(JournalError::Contract(Error::Binding))));
    let fresh = command(&host, 8, request(&[], 1));
    assert!(matches!(host.generate_decoder(0, fresh), Err(JournalError::Contract(Error::Stale))));
    assert_eq!(host.inspect(), before); assert_eq!(host.decoder_inspection().unwrap(), numerical);
}

#[test]
fn generation_codec_roundtrips_and_refuses_every_truncation_and_empty_witness() {
    let command = Rc::new(FileGenerationCommand::new(7, 1, 0, request(&[0, 1], 2)).unwrap());
    for (tag, event) in [
        (6, DecoderEvent::Generate(Rc::clone(&command), Rc::from(&b"comparison"[..]))),
        (7, DecoderEvent::BeginGeneration(Rc::clone(&command))),
    ] {
        let mut w = Writer::new(1000); write(&mut w, &event).unwrap(); let bytes = w.finish();
        assert_eq!(bytes[0], tag);
        let mut r = Reader::new(&bytes); let decoded = read(&mut r).unwrap(); r.end().unwrap();
        let mut w = Writer::new(1000); write(&mut w, &decoded).unwrap(); assert_eq!(w.finish(), bytes);
        for end in 0..bytes.len() { assert!(read(&mut Reader::new(&bytes[..end])).is_err()); }
    }
    let empty = DecoderEvent::Generate(command, Rc::from(&b""[..]));
    assert_eq!(write(&mut Writer::new(1000), &empty), Err(Error::Incomplete));
    assert!(FileGenerationCommand::new(0, 1, 0, request(&[0], 1)).is_err());
    let mut duplicate = request(&[0], 1); duplicate.stop_tokens = vec![0, 0];
    assert_eq!(FileGenerationCommand::new(1, 1, 0, duplicate).unwrap_err(), Error::Duplicate);
}

#[test]
fn altered_early_or_final_comparisons_and_repeated_journal_ids_cannot_replay() {
    let root = Directory::new(); let mut host = owner(&root, &config(3.0));
    let c = command(&host, 7, request(&[0], 2));
    host.generate_decoder(host.revision(), c).unwrap();
    let last = host.events.len() - 1;
    let Event::Decoder(DecoderEvent::Generate(command, witness)) = &host.events[last] else { panic!("generation event"); };
    let mut command_bytes = Writer::new(1000);
    super::super::generation::write_command(&mut command_bytes, command).unwrap();
    // Skip the seven-byte magic and command. Corrupt first-token evidence, not
    // merely its request header, then separately corrupt the final comparison.
    let early = 7 + command_bytes.encoded_len() + 32;
    for offset in [early, witness.len() - 1] {
        let mut changed = witness.to_vec(); changed[offset] ^= 1;
        let mut events = host.events.clone();
        events[last] = Event::Decoder(DecoderEvent::Generate(Rc::clone(command), changed.into()));
        assert!(matches!(Machine::replay(&host_profile(), &events), Err(Error::Binding)));
    }
    let mut input = command.request().clone(); input.budget.sampling_entries = 0;
    let changed = FileGenerationCommand::new(command.id(), command.actor_revision(), command.position(), input).unwrap();
    let mut events = host.events.clone();
    events[last] = Event::Decoder(DecoderEvent::Generate(Rc::new(changed), Rc::clone(witness)));
    assert!(matches!(Machine::replay(&host_profile(), &events), Err(Error::Binding)));
    let mut events = host.events.clone(); events.push(host.events[last].clone());
    assert!(matches!(Machine::replay(&host_profile(), &events), Err(Error::Duplicate)));
}

#[test]
fn recovery_retains_receipts_but_requires_fresh_time_and_resume_for_new_generation() {
    let root = Directory::new(); let config = config(3.0); let mut host = owner(&root, &config);
    let c = command(&host, 7, request(&[0], 1));
    let receipt = host.generate_decoder(host.revision(), c.clone()).unwrap();
    let tokens = receipt.result().unwrap().tokens().to_vec();
    drop(host);
    let (mut host, _) = FileOversight::open_with_decoder(root.store(), host_profile(), &config).unwrap();
    let before = host.inspect(); let numerical = host.decoder_inspection().unwrap();
    assert!(numerical.paused); assert!(!host.clock_ready());
    assert_eq!(host.decoder_generation(7).unwrap().result().unwrap().tokens(), tokens.as_slice());
    assert_eq!(host.generate_decoder(0, c).unwrap().result().unwrap().tokens(), tokens.as_slice());
    assert_eq!(host.inspect(), before); assert_eq!(host.decoder_inspection().unwrap(), numerical);
    let next = command(&host, 8, request(&[], 1));
    assert!(matches!(host.generate_decoder(host.revision(), next.clone()), Err(JournalError::Contract(Error::Incomplete))));
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert!(matches!(host.generate_decoder(host.revision(), next.clone()), Err(JournalError::Contract(Error::Incomplete))));
    host.resume_decoder(host.revision(), numerical.numerical.actor_revision, numerical.numerical.position).unwrap();
    assert_eq!(host.generate_decoder(host.revision(), next).unwrap().result().unwrap().tokens().len(), 1);
    assert_eq!(host.decoder_generation(7).unwrap().result().unwrap().tokens(), tokens.as_slice());
}

#[test]
fn held_sample_is_durable_and_retries_cannot_bypass_original_automatic_stop() {
    let root = Directory::new(); let config = config(1.5); let mut host = owner(&root, &config);
    host.enable_decoder_stop(host.revision(), HostedStopPolicy::new(7, 1, 900).unwrap()).unwrap();
    let c = command(&host, 7, request(&[0], 2));
    let receipt = host.generate_decoder(host.revision(), c.clone()).unwrap();
    let report = receipt.result().unwrap();
    assert_eq!(report.finish(), GenerationFinish::Held); assert!(report.tokens().is_empty());
    assert_eq!(report.reviewed_prompt_tokens(), 1); assert_eq!(report.end_position(), 2);
    assert_eq!(report.work().attempted_samples, 1);
    let n = host.decoder_inspection().unwrap(); assert!(n.paused);
    assert_eq!(n.numerical.sampled_draws, 1); assert_eq!(n.numerical.status, MonitoringStatus::Held);
    assert!(host.inspect().control.suspended); assert_eq!(host.inspect().executions, 0);
    let incident = host.decoder_stop_incident().unwrap().unwrap();
    assert_eq!(incident.cause(), HostedStopCause::Monitoring(MonitorOutcome::Alarm));
    assert!(incident.stop_receipt().is_some());
    drop(host);
    let (mut host, _) = FileOversight::open_with_decoder(root.store(), host_profile(), &config).unwrap();
    let before = host.inspect();
    assert_eq!(host.generate_decoder(0, c).unwrap().result().unwrap().finish(), GenerationFinish::Held);
    assert_eq!(host.inspect(), before); assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 1);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    assert!(host.resume_decoder(host.revision(), n.actor_revision, n.position).is_err());
}

#[test]
fn full_prompt_refusal_and_continuation_budget_exhaustion_are_distinct_recorded_results() {
    let root = Directory::new(); let mut host = owner(&root, &config(3.0));
    let before = host.decoder_inspection().unwrap().numerical;
    let mut input = request(&[0, 1], 1); input.budget.scalar_products = 75;
    let c = command(&host, 7, input);
    let receipt = host.generate_decoder(host.revision(), c.clone()).unwrap();
    assert_eq!(receipt.result().unwrap_err(), Error::Limit);
    assert_eq!(host.decoder_inspection().unwrap().numerical, before);
    let revision = host.revision();
    assert_eq!(host.generate_decoder(0, c).unwrap().result().unwrap_err(), Error::Limit);
    assert_eq!(host.revision(), revision);
    let mut input = request(&[0, 1], 1); input.budget.scalar_products = 76;
    let c = command(&host, 8, input);
    let receipt = host.generate_decoder(host.revision(), c).unwrap(); let report = receipt.result().unwrap();
    assert_eq!(report.finish(), GenerationFinish::BudgetExhausted);
    assert_eq!(report.reviewed_prompt_tokens(), 2); assert_eq!(report.work().admitted_scalar_products, 76);
    assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 0);
    assert!(host.decoder_stop_incident().unwrap().is_none());
}

#[test]
fn bounded_request_history_cannot_be_refilled_by_retries_or_failed_commands() {
    let root = Directory::new(); let mut host = owner(&root, &config(3.0));
    let n = host.decoder_inspection().unwrap().numerical;
    // Invalid vocabulary refuses before inference but its recorded ID still
    // consumes one bounded history slot. No expensive successful token loop.
    for id in 1..=MAX_FILE_GENERATIONS as u64 {
        let c = FileGenerationCommand::new(id, n.actor_revision, n.position, request(&[2], 0)).unwrap();
        let receipt = host.generate_decoder(host.revision(), c).unwrap();
        assert_eq!(receipt.result().unwrap_err(), Error::InvalidInput);
    }
    let before = host.inspect();
    let c = command(&host, MAX_FILE_GENERATIONS as u64 + 1, request(&[0], 1));
    assert!(matches!(host.generate_decoder(host.revision(), c), Err(JournalError::Contract(Error::Limit))));
    assert_eq!(host.inspect(), before); assert_eq!(host.decoder_inspection().unwrap().numerical, n);
}

#[path = "intent_tests.rs"]
mod intent_tests;
