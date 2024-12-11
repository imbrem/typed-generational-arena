#[macro_use]
extern crate criterion;
extern crate typed_generational_arena;

use criterion::{BenchmarkId, Criterion, Throughput};
use typed_generational_arena::{Arena, Index};

#[derive(Default)]
#[allow(dead_code)]
struct Small(usize);

#[derive(Default)]
#[allow(dead_code)]
struct Big([usize; 32]);

fn insert<T: Default>(n: usize) {
    let mut arena = Arena::<T>::new();
    for _ in 0..n {
        let idx = arena.insert(Default::default());
        criterion::black_box(idx);
    }
}

fn lookup<T>(arena: &Arena<T>, idx: Index<T>, n: usize) {
    for _ in 0..n {
        criterion::black_box(&arena[idx]);
    }
}

fn collect<T>(arena: &Arena<T>, n: usize) {
    for _ in 0..n {
        criterion::black_box(arena.iter().collect::<Vec<_>>());
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert-small");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            b.iter(|| insert::<Small>(*n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("insert-big");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            b.iter(|| insert::<Big>(*n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("lookup-small");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            let mut small_arena = Arena::<Small>::new();
            for _ in 0..1024 {
                small_arena.insert(Default::default());
            }
            let small_idx = small_arena.iter().map(|pair| pair.0).next().unwrap();
            b.iter(|| lookup(&small_arena, small_idx, *n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("lookup-big");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            let mut big_arena = Arena::<Big>::new();
            for _ in 0..1024 {
                big_arena.insert(Default::default());
            }
            let big_idx = big_arena.iter().map(|pair| pair.0).next().unwrap();
            b.iter(|| lookup(&big_arena, big_idx, *n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("collect-small");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            let mut small_arena = Arena::<Small>::new();
            for _ in 0..1024 {
                small_arena.insert(Default::default());
            }
            b.iter(|| collect(&small_arena, *n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("collect-big");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            let mut big_arena = Arena::<Big>::new();
            for _ in 0..1024 {
                big_arena.insert(Default::default());
            }
            b.iter(|| collect(&big_arena, *n))
        });
    }
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
