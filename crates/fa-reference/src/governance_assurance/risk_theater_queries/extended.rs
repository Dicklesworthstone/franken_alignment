//! Typed comparative predicates for FA-125's bounded reference history.
//!
//! These facts are assertions by the independently selected source. They are
//! not authenticated native journal events, a governed policy installation, or
//! permission to close an incident or change coverage. The query derives its
//! measurements from original records; callers cannot submit a finding score.
//! Every coordinate below is in the enclosing history's CONTROL-sequence domain.
//! Missing premises preserve incompleteness alongside independently supported
//! positive findings. No native Tower capability is activated by this module.

use super::{
    FamilyEvaluation, HistoryFact, HistoryRecord, QueryError, QueryFinding, find_record,
    resolve_assessment_chain, validate_address, validate_family, MAX_QUERY_RECORDS,
};
use crate::action::consequence::Consequence;
use crate::address_resolver::{Address, Noun};
use crate::governance_assurance::RiskTheaterFamily;
use std::collections::{BTreeMap, BTreeSet};

/// Exact rational policy threshold, never a rounded percentage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rate {
    pub numerator: u64,
    pub denominator: u64,
}

/// Immutable parameters of this reference comparison, not native authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtendedQueryPolicy {
    pub minimum_continue_increase: Rate,
    pub minimum_canary_escape: Rate,
    pub minimum_rounds_per_window: u64,
    pub minimum_canaries: u64,
    pub andon_roster_closure_threshold: u64,
}

impl Default for ExtendedQueryPolicy {
    fn default() -> Self {
        Self {
            minimum_continue_increase: Rate { numerator: 1, denominator: 4 },
            minimum_canary_escape: Rate { numerator: 1, denominator: 4 },
            minimum_rounds_per_window: 1,
            minimum_canaries: 1,
            andon_roster_closure_threshold: 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ComparisonWindow {
    Baseline,
    Current,
}

/// Original sample identity and its registered sampling/campaign lineage.
/// Repeating an origin under a different round does not create another sample.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanarySource {
    pub lineage: Address,
    pub origin: Address,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanaryOutcome {
    Contained,
    Escaped,
    Censored,
}

/// A window closes an occurrence interval [start, end), not a label deadline.
/// A delayed assessment belongs to the original round's window. Baseline and
/// current windows compare exactly one effect/helper/policy-generation group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConsequenceFact {
    Round {
        effect_family: String,
        helper_generation: Address,
        policy_generation: Address,
        consequence: Consequence,
        canary: Option<CanarySource>,
    },
    Window {
        effect_family: String,
        helper_generation: Address,
        policy_generation: Address,
        role: ComparisonWindow,
        start: u64,
        end: u64,
    },
    CanaryAssessment {
        round: Address,
        outcome: CanaryOutcome,
        evaluator: Address,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IncidentClosureMode {
    Evidence,
    Policy,
    Roster,
}

/// A separately observed disposition distinguishes a known open incident from
/// a declared closure whose original record is unavailable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IncidentDisposition {
    Open,
    Closed { closure: Address },
}

/// One bounded reference Andon ledger per query scope. Windows do not overlap
/// and are registered no later than their start; a later label cannot install
/// a retrospective budget. Costs use each window's original fixed per-stop cost. A false
/// stop references its original Round; arrival time cannot move its window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AndonFact {
    Window {
        start: u64,
        end: u64,
        false_stops_allowed: u64,
        cost_per_stop: u64,
    },
    FalseStop {
        window: Address,
        round: Address,
        evaluator: Address,
    },
    Incident {
        window: Address,
        opened_by: Address,
    },
    Disposition {
        incident: Address,
        state: IncidentDisposition,
    },
    Closure {
        incident: Address,
        mode: IncidentClosureMode,
        authorization: Address,
    },
    /// The source asserts this exact authorizing transition. The query checks
    /// its subject, mode and causal order, not its signature or actual authority.
    Authorization {
        incident: Address,
        mode: IncidentClosureMode,
    },
}

/// Only this registered mediation dimension has a total order. It does not
/// impose an order on capture contracts, modalities or arbitrary coverage data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MediationLevel {
    ObserveOnly,
    Cooperative,
    Brokered,
    Attested,
}

/// States are occurrence-ordered observations of effective coverage, not
/// arbitrary predeclared profile definitions. Changes form one active chain per
/// effect family; the first observed Change declares that chain's baseline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoverageFact {
    State {
        effect_family: String,
        mediation: MediationLevel,
        capture_contract: Address,
        channels: BTreeSet<String>,
    },
    Change {
        effect_family: String,
        previous: Address,
        current: Address,
        authorization: Option<Address>,
    },
    /// Must precede the exact Change it names. Merely knowing a receipt for some
    /// other profile transition never makes this transition fully observed.
    Authorization {
        change: Address,
        previous: Address,
        current: Address,
    },
}

