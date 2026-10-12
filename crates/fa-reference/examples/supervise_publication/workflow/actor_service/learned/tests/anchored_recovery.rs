//! Actual loaded owners and separately retained original history archives.
//! Equal-counter forks are valid original histories, not malformed fixtures.
//! These controls do not authenticate custodian independence or remote storage.
use super::*;
use super::super::anchors;
use fa_reference::action::consequence::delivery::persistent::observed::{
    consistency::FileConsistencyObserver,
    guarded::anchored::MAX_HISTORY_ANCHOR_BYTES,
};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode { Legacy, Raw, Owned }
const MODES: [Mode; 3] = [Mode::Legacy, Mode::Raw, Mode::Owned];

fn configured_owner(root: &Root, mode: Mode) -> (Config, PathBuf, Loaded) {
    let config = configured(root);
    let path = super::continuation::write(root);
    if mode != Mode::Legacy {
        super::forecast::install(root, &path, mode == Mode::Owned, 60_000);
    }
    evidence(root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, false).unwrap();
    (config, path, loaded)
}

fn disk(config: &Config) -> Vec<u8> { fs::read(config.store.join("delivery.bin")).unwrap() }

fn archive(root: &Root, host: &FileOversight) -> PathBuf {
    let expected = host.history_anchor().unwrap();
    let mut bytes = Vec::new();
    expected.write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES).unwrap();
    let path = root.0.join("independent-history.anchor");
    fs::write(&path, bytes).unwrap();
    assert_eq!(anchors::load(&path).unwrap(), expected);
    path
}

fn fresh(host: &mut FileOversight, config: &mut Config, tick: u64) {
    host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(tick)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(tick)).unwrap();
}

fn prefill(host: &mut FileOversight, config: &mut Config) {
    fresh(host, config, 1000);
    let current = host.learned_generation_inspection().unwrap();
    assert!(!current.paused);
    assert_eq!(current.numerical.status, GenerationStatus::Prefilling);
    host.advance_learned_generation(host.revision(), current.numerical.actor_revision,
        current.numerical.position).unwrap().unwrap();
}

fn forecast(host: &mut FileOversight, observer: &FileConsistencyObserver, mode: Mode,
    request: u64) -> Result<Result<(), Error>, JournalError> {
    let numerical = host.learned_generation_inspection()?.numerical;
    let revision = host.revision();
    match mode {
        Mode::Raw => observer.forecast_hosted_request(host, revision, request, numerical.actor_revision)
            .map(|result| result.map(|_| ())),
        Mode::Owned => observer.forecast_owned_learned_request(host, revision, request, numerical.actor_revision)
            .map(|result| result.and_then(|report| report.prediction().map(|_| ()))),
        Mode::Legacy => panic!("legacy mode has no forecast observer"),
    }
}

#[test]
fn serialized_original_prefix_accepts_successors_and_returns_fresh_separate_custody() {
    for mode in MODES {
        let root = Root::new(); let (mut config, _, mut loaded) = configured_owner(&root, mode);
        let (mut host, _, old_observer) = recovery::create_selected(&config, &loaded).unwrap();
        prefill(&mut host, &mut config);
        let path = archive(&root, &host); let archive_bytes = fs::read(&path).unwrap();
        loaded.recovery_anchor = Some(anchors::load(&path).unwrap());
        let prefix_revision = loaded.recovery_anchor.as_ref().unwrap().revision();
        prefill(&mut host, &mut config);
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, 2); assert_eq!(numerical.sampled_draws, 0);
        assert_eq!(numerical.status, GenerationStatus::Generating);
        let before = host.inspect(); assert!(before.revision > prefix_revision);
        loaded.floor.journal_revision = before.revision;
        loaded.floor.control_sequence = before.control.sequence;
        loaded.floor.authority_epoch = before.control.ledger.epoch;
        let pending = config.store.join("delivery.pending");
        fs::write(&pending, b"retained until verified recovery").unwrap();
        drop(host);
        let (mut host, _, observer) = recovery::open_selected(&config, &loaded).unwrap();
        assert_eq!(host.revision(), before.revision + 1);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert!(!host.clock_ready()); assert!(!pending.exists());
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(observer.is_some(), mode != Mode::Legacy);
        assert_eq!(host.action_consistency_required(), mode != Mode::Legacy);
        fresh(&mut host, &mut config, 1001);
        host.resume_learned_generation(host.revision(), numerical.actor_revision, numerical.position).unwrap();
        if let Some(observer) = observer {
            let revision = host.revision(); let bytes = disk(&config);
            assert!(matches!(forecast(&mut host, old_observer.as_ref().unwrap(), mode, loaded.request),
                Err(JournalError::Contract(Error::Binding))));
            assert_eq!(host.revision(), revision); assert_eq!(disk(&config), bytes);
            forecast(&mut host, &observer, mode, loaded.request).unwrap().unwrap();
            assert_eq!(host.pending_forecast_request().unwrap(), Some(loaded.request));
            assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
            assert!(!host.action_consistency_snapshot().unwrap().coverage_lost);
        } else {
            host.advance_learned_generation(host.revision(), numerical.actor_revision, numerical.position)
                .unwrap().unwrap();
            assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 1);
        }
        assert_eq!(fs::read(path).unwrap(), archive_bytes);
    }
}

