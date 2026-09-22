//! Custodian archives and restart/advancement through real canonical files.
use super::*;
use std::fs::{File, OpenOptions};
use std::io::Cursor;
use std::os::unix::fs::OpenOptionsExt;

#[test]
fn external_private_archive_survives_owner_drop_and_drives_guarded_recovery() {
    let root = Directory::new();
    let (host, expected) = setup(&root);
    let anchor = host.history_anchor().unwrap();
    let revision = anchor.revision();
    let archive = root.0.join("independent-custodian.anchor");
    let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600).open(&archive).unwrap();
    let size = anchor.write_to(&mut file, MAX_HISTORY_ANCHOR_BYTES).unwrap();
    file.sync_all().unwrap();
    assert_eq!(file.metadata().unwrap().len(), size as u64);
    drop(file);
    drop(anchor);
    drop(host);
    // The required prefix now comes from a separate real file, not an old
    // owner's event vector. This is an exclusive reopen, not a fresh-process
    // claim or a claim that this test directory provides independent trust.
    let restored = FileHistoryAnchor::read_trusted(&mut File::open(archive).unwrap(),
        MAX_HISTORY_ANCHOR_BYTES).unwrap();
    assert_eq!(restored.revision(), revision);
    let (host, _) = FileOversight::open_guarded_anchored(root.store(), profile(), &expected, &restored).unwrap();
    assert_eq!(host.revision(), revision + 1);
    assert!(!host.clock_ready());
    let advanced = host.history_anchor_after(&restored).unwrap();
    assert_eq!(advanced.revision(), host.revision());
    assert_eq!(host.history_anchor_after(&advanced).unwrap(), advanced);
    assert_eq!(restored.revision(), revision);
}

#[test]
fn framing_acceptance_cannot_launder_changed_journal_bytes_or_false_revision() {
    let root = Directory::new();
    let (host, expected) = setup(&root);
    let anchor = host.history_anchor().unwrap();
    let mut bytes = Vec::new();
    anchor.write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES).unwrap();
    let before = std::fs::read(root.canonical()).unwrap();
    drop(host);
    let mut altered = bytes.clone();
    *altered.last_mut().unwrap() ^= 1;
    let mut false_revision = bytes.clone();
    false_revision[8..16].copy_from_slice(&(anchor.revision() - 1).to_le_bytes());
    for frame in [altered, false_revision] {
        // A well-framed custodian object is NOT a validated original journal.
        let imported = FileHistoryAnchor::read_trusted(&mut Cursor::new(frame),
            MAX_HISTORY_ANCHOR_BYTES).unwrap();
        assert_eq!(FileOversight::open_guarded_anchored(root.store(), profile(), &expected, &imported)
            .unwrap_err(), JournalError::Contract(Error::Binding));
        assert_eq!(std::fs::read(root.canonical()).unwrap(), before);
    }
    let original = FileHistoryAnchor::read_trusted(&mut Cursor::new(bytes),
        MAX_HISTORY_ANCHOR_BYTES).unwrap();
    assert_eq!(original, anchor);
    assert!(FileOversight::open_guarded_anchored(root.store(), profile(), &expected, &original).is_ok());
}

#[test]
fn anchor_advancement_rejects_foreign_and_rolled_back_owners_without_writes() {
    let root = Directory::new();
    let other_root = Directory::new();
    let (mut host, expected) = setup(&root);
    let (other, _) = setup(&other_root);
    let previous = host.history_anchor().unwrap();
    let old_bytes = std::fs::read(root.canonical()).unwrap();
    assert_eq!(host.history_anchor_after(&previous).unwrap(), previous);
    assert_eq!(host.history_anchor_after(&other.history_anchor().unwrap()),
        Err(JournalError::Contract(Error::Binding)));
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let newer = host.history_anchor_after(&previous).unwrap();
    assert!(newer.revision() > previous.revision());
    assert_eq!(host.history_anchor_after(&newer).unwrap(), newer);
    drop(host);
    std::fs::write(root.canonical(), old_bytes).unwrap();
    // Intentionally select the weaker existing opener to construct an owner
    // whose history differs from what the external custodian already retained.
    let (mut rolled_back, _) = FileOversight::open_guarded(root.store(), profile(), &expected).unwrap();
    let before = std::fs::read(root.canonical()).unwrap();
    assert_eq!(rolled_back.history_anchor_after(&newer), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(std::fs::read(root.canonical()).unwrap(), before);
    // Its genuine old prefix remains demonstrably extendable; the refusal
    // above is about the custodian's newer prefix, not a universally broken API.
    assert!(rolled_back.history_anchor_after(&previous).is_ok());
    rolled_back.store.fail_once(JournalIo::Write);
    assert!(matches!(rolled_back.observe_time(rolled_back.revision(), ElapsedTick(2)),
        Err(JournalError::Io(_))));
    assert_eq!(rolled_back.history_anchor_after(&previous), Err(JournalError::Unavailable));
}
