# Durable source-bound learned text

## The same original output constraint after recovery

`FileLearnedConfig::new_text` binds the completed-message profile to the existing
learned-generation journal. It composes the original byte-BPE constructor and
`FileLearnedConfig::new` numerical-recipe encoder. No weights, codec, tokenizer,
cache, sampler, score or saved verdict from disk is installed as executable
state. Recovery still requires the entire independently supplied runtime recipe,
compares its bytes first, and reconstructs the original owner by actual replay.
This connects the source-derived publication behavior to the existing durable
recovery path (plan sections 8, 10.15 and 11.1–11.2).

The configuration retains the complete original numerical recipe, exact
canonical tokenizer vocabulary/ranks/named controls, original UTF-8 prompt,
tokenization limits, output-byte limit and completion policy. Matching token
IDs, model labels or decoded prompt text is insufficient. Changing an output
token's spelling without changing its ID must reject recovery. `StopRequired`
cannot silently become `StopOrTokenLimit`, and a text recipe cannot be opened
with the otherwise identical generic numerical recipe.

`FileOversight::create_with_learned_text` validates the configuration through
original replay before creating storage, then writes a FIRST canonical image
that already includes its text restriction and final-publication guard. No
empty generic intermediate journal is published. It performs no inference or
clock observation. An incompatible numerical-only recipe refuses before a
publication directory is created. The caller still observes current time before
stepping or requesting effects.

The original `enable_learned_generation`, `open_with_learned_generation` and
`read_publication_with_learned_generation` APIs consume this configuration.
Only its private installation selection changes: text configurations construct
the original text-constrained broker owner, while numerical configurations keep
the original generic owner. Constructors perform tokenization and admission,
not prompt/sample inference. The full original actor-cache horizon and all
numerical/telemetry allowances remain enforced by their existing owners.

## Existing write-ahead and publication transitions

Every numerical step still has the original durable intent before computation
and exact original-state witness before acknowledgment. A crash between these
transitions cannot publish the previously accepted prefix as completed text.
In particular, a final stop token with only a persisted intent is not an end
marker. Recovery preserves that pending step; resume cannot skip it, change its
random state, or erase its already recorded work. Holds and preparation failures
remain their original outcomes after reconstruction, not fresh quiet owners.

`FileOversight::learned_text_message` exposes only acknowledged output after
fresh clock observation and explicit numerical resume. It refuses while the
owner is poisoned, paused, interrupted by a pending step, held or unfinished.
It delegates decoding, UTF-8, control-token and completion checks to the original
broker source. Its result is an immutable observation, not publication or a key.

`FileOversight::propose_learned_text` accepts destination and policy prerequisites,
not payload bytes. It derives the exact completed output and uses the original
durable proposal operation. Generic durable `propose` is equally constrained
because every reconstructed broker installs the same original text gate.
Independent congress review, human approval and final checked publication are
still mandatory. A changed or incomplete payload cannot bypass the restriction
through another proposal entry point.

Historical publication projection is deliberately separate from live text
capture. An executed receipt may be read while numerics are paused. A dispatch
with no terminal receipt remains charged after recovery until the original
endpoint reconciliation/seal machinery establishes its real outcome. Neither
text unavailability nor recreation of a source creates new keys or refunds an
unknown effect. A prior key's issuer is not revived by replay.

## Format, cost and scope

Numerical-only configuration bytes remain byte-for-byte unchanged. Text recipes
use a disjoint extension inside the existing opaque configuration payload:
`FALBOOT/1`, zero in the formerly nonzero monitor-generation slot, `FALTEXT/1`,
then length-framed original numerical recipe, canonical tokenizer, exact prompt,
limits and a completion-policy tag. The complete payload has the same original
maximum size. Outer journal event tags and step witnesses are unchanged.
An old numerical-only implementation cannot independently construct a matching
text recipe, so it refuses rather than interpreting the extension as a generic
numerical configuration. No parser permissiveness or fallback is added.

This profile retains and re-encodes full recipes and replays numerical history
through the existing journal owner. Tokenization, encoding, allocations, cache
copying and repeated replay have costs; there is no constant-time recovery,
measured speedup, incremental-storage or peak-memory claim. A constructor error
can follow bounded tokenization without a completed-cost receipt. The existing
local canonical-file durability assumptions, exclusive owner, anti-rollback
limits and trusted clock/provisioning obligations are unchanged. This adds no
CLI mode, remote provider, helper authentication, learned-sidecar persistence,
qualified detector, or production release.

## Authored verification; execution pending

Nine Unix integration tests use original learned inference, actual native
byte-BPE, original journal replacement and original two-key publication. They
cover every empty/prompt/sample/terminal recovery cut; canonical-tokenizer
reconstruction and same-ID changed-spelling rejection; prompt/budget/completion
mismatches; a pending final stop; old keys and successful checked publication;
executed and unknown outcomes while paused; actual monitoring holds and aggregate
telemetry failure; finite-horizon/undecodable output; legacy generic recovery; and first-image startup with no synthetic clock or
token.
The changed-spelling control also runs the original model and observes the
changed output, rather than merely comparing two configuration labels.
Tiny weights and reference ballots are synthetic test inputs, not detector or
independent-human-quality evidence. Existing numerical, authority and journal
test bodies are unchanged.

All nine new tests are authored, not executed. Compilation, all Rust tests, rustfmt and Clippy remain UNEXECUTED here. The fresh
targeted test and complete xtask commands were attempted through remote-only
RCH; both stopped before compilation because `rch` is absent (exit 127). No local
compilation fallback, gate relaxation, release qualification or Beads closure
is claimed. Source/hash checks do not establish runtime correctness.
