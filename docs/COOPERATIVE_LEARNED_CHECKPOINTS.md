# Cooperative paired learned checkpoints

Scoped FA-108 work (plan 11.10, 11.1 and 9.8): preserve an original quiet
numerical/authority pair for rewind containment without monopolizing the
publication supervisor during historical replay. This does not close FA-108
or qualify the behavioral claims of H17.

`FileOversight::prepare_learned_checkpoint` creates a fresh checkpoint task.
The checkpoint ID, actor revision, position and authority epoch are fixed.
`advance` verifies a bounded number of original journal transitions; `catch_up`
explicitly adopts the same live owner's appended source/time/control tail only
while the original quiet-capture laws still hold. Neither operation samples,
captures a checkpoint, writes a journal, nor returns authority. The existing
learned preparation cursor performs replay and retains failure/unwind latches.
There is no second replay implementation or externally supplied candidate.

`finish` captures the original pair on that fully verified cut and acknowledges
its original Capture witness through the unchanged persistence boundary. It
returns the existing owner-bound checkpoint handle only after acknowledgment.
No second history replay occurs. The captured control sequence is the final
verified sequence, not an unverified claim about the initial preparation cut.
The actor revision, position and authority epoch cannot be silently replaced.

The live replay cache is not taken during cold preparation. Supervisory source,
clock and stop work remains possible between quanta. Inference advancement,
pending numerical/reset work, pause, fencing, source loss, a foreign/recovered
owner or exhausted ordinary capacity cannot be repaired by increasing the
revision. A dropped preparation writes nothing and cannot keep the owner's
Store lock alive. Existing capture retries retain their original historical-ID
behavior; a new preparation is not a recapture API.

Checkpoint count/bytes and ordinary journal capacity remain on the original
owners. The terminal-only reserve cannot fund a new capture. Existing witness
encoding, two publication keys, effect liabilities, revocation and reset
incident rules are unchanged. Capturing a checkpoint does not reset, renew a
forecast, qualify a monitor, or authorize publication.

Six regression functions compare cooperative Capture bytes against the
original synchronous API, verify recovered pair identity and unchanged numerical
state, exercise exact event quanta and explicit clock-tail adoption, reject
stale/foreign/pending/fenced preparations, retain writer-lock lifetime, check
ordinary exhaustion with terminal capacity still available, and inject all five
original storage barriers. The sixth passes a cooperatively captured pair into
the original reset engine, checks deterministic resumed samples and logits, and
retains dispatched/published charges, revoked old keys and the out-of-band
incident count through reset and disk recovery. Two compile-fail doctests prohibit cloning and
sampling. Fixture models establish causal controls, not detector calibration.

All new Rust tests are authored but UNEXECUTED. The required remote-only RCH
launcher is absent, as are cargo, rustc and rustfmt. Source hash and whitespace
checks are not compilation, formatting, tests or qualification. A replay event,
checkpoint construction, encoding or storage operation can be synchronous and
expensive; the event quantum is not a hard time, memory or throughput bound.


## Scope boundary

The original paired-checkpoint mechanism excludes learned text-stream owners
and audit-only restart profiles. This implementation preserves both exclusions.
It serves already-supported numerical checkpoint owners and does not add an
operator flag to `create-learned-generated`. An attempted operator integration
was excluded after source review found the unconditional stream-capture refusal
in `OversightBroker::capture_hosted_learned_checkpoint`. Making it work would
require a separately defined stream-frontier checkpoint/restore contract; a
flag or quieter history replay is not that implementation.

The incompatible operator draft is not included. Rust execution remains pending;
selected-file hash and whitespace checks are not a complete workspace check.
No repository gate or bead is closed, and neither old tests nor authority
boundaries were relaxed.
