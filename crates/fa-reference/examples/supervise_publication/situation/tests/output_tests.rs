use super::*;
use fa_reference::action::{MAX_ATTEMPTS, Purpose, ResolvedTarget, Scope};
use fa_reference::action::consequence::delivery::persistent::observed::replay::situation::{
    FileSituationAttempt, FileSituationBasis, FileSituationOrigin, FileSituationPage, FileSituationSummary,
};

#[test]
fn every_replay_checkpoint_can_expire_without_publishing_partial_knowledge() {
    let mut fixture = Fixture::new(); fixture.clock(); fixture.propose(1, true); fixture.propose(2, false);
    let files = fixture.files();
    for stop in 1..=fixture.host.revision() as usize + 2 {
        let replay = FileOversight::begin_read_situation(&fixture.config.store, &fixture.config.profile, None).unwrap();
        let mut checks = 0;
        let error = finish_replay(replay, &mut || {
            checks += 1;
            check_elapsed(if checks == stop { Duration::from_secs(1) } else { Duration::ZERO }, Duration::from_secs(1))
        }).unwrap_err();
        assert!(error.contains("elapsed allowance")); assert_eq!(checks, stop); assert_eq!(fixture.files(), files);
    }
    let replay = FileOversight::begin_read_situation(&fixture.config.store, &fixture.config.profile, None).unwrap();
    assert!(finish_replay(replay, &mut || Ok(())).is_ok());
}

struct Partial { bytes: Vec<u8>, writes: usize }
impl Write for Partial {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        if self.writes > 1 { return Err(io::Error::new(io::ErrorKind::BrokenPipe, "fixture")); }
        let count = 3.min(bytes.len()); self.bytes.extend_from_slice(&bytes[..count]); Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> { panic!("partial failed write cannot flush") }
}
#[test]
fn partial_stdout_failure_never_reopens_recovers_or_retries() {
    let mut fixture = Fixture::new(); fixture.clock(); fixture.propose(1, true);
    let files = fixture.files(); let state = fixture.host.inspect(); let mut out = Partial { bytes: vec![], writes: 0 };
    assert!(command(&fixture.args(1), &mut out).unwrap_err().contains("output failed"));
    assert_eq!(out.writes, 2); assert_eq!(out.bytes.len(), 3);
    assert_eq!(fixture.files(), files); assert_eq!(fixture.host.inspect(), state);
}

#[test]
fn cumulative_output_bounds_and_post_write_deadlines_preserve_partial_enumeration() {
    let mut fixture = Fixture::new(); fixture.clock(); fixture.propose(1, true); fixture.propose(2, true);
    let report = fixture.host.situation().unwrap(); let files = fixture.files(); let mut full = Vec::new();
    emit(&report, 64, &mut full, MAX_OUTPUT_BYTES, &mut || Ok(())).unwrap();
    let mut exact = Vec::new(); emit(&report, 64, &mut exact, full.len(), &mut || Ok(())).unwrap();
    assert_eq!(exact, full);
    let mut out = Vec::new(); assert!(emit(&report, 64, &mut out, full.len() - 1, &mut || Ok(())).is_err());
    assert!(out.is_empty());
    let Knowledge::Known { value: first, basis } = report.page(None, 1).unwrap() else { panic!("known") };
    let first = render::page(&first, basis).unwrap();
    assert!(emit(&report, 1, &mut out, first.len(), &mut || Ok(())).is_err());
    assert_eq!(out, first); assert!(!complete(&lines(&out)[0]));
    let mut late = Vec::new(); let mut checks = 0;
    assert!(emit(&report, 64, &mut late, MAX_OUTPUT_BYTES, &mut || {
        checks += 1; check_elapsed(if checks == 3 { Duration::from_secs(1) } else { Duration::ZERO }, Duration::from_secs(1))
    }).is_err());
    assert_eq!(late, full); assert_eq!(checks, 3); assert_eq!(fixture.files(), files);
}

