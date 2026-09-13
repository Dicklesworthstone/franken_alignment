//! The existing operator command consumes these outcomes before artifact export.
use super::{TrajectoryExpectation, TrajectoryReport, TrajectoryTermination};
use crate::action::consequence::activation::monitor::MonitorOutcome;
use std::io::{self, Write};

impl TrajectoryReport {
    /// Every planned task is reported, including failures. No tokens, logits or
    /// hidden model state are printed. Output failure cannot rerun an admitted suite.
    pub fn write_ndjson(&self, output: &mut impl Write) -> io::Result<()> {
        for case in self.cases().values() {
            let (label, effect_position) = match case.expectation() {
                TrajectoryExpectation::Benign => ("benign", "null".to_owned()),
                TrajectoryExpectation::Violation { effect_position } => ("violation", effect_position.to_string()),
            };
            let (termination, stopped) = match case.termination() {
                TrajectoryTermination::Complete => ("complete", "null".to_owned()),
                TrajectoryTermination::Failed { position, .. } => ("numerical_failure", position.to_string()),
                TrajectoryTermination::Held { position, review } => {
                    let cause = match review.outcome() {
                        MonitorOutcome::Alarm => "alarm",
                        MonitorOutcome::AtThreshold => "at_threshold",
                        MonitorOutcome::BudgetExhausted => "budget_exhausted",
                        MonitorOutcome::Unresolved => "unresolved",
                        MonitorOutcome::NoAlarm => "incomplete_coverage",
                    };
                    (cause, position.to_string())
                }
            };
            let lead = case.alarm_lead_tokens().map_or_else(|| "null".to_owned(), |n| n.to_string());
            writeln!(output, "{{\"kind\":\"trajectory_case\",\"task\":{},\"lineage\":{},\"label\":\"{label}\",\"effect_position\":{effect_position},\"planned_tokens\":{},\"quiet_tokens\":{},\"computed_tokens\":{},\"termination\":\"{termination}\",\"first_stop_position\":{stopped},\"alarm_lead_tokens\":{lead},\"monitor_encoded_bytes\":{},\"permission\":\"not_issued\"}}",
                case.origin().task, case.origin().lineage, case.planned_tokens(), case.quiet_tokens(),
                case.numerical().tokens, case.monitoring().encoded_bytes)?;
        }
        let counts = self.counts(); let work = self.admitted_work(); let criteria = self.criteria();
        writeln!(output, "{{\"kind\":\"trajectory_complete\",\"criteria_id\":{},\"criteria_generation\":{},\"accepted\":{},\"cases\":{},\"benign_cases\":{},\"violation_cases\":{},\"benign_complete\":{},\"benign_alarm\":{},\"benign_other_hold\":{},\"benign_failed\":{},\"violation_timely_alarm\":{},\"violation_late_alarm\":{},\"violation_complete\":{},\"violation_other_hold\":{},\"violation_failed\":{},\"admitted_original_tokens\":{},\"admitted_scalar_products\":{},\"admitted_monitor_bytes\":{},\"permission\":\"not_issued\"}}",
            criteria.id(), criteria.generation(), self.accepted(), counts.total(), counts.benign(), counts.violations(),
            counts.benign_complete, counts.benign_alarm, counts.benign_other_hold, counts.benign_failed,
            counts.violation_timely_alarm, counts.violation_late_alarm, counts.violation_complete,
            counts.violation_other_hold, counts.violation_failed, work.original_tokens, work.scalar_products,
            work.monitor_encoded_bytes)?;
        output.flush()
    }
}
