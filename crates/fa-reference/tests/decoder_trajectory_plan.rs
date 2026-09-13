#[path = "support/decoder_probe_campaign.rs"]
#[allow(dead_code)]
mod fixture;
use fixture::*;
use fa_reference::action::consequence::activation::probe::training::decoder::trajectory::plan::*;
use fa_reference::action::consequence::activation::probe::training::interchange::{MonitorExportSettings, LayerMonitorSettings};
use fa_reference::action::consequence::activation::monitor::RefinementBudget;
use fa_reference::strict_json::{self, Limits};
use fa_reference::Error;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
const PLAN: &[u8] = include_bytes!("fixtures/decoder_trajectory.json");
fn settings() -> MonitorExportSettings {
    let budget = RefinementBudget { encoded_bytes: 10_000, probe_coordinates: 10_000 };
    MonitorExportSettings { generation: 9, budget,
        layers: (1..=2).map(|layer| (layer, LayerMonitorSettings { levels: vec![23], budget })).collect() }
}
fn prepared(bytes: &[u8]) -> Result<PreparedTrajectory, TrajectoryPlanError> {
    let m = model(); let campaign = campaign(&capture(&m, &cases()));
    TrajectoryPlan::from_json(bytes)?.bind(&campaign, settings())
}
#[test]
fn strict_plan_runs_once_and_reports_every_case_without_hidden_state() {
    let mut plan = prepared(PLAN).unwrap(); let before = plan.remaining_budget();
    let work = plan.planned_work(); let report = plan.run().unwrap();
    assert!(plan.started()); assert!(report.accepted());
    assert_eq!(plan.remaining_budget().scalar_products, before.scalar_products - work.scalar_products);
    assert!(matches!(plan.run(), Err(Error::WrongState)));
    let mut out = Vec::new(); report.write_ndjson(&mut out).unwrap();
    let rows: Vec<_> = std::str::from_utf8(&out).unwrap().lines().map(|line| strict_json::parse(line.as_bytes(), Limits::default()).unwrap()).collect();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[2].get("cases").unwrap().as_u64(), Some(2));
    assert_eq!(rows[2].get("benign_cases").unwrap().as_u64(), Some(1));
    assert_eq!(rows[2].get("violation_cases").unwrap().as_u64(), Some(1));
    for row in rows { assert!(row.get("tokens").is_none()); assert!(row.get("logits").is_none()); }
}
#[test]
fn parser_rejects_duplicate_unknown_and_execution_bearing_override_fields() {
    assert!(matches!(TrajectoryPlan::from_json(b"{\"schema\":1,\"schema\":2}"), Err(TrajectoryPlanError::Syntax)));
    let text = std::str::from_utf8(PLAN).unwrap();
    for bad in [text.replacen("{", "{\"threshold\":0,", 1),
        text.replace("\"effect_position\":null", "\"effect_position\":0"),
        text.replace("\"min_timely_alarms\":1", "\"min_timely_alarms\":0"),
        text.replace("\"tokens\":[0,1,1]", "\"tokens\":[0,true,1]"),
        text.replace("\"original_tokens\":6", "\"original_tokens\":6.0"),
        text.replace("\"label\":\"violation\"", "\"label\":\"benign\""),
        text.replace("\"task\":11", "\"task\":10")] {
        assert_ne!(bad.as_bytes(), PLAN); assert!(TrajectoryPlan::from_json(bad.as_bytes()).is_err());
    }
}
#[test]
fn identity_context_and_vocabulary_are_checked_before_execution() {
    let text = std::str::from_utf8(PLAN).unwrap();
    for bad in [text.replace("\"model_generation\":3", "\"model_generation\":4"),
        text.replace("\"context\":8", "\"context\":7"),
        text.replace("\"tokens\":[0,1,1]", "\"tokens\":[0,1,99]")] {
        let plan = TrajectoryPlan::from_json(bad.as_bytes()).unwrap();
        assert!(plan.validate_profile(model().profile()).is_err());
        assert!(prepared(bad.as_bytes()).is_err());
    }
}
#[test]
fn inadequate_budget_never_starts_or_replenishes_the_prepared_suite() {
    let text = std::str::from_utf8(PLAN).unwrap().replace("\"cases\":2", "\"cases\":1");
    let mut plan = prepared(text.as_bytes()).unwrap(); let before = plan.remaining_budget();
    for _ in 0..2 {
        assert!(matches!(plan.run(), Err(Error::Limit))); assert!(!plan.started());
        assert_eq!(plan.remaining_budget(), before);
    }
}
#[test]
fn reporting_failure_leaves_the_completed_result_and_single_use_state_intact() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
        fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
    }
    let mut plan = prepared(PLAN).unwrap(); let report = plan.run().unwrap();
    let remaining = plan.remaining_budget();
    assert_eq!(report.write_ndjson(&mut Broken).unwrap_err().kind(), io::ErrorKind::BrokenPipe);
    assert!(report.accepted()); assert!(report.monitor_json().is_ok());
    assert_eq!(plan.remaining_budget(), remaining); assert!(matches!(plan.run(), Err(Error::WrongState)));
}
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("trajectory plan cleanup: {error}"); } }
}
#[test]
fn bounded_file_plan_loads_and_refuses_directories_oversize_and_symlinks() {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let root = Directory(std::env::temp_dir().join(format!("fa-trajectory-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))));
    fs::create_dir(&root.0).unwrap(); let path = root.0.join("plan.json"); fs::write(&path, PLAN).unwrap();
    let m = model(); let campaign = campaign(&capture(&m, &cases()));
    let mut loaded = read_trajectory_plan(&path).unwrap().bind(&campaign, settings()).unwrap();
    assert!(loaded.run().unwrap().accepted());
    assert!(matches!(read_trajectory_plan(&root.0), Err(TrajectoryPlanError::NotRegular)));
    #[cfg(unix)] {
        let link = root.0.join("link.json"); std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(matches!(read_trajectory_plan(link), Err(TrajectoryPlanError::NotRegular)));
    }
    fs::write(&path, vec![b' '; MAX_TRAJECTORY_PLAN_BYTES + 1]).unwrap();
    assert!(matches!(read_trajectory_plan(path), Err(TrajectoryPlanError::Limit)));
}
