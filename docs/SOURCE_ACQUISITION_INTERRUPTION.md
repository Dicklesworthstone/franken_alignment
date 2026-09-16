# Interrupted registered-source acquisition

The durable FileOversight source path now closes its process-local admission latch BEFORE calling the sealed evidence reader. A caught unwind in reading, encoding, replay or storage therefore cannot retain permission to submit, review, approve, dispatch or publish against the old source. The latch clears only after an acknowledged observation, explicit withdrawal/replacement, or the existing recovery fence. Committed observation refusals already withdraw the original source, inputs and reviewer keys; clearing the latch does not make refused evidence eligible.

Successful unchanged refreshes do not unnecessarily invalidate reviewed inputs. Stale preflight requests do not read or withdraw a valid source. Recovery from an interrupted read first commits withdrawal and then performs a new real file read. Source timestamps remain observation-start times, so slow reads cannot receive artificially extended leases.

Five regression functions exercise caught read unwind with a positive proposal/recapture control, unchanged-input preservation, ordinary failed reads, read-free stale preflight and all five journal replacement fault barriers. The callback seam is private; the public EvidenceFile remains sealed. No journal encoding, permit, refund or endpoint outcome law changes.

Plan §§7.2, 7.6–7.8, 8.8, 16.4; FA-062/FA-INV-004 and FA-INV-014. Rust/RCH are absent in the editing environment: compilation, formatting, Clippy and these Rust tests are unexecuted. This source change does not close a bead or qualify a deployment.
