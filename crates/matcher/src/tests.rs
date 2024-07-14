use super::*;
use crate::algo::fzy;
use types::SourceItem;

#[test]
fn test_resize() {
    let total_len = 100;
    let sub_query = "hello";

    let new_indices1 = {
        let mut indices = [1, 2, 3].to_vec();
        let sub_indices = (total_len - sub_query.len()..total_len).collect::<Vec<_>>();
        indices.extend_from_slice(&sub_indices);
        indices
    };

    let new_indices2 = {
        let mut indices = [1, 2, 3].to_vec();
        let mut start = total_len - sub_query.len() - 1;
        let new_len = indices.len() + sub_query.len();
        indices.resize_with(new_len, || {
            start += 1;
            start
        });
        indices
    };

    assert_eq!(new_indices1, new_indices2);
}

#[test]
fn test_match_scope_grep_line() {
    let query = "rules";
    let line = "crates/maple_cli/src/lib.rs:2:1:macro_rules! println_json {";
    let matched_item1 = fzy::fuzzy_indices(line, query, CaseMatching::Smart).unwrap();

    let item = SourceItem::from(line.to_string());
    let fuzzy_text = item.fuzzy_text(MatchScope::GrepLine).unwrap();
    let matched_item2 = FuzzyAlgorithm::Fzy
        .fuzzy_match(query, &fuzzy_text, CaseMatching::Smart)
        .unwrap();

    assert_eq!(matched_item1.indices, matched_item2.indices);
    assert!(matched_item2.score > matched_item1.score);
}

#[test]
fn test_match_scope_filename() {
    let query = "lib";
    let line = "crates/extracted_fzy/src/lib.rs";
    let matched_item1 = fzy::fuzzy_indices(line, query, CaseMatching::Smart).unwrap();

    let item = SourceItem::from(line.to_string());
    let fuzzy_text = item.fuzzy_text(MatchScope::FileName).unwrap();
    let matched_item2 = FuzzyAlgorithm::Fzy
        .fuzzy_match(query, &fuzzy_text, CaseMatching::Smart)
        .unwrap();

    assert_eq!(matched_item1.indices, matched_item2.indices);
    assert!(matched_item2.score > matched_item1.score);
}

#[test]
fn test_filename_bonus() {
    let lines = vec![
        "autoload/clap/filter.vim",
        "autoload/clap/provider/files.vim",
        "lua/fzy_filter.lua",
    ];
    let query = "fil";
    let matcher = MatcherBuilder::new()
        .bonuses(vec![Bonus::FileName])
        .build(query.into());
    for line in lines {
        let item: Arc<dyn ClapItem> = Arc::new(SourceItem::from(line.to_string()));
        let fuzzy_text = item.fuzzy_text(matcher.match_scope()).unwrap();
        let match_result_base = matcher
            .fuzzy_matcher
            .fuzzy_algo
            .fuzzy_match(query, &fuzzy_text, matcher.fuzzy_matcher.case_matching)
            .unwrap();
        let match_result_with_bonus = matcher.match_item(item).unwrap();
        assert!(match_result_base.indices == match_result_with_bonus.indices);
        assert!(match_result_with_bonus.rank[0] > match_result_base.score);
    }
}

#[test]
fn test_language_keyword_bonus() {
    let lines = ["hellorsr foo", "function foo"];
    let query: Query = "fo".into();
    let matcher = MatcherBuilder::new()
        .bonuses(vec![Bonus::Language("vim".into())])
        .build(query);
    let matched_item1 = matcher
        .match_item(Arc::new(lines[0]) as Arc<dyn ClapItem>)
        .unwrap();
    let matched_item2 = matcher
        .match_item(Arc::new(lines[1]) as Arc<dyn ClapItem>)
        .unwrap();
    assert!(matched_item1.indices == matched_item2.indices);
    assert!(matched_item1.rank < matched_item2.rank);
}

#[test]
fn test_exact_search_term_bonus() {
    let lines = ["function foo qwer", "function foo"];
    let query: Query = "'fo".into();
    let matcher = MatcherBuilder::new().build(query);
    let matched_item1 = matcher
        .match_item(Arc::new(lines[0]) as Arc<dyn ClapItem>)
        .unwrap();
    let matched_item2 = matcher
        .match_item(Arc::new(lines[1]) as Arc<dyn ClapItem>)
        .unwrap();
    assert!(matched_item1.indices == matched_item2.indices);
    assert!(matched_item1.rank < matched_item2.rank);
}

