//! Private conservation at the broker's paired containment reset. Historical
//! counters describe the restored original timeline; its remaining allowance
//! must also account for work spent on the continuation being abandoned.
use super::{GenerationBudget, GenerationTelemetryBudget, LearnedGeneration};
use crate::Error;
use std::rc::Rc;

impl LearnedGeneration {
    /// Inspect immutable parameters from THIS original generation. The existing
    /// identity engine owns fresh anchor sessions, never this live KV or sampler.
    /// Only the crate's supervised owner can select this source; no model getter
    /// or untrusted archive-to-probe constructor is introduced here.
    pub(crate) fn host_identity_probe(
        &self,
        passport: &crate::action::consequence::activation::identity::ModelPassport,
        measurement_sequence: u64,
        budget: crate::action::consequence::activation::tensor::kv::decoder::DecoderBudget,
    ) -> Result<crate::action::consequence::activation::identity::decoder::DecoderIdentityProbe, Error> {
        crate::action::consequence::activation::identity::decoder::DecoderIdentityProbe::new(
            self.model.clone(), passport, measurement_sequence, budget,
        )
    }

    /// Only a sealed, freshly audited restart reaches this seam. Restrict every
    /// remaining ceiling before the original authority transition. Repeating a
    /// rewind, including to an older checkpoint, never restores spent allowance.
    pub(crate) fn conserve_host_continuation_budget(&mut self, live: &Self) -> Result<(), Error> {
        if !Rc::ptr_eq(&self.model.data, &live.model.data)
            || self.evaluation_origin() != live.evaluation_origin()
            || self.spec.prompt() != live.spec.prompt()
            || self.spec.max_new_tokens() != live.spec.max_new_tokens()
            || self.spec.stop_tokens() != live.spec.stop_tokens()
            || self.spec.sampling().policy != live.spec.sampling().policy
            || self.spec.sampling().stream != live.spec.sampling().stream
            || self.spec.sampling().seed != live.spec.sampling().seed {
            return Err(Error::Binding);
        }
        let budget = GenerationBudget {
            decoder_products: conserve(self.work.reserved_decoder_products, self.budget.decoder_products,
                live.work.reserved_decoder_products, live.budget.decoder_products)?,
            vocabulary_scores: conserve(self.work.reserved_vocabulary_scores, self.budget.vocabulary_scores,
                live.work.reserved_vocabulary_scores, live.budget.vocabulary_scores)?,
        };
        let saved = self.telemetry_work;
        let spent = live.telemetry_work;
        let ceiling = self.telemetry_budget;
        let current = live.telemetry_budget;
        macro_rules! remaining {
            ($field:ident) => { conserve(saved.$field, ceiling.$field, spent.$field, current.$field)? };
        }
        let telemetry = GenerationTelemetryBudget {
            compression_source_values: remaining!(compression_source_values),
            compression_encoded_bytes: remaining!(compression_encoded_bytes),
            compression_work_units: remaining!(compression_work_units),
            source_check_values: remaining!(source_check_values),
            source_check_encoded_bytes: remaining!(source_check_encoded_bytes),
            source_check_reconstruction_products: remaining!(source_check_reconstruction_products),
            monitor_encoded_bytes: remaining!(monitor_encoded_bytes),
            monitor_probe_coordinates: remaining!(monitor_probe_coordinates),
            monitor_reconstruction_products: remaining!(monitor_reconstruction_products),
            monitor_materialized_values: remaining!(monitor_materialized_values),
            monitor_refinements: remaining!(monitor_refinements),
        };
        // Do not replace historical work with lifetime totals: the checkpoint's
        // token/sample/cache invariants require the exact original prefix work.
        self.budget = budget;
        self.telemetry_budget = telemetry;
        Ok(())
    }
}

fn conserve(saved_spend: u64, saved_ceiling: u64, live_spend: u64, live_ceiling: u64)
    -> Result<u64, Error>
{
    if saved_spend > live_spend { return Err(Error::Binding); }
    let saved_remaining = saved_ceiling.checked_sub(saved_spend).ok_or(Error::Binding)?;
    let live_remaining = live_ceiling.checked_sub(live_spend).ok_or(Error::Binding)?;
    saved_spend.checked_add(saved_remaining.min(live_remaining)).ok_or(Error::Overflow)
}
