//! Operator composition over retained native source work and original numerical
//! histories. Native tests own terminal replay semantics; these cases exercise
//! explicit selection, cumulative intake, custody, and unavailable members.
use super::*;
#[path = "topology_fixture.rs"]
mod fixture;
use fixture::Case;

#[test]
fn mediated_fleet_keeps_later_source_custody_across_admission_passive_drain_and_passive() {
    for mode in [None, Some(false), Some(true)] {
        let case = Case::new(mode);
        let custody = case.custody_bytes();
        let before = case.fleet.states();
        let (result, json) = run(&case.fleet, "create", &["--admission-only"], || {
            panic!("mediated admission cannot acquire settlement time")
        });
        result.unwrap();
        assert!(flag(&json, "all_observed_stopped"));
        assert!(!flag(&json, "all_observed_drained"));
        assert_eq!(members(&json).len(), 2);
        assert_eq!(
            text(&members(&json)[0], "charged"),
            before[0].control.ledger.charged.to_string()
        );
        let admitted = case.fleet.states();
        assert_eq!(admitted[0].revision, before[0].revision + 2);
        assert_eq!(admitted[0].executions, 0);
        assert_eq!(
            admitted[0].control.ledger.charged,
            before[0].control.ledger.charged
        );
        assert_eq!(
            admitted[0].stop.as_ref().unwrap().request().operation,
            OPERATION
        );
        case.assert_source_unchanged(&custody);

        let stopped = case.fleet.files();
        let coordinator = case.fleet.coordinated();
        let (result, json) = run(&case.fleet, "resume", &["--admission-only"], || {
            panic!("passive mediated admission cannot acquire a clock")
        });
        result.unwrap();
        assert!(flag(&json, "all_observed_stopped"));
        assert!(!flag(&json, "all_observed_drained"));
        assert!(
            members(&json)
                .iter()
                .all(|member| member.get("acquired_tick") == Some(&Json::Null))
        );
        assert_eq!(case.fleet.files(), stopped);
        assert_eq!(case.fleet.coordinated(), coordinator);
        case.assert_source_unchanged(&custody);

        let calls = Cell::new(0);
        let (result, json) = run(&case.fleet, "recover", &["--drain-only"], || {
            calls.set(calls.get() + 1);
            Ok(ElapsedTick(1001))
        });
        result.unwrap();
        assert_eq!(calls.get(), 1);
        assert!(flag(&json, "all_observed_drained"));
        let drained = case.fleet.states();
        assert_eq!(drained[0].revision, admitted[0].revision + 1);
        assert_eq!(drained[0].executions, 0);
        assert_eq!(drained[0].control.ledger.charged, 0);
        assert_eq!(drained[1], admitted[1]);
        case.assert_source_unchanged(&custody);

        let complete = case.fleet.files();
        let coordinator = case.fleet.coordinated();
        let (result, json) = run(&case.fleet, "resume", &[], || {
            panic!("same retained mediated pair must resolve without new time")
        });
        result.unwrap();
        assert!(flag(&json, "all_observed_drained"));
        assert!(
            members(&json)
                .iter()
                .all(|member| member.get("acquired_tick") == Some(&Json::Null))
        );
        assert_eq!(case.fleet.files(), complete);
        assert_eq!(case.fleet.coordinated(), coordinator);
        case.assert_source_unchanged(&custody);
    }
}

#[test]
fn mediated_fleet_charges_all_five_topology_inputs_to_the_same_exact_allowance() {
    let case = Case::new(Some(true));
    let exact = case.exact_input_bytes();
    assert!(exact < INPUT_LIMIT);
    let before = case.fleet.files();
    let custody = case.custody_bytes();
    case.write(exact - 1);
    let mut output = Vec::new();
    assert!(
        command_fleet::command_with(
            &case.fleet.args("create", &[]),
            || panic!("one-short topology intake cannot acquire a terminal clock"),
            &mut || Duration::ZERO,
            &mut output
        )
        .is_err()
    );
    assert!(output.is_empty());
    assert!(!case.fleet.coordinator().exists());
    assert_eq!(case.fleet.files(), before);
    case.assert_source_unchanged(&custody);

    case.write(exact);
    let (result, json) = run(&case.fleet, "create", &[], || Ok(ElapsedTick(1001)));
    result.unwrap();
    assert_eq!(members(&json).len(), 2);
    assert!(flag(&json, "all_observed_drained"));
    case.assert_source_unchanged(&custody);
}

