//! Real bounded model inputs pass through the original loader after path-only
//! admission. Missing assets distinguish preflight refusal from numerical I/O.
use super as recipe;
use super::super::operator;
use super::*;
use std::{fs, os::unix::net::UnixStream};

#[allow(dead_code)]
mod fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/supervise_publication/workflow/actor_service/native/tests/fixture.rs"
    ));
}
mod split {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/action/consequence/delivery/persistent/observed/decoder/config/sharded/tests/split.rs"
    ));
}
use fixture::Root;

const ALLOWANCE: usize = 4 * 1024 * 1024;
const CUSTODY_REFUSAL: &str = "selected input aliases independently retained custody";
const WEIGHTS: &str = r#"{"index":"index.json","shards":{"left.safetensors":"left.bin","right.safetensors":"right.bin"}}"#;

fn selected(root: &Root, sharded: bool, explicit_tokenizer: bool) -> PathBuf {
    fixture::write_inputs(root);
    let path = root.0.join("recipe.json");
    let mut text = fs::read_to_string(&path).unwrap();
    if sharded {
        let (index, sources) = split::split(&fs::read(root.0.join("model.safetensors")).unwrap());
        fs::write(root.0.join("index.json"), index).unwrap();
        for (label, bytes) in sources {
            let name = if label == "left.safetensors" {
                "left.bin"
            } else {
                "right.bin"
            };
            fs::write(root.0.join(name), bytes).unwrap();
        }
        text = text.replace(
            "\"weights\":\"model.safetensors\"",
            &format!("\"weights\":{WEIGHTS}"),
        );
        fs::remove_file(root.0.join("model.safetensors")).unwrap();
    }
    let version = match (sharded, explicit_tokenizer) {
        (false, false) => 1,
        (true, false) => 2,
        (false, true) => 3,
        (true, true) => 4,
    };
    text = text.replace(
        "fa.native-text-service/1",
        &format!("fa.native-text-service/{version}"),
    );
    if explicit_tokenizer {
        text = text.replace(
            "\"tokenizer\":\"tokenizer.bbpe\"",
            r#""tokenizer":{"format":"native_archive","path":"tokenizer.bbpe"}"#,
        );
    }
    fs::write(&path, text).unwrap();
    path
}

fn paths(root: &Root, sharded: bool) -> Vec<PathBuf> {
    let mut names = vec![
        "recipe.json",
        "model.json",
        "monitor.json",
        "sampling.json",
        "prompt.txt",
        "tokenizer.bbpe",
    ];
    if sharded {
        names.extend(["index.json", "left.bin", "right.bin"]);
    } else {
        names.push("model.safetensors");
    }
    names.into_iter().map(|name| root.0.join(name)).collect()
}

fn same_inputs(actual: &Inputs, expected: &Inputs) {
    assert_eq!(actual.decoder, expected.decoder);
    assert_eq!(
        actual.tokenizer.to_bytes().unwrap(),
        expected.tokenizer.to_bytes().unwrap()
    );
    assert_eq!(actual.stream, expected.stream);
    assert_eq!(actual.generation, expected.generation);
    assert_eq!(actual.request, expected.request);
}

fn failure(result: Result<Inputs, String>) -> String {
    result
        .err()
        .expect("original native input admission must refuse")
}

fn independent(path: &Path, retained: &Path) -> Result<(), String> {
    if operator::resolve(path)? == operator::resolve(retained)? {
        Err(CUSTODY_REFUSAL.into())
    } else {
        Ok(())
    }
}

#[test]
fn native_recipe_path_preflight_preserves_all_four_original_loaders_and_exact_byte_allowance() {
    for (sharded, explicit) in [(false, false), (true, false), (false, true), (true, true)] {
        let root = Root::new();
        let recipe = selected(&root, sharded, explicit);
        let mut expected_paths = paths(&root, sharded);
        let total: usize = expected_paths
            .iter()
            .map(|path| fs::metadata(path).unwrap().len() as usize)
            .sum();
        let original = load(&recipe, 1, total).unwrap();
        let mut seen = Vec::new();
        let admitted = load_with_path_check(&recipe, 1, total, |path| {
            seen.push(path.to_path_buf());
            Ok(())
        })
        .unwrap();
        same_inputs(&admitted, &original);
        seen.sort();
        expected_paths.sort();
        assert_eq!(seen, expected_paths);
        assert!(!seen.contains(&root.0.join("left.safetensors")));
        assert!(!seen.contains(&root.0.join("right.safetensors")));
        // The index labels never become filesystem inputs, and a callback cannot
        // expand the original single aggregate allowance by accepting a path.
        assert_eq!(
            failure(load_with_path_check(&recipe, 1, total - 1, |_| Ok(()))),
            failure(load(&recipe, 1, total - 1))
        );
        assert!(!root.0.join("store").exists());
    }
}

