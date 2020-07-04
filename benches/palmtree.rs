use criterion::{
    black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput,
};
use palmtree::StdPalmTree as PalmTree;
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
        // group.bench_with_input(BenchmarkId::new("b+tree/linear", size), size, |b, &size| {
        //     b.iter_batched_ref(
        //         || PalmTree::<usize, usize>::load((0..size).map(|i| (i, i))),
        //         |map| {
        //             for i in 0..size {
        //                 black_box(map.get_linear(&i));
        //             }
        //         },
        //         BatchSize::SmallInput,
        //     )
        // });
        group.bench_with_input(BenchmarkId::new("b+tree", size), size, |b, &size| {
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

fn iterate_owned(c: &mut Criterion) {
    let mut group = c.benchmark_group("iterate_owned");
    for size in SIZES {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("std::btree", size), size, |b, &size| {
            b.iter_batched(
                || BTreeMap::<usize, usize>::from_iter((0..size).map(|i| (i, i))),
                |map| {
                    for entry in map {
                        black_box(entry);
                    }
                },
                BatchSize::SmallInput,
            )
        });
        group.bench_with_input(BenchmarkId::new("b+tree", size), size, |b, &size| {
            b.iter_batched(
                || PalmTree::<usize, usize>::from_iter((0..size).map(|i| (i, i))),
                |map| {
                    for entry in map {
                        black_box(entry);
                    }
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

fn find_key_binary<K>(keys: &[K], key: &K) -> usize
where
    K: Ord,
{
    let size = keys.len();

    let mut low = 0;
    let mut high = size - 1;
    while low != high {
        let mid = (low + high) / 2;
        if unsafe { keys.get_unchecked(mid) } < key {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    low
}

fn branchless_binary_search<K: Ord>(keys: &[K], key: &K) -> usize {
    unsafe {
        let mut base = keys.as_ptr();
        let mut n = keys.len();
        while n > 1 {
            let half = n / 2;
            if *base.add(half) < *key {
                base = base.add(half);
            }
            n -= half;
        }
        ((if *base < *key { base.add(1) } else { base }) as usize - keys.as_ptr() as usize)
            / std::mem::size_of::<K>()
    }
}

pub fn search_strategies(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_strategies");
    for size in &[8, 16, 32, 64, 128, 256usize] {
        let keys = Vec::<u64>::from_iter(0..(*size as u64));
        group.bench_with_input(BenchmarkId::new("binary", size), size, |b, &size| {
            b.iter_batched_ref(
                || Vec::from_iter((0..256u64).map(|i| i % (size as u64))),
                |lookup| {
                    for key in lookup {
                        let index = find_key_binary(&keys, &key);
                        assert_eq!(keys[index], *key);
                    }
                },
                BatchSize::SmallInput,
            )
        });
        group.bench_with_input(BenchmarkId::new("branchless", size), size, |b, &size| {
            b.iter_batched_ref(
                || Vec::from_iter((0..256u64).map(|i| i % (size as u64))),
                |lookup| {
                    for key in lookup {
                        let index = branchless_binary_search(&keys, &key);
                        assert_eq!(keys[index], *key);
                    }
                },
                BatchSize::SmallInput,
            )
        });
    }
}

criterion_group!(
    palmtree,
    insert_sequence,
    insert_random,
    remove_sequence,
    remove_random,
    lookup,
    iterate,
    iterate_owned,
    search_strategies,
);
criterion_main!(palmtree);
