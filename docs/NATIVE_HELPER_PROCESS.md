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
