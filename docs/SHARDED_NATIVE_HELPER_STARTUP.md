# Sharded native-helper file startup

## Capability and boundary

`NativeEvaluator::from_llama_shard_files` connects explicitly provisioned local
shard files to the existing checkpoint loader, all-layer monitor, sampler,
immutable tokenizer and one-request native evaluator. Its consumer is the native
helper's cold startup path (FA-025). It is not a second inference implementation,
model discovery service, new effect gate or replacement authority ledger.

`NativeHelperShardFiles` supplies five auxiliary paths and a complete ordered map
from index labels to operator-selected paths. A label may differ from its actual
filename. Neither an index label nor checkpoint metadata is joined to a directory,
opened as a path, downloaded or executed. An invalid or incomplete label inventory
must fail through the original index parser before any weight file is inspected.

The four existing configuration/tokenizer/monitor/sampler files retain their
original admission rules. Native profile/tokenizer/policy/sampler preflight runs
before the index is read. Model-dependent monitor checks remain after loading,
before an evaluator is returned. Missing or invalid monitoring never becomes a
quiet substitute. Successful startup computes no token or categorical judgment.
The original native evaluator, including its monitored terminal-token requirement,
is unchanged.

The file opener checks all declared weight handles and their aggregate metadata
size before reading any weight bytes. The original streaming parser then checks
all directories before decoding any scalar, including exact tensor assignment,
shapes, encodings and index total-size claims. `weight_bytes` is the sum of all
shard file prefixes, headers and data, not a fresh ceiling per shard. The same
ceiling is checked against actual parsed directories before scalar ingestion,
so metadata checks alone cannot admit a larger complete export after file growth.
Each handle reaches the original real-EOF probe; no `Take` adapter creates a
false successful EOF at a byte ceiling. No concatenated raw checkpoint is built.

`NativeAssetReadBudget::for_shards` explicitly permits the bounded index allowance
in addition to the existing auxiliary maximum. It uses the existing counters and
read implementation. The old `new` constructor's maximum is unchanged. Both asset
and weight budgets are caller-owned, cumulative across shards and failed calls,
and never replenished by this loader. EOF probes require remaining allowance;
zero-byte EOF consumes a read attempt but no returned bytes. Disk metadata/open
operations and a blocking read's wall time are not covered by these read counters.

Errors retain the auxiliary asset or bounded shard label and distinguish native
bootstrap, file, and original weight-reader failures. No partial evaluator, vote,
model or default response is returned. Opened handles and partial allocations drop
on error; all performed read work remains charged. Retries require an explicit
caller invocation with its retained budgets.

The existing final-component symlink and opened-regular-file checks are not a
race-free filesystem sandbox, an atomic cross-file snapshot, or authentication.
Protect all path components and immutable asset contents during startup. Synthetic
fixture weights are not pretrained-model accuracy evidence. No success here grants
permission to publish an effect or changes the mandatory two-key authority path.

## Regression coverage and qualification

Twelve authored tests (`shard_files_`) exercise real files and the original native
numerical path. Positive fixtures compute different verdicts for `?` and `!`,
compare original streamed-reader receipts/work, and continue after source paths
are removed. Causal negatives include profile failure before index/weight access,
invalid and incomplete source maps with unavailable paths, exact/one-under index
and aggregate limits, all-directory admission before any earlier scalar read,
truncation, trailing data, nonfinite values, symlinks/nonregular files, retained
budget failures, and an actual monitor hold that consumes its draw without a vote.

A direct original-reader test checks the narrower aggregate ceiling against actual
headers independently of filesystem metadata. Existing public reader entry points
still select the original global ceiling and otherwise use that same implementation.

The targeted command was attempted on 2026-09-22:

```
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference shard_files_
```

It failed before compilation (`rch: command not found`, exit 127). Compilation,
Rust tests, rustfmt, Clippy and the full repository gate are **UNEXECUTED**. Source
whitespace and exact upstream preimage blob hashes were checked; these are not
execution evidence. No broader bead or production qualification is closed.


## Integration with intervening tied-embedding support

Publication rebases these source additions onto `f95ab32fcff0122cdc6bd6fe2d3d412dc8bf6150`,
preserving both intervening tied-embedding commits. The new bounded reader accepts
an explicit `OutputHead` and all original public loaders retain their existing
semantics. File label preflight and actual tensor admission use the SAME value
from the original `LlamaConfig` receipt. No missing head implicitly enables tying;
an indexed redundant head must still match normalized embedding bits.

Two additional authored regressions cover omitted and redundant tied heads,
original-loader receipt/prefill parity, independent-mode refusal before weight
I/O, actual aggregate directory limits and conflicting redundant heads. These
join the twelve saved file regressions; all fourteen remain UNEXECUTED.
The rebase does not qualify the new sources or the preserved tied-head sources.
