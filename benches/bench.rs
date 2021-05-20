use skiplist::SkipList;
use criterion::{criterion_main, criterion_group, Criterion};

fn basic<const N: usize>(c: &mut Criterion) {
    let mut l = SkipList::<i32, 32>::new();
    let rng = fastrand::Rng::new();
    let mut nums = Vec::with_capacity(N);
    for _ in 0..N {
        let i = rng.i32(..);
        l.insert(i);
        nums.push(i);
    }

    c.bench_function(format!("contains(): N = {}", N).as_str(), |b| b.iter(|| {
        let i = rng.usize(0..nums.len());
        l.contains(&nums[i])
    }));
}

criterion_group!(basics, basic<100>, basic<1000>, basic<10000>, basic<100000>, basic<1000000>);
criterion_main!(basics);