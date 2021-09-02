use criterion::{black_box, criterion_group, criterion_main, Criterion};

use filter::{FilteredItem, Query, SourceItem};
use matcher::{FuzzyAlgorithm, MatchType, Matcher};

use maple_cli::command::ctags::recursive::build_recursive_ctags_cmd;

fn prepare_source_items() -> Vec<SourceItem> {
    use std::io::BufRead;

    std::io::BufReader::new(
        std::fs::File::open("/home/xlc/.cache/vimclap/17131070373568728185").unwrap(), // 1 million +
    )
    .lines()
    .filter_map(|x| x.ok().map(Into::<SourceItem>::into))
    .collect()
}

fn filter(list: Vec<SourceItem>, matcher: &Matcher, query: &Query) -> Vec<FilteredItem> {
    let scorer = |item: &SourceItem| matcher.match_query(item, query);
    list.into_iter()
        .filter_map(|item| scorer(&item).map(|(score, indices)| (item, score, indices)))
        .map(Into::into)
        .collect()
}

// 3 times faster than filter
fn par_filter(list: Vec<SourceItem>, matcher: &Matcher, query: &Query) -> Vec<FilteredItem> {
    use rayon::prelude::*;

    let scorer = |item: &SourceItem| matcher.match_query(item, query);
    list.into_par_iter()
        .filter_map(|item| scorer(&item).map(|(score, indices)| (item, score, indices)))
        .map(Into::into)
        .collect()
}

fn bench_filter(c: &mut Criterion) {
    let source_items = prepare_source_items();

    let take_items = |n: usize| source_items.iter().take(n).cloned().collect::<Vec<_>>();

    let source_items_1k = take_items(1_000);
    let source_items_10k = take_items(10_000);
    let source_items_100k = take_items(100_000);
    let source_items_1m = take_items(1_000_000);

    let matcher = matcher::Matcher::with_bonuses(FuzzyAlgorithm::Fzy, MatchType::Full, Vec::new());
    let query: Query = "executor".into();

    c.bench_function("filter 1k", |b| {
        b.iter(|| filter(black_box(source_items_1k.clone()), &matcher, &query))
    });

    c.bench_function("par filter 1k", |b| {
        b.iter(|| par_filter(black_box(source_items_1k.clone()), &matcher, &query))
    });

    c.bench_function("filter 10k", |b| {
        b.iter(|| filter(black_box(source_items_10k.clone()), &matcher, &query))
    });

    c.bench_function("par filter 10k", |b| {
        b.iter(|| par_filter(black_box(source_items_10k.clone()), &matcher, &query))
    });

    c.bench_function("filter 100k", |b| {
        b.iter(|| filter(black_box(source_items_100k.clone()), &matcher, &query))
    });

    c.bench_function("par filter 100k", |b| {
        b.iter(|| par_filter(black_box(source_items_100k.clone()), &matcher, &query))
    });

    c.bench_function("filter 1m", |b| {
        b.iter(|| filter(black_box(source_items_1m.clone()), &matcher, &query))
    });

    c.bench_function("par filter 1m", |b| {
        b.iter(|| par_filter(black_box(source_items_1m.clone()), &matcher, &query))
    });

    let ctags_cmd =
        build_recursive_ctags_cmd("/home/xlc/src/github.com/paritytech/substrate".into());

    c.bench_function("parallel recursive ctags", |b| {
        b.iter(|| ctags_cmd.par_formatted_lines())
    });

    c.bench_function("recursive ctags", |b| {
        b.iter(|| ctags_cmd.formatted_lines())
    });
}

criterion_group!(benches, bench_filter);
criterion_main!(benches);