#[test]
fn mediated_fleet_rejects_nested_topology_inputs_from_any_registered_authority() {
    let case = Case::new(None);
    let profile = fs::read_to_string(&case.profile).unwrap();
    let root = &case.fleet.members[0].root.0;
    let other_store = &case.fleet.members[1].config.store;
    let inputs = [
        root.join("initial.json"),
        root.join("initial-inventory.json"),
        case.checkpoint.clone(),
    ];
    for (index, source) in inputs.iter().enumerate() {
        let foreign = other_store.join(format!("foreign-topology-{index}.input"));
        private(&foreign, &fs::read(source).unwrap());
        let changed = profile.replace(source.to_str().unwrap(), foreign.to_str().unwrap());
        assert_ne!(changed, profile);
        private(&case.profile, changed.as_bytes());
        let before = case.fleet.files();
        let mut output = Vec::new();
        let error = command_fleet::command_with(
            &case.fleet.args("create", &[]),
            || panic!("foreign topology custody cannot acquire time"),
            &mut || Duration::ZERO,
            &mut output,
        )
        .unwrap_err();
        assert!(
            error.contains("authority") || error.contains("outside") || error.contains("custody"),
            "{error}"
        );
        assert!(output.is_empty());
        assert!(!case.fleet.coordinator().exists());
        assert_eq!(case.fleet.files(), before);
        private(&case.profile, profile.as_bytes());
    }
    // A different lexical path cannot conceal that the initial graph comes
    // from another registered authority's directory.
    let alias = root.join("other-authority-alias");
    std::os::unix::fs::symlink(other_store, &alias).unwrap();
    let concealed = alias.join("foreign-topology-0.input");
    let changed = profile.replace(inputs[0].to_str().unwrap(), concealed.to_str().unwrap());
    assert_ne!(changed, profile);
    private(&case.profile, changed.as_bytes());
    let before = case.fleet.files();
    assert!(
        command_fleet::command_with(
            &case.fleet.args("create", &[]),
            || panic!("canonical foreign topology custody"),
            &mut || Duration::ZERO,
            &mut Vec::new()
        )
        .is_err()
    );
    assert!(!case.fleet.coordinator().exists());
    assert_eq!(case.fleet.files(), before);
    private(&case.profile, profile.as_bytes());
    let custody = case.custody_bytes();
    let (result, json) = run(&case.fleet, "create", &[], || Ok(ElapsedTick(1001)));
    result.unwrap();
    assert!(flag(&json, "all_observed_drained"));
    case.assert_source_unchanged(&custody);
}

