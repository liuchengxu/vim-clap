use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::Rng;
use tree_sitter::{UncheckedUtf8CharIndices, Utf8CharIndices};

fn bench_utf8_char_indices(c: &mut Criterion) {
    const CHARSET: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789)(*&^%$#@!~\n\
    ";

    let multi_byte_set = String::from_utf8_lossy("你好，世界！".as_bytes())
        .chars()
        .collect::<Vec<_>>();

    let mut rng = rand::thread_rng();
    let input = std::iter::repeat_with(|| {
        if rand::random::<bool>() {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        } else {
            let idx = rng.gen_range(0..multi_byte_set.len());
            multi_byte_set[idx]
        }
    })
    .take(10_000)
    .collect::<String>();
    let input = input.as_bytes();

    c.bench_function("Utf8CharIndices", |b| {
        b.iter(|| {
            for (index, ch) in Utf8CharIndices::new(black_box(input)) {
                black_box(index);
                black_box(ch);
            }
        })
    });

    c.bench_function("UncheckedUtf8CharIndices", |b| {
        b.iter(|| {
            for (index, ch) in UncheckedUtf8CharIndices::new(black_box(input)) {
                black_box(index);
                black_box(ch);
            }
        })
    });

    c.bench_function("String::from_utf8_lossy().char_indices()", |b| {
        b.iter(|| {
            let utf8_string = String::from_utf8_lossy(black_box(input));
            for (index, ch) in utf8_string.char_indices() {
                black_box(index);
                black_box(ch);
            }
        })
    });

    assert_eq!(
        String::from_utf8_lossy(input)
            .char_indices()
            .collect::<Vec<_>>(),
        Utf8CharIndices::new(input).collect::<Vec<_>>()
    );

    assert_eq!(
        String::from_utf8_lossy(input)
            .char_indices()
            .collect::<Vec<_>>(),
        UncheckedUtf8CharIndices::new(input).collect::<Vec<_>>()
    );
}

criterion_group!(benches, bench_utf8_char_indices);

criterion_main!(benches);
