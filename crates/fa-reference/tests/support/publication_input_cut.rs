#![allow(dead_code)]
#[path = "file_publication_capture.rs"] pub mod base;
pub use base::{Directory, Keys, profile, snapshot};
use fa_reference::action::{ElapsedTick, FrozenAction};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanReviewer};
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::{FileCaptureIdentity, FilePublicationCapture};
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::heartbeat::feed::{PublicationFeedBatch, PublicationFeedFile};
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::FilePublicationInputs;
use fa_reference::action::consequence::delivery::publication_gate::changes::{PublicationChange, PublicationChangePolicy, PublicationInputCut};
use fa_reference::action::consequence::delivery::publication_gate::changes::freshness::{PublicationFreshnessPolicy, PublicationHeartbeat};
use fa_reference::action::consequence::oversight::CommitteeInput;
use fa_reference::product_frontier::ProjectionKey;
use fa_reference::witness::DomainProjection;
use fa_reference::witness::refinement::index::routing::{RoutingBudget, WitnessChange};

pub const FEED: u64 = 41;
pub fn cut(through: u64) -> PublicationInputCut { PublicationInputCut { source: FEED, through } }
pub fn notice(sequence: u64, key: u64) -> PublicationChange {
    PublicationChange { source: FEED, sequence, change: WitnessChange::Key {
        domain: DomainProjection::new(40, 1, ProjectionKey { source: 40, branch: 4, projection: 7, source_epoch: 1 }), key,
    } }
}
pub fn host(root: &Directory, freshness: bool) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = base::source_host(root);
    host.enable_publication_changes(host.revision(), PublicationChangePolicy {
        source: FEED, after: 0, lookup: RoutingBudget { steps: 10_000, bytes: 1_000_000 },
    }).unwrap();
    if freshness {
        host.enable_publication_change_freshness(host.revision(), PublicationFreshnessPolicy {
            clock_domain: profile().delivery.clock_domain, max_age_ticks: 100,
        }).unwrap();
        write_feed(root, 0);
        host.refresh_publication_feed(host.revision(), &feed(root), || ElapsedTick(1)).unwrap().unwrap();
    }
    (host, reviewer)
}
pub fn packet(action: &FrozenAction, inputs: &CommitteeInput, generation: u64,
    through: u64, keys: &[u64], opaque: bool) -> FilePublicationCapture
{
    let inputs = base::observations(inputs, generation, keys);
    let inputs = if opaque { inputs } else { FilePublicationInputs::new(inputs.structured().cloned(), None) };
    FilePublicationCapture::new_at_cut(1, FileCaptureIdentity { source: base::SOURCE, generation }, action, inputs, cut(through)).unwrap()
}
pub fn ready(root: &Directory, opaque: bool, freshness: bool) -> (FileOversight, FileHumanReviewer, Keys) {
    let (mut host, reviewer) = host(root, freshness);
    let (action, inputs) = base::reviewed(&mut host, 1, b"visible");
    let original = packet(&action, &inputs, 1, 0, &[0, 2, 4], opaque);
    base::replace_source(root, &original);
    host.bind_publication_file_source(host.revision(), 1, original, base::requests()).unwrap();
    base::refresh(&mut host, root, 1);
    let automatic = host.authorize(host.revision(), 1, &inputs, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), 1001, 1, &inputs, ElapsedTick(31)).unwrap();
    let revision = host.revision();
    let human = reviewer.approve(&mut host, revision, &request).unwrap();
    (host, reviewer, Keys { action, inputs, automatic, human, request })
}
pub fn feed(root: &Directory) -> PublicationFeedFile {
    PublicationFeedFile::new(root.0.join("cut-feed.bin"), FEED).unwrap()
}
pub fn write_feed(root: &Directory, through: u64) {
    let pulse = PublicationHeartbeat { source: FEED, clock_domain: profile().delivery.clock_domain,
        generation: through + 1, through, produced_at: ElapsedTick(1) };
    let batch = PublicationFeedBatch::new(pulse, 0, (1..=through).map(|sequence| {
        PublicationChange { source: FEED, sequence, change: WitnessChange::All }
    }).collect()).unwrap();
    let pending = root.0.join("cut-feed.next");
    std::fs::write(&pending, batch.to_bytes().unwrap()).unwrap();
    std::fs::rename(pending, root.0.join("cut-feed.bin")).unwrap();
}
