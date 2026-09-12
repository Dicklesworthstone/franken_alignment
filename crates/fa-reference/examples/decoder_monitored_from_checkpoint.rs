//! Offline original-token inference with compulsory per-layer residual review.
//! Every generated ID is printed only after its own review. Holds exit with 2.
use fa_reference::action::consequence::activation::monitor::decoder::{MonitoredDecoder, MonitoredStep, MonitoringStatus, MonitoringWork};
use fa_reference::action::consequence::activation::monitor::decoder::config::MAX_MONITOR_CONFIG_BYTES;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledDecoder;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::config::MAX_SAMPLING_CONFIG_BYTES;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, DecoderIdentity, DecoderModel, DecoderProfile, DecoderWork, MAX_DECODER_PRODUCTS};
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{SampleBudget, SamplingBudget};
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::pretrained::CheckpointFileLimits;
use fa_reference::action::consequence::activation::tensor::kv::MAX_KV_POSITIONS;
use fa_reference::strict_json::{self, Limits};
use fa_reference::Error;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::ExitCode;

const MAX_REQUEST_BYTES: usize = 65_536;
const USAGE: &str = "decoder_monitored_from_checkpoint CONFIG_JSON WEIGHTS_SAFETENSORS TOKEN_REQUEST_JSON MONITOR_JSON CONTEXT NEW_TOKENS PRODUCT_BUDGET [SAMPLING_JSON]";
fn invalid(message: impl Into<String>) -> io::Error { io::Error::new(io::ErrorKind::InvalidInput, message.into()) }
fn numerical(error: impl std::fmt::Debug) -> io::Error { invalid(format!("numerical refusal: {error:?}")) }

// Same explicit original-token request as decoder_from_checkpoint. No tokenizer
// round trip, inferred BOS, EOS early exit or unbounded text input is introduced.
fn request(bytes: &[u8]) -> io::Result<(DecoderIdentity, Vec<u32>)> {
    let json = strict_json::parse(bytes, Limits { max_bytes: MAX_REQUEST_BYTES, max_depth: 3,
        max_items: MAX_KV_POSITIONS + 32, max_string_bytes: 128 }).map_err(|error| invalid(error.to_string()))?;
    let root = json.as_object().ok_or_else(|| invalid("request must be an object"))?;
    if root.len() != 2 || !root.contains_key("identity") || !root.contains_key("tokens") { return Err(invalid("request fields: identity, tokens")); }
    let ids = root["identity"].as_object().ok_or_else(|| invalid("identity must be an object"))?;
    let names = ["tenant", "model", "model_generation", "tokenizer_generation", "profile_generation"];
    if ids.len() != names.len() || names.iter().any(|name| !ids.contains_key(*name)) { return Err(invalid("identity inventory mismatch")); }
    let get = |name: &str| ids[name].as_u64().filter(|value| *value > 0).ok_or_else(|| invalid(format!("invalid identity: {name}")));
    let identity = DecoderIdentity { tenant: get("tenant")?, model: get("model")?, model_generation: get("model_generation")?,
        tokenizer_generation: get("tokenizer_generation")?, profile_generation: get("profile_generation")? };
    let values = root["tokens"].as_array().ok_or_else(|| invalid("tokens must be an array"))?;
    if values.is_empty() || values.len() > MAX_KV_POSITIONS { return Err(invalid("invalid original-token count")); }
    let tokens = values.iter().map(|value| value.as_u64().and_then(|id| u32::try_from(id).ok())
        .ok_or_else(|| invalid("token IDs must be unsigned 32-bit integers"))).collect::<io::Result<Vec<_>>>()?;
    Ok((identity, tokens))
}
fn read_file(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    let before = fs::symlink_metadata(path)?;
    if !before.is_file() || before.file_type().is_symlink() || before.len() > limit as u64 {
        return Err(invalid("input must be a bounded regular file"));
    }
    let file = File::open(path)?; let opened = file.metadata()?;
    if !opened.is_file() || opened.len() > limit as u64 { return Err(invalid("opened input exceeds its file contract")); }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(opened.len() as usize).map_err(|_| invalid("input allocation refused"))?;
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit { return Err(invalid("input grew past its byte limit")); }
    Ok(bytes)
}

