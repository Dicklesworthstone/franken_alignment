//! Fixed labels and native counters only; never serialize evidence or error prose.
use fa_reference::action::consequence::delivery::persistent::observed::shutdown::coordinator::{
    FileShutdownRecoveryPass, FileShutdownPassResult, FileShutdownClockedRecoveryPass};
use fa_reference::action::consequence::delivery::persistent::{JournalError, JournalIo};
use fa_reference::Error;
use std::fmt::Write;

pub(super) fn encode(mode: &str, timed_out: bool, pass: &FileShutdownRecoveryPass) -> String {
    let mut out = String::new();
    // mode is selected from the command's closed enum, never copied from input.
    write!(out, concat!("{{\"schema_version\":\"fa.shutdown-fleet-result/1\",\"mode\":\"{}\",",
        "\"operation\":\"{}\",\"initial_revision\":\"{}\",\"final_revision\":\"{}\",",
        "\"deadline_reached\":{},\"all_observed_stopped\":{},\"all_observed_drained\":{},\"members\":["),
        mode, pass.operation(), pass.initial_revision(), pass.final_revision(), timed_out,
        pass.all_observed_stopped(), pass.all_observed_drained()).expect("String write");
    for (index, member) in pass.members().iter().enumerate() {
        if index != 0 { out.push(','); }
        record(&mut out, member.clock.domain, member.clock.clock_domain, &member.result, None, false);
    }
    out.push_str("]}\n"); out
}
pub(super) fn encode_clocked(mode: &str, timed_out: bool, pass: &FileShutdownClockedRecoveryPass) -> String {
    let mut out = String::new();
    write!(out, concat!("{{\"schema_version\":\"fa.shutdown-fleet-result/2\",\"mode\":\"{}\",",
        "\"operation\":\"{}\",\"initial_revision\":\"{}\",\"final_revision\":\"{}\",",
        "\"deadline_reached\":{},\"all_observed_stopped\":{},\"all_observed_drained\":{},\"members\":["),
        mode, pass.operation(), pass.initial_revision(), pass.final_revision(), timed_out,
        pass.all_observed_stopped(), pass.all_observed_drained()).expect("String write");
    for (index, member) in pass.members().iter().enumerate() {
        if index != 0 { out.push(','); }
        record(&mut out, member.binding.domain, member.binding.clock_domain, &member.result, Some(member.acquired_tick), false);
    }
    out.push_str("]}\n"); out
}

pub(super) fn encode_phased(mode: &str, phase: super::phases::Phase, timed_out: bool,
    pass: &FileShutdownClockedRecoveryPass) -> String {
    let mut out = String::new();
    write!(out, concat!("{{\"schema_version\":\"fa.shutdown-fleet-result/3\",\"mode\":\"{}\",",
        "\"phase\":\"{}\",\"settlement_requested\":{},\"operation\":\"{}\",",
        "\"initial_revision\":\"{}\",\"final_revision\":\"{}\",\"deadline_reached\":{},",
        "\"all_observed_stopped\":{},\"all_observed_drained\":{},\"members\":["),
        mode, phase.label(), phase == super::phases::Phase::Drain, pass.operation(),
        pass.initial_revision(), pass.final_revision(), timed_out,
        pass.all_observed_stopped(), pass.all_observed_drained()).expect("String write");
    for (index, member) in pass.members().iter().enumerate() {
        if index != 0 { out.push(','); }
        record(&mut out, member.binding.domain, member.binding.clock_domain, &member.result,
            Some(member.acquired_tick), true);
    }
    out.push_str("]}\n"); out
}

pub(super) fn encode_selected(mode: &str, phase: Option<super::phases::Phase>,
    selected: &[u64], timed_out: bool, pass: &FileShutdownClockedRecoveryPass) -> String {
    let mut out = String::new();
    write!(out, concat!("{{\"schema_version\":\"fa.shutdown-fleet-result/4\",\"mode\":\"{}\",",
        "\"phase\":\"{}\",\"settlement_requested\":{},\"operation\":\"{}\",",
        "\"initial_revision\":\"{}\",\"final_revision\":\"{}\",\"deadline_reached\":{},",
        "\"all_observed_stopped\":{},\"all_observed_drained\":{},\"selected_domains\":["),
        mode, phase.map_or("combined", super::phases::Phase::label),
        phase != Some(super::phases::Phase::Admission), pass.operation(),
        pass.initial_revision(), pass.final_revision(), timed_out,
        pass.all_observed_stopped(), pass.all_observed_drained()).expect("String write");
    // Requested new work is not proof it ran. All original outcomes follow.
    ids(&mut out, selected); out.push_str("],\"members\":[");
    for (index, member) in pass.members().iter().enumerate() {
        if index != 0 { out.push(','); }
        record(&mut out, member.binding.domain, member.binding.clock_domain, &member.result,
            Some(member.acquired_tick), true);
    }
    out.push_str("]}\n"); out
}

