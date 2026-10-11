use super::*;
pub(super) use probe_fixture::Root;

pub(super) fn files(root: &Root) -> Vec<OsString> {
    let probe_args = probe_fixture::files(root);
    // Remove only the independently created test fixture's fitting artifacts.
    // Every later consumer must read the new command's actual output files.
    fs::remove_file(&probe_args[4]).unwrap(); fs::remove_file(&probe_args[5]).unwrap();
    let args: Vec<_> = ["config.json", "weights.safetensors", "codec-training.json",
        "codec-binding.json", "codec.fakv", "codec-report.ndjson"]
        .into_iter().map(|name| root.0.join(name).into_os_string()).collect();
    fs::write(&args[2], r#"{"schema":"fa.learned-kv-codec-training/1","identity":{"tenant":1,"model":2,"model_generation":3,"tokenizer_generation":4,"profile_generation":5},"context":8,"input_limits":{"auxiliary_bytes":1048576,"weight_bytes":1048576,"weight_read_calls":10000},"output_limits":{"binding_bytes":1048576,"archive_bytes":1048576,"report_bytes":1048576},"policy":{"id":1,"generation":1,"rank":1,"sweeps":8},"budget":{"tokens":2,"source_values":16,"decoder_products":1000000,"fitting":{"source_values":16,"parameter_values":16,"scratch_values":16,"work_units":1696}},"documents":[{"origin":1010,"stream":1000,"tokens":[0,1]}]}"#).unwrap();
    fund(&args); args
}
pub(super) fn monitor_args(root: &Root) -> Vec<OsString> {
    ["config.json", "weights.safetensors", "tokenizer.bbpe", "campaign.json",
        "codec-binding.json", "codec.fakv", "report.ndjson", "monitor.fakvmon"]
        .into_iter().map(|name| root.0.join(name).into_os_string()).collect()
}
pub(super) fn independent(model: &DecoderModel, plan: &Plan) -> (LearnedKvCodec, LearnedKvFitCheckpoint) {
    let sources = plan.documents.iter().map(|document| (document.origin,
        model.recompute(document.stream, &document.tokens, original::inference()).unwrap().cache_image().unwrap())).collect();
    LearnedKvCodec::fit_with_checkpoint(plan.policy, &sources, plan.budget.fitting).unwrap()
}
pub(super) fn fund(args: &[OsString]) {
    let plan = Plan::decode(&fs::read(&args[2]).unwrap()).unwrap(); let model = original::model();
    let rows: usize = plan.documents.iter().map(|document| document.tokens.len()).sum();
    let fit = plan.policy.estimate_fit(model.cache_profile(), rows, FitBudget::default()).unwrap();
    let products: u64 = plan.documents.iter().map(|document| model.estimate(0, document.tokens.len()).unwrap().scalar_products().unwrap()).sum();
    for (section, field, value) in [("budget", "tokens", rows as u64), ("budget", "source_values", fit.source_values as u64),
        ("budget", "decoder_products", products), ("fitting", "source_values", fit.source_values as u64),
        ("fitting", "parameter_values", fit.parameter_values as u64), ("fitting", "scratch_values", fit.scratch_values as u64),
        ("fitting", "work_units", fit.work_units)] { set_number(args, section, field, value); }
}
pub(super) fn fund_exact_outputs(args: &[OsString]) {
    let (plan, inputs) = assets::load(args).unwrap();
    assert!(plan.estimate(inputs.model.profile()).is_ok()); let e = inputs.estimate;
    for (name, value) in [("binding_bytes", e.binding_reservation), ("archive_bytes", e.archive_reservation), ("report_bytes", e.report_reservation)] {
        set_number(args, "output_limits", name, value as u64);
    }
}
pub(super) fn rows(args: &[OsString]) -> Vec<Json> {
    fs::read_to_string(&args[5]).unwrap().lines().map(|line| strict_json::parse(line.as_bytes(), Limits::default()).unwrap()).collect()
}
pub(super) fn selected(rows: &[Json], kind: &str) -> Vec<Json> {
    rows.iter().filter(|row| row.get("kind").unwrap().as_str() == Some(kind)).cloned().collect()
}
pub(super) fn replace(args: &[OsString], before: &str, after: &str) {
    let source = fs::read_to_string(&args[2]).unwrap(); assert!(source.contains(before), "missing fixture: {before}");
    fs::write(&args[2], source.replacen(before, after, 1)).unwrap();
}
pub(super) fn number(args: &[OsString], section: &str, field: &str) -> u64 {
    let bytes = fs::read(&args[2]).unwrap(); let parsed = strict_json::parse(&bytes, Limits::default()).unwrap();
    let section = if section == "fitting" { parsed.get("budget").unwrap().get("fitting").unwrap() }
        else { parsed.get(section).unwrap() };
    section.get(field).unwrap().as_u64().unwrap()
}
pub(super) fn set_number(args: &[OsString], section: &str, field: &str, value: u64) {
    let original = number(args, section, field);
    let source = fs::read_to_string(&args[2]).unwrap(); let start = source.find(&format!("\"{section}\":{{")).unwrap();
    let mut changed = source[..start].to_owned();
    changed.push_str(&source[start..].replacen(&format!("\"{field}\":{original}"), &format!("\"{field}\":{value}"), 1));
    fs::write(&args[2], changed).unwrap();
}
