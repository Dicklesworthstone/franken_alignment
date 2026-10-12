//! Exact private archive intake/export around the real learned operator owner.
//! These controls exercise custody boundaries, not custodian authentication.
use super::*;
use super::super::anchors as custody;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::anchored::{
    FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES,
};
use std::{path::{Path, PathBuf}, os::unix::fs::{PermissionsExt, symlink}};

fn args(mode: &[&str], input: Option<&Path>, output: Option<&Path>) -> Vec<String> {
    let mut args: Vec<_> = mode.iter().map(|value| (*value).to_owned()).collect();
    for (flag, path) in [("--history-anchor", input), ("--retain-history-anchor", output)] {
        if let Some(path) = path { args.extend([flag.into(), path.to_str().unwrap().into()]); }
    }
    args
}

fn admit(config: &Config, loaded: &mut Loaded, recovering: bool,
    input: Option<&Path>, output: Option<&Path>) -> Result<(), String>
{
    let (_, options) = custody::take_options(&args(
        &["create-learned-generated", "config", "recipe", "reviewer"], input, output))?;
    options.admit(config, loaded, recovering)
}

fn archive(path: &Path, anchor: &FileHistoryAnchor) -> Vec<u8> {
    let mut bytes = Vec::new();
    anchor.write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES).unwrap();
    fs::write(path, &bytes).unwrap();
    bytes
}

fn loaded(root: &Root) -> (Config, PathBuf, Loaded) {
    let config = configured(root);
    let recipe = fixture::write(root, b"allow");
    let loaded = recipe::load(&recipe, &config, false).unwrap();
    (config, recipe, loaded)
}

#[test]
fn history_options_preserve_every_original_mode_and_reject_duplicate_or_missing_operands() {
    let input = Path::new("/operator/previous.anchor");
    let output = Path::new("/operator/next.anchor");
    for mode in [
        vec!["create-learned-generated", "config", "recipe", "reviewer"],
        vec!["create-learned-generated", "config", "recipe", "reviewer", "--continue-generation"],
        vec!["create-learned-generated", "config", "recipe", "reviewer", "--after-message", "1"],
        vec!["create-learned-generated", "config", "recipe", "reviewer", "--finish-after-message", "1"],
        vec!["create-learned-generated", "config", "recipe", "--resume"],
        vec!["create-learned-generated", "config", "recipe", "--resume", "--finish-after-message", "1"],
    ] {
        let (remaining, _) = custody::take_options(&args(&mode, Some(input), Some(output))).unwrap();
        assert_eq!(remaining, mode);
        let (remaining, _) = super::super::continuation::take_option(&remaining).unwrap();
        assert!(super::super::finish::take_option(remaining).is_ok());
        for flag in ["--history-anchor", "--retain-history-anchor"] {
            let mut duplicate = args(&mode, Some(input), Some(output));
            duplicate.extend([flag.into(), input.to_str().unwrap().into()]);
            assert!(custody::take_options(&duplicate).is_err());
            let mut missing: Vec<_> = mode.iter().map(|value| (*value).to_owned()).collect();
            missing.push(flag.into());
            assert!(custody::take_options(&missing).is_err());
            missing.push("relative.anchor".into());
            assert!(custody::take_options(&missing).is_err());
        }
    }
}

#[test]
fn selected_anchor_is_never_defaulted_and_input_is_refused_on_initial_creation() {
    let root = Root::new(); let (config, _, mut loaded) = loaded(&root);
    let missing = root.0.join("missing.anchor");
    assert!(admit(&config, &mut loaded, false, Some(&missing), None).unwrap_err()
        .contains("only valid for recovery"));
    assert!(!config.store.exists());
    assert!(admit(&config, &mut loaded, true, Some(&missing), None).is_err());
    assert!(!config.store.exists());
    assert!(loaded.recovery_anchor.is_none() && loaded.anchor_output.is_none());
    // No selected options perform no anchor filesystem access, even with no store.
    assert!(admit(&config, &mut loaded, false, None, None).is_ok());
    assert!(!config.store.exists());
}

