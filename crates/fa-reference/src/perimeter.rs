//! Bounded reference semantics for an effect-perimeter inventory.
//!
//! This module classifies declared mediation for one exact, caller-supplied
//! scope/family/route. It is not a broker, permit, credential store, policy
//! evaluator, loader, or proof that a deployment perimeter is complete.

use crate::Error;

/// Bound the number of scoped effect-family records held by this reference model.
pub const MAX_FAMILIES: usize = 64;
/// Bound the explicitly modeled route vector for one effect family.
pub const MAX_ROUTES_PER_FAMILY: usize = 32;
/// Bound any individual identifier or residual nonclaim text.
pub const MAX_TEXT_BYTES: usize = 128;
/// Bound all owned text in one family record before it is retained.
pub const MAX_FAMILY_TEXT_BYTES: usize = 4_096;
/// Bound declared credential exposures for one family.
pub const MAX_CREDENTIALS_PER_FAMILY: usize = 16;
/// Bound explicit residual nonclaims for one family.
pub const MAX_RESIDUAL_NONCLAIMS: usize = 16;

/// The scope for which a route classification was declared. This is identity
/// data only; it cannot confer authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PerimeterScope {
    pub tenant: u64,
    pub principal: u64,
    pub purpose: u64,
}

impl PerimeterScope {
    fn validate(self) -> Result<(), Error> {
        if [self.tenant, self.principal, self.purpose].contains(&0) {
            Err(Error::InvalidInput)
        } else {
            Ok(())
        }
    }
}

/// The four trust domains named by plan §4.2. A complete family record names
/// each one rather than implying a blanket protected scalar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrustDomain {
    Actor,
    ObservationAndAnalysis,
    Enforcement,
    GovernanceAndInvestigation,
}

/// Where one named credential is exposed for a family. `ActorDirect` is a
/// declared residual path, not a brokered credential.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CredentialExposure {
    pub credential: String,
    pub holder: CredentialHolder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialHolder {
    ActorDirect,
    Broker,
}

/// A bounded threat classification. A missing classification is represented by
/// `None` in `RouteRecord` so callers cannot mistake absence for a class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreatClass {
    GuardrailRemoval,
    PromptInjection,
    PayloadSubstitution,
    DirectCredentialOrEgress,
    UnknownRemoteOutcome,
}

/// Whether an identified bypass route is blocked by the declared mediation,
/// remains an explicit residual, or has not been modeled at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BypassDisposition {
    Blocked,
    ResidualUncovered,
    Unmodeled,
}

/// The declared mediation classification, never a permit or prevention claim
/// beyond the relevant checked inventory record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mediation {
    ObserveOnly,
    CooperativeGate,
    BrokeredEffects,
}

/// One exact route in a family's mediation vector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteRecord {
    pub route: String,
    pub threat: Option<ThreatClass>,
    pub mediation: Mediation,
    pub bypass: BypassDisposition,
}

/// The complete checked perimeter declaration for one exact scope and effect
/// family. Its residual nonclaims are mandatory because a bounded inventory is
/// not itself a completed deployment perimeter.
///
/// This initial profile has family-wide credential exposure: when any route is
/// brokered, no `ActorDirect` credential may appear anywhere in the family and
/// at least one broker credential is required. Mixed route credential profiles
/// need a later explicit contract; this model does not infer one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectFamilyRecord {
    pub scope: PerimeterScope,
    pub family: String,
    pub trust_domains: Vec<TrustDomain>,
    pub credentials: Vec<CredentialExposure>,
    pub routes: Vec<RouteRecord>,
    pub residual_nonclaims: Vec<String>,
}

/// An immutable, bounded inventory. It performs no I/O and has no external
/// effect; future broker-policy code may consume only its classifications.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PerimeterInventory {
    records: Vec<EffectFamilyRecord>,
}

