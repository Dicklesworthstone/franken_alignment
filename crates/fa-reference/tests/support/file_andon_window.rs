#![allow(dead_code)]
//! Original ordinary evaluation/Andon owner for finite live registration tests.
//! Synthetic helper verdicts exercise enforcement, not model-quality claims.
#[path = "file_credibility_promotion.rs"]
mod original;
pub use original::{Directory, assess, begin, finish, profile, protocol, review, snapshot};

use fa_reference::action::consequence::delivery::persistent::{
    RecoveryReserve,
    observed::{
        FileHumanReviewer, FileOversight,
        credibility::{FileIndependentEvaluator, andon::FileAndonConfig},
    },
};
use fa_reference::round::review::andon::{AndonBudget, LedgerIdentity};

pub fn identity() -> LedgerIdentity {
    LedgerIdentity {
        deployment: 91,
        evaluator: 92,
    }
}
pub fn initial_window() -> AndonBudget {
    AndonBudget {
        window: 1,
        start: 0,
        end: 10,
        false_stops_allowed: 1,
        cost_per_stop: 7,
    }
}
pub fn later_window() -> AndonBudget {
    AndonBudget {
        window: 2,
        start: 10,
        end: 30,
        false_stops_allowed: 1,
        cost_per_stop: u64::MAX,
    }
}
pub fn config() -> FileAndonConfig {
    FileAndonConfig::new(identity(), vec![initial_window()]).unwrap()
}

/// No time has been observed. review(host, 3, Hold, Allow) later observes 12
/// and creates original applied case 103 outside the sole bootstrap window.
pub fn ready() -> (
    Directory,
    FileOversight,
    FileHumanReviewer,
    FileIndependentEvaluator,
) {
    let root = Directory::new();
    let (mut host, human) = FileOversight::create(&root.0, profile(0)).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal())
        .unwrap();
    let evaluator = host
        .enable_credibility(host.revision(), protocol())
        .unwrap();
    host.enable_andon(host.revision(), &config()).unwrap();
    (root, host, human, evaluator)
}
