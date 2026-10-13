use super::*;
use fa_reference::strict_json::{self, Limits};
use std::{fs, io, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

const CONFIG: &[u8] = include_bytes!("../../fixtures/supervised_publication.json");
const SETTINGS: &[u8] = include_bytes!("../../fixtures/native_andon_operator.json");

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-andon-operator-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn config(&self) -> Config {
        let mut config = Config::decode(CONFIG).unwrap();
        config.store = self.0.join("store");
        config
    }
    fn anchor(&self) -> PathBuf { self.0.join("anchor") }
    fn resume(&self, output: Option<&Path>) -> PathBuf {
        let path = self.0.join("resume.json");
        let extra = output.map(|p| format!(",\"retain_history_anchor\":\"{}\"", p.display())).unwrap_or_default();
        fs::write(&path, format!("{{\"schema_version\":\"fa.guarded-resume/1\",\"minimum\":{{\"journal_revision\":0,\"control_sequence\":0,\"authority_epoch\":0}},\"history_anchor\":\"{}\"{extra}}", self.anchor().display())).unwrap();
        path
    }
    fn bytes(&self) -> Vec<u8> { fs::read(self.0.join("store/delivery.bin")).unwrap() }
    fn create(&self) {
        create(&self.config(), &Settings::decode(SETTINGS).unwrap(), &self.anchor(),
            &mut || Ok(()), &mut Vec::new()).unwrap();
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("Andon operator test cleanup: {error}"); }
    }
}
fn valid_json(bytes: &[u8]) {
    strict_json::parse(bytes, Limits { max_bytes: 64 * 1024, max_depth: 8,
        max_items: 4096, max_string_bytes: 4096 }).unwrap();
}

#[test]
fn creation_and_locked_inspection_use_original_guarded_state_without_evidence_or_helpers() {
    let root = Directory::new();
    let config = root.config();
    let settings = Settings::decode(SETTINGS).unwrap();
    let mut output = Vec::new();
    create(&config, &settings, &root.anchor(), &mut || Ok(()), &mut output).unwrap();
    valid_json(&output);
    assert!(String::from_utf8(output).unwrap().contains("\"history_anchor_retained\":true"));
    let resume = root.resume(None);
    let request = ResumeProfile::read(&resume).unwrap();
    let custody = request.admit(&config).unwrap();
    let expected = settings.requirements(&config, request.minimum()).unwrap();
    let (host, _roles) = FileOversight::open_andon_guarded(&config.store, config.profile.clone(),
        &expected, &settings.protocol, custody.anchor()).unwrap();
    let before = root.bytes();
    let mut output = Vec::new();
    inspect(&config, &settings, &resume, &mut || Ok(()), &mut output).unwrap();
    valid_json(&output);
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("\"historical\":true"));
    assert!(text.contains("\"false_stops\":\"0\""));
    assert!(!text.contains("profile_bytes"));
    assert_eq!(root.bytes(), before);
    assert!(!host.clock_ready());
    assert_eq!(host.andon_ledger().unwrap().windows().len(), 1);
}

#[test]
fn settings_are_closed_bounded_and_cannot_launder_changed_window_budgets() {
    let text = std::str::from_utf8(SETTINGS).unwrap();
    assert!(Settings::decode(SETTINGS).is_ok());
    for changed in [
        text.replacen("\"domain\": 1", "\"domain\": 1, \"domain\": 2", 1),
        text.replacen("\"domain\": 1", "\"domain\": 1, \"extra\": 2", 1),
        text.replacen("\"denominator\": 2", "\"denominator\": 0", 1),
        text.replacen("\"cost_per_stop\": 7", "\"cost_per_stop\": 8", 1),
        text.replacen("fa.native-andon-operator/1", "fa.native-andon-operator/2", 1),
    ] { assert!(Settings::decode(changed.as_bytes()).is_err()); }
    let changed = text.replace("\"minimum_evaluation_revision\": 0", "\"minimum_evaluation_revision\": 1");
    assert!(Settings::decode(changed.as_bytes()).unwrap().creation().is_err());
    assert!(Settings::decode(&vec![b' '; 65537]).is_err());
}

