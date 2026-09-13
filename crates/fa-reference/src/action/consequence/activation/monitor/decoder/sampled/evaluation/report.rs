//! Typed NDJSON observations. Reporting never changes a case or its acceptance.
use super::{BaselineEnd, MonitoredEnd, RolloutReport, RolloutWork};
use std::io::{self, Write};

impl RolloutReport {
    /// Every planned task appears once, including failed numerical arms. The
    /// final flush is a reporting barrier, not durable storage acknowledgment.
    pub fn write_ndjson<W: Write + ?Sized>(&self, out: &mut W) -> io::Result<()> {
        let id = self.profile.identity(); let policy = &self.protocol.sampling;
        write!(out, "{{\"kind\":\"sampled_protocol\",\"identity\":{{\"tenant\":{},\"model\":{},\"model_generation\":{},\"tokenizer_generation\":{},\"profile_generation\":{}}},\"context\":{},\"sampling\":{{\"id\":{},\"generation\":{},\"vocabulary\":{},\"temperature\":{},\"top_k\":{},\"top_p\":{}}},\"max_new_tokens\":{},\"stop_tokens\":",
            id.tenant, id.model, id.model_generation, id.tokenizer_generation, id.profile_generation,
            self.profile.shape().context, policy.id(), policy.generation(), policy.vocabulary(),
            policy.temperature(), policy.top_k(), policy.top_p(), self.protocol.max_new_tokens)?;
        numbers(out, self.protocol.stop_tokens.iter().copied())?;
        out.write_all(b",\"effect_patterns\":[")?;
        for (index, pattern) in self.protocol.effect_patterns.iter().enumerate() {
            if index != 0 { out.write_all(b",")?; } numbers(out, pattern.iter().copied())?;
        }
        let c = self.criteria;
        write!(out, "],\"criteria\":{{\"minimum_benign\":{},\"minimum_effects\":{},\"minimum_timely_alarms\":{},\"maximum_benign_stops\":{},\"maximum_misses\":{}}},\"admitted\":",
            c.minimum_benign, c.minimum_effects, c.minimum_timely_alarms, c.maximum_benign_stops, c.maximum_misses)?;
        work(out, self.admitted)?;
        out.write_all(b",\"oracle_scope\":\"generated_token_patterns_only\",\"permission\":\"not_issued\"}\n")?;
        for result in &self.cases {
            let case = &result.case;
            write!(out, "{{\"kind\":\"sampled_case\",\"task\":{},\"lineage\":{},\"random_stream\":{},\"seed\":{},\"prompt\":",
                case.origin.task, case.origin.lineage, case.random_stream, case.seed)?;
            numbers(out, case.prompt.iter().copied())?;
            out.write_all(b",\"baseline_tokens\":")?; numbers(out, result.baseline_tokens.iter().copied())?;
            out.write_all(b",\"released_tokens\":")?; numbers(out, result.released_tokens.iter().copied())?;
            out.write_all(b",\"first_effect_position\":")?;
            match result.first_effect_position { Some(position) => write!(out, "{position}")?, None => out.write_all(b"null")?, }
            out.write_all(b",\"baseline_end\":")?;
            match &result.baseline_end {
                BaselineEnd::Horizon => out.write_all(b"{\"kind\":\"horizon\"}")?,
                BaselineEnd::StopToken(token) => write!(out, "{{\"kind\":\"stop_token\",\"token\":{token}}}")?,
                BaselineEnd::Failed { position, error } => {
                    write!(out, "{{\"kind\":\"failed\",\"position\":{position},\"error\":")?;
                    string(out, &format!("{error:?}"))?; out.write_all(b"}")?;
                }
            }
            out.write_all(b",\"monitored_end\":")?;
            match &result.monitored_end {
                MonitoredEnd::Complete => out.write_all(b"{\"kind\":\"complete\"}")?,
                MonitoredEnd::NotCompleted { position } => write!(out, "{{\"kind\":\"not_completed\",\"position\":{position}}}")?,
                MonitoredEnd::PairMismatch { position } => write!(out, "{{\"kind\":\"pair_mismatch\",\"position\":{position}}}")?,
                MonitoredEnd::Failed { position, error } => {
                    write!(out, "{{\"kind\":\"failed\",\"position\":{position},\"error\":")?;
                    string(out, &format!("{error:?}"))?; out.write_all(b"}")?;
                }
                MonitoredEnd::Held { position, review } => {
                    write!(out, "{{\"kind\":\"held\",\"position\":{position},\"generation\":{},\"outcome\":",
                        review.generation())?;
                    string(out, &format!("{:?}", review.outcome()))?;
                    write!(out, ",\"required_layers\":{},\"unreviewed_layers\":{},\"layers\":[",
                        review.required_layers(), review.unreviewed_layers())?;
                    for (index, layer) in review.layers().iter().enumerate() {
                        if index != 0 { out.write_all(b",")?; }
                        write!(out, "{{\"layer\":{},\"outcome\":", layer.layer)?;
                        string(out, &format!("{:?}", layer.report.outcome()))?;
                        write!(out, ",\"encoded_bytes\":{},\"probe_coordinates\":{}}}",
                            layer.report.encoded_bytes(), layer.report.probe_coordinates())?;
                    }
                    out.write_all(b"]}")?;
                }
            }
            out.write_all(b",\"outcome\":")?; string(out, &format!("{:?}", result.outcome()))?;
            let monitoring = result.monitoring_work;
            write!(out, ",\"baseline_draws\":{},\"monitored_draws\":{},\"paired_positions\":{},\"baseline_scalar_products\":{},\"monitored_scalar_products\":{},\"monitoring\":{{\"frame_reviews\":{},\"encoded_bytes\":{},\"probe_coordinates\":{},\"codec_coordinates\":{}}}}}\n",
                result.baseline_draws, result.monitored_draws, result.paired_positions,
                result.baseline_work.scalar_products().map_err(invalid_work)?,
                result.monitored_work.scalar_products().map_err(invalid_work)?,
                monitoring.frame_reviews, monitoring.encoded_bytes, monitoring.probe_coordinates, monitoring.codec_coordinates)?;
        }
        let c = self.counts;
        writeln!(out, "{{\"kind\":\"sampled_complete\",\"accepted\":{},\"planned_cases\":{},\"counts\":{{\"benign_complete\":{},\"benign_alarm\":{},\"benign_other_hold\":{},\"timely_alarm\":{},\"late_alarm\":{},\"quiet_miss\":{},\"other_hold\":{},\"failed\":{}}},\"permission\":\"not_issued\"}}",
            self.accepted(), self.admitted.cases, c.benign_complete, c.benign_alarm, c.benign_other_hold,
            c.timely_alarm, c.late_alarm, c.quiet_miss, c.other_hold, c.failed)?;
        out.flush()
    }
}
fn invalid_work(error: crate::Error) -> io::Error { io::Error::new(io::ErrorKind::InvalidData, format!("{error:?}")) }
fn numbers<W: Write + ?Sized>(out: &mut W, values: impl IntoIterator<Item = u32>) -> io::Result<()> {
    out.write_all(b"[")?;
    for (index, value) in values.into_iter().enumerate() {
        if index != 0 { out.write_all(b",")?; } write!(out, "{value}")?;
    }
    out.write_all(b"]")
}
fn work<W: Write + ?Sized>(out: &mut W, value: RolloutWork) -> io::Result<()> {
    write!(out, "{{\"cases\":{},\"token_steps\":{},\"scalar_products\":{},\"sampling_entries\":{},\"comparison_entries\":{},\"oracle_comparisons\":{},\"monitor_bytes\":{},\"probe_coordinates\":{},\"retained_score_words\":{}}}",
        value.cases, value.token_steps, value.scalar_products, value.sampling_entries, value.comparison_entries,
        value.oracle_comparisons, value.monitor_bytes, value.probe_coordinates, value.retained_score_words)
}
fn string<W: Write + ?Sized>(out: &mut W, value: &str) -> io::Result<()> {
    out.write_all(b"\"")?;
    for character in value.chars() {
        match character {
            '"' => out.write_all(b"\\\"")?, '\\' => out.write_all(b"\\\\")?,
            '\n' => out.write_all(b"\\n")?, '\r' => out.write_all(b"\\r")?, '\t' => out.write_all(b"\\t")?,
            c if (c as u32) < 0x20 => write!(out, "\\u{:04x}", c as u32)?,
            c => { let mut bytes = [0_u8; 4]; out.write_all(c.encode_utf8(&mut bytes).as_bytes())?; }
        }
    }
    out.write_all(b"\"")
}