#[test]
fn test_search_syntax() {
    let items = vec![
        Arc::new("autoload/clap/provider/search_history.vim"),
        Arc::new("autoload/clap/provider/files.vim"),
        Arc::new("vim-clap/crates/matcher/src/algo.rs"),
        Arc::new("pythonx/clap/scorer.py"),
    ];

    let match_with_query = |query: Query| {
        let matcher = MatcherBuilder::new()
            .bonuses(vec![Bonus::FileName])
            .build(query);
        items
            .clone()
            .into_iter()
            .map(|item| {
                let item: Arc<dyn ClapItem> = item;
                matcher.match_item(item)
            })
            .map(|maybe_matched_item| {
                if let Some(matched_item) = maybe_matched_item {
                    Some(MatchResult::new(matched_item.rank[0], matched_item.indices))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    };

    let query: Query = "clap .vim$ ^auto".into();
    let match_results: Vec<_> = match_with_query(query);
    assert_eq!(
        vec![
            Some(MatchResult::new(
                763,
                [0, 1, 2, 3, 9, 10, 11, 12, 37, 38, 39, 40].to_vec()
            )),
            Some(MatchResult::new(
                776,
                [0, 1, 2, 3, 9, 10, 11, 12, 28, 29, 30, 31].to_vec()
            )),
            None,
            None
        ],
        match_results
    );

    let query: Query = ".rs$".into();
    let match_results: Vec<_> = match_with_query(query);
    assert_eq!(
        vec![
            None,
            None,
            Some(MatchResult::new(24, [32, 33, 34].to_vec())),
            None
        ],
        match_results
    );

    let query: Query = "py".into();
    let match_results: Vec<_> = match_with_query(query);
    assert_eq!(
        vec![
            Some(MatchResult::new(138, [14, 36].to_vec())),
            None,
            None,
            Some(MatchResult::new(383, [0, 1].to_vec()))
        ],
        match_results
    );

    let query: Query = "'py".into();
    let match_results: Vec<_> = match_with_query(query);
    assert_eq!(
        vec![
            None,
            None,
            None,
            Some(MatchResult::new(25, [0, 1].to_vec()))
        ],
        match_results
    );
}

#[test]
fn test_word_matcher() {
    let line = r#"Cargo.toml:19:24:clippy = { path = "crates/cli" }"#;
    let query: Query = "\"cli".into();

    let matcher = MatcherBuilder::new().build(query);

    let match_result = matcher
        .match_item(Arc::new(line) as Arc<dyn ClapItem>)
        .unwrap();

    // match cli instead of clippy
    assert_eq!(
        "cli".to_string(),
        line.chars()
            .enumerate()
            .filter_map(|(idx, c)| if match_result.indices.contains(&idx) {
                Some(c)
            } else {
                None
            })
            .collect::<String>()
    );
}

#[test]
fn test_rank() {
    let items = vec![
        Arc::new("pythonx/clap/fuzzymatch-rs/.cargo/config"),
        Arc::new("crates/maple_core/src/config.rs"),
        Arc::new("config.toml"),
        Arc::new(".editorconfig"),
    ];

    let query: Query = "config".into();
    let matcher = MatcherBuilder::new().build(query);

    for item in items {
        let matched_item = matcher.match_item(item).unwrap();

        println!("{matched_item:?}");
    }
}

#[test]
fn test_grep() {
    let items = vec![
        (
            "substrate/primitives/wasm-interface/src/lib.rs",
            "is_major_syncing: &str",
        ),
        (
            "substrate/client/network/sync/src/strategy.rs",
            "pub fn is_major_syncing(&self) -> bool {",
        ),
    ];
    // let query: Query = "\"is_major_syncing 'fn \"str".into();
    let query: Query = "is_major_syncing 'fn 'strategy".into();
    println!("Query: {query:?}");
    let matcher = MatcherBuilder::new().build(query);
    for (path, line) in items {
        println!("{:?}", matcher.match_file_result(path.as_ref(), line));
    }
}