#[test]
fn private_anchor_loader_accepts_original_frame_and_refuses_truncation_trailing_and_oversize_files() {
    let root = Root::new(); let (config, _, loaded) = loaded(&root);
    let (host, _, _) = recovery::create_selected(&config, &loaded).unwrap();
    let anchor = host.history_anchor().unwrap();
    let path = root.0.join("original.anchor"); let bytes = archive(&path, &anchor);
    assert_eq!(custody::load(&path).unwrap(), anchor);
    let damaged = root.0.join("damaged.anchor");
    for cut in [0, 1, 8, 23, bytes.len() - 1] {
        fs::write(&damaged, &bytes[..cut]).unwrap();
        assert!(custody::load(&damaged).is_err());
    }
    let mut trailing = bytes.clone(); trailing.push(0); fs::write(&damaged, trailing).unwrap();
    assert!(custody::load(&damaged).is_err());
    let mut foreign = bytes.clone(); foreign[0] ^= 1; fs::write(&damaged, foreign).unwrap();
    assert!(custody::load(&damaged).is_err());
    let oversized = fs::File::create(&damaged).unwrap();
    oversized.set_len(MAX_HISTORY_ANCHOR_BYTES as u64 + 1).unwrap(); drop(oversized);
    assert!(custody::load(&damaged).is_err());
    let alias = root.0.join("alias.anchor"); symlink(&path, &alias).unwrap();
    assert!(custody::load(&alias).is_err());
    assert!(custody::load(&root.0).is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn anchor_output_admission_refuses_existing_files_dangling_links_and_store_parent_aliases() {
    let root = Root::new(); let (config, _, mut loaded) = loaded(&root);
    let path = root.0.join("existing.anchor"); fs::write(&path, b"keep old custody").unwrap();
    assert!(admit(&config, &mut loaded, false, None, Some(&path)).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"keep old custody"); assert!(!config.store.exists());
    let dangling = root.0.join("dangling.anchor"); symlink(root.0.join("absent"), &dangling).unwrap();
    assert!(admit(&config, &mut loaded, false, None, Some(&dangling)).is_err());
    assert!(fs::symlink_metadata(&dangling).unwrap().file_type().is_symlink());
    assert!(!config.store.exists());
    let (host, _, _) = recovery::create_selected(&config, &loaded).unwrap();
    let bytes = fs::read(config.store.join("delivery.bin")).unwrap();
    let alias = root.0.join("store-alias"); symlink(&config.store, &alias).unwrap();
    for selected in [config.store.join("new.anchor"), alias.join("new.anchor")] {
        assert!(admit(&config, &mut loaded, true, None, Some(&selected)).unwrap_err()
            .contains("outside the authority store"));
    }
    let in_store = config.store.join("inside.anchor"); archive(&in_store, &host.history_anchor().unwrap());
    assert!(admit(&config, &mut loaded, true, Some(&in_store), None).unwrap_err()
        .contains("outside the authority store"));
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), bytes);
    assert!(loaded.recovery_anchor.is_none() && loaded.anchor_output.is_none());
}

