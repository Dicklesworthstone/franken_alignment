//! Genuine mediated numerical history, retained before and after native source work.
//! These artifacts exercise recovery custody and control behavior, not deployment
//! inventory completeness or forecast quality.
use super::super::super::super::{anchors, topology as custody};
use super::super::super::{continuation as native, mediated_topology};
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::{
    driver::topology::checkpoint::{MAX_TOPOLOGY_CHECKPOINT_BYTES, TopologyCheckpoint},
    mediation::source::TOPOLOGY_REFRESH_READ_BYTES,
};
use std::{
    collections::BTreeMap,
    io::Cursor,
    path::{Path, PathBuf},
};

pub(super) struct Case {
    pub fleet: Fleet,
    pub profile: PathBuf,
    pub source_anchor: PathBuf,
    pub checkpoint: PathBuf,
    pub saved: TopologyCheckpoint,
}

impl Case {
    pub fn new(mode: Option<bool>) -> Self {
        let root = Root::new();
        let (mut config, recipe_path, loaded) = mediated_topology::selected(&root, mode, false);
        let deadline = deadline();
        let mut control = Control::new(&config, loaded.request, None).unwrap();
        let custody::Started {
            custody: (host, reviewer, observer, _identity),
            source,
            ..
        } = custody::start(
            &config,
            &loaded,
            false,
            &mut control,
            &deadline,
            &mut || ElapsedTick(1000),
        )
        .unwrap();

        // Registration deliberately precedes certification, inference, review,
        // and dispatch. A later independent source cut must extend this prefix.
        let registration = root.0.join("fleet-registration.anchor");
        let original = host.history_anchor().unwrap();
        let mut bytes = Vec::new();
        original
            .write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES)
            .unwrap();
        private(&registration, &bytes);
        let mut forecast =
            observer.map(|observer| prediction::Session::new(loaded.request, observer));
        let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let mut driver = FileSupervisedDriver::new(supervisor);
        let mut wire = ActorWire::new(port);
        custody::attach(&mut driver, source).unwrap();
        custody::certify(&mut driver, &deadline, &mut || ElapsedTick(1000)).unwrap();
        assert!(
            generate_selected(
                &mut driver,
                &reviewer,
                &mut config,
                &mut control,
                &deadline,
                &mut || ElapsedTick(1000),
                forecast.as_mut()
            )
            .unwrap()
        );
        submit(
            &mut driver,
            &mut wire,
            &mut config,
            &loaded,
            &deadline,
            &mut || ElapsedTick(1000),
        )
        .unwrap();
        let (mut driver, review) =
            native::review_current(&mut config, &loaded, &reviewer, driver, &deadline);
        let human = native::approve(&mut driver, &mut config, &reviewer, loaded.request);
        assert!(matches!(
            driver
                .step_computed_from_policy_file(
                    &mut config.source,
                    || ElapsedTick(1000),
                    Some(&human)
                )
                .result
                .unwrap(),
            FileDriverEvent::Dispatched { .. }
        ));
        let host = driver.supervisor().host().unwrap();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, 4);
        assert_eq!(numerical.sampled_draws, 2);
        let before = host.inspect();
        assert_eq!(before.executions, 0);
        assert!(before.control.ledger.charged > 0);
        assert!(before.stop.is_none());
        let saved = driver
            .topology()
            .unwrap()
            .checkpoint(
                &host,
                &fs::read(root.0.join("live-1.json")).unwrap(),
                &fs::read(root.0.join("inventory-1.json")).unwrap(),
            )
            .unwrap();
        assert!(saved.available());
        assert!(saved.work().checks > 0);
        assert_eq!(
            saved.work().reserved_read_bytes,
            saved.work().checks * TOPOLOGY_REFRESH_READ_BYTES
        );
        assert!(saved.last_operation() >= saved.work().checks);
        assert!(saved.history_anchor().revision() > original.revision());
        assert_eq!(saved.history_anchor().revision(), before.revision);
        let source_anchor = root.0.join("fleet-topology.anchor");
        let checkpoint = root.0.join("fleet-topology.source");
        let mut bytes = Vec::new();
        saved
            .history_anchor()
            .write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES)
            .unwrap();
        private(&source_anchor, &bytes);
        let mut bytes = Vec::new();
        saved
            .write_to(&mut bytes, MAX_TOPOLOGY_CHECKPOINT_BYTES)
            .unwrap();
        private(&checkpoint, &bytes);
        drop(host);
        drop(review);
        drop(wire);
        drop(forecast);
        assert_eq!(cleanup(driver, 1000, 1), 0);

        let profile = root.0.join("fleet-topology.json");
        let initial = pair(
            &root.0.join("initial.json"),
            &root.0.join("initial-inventory.json"),
        );
        let next = pair(
            &root.0.join("unread-next.json"),
            &root.0.join("unread-next-inventory.json"),
        );
        private(&profile, format!(r#"{{"schema_version":"fa.mediated-submit/2","initial":{initial},"next":{next},"operation":{},"source_checkpoint":"{}"}}"#,
            saved.last_operation() + 1, checkpoint.display()).as_bytes());
        let config_path = root.0.join("fleet-member.json");
        let text = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fixtures/supervised_publication.json"
        ))
        .replace(
            "/operator/publication-store",
            config.store.to_str().unwrap(),
        )
        .replace(
            "/operator/evidence.json",
            root.0.join("evidence.json").to_str().unwrap(),
        )
        .replace(
            "\"initial_payload_hex\": \"696e697469616c\"",
            "\"initial_payload_hex\": \"\"",
        )
        .replace("\"events\": 256", "\"events\": 1024")
        .replace("\"runtime_ms\": 120000", "\"runtime_ms\": 60000");
        private(&config_path, text.as_bytes());
        crate::config::assert_oversight_profiles_eq(
            &Config::read(&config_path).unwrap().profile,
            &config.profile,
        );
        for name in [
            "live-1.json",
            "inventory-1.json",
            "evidence.json",
            "native-roster.json",
            "helper-model.json",
            "helper-weights.safetensors",
            "helper-monitor.json",
            "helper-tokenizer.bbpe",
            "helper-sampling.json",
        ] {
            fs::remove_file(root.0.join(name)).unwrap();
        }
        fs::write(
            config.store.join("delivery.pending"),
            b"retain passive topology evidence",
        )
        .unwrap();
        let member = Member {
            id: 1,
            root,
            config,
            config_path,
            recipe: Some(recipe_path),
            anchor: registration,
            before,
            generation: Some(loaded.generation),
        };
        assert_eq!(member.snapshot(), member.before);
        let fleet = Fleet::new(vec![member, Member::new(2, Model::Ordinary, false)]);
        let case = Self {
            fleet,
            profile,
            source_anchor,
            checkpoint,
            saved,
        };
        case.write(INPUT_LIMIT);
        case
    }

    pub fn document(&self, input: usize) -> String {
        let original = self.fleet.document(input);
        let recipe = format!(
            r#""learned_recipe":"{}""#,
            self.fleet.members[0].recipe.as_ref().unwrap().display()
        );
        let selected = format!(
            r#"{recipe},"topology_profile":"{}","topology_history_anchor":"{}""#,
            self.profile.display(),
            self.source_anchor.display()
        );
        assert!(original.contains(&recipe));
        original.replacen(&recipe, &selected, 1)
    }

    pub fn write(&self, input: usize) {
        private(&self.fleet.manifest(), self.document(input).as_bytes());
    }

    pub fn topology_inputs(&self) -> [PathBuf; 5] {
        [
            self.profile.clone(),
            self.source_anchor.clone(),
            self.checkpoint.clone(),
            self.fleet.members[0].root.0.join("initial.json"),
            self.fleet.members[0].root.0.join("initial-inventory.json"),
        ]
    }

    pub fn custody_bytes(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        self.topology_inputs()
            .into_iter()
            .chain([self.fleet.members[0].anchor.clone()])
            .map(|path| {
                let bytes = fs::read(&path).unwrap();
                (path, bytes)
            })
            .collect()
    }

    pub fn assert_source_unchanged(&self, original: &BTreeMap<PathBuf, Vec<u8>>) {
        assert_eq!(&self.custody_bytes(), original);
        let saved = TopologyCheckpoint::read_trusted(
            &mut Cursor::new(fs::read(&self.checkpoint).unwrap()),
            MAX_TOPOLOGY_CHECKPOINT_BYTES,
        )
        .unwrap();
        assert_eq!(saved, self.saved);
        assert!(saved.work().checks > 0);
        assert_eq!(
            saved.work().reserved_read_bytes,
            saved.work().checks * TOPOLOGY_REFRESH_READ_BYTES
        );
        assert_eq!(saved.last_operation(), self.saved.last_operation());
        let limits = saved.expected().source_limits();
        assert_eq!(
            limits.checks - saved.work().checks,
            self.saved.expected().source_limits().checks - self.saved.work().checks
        );
        assert_eq!(
            limits.read_bytes - saved.work().reserved_read_bytes,
            self.saved.expected().source_limits().read_bytes
                - self.saved.work().reserved_read_bytes
        );
        assert_eq!(
            saved.history_anchor(),
            &anchors::load(&self.source_anchor).unwrap()
        );
        assert!(
            saved.history_anchor().revision()
                > anchors::load(&self.fleet.members[0].anchor)
                    .unwrap()
                    .revision()
        );
        for name in [
            "live-1.json",
            "inventory-1.json",
            "unread-next.json",
            "unread-next-inventory.json",
            "evidence.json",
            "native-roster.json",
        ] {
            assert!(!self.fleet.members[0].root.0.join(name).exists(), "{name}");
        }
    }

    // The existing recipe helper measures its original numerical reads. The
    // topology increment is independently counted from ALL five input files;
    // an uncharged profile, graph, inventory, checkpoint, or separate source
    // anchor therefore makes the one-short test unexpectedly succeed.
    pub fn exact_input_bytes(&self) -> usize {
        let base = self.fleet.exact_input_bytes();
        let original_inputs = base - self.fleet.document(base).len();
        let topology: usize = self
            .topology_inputs()
            .iter()
            .map(|path| fs::read(path).unwrap().len())
            .sum();
        assert!(topology > fs::read(&self.checkpoint).unwrap().len());
        let mut input = original_inputs + topology;
        loop {
            let next = original_inputs + topology + self.document(input).len();
            if next == input {
                return input;
            }
            input = next;
        }
    }

    pub fn manual_profile(&self) -> String {
        let root = &self.fleet.members[0].root.0;
        let initial = pair(
            &root.join("initial.json"),
            &root.join("initial-inventory.json"),
        );
        let next = pair(
            &root.join("unread-next.json"),
            &root.join("unread-next-inventory.json"),
        );
        format!(
            r#"{{"schema_version":"fa.mediated-submit/1","initial":{initial},"current":{initial},"next":{next},"operation":{},"available":true,"source":{{"checks":{},"reserved_read_bytes":{},"last_operation":{}}}}}"#,
            self.saved.last_operation() + 1,
            self.saved.work().checks,
            self.saved.work().reserved_read_bytes,
            self.saved.last_operation()
        )
    }
}

fn pair(topology: &Path, inventory: &Path) -> String {
    format!(
        r#"{{"topology_path":"{}","inventory_path":"{}"}}"#,
        topology.display(),
        inventory.display()
    )
}
