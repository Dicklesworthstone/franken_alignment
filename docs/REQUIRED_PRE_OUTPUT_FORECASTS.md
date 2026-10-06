# Journal-enforced pre-output forecasting

Source implementation; compilation and execution are not yet qualified. This
opts the original durable learned-text owner into the FA-111 timing boundary
(plan 12.7), rather than relying on the caller to keep using an optional handle.
It adds no predictor, likelihood process, generator, request book or event tag.

## Bootstrap and admission

Configure the original predictor with `with_hosted_residual(layer)` followed by
`with_pre_output_forecast()`, then install it before numerical work. Version 5
wraps the exact original hosted configuration, optionally its matching stream
message domain. The original terminal-stop wrapper remains outermost; applying
these options in either order produces the same bytes. Versions 1 through 4
retain their old meaning and encoding. Unknown/truncated/nested modes cannot
fall back to a weaker configuration. Installation requires the original learned
TEXT owner and exact agreement on raw-message versus stream semantics.

Both ordinary and cooperative learned Begin/Step operations now check the
requirement in the original Machine. Prompt processing needs no prediction.
Continuation needs an acknowledged, request-bound forecast from the exact end
of that prompt, unexpired in the original clock/authority epoch. Source sequence,
actor revision and actual current/cumulative sampling counts must agree. A
checkpoint cannot hide abandoned draws or rebind an old forecast to a new actor
revision. Lost coverage and expired forecasts cannot execute another sample,
even when its Begin was already acknowledged. That pending intent is retained,
not refunded or falsely completed. The existing optional FilePreOutputForecast
handle is compatible, but retaining that handle is not what enforces the rule.

Supplied-frame and unkeyed forecasts cannot replace the required request binding.
A request cannot answer its forecast before original output completion, nor
supply a different payload as the observed category. Original request retries,
likelihood counting, policy refusals, congress and both effect keys remain in
the original implementations. All timing checks also run on semantic replay;
relabeling a legacy sample-before-forecast journal as this mode is invalid.

## Streams and limits

For the matching stream-message domain, the prompt forecast belongs to Message.
Only receipt confirmation of that exact append makes a later hosted forecast
available for Finish. That later forecast cannot authorize a second Message,
and dispatch or visible-but-unreconciled publication does not count as receipt
confirmation. Finish still needs its own review and both original effect keys.
No new numerical step is run just to obtain its residual, and the strictly
increasing source-sequence rule is unchanged.

This is a before-sampling contract for the registered complete-output category,
not proof of conditional calibration, model independence, maliciousness or
latency bounds. Enforcement is in FileOversight's original Machine, not a new
claim about unrelated in-memory brokers or legacy configurations. Recovery
retains the requirement and pending-forecast coverage loss; no replacement role,
error budget, sampled continuation or old approval key is returned.

## Verification debt

Nine regression functions cover codec preservation/canonical composition,
malformed configuration, original warm/cooperative/replayed admission,
exact legacy numerical/witness parity, early/substituted output, required-mode
replay of invalid chronology, expiry/loss between writes, optional-handle use
and bootstrap limits with working controls. They are authored tests, not runs.
The mandatory targeted and full RCH commands cannot start in this environment
because rch is absent. Rust compilation, tests, rustfmt and Clippy remain
unexecuted; no local compiler fallback or qualification/bead closure is claimed.

## Required-mode consumer and failure coverage

Five additional functions use the original native helper models and journal
storage: permitting versus predictive-crossing and native-denial controls;
message publication followed by a separately forecast/reviewed stream finish;
unknown dispatch recovery with no evidence read or generation resume; all five
forecast/intent/completion write barriers; and prepared completion catch-up on
both sides of the exact forecast-expiry boundary. Finish forecasts refuse
before dispatch, after dispatch and after visible but unreconciled publication.
The old message frame cannot consume the later finish forecast. The message's
human key cannot authorize finish. No existing assertion is weakened or removed.
These bring the authored total to fourteen regression functions; all remain
unexecuted pending RCH and the full mandatory gate on the final revision.