#[test]
fn wrong_source_profile_and_inspection_output_request_leave_all_bytes_untouched() {
    let root = Directory::new(); root.create();
    let before = root.bytes(); let anchor = fs::read(root.anchor()).unwrap();
    let output = root.0.join("forbidden-export");
    assert!(inspect(&root.config(), &Settings::decode(SETTINGS).unwrap(), &root.resume(Some(&output)),
        &mut || Ok(()), &mut Vec::new()).is_err());
    assert!(!output.exists());
    let mut settings = Settings::decode(SETTINGS).unwrap(); settings.protocol.period += 1;
    assert!(inspect(&root.config(), &settings, &root.resume(None), &mut || Ok(()), &mut Vec::new()).is_err());
    assert_eq!(root.bytes(), before); assert_eq!(fs::read(root.anchor()).unwrap(), anchor);
}

struct BrokenOutput { flush_only: bool }
impl Write for BrokenOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.flush_only { Ok(bytes.len()) } else { Err(io::ErrorKind::BrokenPipe.into()) }
    }
    fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
}
#[test]
fn acknowledged_creation_retains_anchor_despite_deadline_or_stdout_failure() {
    for flush_only in [false, true] {
        let root = Directory::new(); let mut checks = 0;
        let error = create(&root.config(), &Settings::decode(SETTINGS).unwrap(), &root.anchor(),
            &mut || { checks += 1; if checks == 1 { Ok(()) } else { Err("deadline after acknowledgment".into()) } },
            &mut BrokenOutput { flush_only }).unwrap_err();
        assert!(error.contains("deadline")); assert!(error.contains("output failed"));
        assert!(root.anchor().is_file()); let before = root.bytes();
        inspect(&root.config(), &Settings::decode(SETTINGS).unwrap(), &root.resume(None),
            &mut || Ok(()), &mut Vec::new()).unwrap();
        assert_eq!(root.bytes(), before);
        assert!(create(&root.config(), &Settings::decode(SETTINGS).unwrap(), &root.anchor(),
            &mut || Ok(()), &mut Vec::new()).is_err());
        assert_eq!(root.bytes(), before);
    }
}

#[test]
fn custody_and_deadline_refuse_before_creation_and_never_overwrite_outputs() {
    let root = Directory::new();
    fs::write(root.anchor(), b"retain original bytes").unwrap();
    assert!(create(&root.config(), &Settings::decode(SETTINGS).unwrap(), &root.anchor(),
        &mut || Ok(()), &mut Vec::new()).is_err());
    assert_eq!(fs::read(root.anchor()).unwrap(), b"retain original bytes");
    assert!(!root.config().store.exists());
    assert!(create(&root.config(), &Settings::decode(SETTINGS).unwrap(), &root.0.join("new"),
        &mut || Err("deadline".into()), &mut Vec::new()).is_err());
    assert!(!root.config().store.exists()); assert!(!root.0.join("new").exists());
}

#[test]
fn actual_command_routing_checks_closed_arguments_before_configuration_and_counts_ingestion_time() {
    assert_eq!(command(&["create".into(), "/missing".into(), "/missing".into(), "/missing".into(),
        "--guarded".into()], &mut Vec::new()), Err(USAGE.into()));
    let root = Directory::new();
    let path = root.0.join("config.json");
    let config = std::str::from_utf8(CONFIG).unwrap().replace("/operator/publication-store", root.config().store.to_str().unwrap());
    fs::write(&path, config).unwrap();
    let args = ["create".into(), path.to_str().unwrap().into(), "/missing-settings".into(),
        root.anchor().to_str().unwrap().into()];
    let error = command_with_elapsed(&args, &mut || Duration::from_secs(1000), &mut Vec::new()).unwrap_err();
    assert!(error.contains("deadline")); assert!(!root.config().store.exists());
    let settings = root.0.join("settings.json"); fs::write(&settings, SETTINGS).unwrap();
    let args = [args[0].clone(), args[1].clone(), settings.to_str().unwrap().into(), args[3].clone()];
    command_with_elapsed(&args, &mut || Duration::ZERO, &mut Vec::new()).unwrap();
    assert!(root.anchor().exists());
}

#[path = "mutation_tests.rs"]
mod mutations;
