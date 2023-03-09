use std::sync::RwLock;

use criterion::{criterion_group, criterion_main, Criterion};
use game_of_life::gameoflife::*;
use ndarray::{self, Array1};
use rand::{self, Rng};

fn criterion_benchmark(c: &mut Criterion) {
    let mut rng = rand::thread_rng();

    let numx: usize = 100;
    let numy: usize = 100;
    let field_vec_std: Vec<u8> = (0..numx * numy).map(|_| rng.gen_bool(0.3) as u8).collect();
    let field_vec_conv = field_vec_std.clone();

    let field_std = Array1::<u8>::from_vec(field_vec_std)
        .map(|elem| RwLock::new(*elem))
        .into_shape((numx, numy))
        .unwrap();
    let field_conv = Array1::<u8>::from_vec(field_vec_conv)
        .into_shape((numx, numy))
        .unwrap();

    let rules_std = Rule::new(
        LifeRule::Raw([false, false, true, true, false, false, false, false, false]),
        LifeRule::Raw([false, false, false, true, false, false, false, false, false]),
        1,
        NeighborRule::Moore,
    );
    let rules_conv = rules_std.clone();

    let mut gol_std = GameOfLifeStd::new(field_std, rules_std);
    let mut gol_conv = GameOfLifeConvolution::new(field_conv, rules_conv);

    c.bench_function("GOL Std", |b| {
        b.iter(|| {
            for _ in 0..20 {
                gol_std.compute_next_generation()
            }
        })
    });
    c.bench_function("GOL Conv", |b| {
        b.iter(|| {
            for _ in 0..20 {
                gol_conv.compute_next_generation()
            }
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