// One output/preflight loop for both modes. This private example adapter exposes
// no mutable inner decoder and cannot turn a held result into a Released variant.
trait MonitoredExecution {
    fn profile(&self) -> &DecoderProfile;
    fn position(&self) -> u64;
    fn status(&self) -> MonitoringStatus;
    fn estimate(&self, tokens: usize) -> Result<DecoderWork, Error>;
    fn monitoring_work(&self) -> MonitoringWork;
    fn decoder_work(&self) -> DecoderWork;
    fn input(&mut self, position: u64, token: u32, budget: DecoderBudget) -> Result<MonitoredStep, Error>;
    fn generate(&mut self, position: u64, budget: DecoderBudget) -> Result<MonitoredStep, Error>;
}
impl MonitoredExecution for MonitoredDecoder {
    fn profile(&self) -> &DecoderProfile { MonitoredDecoder::profile(self) }
    fn position(&self) -> u64 { MonitoredDecoder::position(self) }
    fn status(&self) -> MonitoringStatus { MonitoredDecoder::status(self) }
    fn estimate(&self, tokens: usize) -> Result<DecoderWork, Error> { MonitoredDecoder::estimate(self, tokens) }
    fn monitoring_work(&self) -> MonitoringWork { MonitoredDecoder::monitoring_work(self) }
    fn decoder_work(&self) -> DecoderWork { MonitoredDecoder::decoder_work(self) }
    fn input(&mut self, position: u64, token: u32, budget: DecoderBudget) -> Result<MonitoredStep, Error> {
        self.advance(position, token, budget)
    }
    fn generate(&mut self, position: u64, budget: DecoderBudget) -> Result<MonitoredStep, Error> {
        self.advance_greedy(position, budget)
    }
}
impl MonitoredExecution for MonitoredSampledDecoder {
    fn profile(&self) -> &DecoderProfile { MonitoredSampledDecoder::profile(self) }
    fn position(&self) -> u64 { MonitoredSampledDecoder::position(self) }
    fn status(&self) -> MonitoringStatus { MonitoredSampledDecoder::status(self) }
    fn estimate(&self, tokens: usize) -> Result<DecoderWork, Error> { MonitoredSampledDecoder::estimate(self, tokens) }
    fn monitoring_work(&self) -> MonitoringWork { MonitoredSampledDecoder::monitoring_work(self) }
    fn decoder_work(&self) -> DecoderWork { MonitoredSampledDecoder::decoder_work(self) }
    fn input(&mut self, position: u64, token: u32, budget: DecoderBudget) -> Result<MonitoredStep, Error> {
        self.advance_forced(position, token, budget)
    }
    fn generate(&mut self, position: u64, budget: DecoderBudget) -> Result<MonitoredStep, Error> {
        let sampling = SamplingBudget { vocabulary: self.profile().shape().vocabulary };
        self.advance_sampled(position, SampleBudget { decoder: budget, sampling }).map(|step| step.into_monitored())
    }
}

