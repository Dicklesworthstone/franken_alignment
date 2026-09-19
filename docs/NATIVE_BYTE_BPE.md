# Native byte-BPE input contract

`generation::tokenizer::ByteBpe` supplies a bounded, exact byte-pair encoder for
native monitored inference. Its scope is **FA-BBPE/1**, not the general
Hugging Face tokenizer format. Parameter loading alone does not establish a
checkpoint's tokenization: the operator must supply and authenticate a vocabulary
and ordered merge table whose trained semantics match this profile.

## Supported meaning

The entire input is one byte sequence. There is no normalization, whitespace
rewriting, regex pretokenization, chat templating, dropout, automatic BOS/EOS,
unknown-token substitution or recognition of control spellings in user text.
All 256 byte values have a unique singleton token; vector positions are the actual
model token IDs, which need not equal byte values. Each content token has unique
nonempty bytes. Each ordered rule concatenates already defined content tokens.
Every multibyte token must be reachable. A control token has no guessed spelling
and cannot be produced by encoding text or silently stripped while decoding.

The next merge is the lowest-ranked currently adjacent pair, with the leftmost
occurrence winning ties. The encoder uses a heap and linked input intervals; only
the two neighboring candidates change after a merge. Stale candidates are
rechecked and counted. The resulting immutable `TokenizedInput` retains original
bytes, original IDs, and a complete partition of byte spans. Unicode and invalid
UTF-8 are both retained verbatim. Existing numerical replay continues to consume
original IDs, not a text round trip.

The complete `DecoderProfile` binds model/tenant/tokenizer generations, vocabulary,
dimensions, context and numerical configuration. This is an exact data binding,
not authentication of an operator-supplied model or tokenizer. Cloning these
objects copies observations; none can supply effect credentials, a permit, or a
monitoring verdict. The optional front end does not change the numerical decoder's
original-token API.

## Budgets and interchange

Encoding admits at most 65,536 input bytes. Pair examinations and heap pops have
independent caller budgets capped at three times that maximum. Actual stale pops
count, and a budget failure returns spent logical work with no partial encoding.
Vocabulary, token lengths, control count, merge count and decoded bytes are also
bounded. These counts do not claim allocator, runtime-latency or deployment SLOs.

`to_bytes` and `from_bytes` use the strict `FABBPE01` binary domain. The 120-byte
header contains the exact model profile in big-endian fields. Vocabulary entries
are either tag 0 (control) or tag 1 plus a u32 byte length and content. A u32 merge
count precedes ordered u32 `(left, right, result)` triples. No trailing bytes or
alternate tags are admitted. The expected profile is supplied independently and
checked before vocabulary allocation. Imported data passes the same complete
constructor validation; no file bytes authorize code execution or set authority.

## Verification boundary

Twelve authored Rust tests cover ordinary-byte controls, arbitrary original IDs,
rank and overlap semantics, exhaustive binary inputs, exact budgets, malformed
inventories and merges, decoding controls, strict wire round trips/truncations,
and profile changes. They have not been compiled or executed in this environment:
the required RCH runner is absent. A separate Python heap-versus-rule-scan check
exercises 2,047 inputs and exact partitions, but is not execution evidence for the
Rust implementation. No production qualification or roadmap/bead closure follows.

## Monitored text execution

`generation::text::TextDecoder` owns a **fresh** `MonitoredSampledDecoder` and its
fixed `ByteBpe`. Construction rejects an already advanced owner or a mismatched
profile. There is no mutable decoder/tokenizer accessor: a later request cannot
swap tokenization semantics while retaining the old KV cache.

`TextGenerationRequest` carries original prompt bytes, explicit prefix control IDs,
stop IDs, the existing cumulative numerical/sampling budget, the BPE work budget,
and an output-byte ceiling. All prompt bytes are encoded before native inference.
BOS-like IDs may be supplied only through the explicit control field; spellings in
user text remain ordinary content. Every non-text control must be a declared stop
when sampling, and a stop is suppressed only after its own full native review.
The worst-case output size (`max_new_tokens * max_content_bytes`) is admitted and
allocated before inference. Insufficient output capacity is a refusal, not a
reason to truncate computed output.

Execution delegates directly to the original `generate` implementation and its
`GenerationCursor`; it does not reimplement sampling, monitors or numerical steps.
`TextGenerationReport` retains that original report plus encoded prompt provenance
and exact output bytes. A held token is never decoded, but an earlier quiet prefix
may remain visible with an explicit `Held`/`Failed` finish. A stop or held sample
still consumes its original draw and context position. A strict `utf8()` accessor
refuses invalid or incomplete UTF-8; `bytes()` and the original IDs remain available
without replacement characters. An unexpected output-contract failure retains the
numerical report instead of presenting an empty or partially decoded success.

An empty additional prompt continues only the existing reviewed numerical prefix.
Old text is never reconstructed or re-tokenized to rebuild KV history. The caller
must still use the ordinary action/congress/two-key publication path before these
observations can become an external effect. This synchronous adapter does not
claim process isolation, serving-runtime admission, checkpoint authenticity or a
trained-checkpoint compatibility profile beyond explicitly supplied FA-BBPE/1.

Twelve additional Rust tests use the real native decoder, exact linear residual
monitor and original sampler. They check direct-token numerical equivalence,
monitored stop consumption, held prompt/output suppression, a quiet prefix before
a later hold, exact/invalid UTF-8, all-input/context/output admission, cumulative
sampling exhaustion, continuation, generation mismatches and advanced-owner
refusal. Three compile-fail examples protect the lack of permission conversion,
mutable decoder escape and owner cloning. These tests and examples are also
**unexecuted** pending the required RCH build/test gate.
