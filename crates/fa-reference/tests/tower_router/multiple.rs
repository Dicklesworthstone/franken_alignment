//! Multiple live runs retain their independent native authority and history.

use super::*;
use fa_reference::action::Scope;
use fa_reference::tower::router::MAX_PUBLICATION_MOUNTS;

struct Owner {
    // Keep the original supervisor alive until after the weak facades are used.
    supervisor: Supervisor,
    reviewer: FileHumanReviewer,
    port: Port,
    proposal: ActorProposal,
    source: FileEvidenceSource,
    captured: EvidenceSnapshot,
    scope: Scope,
    root: Directory,
}

impl Owner {
    fn new(run: u64, payload: &[u8]) -> Self {
        let mut scope = profile().delivery.scope;
        scope.run = run;
        Self::scoped(scope, payload)
    }

    fn scoped(scope: Scope, payload: &[u8]) -> Self {
        let root = Directory::new();
        let mut configured = profile();
        configured.delivery.scope = scope;
        let (mut host, reviewer) = FileOversight::create(root.store(), configured).unwrap();
        host.enable_file_source(
            host.revision(),
            FileSourcePolicy {
                source: StateSource {
                    scope,
                    source: 51,
                    generation: 1,
                },
                limits: StateLimits::default(),
                freshness: StateFreshness::new(50).unwrap(),
            },
        )
        .unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let captured = EvidenceSnapshot::new(
            EvidenceIdentity {
                source: 51,
                generation: 1,
                scope,
            },
            snapshot(),
            BTreeMap::from([
                ("alpha".to_owned(), SOURCE_BYTES.to_vec()),
                ("beta".to_owned(), SOURCE_BYTES.to_vec()),
            ]),
        )
        .unwrap();
        let path = root.0.join("source.json");
        std::fs::write(&path, captured.encode()).unwrap();
        let mut source = FileEvidenceSource::new(path, 51, scope, 1_048_576).unwrap();
        host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1))
            .unwrap();
        let state = host.inspect();
        let proposal = ActorProposal {
            target: state.target,
            payload: payload.to_vec(),
            units: 16,
            deadline: ElapsedTick(101),
            expected_policy_epoch: state.control.ledger.epoch,
        };
        let (port, supervisor) = host.into_actor_gateway();
        Self {
            supervisor,
            reviewer,
            port,
            proposal,
            source,
            captured,
            scope,
            root,
        }
    }

    fn facade(&self, generation: u64) -> PublicationTower {
        PublicationTower::new(
            Address::new(
                self.scope.tenant.to_string(),
                Noun::Run,
                self.scope.run.to_string(),
                Some(generation),
            )
            .unwrap(),
            self.port.clone(),
            PublicationLimits::default(),
        )
        .unwrap()
    }

    fn image(&self) -> Vec<u8> {
        canonical(&self.root)
    }

    fn ready(&mut self, request: u64) -> oversight::Keys {
        let mut host = self.supervisor.host_mut().unwrap();
        let id = support::attempt(&host, request);
        let action = host.request_action(request).unwrap().clone();
        assert_eq!(action.spec().scope, self.scope);
        let inputs = self
            .captured
            .inputs_for(&action, &profile().committee)
            .unwrap();
        let root = self.captured.reference_root();
        let revision = host.revision();
        let input_revision = host.input_revision(id).unwrap();
        host.record_inputs(revision, id, input_revision, inputs.clone())
            .unwrap();
        let round = request + 100;
        let revision = host.revision();
        let window = oversight::window(&host);
        host.begin_review(revision, id, round, root, window, snapshot())
            .unwrap();
        for member in oversight::MEMBERS {
            let digest = commitment(
                round,
                member,
                &root,
                Verdict::Allow,
                &oversight::salt(member),
            )
            .unwrap();
            let revision = host.revision();
            host.commit_review(revision, round, member, digest).unwrap();
        }
        let revision = host.revision();
        host.open_reveals(revision, round).unwrap();
        for member in oversight::MEMBERS {
            let revision = host.revision();
            host.reveal_review(
                revision,
                round,
                member,
                Verdict::Allow,
                oversight::salt(member),
            )
            .unwrap();
        }
        let revision = host.revision();
        host.finish_review(revision, round, Some(&inputs), snapshot())
            .unwrap()
            .unwrap();
        let revision = host.revision();
        let automatic = host.authorize(revision, id, &inputs, snapshot()).unwrap();
        let revision = host.revision();
        let human_request = host
            .request_human_approval(revision, request + 1000, id, &inputs, ElapsedTick(31))
            .unwrap();
        let revision = host.revision();
        let human = self
            .reviewer
            .approve(&mut host, revision, &human_request)
            .unwrap();
        oversight::Keys {
            action,
            inputs,
            automatic,
            human,
            request: human_request,
        }
    }

    fn publish_and_reconcile(&mut self, keys: &oversight::Keys) {
        let mut host = self.supervisor.host_mut().unwrap();
        let id = keys.automatic.attempt();
        oversight::dispatch(&mut host, keys);
        let revision = host.revision();
        let capture = host
            .refresh_file_source(revision, &mut self.source, ElapsedTick(2))
            .unwrap();
        let inputs = capture
            .inputs_for(&keys.action, &profile().committee)
            .unwrap();
        let revision = host.revision();
        let result = host
            .publish_checked(
                revision,
                id,
                Some(&inputs),
                capture.snapshot().clone(),
                ElapsedTick(3),
            )
            .unwrap();
        assert_eq!(result.basis, PublicationBasis::Revalidated);
        assert_eq!(host.inspect().payload, self.proposal.payload);
        assert_eq!(host.inspect().executions, 1);
        let revision = host.revision();
        assert!(matches!(
            host.reconcile(revision, id).unwrap(),
            Reconciliation::Resolved(_)
        ));
    }
}