/// Derived identities of an opening already validated at an accepted source
/// boundary, not another source or permission. Retractions cannot move that
/// opening to a later confirmation. Replaying the original source history
/// reconstructs these bindings without any external cache or native claim.
///
/// There are at most MAX_QUERY_RECORDS incident bindings, each containing at
/// most MAX_QUERY_RECORDS distinct source addresses. These are existing lifetime
/// source identities; retiring a record cannot allocate a new identity here.
#[derive(Clone, Debug, Default)]
pub(super) struct AndonOpeningProvenance {
    bindings: BTreeMap<Address, OpeningSupport>,
}

#[derive(Clone, Debug)]
struct OpeningSupport {
    window: Address,
    opened_by: Address,
    sources: BTreeSet<Address>,
}

impl AndonOpeningProvenance {
    fn missing_sources(
        &self,
        incident: &Address,
        window: &Address,
        opened_by: &Address,
        records: &[HistoryRecord],
    ) -> Result<bool, QueryError> {
        let Some(original) = self.bindings.get(incident) else {
            return Ok(false);
        };
        if &original.window != window || &original.opened_by != opened_by {
            return Err(QueryError::InvalidReference);
        }
        Ok(original.sources.iter().any(|address| find_record(records, address).is_none()))
    }

    fn retain(
        &mut self,
        incident: &Address,
        window: &Address,
        opened_by: &Address,
        prefix: &[&HistoryRecord],
    ) -> Result<(), QueryError> {
        if self.bindings.contains_key(incident) {
            return Ok(());
        }
        if self.bindings.len() >= MAX_QUERY_RECORDS {
            return Err(QueryError::ResourceLimit);
        }
        let mut sources = BTreeSet::from([window.clone()]);
        for confirmation in prefix {
            let HistoryFact::Andon(AndonFact::FalseStop { round, .. }) = &confirmation.fact else {
                return Err(QueryError::InvalidReference);
            };
            sources.insert(confirmation.address.clone());
            sources.insert(round.clone());
        }
        if sources.len() > MAX_QUERY_RECORDS {
            return Err(QueryError::ResourceLimit);
        }
        self.bindings.insert(incident.clone(), OpeningSupport {
            window: window.clone(), opened_by: opened_by.clone(), sources,
        });
        Ok(())
    }
}

