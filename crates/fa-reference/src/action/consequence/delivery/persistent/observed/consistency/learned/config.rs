//! Frozen source selection and limits for the original learned forecast lane.
use super::*;
use super::super::FileConsistencyConfig;
use crate::action::consequence::oversight::consistency::LearnedConsistencyConfig;

pub(super) const MAX_CONFIG_BYTES: usize = super::super::config::MAX_CONFIG_BYTES + 128;
const DOMAIN: &[u8; 8] = b"FALCPRD\x01";
const OWNED_DOMAIN: &[u8; 8] = b"FALCPRD\x02";
const PRE_OUTPUT_DOMAIN: &[u8; 8] = b"FALCPRD\x03";

/// Numerical/source policy, not calibration evidence or an effect capability.
/// The original raw configuration bytes and all five per-job/lifetime caps are
/// retained exactly. Hosted-residual and pre-output profiles require their own
/// owned-source contract and cannot be relabelled as supplied learned captures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileLearnedConsistencyConfig {
    consistency: FileConsistencyConfig,
    owned_generation: bool,
    pre_output: bool,
    layer: u64,
    side: KvSide,
    per_job: LearnedMonitorBudget,
    lifetime: LearnedMonitorBudget,
    retained_source_bytes: usize,
    bytes: Rc<[u8]>,
}
impl FileLearnedConsistencyConfig {
    pub fn new(consistency: FileConsistencyConfig, layer: u64, side: KvSide,
        per_job: LearnedMonitorBudget, lifetime: LearnedMonitorBudget,
        max_retained_source_bytes: usize) -> Result<Self, Error>
    {
        if layer == 0 { return Err(Error::InvalidInput); }
        if consistency.hosted_residual_layer().is_some() || consistency.requires_pre_output_forecast() {
            return Err(Error::Binding);
        }
        // The original constructor also rejects scalar progressive ladders and
        // invalid per-job limits. A lifetime ceiling is not a per-job allowance.
        consistency.build()?.model.into_learned(per_job)?;
        let mut w = Writer::new(MAX_CONFIG_BYTES);
        w.raw(DOMAIN)?; w.blob(consistency.encoded())?; w.u64(layer)?;
        write_side(&mut w, side)?;
        write_budget(&mut w, per_job)?; write_budget(&mut w, lifetime)?;
        write_count(&mut w, max_retained_source_bytes)?;
        Ok(Self { consistency, owned_generation: false, pre_output: false, layer, side, per_job, lifetime,
            retained_source_bytes: max_retained_source_bytes, bytes: Rc::from(w.finish()) })
    }
    /// Immutable source selection, validated against the actual owned generator
    /// at Enable. Does not install a host or loosen the original pre-output rules.
    pub fn with_owned_generation(mut self) -> Result<Self, Error> {
        if self.owned_generation { return Err(Error::Duplicate); }
        let mut bytes = self.bytes.to_vec();
        bytes[..OWNED_DOMAIN.len()].copy_from_slice(OWNED_DOMAIN);
        self.bytes = Rc::from(bytes);
        self.owned_generation = true;
        Ok(self)
    }
    pub fn uses_owned_generation(&self) -> bool { self.owned_generation }

    /// Require an acknowledged request-bound prompt forecast before the original
    /// generator may sample. This is immutable durable configuration, not a
    /// handle-based calling convention or a raw-residual source substitution.
    /// Select owned generation first. Actual text/stream/model admission occurs
    /// at Enable, before any numerical work; legacy source modes stay unchanged.
    pub fn with_pre_output_forecast(mut self) -> Result<Self, Error> {
        if !self.owned_generation { return Err(Error::Binding); }
        if self.pre_output { return Err(Error::Duplicate); }
        let mut bytes = self.bytes.to_vec();
        bytes[..PRE_OUTPUT_DOMAIN.len()].copy_from_slice(PRE_OUTPUT_DOMAIN);
        self.bytes = Rc::from(bytes);
        self.pre_output = true;
        Ok(self)
    }
    pub fn requires_pre_output_forecast(&self) -> bool { self.pre_output }

    pub fn encoded(&self) -> &[u8] { &self.bytes }
    pub fn consistency(&self) -> &FileConsistencyConfig { &self.consistency }
    pub fn lifetime_budget(&self) -> LearnedMonitorBudget { self.lifetime }
    pub fn max_retained_source_bytes(&self) -> usize { self.retained_source_bytes }

    pub(in super::super::super) fn native(&self) -> Result<LearnedConsistencyConfig, Error> {
        Ok(LearnedConsistencyConfig { consistency: self.consistency.build()?,
            layer: self.layer, side: self.side, budget: self.per_job })
    }
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_CONFIG_BYTES { return Err(Error::Limit); }
        let mut r = Reader::new(bytes);
        let domain = r.take(DOMAIN.len())?;
        if domain != DOMAIN && domain != OWNED_DOMAIN && domain != PRE_OUTPUT_DOMAIN { return Err(Error::Binding); }
        let consistency = FileConsistencyConfig::from_bytes(r.blob(super::super::config::MAX_CONFIG_BYTES)?)?;
        let layer = r.u64()?; let side = read_side(&mut r)?;
        let per_job = read_budget(&mut r)?; let lifetime = read_budget(&mut r)?;
        let retained = read_count(&mut r)?; r.end()?;
        let config = Self::new(consistency, layer, side, per_job, lifetime, retained)?;
        let config = if domain != DOMAIN { config.with_owned_generation()? } else { config };
        let config = if domain == PRE_OUTPUT_DOMAIN { config.with_pre_output_forecast()? } else { config };
        if config.encoded() != bytes { return Err(Error::Binding); }
        Ok(config)
    }
}
fn write_budget(w: &mut Writer, b: LearnedMonitorBudget) -> Result<(), Error> {
    write_count(w, b.encoded_bytes)?; write_count(w, b.probe_coordinates)?;
    w.u64(b.reconstruction_products)?; write_count(w, b.materialized_values)?;
    write_count(w, b.refinements)
}
fn read_budget(r: &mut Reader<'_>) -> Result<LearnedMonitorBudget, Error> {
    Ok(LearnedMonitorBudget { encoded_bytes: read_count(r)?, probe_coordinates: read_count(r)?,
        reconstruction_products: r.u64()?, materialized_values: read_count(r)?, refinements: read_count(r)? })
}
fn write_count(w: &mut Writer, n: usize) -> Result<(), Error> {
    w.u64(u64::try_from(n).map_err(|_| Error::Overflow)?)
}
fn read_count(r: &mut Reader<'_>) -> Result<usize, Error> {
    usize::try_from(r.u64()?).map_err(|_| Error::Limit)
}