#[test]
fn two_original_runs_publish_the_same_request_key_without_sharing_tickets_or_ledgers() {
    let mut first = Owner::new(3, b"first-payload");
    let mut second = Owner::new(30, b"second-payload");
    let a = first.facade(41);
    let b = second.facade(42);
    let a_mount = a.mount_address().clone();
    let b_mount = b.mount_address().clone();
    let a_request = a.request_address(70).unwrap();
    let b_request = b.request_address(70).unwrap();
    assert_ne!(a_request, b_request);
    let decoys = [
        a_mount.clone(),
        b_mount.clone(),
        a_request.clone(),
        b_request.clone(),
    ];
    let mut router =
        TowerRouter::with_publications(imported(TENANT, &decoys, 1), vec![b, a]).unwrap();
    observe(&mut first.supervisor);
    observe(&mut second.supervisor);
    let before_second = second.image();
    let first_call = submit(&a_mount, 70, &first.proposal);
    assert_eq!(
        request_result(&router.dispatch(&access(), first_call.clone())).result,
        Ok(ActorKnowledge::Pending { request: 70 })
    );
    let first_ticket = first.port.submit(70, &first.proposal).unwrap();
    // Matching numeric request IDs cannot acquire the other owner's ticket.
    let missing_second = router.dispatch(&reader(), get(&b_request));
    assert_eq!(
        request_result(&missing_second).result,
        Ok(ActorKnowledge::Withheld {
            authority_required: "own_request"
        })
    );
    assert_eq!(second.image(), before_second);
    assert_eq!(second.supervisor.host().unwrap().retained_requests(), 0);
    let before_first = first.image();
    let second_call = submit(&b_mount, 70, &second.proposal);
    assert_eq!(
        request_result(&router.dispatch(&access(), second_call.clone())).result,
        Ok(ActorKnowledge::Pending { request: 70 })
    );
    let second_ticket = second.port.submit(70, &second.proposal).unwrap();
    assert_eq!(first.image(), before_first);

    let first_keys = first.ready(70);
    let second_before_publish = second.image();
    first.publish_and_reconcile(&first_keys);
    assert_eq!(second.image(), second_before_publish);
    assert_eq!(
        request_result(&router.dispatch(&reader(), get(&a_request))).result,
        Ok(first.port.poll(&first_ticket))
    );
    assert!(matches!(
        request_result(&router.dispatch(&reader(), get(&a_request))).result,
        Ok(ActorKnowledge::Known {
            value: ActorOutcome::Executed,
            ..
        })
    ));
    assert_eq!(
        request_result(&router.dispatch(&reader(), get(&b_request))).result,
        Ok(second.port.poll(&second_ticket))
    );
    assert!(!matches!(
        second.port.poll(&second_ticket),
        ActorKnowledge::Known { .. }
    ));

    let first_final = first.image();
    let second_keys = second.ready(70);
    second.publish_and_reconcile(&second_keys);
    assert_eq!(first.image(), first_final);
    assert!(matches!(
        request_result(&router.dispatch(&reader(), get(&b_request))).result,
        Ok(ActorKnowledge::Known {
            value: ActorOutcome::Executed,
            ..
        })
    ));
    let second_final = second.image();
    for (call, original) in [
        (first_call, first.port.poll(&first_ticket)),
        (second_call, second.port.poll(&second_ticket)),
    ] {
        let reply = router.dispatch(&access(), call);
        assert_eq!(request_result(&reply).result, Ok(original));
        assert_native_json(&reply);
    }
    assert_eq!(first.image(), first_final);
    assert_eq!(second.image(), second_final);
    known_evidence(
        &router.dispatch(&operator(TENANT), get(&evidence(TENANT))),
        TENANT,
    );
    assert_eq!(
        router
            .dispatch(&operator(TENANT), get(&evidence(TENANT)))
            .reason_code(),
        Some(ReasonCode::BudgetExhausted)
    );
}