/// The original occurrence order of coverage transitions already admitted at a
/// closed source boundary. A withdrawn transition is not live evidence, but it
/// cannot turn its successor into a fresh baseline or permit a stale fork.
///
/// At most MAX_QUERY_RECORDS transitions are retained. Their identities and
/// endpoints are drawn from the existing lifetime source inventory. This is
/// derived provenance, not an additional source, permission or completeness
/// assertion; replaying accepted source history reconstructs the same spine.
#[derive(Clone, Debug, Default)]
pub(super) struct CoveragePredecessorProvenance {
    transitions: BTreeMap<Address, CoverageTransition>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CoverageTransition {
    effect_family: String,
    occurred_at: u64,
    previous: Address,
    current: Address,
}

impl CoveragePredecessorProvenance {
    fn retain(&mut self, records: &[HistoryRecord]) -> Result<(), QueryError> {
        for record in records {
            let HistoryFact::Coverage(CoverageFact::Change {
                effect_family, previous, current, ..
            }) = &record.fact else {
                continue;
            };
            let transition = CoverageTransition {
                effect_family: effect_family.clone(),
                occurred_at: record.occurred_at,
                previous: previous.clone(),
                current: current.clone(),
            };
            if let Some(original) = self.transitions.get(&record.address) {
                if original != &transition {
                    return Err(QueryError::InvalidReference);
                }
            } else {
                if self.transitions.len() >= MAX_QUERY_RECORDS {
                    return Err(QueryError::ResourceLimit);
                }
                self.transitions.insert(record.address.clone(), transition);
            }
        }
        Ok(())
    }
}

pub(super) fn validate_policy(policy: &ExtendedQueryPolicy) -> Result<(), QueryError> {
    for rate in [policy.minimum_continue_increase, policy.minimum_canary_escape] {
        if rate.numerator == 0 || rate.denominator == 0 || rate.numerator > rate.denominator {
            return Err(QueryError::InvalidPolicy);
        }
    }
    if policy.minimum_rounds_per_window == 0
        || policy.minimum_canaries == 0
        || policy.andon_roster_closure_threshold == 0
    {
        return Err(QueryError::InvalidPolicy);
    }
    Ok(())
}

fn reference(address: &Address, scope: &Address, kind: Noun) -> Result<(), QueryError> {
    validate_address(address, scope)?;
    if address.kind != kind {
        return Err(QueryError::InvalidReference);
    }
    Ok(())
}

fn record_kind(record: &HistoryRecord, kind: Noun) -> Result<(), QueryError> {
    if record.address.kind != kind {
        return Err(QueryError::InvalidRecord);
    }
    Ok(())
}

pub(super) fn validate_fact(record: &HistoryRecord, scope: &Address) -> Result<(), QueryError> {
    match &record.fact {
        HistoryFact::Consequence(ConsequenceFact::Round {
            effect_family, helper_generation, policy_generation, ..
        })
        | HistoryFact::Consequence(ConsequenceFact::Window {
            effect_family, helper_generation, policy_generation, ..
        }) => {
            validate_family(effect_family)?;
            reference(helper_generation, scope, Noun::Helper)?;
            reference(policy_generation, scope, Noun::Policy)?;
            match &record.fact {
                HistoryFact::Consequence(ConsequenceFact::Round { canary, .. }) => {
                    record_kind(record, Noun::Round)?;
                    if let Some(canary) = canary {
                        reference(&canary.lineage, scope, Noun::Profile)?;
                        reference(&canary.origin, scope, Noun::Observation)?;
                    }
                }
                HistoryFact::Consequence(ConsequenceFact::Window { start, end, .. }) => {
                    record_kind(record, Noun::Frontier)?;
                    if *start == 0 || start >= end || *end > record.occurred_at {
                        return Err(QueryError::InvalidRecord);
                    }
                }
                _ => unreachable!("matched consequence source"),
            }
        }
        HistoryFact::Consequence(ConsequenceFact::CanaryAssessment { round, evaluator, .. }) => {
            record_kind(record, Noun::Evidence)?;
            reference(round, scope, Noun::Round)?;
            validate_address(evaluator, scope)?;
        }
        HistoryFact::Andon(fact) => match fact {
            AndonFact::Window { start, end, .. } => {
                record_kind(record, Noun::Ledger)?;
                if *start == 0 || start >= end || record.occurred_at > *start {
                    return Err(QueryError::InvalidRecord);
                }
            }
            AndonFact::FalseStop { window, round, evaluator } => {
                record_kind(record, Noun::Evidence)?;
                reference(window, scope, Noun::Ledger)?;
                reference(round, scope, Noun::Round)?;
                validate_address(evaluator, scope)?;
            }
            AndonFact::Incident { window, opened_by } => {
                record_kind(record, Noun::Incident)?;
                reference(window, scope, Noun::Ledger)?;
                reference(opened_by, scope, Noun::Evidence)?;
            }
            AndonFact::Disposition { incident, state } => {
                record_kind(record, Noun::Observation)?;
                reference(incident, scope, Noun::Incident)?;
                if let IncidentDisposition::Closed { closure } = state {
                    reference(closure, scope, Noun::Receipt)?;
                }
            }
            AndonFact::Closure { incident, authorization, .. } => {
                record_kind(record, Noun::Receipt)?;
                reference(incident, scope, Noun::Incident)?;
                reference(authorization, scope, Noun::Consequence)?;
            }
            AndonFact::Authorization { incident, .. } => {
                record_kind(record, Noun::Consequence)?;
                reference(incident, scope, Noun::Incident)?;
            }
        },
        HistoryFact::Coverage(fact) => match fact {
            CoverageFact::State { effect_family, capture_contract, channels, .. } => {
                record_kind(record, Noun::Profile)?;
                validate_family(effect_family)?;
                reference(capture_contract, scope, Noun::Profile)?;
                if channels.len() > 64 {
                    return Err(QueryError::ResourceLimit);
                }
                for channel in channels {
                    validate_family(channel)?;
                }
            }
            CoverageFact::Change { effect_family, previous, current, authorization } => {
                record_kind(record, Noun::Consequence)?;
                validate_family(effect_family)?;
                reference(previous, scope, Noun::Profile)?;
                reference(current, scope, Noun::Profile)?;
                if previous == current {
                    return Err(QueryError::InvalidRecord);
                }
                if let Some(authorization) = authorization {
                    reference(authorization, scope, Noun::Receipt)?;
                }
            }
            CoverageFact::Authorization { change, previous, current } => {
                record_kind(record, Noun::Receipt)?;
                reference(change, scope, Noun::Consequence)?;
                reference(previous, scope, Noun::Profile)?;
                reference(current, scope, Noun::Profile)?;
                if previous == current {
                    return Err(QueryError::InvalidRecord);
                }
            }
        },
        _ => {}
    }
    Ok(())
}

pub(super) fn evaluate(
    policy: &ExtendedQueryPolicy,
    records: &[HistoryRecord],
    _frontier: u64,
    evaluation_owner: &Address,
    andon_openings: &mut AndonOpeningProvenance,
    coverage_predecessors: &mut CoveragePredecessorProvenance,
) -> Result<Vec<FamilyEvaluation>, QueryError> {
    Ok(vec![
        consequence_drift(policy, records, evaluation_owner)?,
        andon_closures(policy, records, evaluation_owner, andon_openings)?,
        coverage_erosion(records, coverage_predecessors)?,
    ])
}

type Group = (String, Address, Address);

#[derive(Default)]
struct Comparison<'a> {
    rounds: Vec<&'a HistoryRecord>,
    windows: BTreeMap<ComparisonWindow, &'a HistoryRecord>,
}

