use super::*;
use super::super::tests::fixture::*;
use std::fs;

#[test]
fn generated_publication_recipe_loads_real_files_and_binds_every_native_configuration() {
    let root = Root::new(); let config = configured(&root); let path = write_recipe(&root);
    let loaded = load(&path, &config).unwrap();
    assert_eq!(loaded.text.prompt, b"x"); assert_eq!(loaded.request, 1); assert_eq!(loaded.generation, 7);
    assert_eq!(loaded.decoder.profile(), &model_profile());
    assert_eq!(loaded.tokenizer.to_bytes().unwrap(), tokenizer().to_bytes().unwrap());
    assert!(!config.store.exists()); // loading has no live owner, output or key
    let monitor = root.0.join("monitor.json");
    let old = fs::read(&monitor).unwrap();
    fs::write(&monitor, String::from_utf8(old).unwrap().replace("\"model\":9", "\"model\":10")).unwrap();
    assert!(load(&path, &config).is_err()); assert!(!config.store.exists());
}

#[test]
fn generated_publication_recipe_refuses_unknown_duplicate_oversized_and_malformed_fields() {
    let root = Root::new(); let path = write_recipe(&root); let text = fs::read_to_string(path).unwrap();
    assert!(Recipe::decode(text.as_bytes()).is_ok());
    for changed in [text.replacen("\"schema\":", "\"extra\":true,\"schema\":", 1),
        text.replacen("\"request\":1", "\"request\":1,\"request\":1", 1),
        text.replace("\"request\":1", "\"request\":0"),
        text.replace("\"max_new_tokens\":2", "\"max_new_tokens\":4097"),
        text.replace("\"stop_tokens\":[256]", "\"stop_tokens\":[]"),
        text.replace("\"prefix_controls\":[256]", "\"prefix_controls\":[4294967296]"),
        text.replace("\"scalar_products\":1000000", "\"scalar_products\":-1"),
        text.replace("/model.json", "/../model.json"),
        text.replace("\"ttl_ms\":100000", "\"ttl_ms\":3600001")]
    { assert!(Recipe::decode(changed.as_bytes()).is_err()); }
    assert!(Recipe::decode(&vec![b' '; MAX_RECIPE_BYTES + 1]).is_err());
    assert!(Recipe::decode(&text.as_bytes()[..text.len() - 1]).is_err());
}

#[test]
fn generated_publication_recipe_rejects_tied_and_foreign_models_without_opening_weights() {
    let root = Root::new(); let config = configured(&root); let path = write_recipe(&root);
    let original = fs::read_to_string(&path).unwrap();
    fs::remove_file(root.0.join("weights.safetensors")).unwrap();
    let foreign = original.replace("\"tenant\":1", "\"tenant\":2");
    fs::write(&path, foreign).unwrap();
    assert!(matches!(load(&path, &config), Err(error) if error.contains("tenant")));
    fs::write(&path, original).unwrap();
    let model = root.0.join("model.json"); let original = fs::read_to_string(&model).unwrap();
    fs::write(model, original.replacen('{', "{\"tie_word_embeddings\":true,", 1)).unwrap();
    assert!(matches!(load(&path, &config), Err(error) if error.contains("independent output head")));
    assert!(!config.store.exists());
}
