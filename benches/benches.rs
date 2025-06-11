#[macro_use]
extern crate criterion;
extern crate typed_generational_arena;

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput};
use typed_generational_arena::{
    PtrSlab, PtrSlabIndex, SmallPtrSlab, SmallPtrSlabIndex, SmallSlab, SmallSlabIndex,
    StandardSlab as Slab, StandardSlabIndex as SlabIndex,
};
use typed_generational_arena::{
    SmallArena, SmallIndex, StandardArena as Arena, StandardIndex as Index,
};

#[allow(dead_code)]
#[derive(Default)]
struct Small(usize);

#[allow(dead_code)]
#[derive(Default)]
struct Big([usize; 32]);

fn insert<T: Default>(n: usize) {
    let mut arena = Arena::<T>::new();
    for _ in 0..n {
        let idx = arena.insert(Default::default());
        black_box(idx);
    }
}

fn lookup<T>(arena: &Arena<T>, idx: Index<T>, n: usize) {
    for _ in 0..n {
        black_box(&arena[idx]);
    }
}

fn collect<T>(arena: &Arena<T>, n: usize) {
    for _ in 0..n {
        black_box(arena.iter().collect::<Vec<_>>());
    }
}

fn u32_insert<T: Default>(n: usize) {
    let mut arena = SmallArena::<T>::new();
    for _ in 0..n {
        let idx = arena.insert(Default::default());
        black_box(idx);
    }
}

fn u32_lookup<T>(arena: &SmallArena<T>, idx: SmallIndex<T>, n: usize) {
    for _ in 0..n {
        black_box(&arena[idx]);
    }
}

fn u32_collect<T>(arena: &SmallArena<T>, n: usize) {
    for _ in 0..n {
        black_box(arena.iter().collect::<Vec<_>>());
    }
}

fn slab_insert<T: Default>(n: usize) {
    let mut slab = Slab::<T>::new();
    for _ in 0..n {
        let idx = slab.insert(Default::default());
        black_box(idx);
    }
}

fn slab_lookup<T>(slab: &Slab<T>, idx: SlabIndex<T>, n: usize) {
    for _ in 0..n {
        black_box(&slab[idx]);
    }
}

fn slab_collect<T>(slab: &Slab<T>, n: usize) {
    for _ in 0..n {
        black_box(slab.iter().collect::<Vec<_>>());
    }
}

fn u32_slab_insert<T: Default>(n: usize) {
    let mut slab = SmallSlab::<T>::new();
    for _ in 0..n {
        let idx = slab.insert(Default::default());
        black_box(idx);
    }
}

fn u32_slab_lookup<T>(slab: &SmallSlab<T>, idx: SmallSlabIndex<T>, n: usize) {
    for _ in 0..n {
        black_box(&slab[idx]);
    }
}

fn u32_slab_collect<T>(slab: &SmallSlab<T>, n: usize) {
    for _ in 0..n {
        black_box(slab.iter().collect::<Vec<_>>());
    }
}

fn ptr_slab_insert<T: Default>(n: usize) {
    let mut slab = PtrSlab::<T>::new();
    for _ in 0..n {
        let idx = slab.insert(Default::default());
        black_box(idx);
    }
}

fn ptr_slab_lookup<T>(slab: &PtrSlab<T>, idx: PtrSlabIndex<T>, n: usize) {
    for _ in 0..n {
        black_box(&slab[idx]);
    }
}

fn ptr_slab_collect<T>(slab: &PtrSlab<T>, n: usize) {
    for _ in 0..n {
        black_box(slab.iter().collect::<Vec<_>>());
    }
}

fn u32_ptr_slab_insert<T: Default>(n: usize) {
    let mut slab = SmallPtrSlab::<T>::new();
    for _ in 0..n {
        let idx = slab.insert(Default::default());
        black_box(idx);
    }
}

fn u32_ptr_slab_lookup<T>(slab: &SmallPtrSlab<T>, idx: SmallPtrSlabIndex<T>, n: usize) {
    for _ in 0..n {
        black_box(&slab[idx]);
    }
}

