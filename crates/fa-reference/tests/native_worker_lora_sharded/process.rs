//! Actual /4 binary through its original private socket, independent wire oracle.
use super::{Fixture, process, assets, Verdict};
use std::fs;

#[test]
fn worker_v4_sharded_adapter_changes_actual_commit_reveal_without_a_single_file_fallback() {
    for rank in [1, 2] {
        for json in [false, true] {
            for zero in [false, true] {
                let fixture = Fixture::new(rank, zero, json, false);
                process::exchange(&fixture.inner, b"?", if zero { Verdict::Allow } else { Verdict::Deny });
                process::exchange(&fixture.inner, b"!", if zero { Verdict::Deny } else { Verdict::Allow });
            }
        }
    }
}

#[test]
fn worker_v4_sharded_adapter_late_missing_or_malformed_sources_emit_no_commitment() {
    for defect in 0..4 {
        let fixture = Fixture::new(1, false, true, false);
        match defect {
            0 => fs::remove_file(&fixture.shards["base-1.safetensors"]).unwrap(),
            1 => {
                let path = &fixture.shards["base-1.safetensors"]; let bytes = fs::read(path).unwrap();
                fs::write(path, &bytes[..bytes.len() - 1]).unwrap();
            }
            2 => fs::remove_file(&fixture.adapter_weights).unwrap(),
            _ => {
                let mut bytes = fs::read(&fixture.adapter_weights).unwrap(); bytes.push(0); fs::write(&fixture.adapter_weights, bytes).unwrap();
            }
        }
        let stderr = process::no_vote(&fixture.inner, None);
        assert!(stderr.contains("adapted native startup"), "{stderr}");
        assert!(!stderr.contains("adapter.single_base"), "{stderr}");
    }
}

#[test]
fn worker_v4_sharded_adapter_full_input_monitor_and_generation_limits_remain_original() {
    for defect in 0..4 {
        let mut fixture = Fixture::new(2, false, true, defect == 0);
        match defect {
            1 => fixture.inner.manifest = fixture.inner.manifest.replace("\"max_new_tokens\":2", "\"max_new_tokens\":1"),
            2 => fixture.inner.manifest = fixture.inner.manifest.replace("\"steps\":10000", "\"steps\":1"),
            _ => {},
        }
        fixture.inner.save();
        let stderr = if defect == 3 { process::no_vote_frame(&fixture.inner, Some(&assets::frame(b"?"))) }
            else { process::no_vote(&fixture.inner, Some(b"?")) };
        assert!(stderr.contains(match defect { 0 => "Held", 1 => "TokenLimit", 2 => "StepLimit", _ => "Protocol(Binding)" }), "{stderr}");
        if defect == 3 { assert!(stderr.contains("evaluations: 0"), "{stderr}"); }
    }
}