fn group(effect_family: &str, helper: &Address, policy: &Address) -> Group {
    (effect_family.to_owned(), helper.clone(), policy.clone())
}

fn multiply(left: u128, right: u128) -> Result<u128, QueryError> {
    left.checked_mul(right).ok_or(QueryError::ArithmeticOverflow)
}

fn enough(numerator: u64, denominator: u64, threshold: Rate) -> Result<bool, QueryError> {
    if denominator == 0 {
        return Err(QueryError::IncompleteHistory);
    }
    Ok(multiply(u128::from(numerator), u128::from(threshold.denominator))?
        >= multiply(u128::from(threshold.numerator), u128::from(denominator))?)
}

fn increased(
    prior_yes: u64, prior_all: u64, current_yes: u64, current_all: u64, threshold: Rate,
) -> Result<bool, QueryError> {
    if prior_all == 0 || current_all == 0 {
        return Err(QueryError::IncompleteHistory);
    }
    let current = multiply(u128::from(current_yes), u128::from(prior_all))?;
    let prior = multiply(u128::from(prior_yes), u128::from(current_all))?;
    let Some(difference) = current.checked_sub(prior) else { return Ok(false); };
    Ok(multiply(difference, u128::from(threshold.denominator))?
        >= multiply(
            multiply(u128::from(threshold.numerator), u128::from(prior_all))?,
            u128::from(current_all),
        )?)
}

fn consequence_drift(
    policy: &ExtendedQueryPolicy,
    records: &[HistoryRecord],
    evaluation_owner: &Address,
) -> Result<FamilyEvaluation, QueryError> {
    let family = RiskTheaterFamily::ConsequenceDrift;
    let mut groups: BTreeMap<Group, Comparison<'_>> = BTreeMap::new();
    let mut complete = true;
    let mut origins = BTreeSet::new();
    let mut assessments = BTreeMap::<&Address, Vec<_>>::new();
    for record in records {
        match &record.fact {
            HistoryFact::Consequence(ConsequenceFact::Round {
                effect_family, helper_generation, policy_generation, canary, ..
            }) => {
                if let Some(source) = canary {
                    if !origins.insert(source.origin.clone()) {
                        return Err(QueryError::ConflictingRecord);
                    }
                }
                groups.entry(group(effect_family, helper_generation, policy_generation))
                    .or_default().rounds.push(record);
            }
            HistoryFact::Consequence(ConsequenceFact::Window {
                effect_family, helper_generation, policy_generation, role, ..
            }) => {
                if groups.entry(group(effect_family, helper_generation, policy_generation))
                    .or_default().windows.insert(*role, record).is_some()
                {
                    return Err(QueryError::ConflictingRecord);
                }
            }
            HistoryFact::Consequence(ConsequenceFact::CanaryAssessment { round, evaluator, outcome }) => {
                if evaluator != evaluation_owner { complete = false; }
                assessments.entry(round).or_default().push((record, evaluator, *outcome));
            }
            _ => {}
        }
    }
    // Check each retained chain even if its original round is missing or the
    // round is outside the selected windows. A correction may withdraw the
    // round, but cannot make two independent final answers consistent.
    let mut resolved = BTreeMap::new();
    for (round, labels) in assessments {
        let original_sequence = match find_record(records, round) {
            Some(record) if matches!(
                &record.fact, HistoryFact::Consequence(ConsequenceFact::Round { canary: Some(_), .. })
            ) => Some(record.occurred_at),
            Some(_) => return Err(QueryError::InvalidReference),
            None => {
                complete = false;
                None
            }
        };
        let assessment = resolve_assessment_chain(
            original_sequence, labels, evaluation_owner, CanaryOutcome::Censored,
        )?;
        resolved.insert(round, assessment);
    }
    if groups.is_empty() {
        complete = false;
    }
    let mut findings = Vec::new();
    for comparison in groups.values() {
        let (Some(prior), Some(current)) = (
            comparison.windows.get(&ComparisonWindow::Baseline),
            comparison.windows.get(&ComparisonWindow::Current),
        ) else {
            complete = false;
            continue;
        };
        let bounds = |record: &HistoryRecord| match &record.fact {
            HistoryFact::Consequence(ConsequenceFact::Window { start, end, .. }) => (*start, *end),
            _ => unreachable!("window index"),
        };
        let (prior_start, prior_end) = bounds(prior);
        let (current_start, current_end) = bounds(current);
        if prior_end > current_start || prior.occurred_at >= current.occurred_at {
            return Err(QueryError::ConflictingRecord);
        }
        let mut prior_all = 0u64;
        let mut prior_yes = 0u64;
        let mut current_all = 0u64;
        let mut current_yes = 0u64;
        let mut canaries = 0u64;
        let mut escapes = 0u64;
        let mut lineage = None;
        let mut ready = true;
        let mut support = vec![prior.address.clone(), current.address.clone()];
        for round in &comparison.rounds {
            let HistoryFact::Consequence(ConsequenceFact::Round { consequence, canary, .. }) = &round.fact else {
                unreachable!("round index");
            };
            let in_prior = prior_start <= round.occurred_at && round.occurred_at < prior_end;
            let in_current = current_start <= round.occurred_at && round.occurred_at < current_end;
            if in_prior {
                prior_all = prior_all.checked_add(1).ok_or(QueryError::ArithmeticOverflow)?;
                if *consequence == Consequence::Continue {
                    prior_yes = prior_yes.checked_add(1).ok_or(QueryError::ArithmeticOverflow)?;
                }
                support.push(round.address.clone());
            }
            if in_current {
                current_all = current_all.checked_add(1).ok_or(QueryError::ArithmeticOverflow)?;
                if *consequence == Consequence::Continue {
                    current_yes = current_yes.checked_add(1).ok_or(QueryError::ArithmeticOverflow)?;
                }
                support.push(round.address.clone());
                if let Some(source) = canary {
                    if lineage.as_ref().is_some_and(|old| old != &source.lineage) {
                        ready = false;
                    } else {
                        lineage = Some(source.lineage.clone());
                    }
                    canaries = canaries.checked_add(1).ok_or(QueryError::ArithmeticOverflow)?;
                    let Some((outcome, labels)) = resolved.get(&round.address) else {
                        ready = false;
                        continue;
                    };
                    support.extend(labels.iter().cloned());
                    match outcome {
                        Some(CanaryOutcome::Escaped) => {
                            escapes = escapes.checked_add(1).ok_or(QueryError::ArithmeticOverflow)?;
                        }
                        Some(CanaryOutcome::Contained) => {}
                        _ => ready = false,
                    }
                }
            }
        }
        if prior_all < policy.minimum_rounds_per_window
            || current_all < policy.minimum_rounds_per_window
            || canaries < policy.minimum_canaries
            || !ready
        {
            complete = false;
            continue;
        }
        if increased(prior_yes, prior_all, current_yes, current_all, policy.minimum_continue_increase)?
            && enough(escapes, canaries, policy.minimum_canary_escape)?
        {
            findings.push(QueryFinding {
                family,
                subject: current.address.clone(),
                support,
                reason: "continue_drift_with_canary_escape",
            });
        }
    }
    Ok(FamilyEvaluation { family, complete, findings })
}

