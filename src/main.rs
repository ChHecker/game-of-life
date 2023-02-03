use core::panic;
use std::{
    fmt::Display,
    path::{Path, PathBuf},
    str::FromStr,
    sync::RwLock,
};

use clap::{Parser, Subcommand};
use game_of_life::game_of_life::*;
use game_of_life::tui::*;
use indicatif::{ProgressBar, ProgressStyle};
use inquire::Confirm;
use ndarray::{self, Array1};
use rand::{self, Rng};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Whether to plot as a GIF or in the terminal
    #[command(subcommand)]
    command: Commands,

    /// Number of iterations
    #[arg(short, long)]
    iterations: Option<usize>,

    /// Time per iteration (in ms)
    #[arg(short, long)]
    timeiter: Option<u32>,

    /// x dimension of the field
    #[arg(short)]
    x: Option<u16>,

    /// y dimension of the field
    #[arg(short)]
    y: Option<u16>,

    /// Algorithm
    #[arg(short, long)]
    algorithm: Option<String>,

    /// Probability of living cells in the initial field
    #[arg(short, long)]
    probability: Option<f64>,
}

#[derive(Subcommand)]
enum Commands {
    GIF { output: String },
    TUI,
}

enum Algorithm {
    Std,
    Conv,
    FFTConv,
}

impl FromStr for Algorithm {
    type Err = ();

    fn from_str(input: &str) -> Result<Algorithm, Self::Err> {
        match input.to_lowercase().as_str() {
            "std" => Ok(Algorithm::Std),
            "standard" => Ok(Algorithm::Std),
            "conv" => Ok(Algorithm::Conv),
            "convolution" => Ok(Algorithm::Conv),
            "fft" => Ok(Algorithm::FFTConv),
            "fftconv" => Ok(Algorithm::FFTConv),
            "fft_conv" => Ok(Algorithm::FFTConv),
            "fftconvolution" => Ok(Algorithm::FFTConv),
            "fft_convolution" => Ok(Algorithm::FFTConv),
            _ => Err(()),
        }
    }
}

impl Display for Algorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Algorithm::Std => write!(f, "standard"),
            Algorithm::Conv => write!(f, "convolution"),
            Algorithm::FFTConv => write!(f, "fft_convolution"),
        }
    }
}

fn handle_path(output_file: &str) -> PathBuf {
    let mut output_file = Path::new(&output_file).to_path_buf();
    if output_file.exists() {
        let ans = Confirm::new(
            format!(
                "The file {} already exists. Overwrite?",
                output_file.display()
            )
            .as_str(),
        )
        .with_default(false)
        .prompt();

        match ans {
            Ok(true) => (),
            Ok(false) => {
                println!("Aborting...");
                std::process::exit(exitcode::CANTCREAT);
            }
            Err(e) => panic!("{e}"),
        }
    }
    match output_file.extension() {
        Some(extension) => {
            if extension != "gif" {
                println!("The field must be saved as a \".gif\" file.\nAborting...");
                std::process::exit(exitcode::CONFIG);
            };
        }
        None => {
            output_file.set_extension("gif");
        }
    }
    output_file
}

fn start<G: GameOfLife>(
    cli: &Cli,
    mut gol: G,
    iterations: usize,
    time_per_iteration: u32,
    pb: Option<ProgressBar>,
    output_file: Option<PathBuf>,
) {
    match &cli.command {
        Commands::GIF { output: _ } => gol.start(
            output_file.unwrap().as_path(),
            iterations,
            time_per_iteration,
            pb,
        ),
        Commands::TUI => {
            let mut tui = TUI::new(gol);
            tui.start(iterations, time_per_iteration);
        }
    }
}

fn main() {
    let cli = Cli::parse();

    let algorithm = match cli.algorithm {
        Some(ref alg_str) => match Algorithm::from_str(alg_str) {
            Ok(alg) => alg,
            Err(_) => {
                println!(
                    "Invalid algorithm.\nPlease choose from {}, {}, or {}.",
                    Algorithm::Std,
                    Algorithm::Conv,
                    Algorithm::FFTConv
                );
                return;
            }
        },
        None => Algorithm::Conv,
    };

    let iterations = cli.iterations.or(Some(10)).unwrap();
    let time_per_iteration = cli.timeiter.or(Some(500)).unwrap();
    let probability = cli.probability.or(Some(0.2)).unwrap();
    let numx: u16;
    let numy: u16;

    let output_file: Option<PathBuf>;

    let pb: Option<ProgressBar>;

    match cli.command {
        Commands::GIF { ref output } => {
            output_file = Some(handle_path(&output));
            numx = cli.x.or(Some(10)).unwrap();
            numy = cli.y.or(Some(10)).unwrap();
            let pb_def = ProgressBar::new(iterations as u64);
            pb_def.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})").unwrap().progress_chars("#>-"));
            pb = Some(pb_def);
        }
        Commands::TUI => {
            output_file = None;
            (numx, numy) = get_size(cli.x, cli.y);
            pb = None;
        }
    }

    let mut rng = rand::thread_rng();
    let field_vec: Vec<bool> = (0..numx * numy)
        .map(|_| rng.gen_bool(probability))
        .collect();

    match algorithm {
        Algorithm::Std => {
            let field_vec_std = field_vec.iter().map(|elem| RwLock::new(*elem)).collect();
            let field = Array1::<RwLock<bool>>::from_vec(field_vec_std)
                .into_shape((numx as usize, numy as usize))
                .unwrap();
            let gol = GameOfLifeStd::new(field);
            start(&cli, gol, iterations, time_per_iteration, pb, output_file)
        }
        Algorithm::Conv => {
            let field = Array1::<bool>::from_vec(field_vec)
                .into_shape((numx as usize, numy as usize))
                .unwrap();
            let gol = GameOfLifeConvolution::new(field);
            start(&cli, gol, iterations, time_per_iteration, pb, output_file)
        }
        Algorithm::FFTConv => {
            let field = Array1::<bool>::from_vec(field_vec)
                .into_shape((numx as usize, numy as usize))
                .unwrap();
            let gol = GameOfLifeFFT::new(field);
            start(&cli, gol, iterations, time_per_iteration, pb, output_file)
        }
    }
}

#[cfg(test)]
mod test {
    use game_of_life::{game_of_life::*, tui::TUI};
    use ndarray::Array1;
    use rand::Rng;
    use std::path::Path;

    #[test]
    #[ignore]
    fn compare_gif_tui() {
        let mut rng = rand::thread_rng();
        let field_vec: Vec<bool> = (0..10 * 10).map(|_| rng.gen_bool(0.2)).collect();
        let field = Array1::<bool>::from_vec(field_vec)
            .into_shape((10, 10))
            .unwrap();
        let field2 = field.clone();
        let mut gol = GameOfLifeConvolution::new(field);
        let gol2 = GameOfLifeConvolution::new(field2);

        gol.start(Path::new("test.gif"), 2, 1000, None);
        let mut tui = TUI::new(gol2);
        tui.start(2, 10000);
    }

    // #[test]
    // fn
}
