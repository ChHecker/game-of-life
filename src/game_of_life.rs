use std::{path::Path, sync::RwLock};

use fftconvolve::{fftconvolve, Mode};
use indicatif::ProgressBar;
use ndarray::{self, arr2, s, Array2, Zip};
use ndarray_ndimage::convolve;
use plotters::prelude::*;

pub trait GameOfLife {
    type Data;

    /// Generate a new Game of Life from initial field
    fn new(field: Array2<Self::Data>) -> Self;

    /// Compute the next generation
    fn compute_next_generation(&mut self);

    /// Returns the value at (x,y)
    fn cell(&self, x: usize, y: usize) -> bool;
    /// Returns the number of columns
    fn numx(&self) -> usize;
    /// Returns the number of rows
    fn numy(&self) -> usize;

    /// Plots the field as a GIF
    fn start(
        &mut self,
        file: &Path,
        iterations: usize,
        time_per_iteration: u32,
        pb: Option<ProgressBar>,
    ) {
        let area = BitMapBackend::gif(file, (300, 300), time_per_iteration)
            .unwrap()
            .into_drawing_area();
        let subareas = area.split_evenly((self.numx(), self.numy()));

        for _ in 0..iterations {
            for (id, subarea) in subareas.iter().enumerate() {
                let x = id % self.numx();
                let y = id / self.numy();
                let color = if self.cell(x, y) { &WHITE } else { &BLACK };
                subarea.fill(color).unwrap();
            }
            area.present().unwrap();
            self.compute_next_generation();
            if let Some(ref p) = pb {
                p.inc(1);
            }
        }
        println!("Saved Game of Life to {}.", file.display());
    }
}

#[derive(Debug)]
pub struct GameOfLifeStd {
    field: Array2<RwLock<bool>>,
    numx: usize,
    numy: usize,
}

impl GameOfLifeStd {
    /// Counts the living neighbors of the cell at (x, y)
    fn count_living_neighbors(&self, x: usize, y: usize) -> u8 {
        let right_border = if x + 1 < self.numx { x + 2 } else { self.numx };
        let left_border = if x > 1 { x - 1 } else { 0 };
        let border = if y + 1 < self.numy { y + 2 } else { self.numy };
        let top_border = if y > 1 { y - 1 } else { 0 };
        self.field
            .slice(s![left_border..right_border, top_border..border])
            .map(|x| *x.read().unwrap() as u8)
            .sum()
            - *self.field[[x, y]].read().unwrap() as u8
    }
}

impl GameOfLife for GameOfLifeStd {
    type Data = RwLock<bool>;

    fn new(field: Array2<RwLock<bool>>) -> Self {
        let shape = field.shape().to_owned();
        let numx = shape[0];
        let numy = shape[1];
        Self { field, numx, numy }
    }

    /// Computes the new generation
    fn compute_next_generation(&mut self) {
        let mut temp = Array2::<RwLock<bool>>::default((self.numx, self.numy));
        Zip::indexed(&self.field)
            .and(&mut temp)
            .par_for_each(|(x, y), elem_field, elem_temp| {
                let count = self.count_living_neighbors(x, y);
                if count == 3 || (*elem_field.read().unwrap() && count == 2) {
                    *elem_temp.write().unwrap() = true;
                }
            });
        self.field = temp;
    }

    fn cell(&self, x: usize, y: usize) -> bool {
        *self.field[[x, y]].read().unwrap()
    }

    fn numx(&self) -> usize {
        self.numx
    }

    fn numy(&self) -> usize {
        self.numy
    }
}

#[derive(Debug)]
/// Computes the time steps using `ndarray_ndimage`'s `convolve`.
pub struct GameOfLifeConvolution {
    field: Array2<bool>,
    numx: usize,
    numy: usize,
}

impl GameOfLife for GameOfLifeConvolution {
    type Data = bool;

    fn new(field: Array2<bool>) -> Self {
        let shape = field.shape().to_owned();
        let numx = shape[0];
        let numy = shape[1];
        Self { field, numx, numy }
    }

    fn compute_next_generation(&mut self) {
        let kernel = arr2(&[[1, 1, 1], [1, 0, 1], [1, 1, 1]]);
        let temp = convolve(
            &self.field.map(|x| *x as u8),
            &kernel,
            ndarray_ndimage::BorderMode::Constant(0),
            0,
        );
        Zip::from(&mut self.field)
            .and(&temp)
            .par_for_each(|elem_field, elem_temp| {
                if *elem_temp == 3 || (*elem_field && *elem_temp == 2) {
                    *elem_field = true;
                } else {
                    *elem_field = false;
                }
            });
    }

    fn cell(&self, x: usize, y: usize) -> bool {
        self.field[[x, y]]
    }