#[test]
fn valid_equal_counter_fork_refuses_selected_anchor_before_cleanup_or_fence() {
    for mode in MODES {
        let root = Root::new(); let (config, _, mut loaded) = configured_owner(&root, mode);
        let (host, _, _) = recovery::create_selected(&config, &loaded).unwrap();
        let bootstrap = disk(&config); drop(host);
        // Both branches use original creation/recovery/time transitions, at the
        // same path and with the same full independently loaded generation.
        let (mut left, _, _) = recovery::open_selected(&config, &loaded).unwrap();
        left.observe_time(left.revision(), ElapsedTick(1000)).unwrap();
        let path = archive(&root, &left); let archive_bytes = fs::read(&path).unwrap();
        let left_view = left.inspect(); let left_bytes = disk(&config); drop(left);
        fs::write(config.store.join("delivery.bin"), bootstrap).unwrap();
        let (mut right, _, _) = recovery::open_selected(&config, &loaded).unwrap();
        right.observe_time(right.revision(), ElapsedTick(1001)).unwrap();
        let right_view = right.inspect(); let right_bytes = disk(&config); drop(right);
        assert_eq!(right_view.revision, left_view.revision);
        assert_eq!(right_view.control.sequence, left_view.control.sequence);
        assert_eq!(right_view.control.ledger.epoch, left_view.control.ledger.epoch);
        assert_ne!(right_view.control.ledger.elapsed, left_view.control.ledger.elapsed);
        assert_ne!(right_bytes, left_bytes);
        loaded.floor.journal_revision = left_view.revision;
        loaded.floor.control_sequence = left_view.control.sequence;
        loaded.floor.authority_epoch = left_view.control.ledger.epoch;
        // The ordinary selected guard accepts this genuine alternate history;
        // the independent full prefix supplies the additional rejection.
        let (control, _, _) = recovery::open_selected(&config, &loaded).unwrap();
        assert_eq!(control.revision(), right_view.revision + 1); drop(control);
        fs::write(config.store.join("delivery.bin"), &right_bytes).unwrap();
        let pending = config.store.join("delivery.pending");
        fs::write(&pending, b"not a receipt").unwrap();
        loaded.recovery_anchor = Some(anchors::load(&path).unwrap());
        assert_eq!(recovery::open_selected(&config, &loaded).err().unwrap(), "Contract(Binding)");
        assert_eq!(disk(&config), right_bytes); assert_eq!(fs::read(&pending).unwrap(), b"not a receipt");
        assert_eq!(fs::read(&path).unwrap(), archive_bytes);
        fs::write(config.store.join("delivery.bin"), left_bytes).unwrap();
        let (host, _, observer) = recovery::open_selected(&config, &loaded).unwrap();
        assert_eq!(host.revision(), left_view.revision + 1); assert!(!pending.exists());
        assert_eq!(observer.is_some(), mode != Mode::Legacy);
        assert_eq!(fs::read(path).unwrap(), archive_bytes);
    }
}

#[test]
fn retained_anchor_rejects_rollback_even_with_zero_external_counter_floors() {
    for mode in MODES {
        let root = Root::new(); let (mut config, _, mut loaded) = configured_owner(&root, mode);
        let (mut host, _, _) = recovery::create_selected(&config, &loaded).unwrap();
        let old = disk(&config);
        prefill(&mut host, &mut config);
        let path = archive(&root, &host); let current = disk(&config);
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let revision = host.revision(); drop(host);
        assert_eq!(loaded.floor.journal_revision, 0);
        assert_eq!(loaded.floor.control_sequence, 0); assert_eq!(loaded.floor.authority_epoch, 0);
        fs::write(config.store.join("delivery.bin"), &old).unwrap();
        let (control, _, _) = recovery::open_selected(&config, &loaded).unwrap();
        assert!(control.revision() <= revision); drop(control);
        fs::write(config.store.join("delivery.bin"), &old).unwrap();
        let pending = config.store.join("delivery.pending");
        fs::write(&pending, b"rollback staging").unwrap();
        loaded.recovery_anchor = Some(anchors::load(&path).unwrap());
        assert_eq!(recovery::open_selected(&config, &loaded).err().unwrap(), "Contract(Stale)");
        assert_eq!(disk(&config), old); assert_eq!(fs::read(&pending).unwrap(), b"rollback staging");
        fs::write(config.store.join("delivery.bin"), current).unwrap();
        let (host, _, observer) = recovery::open_selected(&config, &loaded).unwrap();
        assert_eq!(host.revision(), revision + 1);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert_eq!(observer.is_some(), mode != Mode::Legacy); assert!(!pending.exists());
    }
}

