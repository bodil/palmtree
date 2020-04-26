use criterion::{
    black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput,
};
use palmtree::PalmTree;
use rand::prelude::SliceRandom;
use rand::{Rng, SeedableRng};
use std::collections::BTreeMap;
use std::iter::FromIterator;

const SIZES: &[usize] = &[64, 256, 1024, 4096, 16384, 32768, 65536];
// const SIZES: &[usize] = &[256, 65536];

fn insert_sequence(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_sequence");
    for size in SIZES {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("std::BTreeMap::insert", size),
            size,
            |b, &size| {
                b.iter_batched_ref(
                    BTreeMap::<usize, usize>::new,
                    |map| {
                        for i in 0..size {
                            map.insert(i, i);
                        }
                    },
                    BatchSize::SmallInput,
                )
            },
        );
        group.bench_with_input(
            BenchmarkId::new("b+tree::insert", size),
            size,
            |b, &size| {
                b.iter_batched_ref(
                    PalmTree::<usize, usize>::new,
                    |map| {
                        for i in 0..size {
                            map.insert(i, i);
                        }
                    },
                    BatchSize::SmallInput,
                )
            },
        );
        group.bench_with_input(BenchmarkId::new("b+tree::load", size), size, |b, &size| {
            b.iter(|| PalmTree::<usize, usize>::load((0..size).map(|i| (i, i))))
        });
    }
    group.finish();
}

fn insert_random(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_random");
    for size in SIZES {
        let input_data: Vec<(usize, usize)> = rand::rngs::StdRng::seed_from_u64(31337)
            .sample_iter(rand::distributions::Standard)
            .take(*size)
            .collect();
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("std::btree", size),
            &input_data,
            |b, input_data| {
                b.iter_batched_ref(
                    BTreeMap::<usize, usize>::new,
                    |map| {
                        for (k, v) in input_data {
                            map.insert(*k, *v);
                        }
                    },
                    BatchSize::SmallInput,
                )
            },
        );
        group.bench_with_input(
            BenchmarkId::new("b+tree", size),
            &input_data,
            |b, input_data| {
                b.iter_batched_ref(
                    PalmTree::<usize, usize>::new,
                    |map| {
                        for (k, v) in input_data {
                            map.insert(*k, *v);
                        }
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }
    group.finish();
}

fn remove_sequence(c: &mut Criterion) {
    let mut group = c.benchmark_group("remove_sequence");
    for size in SIZES {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("std::btree", size), size, |b, &size| {
            b.iter_batched_ref(
                || BTreeMap::<usize, usize>::from_iter((0..size).map(|i| (i, i))),
                |map| {
                    for k in 0..size {
                        map.remove(&k);
                    }
                },
                BatchSize::SmallInput,
            )
        });
        group.bench_with_input(BenchmarkId::new("b+tree", size), size, |b, &size| {
            b.iter_batched_ref(
                || PalmTree::<usize, usize>::load((0..size).map(|i| (i, i))),
                |map| {
                    for k in 0..size {
                        map.remove(&k);
                    }
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

fn remove_random(c: &mut Criterion) {
    let mut group = c.benchmark_group("remove_random");
    for size in SIZES {
        let mut indices = Vec::from_iter(0..*size);
        indices.shuffle(&mut rand::rngs::StdRng::seed_from_u64(31337));
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("std::btree", size),
            &(&indices, size),
            |b, &(indices, &size)| {
                b.iter_batched_ref(
                    || BTreeMap::<usize, usize>::from_iter((0..size).map(|i| (i, i))),
                    |map| {
                        for k in indices {
                            map.remove(k);
                        }
                    },
                    BatchSize::SmallInput,
                )
            },
        );
        group.bench_with_input(
            BenchmarkId::new("b+tree", size),
            &(&indices, size),
            |b, &(indices, &size)| {
                b.iter_batched_ref(
                    || PalmTree::<usize, usize>::load((0..size).map(|i| (i, i))),
                    |map| {
                        for k in indices {
                            map.remove(k);
                        }
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }
    group.finish();
}

fn lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("lookup");
    for size in SIZES {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("std::btree", size), size, |b, &size| {
            b.iter_batched_ref(
                || BTreeMap::<usize, usize>::from_iter((0..size).map(|i| (i, i))),
                |map| {
                    for i in 0..size {
                        black_box(map.get(&i));
                    }
                },
                BatchSize::SmallInput,
            )
        });
        group.bench_with_input(BenchmarkId::new("b+tree/linear", size), size, |b, &size| {
            b.iter_batched_ref(
                || PalmTree::<usize, usize>::load((0..size).map(|i| (i, i))),
                |map| {
                    for i in 0..size {
                        black_box(map.get_linear(&i));
                    }
                },
                BatchSize::SmallInput,
            )
        });
        group.bench_with_input(BenchmarkId::new("b+tree/binary", size), size, |b, &size| {
            b.iter_batched_ref(
                || PalmTree::<usize, usize>::load((0..size).map(|i| (i, i))),
                |map| {
                    for i in 0..size {
                        black_box(map.get(&i));
                    }
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

fn iterate(c: &mut Criterion) {
    let mut group = c.benchmark_group("iterate");
    for size in SIZES {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("std::btree", size), size, |b, &size| {
            b.iter_batched_ref(
                || BTreeMap::<usize, usize>::from_iter((0..size).map(|i| (i, i))),
                |map| {
                    map.iter().for_each(|i| {
                        black_box(i);
                    });
                },
                BatchSize::SmallInput,
            )
        });
        group.bench_with_input(BenchmarkId::new("b+tree", size), size, |b, &size| {
            b.iter_batched_ref(
                || PalmTree::<usize, usize>::load((0..size).map(|i| (i, i))),
                |map| {
                    map.iter().for_each(|i| {
                        black_box(i);
                    });
                },
                BatchSize::PerIteration,
            )
        });
    }
    group.finish();
}

criterion_group!(
    palmtree,
    insert_sequence,
    insert_random,
    remove_sequence,
    remove_random,
    lookup,
    iterate
);
criterion_main!(palmtree);
