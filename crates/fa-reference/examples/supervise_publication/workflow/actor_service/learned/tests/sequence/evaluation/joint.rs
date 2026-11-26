//! The resident /2 selection keeps one evaluator and original learned custody.
//! Fixed fixture weights and benign labels do not qualify a joint committee.
use super::*;

#[test]
fn jointly_accounted_resident_command_keeps_the_exact_original_policy_through_finish() {
    actual_command_profile(true);
}

#[test]
fn jointly_accounted_messages_reuse_one_evaluator_socket_and_allowance() {
    run_sequence_profile(false, true, "both", 4, true);
}

#[test]
fn jointly_accounted_resident_hold_and_human_rejection_are_independent() {
    run_sequence_profile(true, false, "final", 2, true);
    run_sequence_profile(false, false, "final", 2, true);
}

#[test]
fn jointly_accounted_resident_evaluator_budget_does_not_reset_between_messages() {
    run_sequence_profile(false, true, "exhausted", 2, true);
}

#[test]
fn jointly_accounted_resident_stop_discards_partial_evaluation_after_hold() {
    run_sequence_profile(true, false, "partial", 2, true);
}

#[test]
fn jointly_accounted_resident_deadline_does_not_renew_the_evaluator_lifetime() {
    run_sequence_profile(true, false, "deadline", 2, true);
}
