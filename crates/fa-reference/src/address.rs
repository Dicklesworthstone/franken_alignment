//! Canonical typed addresses for the registered FrankenAlignment noun set.
//!
//! This is a bounded reference parser/printer, not an authorization resolver.
//! Parsing an address proves only syntactic membership in the registered scheme.
use crate::Error;
use core::fmt;

pub const PREFIX: &str = "fa://";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AddressKind {
    Run,
    Attempt,
    Effect,
    Permit,
    Closure,
    Receipt,
    Round,
    Consequence,
    Helper,
    Evidence,
    View,
    Witness,
    Frontier,
    Observation,
    Passport,
    Policy,
    Profile,
    Grade,
    Fence,
    Branch,
    Checkpoint,
    Incident,
    Finding,
    Ledger,
    Annotation,
    Handoff,
}

impl AddressKind {
    pub const ALL: [Self; 26] = [
        Self::Run, Self::Attempt, Self::Effect, Self::Permit, Self::Closure,
        Self::Receipt, Self::Round, Self::Consequence, Self::Helper, Self::Evidence,
        Self::View, Self::Witness, Self::Frontier, Self::Observation, Self::Passport,
        Self::Policy, Self::Profile, Self::Grade, Self::Fence, Self::Branch,
        Self::Checkpoint, Self::Incident, Self::Finding, Self::Ledger,
        Self::Annotation, Self::Handoff,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Run => "run",
            Self::Attempt => "attempt",
            Self::Effect => "effect",
            Self::Permit => "permit",
            Self::Closure => "closure",
            Self::Receipt => "receipt",
            Self::Round => "round",
            Self::Consequence => "consequence",
            Self::Helper => "helper",
            Self::Evidence => "evidence",
            Self::View => "view",
            Self::Witness => "witness",
            Self::Frontier => "frontier",
            Self::Observation => "observation",
            Self::Passport => "passport",
            Self::Policy => "policy",
            Self::Profile => "profile",
            Self::Grade => "grade",
            Self::Fence => "fence",
            Self::Branch => "branch",
            Self::Checkpoint => "checkpoint",
            Self::Incident => "incident",
            Self::Finding => "finding",
            Self::Ledger => "ledger",
            Self::Annotation => "annotation",
            Self::Handoff => "handoff",
        }
    }

    pub fn parse(value: &str) -> Result<Self, Error> {
        match value {
            "run" => Ok(Self::Run),
            "attempt" => Ok(Self::Attempt),
            "effect" => Ok(Self::Effect),
            "permit" => Ok(Self::Permit),
            "closure" => Ok(Self::Closure),
            "receipt" => Ok(Self::Receipt),
            "round" => Ok(Self::Round),
            "consequence" => Ok(Self::Consequence),
            "helper" => Ok(Self::Helper),
            "evidence" => Ok(Self::Evidence),
            "view" => Ok(Self::View),
            "witness" => Ok(Self::Witness),
            "frontier" => Ok(Self::Frontier),
            "observation" => Ok(Self::Observation),
            "passport" => Ok(Self::Passport),
            "policy" => Ok(Self::Policy),
            "profile" => Ok(Self::Profile),
            "grade" => Ok(Self::Grade),
            "fence" => Ok(Self::Fence),
            "branch" => Ok(Self::Branch),
            "checkpoint" => Ok(Self::Checkpoint),
            "incident" => Ok(Self::Incident),
            "finding" => Ok(Self::Finding),
            "ledger" => Ok(Self::Ledger),
            "annotation" => Ok(Self::Annotation),
            "handoff" => Ok(Self::Handoff),
            _ => Err(Error::InvalidInput),
        }
    }
}

impl fmt::Display for AddressKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A canonical address in the normative `fa://<tenant>/<kind>/<id>[@generation]`
/// scheme. Numeric IDs are deliberately narrow in the reference model. A future
/// production resolver may map richer external identifiers before construction.
///
/// This value carries identity, not authority. It never authorizes lookup,
/// disclosure, mutation, or cross-tenant existence leakage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Address {
    tenant: u64,
    kind: AddressKind,
    id: u64,
    generation: Option<u64>,
}

impl Address {
    pub fn new(tenant: u64, kind: AddressKind, id: u64, generation: Option<u64>)
        -> Result<Self, Error>
    {
        if tenant == 0 || id == 0 || generation == Some(0) {
            return Err(Error::InvalidInput);
        }
        Ok(Self { tenant, kind, id, generation })
    }

