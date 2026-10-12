use super::*;
use crate::{Event, batch::BatchLimits};
use fa_reference::{Effect, State};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-reserve-fault-{}-{n}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn path(&self) -> PathBuf { self.0.join("journal") }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("reserve fault cleanup: {error}"); }
    }
}
fn identity() -> Identity { Identity { run: [1; 32], policy: [2; 32], total_units: 100 } }
fn limits() -> Limits { Limits { max_frame_bytes: 1024, max_records: 6, max_total_bytes: 1024 } }
fn reserve() -> RecoveryReserve { RecoveryReserve { records: 4, bytes: 120 } }
fn full(root: &Directory) -> FileJournal {
    let mut file = FileJournal::create_with_recovery_reserve(root.path(), identity(), limits(), reserve()).unwrap();
    let effect = Effect { principal: "actor".into(), resolved_target: "disposable".into(), payload: vec![1], units: 40 };
    file.commit_batch(&[Event::Reserve { id: 1, effect: effect.clone() }, Event::Dispatch { id: 1, effect }], BatchLimits::default()).unwrap();
    file
}

#[test]
fn every_interrupted_recovery_group_cut_preserves_capacity_spend_and_unknown_liability() {
    let tail = [Event::RevokeEpoch, Event::MarkUnknown { id: 1 }];
    // Fixed original record boundaries at 21 and 50, independent of profile.
    for cut in 0..=50 {
        let root = Directory::new(); let mut file = full(&root); let anchor = file.acknowledged_head();
        assert!(file.commit_batch_with(&tail, BatchLimits::default(), |file, bytes| {
            assert_eq!(bytes.len(), 50); file.write_all(&bytes[..cut])?;
            Err(io::Error::other("injected recovery write/sync failure"))
        }).is_err());
        assert!(file.is_poisoned()); assert!(matches!(file.capacity(), Err(Error::Poisoned)));
        assert!(matches!(file.rights(), Err(Error::Poisoned))); assert_eq!(file.acknowledged_head(), anchor);
        assert!(matches!(FileJournal::open_with_recovery_reserve(root.path(), identity(), anchor, limits(), reserve()), Err(Error::Busy)));
        drop(file); let bytes = std::fs::read(root.path()).unwrap();
        let recovered = FileJournal::open_with_recovery_reserve(root.path(), identity(), anchor, limits(), reserve());
        if [0, 21, 50].contains(&cut) {
            let mut recovered = recovered.unwrap();
            let records = if cut == 50 { 2 } else { u64::from(cut == 21) };
            assert_eq!(recovered.capacity().unwrap().ordinary.records, 0);
            assert_eq!(recovered.capacity().unwrap().recovery.records, 4 - records);
            assert_eq!(recovered.rights().unwrap().state(1), Ok(if cut == 50 { State::Unknown } else { State::Dispatched }));
            assert_eq!(recovered.rights().unwrap().available(), 60);
            recovered.commit(&Event::Reconcile { id: 1, occurred: true }).unwrap();
            assert_eq!(recovered.rights().unwrap().state(1), Ok(State::Committed));
        } else {
            assert!(recovered.is_err()); assert_eq!(std::fs::read(root.path()).unwrap(), bytes);
        }
    }
}

#[test]
fn recovery_write_unwind_poisoning_does_not_turn_the_reserve_into_a_refund_or_retry() {
    let root = Directory::new(); let mut file = full(&root); let anchor = file.acknowledged_head();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = file.commit_with(&Event::MarkUnknown { id: 1 }, |file, bytes| {
            file.write_all(bytes)?; panic!("after complete record, before acknowledgement");
        });
    }));
    assert!(result.is_err()); assert!(matches!(file.capacity(), Err(Error::Poisoned)));
    assert!(matches!(file.commit(&Event::Reconcile { id: 1, occurred: false }), Err(Error::Poisoned)));
    drop(file);
    let mut file = FileJournal::open_with_recovery_reserve(root.path(), identity(), anchor, limits(), reserve()).unwrap();
    assert_eq!(file.rights().unwrap().state(1), Ok(State::Unknown));
    assert_eq!(file.capacity().unwrap().recovery.records, 3);
    assert_eq!(file.rights().unwrap().available(), 60);
    assert!(file.commit_with(&Event::AbortBeforeDispatch { id: 1 }, |_, _| panic!("invalid recovery reached I/O")).is_err());
    assert!(!file.is_poisoned()); file.commit(&Event::Reconcile { id: 1, occurred: false }).unwrap();
    assert_eq!(file.rights().unwrap().available(), 100);
    assert_eq!(file.capacity().unwrap().ordinary.records, 0);
}