fn record(out: &mut String, domain: u64, clock_domain: u64, result: &FileShutdownPassResult,
    acquisition: Option<Option<fa_reference::action::ElapsedTick>>, include_fence: bool) {
    let label = match result {
        FileShutdownPassResult::NotVisited => "not_visited",
        FileShutdownPassResult::Observed(_) => "observed",
        FileShutdownPassResult::Refused(_) => "refused",
        FileShutdownPassResult::EvidenceRefused(_) => "evidence_refused",
        FileShutdownPassResult::Unacknowledged(_) => "unacknowledged",
    };
    write!(out, "{{\"domain\":\"{}\",\"clock_domain\":\"{}\",\"result\":\"{}\"",
        domain, clock_domain, label).expect("String write");
    if let FileShutdownPassResult::Observed(observation) = result {
        write!(out, ",\"journal_revision\":\"{}\",\"executions\":\"{}\"",
            observation.journal_revision, observation.executions).expect("String write");
        if let Some(progress) = &observation.stop {
            write!(out, ",\"stop_operation\":\"{}\",\"drained\":{},\"reserved\":\"{}\",\"charged\":\"{}\",\"unresolved\":[",
                progress.receipt.request().operation, progress.drained(), progress.reserved_units,
                progress.charged_units).expect("String write");
            ids(out, &progress.unresolved);
            out.push_str("],\"irrecoverable\":["); ids(out, &progress.irrecoverable); out.push(']');
        }
    }
    match result {
        FileShutdownPassResult::Refused(error) | FileShutdownPassResult::EvidenceRefused(error)
        | FileShutdownPassResult::Unacknowledged(error) => failure(out, error),
        _ => {}
    }
    if let Some(tick) = acquisition {
        match tick {
            Some(at) => write!(out, ",\"acquired_tick\":\"{}\"", at.0).expect("String write"),
            None => out.push_str(",\"acquired_tick\":null"),
        }
    }
    if include_fence && let FileShutdownPassResult::Observed(observation) = result
        && let Some(stop) = &observation.stop {
        write!(out, ",\"endpoint_fenced\":{}", stop.endpoint_fenced).expect("String write");
    }
    out.push('}');
}

fn ids(out: &mut String, values: &[u64]) {
    for (index, value) in values.iter().enumerate() {
        if index != 0 { out.push(','); }
        write!(out, "\"{value}\"").expect("String write");
    }
}

fn failure(out: &mut String, error: &JournalError) {
    let code = match error {
        JournalError::Contract(Error::InvalidInput) => "invalid_input",
        JournalError::Contract(Error::Incomplete) => "incomplete",
        JournalError::Contract(Error::Limit) => "limit",
        JournalError::Contract(Error::Overflow) => "overflow",
        JournalError::Contract(Error::Duplicate) => "duplicate",
        JournalError::Contract(Error::Missing) => "missing",
        JournalError::Contract(Error::WrongState) => "wrong_state",
        JournalError::Contract(Error::Stale) => "stale",
        JournalError::Contract(Error::Binding) => "binding",
        JournalError::Busy => "busy", JournalError::Unavailable => "unavailable",
        JournalError::InvalidFile => "invalid_file", JournalError::Io(_) => "io",
    };
    write!(out, ",\"error\":\"{code}\"").expect("String write");
    if let JournalError::Io(failure) = error {
        let operation = match failure.operation {
            JournalIo::Directory => "directory", JournalIo::Lock => "lock", JournalIo::Read => "read",
            JournalIo::Stage => "stage", JournalIo::Write => "write", JournalIo::FileSync => "file_sync",
            JournalIo::Rename => "rename", JournalIo::DirectorySync => "directory_sync", JournalIo::Cleanup => "cleanup",
        };
        write!(out, ",\"io_operation\":\"{operation}\",\"replacement_may_be_visible\":{}",
            failure.replacement_may_be_visible).expect("String write");
    }
}