#[test]
fn matching_history_anchor_cannot_replace_original_terminal_reserve_or_current_floor() {
    for mode in MODES {
        for case in 0..4 {
            let root = Root::new(); let (config, _, mut loaded) = configured_owner(&root, mode);
            let (host, _) = if case == 1 {
                FileOversight::create_with_learned_text_stream(&config.store,
                    config.profile.clone(), loaded.generation.clone()).unwrap()
            } else {
                let mut reserve = RecoveryReserve::terminal();
                if case == 2 { reserve.bytes += 1; }
                FileOversight::create_with_learned_text_stream_with_reserve(&config.store,
                    config.profile.clone(), loaded.generation.clone(), reserve).unwrap()
            };
            let path = archive(&root, &host); let revision = host.revision(); let bytes = disk(&config);
            drop(host);
            loaded.recovery_anchor = Some(anchors::load(&path).unwrap());
            if case == 3 { loaded.floor.journal_revision = revision + 1; }
            let pending = config.store.join("delivery.pending");
            fs::write(&pending, b"guarded staging").unwrap();
            let result = recovery::open_selected(&config, &loaded);
            if case == 0 {
                let (host, _, observer) = result.unwrap();
                assert_eq!(host.revision(), revision + 1); assert!(!pending.exists());
                assert_eq!(observer.is_some(), mode != Mode::Legacy);
            } else {
                assert_eq!(result.err().unwrap(), if case == 3 { "Contract(Stale)" } else { "Contract(Binding)" });
                assert_eq!(disk(&config), bytes); assert_eq!(fs::read(pending).unwrap(), b"guarded staging");
            }
        }
    }
}

#[test]
fn selected_anchor_does_not_admit_a_changed_predictor_or_recipe_downgrade() {
    for mode in [Mode::Raw, Mode::Owned] {
        let root = Root::new(); let (config, recipe_path, loaded) = configured_owner(&root, mode);
        let (host, _, _) = recovery::create_selected(&config, &loaded).unwrap();
        let path = archive(&root, &host); let bytes = disk(&config); let revision = host.revision(); drop(host);
        let forecast_path = root.0.join("forecast.json");
        let original_forecast = fs::read_to_string(&forecast_path).unwrap();
        assert!(original_forecast.contains("\"bias_bits\":0"));
        fs::write(&forecast_path, original_forecast.replacen("\"bias_bits\":0",
            "\"bias_bits\":1065353216", 1)).unwrap();
        let mut changed = recipe::load(&recipe_path, &config, false).unwrap();
        changed.recovery_anchor = Some(anchors::load(&path).unwrap());
        assert_ne!(changed.generation, loaded.generation);
        let pending = config.store.join("delivery.pending"); fs::write(&pending, b"immutable guard").unwrap();
        assert_eq!(recovery::open_selected(&config, &changed).err().unwrap(), "Contract(Binding)");
        assert_eq!(disk(&config), bytes); assert_eq!(fs::read(&pending).unwrap(), b"immutable guard");
        fs::write(&forecast_path, original_forecast).unwrap();
        let original_recipe = fs::read_to_string(&recipe_path).unwrap();
        fs::write(&recipe_path, original_recipe.replacen("fa.learned-publication/5", "fa.learned-publication/4", 1)
            .replacen(&format!("\"forecast\":\"{}\",", forecast_path.display()), "", 1)).unwrap();
        let mut downgraded = recipe::load(&recipe_path, &config, false).unwrap();
        assert!(downgraded.generation.required_pre_output_forecast().is_none());
        assert!(downgraded.generation.required_owned_pre_output_forecast().is_none());
        downgraded.recovery_anchor = Some(anchors::load(&path).unwrap());
        assert_eq!(recovery::open_selected(&config, &downgraded).err().unwrap(), "Contract(Binding)");
        assert_eq!(disk(&config), bytes); assert_eq!(fs::read(&pending).unwrap(), b"immutable guard");
        fs::write(&recipe_path, original_recipe).unwrap();
        let mut restored = recipe::load(&recipe_path, &config, false).unwrap();
        restored.recovery_anchor = Some(anchors::load(&path).unwrap());
        let (host, _, observer) = recovery::open_selected(&config, &restored).unwrap();
        assert_eq!(host.revision(), revision + 1); assert!(observer.is_some()); assert!(!pending.exists());
    }
}
