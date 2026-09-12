# Bounded file readers for exported decoder parameters

Consumer: the host admitting a compatible model from already opened File/Read
handles into the existing DecoderModel, including its original numerical forward
pass, all-layer KV capture and checkpoint continuation. This extends the existing
single-file and indexed-shard import paths with actual bounded input operations,
not a second parser, inference backend, path resolver or task runtime.

## Admission and byte movement

`DecoderModel::read_safetensors(profile, reader, budget)` consumes one complete
single-file export from its current reader position to EOF. It uses the original
strict descriptor checker, finite F32/F16/BF16 conversion and model constructor.
Successful WeightLoadReceipt values equal those from the corresponding in-memory
import. No seek, memory mapping, raw whole-file copy or whole-tensor raw buffer is
required. Headers are capped at the original 1 MiB; tensor bodies are decoded in
4 KiB raw chunks into their final normalized f32 vectors. The header buffer is
dropped before body processing. Those vectors move into the original immutable
parameter owner rather than being concatenated or copied into another archive.

`DecoderModel::read_safetensors_shards(profile, index, readers, budget)` validates
the complete original index and exact reader-key set before doing any I/O. It then
reads and checks ALL assigned shard directories before reading ANY tensor body.
Their exact shapes, encodings, byte coverage, aggregate size and optional indexed
total_size must agree. The body phase follows physical offset order within each
source and requires the exact declared payload followed by EOF. A bad final shard
header therefore wins over a nonfinite value in an earlier, unread tensor body.
A late body/EOF failure still returns no partially admitted model.

Index labels select only the explicitly supplied map of readers. No index value
opens a file, traverses directories, executes code or changes a profile. Hosts
must authorize those handles and provide exclusive, stable source content during
admission. An opened file or a matching tensor name is not authentication; this
does not prevent a concurrent writer from changing a file while it is read, or
prove that externally supplied parameters have the declared architecture.

## Shared consumption budget and failure semantics

WeightReadBudget bounds actual bytes returned and the number of read attempts.
It persists across all shards and survives failed imports. Interrupted calls
consume attempts before being retried, so even an always-interrupted source
cannot spin past the configured call limit. Other errors, including WouldBlock,
return immediately. A failed bulk import is not automatically restarted or
resumed; the source position remains advanced and any new/repositioned source is
an explicit host decision. Reusing the same budget does not reset consumed work.

The global source-set cap remains MAX_WEIGHT_SET_BYTES. The read-byte maximum is
that cap plus one byte for rejecting an unexpected suffix, and the read-call
maximum is 1,000,000. Exact EOF verification needs one byte of remaining read
allowance even though successful EOF returns zero bytes. After all headers are
validated, the full remaining payload plus that one-byte headroom is checked
before decoding. An exact successful import leaves the headroom unused; it does
not count an EOF probe as a byte read. Each attempted EOF read does count as a
read call. Receipts count the actual source format, not I/O retries or probes.

These budgets describe startup work, not effect rights, wall-clock deadlines,
peak RSS or asynchronous cancellation. A single blocking Read can still block;
timeouts/readiness and choosing suitable sources remain host responsibilities.
No hidden executor, thread, network connection or OS credential handling is added.
The normalized parameter and cache/context limits remain those of DecoderProfile.

## Regression source

Eleven new scenarios in decoder_weight_reader.rs exercise actual filesystem
handles and controlled readers. The retained official-library synthetic fixture
from the concurrent single-file loader is read from a temporary file, compared
against the in-memory import, and continued after that file and the original
session/model owners are dropped. Real multi-file imports match the original
sharded receipts, complete logits, every layer's KV image and greedy continuation.

Additional controls cover single-byte reads with interruptions, tensors larger
than the scratch buffer, all-header preflight, missing/hostile source mappings,
aggregate byte budgets, endless interruption and repeated-attempt accounting,
truncated inputs, a late PermissionDenied, WouldBlock, trailing bytes, oversized
header declarations, exact read allowance and late nonfinite shard data. A failed
import leaves an already admitted model and its numerical state unchanged.

All new Rust source remains uncompiled and unexecuted in this editing environment;
Cargo, rustc, rustfmt, RCH and br are unavailable. No local compiler fallback,
qualification claim, bead closure, dependency admission or historical result
promotion is made. Existing official-fixture provenance belongs to its original
producer; these reader scenarios do not themselves establish a passing run or
pretrained-model quality. The source/format contracts remain those of the original
[SafeTensors loader](SAFETENSORS_DECODER_REFERENCE.md) and
[indexed shard loader](SHARDED_WEIGHT_LOADING_REFERENCE.md).

## Configured entry points and the existing CLI

The original from_llama_files API now sends its validated regular weight-file
handle through this reader. Config parsing still completes before weight access;
regular-file/symlink checks and exact CheckpointFileLimits stay in place. Its
internal I/O allowance adds only the EOF-probe headroom, not another permitted
file byte. The existing decoder_from_checkpoint example inherits this bounded
raw-weight path without changing its CLI, tokens or generation policy. The small
configuration file remains separately bounded at 64 KiB.

read_llama_safetensors and read_llama_shards compose the same LlamaConfig admission
with explicitly supplied readers and a caller-owned persistent read budget.
Unsupported architecture/configuration refuses before any weight I/O or shard
index processing. Sharded receipts keep both the accepted configuration and each
actual shard's interpretation. These methods do not infer local paths from index
labels, load a tokenizer, change model capacity or bypass the original constructor.

Four additional configured-reader regression scenarios compare file/memory/
fragmented imports and receipts, run config-plus-shards through actual File
handles and checkpoint continuation after source removal, prove unsupported
configs never touch readers, and preserve exact file limits and truncation error
classes. These also remain uncompiled/unexecuted. The original concurrent config
and file regression suites and checkpoint CLI tests are retained unchanged.