impl PerimeterInventory {
    /// Validate a complete in-memory declaration before retaining it.
    pub fn new(records: Vec<EffectFamilyRecord>) -> Result<Self, Error> {
        if records.is_empty() {
            return Err(Error::InvalidInput);
        }
        if records.len() > MAX_FAMILIES {
            return Err(Error::Limit);
        }

        for (index, record) in records.iter().enumerate() {
            validate_record(record)?;
            if records[..index]
                .iter()
                .any(|prior| prior.scope == record.scope && prior.family == record.family)
            {
                return Err(Error::Duplicate);
            }
        }
        Ok(Self { records })
    }

    /// Return the immutable complete records for inspection. These records are
    /// declarations, not evidence that a real broker or endpoint exists.
    pub fn records(&self) -> &[EffectFamilyRecord] {
        &self.records
    }

    /// Look up one exact immutable declared route, retaining its threat and
    /// bypass disposition for consumers that need more than classification.
    pub fn route_for(
        &self,
        scope: PerimeterScope,
        family: &str,
        route: &str,
    ) -> Result<&RouteRecord, Error> {
        scope.validate()?;
        validate_text(family)?;
        validate_text(route)?;
        let record = self
            .records
            .iter()
            .find(|record| record.scope == scope && record.family == family)
            .ok_or(Error::Missing)?;
        record
            .routes
            .iter()
            .find(|candidate| candidate.route == route)
            .ok_or(Error::Missing)
    }

    /// Convenience access to a route's declared classification only.
    ///
    /// `ObserveOnly` and `CooperativeGate` remain their own legitimate
    /// classifications; this method never upgrades either to brokered coverage.
    pub fn mediation_for(
        &self,
        scope: PerimeterScope,
        family: &str,
        route: &str,
    ) -> Result<Mediation, Error> {
        Ok(self.route_for(scope, family, route)?.mediation)
    }
}

fn validate_record(record: &EffectFamilyRecord) -> Result<(), Error> {
    record.scope.validate()?;
    validate_text(&record.family)?;
    if record.routes.is_empty() || record.residual_nonclaims.is_empty() {
        return Err(Error::Incomplete);
    }
    if record.routes.len() > MAX_ROUTES_PER_FAMILY
        || record.credentials.len() > MAX_CREDENTIALS_PER_FAMILY
        || record.residual_nonclaims.len() > MAX_RESIDUAL_NONCLAIMS
    {
        return Err(Error::Limit);
    }
    if record.trust_domains.len() != 4 {
        return Err(Error::Incomplete);
    }
    for domain in [
        TrustDomain::Actor,
        TrustDomain::ObservationAndAnalysis,
        TrustDomain::Enforcement,
        TrustDomain::GovernanceAndInvestigation,
    ] {
        if record
            .trust_domains
            .iter()
            .filter(|seen| **seen == domain)
            .count()
            != 1
        {
            return Err(Error::Incomplete);
        }
    }

    let mut total_text = record.family.len();
    for credential in &record.credentials {
        validate_text(&credential.credential)?;
        total_text = add_text(total_text, credential.credential.len())?;
    }
    for nonclaim in &record.residual_nonclaims {
        validate_text(nonclaim)?;
        total_text = add_text(total_text, nonclaim.len())?;
    }
    for (index, route) in record.routes.iter().enumerate() {
        validate_text(&route.route)?;
        total_text = add_text(total_text, route.route.len())?;
        if route.threat.is_none() {
            return Err(Error::Incomplete);
        }
        if record.routes[..index]
            .iter()
            .any(|prior| prior.route == route.route)
        {
            return Err(Error::Duplicate);
        }
        if route.bypass == BypassDisposition::Unmodeled {
            return Err(if route.mediation == Mediation::BrokeredEffects {
                Error::Binding
            } else {
                Error::Incomplete
            });
        }
        if route.mediation == Mediation::BrokeredEffects
            && route.bypass != BypassDisposition::Blocked
        {
            return Err(Error::Binding);
        }
    }
    for (index, credential) in record.credentials.iter().enumerate() {
        if record.credentials[..index]
            .iter()
            .any(|prior| prior.credential == credential.credential)
        {
            return Err(Error::Duplicate);
        }
    }

    if record
        .routes
        .iter()
        .any(|route| route.mediation == Mediation::BrokeredEffects)
    {
        if record
            .credentials
            .iter()
            .any(|credential| credential.holder == CredentialHolder::ActorDirect)
        {
            return Err(Error::Binding);
        }
        if !record
            .credentials
            .iter()
            .any(|credential| credential.holder == CredentialHolder::Broker)
        {
            return Err(Error::Incomplete);
        }
    }
    Ok(())
}

