use std::io::BufRead;
use std::sync::Arc;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rayon::prelude::*;

use filter::{MatchedItem, MultiItem, Query};
use matcher::{FuzzyAlgorithm, MatchScope, Matcher};
use types::ClapItem;

use maple_cli::command::ctags::recursive_tags::build_recursive_ctags_cmd;
use maple_cli::command::dumb_jump::DumbJump;
use maple_cli::CACHE_INFO_IN_MEMORY;

fn prepare_source_items() -> Vec<MultiItem> {
    let cache_info = CACHE_INFO_IN_MEMORY.lock();
    let mut digests = cache_info.digests();
    digests.sort_unstable_by_key(|digest| digest.total);
    let largest_cache = digests.last().expect("Cache is empty");
    println!("====  Total items: {}  ====", largest_cache.total);

    std::io::BufReader::new(std::fs::File::open(&largest_cache.cached_path).unwrap())
        .lines()
        .filter_map(|x| x.ok().map(Into::<MultiItem>::into))
        .collect()
}

fn filter(list: Vec<MultiItem>, matcher: &Matcher, query: &Query) -> Vec<MatchedItem> {
    list.into_iter()
        .filter_map(|item| {
            let item: Arc<dyn ClapItem> = Arc::new(item);
            matcher.match_item(item, query)
        })
        .collect()
}

// 3 times faster than filter
fn par_filter(list: Vec<MultiItem>, matcher: &Matcher, query: &Query) -> Vec<MatchedItem> {
    list.into_par_iter()
        .filter_map(|item| {
            let item: Arc<dyn ClapItem> = Arc::new(item);
            matcher.match_item(item, query)
        })
        .collect()
}

fn bench_filter(c: &mut Criterion) {
    let source_items = prepare_source_items();
    let total_items = source_items.len();

    let take_items = |n: usize| source_items.iter().take(n).cloned().collect::<Vec<_>>();

    let matcher = Matcher::with_bonuses(Vec::new(), FuzzyAlgorithm::Fzy, MatchScope::Full);
    let query: Query = "executor".into();

    if total_items > 1_000 {
        let source_items_1k = take_items(1_000);
        c.bench_function("filter 1k", |b| {
            b.iter(|| filter(black_box(source_items_1k.clone()), &matcher, &query))
        });

        c.bench_function("par filter 1k", |b| {
            b.iter(|| par_filter(black_box(source_items_1k.clone()), &matcher, &query))
        });
    }

    if total_items > 10_000 {
        let source_items_10k = take_items(10_000);
        c.bench_function("filter 10k", |b| {
            b.iter(|| filter(black_box(source_items_10k.clone()), &matcher, &query))
        });

        c.bench_function("par filter 10k", |b| {
            b.iter(|| par_filter(black_box(source_items_10k.clone()), &matcher, &query))
        });
    }

    if total_items > 100_000 {
        let source_items_100k = take_items(100_000);
        c.bench_function("filter 100k", |b| {
            b.iter(|| filter(black_box(source_items_100k.clone()), &matcher, &query))
        });

        c.bench_function("par filter 100k", |b| {
            b.iter(|| par_filter(black_box(source_items_100k.clone()), &matcher, &query))
        });
    }

    if total_items > 1_000_000 {
        let source_items_1m = take_items(1_000_000);
        c.bench_function("filter 1m", |b| {
            b.iter(|| filter(black_box(source_items_1m.clone()), &matcher, &query))
        });

        c.bench_function("par filter 1m", |b| {
            b.iter(|| par_filter(black_box(source_items_1m.clone()), &matcher, &query))
        });
    }
}

fn bench_ctags(c: &mut Criterion) {
    let ctags_cmd =
        build_recursive_ctags_cmd("/home/xlc/src/github.com/paritytech/substrate".into());

    // TODO: Make the parallel version faster, the previous benchmark result in the initial PR
    // https://github.com/liuchengxu/vim-clap/pull/755 is incorrect due to the cwd set incorrectly.
    c.bench_function("parallel recursive ctags", |b| {
        b.iter(|| ctags_cmd.par_formatted_lines())
    });

    c.bench_function("recursive ctags", |b| {
        b.iter(|| ctags_cmd.formatted_lines())
    });
}

fn bench_regex_searcher(c: &mut Criterion) {
    let dumb_jump = DumbJump {
        word: "unsigned".to_string(),
        extension: "rs".to_string(),
        kind: None,
        cmd_dir: Some("/home/xlc/src/github.com/paritytech/substrate".into()),
    };

    c.bench_function("regex searcher", move |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();

        b.to_async(rt).iter(|| async {
            dumb_jump
                .references_or_occurrences(false, &Default::default())
                .await
                .unwrap();
        })
    });
}

// criterion_group!(benches, bench_filter, bench_ctags, bench_regex_searcher);
criterion_group!(benches, bench_regex_searcher);
criterion_main!(benches);