fn andon_closures(
    policy: &ExtendedQueryPolicy,
    records: &[HistoryRecord],
    evaluation_owner: &Address,
    openings: &mut AndonOpeningProvenance,
) -> Result<FamilyEvaluation, QueryError> {
    let family = RiskTheaterFamily::AndonClosureMode;
    let windows: Vec<_> = records.iter().filter(|record| matches!(
        record.fact, HistoryFact::Andon(AndonFact::Window { .. })
    )).collect();
    let mut complete = !windows.is_empty();
    let mut seen_rounds = BTreeSet::new();
    let mut by_window: BTreeMap<Address, Vec<&HistoryRecord>> = BTreeMap::new();
    let mut unavailable_windows = BTreeSet::new();
    for (index, window) in windows.iter().enumerate() {
        let HistoryFact::Andon(AndonFact::Window { start, end, .. }) = &window.fact else {
            unreachable!("window selection");
        };
        for other in &windows[..index] {
            let HistoryFact::Andon(AndonFact::Window { start: other_start, end: other_end, .. }) = &other.fact else {
                unreachable!("window selection");
            };
            if start < other_end && other_start < end {
                return Err(QueryError::ConflictingRecord);
            }
        }
    }
    // Every original round contributes once, even if a caller invents another
    // confirmation address or tries to charge it to a later arrival window.
    for record in records {
        let HistoryFact::Andon(AndonFact::FalseStop { window, round, evaluator }) = &record.fact else { continue; };
        if !seen_rounds.insert(round.clone()) {
            return Err(QueryError::ConflictingRecord);
        }
        let Some(selected) = find_record(records, window) else {
            complete = false;
            continue;
        };
        let HistoryFact::Andon(AndonFact::Window { start, end, .. }) = &selected.fact else {
            return Err(QueryError::InvalidReference);
        };
        let Some(original) = find_record(records, round) else {
            complete = false;
            unavailable_windows.insert(window.clone());
            continue;
        };
        let HistoryFact::Consequence(ConsequenceFact::Round { consequence, .. }) = &original.fact else {
            return Err(QueryError::InvalidReference);
        };
        if *consequence != Consequence::HoldEffect
            || record.occurred_at <= original.occurred_at
            || record.occurred_at <= selected.occurred_at
            || original.occurred_at < *start
            || original.occurred_at >= *end
        {
            return Err(QueryError::InvalidReference);
        }
        if evaluator != evaluation_owner {
            complete = false;
            unavailable_windows.insert(window.clone());
            continue;
        }
        by_window.entry(window.clone()).or_default().push(record);
    }
    // Validate every available causal link, even before a later disposition
    // selects it. An orphan reference is incomplete; an observed contradiction
    // is refused instead of being hidden by a still-open/missing disposition.
    let mut closures = BTreeSet::new();
    let mut dispositions = BTreeMap::<Address, Vec<_>>::new();
    for record in records {
        match &record.fact {
            HistoryFact::Andon(AndonFact::Incident { window, opened_by }) => {
                match find_record(records, window) {
                    Some(selected) if matches!(&selected.fact, HistoryFact::Andon(AndonFact::Window { .. })) => {}
                    Some(_) => return Err(QueryError::InvalidReference),
                    None => complete = false,
                }
                match find_record(records, opened_by) {
                    Some(opening) => match &opening.fact {
                        HistoryFact::Andon(AndonFact::FalseStop { window: selected, .. })
                            if selected == window && opening.occurred_at <= record.occurred_at => {}
                        _ => return Err(QueryError::InvalidReference),
                    },
                    None => {
                        complete = false;
                        unavailable_windows.insert(window.clone());
                    }
                }
            }
            HistoryFact::Andon(AndonFact::Disposition { incident, state }) => {
                dispositions.entry(incident.clone()).or_default().push((record, state));
                match find_record(records, incident) {
                    Some(selected) if matches!(&selected.fact, HistoryFact::Andon(AndonFact::Incident { .. }))
                        && selected.occurred_at <= record.occurred_at => {}
                    Some(_) => return Err(QueryError::InvalidReference),
                    None => complete = false,
                }
                if let IncidentDisposition::Closed { closure } = state {
                    match find_record(records, closure) {
                        Some(closed) => match &closed.fact {
                            HistoryFact::Andon(AndonFact::Closure { incident: selected, .. })
                                if selected == incident && closed.occurred_at <= record.occurred_at => {}
                            _ => return Err(QueryError::InvalidReference),
                        },
                        None => complete = false,
                    }
                }
            }
            HistoryFact::Andon(AndonFact::Closure { incident, mode, authorization }) => {
                if !closures.insert(incident.clone()) {
                    return Err(QueryError::ConflictingRecord);
                }
                match find_record(records, incident) {
                    Some(selected) if matches!(&selected.fact, HistoryFact::Andon(AndonFact::Incident { .. }))
                        && selected.occurred_at <= record.occurred_at => {}
                    Some(_) => return Err(QueryError::InvalidReference),
                    None => complete = false,
                }
                match find_record(records, authorization) {
                    Some(authorized) => match &authorized.fact {
                        HistoryFact::Andon(AndonFact::Authorization { incident: selected, mode: approved })
                            if selected == incident && approved == mode
                                && authorized.occurred_at < record.occurred_at => {}
                        _ => return Err(QueryError::InvalidReference),
                    },
                    None => complete = false,
                }
            }
            HistoryFact::Andon(AndonFact::Authorization { incident, .. }) => {
                match find_record(records, incident) {
                    Some(selected) if matches!(&selected.fact, HistoryFact::Andon(AndonFact::Incident { .. }))
                        && selected.occurred_at <= record.occurred_at => {}
                    Some(_) => return Err(QueryError::InvalidReference),
                    None => complete = false,
                }
            }
            _ => {}
        }
    }
    // An unavailable prerequisite cannot conceal an observed reopening or
    // conflicting same-boundary dispositions. Check every retained chain before
    // any per-window incompleteness shortcut, including orphan source views.
    for chain in dispositions.values_mut() {
        chain.sort_by_key(|(record, _)| record.occurred_at);
        let mut previous: Option<(u64, &IncidentDisposition)> = None;
        for (record, state) in chain.iter() {
            if let Some((sequence, prior)) = previous {
                if (sequence == record.occurred_at && prior != *state)
                    || (matches!(prior, IncidentDisposition::Closed { .. }) && prior != *state)
                {
                    return Err(QueryError::ConflictingRecord);
                }
            }
            previous = Some((record.occurred_at, *state));
        }
    }
    let mut roster_closed = Vec::new();
    for window in windows {
        let HistoryFact::Andon(AndonFact::Window { false_stops_allowed, cost_per_stop, .. }) = &window.fact else {
            unreachable!("window selection");
        };
        let mut stops = by_window.remove(&window.address).unwrap_or_default();
        stops.sort_by_key(|record| record.occurred_at);
        if stops.windows(2).any(|pair| pair[0].occurred_at == pair[1].occurred_at) {
            return Err(QueryError::ConflictingRecord);
        }
        let count = u64::try_from(stops.len()).map_err(|_| QueryError::ArithmeticOverflow)?;
        // Same full-width cost law as native Andon, including u64::MAX prices.
        multiply(u128::from(count), u128::from(*cost_per_stop))?;
        let incidents: Vec<_> = records.iter().filter(|record| matches!(
            &record.fact,
            HistoryFact::Andon(AndonFact::Incident { window: selected, .. }) if selected == &window.address
        )).collect();
        if incidents.len() > 1 {
            return Err(QueryError::ConflictingRecord);
        }
        if unavailable_windows.contains(&window.address) {
            complete = false;
            continue;
        }
        // Native Andon opens on the first confirmation reaching the allowance.
        // Zero allowance still requires an actual first false stop.
        let exhausted = count != 0 && count >= *false_stops_allowed;
        if !exhausted {
            if !incidents.is_empty() { complete = false; }
            continue;
        }
        let Some(incident) = incidents.first() else {
            complete = false;
            continue;
        };
        let crossing_index = usize::try_from(false_stops_allowed.saturating_sub(1))
            .map_err(|_| QueryError::ArithmeticOverflow)?;
        let opening = stops.get(crossing_index).ok_or(QueryError::IncompleteHistory)?;
        let HistoryFact::Andon(AndonFact::Incident { opened_by, .. }) = &incident.fact else {
            unreachable!("incident selection");
        };
        let Some(original_opening) = find_record(records, opened_by) else {
            complete = false;
            continue;
        };
        // Missing A in a formerly validated A/B prefix cannot make later C the
        // cause of B's incident. Conversely, an observed earlier crossing is a
        // contradiction even if another original prerequisite is unavailable.
        if opened_by != &opening.address && opening.occurred_at <= original_opening.occurred_at {
            return Err(QueryError::InvalidReference);
        }
        if openings.missing_sources(&incident.address, &window.address, opened_by, records)? {
            complete = false;
            continue;
        }
        if opened_by != &opening.address || incident.occurred_at < opening.occurred_at {
            return Err(QueryError::InvalidReference);
        }
        // Capture the strict prefix before any closure-mode filtering: known
        // open and evidence/policy-closed incidents need the same provenance.
        openings.retain(&incident.address, &window.address, opened_by, &stops[..=crossing_index])?;
        let Some((disposition, state)) = dispositions.get(&incident.address).and_then(|chain| chain.last()) else {
            complete = false;
            continue;
        };
        let IncidentDisposition::Closed { closure } = state else {
            // An explicitly open source observation is not a missing closure.
            if records.iter().any(|record| matches!(
                &record.fact,
                HistoryFact::Andon(AndonFact::Closure { incident: selected, .. })
                    if selected == &incident.address
            )) {
                complete = false;
            }
            continue;
        };
        let Some(closed) = find_record(records, closure) else {
            complete = false;
            continue;
        };
        let HistoryFact::Andon(AndonFact::Closure { incident: selected, mode, authorization }) = &closed.fact else {
            return Err(QueryError::InvalidReference);
        };
        if selected != &incident.address
            || closed.occurred_at < incident.occurred_at
            || closed.occurred_at > disposition.occurred_at
        {
            return Err(QueryError::InvalidReference);
        }
        let Some(authorized) = find_record(records, authorization) else {
            complete = false;
            continue;
        };
        match &authorized.fact {
            HistoryFact::Andon(AndonFact::Authorization { incident: selected, mode: approved })
                if selected == &incident.address
                    && approved == mode
                    && authorized.occurred_at >= incident.occurred_at
                    && authorized.occurred_at < closed.occurred_at => {}
            _ => return Err(QueryError::InvalidReference),
        }
        if *mode == IncidentClosureMode::Roster {
            let mut support = vec![
                window.address.clone(), incident.address.clone(), disposition.address.clone(),
                closed.address.clone(), authorized.address.clone(),
            ];
            for stop in &stops {
                support.push(stop.address.clone());
                if let HistoryFact::Andon(AndonFact::FalseStop { round, .. }) = &stop.fact {
                    support.push(round.clone());
                }
            }
            roster_closed.push((incident.address.clone(), support));
        }
    }
    let mut findings = Vec::new();
    if u64::try_from(roster_closed.len()).map_err(|_| QueryError::ArithmeticOverflow)?
        >= policy.andon_roster_closure_threshold
    {
        // Every contributor to the threshold remains in each finding's support,
        // including the exact original windows, labels and authorizing changes.
        let support: Vec<_> = roster_closed.iter().flat_map(|(_, support)| support.iter().cloned()).collect();
        for (subject, _) in roster_closed {
            findings.push(QueryFinding {
                family,
                subject,
                support: support.clone(),
                reason: "exhausted_andon_closed_by_roster",
            });
        }
    }
    Ok(FamilyEvaluation { family, complete, findings })
}

