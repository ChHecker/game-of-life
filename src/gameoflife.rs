//! # GameOfLife
//! Contains a collection of structure necessary for building a Game of Life.

use std::{fmt::Display, ops::Range, str::FromStr, sync::RwLock};

use ndarray::{self, arr2, s, Array2, Zip};
use ndarray_ndimage::convolve;

#[derive(Clone)]
pub enum NeighborRule {
    Moore,
    VonNeumann,
}

impl FromStr for NeighborRule {
    type Err = ();

    fn from_str(input: &str) -> Result<NeighborRule, Self::Err> {
        match input.to_lowercase().as_str() {
            "m" => Ok(NeighborRule::Moore),
            "moore" => Ok(NeighborRule::Moore),
            "vn" => Ok(NeighborRule::VonNeumann),
            "vonneumann" => Ok(NeighborRule::VonNeumann),
            _ => Err(()),
        }
    }
}

impl Display for NeighborRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            NeighborRule::Moore => write!(f, "Moore"),
            NeighborRule::VonNeumann => write!(f, "von Neumann"),
        }
    }
}

#[derive(Clone)]
pub enum LifeRule {
    One(usize),
    Range(Range<usize>),
    Raw([bool; 9]),
}

impl LifeRule {
    fn to_array(self) -> [bool; 9] {
        let mut return_array = [false; 9];
        match self {
            LifeRule::One(one) => return_array[one] = true,
            LifeRule::Range(range) => {
                for i in range {
                    return_array[i] = true;
                }
            }
            LifeRule::Raw(array) => return_array = array,
        }
        return_array
    }
}

/// Rule of a Game of Life
/// - `survival`: With how many neighbors a living cell survives
/// - `birth`: With how many neighbors a dead cell is born
/// - `state`: After how many iterations a cell dies
/// - `neighbor`: Neighbor counting algorithm
#[derive(Clone)]
pub struct Rule {
    survival: [bool; 9],
    birth: [bool; 9],
    state: u8,
    neighbor: NeighborRule,
}

impl Rule {
    pub fn new(survival: LifeRule, birth: LifeRule, state: u8, neighbor: NeighborRule) -> Self {
        // TODO: more comfort in survival and birth
        Self {
            survival: survival.to_array(),
            birth: birth.to_array(),
            state,
            neighbor,
        }
    }
}

/// Every Game of Life algorithm should implement this trait.
pub trait GameOfLife {
    type Data;

    /// Generate a new Game of Life from initial field
    fn new(field: Array2<Self::Data>, rules: Rule) -> Self;

    /// Compute the next generation
    fn compute_next_generation(&mut self);

    /// Returns the value at (x,y)
    fn cell(&self, x: usize, y: usize) -> Option<u8>;
    /// Returns the number of columns
    fn numx(&self) -> usize;
    /// Returns the number of rows
    fn numy(&self) -> usize;
}

/// Computes the time steps using ordinary iterations.
pub struct GameOfLifeStd {
    field: Array2<RwLock<u8>>,
    rules: Rule,
    numx: usize,
    numy: usize,
}

impl GameOfLifeStd {
    /// Counts the living neighbors of the cell at (x, y)
    fn count_living_neighbors(&self, x: usize, y: usize) -> usize {
        match self.rules.neighbor {
            NeighborRule::Moore => {
                let right_border = if x + 1 < self.numx { x + 2 } else { self.numx };
                let left_border = if x > 1 { x - 1 } else { 0 };
                let bottom_border = if y + 1 < self.numy { y + 2 } else { self.numy };
                let top_border = if y > 1 { y - 1 } else { 0 };
                self.field
                    .slice(s![left_border..right_border, top_border..bottom_border])
                    .map(|x| (*x.read().unwrap() == self.rules.state) as usize)
                    .sum()
                    - (*self.field[[x, y]].read().unwrap() == self.rules.state) as usize
            }
            NeighborRule::VonNeumann => {
                let mut sum = 0;
                if x != 0 {
                    if let Some(cell) = self.cell(x - 1, y) {
                        sum += (cell == self.rules.state) as usize;
                    }
                }
                if let Some(cell) = self.cell(x + 1, y) {
                    sum += (cell == self.rules.state) as usize;
                }
                if y != 0 {
                    if let Some(cell) = self.cell(x, y - 1) {
                        sum += (cell == self.rules.state) as usize;
                    }
                }
                if let Some(cell) = self.cell(x, y + 1) {
                    sum += (cell == self.rules.state) as usize;
                }
                sum
            }
        }
    }
}