struct AdvanceOwner<'a> { host: &'a mut FileOversight, action: Option<ActionSpec>, bytes: Vec<u8> }
impl Write for AdvanceOwner<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> { self.bytes.extend_from_slice(bytes); Ok(bytes.len()) }
    fn flush(&mut self) -> io::Result<()> {
        if let Some(action) = self.action.take() {
            self.host.propose(self.host.revision(), 3, action, snapshot(true)).unwrap();
        }
        Ok(())
    }
}
#[test]
fn pages_do_not_reopen_a_new_journal_when_the_real_writer_advances_during_output() {
    let mut fixture = Fixture::new(); fixture.clock();
    let action = fixture.propose(1, true).spec().clone(); fixture.propose(2, true);
    let revision = fixture.host.revision(); let args = fixture.args(1);
    let mut out = AdvanceOwner { host: &mut fixture.host, action: Some(action), bytes: vec![] };
    command(&args, &mut out).unwrap(); let pages = lines(&out.bytes); drop(out);
    assert_eq!(pages.len(), 2);
    for page in &pages {
        assert_eq!(text(summary(page), "attempts"), "2");
        assert_eq!(text(knowledge(page).get("basis").unwrap(), "journal_revision"), revision.to_string());
    }
    assert_eq!(fixture.host.revision(), revision + 1);
    assert_eq!(fixture.host.inspect().control.ledger.stages.len(), 3);
    assert!(complete(pages.last().unwrap()));
}

#[test]
fn closed_stage_rendering_and_full_width_bounds_do_not_truncate_metadata() {
    // Synthetic serialization vectors, NOT accepted native control histories.
    let stages = [ActionState::Proposed, ActionState::Prepared, ActionState::Reviewing, ActionState::Authorized,
        ActionState::Dispatching, ActionState::Confirmed, ActionState::Denied, ActionState::Cancelled,
        ActionState::Unknown, ActionState::ConfirmedNotExecuted, ActionState::IrrecoverablyUnknown];
    let names = ["proposed", "prepared", "reviewing", "authorized", "dispatching", "confirmed", "denied", "cancelled",
        "unknown", "confirmed_not_executed", "irrecoverably_unknown"];
    let basis = FileSituationBasis { scope: Scope { tenant: u64::MAX, principal: u64::MAX,
        run: u64::MAX, branch: u64::MAX, authority: u64::MAX, purpose: Purpose::Effect },
        journal_revision: u64::MAX, control_sequence: u64::MAX, recorded_at: Some(ElapsedTick(u64::MAX)),
        origin: FileSituationOrigin::CanonicalImage, anchored_through: Some(u64::MAX) };
    let row = FileSituationAttempt { attempt: u64::MAX, stage: ActionState::IrrecoverablyUnknown,
        target: ResolvedTarget { adapter: u64::MAX, object: u64::MAX, contract_version: u64::MAX,
            expected_version: u64::MAX, generation: u64::MAX }, units: u64::MAX, payload_bytes: usize::MAX,
        proposal_policy_epoch: u64::MAX, deadline: ElapsedTick(u64::MAX) };
    let mut page = FileSituationPage { summary: FileSituationSummary {
        total_units: u64::MAX, available_units: u64::MAX, reserved_units: 0, charged_units: 0,
        policy_epoch: u64::MAX, dispatcher_epoch: u64::MAX, suspended: true, stop_recorded: true,
        executions: u64::MAX, attempts: usize::MAX, retained_requests: usize::MAX,
        pending_attempts: usize::MAX, unresolved_attempts: usize::MAX,
        irrecoverably_unknown_attempts: usize::MAX, terminal_attempts: usize::MAX,
    }, attempts: vec![row; MAX_SITUATION_PAGE_ATTEMPTS], offset: usize::MAX, next: None };
    let bytes = render::page(&page, basis).unwrap(); assert!(bytes.len() <= render::MAX_PAGE_BYTES);
    let decoded = lines(&bytes); assert_eq!(rows(&decoded[0]).len(), MAX_SITUATION_PAGE_ATTEMPTS);
    assert_eq!(text(&rows(&decoded[0])[0], "deadline"), u64::MAX.to_string());
    page.attempts.push(row); assert!(render::page(&page, basis).is_err());
    page.attempts = vec![row];
    assert!(render::page(&page, basis).unwrap().len() * MAX_ATTEMPTS < MAX_OUTPUT_BYTES);
    for (stage, name) in stages.into_iter().zip(names) {
        page.attempts[0].stage = stage;
        assert_eq!(text(&rows(&lines(&render::page(&page, basis).unwrap())[0])[0], "stage"), name);
    }
}
