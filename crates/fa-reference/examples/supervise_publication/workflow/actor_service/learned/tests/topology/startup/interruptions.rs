//! Preserve the exact Fence cut on deadline refusal; never archive poisoned custody.
use super::*;

#[test]
fn late_recovery_startup_deadline_retains_the_acknowledged_pair_without_a_stop() {
    let root = Root::new();
    let (input, before) = first(&root, None);
    let original = input.read();
    let output = Archives::named(&root, "expired-startup");
    let config = configured(&root);
    let (_, loaded) = load_recovery(&root, &config, 2, &input, &output, 2);
    write_next(&root, 2);
    let canonical = config.store.join("delivery.bin");
    let before_image = fs::read(&canonical).unwrap();
    let mut expired = false;
    let mut control = Control::new(&config, loaded.request, None).unwrap();
    let result = custody::start(&config, &loaded, true, &mut control, &deadline(), &mut || {
        if fs::read(&canonical).unwrap() != before_image {
            expired = true;
            ElapsedTick(61_001)
        } else {
            ElapsedTick(1000)
        }
    });
    let error = match result {
        Ok(_) => panic!("the post-Fence deadline must refuse before source binding"),
        Err(error) => error,
    };
    assert!(expired);
    assert!(error.contains("deadline"), "{error}");
    assert!(!error.contains("retention failed"), "{error}");
    let after = observed(&config, &loaded);
    assert_unchanged_effects(&before, &after, 1, 2);
    assert_retained_cut(&output, &original, &after, false);
    assert_eq!(input.read(), original);
}

#[test]
fn failed_native_rebind_cannot_export_an_obsolete_pair_from_a_poisoned_owner() {
    let root = Root::new();
    let (input, before) = first(&root, None);
    let original = input.read();
    let output = Archives::named(&root, "poisoned-startup");
    let config = configured(&root);
    let (_, loaded) = load_recovery(&root, &config, 2, &input, &output, 2);
    write_next(&root, 2);
    let canonical = config.store.join("delivery.bin");
    let before_image = fs::read(&canonical).unwrap();
    let pending = config.store.join("delivery.pending");
    let mut recovered_image = None;
    let mut control = Control::new(&config, loaded.request, None).unwrap();
    let result = custody::start(&config, &loaded, true, &mut control, &deadline(), &mut || {
        let current = fs::read(&canonical).unwrap();
        if recovered_image.is_none() && current != before_image {
            // The Fence is acknowledged. Prevent the next native replacement
            // from staging without changing the retained canonical history.
            recovered_image = Some(current);
            fs::create_dir(&pending).unwrap();
        }
        ElapsedTick(1000)
    });
    let error = match result {
        Ok(_) => panic!("the native source rebind must fail at its staging boundary"),
        Err(error) => error,
    };
    assert!(error.contains("operation: Stage"), "{error}");
    assert!(error.contains("recovery history retention failed: Unavailable"), "{error}");
    assert!(error.contains("recovery topology retention failed: Unavailable"), "{error}");
    assert!(pending.is_dir());
    assert_eq!(fs::read(&canonical).unwrap(), recovered_image.unwrap());
    assert!(!output.source.exists());
    assert!(!output.history.exists());
    let after = observed(&config, &loaded);
    assert_unchanged_effects(&before, &after, 1, 2);
    assert_eq!(input.read(), original);
}
