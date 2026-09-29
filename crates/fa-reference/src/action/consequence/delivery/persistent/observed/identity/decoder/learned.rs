//! Bind original durable identity challenges to the actual owned learned model.
//! No imported probe/model, cached frame, alternate matcher or journal format.
use super::{FileDecoderIdentityProbe, FileIdentityChallenge, FileIdentityObserver,
    FileOversight, JournalError, ModelManifest};
use crate::action::consequence::activation::tensor::kv::decoder::DecoderBudget;

impl FileIdentityObserver {
    /// Select immutable parameters from THIS journal owner's original learned
    /// generation and the passport from THIS already-begun identity challenge.
    /// The caller cannot supply a surrogate model, probe, anchor subset or
    /// precomputed activations. Admission executes no token and writes nothing.
    ///
    /// The separately observed manifest is not copied from the expected passport:
    /// the original runner must record and compare it before numerical work.
    /// Digest provenance is still a trusted host input, not hardware attestation.
    ///
    /// The returned ORIGINAL runner checks actor/control/epoch/basis currentness
    /// on every step, samples receipt time around inference, and persists actual
    /// measurements through the existing identity transactions. Success installs
    /// identity eligibility only; it cannot resume a paused generator or issue
    /// either effect key. Old runners/challenges never resume after reopen.
    /// Legacy trusted manual/external-probe observation APIs remain unchanged.
    ///
    /// ```compile_fail,E0599
    /// use fa_reference::action::consequence::delivery::persistent::observed::identity::decoder::FileDecoderIdentityProbe;
    /// fn replace(run: &mut FileDecoderIdentityProbe) { run.replace_learned_model(); }
    /// ```
    pub fn learned_decoder_probe(&self, host: &FileOversight,
        challenge: &FileIdentityChallenge, measurement_sequence: u64,
        budget: DecoderBudget, observed_manifest: ModelManifest)
        -> Result<FileDecoderIdentityProbe, JournalError>
    {
        self.check(host, challenge)?;
        let expected = challenge.evidence();
        let probe = host.machine.broker.hosted_learned_identity_probe(
            expected.actor_revision(), expected.passport(), measurement_sequence, budget,
        )?;
        self.decoder_probe(host, challenge, probe, observed_manifest)
    }
}
