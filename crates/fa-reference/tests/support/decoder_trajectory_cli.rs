// Included in the original example's tests; its five existing scenarios and
// checkpoint fixture stay unchanged. These are synthetic numerical cases.
const TRAJECTORIES: &[u8] = include_bytes!("../fixtures/decoder_trajectory.json");
fn trajectory_args(directory: &Directory, content: &[u8]) -> Vec<OsString> {
    let mut args = checkpoint(&directory.0);
    let path = directory.0.join("trajectories.json"); fs::write(&path, content).unwrap();
    args.push(path.into_os_string()); args
}
#[test]
fn trajectory_checked_export_matches_legacy_bytes_but_adds_all_task_outcomes() {
    let a = Directory::new(); let original = checkpoint(&a.0);
    assert!(run(&original, &mut Vec::new()).unwrap());
    let b = Directory::new(); let checked = trajectory_args(&b, TRAJECTORIES); let mut output = Vec::new();
    assert!(run(&checked, &mut output).unwrap());
    assert_eq!(fs::read(&original[3]).unwrap(), fs::read(&checked[3]).unwrap());
    let log = rows(&output);
    assert_eq!(log.iter().filter(|row| kind(row) == "trajectory_case").count(), 2);
    let summary = log.iter().find(|row| kind(row) == "trajectory_complete").unwrap();
    assert_eq!(summary.get("accepted").unwrap().as_bool(), Some(true));
    assert_eq!(summary.get("violation_timely_alarm").unwrap().as_u64(), Some(1));
    assert_eq!(kind(log.last().unwrap()), "monitor_saved");
    for row in log.iter().filter(|row| kind(row) == "trajectory_case") {
        assert!(row.get("tokens").is_none()); assert!(row.get("logits").is_none());
    }
}
#[test]
fn continuous_benign_false_stop_rejects_export_despite_successful_final_frame_training() {
    let root = Directory::new();
    let plan = std::str::from_utf8(TRAJECTORIES).unwrap().replace("\"tokens\":[0,0,0]", "\"tokens\":[0,1,0]");
    let args = trajectory_args(&root, plan.as_bytes()); let mut output = Vec::new();
    assert!(!run(&args, &mut output).unwrap()); assert!(!Path::new(&args[3]).exists());
    let log = rows(&output);
    assert_eq!(log.iter().find(|row| kind(row) == "campaign_complete").unwrap().get("accepted").unwrap().as_bool(), Some(true));
    let summary = log.last().unwrap(); assert_eq!(kind(summary), "trajectory_complete");
    assert_eq!(summary.get("accepted").unwrap().as_bool(), Some(false));
    assert_eq!(summary.get("benign_alarm").unwrap().as_u64(), Some(1));
}
#[test]
fn late_alarm_reports_its_actual_position_and_cannot_create_a_monitor_file() {
    let root = Directory::new();
    let plan = std::str::from_utf8(TRAJECTORIES).unwrap().replace("\"tokens\":[0,1,1]", "\"tokens\":[0,0,1]");
    let args = trajectory_args(&root, plan.as_bytes()); let mut output = Vec::new();
    assert!(!run(&args, &mut output).unwrap()); assert!(!Path::new(&args[3]).exists());
    let log = rows(&output);
    let case = log.iter().find(|row| kind(row) == "trajectory_case" && row.get("task").unwrap().as_u64() == Some(11)).unwrap();
    assert_eq!(case.get("first_stop_position").unwrap().as_u64(), Some(2));
    assert!(case.get("alarm_lead_tokens").unwrap().is_null());
    assert_eq!(log.last().unwrap().get("violation_late_alarm").unwrap().as_u64(), Some(1));
}
#[test]
fn malformed_optional_plan_is_rejected_before_checkpoint_access() {
    let root = Directory::new(); let args = trajectory_args(&root, b"{\"schema\":1,\"schema\":2}");
    fs::remove_file(&args[0]).unwrap(); fs::remove_file(&args[1]).unwrap();
    let mut output = Vec::new();
    assert!(run(&args, &mut output).unwrap_err().to_string().contains("Syntax"));
    assert!(output.is_empty()); assert!(!Path::new(&args[3]).exists());
}
#[test]
fn a_broken_trajectory_report_sink_blocks_export_without_retraining() {
    struct BrokenAtTrajectory { bytes: Vec<u8> }
    impl Write for BrokenAtTrajectory {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.bytes.extend_from_slice(bytes);
            if self.bytes.windows(b"trajectory_case".len()).any(|window| window == b"trajectory_case") {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
    let root = Directory::new(); let destination = root.0.join("monitor.json");
    let mut plan = ProbeRunPlan::from_json(PLAN).unwrap();
    let trajectory = TrajectoryPlan::from_json(TRAJECTORIES).unwrap();
    let mut broken = BrokenAtTrajectory { bytes: Vec::new() };
    assert!(execute_validated(&mut plan, &fixture::model(), &destination, Some(trajectory), &mut broken).is_err());
    assert!(!destination.exists()); assert_eq!(plan.remaining_capture(), CaptureWork::default());
    assert!(plan.run(&fixture::model()).is_err());
}
#[test]
fn reused_training_task_is_not_relabelled_as_an_independent_trajectory() {
    let root = Directory::new();
    let plan = std::str::from_utf8(TRAJECTORIES).unwrap().replace("\"task\":10", "\"task\":3");
    let args = trajectory_args(&root, plan.as_bytes()); let mut output = Vec::new();
    assert!(run(&args, &mut output).unwrap_err().to_string().contains("Duplicate"));
    assert!(!Path::new(&args[3]).exists());
    assert!(rows(&output).iter().all(|row| kind(row) != "monitor_saved"));
}