    fn numx(&self) -> usize {
        self.numx
    }

    fn numy(&self) -> usize {
        self.numy
    }
}

#[derive(Debug)]
/// Computes the time steps using `fftconvolve`.
pub struct GameOfLifeFFT {
    field: Array2<bool>,
    numx: usize,
    numy: usize,
}

impl GameOfLife for GameOfLifeFFT {
    type Data = bool;

    fn new(field: Array2<bool>) -> Self {
        let shape = field.shape().to_owned();
        let numx = shape[0];
        let numy = shape[1];
        Self { field, numx, numy }
    }

    fn compute_next_generation(&mut self) {
        let kernel: Array2<isize> = arr2(&[[1, 1, 1], [1, 0, 1], [1, 1, 1]]);
        let temp: Array2<isize> =
            fftconvolve(&self.field.map(|x| *x as isize), &kernel, Mode::Full).unwrap();
        Zip::from(&mut self.field)
            .and(&temp)
            .par_for_each(|elem_field, elem_temp| {
                if *elem_temp == 3 || (*elem_field && *elem_temp == 2) {
                    *elem_field = true;
                } else {
                    *elem_field = false;
                }
            });
    }

    fn cell(&self, x: usize, y: usize) -> bool {
        self.field[[x, y]]
    }

    fn numx(&self) -> usize {
        self.numx
    }

    fn numy(&self) -> usize {
        self.numy
    }
}

#[cfg(test)]
mod test {
    use ndarray::Array1;
    use rand::Rng;

    use super::*;

    #[test]
    fn count_living_neighbors() {
        let arr = arr2(&[[true, true, true], [true, true, true], [true, true, true]])
            .map(|elem| RwLock::new(*elem));
        let gol = GameOfLifeStd::new(arr);

        let mut temp = Array2::zeros((3, 3));
        for ((x, y), _) in gol.field.indexed_iter() {
            temp[[x, y]] = gol.count_living_neighbors(x, y);
        }

        assert_eq!(temp, arr2(&[[3, 5, 3], [5, 8, 5], [3, 5, 3]]));
    }

    #[test]
    fn compute_next_generation_std() {
        let arr = arr2(&[[true, true, true], [true, true, true], [true, true, true]])
            .map(|elem| RwLock::new(*elem));
        let mut gol = GameOfLifeStd::new(arr);

        gol.compute_next_generation();
        let temp = gol.field.map(|elem| *elem.read().unwrap());

        assert_eq!(
            temp,
            arr2(&[
                [true, false, true],
                [false, false, false],
                [true, false, true]
            ])
        );
    }

    #[test]
    fn compute_next_generation_conv() {
        let arr = arr2(&[[true, true, true], [true, true, true], [true, true, true]]);
        let mut gol = GameOfLifeConvolution::new(arr);

        gol.compute_next_generation();

        assert_eq!(
            gol.field,
            arr2(&[
                [true, false, true],
                [false, false, false],
                [true, false, true]
            ])
        );
    }

    #[test]
    fn algorithms() {
        let mut rng = rand::thread_rng();

        let numx: usize = 10;
        let numy: usize = 10;
        let field_vec_std: Vec<bool> = (0..numx * numy).map(|_| rng.gen_bool(0.3)).collect();
        let field_vec_conv = field_vec_std.clone();
        // let field_vec_fft = field_vec_std.clone();

        let field_std = Array1::<bool>::from_vec(field_vec_std)
            .map(|elem| RwLock::new(*elem))
            .into_shape((numx, numy))
            .unwrap();
        let field_conv = Array1::<bool>::from_vec(field_vec_conv)
            .into_shape((numx, numy))
            .unwrap();
        // let field_fft = Array1::<bool>::from_vec(field_vec_fft)
        //     .into_shape((numx, numy))
        //     .unwrap();

        let mut gol_std = GameOfLifeStd::new(field_std);
        let mut gol_conv = GameOfLifeConvolution::new(field_conv);
        // let mut gol_fft = GameOfLifeFFT::new(field_fft);

        assert!(
            gol_std
                .field
                .iter()
                .zip(gol_conv.field.iter())
                .all(|(x, y)| *x.read().unwrap() == *y),
            "standard and convolution differ"
        );
        // assert!(
        //     gol_std
        //         .field
        //         .iter()
        //         .zip(gol_fft.field.iter())
        //         .all(|(x, y)| *x.read().unwrap() == *y),
        //     "standard and fft differ"
        // );

        gol_std.compute_next_generation();
        gol_conv.compute_next_generation();
        // gol_fft.compute_next_generation();

        assert!(
            gol_std
                .field
                .iter()
                .zip(gol_conv.field.iter())
                .all(|(x, y)| *x.read().unwrap() == *y),
            "standard and convolution differ after one iteration"
        );
    }
}