#[test]
fn missing_mediated_member_does_not_hide_a_reachable_control_or_repeat_its_stop() {
    let mut case = Case::new(Some(false));
    case.fleet.retain_plan();
    case.write(INPUT_LIMIT);
    let custody = case.custody_bytes();
    let original = case.fleet.members[0].image();
    let hidden = case.fleet.members[0].root.0.join("offline-authority");
    fs::rename(&case.fleet.members[0].config.store, &hidden).unwrap();
    let calls = Cell::new(0);
    let (result, json) = run(&case.fleet, "create", &[], || {
        calls.set(calls.get() + 1);
        Ok(ElapsedTick(1001))
    });
    assert!(result.is_err());
    assert_eq!(calls.get(), 1);
    assert_eq!(members(&json).len(), 2);
    assert_eq!(text(&members(&json)[0], "result"), "refused");
    assert_eq!(text(&members(&json)[1], "result"), "observed");
    assert!(!flag(&json, "all_observed_stopped"));
    assert_eq!(fs::read(hidden.join("delivery.bin")).unwrap(), original);
    case.assert_source_unchanged(&custody);
    fs::rename(hidden, &case.fleet.members[0].config.store).unwrap();

    let control = case.fleet.members[1].files();
    let calls = Cell::new(0);
    let (result, json) = run(&case.fleet, "recover", &["--recover-domain", "1"], || {
        calls.set(calls.get() + 1);
        Ok(ElapsedTick(1001))
    });
    result.unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(members(&json).len(), 2);
    assert!(flag(&json, "all_observed_drained"));
    assert_eq!(members(&json)[1].get("acquired_tick"), Some(&Json::Null));
    assert_eq!(case.fleet.members[1].files(), control);
    case.assert_source_unchanged(&custody);
    let complete = case.fleet.files();
    let coordinator = case.fleet.coordinated();
    run(&case.fleet, "resume", &[], || {
        panic!("complete mediated archived roster only resolves")
    })
    .0
    .unwrap();
    assert_eq!(case.fleet.files(), complete);
    assert_eq!(case.fleet.coordinated(), coordinator);
    case.assert_source_unchanged(&custody);
}

#[test]
fn mediated_fleet_requires_paired_v2_source_custody_before_creating_a_coordinator() {
    let case = Case::new(None);
    let original = case.document(INPUT_LIMIT);
    let topology = format!(r#","topology_profile":"{}""#, case.profile.display());
    let anchor = format!(
        r#","topology_history_anchor":"{}""#,
        case.source_anchor.display()
    );
    let recipe = format!(
        r#","learned_recipe":"{}""#,
        case.fleet.members[0].recipe.as_ref().unwrap().display()
    );
    let before = case.fleet.files();
    let refuse = || {
        let mut output = Vec::new();
        assert!(
            command_fleet::command_with(
                &case.fleet.args("create", &[]),
                || panic!("unpaired source custody cannot acquire a clock"),
                &mut || Duration::ZERO,
                &mut output
            )
            .is_err()
        );
        assert!(output.is_empty());
        assert!(!case.fleet.coordinator().exists());
        assert_eq!(case.fleet.files(), before);
    };
    for changed in [
        original.replace(&topology, ""),
        original.replace(&anchor, ""),
        original.replace(&recipe, ""),
        original.replace(&anchor, ",\"topology_history_anchor\":null"),
        original.replace("fa.shutdown-fleet/3", "fa.shutdown-fleet/1"),
    ] {
        assert_ne!(changed, original);
        private(&case.fleet.manifest(), changed.as_bytes());
        refuse();
    }
    case.write(INPUT_LIMIT);

    let original_profile = fs::read_to_string(&case.profile).unwrap();
    private(&case.profile, case.manual_profile().as_bytes());
    refuse();
    let retaining = format!(
        "{},\"retain_checkpoint\":\"{}\"}}",
        original_profile.strip_suffix('}').unwrap(),
        case.fleet.members[0]
            .root
            .0
            .join("unrequested.source")
            .display()
    );
    private(&case.profile, retaining.as_bytes());
    refuse();
    assert!(
        !case.fleet.members[0]
            .root
            .0
            .join("unrequested.source")
            .exists()
    );
    private(&case.profile, original_profile.as_bytes());

    // Both inputs are well-formed native archives. The earlier registration
    // anchor still cannot be substituted for the source checkpoint's exact cut.
    let source_anchor = fs::read(&case.source_anchor).unwrap();
    private(
        &case.source_anchor,
        &fs::read(&case.fleet.members[0].anchor).unwrap(),
    );
    refuse();
    private(&case.source_anchor, &source_anchor);
    let custody = case.custody_bytes();
    let (result, json) = run(&case.fleet, "create", &[], || Ok(ElapsedTick(1001)));
    result.unwrap();
    assert!(flag(&json, "all_observed_drained"));
    case.assert_source_unchanged(&custody);
}