fn execute(
    session: &mut impl MonitoredExecution, tokens: &[u32], generated: usize, products: u64, output: &mut impl Write,
) -> io::Result<bool> {
    if session.position() != 0 || session.status() != MonitoringStatus::Ready || tokens.is_empty()
        || products == 0 || products > MAX_DECODER_PRODUCTS
    { return Err(invalid("execution requires an unused monitored session and a bounded budget")); }
    let count = tokens.len().checked_add(generated).ok_or_else(|| invalid("token count overflow"))?;
    if tokens.iter().any(|token| *token as usize >= session.profile().shape().vocabulary) {
        return Err(invalid("original token outside loaded vocabulary"));
    }
    let total = session.estimate(count).map_err(numerical)?.scalar_products().map_err(numerical)?;
    if total > products { return Err(invalid("complete prefix and continuation exceed product budget")); }
    for index in 0..count {
        let position = session.position();
        let terms = session.estimate(1).map_err(numerical)?.scalar_products().map_err(numerical)?;
        let budget = DecoderBudget { scalar_products: terms };
        let next = match tokens.get(index) {
            Some(token) => session.input(position, *token, budget),
            None => session.generate(position, budget),
        }.map_err(numerical)?;
        let review = next.review(); let work = session.monitoring_work();
        match &next {
            MonitoredStep::Released(step) => {
                if index >= tokens.len() {
                    writeln!(output, "{{\"kind\":\"generated\",\"token_id\":{},\"position\":{},\"monitor_generation\":{},\"reviewed_layers\":{},\"monitor_encoded_bytes\":{},\"permission\":\"not_issued\"}}",
                        step.step().token, position, review.generation(), review.layers().len(), work.encoded_bytes)?;
                } else {
                    writeln!(output, "{{\"kind\":\"input_reviewed\",\"position\":{},\"monitor_generation\":{},\"reviewed_layers\":{},\"monitor_encoded_bytes\":{},\"permission\":\"not_issued\"}}",
                        position, review.generation(), review.layers().len(), work.encoded_bytes)?;
                }
            }
            MonitoredStep::Held(_) => {
                // No selected token or logits are included on this branch.
                writeln!(output, "{{\"kind\":\"held\",\"position\":{},\"outcome\":\"{:?}\",\"reviewed_layers\":{},\"unreviewed_layers\":{},\"monitor_encoded_bytes\":{},\"monitor_probe_coordinates\":{},\"inference_tokens\":{},\"permission\":\"not_issued\"}}",
                    position, review.outcome(), review.layers().len(), review.unreviewed_layers(), work.encoded_bytes,
                    work.probe_coordinates, session.decoder_work().tokens)?;
                output.flush()?; return Ok(false);
            }
        }
        output.flush()?;
    }
    let work = session.monitoring_work();
    writeln!(output, "{{\"kind\":\"complete\",\"prefix_tokens\":{},\"generated_tokens\":{},\"scalar_products\":{},\"monitor_frame_reviews\":{},\"monitor_encoded_bytes\":{},\"monitor_probe_coordinates\":{},\"permission\":\"not_issued\"}}",
        tokens.len(), generated, session.decoder_work().scalar_products().map_err(numerical)?,
        work.frame_reviews, work.encoded_bytes, work.probe_coordinates)?;
    output.flush()?; Ok(true)
}
fn run() -> Result<bool, Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).take(9).collect();
    if args.len() != 7 && args.len() != 8 { return Err(invalid(USAGE).into()); }
    let number = |index: usize| -> io::Result<u64> {
        let text = args[index].to_str().ok_or_else(|| invalid(USAGE))?;
        if text.is_empty() || text.len() > 20 || !text.bytes().all(|byte| byte.is_ascii_digit()) { return Err(invalid(USAGE)); }
        text.parse().map_err(|_| invalid(USAGE))
    };
    let context = usize::try_from(number(4)?).map_err(numerical)?;
    let generated = usize::try_from(number(5)?).map_err(numerical)?;
    let products = number(6)?;
    if context == 0 || context > MAX_KV_POSITIONS || products == 0 || products > MAX_DECODER_PRODUCTS {
        return Err(invalid("execution budget outside reference limits").into());
    }
    let (identity, tokens) = request(&read_file(Path::new(&args[2]), MAX_REQUEST_BYTES)?)?;
    if tokens.len().checked_add(generated).is_none_or(|count| count > context) {
        return Err(invalid("prefix plus continuation exceeds context").into());
    }
    let monitor = read_file(Path::new(&args[3]), MAX_MONITOR_CONFIG_BYTES)?;
    let sampling = args.get(7).map(|path| read_file(Path::new(path), MAX_SAMPLING_CONFIG_BYTES)).transpose()?;
    let (model, _) = DecoderModel::from_llama_files(identity, context, &args[0], &args[1], CheckpointFileLimits::default())?;
    match sampling {
        Some(config) => {
            let mut session = MonitoredSampledDecoder::from_json(model, 1, &monitor, &config)?;
            Ok(execute(&mut session, &tokens, generated, products, &mut io::stdout().lock())?)
        }
        None => {
            let mut session = MonitoredDecoder::from_json(model, 1, &monitor)?;
            Ok(execute(&mut session, &tokens, generated, products, &mut io::stdout().lock())?)
        }
    }
}
fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(2),
        Err(error) => { eprintln!("{error}"); ExitCode::FAILURE }
    }
}

