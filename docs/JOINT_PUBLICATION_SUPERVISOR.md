# Joint qualification in checked supervisor startup

FA-105/FA-062 implementation increment; plan 9.4, 9.7, 9.9 and 8.8;
FI-A07/FI-A08. The consumer is the existing checked-publication supervisor.

The concurrent upstream native JointPublicationProfile implementation is retained.
It already composes joint, witness, feed, freshness and snapshot-fallback selection
in one initial canonical image and pins them before recovery. This increment
adds check_joint_publication_profile: the existing owner can verify the SAME
complete selection before a frontend reads or binds a source. None still requires
absence; a different feed or lookup budget is not equivalent merely because the
witness limits match. No source observation or current qualification is implied
by configuration equality. Invalid selection does not poison or mutate the owner.

Three newly authored native tests pair rejected joint promotion with actual
qualified two-key publication, then change the final opaque model binding after
dispatch to ensure qualification cannot replace publication evidence. Recovery
keeps unknown charges until an endpoint nonexecution receipt, rejects historical
reactivation and old keys, and permits new qualification with new approvals.
The live-selection test checks unchanged authority and bytes on mismatches.
Existing upstream startup and storage-barrier tests are preserved, not counted
as new. Synthetic labels are not detector qualification.

Rust compilation, formatting, Clippy and tests have not executed. The required
RCH command cannot start because rch is unavailable. No qualification gate or
work packet is closed by these source changes.
