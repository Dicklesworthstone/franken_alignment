# One-shot native helper process

`native::process::run_native_worker` consumes one original NativeEvaluator, a
provisioned Unix socket, an independently supplied salt and a NativeProcessBudget.
It drives the original NativeHelperClient: complete input admission, one monitored
token at a time, exact terminal categorical verdict, then original commitment and
reveal framing. It never accepts a caller verdict, reconnects or restarts a model.

This is a synchronous loop for a dedicated child process, not a second async
runtime. Do not run it on an executor worker. Socket reads/writes block under the
remaining absolute deadline; a bounded client-step allowance counts every attempt,
including interrupted operations. There is no busy sleep loop or per-step renewal.
The budget starts at construction so a caller can include checkpoint loading in
the same lifetime. Create it before startup, not after an expensive import.

The clock is checked before and after every native step. A token that finishes
late cannot cause the next commitment/reveal operation to start. This is not
preemption within matrix work or filesystem calls, a hard OS scheduling bound,
or a replacement for the supervisor's original receipt-time deadlines. If a write
finishes late, bytes may already be visible: Deadline is not proof of non-delivery.

The result retains actual native counters, final client phase and the stopping
reason after destroying client ownership. ReplySent describes a transmitted
reveal, not acceptance by the congress or permission to publish an effect. Parent
termination, reaping and missing-worker rules remain with HelperChildren and the
original coordinator. No helper exit code is converted into a ballot.

`inherited_worker_socket` duplicates the launcher's full-duplex stdin with safe
OwnedFd conversion and rejects ordinary pipes/files. It reads no protocol bytes
and does not import a raw descriptor number or assume stdout is a socket. The
existing launcher sends stdout/stderr to the host's private diagnostic channel.

Ten tests cover input-dependent success, real reveal gating, a waiting socket,
startup/after-token/after-write deadline boundaries, interrupted prefill, step
exhaustion and a monitored hold. Synthetic model parameters test integration, not
helper accuracy. Threading is used only by a handshake test, not the worker code.
The required RCH gate cannot start here (`rch: command not found`, exit 127).
Rust compilation, formatting, Clippy and all new tests remain UNEXECUTED; no
production qualification, verifier waiver or bead closure follows.

## Checkpoint-backed executable

`fa-native-helper` is the concrete child for `helper_processes::launch_helpers`.
Build it through the required remote verifier/toolchain:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo build --locked -p fa-reference --bin fa-native-helper
```

Provision its absolute executable, working directory and ONE absolute manifest
path as the original `HelperProgram` argument. No shell, environment-based model
selection or PATH lookup is introduced. Do not pipe prompt text to the program:
stdin must be the launcher's private full-duplex Unix socket. stdout is never
used for responses; the original socket carries the original wire frames. The
existing launcher's private stderr carries bounded failure diagnostics, never
prompt bytes, output text, raw profile bytes or the provisioned salt.

The manifest is strict `fa.native-worker/1` JSON. Every field below is mandatory;
unknown/duplicate fields, implicit epochs, relative asset paths, malformed numeric
values and alternate schema versions refuse. `bytes_hex` is lowercase hexadecimal
for EXACT InputProfileBinding bytes, including non-UTF-8. The decoder profile is
an independent operator expectation, not guessed from checkpoint metadata. The
ordinary native startup still binds it against the tokenizer/config/monitors.

This example has the synthetic test model's shape and stop ID; real assets need
their actual registered profile and control IDs, not these illustrative numbers.

```json
{
  "schema": "fa.native-worker/1",
  "input": {
    "id": 7,
    "bytes_hex": "6e61746976652063617465676f726963616c2066697874757265",
    "model_epoch": 0,
    "tokenizer_epoch": 0,
    "policy_epoch": 0
  },
  "decoder": {
    "identity": {
      "tenant": 1,
      "model": 2,
      "model_generation": 3,
      "tokenizer_generation": 4,
      "profile_generation": 5
    },
    "shape": {
      "vocabulary": 264,
      "hidden": 2,
      "intermediate": 2,
      "layers": 1,
      "query_heads": 1,
      "cache_heads": 1,
      "context": 1024
    },
    "epsilon": 1e-05,
    "theta": 10000.0
  },
  "policy": {
    "max_new_tokens": 2,
    "stop_tokens": [
      263
    ],
    "max_output_bytes": 128,
    "tokenization": {
      "input_bytes": 65536,
      "pair_lookups": 196608,
      "heap_pops": 196608
    },
    "generation": {
      "scalar_products": 1099511627776,
      "sampling_entries": 16777216
    }
  },
  "files": {
    "configuration": "/private/native-worker/config.json",
    "tokenizer": "/private/native-worker/tokenizer.bin",
    "monitoring": "/private/native-worker/monitor.json",
    "sampling": "/private/native-worker/sampling.json",
    "weights": "/private/native-worker/weights.safetensors"
  },
  "stream": 12,
  "salt_file": "/private/native-worker/salt.bin",
  "lifetime": {
    "milliseconds": 10000,
    "steps": 10000
  },
  "startup": {
    "asset_bytes": 1048576,
    "asset_calls": 4096,
    "weight_bytes": 1048576,
    "weight_calls": 4096
  }
}
```

The five model assets retain the original loader's fixed per-file maximums and
strict native profile. The four auxiliary assets share `startup.asset_*`; weights
use `startup.weight_*`. Manifest (64 KiB) and salt (256 bytes) are separately
bounded regular-file reads with a 1,024-call cap. Empty/short salts refuse before
model assets are loaded. Salt bytes are never trimmed, generated from the sampler,
or included in model input. Their secrecy and entropy remain operator obligations.
Protect all paths and source files: final symlink/regular-file checks do not claim
race-free confinement, an atomic asset snapshot or authenticated checkpoints.

After bounded manifest parsing, one lifetime starts BEFORE salt and checkpoint
loading. Loading that finishes late cannot read a worker request or send a vote.
The original model/monitor/tokenizer/sampler compose into one fresh evaluator;
the entry point implements no second loader, network dial or alternative
inference path. OS calls and an individual numerical token are not preempted.
Supervisor process termination and original congress deadlines still apply.

Exit 0 means ReplySent only; exit 1 records an unfinished/failed protocol run;
exit 2 means invocation, attachment or startup failed. None is an accepted ballot.
On failure the process closes its descriptor and never sends an invented default.
The supervisor must process the original commitment/reveal transcript and keep
missing members in the fixed denominator. Never retry a worker to reroll a vote.

Five parser/file tests and six actual-child integration tests accompany the
executable. The latter launch the Cargo-built binary using the ORIGINAL
HelperProgram/launch_helpers, load real serialized synthetic checkpoints, verify
input-dependent frames and reveal gating, and compare a positive congress
submission with a held native worker that remains missing and cannot authorize.
There is no endpoint execution in the congress test. A further loop test preserves
the first native failure when deadline expiry coincides with that failure.
All 22 newly authored tests remain UNEXECUTED pending the RCH xtask gate. Synthetic
weights, source checks, a built process or a zero exit are not detector calibration,
model authenticity, independence, sandbox qualification or production release.
