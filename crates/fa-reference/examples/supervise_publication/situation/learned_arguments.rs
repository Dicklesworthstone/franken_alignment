use super::*;

#[test]
fn learned_recipe_selection_is_optional_unique_and_order_independent() {
    let basic: Vec<String> = ["situation", "config"].into_iter().map(str::to_owned).collect();
    assert!(Arguments::parse(&basic).unwrap().recipe.is_none());
    for pairs in [
        ["--learned-recipe", "recipe", "--history-anchor", "anchor", "--page-size", "1"],
        ["--page-size", "1", "--learned-recipe", "recipe", "--history-anchor", "anchor"],
        ["--history-anchor", "anchor", "--page-size", "1", "--learned-recipe", "recipe"],
    ] {
        let args: Vec<String> = ["situation", "config"].into_iter().chain(pairs).map(str::to_owned).collect();
        let selected = Arguments::parse(&args).unwrap();
        assert_eq!(selected.recipe, Some(Path::new("recipe")));
        assert_eq!(selected.anchor, Some(Path::new("anchor")));
        assert_eq!(selected.page_size, 1);
    }
}

#[test]
fn malformed_learned_options_refuse_before_any_file_or_shared_mutation_flag() {
    for suffix in [
        vec!["--learned-recipe"], vec!["--learned-recipe", ""],
        vec!["--learned-recipe", "--history-anchor"],
        vec!["--learned-recipe", "r", "--learned-recipe", "other"],
        vec!["--learned-recipe", "r", "--resume", "ignored"],
        vec!["--learned-recipe", "r", "--credibility-activation", "ignored"],
        vec!["--learned-recipe", "r", "--andon", "ignored"],
        vec!["--learned-recipe", "r", "--retain-history-anchor", "ignored"],
    ] {
        let args: Vec<String> = ["situation", "missing-config"].into_iter().chain(suffix)
            .map(str::to_owned).collect();
        assert!(matches!(Arguments::parse(&args), Err(error) if error == USAGE));
        assert_eq!(crate::command(args), Err(USAGE.to_owned()));
    }
}
