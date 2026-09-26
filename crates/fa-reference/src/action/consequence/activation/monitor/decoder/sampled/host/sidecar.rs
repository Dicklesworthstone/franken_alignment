//! Only the owning broker can request checked evidence from THIS decoder cache.
//! No source image, replacement model, sampler or saved report is accepted.
use super::super::MonitoredSampledDecoder;
use super::super::super::MonitoringStatus;
use crate::action::consequence::activation::monitor::learned::model::LearnedAuditPreparationBudget;
use crate::action::consequence::activation::probe::learned::{CheckedLearnedKv, ResidualRetention};
use crate::action::consequence::activation::tensor::kv::model::learned::{CompressionReport, LearnedKvCodec};
use crate::Error;

impl MonitoredSampledDecoder {
    pub(crate) fn capture_sidecar(&self, codec: &LearnedKvCodec, evaluation_origin: u64,
        retention: ResidualRetention, budget: LearnedAuditPreparationBudget)
        -> Result<(CheckedLearnedKv, CompressionReport), Error>
    {
        if self.status() != MonitoringStatus::Ready { return Err(Error::Incomplete); }
        // Capturing the original compulsory observation rejects an empty,
        // unavailable, held or failed source before any sidecar work occurs.
        self.observation().capture()?;
        let model = self.monitored.session.model();
        if codec.profile() != model.cache_profile() { return Err(Error::Binding); }
        let positions = usize::try_from(self.position()).map_err(|_| Error::Limit)?;
        let values = positions.checked_mul(model.cache_profile().values_per_token()).ok_or(Error::Overflow)?;
        if values > budget.compression.source_values || values > budget.source_check.source_values {
            return Err(Error::Limit);
        }
        let source = self.monitored.session.cache_image()?;
        let (image, compression) = codec.evaluate_held_out(evaluation_origin, &source, budget.compression)?;
        let checked = CheckedLearnedKv::new(image, &source, retention, budget.source_check)?;
        Ok((checked, compression))
    }
}
