# Portable learned-generation checkpoints

Plan 11.1–11.3 and 11.6; continuation of the original `sampling::replay`
recomputation baseline. This is numerical L7 evidence, not
production recovery, effect authority, detector qualification or source authentication.

`GenerationCheckpoint::encode_archive` exports the accepted state and a V1 exact
recipe binding. `GenerationArchive::decode(bytes, &intended, limits)` requires an
independently constructed `ReplayableGeneration`: the file cannot choose its
model, fitted codec, probes, sampling recipe, stop rules or resource allowances.
The intended owner is neither advanced nor reset by import. It can be a fresh
owner in another process using independently provisioned identical inputs.

The binding includes actual parameter bits, not just IDs: all decoder weights
and profile fields; codec means, axes, training descriptors and fit report;
ordered per-layer/K/V probes and coefficients; residual-retention selection;
per-token/aggregate audit limits; prompt, stop IDs, sampler seed and policy;
capture/evaluation identities; and original generation/telemetry ceilings.
Signed zeros and unused parameters are retained. This is intentionally a full
comparison baseline, not a compact cryptographic commitment or model importer.
Archives contain sensitive model, prompt, cache and RNG material; protect them
like the original inputs. There is no encryption or authenticity claim.

The parsed type has no conversion to a trusted captured checkpoint or mutable
generation. `begin_replay` and `replay` delegate to the original `GenerationReplay`:
every prompt/sample is computed and learned-audited again, then token/sample/RNG,
cache, logit, status and work bits are compared. No saved numerical state is
installed. Partial replay exposes no candidate. Its costs are additional work;
the restored run retains its original consumed allowances and terminal stops.
An old archive cannot clear the original owner's later hold or avoid that same
alarm when the reconstructed computation reaches it.

## V1 framing and resource boundary

All integers and floating-point bits are big-endian. The outer 24-byte header is
`FALGA\0\0\x01`, recipe length (u64), state length (u64). Exact V1 recipe bytes follow,
then the state. No trailing bytes or alternate version is accepted. Recipe bytes
are compared directly against the independently provisioned inputs, never parsed
into executable configuration.

The state has 352 fixed bytes: mandatory component mask 63, logical byte length,
position count, status tag and stop ID, token/sample/logit/cache lengths, twelve
numerical-work words, eleven telemetry-work words, and the existing 96-byte
sampler encoding. Tags 0/1/2/3 mean prefill/generation/token-limit/stop-token; the
stop field must be zero except for tag 3. Variable data is original u32 token IDs,
68-byte sampled records, u32 logit bits, and canonical original cache bytes.
Cache bytes are retained only for comparison, not installed or decoded into a
candidate. The complete variable layout is checked before count-driven allocation.

`ArchiveLimits` separately bounds total encoded bytes, recipe bytes, positions
and state bytes. Recipe export first counts the complete representation; large
parameter vectors are size-checked before their elements are scanned. Import
compares without constructing a second recipe. Native object limits also remain
in force. These are logical data bounds, not peak RSS, CPU cancellation or time
limits. Cross-platform arithmetic drift refuses exact replay; successful framing
alone does not establish continuation equivalence or independent evidence.

Nine authored integration tests cover every prefix after dropping its original
owner, segmentation, exact/one-less limits, strict framing, eighteen independent
recipe substitutions, same-ID weight changes, corrupt expectations, spent budgets,
alarms and stops. Two compile-fail examples separate imported expectations from
trusted checkpoints and mutable owners. Existing replay tests are unchanged.

Validation: the targeted RCH test and required xtask invocations both exited 127
because rch is unavailable. Rust compilation, tests, formatting and Clippy are
UNEXECUTED. No packet is closed and no production qualification is claimed.
