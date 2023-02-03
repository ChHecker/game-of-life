use std::sync::RwLock;

use criterion::{criterion_group, criterion_main, Criterion};
use game_of_life::game_of_life::*;
use ndarray::{self, Array1};
use rand::{self, Rng};

fn criterion_benchmark(c: &mut Criterion) {
    let mut rng = rand::thread_rng();

    let numx: usize = 100;
    let numy: usize = 100;
    let field_vec_std: Vec<bool> = (0..numx * numy).map(|_| rng.gen_bool(0.3)).collect();
    let field_vec_conv = field_vec_std.clone();
    // let field_vec_fft = field_vec_std.clone().iter().map(|x| *x as isize).collect();

    let field_std = Array1::<bool>::from_vec(field_vec_std)
        .map(|elem| RwLock::new(*elem))
        .into_shape((numx, numy))
        .unwrap();
    let field_conv = Array1::<bool>::from_vec(field_vec_conv)
        .into_shape((numx, numy))
        .unwrap();
    // let field_fft = Array1::<isize>::from_vec(field_vec_fft)
    //     .into_shape((numx, numy))
    //     .unwrap();

    let mut gol_std = GameOfLifeStd::new(field_std);
    let mut gol_conv = GameOfLifeConvolution::new(field_conv);
    // let mut gol_fft = GameOfLifeFFT::new(field_fft, numx, numy);

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
    // c.bench_function("GOL FFT", |b| {
    //     b.iter(|| {
    //         for _ in 0..100 {
    //             gol_fft.compute_new_generation()
    //         }
    //     })
    // });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
