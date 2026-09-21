# Sequential actor service

Plan sections 8, 9.11, 14.9 and 17: repeated actor work through the existing
full-input, two-key authority, with bounded intake and retained obligations.

## Native handoff (2026-09-21)

`drive_peer_sequence_from_file` admits fresh evidence acquisition only for the
operator-selected current request. The bounded completed prefix is checked
against the original journal. Earlier exact/conflicting retries still compare
original bytes, without reading a source or sampling time. Future keys refuse
before intake. `drive_peer_sequence_observe` acquires no source and cannot admit
new work. Both use the original authenticated connection, tickets, codec and
lifetime transport limits; no authority or durable request is stored in a second
queue. The maximum schedule has 64 distinct nonzero keys.

`retire_completed_request` checks the original terminal disposition, retires the
matching local job and confirms direct-child reaping before another cohort can
start. Pending review, dispatched and unknown states refuse. It neither cancels
work nor reconciles an effect, resets a budget, restarts a dispatcher, reads
evidence or grants authority. False means keep the owner and poll cleanup again.

Six authored native socket/file regressions cover successive keys on one owner,
source-free historical retries, future-key refusal, nonterminal handoff refusal,
foreign/malformed schedules, unchanged connection accounting and exact/one-over
schedule bounds. Existing assertions and fixtures remain unchanged.

The executable multi-request supervisor loop is the next integration increment.
The required RCH xtask attempt returned exit 127 because `rch` is unavailable.
Compilation, Rust tests, formatting and Clippy are UNEXECUTED. No production
qualification, performance claim or Bead closure is made. Authenticated Linux
peer credentials do not establish executable integrity or evaluator correctness.