impl GameOfLife for GameOfLifeStd {
    type Data = RwLock<u8>;

    fn new(field: Array2<RwLock<u8>>, rules: Rule) -> Self {
        let shape = field.shape().to_owned();
        let numx = shape[0];
        let numy = shape[1];
        Self {
            field,
            rules,
            numx,
            numy,
        }
    }

    /// Computes the new generation
    fn compute_next_generation(&mut self) {
        let mut temp = Array2::<RwLock<u8>>::default((self.numx, self.numy));
        Zip::indexed(&self.field)
            .and(&mut temp)
            .par_for_each(|(x, y), elem_field, elem_temp| {
                let count = self.count_living_neighbors(x, y);
                if self.rules.birth[count]
                    || (*elem_field.read().unwrap() == self.rules.state
                        && self.rules.survival[count])
                {
                    *elem_temp.write().unwrap() = self.rules.state;
                } else if *elem_field.read().unwrap() != 0 {
                    *elem_temp.write().unwrap() = *elem_field.read().unwrap() - 1;
                }
            });
        self.field = temp;
    }

    fn cell(&self, x: usize, y: usize) -> Option<u8> {
        if let Some(cell) = self.field.get((x, y)) {
            return Some(cell.read().unwrap().clone());
        }
        None
    }

    fn numx(&self) -> usize {
        self.numx
    }

    fn numy(&self) -> usize {
        self.numy
    }
}

/// Computes the time steps using `ndarray_ndimage`'s `convolve`.
pub struct GameOfLifeConvolution {
    field: Array2<u8>,
    rules: Rule,
    numx: usize,
    numy: usize,
}

impl GameOfLife for GameOfLifeConvolution {
    type Data = u8;

    fn new(field: Array2<u8>, rules: Rule) -> Self {
        let shape = field.shape().to_owned();
        let numx = shape[0];
        let numy = shape[1];
        Self {
            field,
            rules,
            numx,
            numy,
        }
    }

    fn compute_next_generation(&mut self) {
        let kernel = match self.rules.neighbor {
            NeighborRule::Moore => arr2(&[[1, 1, 1], [1, 0, 1], [1, 1, 1]]),
            NeighborRule::VonNeumann => arr2(&[[0, 1, 0], [1, 0, 1], [0, 1, 0]]),
        };

        let temp = convolve(
            &self.field.map(|elem| (*elem == self.rules.state) as usize),
            &kernel,
            ndarray_ndimage::BorderMode::Constant(0),
            0,
        );

        let survive = temp.map(|elem| self.rules.survival[*elem] as u8);
        let birth = temp.map(|elem| self.rules.birth[*elem] as u8);
        self.field = self.field.map(|elem| (*elem == self.rules.state) as u8) * &survive
            + (self.field.map(|elem| *elem) * survive.map(|elem| 1 - elem)).map(|elem| {
                if *elem != 0 {
                    *elem - 1
                } else {
                    0
                }
            });
        self.field += &(self.field.map(|elem| self.rules.state - elem) * birth);
    }

