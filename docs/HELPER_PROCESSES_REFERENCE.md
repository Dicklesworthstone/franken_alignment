# Executable helper launch and owned child lifetimes

Consumer: the supervising host that previously had to preconnect every HelperPool transport. This is a reference integration of FA-018/FA-019, plan sections 9.1-9.3 and 17.1, serving FI-A03/FI-A06/FI-A13/FI-A14/FI-A16. It does not complete the credential perimeter or native runtime admission.

## Executable path

`helper_processes::launch_helpers` accepts the existing CommitteeContract and an exact member-to-HelperProgram map. Every program has an absolute executable path, an absolute working directory, bounded arguments and an explicit environment map. Complete roster equality, aggregate text capacity and all path metadata are checked before the first process starts. A later socket/spawn error returns every already started child with termination requested. No reduced-roster success is returned and no helper input bytes are sent during setup.

The implementation invokes Command directly, without inserting a shell or searching PATH. It clears the inherited environment and reinstalls only the explicitly configured entries. Configuration is trusted host data, never an actor-supplied execution request. Explicitly selecting a shell or interpreter remains the host's decision. Program debug output omits paths, arguments and environment values.

Each child receives its own unnamed, full-duplex Unix socket as standard input. Both standard output and standard error are directed to the host's private standard-error stream, not the actor wire or commitment parser. A worker uses `HelperClient::from_process_stdin(expected_profile)` to attach through safe owned-descriptor conversion; a non-socket standard input refuses. The original HelperClient and HelperPool still deliver, decode and compare the exact input profile, perform commit/reveal and feed the original congress. Process exit, including status zero, never counts as a vote.

## Cleanup is an owned obligation

`HelperChildren` retains direct child handles and their observed exit records. `reap` performs one nonblocking try_wait per retained live child; stop requests additionally issue at most one kill attempt per child on that pass. A successful kill request is not reported as a reaped process. Repeated polling can finish cleanup or expose an OS refusal, without creating a thread, hidden waiting loop or replacement process. Reaped children keep their terminal status and are not signalled again.

A partial launch error owns its cleanup obligation. Callers must retain it, request shutdown and poll until all children are reaped. Drop attempts bounded termination/reaping and emits a diagnostic for remaining children; it cannot guarantee cleanup of an uninterruptible child or replace explicit host shutdown. Direct-child cleanup does not terminate or contain descendants. Scheduling of reap calls, executable/directory integrity, descriptor hygiene and private diagnostic routing remain host duties.

## Limits and verification status

This is not a sandbox, immutable executable attestation, cryptographic worker identity, control-ledger persistence or production runtime integration. The host can explicitly pass an inappropriate credential, and the program still has its OS user's filesystem/network authority. Clearing an environment is not complete credential isolation; unrelated non-CLOEXEC descriptors and descendant processes require a real host containment profile. OS spawn is synchronous setup and is not claimed wall-clock bounded. The existing commitment remains non-cryptographic.

September 11, 2026: source adds real process/socket launch, worker attachment and bounded cleanup ownership, without a dependency or another executor. Integration-test source launches this test binary as configured workers, sends original helper inputs through the inherited sockets, obtains genuine protocol votes and publishes through the existing ledger. Controls cover adverse votes, status-zero exits with missing reveals, exact roster and metadata preflight, a mid-roster spawn failure retaining its first child, configuration limits and hidden debug values. A separately launched parent is deliberately given a fake credential; its helpers assert that credential and PATH were not inherited. Fixture entry points are subprocess plumbing, not independent execution evidence or trained models.

Cargo, rustc, rustfmt, RCH and br are unavailable in the editing environment. The new Rust code and tests have not been compiled, run, formatted by rustfmt or qualified through the project gate. Historical receipts do not qualify this source. No production packet or br-managed bead is closed.
