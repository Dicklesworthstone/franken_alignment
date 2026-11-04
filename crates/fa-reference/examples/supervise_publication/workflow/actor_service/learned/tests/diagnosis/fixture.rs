//! Actual numerical history and independently retained topology custody.
use super::*;
use fa_reference::action::consequence::delivery::persistent::FileDeliverySnapshot;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub(super) struct Case {
    pub root: Root,
    pub config: Config,
    pub config_path: PathBuf,
    pub recipe: PathBuf,
    pub loaded: Loaded,
    pub anchor: PathBuf,
    pub topology: Option<PathBuf>,
    pub attempt: u64,
    pub before: FileDeliverySnapshot,
}

pub(super) fn config_file(root: &Root, denied: bool) -> PathBuf {
    let mut text = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/supervised_publication.json"
    ))
    .replace(
        "/operator/publication-store",
        root.0.join("store").to_str().unwrap(),
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
    if denied {
        // Only the original action's payload predicate changes. The native
        // generated release is larger; the explicit empty experiment is not.
        text = text.replacen("\"maximum\": 512", "\"maximum\": 1", 1);
    }
    let path = root.0.join("diagnosis-config.json");
    fs::write(&path, text).unwrap();
    path
}

pub(super) fn remove_live(root: &Root, mediated: bool) {
    for name in [
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
    if mediated {
        for name in ["live-1.json", "inventory-1.json"] {
            fs::remove_file(root.0.join(name)).unwrap();
        }
    }
}

impl Case {
    pub(super) fn new(mode: Option<bool>, mediated: bool) -> Self {
        let root = Root::new();
        let recipe = if mediated {
            let (_, path, _) = topology_fixture::selected(&root, mode, false);
            path
        } else {
            let path = fixture::write(&root, b"allow");
            if let Some(owned) = mode {
                super::super::forecast::install(&root, &path, owned, 60_000);
            }
            path
        };
        // External request identity must not be mistaken for native attempt 1.
        let recipe_text =
            fs::read_to_string(&recipe)
                .unwrap()
                .replacen("\"request\":1", "\"request\":901", 1);
        fs::write(&recipe, recipe_text).unwrap();
        let config_path = config_file(&root, true);
        let mut config = Config::read(&config_path).unwrap();
        config.programs.clear();
        evidence(&root, &config, 1, true, false);
        let mut loaded = recipe::load(&recipe, &config, true).unwrap();
        assert_eq!(loaded.request, 901);
        let anchor = root.0.join("history-1.bin");
        loaded.anchor_output = Some(anchor.clone());
        if mediated {
            source_custody::admit(
                &config,
                &mut loaded,
                Some(&root.0.join("create-topology.json")),
                source_custody::Mode::Create,
            )
            .unwrap();
        }
        let limit = deadline();
        let mut control = Control::new(&config, loaded.request, None).unwrap();
        let source_custody::Started {
            custody: (host, reviewer, observer, _identity),
            source,
            output,
        } = source_custody::start(&config, &loaded, false, &mut control, &limit, &mut || {
            ElapsedTick(1000)
        })
        .unwrap();
        let mut prediction =
            observer.map(|observer| prediction::Session::new(loaded.request, observer));
        let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let mut driver = FileSupervisedDriver::new(supervisor);
        let mut wire = ActorWire::new(port);
        source_custody::attach(&mut driver, source).unwrap();
        source_custody::certify(&mut driver, &limit, &mut || ElapsedTick(1000)).unwrap();
        assert!(
            generate_selected(
                &mut driver,
                &reviewer,
                &mut config,
                &mut control,
                &limit,
                &mut || ElapsedTick(1000),
                prediction.as_mut()
            )
            .unwrap()
        );
        submit(
            &mut driver,
            &mut wire,
            &mut config,
            &loaded,
            &limit,
            &mut || ElapsedTick(1000),
        )
        .unwrap();
        let host = driver.supervisor().host().unwrap();
        let FileRequestDisposition::Admitted {
            attempt,
            stage: ActionState::Denied,
        } = host.request_status(loaded.request).unwrap().disposition
        else {
            panic!("actual frozen policy denial required before any helper review");
        };
        assert_ne!(attempt, loaded.request);
        assert!(host.request_action(loaded.request).is_ok());
        let before = host.inspect();
        assert_eq!(before.executions, 0);
        assert_eq!(before.control.ledger.charged, 0);
        assert_eq!(before.control.ledger.reserved, 0);
        drop(host);
        assert!(matches!(
            response(&mut wire, loaded.request).result,
            Ok(Knowledge::Known {
                value: ActorOutcome::Denied,
                ..
            })
        ));
        // No helper/native-review/human workflow has run. Both captures name
        // the same acknowledged original denial, not a manufactured review.
        assert!(source_custody::retain(output.as_ref(), &driver, None).is_none());
        assert!(history::retain(&loaded, &driver, None).is_none());
        assert_eq!(cleanup(driver, 1000, 1), 0);
        drop(wire);
        drop(prediction);
        let topology = mediated.then(|| {
            let path = root.0.join("diagnosis-topology.json");
            fs::write(
                &path,
                format!(
                    r#"{{"schema_version":"fa.mediated-submit/2",
                "initial":{{"topology_path":"{}","inventory_path":"{}"}},
                "next":{{"topology_path":"{}","inventory_path":"{}"}},
                "operation":{},"source_checkpoint":"{}"}}"#,
                    root.0.join("initial.json").display(),
                    root.0.join("initial-inventory.json").display(),
                    root.0.join("missing-next.json").display(),
                    root.0.join("missing-next-inventory.json").display(),
                    topology_fixture::checkpoint(&root, 1).last_operation() + 1,
                    root.0.join("source-1.bin").display()
                ),
            )
            .unwrap();
            path
        });
        remove_live(&root, mediated);
        let mut loaded = recipe::load(&recipe, &config, false).unwrap();
        loaded.recovery_anchor = Some(history::load(&anchor).unwrap());
        source_custody::admit(
            &config,
            &mut loaded,
            topology.as_deref(),
            source_custody::Mode::Historical,
        )
        .unwrap();
        fs::write(
            config.store.join("delivery.pending"),
            b"preserve staged evidence",
        )
        .unwrap();
        Self {
            root,
            config,
            config_path,
            recipe,
            loaded,
            anchor,
            topology,
            attempt,
            before,
        }
    }

    pub(super) fn args(&self) -> Vec<String> {
        let mut args = vec![
            "create-learned-generated".into(),
            self.config_path.display().to_string(),
            self.recipe.display().to_string(),
            "--resume".into(),
            "--explain-proposal".into(),
            "--history-anchor".into(),
            self.anchor.display().to_string(),
        ];
        if let Some(path) = &self.topology {
            args.extend(["--topology-profile".into(), path.display().to_string()]);
        }
        args
    }

    pub(super) fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        files(
            &self.config.store,
            &self.anchor,
            self.topology
                .as_ref()
                .map(|_| self.root.0.join("source-1.bin")),
        )
    }

    pub(super) fn unchanged(&self, before: &BTreeMap<PathBuf, Vec<u8>>) {
        assert_eq!(&self.files(), before);
        assert_eq!(fs::read_dir(self.root.0.join("peers")).unwrap().count(), 0);
        assert!(!self.root.0.join("evidence.json").exists());
        assert!(!self.root.0.join("native-roster.json").exists());
        assert!(!self.root.0.join("missing-next.json").exists());
    }
}

pub(super) fn files(
    store: &Path,
    anchor: &Path,
    source: Option<PathBuf>,
) -> BTreeMap<PathBuf, Vec<u8>> {
    fs::read_dir(store)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .chain([anchor.to_owned()])
        .chain(source)
        .map(|path| {
            assert!(path.is_file());
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect()
}
