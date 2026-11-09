//! Original durable/source owners verify cache sharing and independent quotas.
//! Public Tower tests separately exercise many-page transport and its bindings.
use super::super::{ElapsedTick, FileActorSupervisor, ReplayLimits};
use super::*;
use crate::action::consequence::gate::containment::session::policy::Predicate;
use crate::action::consequence::oversight::evidence_source::FileEvidenceSource;
use crate::action::{ResolvedTarget, Scope};

#[path = "../tests/fixture.rs"]
mod fixture;
use fixture::*;

fn limits() -> ReplayLimits {
    ReplayLimits {
        cases: 16,
        input_bytes: 1_048_576,
    }
}

fn candidate() -> Policy {
    Policy::new(2, vec![Predicate::PayloadAtMost(8)]).unwrap()
}

fn corpus(root: &Root) -> (FileActorSupervisor<FileOversight>, FileEvidenceSource) {
    let (mut host, source) = source_owner(root);
    host.submit_request(host.revision(), 901, spec(b"ok"), snapshot())
        .unwrap();
    host.submit_request(host.revision(), 902, spec(b"blocked"), snapshot())
        .unwrap();
    let (_, supervisor) = host.into_actor_gateway();
    (supervisor, source)
}

fn known(result: FilePolicyReplayPageResult) -> FilePolicyReplayPage {
    let Ok(Knowledge::Known { value, basis }) = result else {
        panic!("expected a page of the original native campaign: {result:?}");
    };
    assert_eq!(value.basis(), basis);
    value
}

#[test]
fn page_handles_share_one_original_report_and_once_computed_complete_summary() {
    let root = Root::new();
    let (supervisor, source) = corpus(&root);
    let (port, grant) = supervisor
        .policy_replay_port_with_pages(1, limits(), 3)
        .unwrap();
    let before = supervisor.host().unwrap().inspect();
    let files = root.files();
    let reads = source.status().read_attempts;
    std::fs::remove_file(root.source()).unwrap();
    let Ok(Knowledge::Known { value: whole, .. }) = port.replay_candidate_policy(9, 1, candidate())
    else {
        panic!("original native replay");
    };
    let retained = grant.retained_bytes();
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(grant.remaining_pages(), 3);
    let header = known(port.replay_candidate_policy_page(9, 1, candidate()));
    assert!(header.is_header() && header.cases().is_empty());
    assert_eq!(
        header.summary(),
        FilePolicyReplaySummary {
            case_count: 2,
            input_bytes: whole.report().input_bytes(),
            unchanged: 1,
            newly_blocked: 0,
            newly_reviewable: 1,
            requires_shadow_cases: 0,
            requires_shadow: false,
        }
    );
    assert!(std::ptr::eq(header.original().report(), whole.report()));
    let metadata = grant.retained_page_bytes();
    let cursor = header.continuation().unwrap();
    let page = known(port.continue_policy_replay(9, cursor));
    assert!(std::ptr::eq(page.original().report(), whole.report()));
    assert_eq!(page.cases(), whole.report().cases());
    assert_eq!(page.summary(), header.summary());
    assert!(page.enumeration_complete());
    assert!(page.continuation().is_none());
    assert_eq!(grant.remaining_pages(), 1);
    assert_eq!(grant.retained_bytes(), retained);
    assert!(grant.retained_page_bytes() > metadata);
    assert!(grant.retained_page_bytes() <= MAX_POLICY_REPLAY_PAGE_RETAINED_BYTES);
    let metadata = grant.retained_page_bytes();
    assert_eq!(
        known(port.replay_candidate_policy_page(9, 1, candidate())),
        header
    );
    assert_eq!(known(port.continue_policy_replay(9, cursor)), page);
    assert_eq!(grant.remaining_pages(), 1);
    assert_eq!(grant.retained_bytes(), retained);
    assert_eq!(grant.retained_page_bytes(), metadata);
    assert_eq!(source.status().read_attempts, reads);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), files);
}

#[test]
fn unavailable_and_stale_native_headers_are_cached_once_across_owner_advancement() {
    let root = Root::new();
    let (_, mut supervisor) = owner(&root).into_actor_gateway();
    let (port, grant) = supervisor
        .policy_replay_port_with_pages(2, limits(), 2)
        .unwrap();
    assert_eq!(
        port.replay_candidate_policy_page(1, 1, candidate()),
        unknown()
    );
    assert_eq!((grant.remaining_queries(), grant.remaining_pages()), (1, 1));
    let metadata = grant.retained_page_bytes();
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.submit_request(revision, 901, spec(b"ok"), snapshot())
            .unwrap();
    }
    assert_eq!(
        port.replay_candidate_policy_page(1, 1, candidate()),
        unknown()
    );
    assert_eq!((grant.remaining_queries(), grant.remaining_pages()), (1, 1));
    assert_eq!(grant.retained_page_bytes(), metadata);
    let successor = Policy::new(3, vec![Predicate::PayloadAtMost(8)]).unwrap();
    let stale = Ok(Knowledge::Stale {
        generation: 2,
        current: 1,
    });
    assert_eq!(
        port.replay_candidate_policy_page(2, 2, successor.clone()),
        stale
    );
    assert_eq!((grant.remaining_queries(), grant.remaining_pages()), (0, 0));
    let metadata = grant.retained_page_bytes();
    assert_eq!(port.replay_candidate_policy_page(2, 2, successor), stale);
    assert_eq!(grant.retained_page_bytes(), metadata);
    assert!(grant.retained_page_bytes() <= MAX_POLICY_REPLAY_PAGE_RETAINED_BYTES);
}

