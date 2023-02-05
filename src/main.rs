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
struct CLI {
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
    x: Option<u32>,

    /// y dimension of the field
    #[arg(short)]
    y: Option<u32>,

    /// Algorithm
    #[arg(short, long)]
    algorithm: Option<String>,

    /// Probability of living cells in the initial field
    #[arg(short, long)]
    probability: Option<f64>,
}

#[derive(Subcommand)]
enum Commands {
    /// Prints the Game of Life in a GIF, takes file name of GIF
    GIF { output: String },
    /// Prints the Game of Life in the terminal, press 'q' to exit
    TUI,
}

#[derive(Debug)]
struct Arguments {
    output_file: Option<PathBuf>,
    iterations: usize,
    time_per_iteration: u32,
    numx: u32,
    numy: u32,
    algorithm: Algorithm,
    probability: f64,
    progressbar: Option<ProgressBar>,
}

impl Arguments {
    /// Reads the command line arguments into the `Arguments` struct.
    /// Checks for valid values and sets defaults if no values were provided.
    fn from_cli(cli: &CLI) -> Self {
        // Choose the algorithm from the String
        let algorithm = match cli.algorithm {
            Some(ref alg_str) => match Algorithm::from_str(alg_str) {
                Ok(alg) => alg,
                Err(_) => {
                    println!(
                        "Invalid algorithm.\nPlease choose from {}, or {}.\nAborting...",
                        Algorithm::Std,
                        Algorithm::Conv,
                    );
                    std::process::exit(exitcode::CONFIG);
                }
            },
            None => Algorithm::Conv,
        };

        // Load command line arguments
        let iterations = cli.iterations.or(Some(10)).unwrap();
        let time_per_iteration = cli.timeiter.or(Some(500)).unwrap();
        let probability = cli.probability.or(Some(0.2)).unwrap();
        if probability < 0.0 || probability > 1.0 {
            println!("Probability has to between 0 and 1!\nAborting...");
            std::process::exit(exitcode::CONFIG);
        }
        let numx: u32;
        let numy: u32;

        let output_file: Option<PathBuf>;

        let progressbar: Option<ProgressBar>;

        match cli.command {
            Commands::GIF { ref output } => {
                output_file = Some(handle_path(&output));
                numx = cli.x.or(Some(10)).unwrap();
                numy = cli.y.or(Some(10)).unwrap();
                let pb_def = ProgressBar::new(iterations as u64);
                pb_def.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})").unwrap().progress_chars("#>-"));
                progressbar = Some(pb_def);
            }
            Commands::TUI => {
                output_file = None;
                (numx, numy) = get_size(cli.x, cli.y);
                progressbar = None;
            }
        }
        Arguments {
            output_file,
            iterations,
            time_per_iteration,
            numx,
            numy,
            algorithm,
            probability,
            progressbar,
        }
    }
}

#[derive(Debug)]
/// Available algorithms to calculate the time steps
enum Algorithm {
    Std,
    Conv,
}

impl FromStr for Algorithm {
    type Err = ();

    fn from_str(input: &str) -> Result<Algorithm, Self::Err> {
        match input.to_lowercase().as_str() {
            "std" => Ok(Algorithm::Std),
            "standard" => Ok(Algorithm::Std),
            "conv" => Ok(Algorithm::Conv),
            "convolution" => Ok(Algorithm::Conv),
            _ => Err(()),
        }
    }
}

impl Display for Algorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Algorithm::Std => write!(f, "standard"),
            Algorithm::Conv => write!(f, "convolution"),
        }
    }
}

/// Handles the path to the output file.
///
/// If the file exists, the user is prompted whether to overwrite it. If not, the program terminate.
/// If the file name has a different extension than ".gif", the program terminates with an error message. If the file name has no extension, ".gif" is appended.
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

/// Start the Game of Life
fn start<G: GameOfLife>(
    cli: &CLI,
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
    let cli = CLI::parse();
    let arguments = Arguments::from_cli(&cli);

    // Generate a random initial distribution
    let mut rng = rand::thread_rng();
    let field_vec: Vec<bool> = (0..arguments.numx * arguments.numy)
        .map(|_| rng.gen_bool(arguments.probability))
        .collect();

    // Pass the field to a GameOfLife instance and start it
    match arguments.algorithm {
        Algorithm::Std => {
            let field_vec_std = field_vec.iter().map(|elem| RwLock::new(*elem)).collect();
            let field = Array1::<RwLock<bool>>::from_vec(field_vec_std)
                .into_shape((arguments.numx as usize, arguments.numy as usize))
                .unwrap();
            let gol = GameOfLifeStd::new(field);
            start(
                &cli,
                gol,
                arguments.iterations,
                arguments.time_per_iteration,
                arguments.progressbar,
                arguments.output_file,
            )
        }
        Algorithm::Conv => {
            let field = Array1::<bool>::from_vec(field_vec)
                .into_shape((arguments.numx as usize, arguments.numy as usize))
                .unwrap();
            let gol = GameOfLifeConvolution::new(field);
            start(
                &cli,
                gol,
                arguments.iterations,
                arguments.time_per_iteration,
                arguments.progressbar,
                arguments.output_file,
            )
        }
    }
}