#[test]
fn confirmed_publication_retains_private_successor_and_receipt_resume_advances_it_without_live_work() {
    let root = Root::new();
    let (mut config, mut loaded, reviewer, driver, mut wire, deadline) = prepared(&root, b"allow");
    let original = driver.supervisor().host().unwrap().history_anchor().unwrap();
    let input = root.0.join("before-publication.anchor"); let old_bytes = archive(&input, &original);
    let output = root.0.join("confirmed.anchor");
    admit(&config, &mut loaded, true, Some(&input), Some(&output)).unwrap();
    let (mut driver, history) = super::continuation::review_current(&mut config, &loaded, &reviewer, driver, &deadline);
    let human = super::continuation::approve(&mut driver, &mut config, &reviewer, 1);
    super::continuation::publish(&mut driver, &mut config, &human); assert!(executed(&mut wire, 1));
    let current = driver.supervisor().host().unwrap().history_anchor_after(&original).unwrap();
    let state = driver.supervisor().host().unwrap().inspect(); let reads = config.source.status().read_attempts;
    let before = fs::read(config.store.join("delivery.bin")).unwrap();
    assert_eq!(custody::retain(&loaded, &driver, None), None);
    assert_eq!(custody::load(&output).unwrap(), current); assert!(current.revision() > original.revision());
    assert_eq!(fs::read(&input).unwrap(), old_bytes);
    assert_eq!(fs::metadata(&output).unwrap().permissions().mode() & 0o077, 0);
    assert_eq!(driver.supervisor().host().unwrap().inspect(), state);
    assert_eq!(config.source.status().read_attempts, reads);
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), before);
    let actor_response = response(&mut wire, 1).encode();
    assert!(!actor_response.windows(6).any(|bytes| bytes == b"FAHANC"));
    assert_eq!(cleanup(driver, 1000, 1), 0); drop(history);

    // Exercise the actual receipt-only finalization hook. These files are not
    // needed to replay the immutable numerical recipe or its retained receipt.
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    fs::remove_file(root.0.join("native-roster.json")).unwrap();
    let mut resumed = recipe::load(&root.0.join("recipe.json"), &config, false).unwrap();
    let next = root.0.join("receipt-recovered.anchor");
    admit(&config, &mut resumed, true, Some(&output), Some(&next)).unwrap();
    let store = config.store.clone(); let profile = config.profile.clone();
    let generation = resumed.generation.clone();
    let result = recovery::resume(config, resumed, || panic!("terminal receipt reads no live time")).unwrap();
    assert!(result.failure.is_none(), "{:?}", result.failure); assert_eq!(result.cleanup_pending, 0);
    assert!(matches!(result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    let retained = custody::load(&next).unwrap(); assert_eq!(retained.revision(), current.revision() + 1);
    let disk = FileOversight::read_publication_with_learned_generation(store, &profile, &generation).unwrap();
    assert_eq!(disk.executions, 1); assert_eq!(disk.payload, b"A");
    assert_eq!(fs::read(&input).unwrap(), old_bytes); assert_eq!(custody::load(&output).unwrap(), current);
}

#[test]
fn output_race_after_publication_preserves_competing_file_receipt_and_original_cleanup() {
    let root = Root::new();
    let (mut config, mut loaded, reviewer, driver, mut wire, deadline) = prepared(&root, b"allow");
    let input = root.0.join("before.anchor");
    let old = archive(&input, &driver.supervisor().host().unwrap().history_anchor().unwrap());
    let output = root.0.join("raced.anchor");
    admit(&config, &mut loaded, true, Some(&input), Some(&output)).unwrap();
    let (mut driver, history) = super::continuation::review_current(&mut config, &loaded, &reviewer, driver, &deadline);
    let human = super::continuation::approve(&mut driver, &mut config, &reviewer, 1);
    super::continuation::publish(&mut driver, &mut config, &human); assert!(executed(&mut wire, 1));
    let original = response(&mut wire, 1); let encoded = original.encode();
    let state = driver.supervisor().host().unwrap().inspect(); let reads = config.source.status().read_attempts;
    let before = fs::read(config.store.join("delivery.bin")).unwrap();
    fs::write(&output, b"different custodian won the path").unwrap();
    let failure = custody::retain(&loaded, &driver, Some("original output diagnosis".into()));
    let error = failure.as_deref().unwrap();
    assert!(error.starts_with("original output diagnosis; history anchor retention failed:"));
    assert!(error.contains("no effect retry was sent"));
    assert_eq!(fs::read(&output).unwrap(), b"different custodian won the path");
    assert_eq!(fs::read(&input).unwrap(), old);
    assert_eq!(driver.supervisor().host().unwrap().inspect(), state); assert_eq!(state.executions, 1);
    assert_eq!(config.source.status().read_attempts, reads);
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), before);
    let cleanup_pending = cleanup(driver, 1000, 1); assert_eq!(cleanup_pending, 0); drop(history);
    let result = RunResult { response: original, cleanup_pending, failure };
    let mut stdout = Vec::new(); assert!(emit(result, &mut stdout).is_err());
    assert_eq!(stdout, [encoded, vec![b'\n']].concat());
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), before);
}