#[test]
fn complete_corpus_refusal_is_not_retried_by_header_or_native_api() {
    let root = Root::new();
    let (supervisor, _) = corpus(&root);
    let (port, grant) = supervisor
        .policy_replay_port_with_pages(
            1,
            ReplayLimits {
                cases: 1,
                ..limits()
            },
            1,
        )
        .unwrap();
    let before = supervisor.host().unwrap().inspect();
    assert_eq!(
        port.replay_candidate_policy_page(9, 1, candidate()),
        Err(ActorError::Capacity)
    );
    assert_eq!((grant.remaining_queries(), grant.remaining_pages()), (0, 0));
    let retained = (grant.retained_bytes(), grant.retained_page_bytes());
    assert_eq!(
        port.replay_candidate_policy_page(9, 1, candidate()),
        Err(ActorError::Capacity)
    );
    assert_eq!(
        port.replay_candidate_policy(9, 1, candidate()),
        Err(ActorError::Capacity)
    );
    assert_eq!(
        (grant.retained_bytes(), grant.retained_page_bytes()),
        retained
    );
    assert_eq!(supervisor.host().unwrap().inspect(), before);
}

#[test]
fn exhausted_page_admission_preserves_unused_native_campaigns_and_original_header() {
    let root = Root::new();
    let (supervisor, _) = corpus(&root);
    let (port, grant) = supervisor
        .policy_replay_port_with_pages(2, limits(), 1)
        .unwrap();
    let header = known(port.replay_candidate_policy_page(9, 1, candidate()));
    assert!(!header.enumeration_complete());
    assert!(header.continuation().is_none());
    assert_eq!((grant.remaining_queries(), grant.remaining_pages()), (1, 0));
    let retained = (grant.retained_bytes(), grant.retained_page_bytes());
    assert_eq!(
        port.replay_candidate_policy_page(10, 1, candidate()),
        Err(ActorError::Capacity)
    );
    assert_eq!((grant.remaining_queries(), grant.remaining_pages()), (1, 0));
    assert_eq!(
        (grant.retained_bytes(), grant.retained_page_bytes()),
        retained
    );
    assert_eq!(
        known(port.replay_candidate_policy_page(9, 1, candidate())),
        header
    );
    assert_eq!(
        port.replay_candidate_policy_page(
            9,
            1,
            Policy::new(2, vec![Predicate::PayloadAtMost(1)]).unwrap()
        ),
        Err(ActorError::IdempotencyConflict)
    );
    assert!(matches!(
        port.replay_candidate_policy(10, 1, candidate()),
        Ok(Knowledge::Known { .. })
    ));
    assert_eq!((grant.remaining_queries(), grant.remaining_pages()), (0, 0));
}

#[test]
fn summary_keeps_unobserved_boolean_arms_explicitly_shadow_required() {
    let root = Root::new();
    let (supervisor, _) = corpus(&root);
    let (port, grant) = supervisor
        .policy_replay_port_with_pages(1, limits(), 2)
        .unwrap();
    let candidate = Policy::new(
        2,
        vec![
            Predicate::PayloadAtMost(8),
            Predicate::Absent { key: 99 },
            Predicate::Any(vec![0, 1]),
        ],
    )
    .unwrap();
    let header = known(port.replay_candidate_policy_page(9, 1, candidate));
    assert_eq!(header.summary().requires_shadow_cases, 2);
    assert!(header.summary().requires_shadow);
    assert_eq!(header.summary().newly_reviewable, 0);
    let page = known(port.continue_policy_replay(9, header.continuation().unwrap()));
    assert_eq!(page.summary(), header.summary());
    assert!(
        page.cases()
            .iter()
            .all(|case| case.candidate().is_none() && case.missing_nodes() == [1])
    );
    assert_eq!(grant.remaining_pages(), 0);
}

#[test]
fn scalar_cursor_ids_are_process_unique_and_never_wrap_to_another_capture() {
    let root = Root::new();
    let (supervisor, _) = corpus(&root);
    let (first, first_grant) = supervisor
        .policy_replay_port_with_pages(1, limits(), 2)
        .unwrap();
    let (second, second_grant) = supervisor
        .policy_replay_port_with_pages(1, limits(), 2)
        .unwrap();
    let first_header = known(first.replay_candidate_policy_page(9, 1, candidate()));
    let second_header = known(second.replay_candidate_policy_page(9, 1, candidate()));
    assert_ne!(
        first_header.continuation().unwrap().id(),
        second_header.continuation().unwrap().id()
    );
    assert_eq!(
        second.continue_policy_replay(9, first_header.continuation().unwrap()),
        Err(ActorError::IdempotencyConflict)
    );
    assert_eq!(
        (
            first_grant.remaining_pages(),
            second_grant.remaining_pages()
        ),
        (1, 1)
    );
    let counter = AtomicU64::new(u64::MAX - 1);
    assert_eq!(next_cursor_id(&counter), Some(u64::MAX - 1));
    assert_eq!(next_cursor_id(&counter), None);
    assert_eq!(next_cursor_id(&counter), None);
    assert_eq!(next_cursor_id(&AtomicU64::new(0)), None);
}
