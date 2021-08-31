use criterion::{black_box, criterion_group, criterion_main, Criterion};

use filter::{FilteredItem, Query, SourceItem};
use matcher::{FuzzyAlgorithm, MatchType, Matcher};

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
    let matcher = matcher::Matcher::with_bonuses(FuzzyAlgorithm::Fzy, MatchType::Full, Vec::new());
    let query: Query = "executor".into();

    c.bench_function("filter", |b| {
        b.iter(|| filter(black_box(source_items.clone()), &matcher, &query))
    });

    c.bench_function("par filter", |b| {
        b.iter(|| par_filter(black_box(source_items.clone()), &matcher, &query))
    });
}

criterion_group!(benches, bench_filter);
criterion_main!(benches);