#[test]
fn explicit_registration_navigation_never_chooses_an_implicit_first_owner() {
    let first = Owner::new(3, b"a");
    let second = Owner::new(30, b"b");
    let a = first.facade(41);
    let b = second.facade(42);
    let a_mount = a.mount_address().clone();
    let b_mount = b.mount_address().clone();
    let a_request = a.request_address(70).unwrap();
    let b_request = b.request_address(70).unwrap();
    let images = (first.image(), second.image());
    let router = TowerRouter::with_publications(imported(TENANT, &[], 1), vec![b, a]).unwrap();
    assert_eq!(
        router.publication_addresses().cloned().collect::<Vec<_>>(),
        vec![a_mount.clone(), b_mount.clone()]
    );
    assert!(router.publication_address().is_none());
    assert_eq!(
        router.publication_request_address(70),
        Err(TowerInstallError::InvalidAddress)
    );
    assert_eq!(
        router.publication_request_address_at(&a_mount, 70),
        Ok(a_request)
    );
    assert_eq!(
        router.publication_request_address_at(&b_mount, 70),
        Ok(b_request)
    );
    assert_eq!(
        router.publication_request_address_at(&a_mount, 0),
        Err(TowerInstallError::InvalidAddress)
    );
    for generation in [None, Some(0), Some(42), Some(99)] {
        let mut changed = a_mount.clone();
        changed.generation = generation;
        assert_eq!(
            router.publication_request_address_at(&changed, 70),
            Err(TowerInstallError::SnapshotMismatch)
        );
    }
    let mut foreign = a_mount.clone();
    foreign.tenant = "2".into();
    assert_eq!(
        router.publication_request_address_at(&foreign, 70),
        Err(TowerInstallError::MissingSource)
    );
    foreign = a_mount;
    foreign.id = "300".into();
    assert_eq!(
        router.publication_request_address_at(&foreign, 70),
        Err(TowerInstallError::MissingSource)
    );
    assert_eq!((first.image(), second.image()), images);
}

#[test]
fn registration_rejects_ambiguous_runs_foreign_tenants_and_excess_mounts_before_native_work() {
    let mut first = Owner::new(3, b"a");
    let second = Owner::new(30, b"b");
    let mut aliased_scope = first.scope;
    aliased_scope.principal += 1;
    aliased_scope.branch += 1;
    aliased_scope.authority += 1;
    let alias = Owner::scoped(aliased_scope, b"alias");
    let mut foreign_scope = first.scope;
    foreign_scope.tenant += 1;
    let foreign = Owner::scoped(foreign_scope, b"foreign");
    observe(&mut first.supervisor);
    let images = (
        first.image(),
        second.image(),
        alias.image(),
        foreign.image(),
    );
    for mounts in [
        vec![first.facade(41), first.facade(41)],
        vec![first.facade(41), first.facade(42)],
        vec![first.facade(41), alias.facade(42)],
        vec![alias.facade(42), second.facade(42), first.facade(41)],
    ] {
        assert_eq!(
            TowerRouter::with_publications(imported(TENANT, &[], 1), mounts).err(),
            Some(TowerInstallError::DuplicateObject)
        );
    }
    assert_eq!(
        TowerRouter::with_publications(
            imported(TENANT, &[], 1),
            vec![first.facade(41), foreign.facade(42)]
        )
        .err(),
        Some(TowerInstallError::SnapshotMismatch)
    );
    assert_eq!(
        TowerRouter::with_publications(branch(), vec![first.facade(41), second.facade(42)]).err(),
        Some(TowerInstallError::SnapshotMismatch)
    );
    assert_eq!(
        TowerRouter::with_publications(
            imported(TENANT, &[], 1),
            (0..=MAX_PUBLICATION_MOUNTS)
                .map(|_| first.facade(41))
                .collect()
        )
        .err(),
        Some(TowerInstallError::LimitExceeded)
    );
    assert_eq!(
        (
            first.image(),
            second.image(),
            alias.image(),
            foreign.image()
        ),
        images
    );
    assert_eq!(first.supervisor.host().unwrap().retained_requests(), 0);
    // Failed setup did not spend or revoke the independently retained port or
    // the original one-use admission observation.
    let facade = first.facade(41);
    let mount = facade.mount_address().clone();
    let mut router =
        TowerRouter::with_publications(imported(TENANT, &[], 1), vec![facade]).unwrap();
    assert_eq!(router.publication_address(), Some(&mount));
    assert_eq!(
        router.publication_request_address(70),
        router.publication_request_address_at(&mount, 70)
    );
    assert_eq!(
        request_result(&router.dispatch(&access(), submit(&mount, 70, &first.proposal))).result,
        Ok(ActorKnowledge::Pending { request: 70 })
    );
    let mut empty = TowerRouter::with_publications(branch(), Vec::new()).unwrap();
    assert_eq!(empty.publication_addresses().len(), 0);
    known_evidence(
        &empty.dispatch(&operator(TENANT), get(&evidence(TENANT))),
        TENANT,
    );
}

