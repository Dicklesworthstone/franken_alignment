//! Real retained native generation and socket review on the same durable owner.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::stream::generated::checked::source::{
    GeneratedSourceProfile, GeneratedSourcePublication,
};
use fa_reference::action::consequence::delivery::persistent::observed::source::FileSourcePolicy;
use fa_reference::action::consequence::oversight::policy_state::{
    StateFreshness, StateLimits, StateSource,
};
use std::collections::BTreeMap;

pub fn document(generation: u64, context: &[u8]) -> EvidenceSnapshot {
    EvidenceSnapshot::new(
        EvidenceIdentity {
            source: 51,
            generation,
            scope: native::profile().delivery.scope,
        },
        snapshot(),
        ["alpha", "beta"]
            .into_iter()
            .map(|member| (member.to_owned(), context.to_vec()))
            .collect(),
    )
    .unwrap()
}

pub fn replace(path: &std::path::Path, document: &EvidenceSnapshot) {
    let pending = path.with_extension("next");
    std::fs::write(&pending, document.encode()).unwrap();
    std::fs::rename(pending, path).unwrap();
}

pub struct Native {
    pub root: Directory,
    pub actor: FileGeneratedTextActorPort,
    pub driver: FileSupervisedDriver,
    pub reviewer: FileHumanReviewer,
    pub source: FileEvidenceSource,
    pub path: std::path::PathBuf,
    pub clients: helper::Clients,
    pub original: FileTextMessageRequest,
}

impl Native {
    pub fn new() -> Self {
        let root = Directory::new();
        let path = root.0.join("policy-evidence.json");
        replace(&path, &document(1, ORIGINAL_CONTEXT));
        let mut source = FileEvidenceSource::new(
            &path,
            51,
            native::profile().delivery.scope,
            MAX_EVIDENCE_FILE_BYTES,
        )
        .unwrap();
        let selected = GeneratedSourceProfile {
            source: FileSourcePolicy {
                source: StateSource {
                    scope: native::profile().delivery.scope,
                    source: 51,
                    generation: 1,
                },
                limits: StateLimits::default(),
                freshness: StateFreshness::new(100).unwrap(),
            },
            publication: GeneratedSourcePublication::Stream {
                stream: StreamProfile::new(9, 1, 4, 64, 256).unwrap(),
                reserve: Some(RecoveryReserve::terminal()),
            },
        };
        let (mut host, reviewer) = FileOversight::create_generated_text_stream_from_source(
            root.store(),
            native::profile(),
            native::decoder(),
            native::tokenizer(),
            selected,
        )
        .unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1))
            .unwrap();
        let (actor, mut supervisor) = host.into_generated_text_actor_gateway().unwrap();
        let original = native::generate(&mut supervisor);
        supervisor
            .prepare_file_intake(&mut source, || ElapsedTick(1))
            .result
            .unwrap();
        actor.submit(&original).unwrap();
        let driver = FileSupervisedDriver::new(supervisor);
        Self {
            root,
            actor,
            driver,
            reviewer,
            source,
            path,
            clients: BTreeMap::new(),
            original,
        }
    }

    pub fn held() -> Self {
        let mut case = Self::new();
        case.start(REQUEST, HELD_ROUND);
        case.complete(Verdict::Abstain, Consequence::HoldEffect);
        assert_eq!(case.driver.phase(), FileDriverPhase::Idle);
        assert_eq!(
            case.driver
                .supervisor()
                .host()
                .unwrap()
                .inspect()
                .executions,
            0
        );
        case
    }

    pub fn image(&self) -> Vec<u8> {
        std::fs::read(self.root.store().join("delivery.bin")).unwrap()
    }

    pub fn launch(&mut self, request: u64, round: u64) -> FileReviewLaunch<UnixStream> {
        let host = self.driver.supervisor().host().unwrap();
        let action = host.request_action(request).unwrap();
        let (workers, clients) = helper::sockets(action.spec().policy_epoch);
        self.clients = clients;
        FileReviewLaunch {
            request,
            round,
            window: helper::window(&host),
            expected_input_revision: host
                .input_revision(support::attempt(&host, request))
                .unwrap(),
            workers,
            limits: HelperLimits::default(),
        }
    }

    pub fn start(&mut self, request: u64, round: u64) {
        let launch = self.launch(request, round);
        self.driver
            .start_file_review(&mut self.source, launch, || ElapsedTick(1))
            .unwrap();
    }

    pub fn complete(&mut self, verdict: Verdict, consequence: Consequence) {
        for _ in 0..128 {
            let result = self
                .driver
                .step_from_file(&mut self.source, || ElapsedTick(1), None)
                .result
                .unwrap();
            match result {
                FileDriverEvent::Workers { .. } => helper::worker_steps(&mut self.clients, verdict),
                FileDriverEvent::ReviewApplied { receipt, .. } => {
                    assert_eq!(receipt.policy.control.decision.consequence, consequence);
                    return;
                }
                other => panic!("native fixture review did not finish: {other:?}"),
            }
        }
        panic!("native fixture exceeded its bounded socket review loop");
    }

    pub fn convene(&mut self, command: &FileConveneRequest) -> EvidenceIdentity {
        let ticket = self
            .driver
            .take_pending_convene(command.request())
            .unwrap()
            .unwrap();
        let launch = self.launch(command.request(), command.round());
        self.driver
            .start_file_convened_review(&mut self.source, ticket, launch, || ElapsedTick(1))
            .unwrap()
    }

    pub fn approve(&mut self, request: u64, key: u64) -> FileHumanPermit {
        assert!(matches!(
            self.driver.step_from_file(&mut self.source, || ElapsedTick(1), None).result,
            Ok(FileDriverEvent::AwaitingHuman { request: actual }) if actual == request
        ));
        let review = self
            .driver
            .request_human_approval_from_file(&mut self.source, key, ElapsedTick(31), || {
                ElapsedTick(1)
            })
            .result
            .unwrap();
        let mut host = self.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        self.reviewer.approve(&mut host, revision, &review).unwrap()
    }

    pub fn publish(&mut self, request: u64, key: &FileHumanPermit) {
        assert!(matches!(
            self.driver.step_from_file(&mut self.source, || ElapsedTick(1), Some(key)).result,
            Ok(FileDriverEvent::Dispatched { request: actual, .. }) if actual == request
        ));
        let publication = self
            .driver
            .step_from_file(&mut self.source, || ElapsedTick(1), None)
            .result
            .unwrap();
        assert!(matches!(publication,
            FileDriverEvent::PublicationChecked { publication, source_failure: None, .. }
                if matches!(publication.outcome, EndpointOutcome::Executed { .. })
        ));
        assert!(matches!(
            self.driver.step_from_file(&mut self.source, || ElapsedTick(1), None).result,
            Ok(FileDriverEvent::Reconciled { request: actual,
                outcome: Reconciliation::Resolved(EndpointOutcome::Executed { .. }) }) if actual == request
        ));
    }
}

pub fn command(port: &FileSupervisorConvenePort, request: u64, round: u64) -> FileConveneRequest {
    let Knowledge::Known { value, .. } = port.next(request, round).unwrap() else {
        panic!("expected an original generated held-review plan");
    };
    assert!(value.can_convene(), "{:?}", value.block_reason());
    value.convene_request().unwrap()
}

pub fn receipt(
    port: &FileSupervisorConvenePort,
    request: &FileConveneRequest,
) -> FileConveneReceipt {
    let Knowledge::Known { value, .. } = port.convene(*request).unwrap() else {
        panic!("expected original finite admission receipt");
    };
    value
}