#[test]
fn native_recipe_path_preflight_checks_missing_and_canonical_aliases_before_any_asset_read() {
    for sharded in [false, true] {
        let root = Root::new();
        let recipe = selected(&root, sharded, true);
        let original_paths = paths(&root, sharded);
        let assets: Vec<_> = original_paths
            .iter()
            .filter(|path| **path != recipe)
            .map(|path| (path.clone(), fs::read(path).unwrap()))
            .collect();
        let text = fs::read_to_string(&recipe).unwrap();
        fs::write(
            &recipe,
            text.replace("\"prompt.txt\"", "\"future/prompt.txt\""),
        )
        .unwrap();
        let mut selected_paths = original_paths;
        *selected_paths
            .iter_mut()
            .find(|path| **path == root.0.join("prompt.txt"))
            .unwrap() = root.0.join("future/prompt.txt");
        for (path, _) in &assets {
            fs::remove_file(path).unwrap();
        }
        let alias = root.0.join("alias");
        std::os::unix::fs::symlink(&root.0, &alias).unwrap();
        // Every numerical asset is absent, including the model config which is
        // the first ordinary read. Each custody error therefore must precede it.
        for path in &selected_paths {
            let reserved = alias.join(path.strip_prefix(&root.0).unwrap());
            assert_eq!(
                failure(load_with_path_check(&recipe, 1, ALLOWANCE, |selected| {
                    independent(selected, &reserved)
                })),
                CUSTODY_REFUSAL
            );
        }
        let reserved = alias.join("independent.socket");
        assert_eq!(
            failure(load_with_path_check(&recipe, 1, ALLOWANCE, |selected| {
                independent(selected, &reserved)
            })),
            failure(load(&recipe, 1, ALLOWANCE))
        );
        // Distinct custody reaches the same real model constructor once its
        // independently selected inputs are provisioned, including the new name.
        for (path, bytes) in &assets {
            fs::write(path, bytes).unwrap();
        }
        fs::create_dir(root.0.join("future")).unwrap();
        fs::write(
            root.0.join("future/prompt.txt"),
            fs::read(root.0.join("prompt.txt")).unwrap(),
        )
        .unwrap();
        let admitted = load_with_path_check(&recipe, 1, ALLOWANCE, |selected| {
            independent(selected, &reserved)
        })
        .unwrap();
        same_inputs(&admitted, &load(&recipe, 1, ALLOWANCE).unwrap());
        assert!(!reserved.exists());
        assert!(!root.0.join("store").exists());
    }
}

#[test]
fn native_recipe_path_preflight_cannot_run_before_closed_schema_and_generation_admission() {
    let root = Root::new();
    let recipe = selected(&root, false, false);
    let original = fs::read_to_string(&recipe).unwrap();
    fs::remove_file(root.0.join("model.json")).unwrap();
    for text in [
        original.replacen('{', "{\"unknown\":true,", 1),
        original.replace("\"generation\":7", "\"generation\":0"),
        original.replace("\"max_new_tokens\":2", "\"max_new_tokens\":0"),
        original.replace("\"weights\":\"model.safetensors\"", "\"weights\":{}"),
        original.replace("\"prompt\":\"prompt.txt\"", "\"prompt\":false"),
    ] {
        fs::write(&recipe, text).unwrap();
        assert_eq!(
            failure(load_with_path_check(&recipe, 1, ALLOWANCE, |_| panic!(
                "invalid original recipe reached a custody callback"
            ))),
            failure(load(&recipe, 1, ALLOWANCE))
        );
    }
    fs::write(&recipe, original).unwrap();
    assert_eq!(
        failure(load_with_path_check(&recipe, 1, ALLOWANCE, |_| Err(
            CUSTODY_REFUSAL.into()
        ))),
        CUSTODY_REFUSAL
    );
}

#[test]
fn native_recipe_path_preflight_preserves_original_shard_duplicate_and_index_semantics() {
    let root = Root::new();
    let recipe = selected(&root, true, true);
    let original = fs::read_to_string(&recipe).unwrap();
    // The physical map remains explicit even when two labels select one file.
    // No path deduplication can hide a declared input or replace index validation.
    fs::write(&recipe, original.replace("\"right.bin\"", "\"left.bin\"")).unwrap();
    let expected = failure(load(&recipe, 1, ALLOWANCE));
    let mut seen = Vec::new();
    assert_eq!(
        failure(load_with_path_check(&recipe, 1, ALLOWANCE, |path| {
            seen.push(path.to_path_buf());
            Ok(())
        })),
        expected
    );
    assert_eq!(
        seen.iter()
            .filter(|path| **path == root.0.join("left.bin"))
            .count(),
        2
    );
    fs::write(
        &recipe,
        original.replace(
            "\"right.safetensors\":\"right.bin\"",
            "\"left.safetensors\":\"right.bin\"",
        ),
    )
    .unwrap();
    assert_eq!(
        failure(load_with_path_check(&recipe, 1, ALLOWANCE, |_| panic!(
            "duplicate shard labels reached a custody callback"
        ))),
        failure(load(&recipe, 1, ALLOWANCE))
    );
    fs::write(&recipe, &original).unwrap();
    let index = fs::read(root.0.join("index.json")).unwrap();
    fs::write(root.0.join("index.json"), b"{}").unwrap();
    let expected = failure(load(&recipe, 1, ALLOWANCE));
    assert!(
        expected.contains("native shard index/labels refused"),
        "{expected}"
    );
    assert_eq!(
        failure(load_with_path_check(&recipe, 1, ALLOWANCE, |_| Ok(()))),
        expected
    );
    fs::write(root.0.join("index.json"), index).unwrap();
    same_inputs(
        &load_with_path_check(&recipe, 1, ALLOWANCE, |_| Ok(())).unwrap(),
        &load(&recipe, 1, ALLOWANCE).unwrap(),
    );
}