fn u32_ptr_slab_collect<T>(slab: &SmallPtrSlab<T>, n: usize) {
    for _ in 0..n {
        black_box(slab.iter().collect::<Vec<_>>());
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

    let mut group = c.benchmark_group("slab-insert-small");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            b.iter(|| slab_insert::<Small>(*n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("slab-insert-big");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            b.iter(|| slab_insert::<Big>(*n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("slab-lookup-small");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            let mut small_slab = Slab::<Small>::new();
            for _ in 0..1024 {
                small_slab.insert(Default::default());
            }
            let small_idx = small_slab.iter().map(|pair| pair.0).next().unwrap();
            b.iter(|| slab_lookup(&small_slab, small_idx, *n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("slab-lookup-big");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            let mut big_slab = Slab::<Big>::new();
            for _ in 0..1024 {
                big_slab.insert(Default::default());
            }
            let big_idx = big_slab.iter().map(|pair| pair.0).next().unwrap();
            b.iter(|| slab_lookup(&big_slab, big_idx, *n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("slab-collect-small");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            let mut small_slab = Slab::<Small>::new();
            for _ in 0..1024 {
                small_slab.insert(Default::default());
            }
            b.iter(|| slab_collect(&small_slab, *n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("slab-collect-big");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            let mut big_slab = Slab::<Big>::new();
            for _ in 0..1024 {
                big_slab.insert(Default::default());
            }
            b.iter(|| slab_collect(&big_slab, *n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("u32-insert-small");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            b.iter(|| u32_insert::<Small>(*n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("u32-insert-big");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            b.iter(|| u32_insert::<Big>(*n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("u32-lookup-small");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            let mut small_arena = SmallArena::<Small>::new();
            for _ in 0..1024 {
                small_arena.insert(Default::default());
            }
            let small_idx = small_arena.iter().map(|pair| pair.0).next().unwrap();
            b.iter(|| u32_lookup(&small_arena, small_idx, *n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("u32-lookup-big");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            let mut big_arena = SmallArena::<Big>::new();
            for _ in 0..1024 {
                big_arena.insert(Default::default());
            }
            let big_idx = big_arena.iter().map(|pair| pair.0).next().unwrap();
            b.iter(|| u32_lookup(&big_arena, big_idx, *n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("u32-collect-small");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            let mut small_arena = SmallArena::<Small>::new();
            for _ in 0..1024 {
                small_arena.insert(Default::default());
            }
            b.iter(|| u32_collect(&small_arena, *n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("u32-collect-big");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            let mut big_arena = SmallArena::<Big>::new();
            for _ in 0..1024 {
                big_arena.insert(Default::default());
            }
            b.iter(|| u32_collect(&big_arena, *n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("u32-slab-insert-small");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            b.iter(|| u32_slab_insert::<Small>(*n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("u32-slab-insert-big");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            b.iter(|| u32_slab_insert::<Big>(*n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("u32-slab-lookup-small");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            let mut small_slab = SmallSlab::<Small>::new();
            for _ in 0..1024 {
                small_slab.insert(Default::default());
            }
            let small_idx = small_slab.iter().map(|pair| pair.0).next().unwrap();
            b.iter(|| u32_slab_lookup(&small_slab, small_idx, *n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("u32-slab-lookup-big");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            let mut big_slab = SmallSlab::<Big>::new();
            for _ in 0..1024 {
                big_slab.insert(Default::default());
            }
            let big_idx = big_slab.iter().map(|pair| pair.0).next().unwrap();
            b.iter(|| u32_slab_lookup(&big_slab, big_idx, *n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("u32-slab-collect-small");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            let mut small_slab = SmallSlab::<Small>::new();
            for _ in 0..1024 {
                small_slab.insert(Default::default());
            }
            b.iter(|| u32_slab_collect(&small_slab, *n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("u32-slab-collect-big");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            let mut big_slab = SmallSlab::<Big>::new();
            for _ in 0..1024 {
                big_slab.insert(Default::default());
            }
            b.iter(|| u32_slab_collect(&big_slab, *n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("ptr-slab-insert-small");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            b.iter(|| ptr_slab_insert::<Small>(*n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("ptr-slab-insert-big");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            b.iter(|| ptr_slab_insert::<Big>(*n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("ptr-slab-lookup-small");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            let mut small_slab = PtrSlab::<Small>::new();
            for _ in 0..1024 {
                small_slab.insert(Default::default());
            }
            let small_idx = small_slab.iter().map(|pair| pair.0).next().unwrap();
            b.iter(|| ptr_slab_lookup(&small_slab, small_idx, *n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("ptr-slab-lookup-big");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            let mut big_slab = PtrSlab::<Big>::new();
            for _ in 0..1024 {
                big_slab.insert(Default::default());
            }
            let big_idx = big_slab.iter().map(|pair| pair.0).next().unwrap();
            b.iter(|| ptr_slab_lookup(&big_slab, big_idx, *n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("ptr-slab-collect-small");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            let mut small_slab = PtrSlab::<Small>::new();
            for _ in 0..1024 {
                small_slab.insert(Default::default());
            }
            b.iter(|| ptr_slab_collect(&small_slab, *n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("ptr-slab-collect-big");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            let mut big_slab = PtrSlab::<Big>::new();
            for _ in 0..1024 {
                big_slab.insert(Default::default());
            }
            b.iter(|| ptr_slab_collect(&big_slab, *n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("u32-ptr-slab-insert-small");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            b.iter(|| u32_ptr_slab_insert::<Small>(*n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("u32-ptr-slab-insert-big");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            b.iter(|| u32_ptr_slab_insert::<Big>(*n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("u32-ptr-slab-lookup-small");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            let mut small_slab = SmallPtrSlab::<Small>::new();
            for _ in 0..1024 {
                small_slab.insert(Default::default());
            }
            let small_idx = small_slab.iter().map(|pair| pair.0).next().unwrap();
            b.iter(|| u32_ptr_slab_lookup(&small_slab, small_idx, *n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("u32-ptr-slab-lookup-big");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            let mut big_slab = SmallPtrSlab::<Big>::new();
            for _ in 0..1024 {
                big_slab.insert(Default::default());
            }
            let big_idx = big_slab.iter().map(|pair| pair.0).next().unwrap();
            b.iter(|| u32_ptr_slab_lookup(&big_slab, big_idx, *n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("u32-ptr-slab-collect-small");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            let mut small_slab = SmallPtrSlab::<Small>::new();
            for _ in 0..1024 {
                small_slab.insert(Default::default());
            }
            b.iter(|| u32_ptr_slab_collect(&small_slab, *n))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("u32-ptr-slab-collect-big");
    for n in 1..3 {
        let n = n * 100;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, n| {
            let mut big_slab = SmallPtrSlab::<Big>::new();
            for _ in 0..1024 {
                big_slab.insert(Default::default());
            }
            b.iter(|| u32_ptr_slab_collect(&big_slab, *n))
        });
    }
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
