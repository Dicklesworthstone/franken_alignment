//! Explicit, finite discovery across original evaluator pages. A cursor is
//! followed only after its complete response has been checked and flushed.
//! No failure restarts the inventory, retries a page or submits an assessment.
use super::{Outbound, Settings, cases, debug, exchange, pending, protocol};
use fa_reference::action::consequence::delivery::persistent::observed::credibility::MAX_PENDING_EVALUATION_PAGE;
use fa_reference::action::consequence::oversight::credibility::MAX_EVALUATION_CASES;
use std::io::Write;

pub(super) const MAX_CASE_PAGES: usize = MAX_EVALUATION_CASES;
pub(super) const MAX_PENDING_PAGES: usize =
    MAX_EVALUATION_CASES.div_ceil(MAX_PENDING_EVALUATION_PAGE);

enum Inventory {
    Pending {
        previous: Option<pending::PageMetadata>,
        received: usize,
    },
    Cases {
        previous: Option<cases::PageMetadata>,
        received: usize,
    },
}

enum Progress {
    Unknown,
    Complete,
    Continue(Outbound),
}

impl Inventory {
    fn start(request: &Outbound) -> Result<Self, String> {
        match request {
            Outbound::Pending(request) if request.continuation.is_none() => Ok(Self::Pending {
                previous: None,
                received: 0,
            }),
            Outbound::Cases(request)
                if request.after_round.is_none() && request.expected_basis.is_none() =>
            {
                Ok(Self::Cases {
                    previous: None,
                    received: 0,
                })
            }
            _ => Err("bounded evaluator inventories require a fresh listing, not a supplied cursor or assessment".into()),
        }
    }

    fn observe(
        &mut self,
        settings: &Settings,
        request: &Outbound,
        receipt: &[u8],
    ) -> Result<Progress, String> {
        match (self, request) {
            (
                Self::Pending { previous, received },
                Outbound::Pending(request),
            ) => {
                let Some(page) = pending::page_metadata(request, receipt)? else {
                    return Ok(Progress::Unknown);
                };
                if let Some(prior) = previous {
                    if page.evaluation_revision != prior.evaluation_revision
                        || page.total_pending != prior.total_pending
                        || page.journal_revision < prior.journal_revision
                        || page.control_sequence < prior.control_sequence
                        || page.authority_epoch < prior.authority_epoch
                        || (page.journal_revision == prior.journal_revision
                            && (page.control_sequence != prior.control_sequence
                                || page.authority_epoch != prior.authority_epoch
                                || page.unfinished_reviews != prior.unfinished_reviews))
                    {
                        return Err("pending inventory changed its original count or moved its acknowledged history backwards".into());
                    }
                }
                let remaining = page
                    .total_pending
                    .checked_sub(*received)
                    .ok_or("pending inventory count moved behind its emitted prefix")?;
                if page.count != remaining.min(MAX_PENDING_EVALUATION_PAGE)
                    || page.next_after_round.is_some() != (remaining > page.count)
                {
                    return Err("pending inventory page skips, repeats or prematurely closes its original inventory".into());
                }
                *received += page.count;
                *previous = Some(page);
                match page.next_after_round {
                    Some(round) => Ok(Progress::Continue(Outbound::Pending(
                        pending::PendingRequest::prepare(
                            settings,
                            Some((page.evaluation_revision, round)),
                        )?,
                    ))),
                    None => Ok(Progress::Complete),
                }
            }
            (Self::Cases { previous, received }, Outbound::Cases(request)) => {
                let Some(page) = cases::page_metadata(request, receipt)? else {
                    return Ok(Progress::Unknown);
                };
                if previous.is_some_and(|prior| {
                    page.basis != prior.basis
                        || page.total_cases != prior.total_cases
                        || page.unapplied_rounds != prior.unapplied_rounds
                }) {
                    return Err("case inventory changed its original cut or complete retained counts".into());
                }
                let remaining = page
                    .total_cases
                    .checked_sub(*received)
                    .ok_or("case inventory count moved behind its emitted prefix")?;
                if page.remaining_cases != remaining
                    || page.count != remaining.min(request.limit)
                    || page.next_after.is_some() != (remaining > page.count)
                {
                    return Err("case inventory page skips, repeats or prematurely closes its original inventory".into());
                }
                *received += page.count;
                *previous = Some(page);
                match page.next_after {
                    Some(round) => Ok(Progress::Continue(Outbound::Cases(
                        cases::CasesRequest::prepare(
                            settings,
                            request.limit,
                            Some(round),
                            Some(page.basis),
                        )?,
                    ))),
                    None => Ok(Progress::Complete),
                }
            }
            _ => Err("evaluator inventory changed query families during traversal".into()),
        }
    }
}

pub(super) fn send(
    settings: &Settings,
    mut request: Outbound,
    max_pages: usize,
    checkpoint: &mut impl FnMut() -> Result<(), String>,
    output: &mut impl Write,
) -> Result<(), String> {
    let mut inventory = Inventory::start(&request)?;
    let candidate_bound = max_pages.min(settings.candidates as usize);
    let max_bytes = (protocol::REQUEST_BYTES + 1 + protocol::RESPONSE_BYTES)
        .checked_mul(candidate_bound)
        .ok_or("evaluator inventory byte allowance overflow")?;
    let mut reserved_bytes = 0_usize;
    let mut original_peer = None;
    let mut reported_pages = 0_usize;
    let result = (|| {
        for candidate in 0..max_pages {
            checkpoint()?;
            if candidate >= candidate_bound {
                return Err("original evaluator candidate allowance exhausted before inventory completion".into());
            }
            // Reserve this exchange's entire request plus maximum response and
            // overlength sentinel before connecting. Unused response capacity
            // is not recycled into more candidates or a longer invocation.
            reserved_bytes = reserved_bytes
                .checked_add(request.wire().len())
                .and_then(|bytes| bytes.checked_add(protocol::RESPONSE_BYTES + 1))
                .filter(|bytes| *bytes <= max_bytes)
                .ok_or("original evaluator inventory byte allowance exhausted")?;
            let receipt = exchange(settings, &request, &mut original_peer, checkpoint)?;
            let progress = inventory.observe(settings, &request, &receipt)?;
            checkpoint()?;
            let reported = output
                .write_all(&receipt)
                .and_then(|()| output.write_all(b"\n"))
                .and_then(|()| output.flush())
                .map_err(debug);
            // Validation and output time count against the SAME clock. A late
            // or failed write must never be followed by another native read.
            let late = checkpoint();
            reported?;
            late?;
            match progress {
                Progress::Unknown => return Err(request.incomplete().into()),
                Progress::Complete => return Ok(()),
                Progress::Continue(next) => {
                    reported_pages += 1;
                    request = next;
                }
            }
        }
        Err("explicit evaluator page bound reached with an outstanding continuation".into())
    })();
    result.map_err(|error: String| {
        format!(
            "evaluator inventory incomplete after {reported_pages} fully reported nonterminal pages: {error}; prior output is retained, with no automatic retry or new listing"
        )
    })
}