    pub fn parse(value: &str) -> Result<Self, Error> {
        if value.len() > 256 || !value.is_ascii() || !value.starts_with(PREFIX) {
            return Err(Error::InvalidInput);
        }
        let body = &value[PREFIX.len()..];
        if body.bytes().any(|byte| byte.is_ascii_whitespace()) {
            return Err(Error::InvalidInput);
        }
        let mut parts = body.split('/');
        let tenant = decimal(parts.next().ok_or(Error::InvalidInput)?)?;
        let kind = AddressKind::parse(parts.next().ok_or(Error::InvalidInput)?)?;
        let tail = parts.next().ok_or(Error::InvalidInput)?;
        if parts.next().is_some() || tail.is_empty() {
            return Err(Error::InvalidInput);
        }
        let mut id_generation = tail.split('@');
        let id = decimal(id_generation.next().ok_or(Error::InvalidInput)?)?;
        let generation = match id_generation.next() {
            Some(value) => Some(decimal(value)?),
            None => None,
        };
        if id_generation.next().is_some() {
            return Err(Error::InvalidInput);
        }
        Self::new(tenant, kind, id, generation)
    }

    pub const fn tenant(self) -> u64 { self.tenant }
    pub const fn kind(self) -> AddressKind { self.kind }
    pub const fn id(self) -> u64 { self.id }
    pub const fn generation(self) -> Option<u64> { self.generation }

    pub fn with_generation(self, generation: u64) -> Result<Self, Error> {
        Self::new(self.tenant, self.kind, self.id, Some(generation))
    }

    pub const fn without_generation(self) -> Self {
        Self { generation: None, ..self }
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{PREFIX}{}/{}/{}", self.tenant, self.kind, self.id)?;
        if let Some(generation) = self.generation {
            write!(f, "@{generation}")?;
        }
        Ok(())
    }
}

fn decimal(value: &str) -> Result<u64, Error> {
    if value.is_empty() || value.starts_with('0') || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Error::InvalidInput);
    }
    value.parse::<u64>().map_err(|_| Error::InvalidInput)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_registered_noun_round_trips_canonically() {
        for (index, kind) in AddressKind::ALL.into_iter().enumerate() {
            let id = u64::try_from(index + 1).unwrap();
            let plain = Address::new(7, kind, id, None).unwrap();
            let generated = plain.with_generation(11).unwrap();
            assert_eq!(Address::parse(&plain.to_string()), Ok(plain));
            assert_eq!(Address::parse(&generated.to_string()), Ok(generated));
            assert_eq!(generated.without_generation(), plain);
        }
    }

    #[test]
    fn unknown_kind_and_noncanonical_forms_are_refused() {
        for value in [
            "", "fa:/1/attempt/1", "FA://1/attempt/1", "fa://1/action/1",
            "fa://1/attempt", "fa://1/attempt/1/extra", "fa://1//1",
            "fa://01/attempt/1", "fa://1/attempt/01", "fa://1/attempt/1@01",
            "fa://0/attempt/1", "fa://1/attempt/0", "fa://1/attempt/1@0",
            "fa://1/attempt/1@", "fa://1/attempt/1@2@3", "fa://1/attempt/-1",
            "fa://1/attempt/+1", "fa://1/attempt/%31", "fa://1/attempt/1?x=2",
            "fa://1/attempt/1#x", "fa://1/attempt/1 ", "fa://1/attempt/18446744073709551616",
        ] {
            assert_eq!(Address::parse(value), Err(Error::InvalidInput), "{value}");
        }
    }

    #[test]
    fn address_identity_keeps_tenant_kind_id_and_generation_distinct() {
        let base = Address::parse("fa://9/attempt/4@2").unwrap();
        assert_eq!((base.tenant(), base.kind(), base.id(), base.generation()),
            (9, AddressKind::Attempt, 4, Some(2)));
        assert_ne!(base, Address::parse("fa://8/attempt/4@2").unwrap());
        assert_ne!(base, Address::parse("fa://9/effect/4@2").unwrap());
        assert_ne!(base, Address::parse("fa://9/attempt/5@2").unwrap());
        assert_ne!(base, Address::parse("fa://9/attempt/4@3").unwrap());
    }
}
