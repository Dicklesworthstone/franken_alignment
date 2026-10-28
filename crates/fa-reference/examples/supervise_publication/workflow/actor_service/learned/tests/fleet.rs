//! The actual fleet command over original numerical histories and coordinator.
//! These fixtures establish control behavior, not inference quality or latency.
use super::super::{fleet as command_fleet, prediction};
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::{
    guarded::anchored::MAX_HISTORY_ANCHOR_BYTES,
    shutdown::coordinator::FileShutdownCoordinatorAnchor,
};
use fa_reference::strict_json::{self, Json, Limits};
use std::{
    cell::Cell,
    io::{self, Write},
};
#[path = "fleet/fixture.rs"]
mod inputs;
use inputs::{Fleet, INPUT_LIMIT, Member, Model, OPERATION, WALL, private};

fn parse(bytes: &[u8]) -> Json {
    strict_json::parse(bytes, Limits::default()).unwrap()
}
fn members(json: &Json) -> &[Json] {
    json.get("members").unwrap().as_array().unwrap()
}
fn text<'a>(json: &'a Json, key: &str) -> &'a str {
    json.get(key).unwrap().as_str().unwrap()
}
fn flag(json: &Json, key: &str) -> bool {
    json.get(key).unwrap().as_bool().unwrap()
}
fn run(
    fleet: &Fleet,
    mode: &str,
    extra: &[&str],
    clock: impl FnMut() -> Result<ElapsedTick, String>,
) -> (Result<(), String>, Json) {
    let mut output = Vec::new();
    let result = command_fleet::command_with(
        &fleet.args(mode, extra),
        clock,
        &mut || Duration::ZERO,
        &mut output,
    );
    assert!(
        !output.is_empty(),
        "expected native fleet report: {result:?}"
    );
    (result, parse(&output))
}

#[test]
fn mixed_original_and_all_three_learned_families_stop_and_resume_without_reopening_live_inputs() {
    let fleet = Fleet::new(vec![
        Member::new(1, Model::Ordinary, false),
        Member::new(2, Model::Plain, false),
        Member::new(3, Model::Raw, false),
        Member::new(4, Model::Owned, false),
    ]);
    let retained = fleet.root.0.join("first-coordinator.anchor");
    let mut args = fleet.args("create", &[]);
    args.extend([
        "--retain-coordinator-anchor".into(),
        retained.display().to_string(),
    ]);
    let calls = Cell::new(0);
    let mut output = Vec::new();
    command_fleet::command_with(
        &args,
        || {
            calls.set(calls.get() + 1);
            Ok(ElapsedTick(1001))
        },
        &mut || Duration::ZERO,
        &mut output,
    )
    .unwrap();
    let json = parse(&output);
    assert_eq!(text(&json, "schema_version"), "fa.shutdown-fleet-result/2");
    assert_eq!(members(&json).len(), 4);
    assert_eq!(calls.get(), 4);
    assert!(flag(&json, "all_observed_stopped"));
    assert!(flag(&json, "all_observed_drained"));
    assert_eq!(text(&json, "final_revision"), "8");
    for (member, report) in fleet.members.iter().zip(members(&json)) {
        assert_eq!(text(report, "domain"), member.id.to_string());
        assert_eq!(text(report, "result"), "observed");
        assert_eq!(text(report, "stop_operation"), OPERATION.to_string());
        let state = member.snapshot();
        assert_eq!(state.revision, member.before.revision + 3);
        assert_eq!(state.executions, 0);
        assert_eq!(state.control.ledger.charged, 0);
        assert!(state.stop.as_ref().unwrap().request().operation == OPERATION);
        fs::write(
            member.config.store.join("delivery.pending"),
            b"passive evidence remains staged",
        )
        .unwrap();
        assert_eq!(
            fs::read_dir(member.root.0.join("peers")).unwrap().count(),
            0
        );
    }
    let before = fleet.files();
    let journal = fleet.coordinated();
    let next = fleet.root.0.join("next-coordinator.anchor");
    let mut args = fleet.args("resume", &[]);
    args.extend([
        "--coordinator-anchor".into(),
        retained.display().to_string(),
        "--retain-coordinator-anchor".into(),
        next.display().to_string(),
    ]);
    let mut output = Vec::new();
    command_fleet::command_with(
        &args,
        || panic!("passive observed resolution cannot acquire a clock"),
        &mut || Duration::ZERO,
        &mut output,
    )
    .unwrap();
    let json = parse(&output);
    assert!(flag(&json, "all_observed_drained"));
    assert_eq!(members(&json).len(), 4);
    for member in members(&json) {
        assert_eq!(member.get("acquired_tick"), Some(&Json::Null));
    }
    assert_eq!(fleet.files(), before);
    assert_eq!(fleet.coordinated(), journal);
    assert_eq!(fs::read(retained).unwrap(), fs::read(next).unwrap());
}