#[test]
fn the_exact_mount_bound_retains_each_original_registration() {
    let owners = (0..MAX_PUBLICATION_MOUNTS)
        .map(|i| Owner::new(100 + i as u64, b"bounded"))
        .collect::<Vec<_>>();
    let mounts = owners.iter().map(|owner| owner.facade(41)).collect();
    let router = TowerRouter::with_publications(imported(TENANT, &[], 1), mounts).unwrap();
    assert_eq!(router.publication_addresses().len(), MAX_PUBLICATION_MOUNTS);
    for owner in &owners {
        let facade = owner.facade(41);
        assert_eq!(
            router.publication_request_address_at(facade.mount_address(), 1),
            facade.request_address(1)
        );
        assert_eq!(owner.supervisor.host().unwrap().retained_requests(), 0);
    }
}

#[test]
fn independent_disclosure_budgets_and_cursor_namespaces_survive_cross_run_queries() {
    let first = Owner::new(3, b"a");
    let second = Owner::new(30, b"b");
    let mut a = first.facade(41);
    let mut b = second.facade(42);
    let a_mount = a.mount_address().clone();
    let b_mount = b.mount_address().clone();
    let (precheck_a, grant_a) = first
        .supervisor
        .disclose_exact_precheck(first.supervisor.host().unwrap().revision(), DISCLOSURE, 1)
        .unwrap();
    let (precheck_b, grant_b) = second
        .supervisor
        .disclose_exact_precheck(second.supervisor.host().unwrap().revision(), DISCLOSURE, 2)
        .unwrap();
    a.set_precheck(Some(precheck_a)).unwrap();
    b.set_precheck(Some(precheck_b)).unwrap();
    let (tail_a, tail_grant_a) = first.supervisor.journal_tail_port(4, 1).unwrap();
    let (tail_b, tail_grant_b) = second.supervisor.journal_tail_port(4, 1).unwrap();
    a.set_journal_tail(Some(tail_a)).unwrap();
    b.set_journal_tail(Some(tail_b)).unwrap();
    let mut router = TowerRouter::with_publications(imported(TENANT, &[], 1), vec![a, b]).unwrap();
    let images = (first.image(), second.image());
    for (mount, proposal) in [(&a_mount, &first.proposal), (&b_mount, &second.proposal)] {
        assert!(matches!(
            precheck_result(&router.dispatch(&reader(), precheck(mount, 1, proposal))).result,
            Ok(ActorKnowledge::Known {
                value: FilePrecheckOutcome::PrecheckPassed,
                ..
            })
        ));
    }
    assert_eq!(
        (grant_a.remaining_queries(), grant_b.remaining_queries()),
        (0, 1)
    );
    assert_eq!(
        router
            .dispatch(&reader(), precheck(&a_mount, 2, &first.proposal))
            .reason_code(),
        Some(ReasonCode::BudgetExhausted)
    );
    assert!(matches!(
        precheck_result(&router.dispatch(&reader(), precheck(&b_mount, 2, &second.proposal)))
            .result,
        Ok(ActorKnowledge::Known {
            value: FilePrecheckOutcome::PrecheckPassed,
            ..
        })
    ));
    let cursor = |reply: &RoutedResponse| {
        let PublicationResult::JournalTail(linked) = &live(reply).result else {
            panic!("native tail");
        };
        let Ok(ActorKnowledge::Known { value, .. }) = linked.native() else {
            panic!("known page");
        };
        value.continuation().unwrap().id()
    };
    let mut tail = |mount: &Address, arguments| {
        let mut call = TowerRequest::new("tail", mount.clone(), Verb::Tail);
        call.arguments = arguments;
        router.dispatch(&operator(TENANT), call)
    };
    let a_cursor = cursor(&tail(
        &a_mount,
        ReadArguments::JournalTail { after_revision: 0 },
    ));
    let b_cursor = cursor(&tail(
        &b_mount,
        ReadArguments::JournalTail { after_revision: 0 },
    ));
    assert_ne!(a_cursor, b_cursor);
    assert_eq!(
        tail(
            &b_mount,
            ReadArguments::JournalTailPage { cursor: a_cursor }
        )
        .reason_code(),
        Some(ReasonCode::IntegrityFailure)
    );
    // Foreign cursor selection does not consume either legitimate continuation.
    assert_eq!(
        (
            tail_grant_a.remaining_queries(),
            tail_grant_b.remaining_queries()
        ),
        (3, 3)
    );
    assert!(
        tail(
            &a_mount,
            ReadArguments::JournalTailPage { cursor: a_cursor }
        )
        .reason_code()
        .is_none()
    );
    assert!(
        tail(
            &b_mount,
            ReadArguments::JournalTailPage { cursor: b_cursor }
        )
        .reason_code()
        .is_none()
    );
    assert_eq!(
        (
            tail_grant_a.remaining_queries(),
            tail_grant_b.remaining_queries()
        ),
        (2, 2)
    );
    assert_eq!((first.image(), second.image()), images);
    assert_eq!(first.supervisor.host().unwrap().retained_requests(), 0);
    assert_eq!(second.supervisor.host().unwrap().retained_requests(), 0);
}

