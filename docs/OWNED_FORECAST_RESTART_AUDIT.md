# Owned K/V forecasts after a prompt-boundary reset

An explicitly selected native learned-text recipe can now restore a checkpoint
at the **complete original prompt boundary**, forecast from a fresh audit of the
actual restored cache, and generate its first sampled token afterward. This
closes the ordering gap where a restored owner had no current accepted event,
the predictor required that event, and the next token required a forecast first.

This is a native, single-message learned-text capability. The current learned
operator command uses cumulative text streams; it does not select this profile.
Predictive stream resets and resets after any sampled work remain unsupported.
There is no new calibration claim, reset permission, role, or effect shortcut.

## Independent recipe selection

Build the original text recipe and mandatory sidecar, pin its owned K/V
pre-output predictor, then select both source-successor and fresh-audit behavior:

```rust,ignore
let generation = FileLearnedConfig::new_text(model, tokenizer, source, limits)?
    .with_required_sidecar()?
    .with_required_owned_pre_output_forecast(predictor)?
    .with_forecast_reset_successors()?
    .with_owned_forecast_restart_audit()?;
```

The final wrapper binds `FALKRFA\x01` and the complete preceding recipe bytes.
Recovery must supply the exact independently retained recipe and predictor.
Omitting the wrapper, switching to raw residual prediction, or changing its
order cannot reinterpret an existing history. Every legacy recipe retains its
original bytes, one-audit reset, and accepted-token source requirements.

The profile accepts a quiet checkpoint only at the complete original prompt,
with zero sampled work and a `functional_restart` or stronger registered host
profile. The existing paired checkpoint API remains unchanged. The original
control predecessor, actor revision, authority epoch, retained target ceiling,
review binding, incident counter, and restart budget still govern restoration.

## Fresh observation without a sampled-token dependency

The original checkpoint reset first audits the retained checkpoint cache under
its original stream and restores the exact numerical cache and sampler. The
new profile then performs a **second** compression, source check, and complete
all-layer K/V monitor pass over the restored owner's actual cache. That cache
names the successor stream. A saved observation is never relabeled as evidence
of the new stream, and the earlier accepted generation event is not restored.

The new audit is available only inside the original owned K/V forecast path.
The ordinary `LearnedObservation` remains `Empty` until a newly accepted token
publishes its usual current evidence. The forecast uses the last row of the
fresh complete prefix, fixed by the original predictor's layer and K/V side.
Its source includes the complete retained audit inventory, so the usual
prediction job, numerical work, retained-source bytes, age, actor cut, and
sequence checks all apply. The first actual generation step clears the private
restart-audit source before computation, including on failure or unwind.

After a fresh forecast, ordinary sampled generation, mandatory sidecar review,
native helper commitment/reveal, human approval, one-use authorization,
dispatch, publication validation, and reconciliation still run. A reset receipt
or fresh forecast alone cannot publish anything.

## Costs, failures, and statistical boundaries

Both audits use the request's per-audit allowance intersected with the original
monitor policy. The reset reserves **two** such allowances before either audit
runs. Failed attempts retain both reservations; completed reports contribute
their actual costs to the original `HostedLearnedRecoveryUsage` totals. Restored
cache bytes and original generation budgets keep their existing accounting.
No prompt inference or random draw is repeated by the new audit.

`HostedLearnedResetReceipt::restored_forecast_audit` retains the second report.
For this recipe only, the reset comparison witness uses `FALRST\0\x02` and binds
both complete audit reports and their actual source descriptors. The ordinary
reset witness remains version 1. Original semantic replay recomputes both
audits and compares them; no archived report supplies a live source.

Pending forecasts, lost coverage, crossed likelihood evidence, unreported
prediction work, and **any cumulative sampling** refuse this reset path before
new reset work. Predictor lifetime counters and the likelihood process never
rewind. In particular, restoring the same cache and RNG after observed output
could reproduce the same event; this profile does not count it again or hide it
behind a neutral likelihood factor. Post-sampling reset support requires a
separate, complete same-origin statistical contract.

The existing write-ahead intent, canonical replacement, recovery fence,
independent guard checks, role custody, and exact operation retry remain in
force. A completed retry does not run either audit again or add another
incident. Recovery still requires fresh time and explicit original resume.

## Verification scope

The new regression sources in `tests/forecast_reset_successors/restart_audit.rs`
exercise actual prompt capture, reset recovery, successor-cache audits,
pre-sampling owned prediction, original sampled output, native model review,
human approval, publication, and independent journal replay. Causal controls
retain the legacy prompt-boundary refusal, reject missing/raw recipe selection,
reject pending forecasts and already answered sampled history, restrict capture
to the original prompt boundary, and retain both reservations on audit failure.

These are source regression additions pending the repository's required RCH
gate. No Rust compilation, test execution, conditional-calibration validation,
restart-grade qualification, deployment containment, or reset-secrecy claim is
made by this change.