#[test]
fn separate_admission_and_selected_drain_preserve_the_original_dispatched_liability() {
    for model in [Model::Plain, Model::Raw, Model::Owned] {
        let fleet = Fleet::new(vec![
            Member::new(1, Model::Ordinary, false),
            Member::new(2, model, true),
        ]);
        let active = fleet.files();
        let (result, json) = run(&fleet, "create", &["--drain-only"], || {
            panic!("active drain has no clock")
        });
        assert!(result.is_err());
        assert!(!flag(&json, "all_observed_stopped"));
        assert!(
            members(&json)
                .iter()
                .all(|member| text(member, "error") == "wrong_state")
        );
        assert_eq!(fleet.files(), active);

        let (result, json) = run(&fleet, "recover", &["--admission-only"], || {
            panic!("admission is clock-free")
        });
        result.unwrap();
        assert!(flag(&json, "all_observed_stopped"));
        assert!(!flag(&json, "all_observed_drained"));
        assert_eq!(text(&json, "phase"), "admission");
        assert!(!flag(&json, "settlement_requested"));
        let admission = fleet.states();
        assert_eq!(admission[1].revision, fleet.members[1].before.revision + 2);
        assert_eq!(
            admission[1].control.ledger.charged,
            fleet.members[1].before.control.ledger.charged
        );
        assert!(admission[1].control.ledger.charged > 0);
        assert_eq!(admission[1].executions, 0);
        let stopped = fleet.files();
        let coordinator = fleet.coordinated();
        let (result, json) = run(&fleet, "resume", &["--drain-only"], || {
            panic!("resume cannot drain observed work")
        });
        assert!(result.is_err());
        assert!(flag(&json, "all_observed_stopped"));
        assert!(!flag(&json, "all_observed_drained"));
        assert_eq!(fleet.files(), stopped);
        assert_eq!(fleet.coordinated(), coordinator);

        let calls = Cell::new(0);
        let (result, json) = run(
            &fleet,
            "recover",
            &["--drain-only", "--recover-domain", "2"],
            || {
                calls.set(calls.get() + 1);
                Ok(ElapsedTick(1001))
            },
        );
        result.unwrap();
        assert_eq!(calls.get(), 1);
        assert!(flag(&json, "all_observed_drained"));
        assert_eq!(text(&json, "schema_version"), "fa.shutdown-fleet-result/4");
        assert_eq!(members(&json).len(), 2);
        assert_eq!(members(&json)[0].get("acquired_tick"), Some(&Json::Null));
        let completed = fleet.states();
        assert_eq!(completed[0], admission[0]);
        assert_eq!(completed[1].revision, admission[1].revision + 1);
        assert_eq!(completed[1].control.ledger.charged, 0);
        assert_eq!(completed[1].executions, 0);
        let completed = fleet.files();
        let coordinator = fleet.coordinated();
        let (result, json) = run(&fleet, "resume", &[], || {
            panic!("completed resolution cannot acquire time")
        });
        result.unwrap();
        assert!(flag(&json, "all_observed_drained"));
        assert_eq!(fleet.files(), completed);
        assert_eq!(fleet.coordinated(), coordinator);
    }
}