fn coverage_erosion(
    records: &[HistoryRecord],
    predecessors: &mut CoveragePredecessorProvenance,
) -> Result<FamilyEvaluation, QueryError> {
    let family = RiskTheaterFamily::CoverageErosion;
    let changes: Vec<_> = records.iter().filter(|record| matches!(
        record.fact, HistoryFact::Coverage(CoverageFact::Change { .. })
    )).collect();
    // The caller stages this spine with every other candidate result. Only a
    // fully accepted source boundary may establish a new predecessor identity.
    predecessors.retain(records)?;
    let mut complete = !predecessors.transitions.is_empty();
    let mut by_family: BTreeMap<&str, Vec<(&Address, &CoverageTransition)>> = BTreeMap::new();
    for (address, transition) in &predecessors.transitions {
        by_family.entry(transition.effect_family.as_str()).or_default()
            .push((address, transition));
        if find_record(records, address).is_none() {
            complete = false;
        }
    }
    // Preserve the original chronological chain even across a withdrawn first,
    // middle or latest transition. Missing steps make evidence incomplete; they
    // cannot erase a known predecessor and make an actually stale change lawful.
    for transitions in by_family.values_mut() {
        transitions.sort_by_key(|(_, transition)| transition.occurred_at);
        for pair in transitions.windows(2) {
            if pair[0].1.occurred_at == pair[1].1.occurred_at
                || pair[1].1.previous != pair[0].1.current
            {
                return Err(QueryError::ConflictingRecord);
            }
        }
    }
    let mut findings = Vec::new();
    for change in changes {
        let HistoryFact::Coverage(CoverageFact::Change {
            effect_family, previous, current, authorization,
        }) = &change.fact else { unreachable!("change selection"); };
        let (Some(prior), Some(next)) = (find_record(records, previous), find_record(records, current)) else {
            complete = false;
            continue;
        };
        let HistoryFact::Coverage(CoverageFact::State {
            effect_family: prior_family, mediation: prior_mediation,
            capture_contract: prior_contract, channels: prior_channels,
        }) = &prior.fact else { return Err(QueryError::InvalidReference); };
        let HistoryFact::Coverage(CoverageFact::State {
            effect_family: next_family, mediation: next_mediation,
            capture_contract: next_contract, channels: next_channels,
        }) = &next.fact else { return Err(QueryError::InvalidReference); };
        if prior_family != effect_family || next_family != effect_family
            || prior.occurred_at >= next.occurred_at || next.occurred_at > change.occurred_at
        {
            return Err(QueryError::InvalidReference);
        }
        // A finding claims the original active-predecessor contract. Its whole
        // transition prefix must be present; private provenance cannot replace
        // a withdrawn source. Earlier and unrelated supported findings survive.
        let mut support = Vec::new();
        let mut prefix_complete = true;
        for (address, predecessor) in &by_family[effect_family.as_str()] {
            if predecessor.occurred_at > change.occurred_at { break; }
            let Some(original) = find_record(records, address) else {
                prefix_complete = false;
                continue;
            };
            support.push(original.address.clone());
            let HistoryFact::Coverage(CoverageFact::Change {
                previous: from, current: to, authorization: authorized, ..
            }) = &original.fact else { return Err(QueryError::InvalidReference); };
            for address in [Some(from), Some(to), authorized.as_ref()].into_iter().flatten() {
                if find_record(records, address).is_some() { support.push(address.clone()); }
            }
        }
        match authorization.as_ref().and_then(|address| find_record(records, address)) {
            Some(authorized) => match &authorized.fact {
                HistoryFact::Coverage(CoverageFact::Authorization {
                    change: selected, previous: approved_prior, current: approved_next,
                }) if selected == &change.address && approved_prior == previous && approved_next == current
                    && authorized.occurred_at < change.occurred_at => {
                        support.push(authorized.address.clone());
                    }
                _ => return Err(QueryError::InvalidReference),
            },
            None => complete = false,
        }
        // A new capture contract has no registered comparison law here. A
        // mediation decrease is still positive evidence in its separate known
        // dimension; unchanged mediation cannot certify incomparable capture.
        let comparable_capture = prior_contract == next_contract;
        if !comparable_capture {
            complete = false;
        }
        let lost_channel = comparable_capture && !prior_channels.is_subset(next_channels);
        if prefix_complete && (next_mediation < prior_mediation || lost_channel) {
            findings.push(QueryFinding {
                family,
                subject: change.address.clone(),
                support,
                reason: "declared_coverage_dimension_lowered",
            });
        }
    }
    Ok(FamilyEvaluation { family, complete, findings })
}