#[test]
fn unavailable_owner_never_falls_through_to_a_sibling_or_imported_decoy() {
    let mut first = Owner::new(3, b"a");
    let mut second = Owner::new(30, b"b");
    let a = first.facade(41);
    let b = second.facade(42);
    let a_mount = a.mount_address().clone();
    let b_mount = b.mount_address().clone();
    let a_request = a.request_address(70).unwrap();
    let b_request = b.request_address(70).unwrap();
    let decoys = [a_mount.clone(), a_request.clone(), b_request.clone()];
    let mut router =
        TowerRouter::with_publications(imported(TENANT, &decoys, 1), vec![a, b]).unwrap();
    observe(&mut first.supervisor);
    observe(&mut second.supervisor);
    assert_eq!(
        request_result(&router.dispatch(&access(), submit(&a_mount, 70, &first.proposal))).result,
        Ok(ActorKnowledge::Pending { request: 70 })
    );
    let second_before = second.image();
    let mut stale_second = b_mount.clone();
    stale_second.generation = a_mount.generation;
    assert_eq!(
        router
            .dispatch(&access(), submit(&stale_second, 70, &second.proposal))
            .reason_code(),
        Some(ReasonCode::StaleGeneration)
    );
    let mut stale = a_mount.clone();
    stale.generation = b_mount.generation;
    assert_eq!(
        router
            .dispatch(&access(), submit(&stale, 70, &first.proposal))
            .reason_code(),
        Some(ReasonCode::StaleGeneration)
    );
    let call = submit(&a_mount, 70, &first.proposal);
    drop(first);
    for request in [get(&a_request), call] {
        let response = router.dispatch(&access(), request);
        assert_eq!(
            response.reason_code(),
            Some(ReasonCode::LowerLayerUnverified)
        );
        let _ = live(&response);
    }
    assert_eq!(second.image(), second_before);
    assert_eq!(second.supervisor.host().unwrap().retained_requests(), 0);
    assert_eq!(
        request_result(&router.dispatch(&access(), submit(&b_mount, 70, &second.proposal))).result,
        Ok(ActorKnowledge::Pending { request: 70 })
    );
    let reserved = Address::new(TENANT, Noun::Attempt, "request-3-2-4-5-invalid", None).unwrap();
    let foreign = Address::new(TENANT, Noun::Attempt, "request-3-99-4-5-70", None).unwrap();
    let _ = live(&router.dispatch(&reader(), get(&reserved)));
    let unavailable = router.dispatch(&reader(), get(&foreign));
    assert_eq!(
        unavailable.reason_code(),
        Some(ReasonCode::MissingRequiredObservation)
    );
    let _ = reference(&unavailable);
    known_evidence(
        &router.dispatch(&operator(TENANT), get(&evidence(TENANT))),
        TENANT,
    );
}
