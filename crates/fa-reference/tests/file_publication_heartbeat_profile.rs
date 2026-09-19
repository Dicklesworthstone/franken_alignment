//! Required heartbeat configuration must not disappear across bootstrap/reopen.
#![cfg(unix)]
#[path = "support/file_publication_capture.rs"] mod capture;
use capture::{Directory, profile};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::publication_gate::changes::PublicationChangePolicy;
use fa_reference::action::consequence::delivery::publication_gate::changes::freshness::PublicationFreshnessPolicy;
use fa_reference::witness::refinement::index::routing::RoutingBudget;
use fa_reference::Error;

fn changes() -> PublicationChangePolicy {
    PublicationChangePolicy { source: 41, after: 10, lookup: RoutingBudget { steps: 1000, bytes: 100_000 } }
}
fn freshness() -> PublicationFreshnessPolicy {
    PublicationFreshnessPolicy { clock_domain: profile().delivery.clock_domain, max_age_ticks: 3 }
}

#[test]
fn first_canonical_image_requires_the_whole_feed_profile_and_starts_without_eligibility() {
    let root = Directory::new();
    let (host, reviewer) = FileOversight::create_with_publication_change_freshness(
        root.store(), profile(), capture::limits(), changes(), freshness()).unwrap();
    assert_eq!(host.revision(), 3);
    assert_eq!(host.publication_validation_profile().unwrap(), Some(capture::limits()));
    assert_eq!(host.publication_change_status().unwrap().through, 10);
    assert_eq!(host.publication_change_freshness().unwrap().policy, freshness());
    assert_eq!(host.publication_change_freshness().unwrap().eligibility, Err(Error::Incomplete));
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), host.inspect());
    drop(reviewer); drop(host);
    let (host, _) = FileOversight::open_with_publication_change_freshness(
        root.store(), profile(), capture::limits(), changes(), freshness()).unwrap();
    assert_eq!(host.revision(), 4);
    assert!(!host.clock_ready());
    assert_eq!(host.publication_change_freshness().unwrap().eligibility, Err(Error::Incomplete));
}

#[test]
fn independently_pinned_limits_source_and_clock_age_refuse_without_recovery_writes() {
    let root = Directory::new();
    let (host, reviewer) = FileOversight::create_with_publication_change_freshness(
        root.store(), profile(), capture::limits(), changes(), freshness()).unwrap();
    let before = host.inspect(); drop(reviewer); drop(host);
    for change in 0..5 {
        let mut validation = capture::limits(); let mut source = changes(); let mut lease = freshness();
        match change {
            0 => validation.bindings -= 1,
            1 => source.source += 1,
            2 => source.after -= 1,
            3 => source.lookup.steps += 1,
            _ => lease.max_age_ticks += 1,
        }
        assert!(matches!(FileOversight::open_with_publication_change_freshness(
            root.store(), profile(), validation, source, lease), Err(JournalError::Contract(Error::Binding))));
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), before);
    }
}

#[test]
fn invalid_bootstrap_and_a_legacy_owner_cannot_masquerade_as_the_required_profile() {
    let root = Directory::new();
    let bad = PublicationFreshnessPolicy { clock_domain: freshness().clock_domain + 1, ..freshness() };
    assert!(matches!(FileOversight::create_with_publication_change_freshness(
        root.store(), profile(), capture::limits(), changes(), bad), Err(JournalError::Contract(Error::Binding))));
    assert!(!root.store().exists());
    let (host, reviewer) = FileOversight::create_with_publication_validation(root.store(), profile(), capture::limits()).unwrap();
    let before = host.inspect(); drop(reviewer); drop(host);
    assert!(matches!(FileOversight::open_with_publication_change_freshness(
        root.store(), profile(), capture::limits(), changes(), freshness()), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), before);
}