#[test]
fn learned_argv_and_closed_v3_custody_refuse_before_any_coordinator_or_live_fallback() {
    let base = ["create".to_owned(), "/missing/fleet-profile".to_owned()];
    for extra in [
        vec!["--coordinator-anchor", "--terminal-clocks", "/archive"],
        vec![
            "--retain-coordinator-anchor",
            "--admission-only",
            "/archive",
        ],
        vec!["--terminal-clocks", "--drain-only"],
        vec!["--admission-only", "--admission-only"],
        vec!["--recover-domain", "1"],
        vec!["--resume"],
        vec!["--credibility-activation", "/evidence"],
        vec!["--coordinator-anchor"],
        vec!["--unknown"],
    ] {
        let args = base
            .iter()
            .cloned()
            .chain(extra.into_iter().map(String::from))
            .collect::<Vec<_>>();
        let error = command_fleet::command_with(
            &args,
            || panic!("malformed argv clock"),
            &mut || panic!("malformed argv cannot start input budget"),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(!error.contains("No such file"), "{error}");
    }
    let fleet = Fleet::new(vec![Member::new(1, Model::Plain, false)]);
    let original = fleet.document(INPUT_LIMIT);
    let before = fleet.files();
    let invalid = [
        original.replace("\"plan_archive\":null,", ""),
        original.replace("\"plan_archive\":null", "\"plan_archive\":false"),
        original.replace(
            "\"learned_recipe\":",
            "\"topology_profile\":\"/missing/topology\",\"learned_recipe\":",
        ),
        original.replace(
            "\"learned_recipe\":",
            "\"unknown\":true,\"learned_recipe\":",
        ),
        original.replace("fa.shutdown-fleet/3", "fa.shutdown-fleet/1"),
        original.replace("\"id\":\"1\"", "\"id\":\"0\""),
    ];
    for changed in invalid {
        assert_ne!(changed, original);
        private(&fleet.manifest(), changed.as_bytes());
        assert!(
            command_fleet::command_with(
                &fleet.args("create", &[]),
                || panic!("profile clock"),
                &mut || Duration::ZERO,
                &mut Vec::new()
            )
            .is_err()
        );
        assert!(!fleet.coordinator().exists());
        assert_eq!(fleet.files(), before);
    }
    fleet.write(INPUT_LIMIT);
    let error =
        crate::fleet_shutdown::command(&fleet.args("create", &[]), &mut Vec::new()).unwrap_err();
    assert!(error.contains("require supervise_publication shutdown-fleet"));
    assert!(!fleet.coordinator().exists());
    assert_eq!(fleet.files(), before);
    let (result, json) = run(&fleet, "create", &[], || Ok(ElapsedTick(1001)));
    result.unwrap();
    assert!(flag(&json, "all_observed_drained"));
}

#[test]
fn one_cumulative_input_allowance_covers_two_numerical_members_and_streamed_checkpoint_weights() {
    let first = Member::new(1, Model::Plain, false);
    first.use_source_manifest();
    let fleet = Fleet::new(vec![first, Member::new(2, Model::Owned, false)]);
    let exact = fleet.exact_input_bytes();
    assert!(exact < INPUT_LIMIT);
    assert!(exact > fleet.document(exact).len());
    let before = fleet.files();
    fleet.write(exact - 1);
    let error = command_fleet::command_with(
        &fleet.args("create", &[]),
        || panic!("one-short intake cannot acquire time"),
        &mut || Duration::ZERO,
        &mut Vec::new(),
    )
    .unwrap_err();
    assert!(!error.is_empty());
    assert!(!fleet.coordinator().exists());
    assert_eq!(fleet.files(), before);
    fleet.write(exact);
    let (result, json) = run(&fleet, "create", &[], || Ok(ElapsedTick(1001)));
    result.unwrap();
    assert_eq!(members(&json).len(), 2);
    assert!(flag(&json, "all_observed_drained"));
}

#[test]
fn nested_weights_and_other_assets_cannot_come_from_another_registered_authority_store() {
    let first = Member::new(1, Model::Plain, false);
    let (source, original_source) = first.use_source_manifest();
    let fleet = Fleet::new(vec![first, Member::new(2, Model::Raw, false)]);
    let foreign_weights = fleet.members[1].config.store.join("foreign-weights.bin");
    private(
        &foreign_weights,
        &fs::read(fleet.members[0].root.0.join("weights.safetensors")).unwrap(),
    );
    let changed = original_source.replace(
        fleet.members[0]
            .root
            .0
            .join("weights.safetensors")
            .to_str()
            .unwrap(),
        foreign_weights.to_str().unwrap(),
    );
    assert_ne!(changed, original_source);
    private(&source, changed.as_bytes());
    let before = fleet.files();
    let error = command_fleet::command_with(
        &fleet.args("create", &[]),
        || panic!("foreign weight custody"),
        &mut || Duration::ZERO,
        &mut Vec::new(),
    )
    .unwrap_err();
    assert!(
        error.contains("authority") || error.contains("custody") || error.contains("outside"),
        "{error}"
    );
    assert!(!fleet.coordinator().exists());
    assert_eq!(fleet.files(), before);
    private(&source, original_source.as_bytes());

    let recipe = fleet.members[0].recipe.as_ref().unwrap();
    let original = fs::read_to_string(recipe).unwrap();
    let tokenizer = fleet.members[0].root.0.join("tokenizer.json");
    let foreign_tokenizer = fleet.members[1].config.store.join("foreign-tokenizer.json");
    private(&foreign_tokenizer, &fs::read(&tokenizer).unwrap());
    let changed = original.replace(
        tokenizer.to_str().unwrap(),
        foreign_tokenizer.to_str().unwrap(),
    );
    assert_ne!(changed, original);
    private(recipe, changed.as_bytes());
    let before = fleet.files();
    assert!(
        command_fleet::command_with(
            &fleet.args("create", &[]),
            || panic!("foreign auxiliary custody"),
            &mut || Duration::ZERO,
            &mut Vec::new()
        )
        .is_err()
    );
    assert!(!fleet.coordinator().exists());
    assert_eq!(fleet.files(), before);
    private(recipe, original.as_bytes());
    let (result, json) = run(&fleet, "create", &[], || Ok(ElapsedTick(1001)));
    result.unwrap();
    assert!(flag(&json, "all_observed_drained"));
}

#[test]
fn passive_learned_resolution_rechecks_full_floors_and_suffix_without_command_fallback() {
    let fleet = Fleet::new(vec![Member::new(1, Model::Owned, false)]);
    run(&fleet, "create", &[], || Ok(ElapsedTick(1001)))
        .0
        .unwrap();
    fs::write(
        fleet.members[0].config.store.join("delivery.pending"),
        b"passive staging retained",
    )
    .unwrap();
    let before = fleet.files();
    let coordinator = fleet.coordinated();
    let recipe = fleet.members[0].recipe.as_ref().unwrap();
    let original = fs::read_to_string(recipe).unwrap();
    let changed = original.replace(
        "\"journal_revision\":0",
        &format!(
            "\"journal_revision\":{}",
            fleet.members[0].snapshot().revision + 1
        ),
    );
    assert_ne!(changed, original);
    private(recipe, changed.as_bytes());
    let (result, json) = run(&fleet, "resume", &[], || {
        panic!("failed independent floor cannot become fresh work")
    });
    assert!(result.is_err());
    assert_eq!(text(&members(&json)[0], "result"), "evidence_refused");
    assert_eq!(text(&members(&json)[0], "error"), "stale");
    assert_eq!(fleet.files(), before);
    assert_eq!(fleet.coordinated(), coordinator);
    private(recipe, original.as_bytes());

    let path = fleet.members[0].config.store.join("delivery.bin");
    let canonical = fs::read(&path).unwrap();
    let mut corrupt = canonical.clone();
    corrupt.push(0xff);
    fs::write(&path, &corrupt).unwrap();
    let before_corrupt = fleet.files();
    let (result, json) = run(&fleet, "resume", &[], || {
        panic!("bad suffix cannot trigger a clock")
    });
    assert!(result.is_err());
    assert_eq!(text(&members(&json)[0], "result"), "evidence_refused");
    assert_eq!(fleet.files(), before_corrupt);
    assert_eq!(fleet.coordinated(), coordinator);
    fs::write(&path, canonical).unwrap();
    let (result, json) = run(&fleet, "resume", &[], || {
        panic!("restored original cut only resolves")
    });
    result.unwrap();
    assert!(flag(&json, "all_observed_drained"));
    assert_eq!(fleet.files(), before);
    assert_eq!(fleet.coordinated(), coordinator);
}

#[test]
fn the_same_deadline_covers_intake_replay_and_both_sides_of_terminal_time() {
    let control = Fleet::new(vec![
        Member::new(1, Model::Plain, false),
        Member::new(2, Model::Ordinary, false),
    ]);
    let checks = Cell::new(0);
    let first_clock = Cell::new(0);
    command_fleet::command_with(
        &control.args("create", &[]),
        || {
            if first_clock.get() == 0 {
                first_clock.set(checks.get());
            }
            Ok(ElapsedTick(1001))
        },
        &mut || {
            checks.set(checks.get() + 1);
            Duration::ZERO
        },
        &mut Vec::new(),
    )
    .unwrap();
    assert!(first_clock.get() > 4);
    for cut in [
        1,
        4,
        first_clock.get() - 1,
        first_clock.get(),
        first_clock.get() + 1,
    ] {
        let fleet = Fleet::new(vec![
            Member::new(1, Model::Plain, false),
            Member::new(2, Model::Ordinary, false),
        ]);
        let before = fleet.files();
        let seen = Cell::new(0);
        let calls = Cell::new(0);
        let mut output = Vec::new();
        let result = command_fleet::command_with(
            &fleet.args("create", &[]),
            || {
                calls.set(calls.get() + 1);
                Ok(ElapsedTick(1001))
            },
            &mut || {
                seen.set(seen.get() + 1);
                if seen.get() >= cut {
                    WALL
                } else {
                    Duration::ZERO
                }
            },
            &mut output,
        );
        assert!(result.is_err(), "cut {cut}");
        assert_eq!(fleet.files(), before, "cut {cut}");
        assert_eq!(
            calls.get(),
            usize::from(cut > first_clock.get()),
            "cut {cut}"
        );
        if !output.is_empty() {
            let json = parse(&output);
            assert_eq!(members(&json).len(), 2);
            assert!(flag(&json, "deadline_reached"));
            assert!(!flag(&json, "all_observed_stopped"));
            assert_eq!(text(&members(&json)[1], "result"), "not_visited");
        }
    }
}

#[test]
fn retained_coordinator_custody_survives_output_write_or_flush_failure() {
    struct Broken {
        flush: bool,
    }
    impl Write for Broken {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.flush {
                Ok(bytes.len())
            } else {
                Err(io::ErrorKind::BrokenPipe.into())
            }
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
    }
    for flush in [false, true] {
        let fleet = Fleet::new(vec![
            Member::new(1, Model::Raw, false),
            Member::new(2, Model::Owned, false),
        ]);
        let saved = fleet.root.0.join("retained.anchor");
        let mut args = fleet.args("create", &["--terminal-clocks"]);
        args.extend([
            "--retain-coordinator-anchor".into(),
            saved.display().to_string(),
        ]);
        let error = command_fleet::command_with(
            &args,
            || Ok(ElapsedTick(1001)),
            &mut || Duration::ZERO,
            &mut Broken { flush },
        )
        .unwrap_err();
        assert!(error.contains("output failed"));
        let anchor =
            FileShutdownCoordinatorAnchor::from_trusted_bytes(&fs::read(&saved).unwrap()).unwrap();
        let before = fleet.files();
        let coordinator = fleet.coordinated();
        let next = fleet.root.0.join("recovered.anchor");
        let mut args = fleet.args("resume", &[]);
        args.extend([
            "--coordinator-anchor".into(),
            saved.display().to_string(),
            "--retain-coordinator-anchor".into(),
            next.display().to_string(),
        ]);
        let mut output = Vec::new();
        command_fleet::command_with(
            &args,
            || panic!("lost stdout does not repeat effects or settlement"),
            &mut || Duration::ZERO,
            &mut output,
        )
        .unwrap();
        assert!(flag(&parse(&output), "all_observed_drained"));
        let recovered =
            FileShutdownCoordinatorAnchor::from_trusted_bytes(&fs::read(next).unwrap()).unwrap();
        assert_eq!(anchor, recovered);
        assert_eq!(fleet.files(), before);
        assert_eq!(fleet.coordinated(), coordinator);
    }
}

#[test]
fn archived_mixed_roster_keeps_busy_and_missing_domains_visible_then_recovers_only_named_members() {
    let mut fleet = Fleet::new(vec![
        Member::new(1, Model::Ordinary, false),
        Member::new(2, Model::Plain, false),
        Member::new(3, Model::Raw, false),
        Member::new(4, Model::Owned, false),
    ]);
    fleet.retain_plan();
    let before = fleet.images();
    let blocked = &fleet.members[1];
    let held = FileOversight::begin_open_with_learned_generation(
        &blocked.config.store,
        blocked.config.profile.clone(),
        blocked.generation.as_ref().unwrap(),
    )
    .unwrap();
    let hidden = fleet.members[2].root.0.join("offline-store");
    fs::rename(&fleet.members[2].config.store, &hidden).unwrap();
    let (result, json) = run(&fleet, "create", &[], || Ok(ElapsedTick(1001)));
    assert!(result.is_err());
    assert_eq!(members(&json).len(), 4);
    assert_eq!(text(&members(&json)[0], "result"), "observed");
    assert_eq!(text(&members(&json)[1], "error"), "busy");
    assert_eq!(text(&members(&json)[2], "result"), "refused");
    assert_eq!(text(&members(&json)[3], "result"), "observed");
    assert!(!flag(&json, "all_observed_stopped"));
    assert_eq!(fleet.members[1].image(), before[1]);
    assert_eq!(fs::read(hidden.join("delivery.bin")).unwrap(), before[2]);
    drop(held);
    fs::rename(hidden, &fleet.members[2].config.store).unwrap();
    let stopped = [fleet.members[0].files(), fleet.members[3].files()];
    let calls = Cell::new(0);
    let (result, json) = run(
        &fleet,
        "recover",
        &["--recover-domain", "3", "--recover-domain", "2"],
        || {
            calls.set(calls.get() + 1);
            Ok(ElapsedTick(1001))
        },
    );
    result.unwrap();
    assert_eq!(calls.get(), 2);
    assert_eq!(members(&json).len(), 4);
    assert!(flag(&json, "all_observed_drained"));
    assert_eq!(
        [fleet.members[0].files(), fleet.members[3].files()],
        stopped
    );
    let complete = fleet.files();
    let coordinator = fleet.coordinated();
    run(&fleet, "resume", &[], || {
        panic!("complete archived fleet has no new terminal time")
    })
    .0
    .unwrap();
    assert_eq!(fleet.files(), complete);
    assert_eq!(fleet.coordinated(), coordinator);
}
