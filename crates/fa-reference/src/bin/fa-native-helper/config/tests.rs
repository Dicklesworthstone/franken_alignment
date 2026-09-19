use super::*;
#[path = "../../../../tests/support/native_worker_assets.rs"]
mod assets;

#[test]
fn independent_manifest_round_trips_full_profile_and_original_bytes() {
    let fixture = assets::Fixture::new(false);
    let manifest = Manifest::parse(fixture.manifest.as_bytes()).unwrap();
    assert_eq!(manifest.policy, fixture.policy);
    assert_eq!(manifest.stream, 12); assert_eq!(manifest.milliseconds, 10000);
    let request = assets::input(b"?");
    assert_eq!(request.actual_input().input_profile(), &manifest.policy.input_profile);
    assert!(assets::frame(b"?").len() > 9);
    assert_eq!(read_regular(&manifest.salt_file, MAX_WORKER_SALT_BYTES).unwrap(), assets::SALT);
}

#[test]
fn missing_unknown_duplicate_or_wrongly_typed_fields_are_not_defaulted() {
    let fixture = assets::Fixture::new(false);
    for (old, new) in [
        ("\"stream\":12", "\"stream\":\"12\""),
        ("\"stream\":12", "\"stream\":12,\"extra\":true"),
        ("\"stream\":12", "\"stream\":12,\"stream\":12"),
        ("\"steps\":10000", "\"stepz\":10000"),
        ("\"context\":1024", "\"context\":1024,\"unknown\":0"),
        ("fa.native-worker/1", "fa.native-worker/2"),
    ] {
        assert!(fixture.manifest.contains(old));
        assert!(Manifest::parse(fixture.manifest.replace(old, new).as_bytes()).is_err(), "{old}");
    }
}

#[test]
fn original_epoch_zero_and_non_utf8_profile_bytes_are_not_reinterpreted() {
    let fixture = assets::Fixture::new(false);
    let hex = fixture.policy.input_profile.profile_bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let raw = fixture.manifest.replace(&hex, "00ff80");
    assert_eq!(Manifest::parse(raw.as_bytes()).unwrap().policy.input_profile.profile_bytes, [0, 255, 128]);
    for invalid in ["0", "zz", "AA"] {
        assert!(Manifest::parse(fixture.manifest.replace(&hex, invalid).as_bytes()).is_err());
    }
}

#[test]
fn bounded_json_and_relative_or_control_character_paths_refuse() {
    let fixture = assets::Fixture::new(false);
    assert!(Manifest::parse(&vec![b' '; MAX_MANIFEST_BYTES + 1]).is_err());
    assert!(Manifest::parse(fixture.manifest[..fixture.manifest.len() - 1].as_bytes()).is_err());
    let name = fixture.root.join("salt.bin");
    for replacement in ["relative.bin", "bad\\u0000path"] {
        assert!(Manifest::parse(fixture.manifest.replace(name.to_str().unwrap(), replacement).as_bytes()).is_err());
    }
}

#[test]
fn real_file_limit_requires_eof_and_rejects_directories_and_symlinks() {
    let fixture = assets::Fixture::new(false);
    let path = fixture.root.join("bounded"); std::fs::write(&path, b"1234").unwrap();
    assert_eq!(read_regular(&path, 4).unwrap(), b"1234");
    assert!(matches!(read_regular(&path, 3), Err(LaunchError::Limit)));
    assert!(matches!(read_regular(&fixture.root, 100), Err(LaunchError::NotRegular)));
    let link = fixture.root.join("alias"); std::os::unix::fs::symlink(&path, &link).unwrap();
    assert!(matches!(read_regular(&link, 100), Err(LaunchError::NotRegular)));
}