#[cfg(test)]
#[path = "../tests/support/decoder_fixture.rs"]
mod fixture;
#[cfg(test)]
mod tests {
    use super::*;
    const CONFIG: &[u8] = include_bytes!("../tests/fixtures/decoder_monitor_quiet.json");
    fn session() -> MonitoredDecoder {
        MonitoredDecoder::from_json(fixture::model(fixture::profile(16)), 1, CONFIG).unwrap()
    }
    fn rows(bytes: &[u8]) -> Vec<strict_json::Json> {
        std::str::from_utf8(bytes).unwrap().lines().map(|line| strict_json::parse(line.as_bytes(), Limits::default()).unwrap()).collect()
    }
    #[test]
    fn cli_loop_emits_only_reviewed_generated_ids_equal_to_the_original_engine() {
        let mut guarded = session(); let mut output = Vec::new();
        assert!(execute(&mut guarded, &[1, 2, 0], 8, MAX_DECODER_PRODUCTS, &mut output).unwrap());
        let model = fixture::model(fixture::profile(16));
        let mut raw = model.recompute(1, &[1, 2, 0], DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap();
        let expected: Vec<_> = (0..8).map(|_| raw.advance_greedy(raw.position(), DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap().token as u64).collect();
        let rows = rows(&output);
        let actual: Vec<_> = rows.iter().filter(|row| row.get("kind").and_then(|v| v.as_str()) == Some("generated"))
            .map(|row| row.get("token_id").unwrap().as_u64().unwrap()).collect();
        assert_eq!(actual, expected); assert_eq!(rows.len(), 12);
        assert_eq!(rows.last().unwrap().get("kind").unwrap().as_str(), Some("complete"));
        assert_eq!(guarded.decoder_work(), raw.work());
    }
    #[test]
    fn hold_emits_no_token_or_logits_and_stops_before_any_later_prefix_or_generation() {
        let config = std::str::from_utf8(CONFIG).unwrap().replace("1000000.0", "-1000000.0");
        let mut guarded = MonitoredDecoder::from_json(fixture::model(fixture::profile(16)), 1, config.as_bytes()).unwrap();
        let mut output = Vec::new();
        assert!(!execute(&mut guarded, &[1, 2, 0], 8, MAX_DECODER_PRODUCTS, &mut output).unwrap());
        let rows = rows(&output); assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("kind").unwrap().as_str(), Some("held"));
        assert!(rows[0].get("token_id").is_none()); assert!(rows[0].get("logits").is_none());
        assert_eq!(guarded.position(), 1);
    }
    #[test]
    fn invalid_tail_token_context_or_complete_budget_produce_no_work_or_output() {
        for (tokens, count, budget) in [(vec![1, 2, 99], 1, MAX_DECODER_PRODUCTS),
            (vec![1], 16, MAX_DECODER_PRODUCTS), (vec![1], 2, 1),
            (vec![], 2, MAX_DECODER_PRODUCTS)]
        {
            let mut guarded = session(); let mut output = Vec::new();
            assert!(execute(&mut guarded, &tokens, count, budget, &mut output).is_err());
            assert!(output.is_empty()); assert_eq!(guarded.position(), 0);
            assert_eq!(guarded.status(), MonitoringStatus::Ready);
        }
    }
    #[test]
    fn failed_output_write_does_not_continue_inference() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::Error::from(io::ErrorKind::BrokenPipe)) }
            fn flush(&mut self) -> io::Result<()> { Ok(()) }
        }
        let mut guarded = session();
        assert_eq!(execute(&mut guarded, &[1, 2, 0], 8, MAX_DECODER_PRODUCTS, &mut Broken).unwrap_err().kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(guarded.position(), 1);
    }
    #[test]
    fn original_id_request_refuses_ambiguous_or_capability_bearing_inputs() {
        let bytes = include_bytes!("../tests/fixtures/decoder_original_tokens.json");
        let (identity, tokens) = request(bytes).unwrap(); assert_eq!(identity.model, 2); assert!(!tokens.is_empty());
        let source = std::str::from_utf8(bytes).unwrap();
        let changed = source.replacen("\"tokens\":", "\"permit\":true,\"tokens\":", 1);
        assert_ne!(changed, source); assert!(request(changed.as_bytes()).is_err());
        assert!(request(br#"{"identity":{},"tokens":[1.0]}"#).is_err());
    }

    const SAMPLING: &[u8] = include_bytes!("../tests/fixtures/decoder_sampling.json");
    fn sampled() -> MonitoredSampledDecoder {
        MonitoredSampledDecoder::from_json(fixture::model(fixture::profile(16)), 1, CONFIG, SAMPLING).unwrap()
    }
    #[test]
    fn sampled_cli_reuses_the_same_output_loop_without_emitting_rng_or_logits() {
        let mut session = sampled(); let mut output = Vec::new();
        assert!(execute(&mut session, &[1, 2, 0], 8, MAX_DECODER_PRODUCTS, &mut output).unwrap());
        let model = fixture::model(fixture::profile(16));
        let start = fa_reference::action::consequence::activation::monitor::decoder::sampled::config::SamplingConfig::decode(SAMPLING, 6).unwrap().start();
        let mut raw = model.recompute_sampled(1, &[1, 2, 0], DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }, start).unwrap();
        let expected: Vec<_> = (0..8).map(|_| raw.advance_sampled(raw.position(), SampleBudget {
            decoder: DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }, sampling: SamplingBudget { vocabulary: 6 },
        }).unwrap().choice.token as u64).collect();
        let records = rows(&output);
        let actual: Vec<_> = records.iter().filter(|row| row.get("kind").and_then(|v| v.as_str()) == Some("generated"))
            .map(|row| row.get("token_id").unwrap().as_u64().unwrap()).collect();
        assert_eq!(actual, expected); assert_eq!(session.sampled_draws(), 8);
        assert_eq!(session.decoder_work(), raw.work()); assert_eq!(records.len(), 12);
        for row in records {
            for field in ["random_word", "seed", "logits", "probability", "sampler"] { assert!(row.get(field).is_none()); }
        }
    }
    fn alarm_session(top_k: usize) -> MonitoredSampledDecoder {
        use fa_reference::action::consequence::activation::monitor::{RefinementBudget, RefinementMonitor};
        use fa_reference::action::consequence::activation::probe::LinearProbe;
        use fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderShape;
        use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{SamplingPolicy, SamplingStart};
        use std::collections::BTreeMap;
        let p = DecoderProfile::new(fixture::profile(4).identity(), DecoderShape { vocabulary: 2, hidden: 2,
            intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context: 4 }, 1e-5, 10000.0).unwrap();
        let layers = fixture::zero_layers(&p);
        let model = DecoderModel::new(p, vec![0.0, 1.0, 1.0, 0.0], layers, vec![1.0; 2], vec![0.0; 4]).unwrap();
        let budget = RefinementBudget { encoded_bytes: 10000, probe_coordinates: 10000 };
        let probe = LinearProbe::new(1, 1, model.residual_contract(1).unwrap().profile(), &[1.0, 0.0], 0.0, 0.5).unwrap();
        let monitors = BTreeMap::from([(1, RefinementMonitor::new(vec![probe], vec![23], budget).unwrap())]);
        MonitoredSampledDecoder::new(model, 1, 11, monitors, budget, SamplingStart {
            policy: SamplingPolicy::new(1, 1, 2, 1.0, top_k, 1.0).unwrap(), stream: 99, seed: 0,
        }).unwrap()
    }
    #[test]
    fn generated_sample_alarm_is_withheld_after_a_successfully_reviewed_prefix() {
        let mut session = alarm_session(0); let mut output = Vec::new();
        assert!(!execute(&mut session, &[0], 2, MAX_DECODER_PRODUCTS, &mut output).unwrap());
        let records = rows(&output); assert_eq!(records.len(), 2);
        assert_eq!(records[0].get("kind").unwrap().as_str(), Some("input_reviewed"));
        assert_eq!(records[1].get("kind").unwrap().as_str(), Some("held"));
        for row in records { assert!(row.get("token_id").is_none()); assert!(row.get("random_word").is_none()); }
        assert_eq!(session.position(), 2); assert_eq!(session.sampled_draws(), 1);
        let mut control = alarm_session(1); let mut output = Vec::new();
        assert!(execute(&mut control, &[0], 2, MAX_DECODER_PRODUCTS, &mut output).unwrap());
        assert_eq!(control.sampled_draws(), 2);
        assert_eq!(rows(&output).iter().filter(|row| row.get("kind").and_then(|v| v.as_str()) == Some("generated")).count(), 2);
    }
    #[test]
    fn sampled_run_preflight_never_spends_a_draw_on_an_invalid_late_input() {
        for (tokens, count, products) in [(vec![0, 99], 1, MAX_DECODER_PRODUCTS), (vec![0], 16, MAX_DECODER_PRODUCTS),
            (vec![0], 2, 1), (vec![], 2, MAX_DECODER_PRODUCTS)]
        {
            let mut session = sampled(); let mut output = Vec::new();
            assert!(execute(&mut session, &tokens, count, products, &mut output).is_err());
            assert!(output.is_empty()); assert_eq!(session.position(), 0); assert_eq!(session.sampled_draws(), 0);
        }
    }
    #[test]
    fn failed_sampled_output_does_not_rewind_the_draw_or_generate_a_replacement() {
        struct BrokenAfterPrefix { flushed: bool }
        impl Write for BrokenAfterPrefix {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                if self.flushed { Err(io::ErrorKind::BrokenPipe.into()) } else { Ok(bytes.len()) }
            }
            fn flush(&mut self) -> io::Result<()> { self.flushed = true; Ok(()) }
        }
        let mut session = sampled(); let mut sink = BrokenAfterPrefix { flushed: false };
        assert_eq!(execute(&mut session, &[0], 3, MAX_DECODER_PRODUCTS, &mut sink).unwrap_err().kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(session.position(), 2); assert_eq!(session.sampled_draws(), 1);
        assert!(execute(&mut session, &[0], 3, MAX_DECODER_PRODUCTS, &mut Vec::new()).is_err());
        assert_eq!(session.position(), 2); assert_eq!(session.sampled_draws(), 1);
    }
}
