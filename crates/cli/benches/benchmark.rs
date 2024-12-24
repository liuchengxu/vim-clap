use cli::command::dumb_jump::DumbJump;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use filter::{MatchedItem, Query, SourceItem};
use maple_core::find_largest_cache_digest;
use maple_core::tools::ctags::{ProjectCtagsCommand, ProjectTag};
use matcher::{Matcher, MatcherBuilder};
use rayon::prelude::*;
use std::io::BufRead;
use std::sync::Arc;
use types::ClapItem;
use utils::io::line_count;

fn prepare_source_items() -> Vec<SourceItem> {
    let largest_cache = find_largest_cache_digest().expect("Cache is empty");
    println!("====  Total items: {}  ====", largest_cache.total);

    std::io::BufReader::new(std::fs::File::open(&largest_cache.cached_path).unwrap())
        .lines()
        .filter_map(|x| x.ok().map(Into::<SourceItem>::into))
        .collect()
}

fn filter_sequential(list: Vec<SourceItem>, matcher: &Matcher) -> Vec<MatchedItem> {
    list.into_iter()
        .filter_map(|item| {
            let item: Arc<dyn ClapItem> = Arc::new(item);
            matcher.match_item(item)
        })
        .collect()
}

// 3 times faster than filter
fn filter_parallel(list: Vec<SourceItem>, matcher: &Matcher) -> Vec<MatchedItem> {
    list.into_par_iter()
        .filter_map(|item| {
            let item: Arc<dyn ClapItem> = Arc::new(item);
            matcher.match_item(item)
        })
        .collect()
}

fn bench_filter(c: &mut Criterion) {
    let source_items = prepare_source_items();
    let total_items = source_items.len();

    let take_items = |n: usize| source_items.iter().take(n).cloned().collect::<Vec<_>>();

    let query: Query = "executor".into();
    let matcher = MatcherBuilder::new().build(query);

    if total_items > 1_000 {
        let source_items_1k = take_items(1_000);
        c.bench_function("filter_sequential 1k", |b| {
            b.iter(|| filter_sequential(black_box(source_items_1k.clone()), &matcher))
        });

        c.bench_function("filter_parallel 1k", |b| {
            b.iter(|| filter_parallel(black_box(source_items_1k.clone()), &matcher))
        });
    }

    if total_items > 10_000 {
        let source_items_10k = take_items(10_000);
        c.bench_function("filter_sequential 10k", |b| {
            b.iter(|| filter_sequential(black_box(source_items_10k.clone()), &matcher))
        });

        c.bench_function("filter_parallel 10k", |b| {
            b.iter(|| filter_parallel(black_box(source_items_10k.clone()), &matcher))
        });
    }

    if total_items > 100_000 {
        let source_items_100k = take_items(100_000);
        c.bench_function("filter_sequential 100k", |b| {
            b.iter(|| filter_sequential(black_box(source_items_100k.clone()), &matcher))
        });

        c.bench_function("filter_parallel 100k", |b| {
            b.iter(|| filter_parallel(black_box(source_items_100k.clone()), &matcher))
        });
    }

    if total_items > 1_000_000 {
        let source_items_1m = take_items(1_000_000);
        c.bench_function("filter_sequential 1m", |b| {
            b.iter(|| filter_sequential(black_box(source_items_1m.clone()), &matcher))
        });

        c.bench_function("filter_parallel 1m", |b| {
            b.iter(|| filter_parallel(black_box(source_items_1m.clone()), &matcher))
        });
    }
}

fn bench_ctags(c: &mut Criterion) {
    let build_ctags_cmd =
        || ProjectCtagsCommand::with_cwd("/home/xlc/src/github.com/paritytech/substrate".into());

    // TODO: Make the parallel version faster, the previous benchmark result in the initial PR
    // https://github.com/liuchengxu/vim-clap/pull/755 is incorrect due to the cwd set incorrectly.
    c.bench_function("parallel recursive ctags", |b| {
        b.iter(|| {
            let mut ctags_cmd = build_ctags_cmd();
            ctags_cmd.par_formatted_lines()
        })
    });

    fn formatted_lines(ctags_cmd: ProjectCtagsCommand) -> Vec<String> {
        ctags_cmd
            .lines()
            .unwrap()
            .filter_map(|tag| {
                if let Ok(tag) = serde_json::from_str::<ProjectTag>(&tag) {
                    Some(tag.format_proj_tag())
                } else {
                    None
                }
            })
            .collect()
    }

    c.bench_function("recursive ctags", |b| {
        b.iter(|| {
            let ctags_cmd = build_ctags_cmd();
            formatted_lines(ctags_cmd)
        })
    });
}

fn bench_regex_searcher(c: &mut Criterion) {
    let dumb_jump = DumbJump {
        word: "unsigned".to_string(),
        extension: "rs".to_string(),
        kind: None,
        cmd_dir: Some("/home/xlc/src/github.com/paritytech/substrate".into()),
        regex: true,
    };

    c.bench_function("regex searcher", |b| {
        b.iter(|| dumb_jump.regex_usages(false, &Default::default()))
    });
}

fn bench_bytecount(c: &mut Criterion) {
    let largest_cache = find_largest_cache_digest().expect("Cache is empty");
    c.bench_function("bytecount", |b| {
        b.iter(|| line_count(&largest_cache.cached_path).unwrap())
    });
}

criterion_group!(
    benches,
    bench_filter,
    bench_ctags,
    bench_regex_searcher,
    bench_bytecount
);
criterion_main!(benches);