fn validate_text(text: &str) -> Result<(), Error> {
    if text.is_empty() {
        return Err(Error::InvalidInput);
    }
    if text.len() > MAX_TEXT_BYTES {
        return Err(Error::Limit);
    }
    Ok(())
}

fn add_text(total: usize, added: usize) -> Result<usize, Error> {
    let total = total.checked_add(added).ok_or(Error::Overflow)?;
    if total > MAX_FAMILY_TEXT_BYTES {
        Err(Error::Limit)
    } else {
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope() -> PerimeterScope {
        PerimeterScope {
            tenant: 1,
            principal: 2,
            purpose: 3,
        }
    }

    fn complete_record(mediation: Mediation) -> EffectFamilyRecord {
        EffectFamilyRecord {
            scope: scope(),
            family: "publication".to_owned(),
            trust_domains: vec![
                TrustDomain::Actor,
                TrustDomain::ObservationAndAnalysis,
                TrustDomain::Enforcement,
                TrustDomain::GovernanceAndInvestigation,
            ],
            credentials: if mediation == Mediation::BrokeredEffects {
                vec![CredentialExposure {
                    credential: "publish-token".to_owned(),
                    holder: CredentialHolder::Broker,
                }]
            } else {
                Vec::new()
            },
            routes: vec![RouteRecord {
                route: "adapter:disposable".to_owned(),
                threat: Some(ThreatClass::DirectCredentialOrEgress),
                mediation,
                bypass: BypassDisposition::Blocked,
            }],
            residual_nonclaims: vec!["No live perimeter claim".to_owned()],
        }
    }

    #[test]
    fn complete_exact_brokered_route_is_classified_without_authority() {
        let inventory =
            PerimeterInventory::new(vec![complete_record(Mediation::BrokeredEffects)]).unwrap();

        assert_eq!(
            inventory
                .mediation_for(scope(), "publication", "adapter:disposable")
                .unwrap(),
            Mediation::BrokeredEffects
        );
        assert_eq!(inventory.records().len(), 1);
        assert_eq!(inventory.records()[0].scope, scope());
    }

    #[test]
    fn observe_and_cooperative_records_are_not_upgraded_to_brokered() {
        let observe =
            PerimeterInventory::new(vec![complete_record(Mediation::ObserveOnly)]).unwrap();
        assert_eq!(
            observe
                .mediation_for(scope(), "publication", "adapter:disposable")
                .unwrap(),
            Mediation::ObserveOnly
        );

        let cooperative =
            PerimeterInventory::new(vec![complete_record(Mediation::CooperativeGate)]).unwrap();
        assert_eq!(
            cooperative
                .mediation_for(scope(), "publication", "adapter:disposable")
                .unwrap(),
            Mediation::CooperativeGate
        );
    }

    #[test]
    fn nonbrokered_residuals_remain_visible_and_unmodeled_bypasses_refuse() {
        for mediation in [Mediation::ObserveOnly, Mediation::CooperativeGate] {
            let mut residual = complete_record(mediation);
            residual.routes[0].bypass = BypassDisposition::ResidualUncovered;
            let inventory = PerimeterInventory::new(vec![residual.clone()]).unwrap();

            let route = inventory
                .route_for(scope(), "publication", "adapter:disposable")
                .unwrap();
            assert_eq!(route.mediation, mediation);
            assert_eq!(route.threat, Some(ThreatClass::DirectCredentialOrEgress));
            assert_eq!(route.bypass, BypassDisposition::ResidualUncovered);
            assert_eq!(
                inventory
                    .mediation_for(scope(), "publication", "adapter:disposable")
                    .unwrap(),
                mediation
            );

            let mut unmodeled = residual;
            unmodeled.routes[0].bypass = BypassDisposition::Unmodeled;
            assert_eq!(
                PerimeterInventory::new(vec![unmodeled]),
                Err(Error::Incomplete)
            );
        }
    }

    #[test]
    fn missing_threat_classification_refuses_next_to_a_classified_control() {
        let allowed = complete_record(Mediation::BrokeredEffects);
        assert!(PerimeterInventory::new(vec![allowed.clone()]).is_ok());

        let mut missing = allowed;
        missing.routes[0].threat = None;
        assert_eq!(
            PerimeterInventory::new(vec![missing]),
            Err(Error::Incomplete)
        );
    }

    #[test]
    fn unknown_route_or_scope_refuses_next_to_an_exact_control() {
        let inventory =
            PerimeterInventory::new(vec![complete_record(Mediation::BrokeredEffects)]).unwrap();
        assert_eq!(
            inventory.mediation_for(scope(), "publication", "adapter:unknown"),
            Err(Error::Missing)
        );
        assert_eq!(
            inventory.route_for(scope(), "publication", "adapter:unknown"),
            Err(Error::Missing)
        );
        assert_eq!(
            inventory.mediation_for(
                PerimeterScope {
                    principal: 9,
                    ..scope()
                },
                "publication",
                "adapter:disposable",
            ),
            Err(Error::Missing)
        );
    }

    #[test]
    fn actor_direct_credential_refuses_next_to_broker_credential_control() {
        let allowed = complete_record(Mediation::BrokeredEffects);
        assert!(PerimeterInventory::new(vec![allowed.clone()]).is_ok());

        let mut direct = allowed;
        direct.credentials[0].holder = CredentialHolder::ActorDirect;
        assert_eq!(PerimeterInventory::new(vec![direct]), Err(Error::Binding));
    }

    #[test]
    fn unmodeled_bypass_refuses_next_to_blocked_bypass_control() {
        let allowed = complete_record(Mediation::BrokeredEffects);
        assert!(PerimeterInventory::new(vec![allowed.clone()]).is_ok());

        let mut unmodeled = allowed;
        unmodeled.routes[0].bypass = BypassDisposition::Unmodeled;
        assert_eq!(
            PerimeterInventory::new(vec![unmodeled]),
            Err(Error::Binding)
        );
    }

    #[test]
    fn count_and_text_bounds_refuse_before_retaining_records() {
        let record = complete_record(Mediation::BrokeredEffects);
        let too_many = vec![record.clone(); MAX_FAMILIES + 1];
        assert_eq!(PerimeterInventory::new(too_many), Err(Error::Limit));

        let mut too_many_routes = record.clone();
        let route = too_many_routes.routes[0].clone();
        too_many_routes.routes = vec![route; MAX_ROUTES_PER_FAMILY + 1];
        assert_eq!(
            PerimeterInventory::new(vec![too_many_routes]),
            Err(Error::Limit)
        );

        let mut oversized = record;
        oversized.family = "x".repeat(MAX_TEXT_BYTES + 1);
        assert_eq!(PerimeterInventory::new(vec![oversized]), Err(Error::Limit));
    }

    #[test]
    fn aggregate_record_text_bound_refuses_many_individually_bounded_fields() {
        let mut record = complete_record(Mediation::BrokeredEffects);
        record.credentials = (0..MAX_CREDENTIALS_PER_FAMILY)
            .map(|index| CredentialExposure {
                credential: format!("{index:02}{}", "c".repeat(MAX_TEXT_BYTES - 2)),
                holder: CredentialHolder::Broker,
            })
            .collect();
        record.residual_nonclaims = (0..MAX_RESIDUAL_NONCLAIMS)
            .map(|index| format!("{index:02}{}", "r".repeat(MAX_TEXT_BYTES - 2)))
            .collect();

        assert_eq!(PerimeterInventory::new(vec![record]), Err(Error::Limit));
    }
}