    fn cell(&self, x: usize, y: usize) -> Option<u8> {
        if let Some(cell) = self.field.get((x, y)) {
            return Some(cell.clone());
        }
        None
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
    fn count_living_neighbors_moore() {
        let arr = arr2(&[[1, 1, 1], [1, 1, 1], [1, 1, 1]]).map(|elem| RwLock::new(*elem));
        let rules = Rule::new(
            LifeRule::Raw([false, false, true, true, false, false, false, false, false]),
            LifeRule::Raw([false, false, false, true, false, false, false, false, false]),
            1,
            NeighborRule::Moore,
        );
        let gol = GameOfLifeStd::new(arr, rules);

        let mut temp = Array2::zeros((3, 3));
        for ((x, y), _) in gol.field.indexed_iter() {
            temp[[x, y]] = gol.count_living_neighbors(x, y);
        }

        assert_eq!(temp, arr2(&[[3, 5, 3], [5, 8, 5], [3, 5, 3]]));
    }

    #[test]
    fn count_living_neighbors_von_neumann() {
        let arr = arr2(&[[1, 1, 1], [1, 1, 1], [1, 1, 1]]).map(|elem| RwLock::new(*elem));
        let rules = Rule::new(
            LifeRule::Raw([false, false, true, true, false, false, false, false, false]),
            LifeRule::Raw([false, false, false, true, false, false, false, false, false]),
            1,
            NeighborRule::VonNeumann,
        );
        let gol = GameOfLifeStd::new(arr, rules);

        let mut temp = Array2::zeros((3, 3));
        for ((x, y), _) in gol.field.indexed_iter() {
            temp[[x, y]] = gol.count_living_neighbors(x, y);
        }

        assert_eq!(temp, arr2(&[[2, 3, 2], [3, 4, 3], [2, 3, 2]]));
    }

    #[test]
    fn compute_next_generation_std() {
        let arr = arr2(&[[1, 1, 1], [1, 1, 1], [1, 1, 1]]).map(|elem| RwLock::new(*elem));
        let rules = Rule::new(
            LifeRule::Raw([false, false, true, true, false, false, false, false, false]),
            LifeRule::Raw([false, false, false, true, false, false, false, false, false]),
            1,
            NeighborRule::Moore,
        );
        let mut gol = GameOfLifeStd::new(arr, rules);

        gol.compute_next_generation();
        let temp = gol.field.map(|elem| *elem.read().unwrap());

        assert_eq!(temp, arr2(&[[1, 0, 1], [0, 0, 0], [1, 0, 1]]));
    }

    #[test]
    fn compute_next_generation_conv() {
        let state = 2;

        let arr = arr2(&[
            [state, state, state],
            [state, state, state],
            [state, state, state],
        ]);
        let rules = Rule::new(
            LifeRule::Raw([false, false, true, true, false, false, false, false, false]),
            LifeRule::Raw([false, false, false, true, false, false, false, false, false]),
            state,
            NeighborRule::Moore,
        );
        let mut gol = GameOfLifeConvolution::new(arr, rules);

        gol.compute_next_generation();

        assert_eq!(
            gol.field,
            arr2(&[
                [state, state - 1, state],
                [state - 1, state - 1, state - 1],
                [state, state - 1, state]
            ])
        );
    }

    #[test]
    fn algorithms_moore() {
        let mut rng = rand::thread_rng();

        let numx: usize = 10;
        let numy: usize = 10;
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
            2,
            NeighborRule::VonNeumann,
        );
        let rules_conv = rules_std.clone();

        let mut gol_std = GameOfLifeStd::new(field_std, rules_std);
        let mut gol_conv = GameOfLifeConvolution::new(field_conv, rules_conv);

        assert_eq!(
            gol_std.field.map(|elem| *elem.read().unwrap()),
            gol_conv.field,
            "standard and convolution differ"
        );

        gol_std.compute_next_generation();
        gol_conv.compute_next_generation();

        assert_eq!(
            gol_std.field.map(|elem| *elem.read().unwrap()),
            gol_conv.field,
            "standard and convolution differ after one iteration"
        );
    }

    #[test]
    fn algorithms_von_neumann() {
        let mut rng = rand::thread_rng();

        let numx: usize = 10;
        let numy: usize = 10;
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
            NeighborRule::VonNeumann,
        );
        let rules_conv = rules_std.clone();

        let mut gol_std = GameOfLifeStd::new(field_std, rules_std);
        let mut gol_conv = GameOfLifeConvolution::new(field_conv, rules_conv);

        assert_eq!(
            gol_std.field.map(|elem| *elem.read().unwrap()),
            gol_conv.field,
            "standard and convolution differ"
        );

        gol_std.compute_next_generation();
        gol_conv.compute_next_generation();

        assert_eq!(
            gol_std.field.map(|elem| *elem.read().unwrap()),
            gol_conv.field,
            "standard and convolution differ after one iteration"
        );
    }
}
