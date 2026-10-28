//! Exhaustive native subset search over an independently reconstructed admission.
//! No alternate evaluator, early-success shortcut, or effect-bearing result.
#[path = "search/menu.rs"] mod menu;
#[path = "search/render.rs"] mod render;
pub(crate) use menu::Menu;
use super::{FileProposalInvestigation, Intervention, InterventionScope, checkpoint, debug, unavailable};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::investigation::FileProposalRepairReport;
use fa_reference::action::consequence::experiment::proposal::search::{ProposalSearchBudget, ProposalSearchStatus, MAX_PROPOSAL_SEARCH_CASES, MAX_PROPOSAL_SEARCH_EDIT_BYTES};
use std::{io::Write, time::Duration};

pub(crate) fn run(investigation: &FileProposalInvestigation, menu: &Menu, wall: Duration,
    elapsed: &mut impl FnMut() -> Duration, out: &mut impl Write) -> Result<(), String>
{
    let report = evaluate(investigation, menu, wall, elapsed)?;
    let complete = report.search().status() == ProposalSearchStatus::Complete;
    let bytes = render::encode(&report, menu)?;
    checkpoint(wall, elapsed)?;
    out.write_all(&bytes).map_err(|error| format!("proposal search output failed: {error}; no authority or effect was changed"))?;
    out.flush().map_err(|error| format!("proposal search output flush failed: {error}; no authority or effect was changed"))?;
    if complete { Ok(()) } else {
        Err(format!("proposal search incomplete: {:?}; see evaluated and unreported masks; no permission was issued", report.search().status()))
    }
}

fn evaluate(investigation: &FileProposalInvestigation, menu: &Menu, wall: Duration,
    elapsed: &mut impl FnMut() -> Duration) -> Result<FileProposalRepairReport, String>
{
    checkpoint(wall, elapsed)?;
    // This immutable investigation and its scope must come from the SAME menu,
    // not a broader registration supplied by another operator invocation.
    if investigation.origin().scope() != &menu.scope { return Err("search registration differs from the original investigation".into()); }
    let mut search = investigation.begin_repair_search(&menu.candidates, menu.budget).map_err(unavailable)?;
    loop {
        // Check after each case too, including the final one. A deadline failure
        // publishes no partial JSON; dropping this data-only cursor cannot retry
        // effects. An explicitly requested case cut instead invokes ORIGINAL
        // cancellation and reports its completed, entered and unreported work.
        checkpoint(wall, elapsed)?;
        if search.status() != ProposalSearchStatus::Running { break; }
        if search.work().completed_cases >= menu.stop_after_cases {
            search.cancel().map_err(unavailable)?;
            break;
        }
        search.advance().map_err(unavailable)?;
    }
    let report = if search.status() == ProposalSearchStatus::Complete {
        search.finish().map_err(unavailable)?
    } else { search.report() };
    checkpoint(wall, elapsed)?;
    Ok(report)
}

#[cfg(test)]
#[path = "search/tests.rs"] mod tests;
