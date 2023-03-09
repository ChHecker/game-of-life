use core::panic;
use std::fmt::Display;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::RwLock;
use std::time::Duration;

use clap::{Parser, Subcommand};
use gameoflife::gameoflife::*;
use gameoflife::presentation::*;
use indicatif::{ProgressBar, ProgressStyle};
use inquire::validator::Validation;
use inquire::{required, Confirm, CustomType, InquireError, MultiSelect, Select, Text};
use ndarray::{self, Array1};
use rand::{self, Rng};

/// CLI Parser using `clap`
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Whether to plot as a GIF or in the terminal
    #[command(subcommand)]
    command: Option<Commands>,

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

    /// Algorithm (standard or convolution)
    #[arg(short, long)]
    algorithm: Option<String>,

    /// Neighbor algorithm (Moore or VonNeumann)
    #[arg(short, long)]
    neighbor: Option<String>,

    /// Probability of living cells in the initial field
    #[arg(short, long)]
    probability: Option<f32>,

    /// Number of iterations before a cell dies
    #[arg(short, long)]
    state: Option<u8>,
}

/// Subcommands of CLI Parser
#[derive(Subcommand)]
enum Commands {
    /// Prints the Game of Life in a GIF, takes file name of GIF
    Gif { output: String },
    /// Prints the Game of Life in the terminal, press 'q' to exit
    Tui,
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

/// Arguments in the final data types
struct Arguments {
    presentation: Presentations,
    output_file: Option<PathBuf>,
    iterations: usize,
    time_per_iteration: Duration,
    numx: u32,
    numy: u32,
    algorithm: Algorithm,
    rule: Rule,
    probability: f32,
    progressbar: Option<ProgressBar>,
}

impl Arguments {
    /// Reads the command line arguments into the `Arguments` struct.
    /// Checks for valid values and sets defaults if no values were provided.
    fn parse_cli(cli: &Cli) -> Self {
        // Choose the algorithm from the String
        let algorithm = match cli.algorithm {
            Some(ref algorithm_string) => match Algorithm::from_str(algorithm_string) {
                Ok(algorithm) => algorithm,
                Err(_) => {
                    eprintln!(
                        "Invalid algorithm.\nPlease choose from {}, or {}.\nAborting...",
                        Algorithm::Std,
                        Algorithm::Conv,
                    );
                    std::process::exit(exitcode::CONFIG);
                }
            },
            None => Algorithm::Conv,
        };

        let neighbor_algorithm = match cli.neighbor {
            Some(ref neighbor_string) => match NeighborRule::from_str(neighbor_string) {
                Ok(neighbor_algorithm) => neighbor_algorithm,
                Err(_) => {
                    eprintln!(
                        "Invalid algorithm.\nPlease choose from {}, or {}.\nAborting...",
                        NeighborRule::Moore,
                        NeighborRule::VonNeumann
                    );
                    std::process::exit(exitcode::CONFIG);
                }
            },
            None => NeighborRule::Moore,
        };

        let state = cli.state.unwrap_or(1);

        let rule = Rule::new(
            LifeRule::Raw([false, false, true, true, false, false, false, false, false]),
            LifeRule::Raw([false, false, false, true, false, false, false, false, false]),
            state,
            neighbor_algorithm,
        );

        let iterations = cli.iterations.unwrap_or(10);
        let time_per_iteration = Duration::from_millis(cli.timeiter.unwrap_or(500) as u64);
        let probability = cli.probability.unwrap_or(0.2);
        if !(0.0..=1.0).contains(&probability) {
            eprintln!("Probability has to between 0 and 1!\nAborting...");
            std::process::exit(exitcode::CONFIG);
        }

        let presentation: Presentations;

        let numx: u32;
        let numy: u32;

        let output_file: Option<PathBuf>;

        let progressbar: Option<ProgressBar>;

        match cli.command.as_ref().unwrap() {
            Commands::Gif { ref output } => {
                presentation = Presentations::Gif;
                output_file = Some(handle_path(output).expect("path inquire"));
                numx = cli.x.unwrap_or(10);
                numy = cli.y.unwrap_or(10);
                let pb_def = ProgressBar::new(iterations as u64);
                pb_def.set_style(
                    ProgressStyle::with_template(
                        "{spinner:.green} [{elapsed}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})",
                    )
                    .expect("progressbar")
                    .progress_chars("#>-"),
                );
                progressbar = Some(pb_def);
            }
            Commands::Tui => {
                presentation = Presentations::Tui;
                output_file = None;
                (numx, numy) = get_size(cli.x, cli.y);
                progressbar = None;
            }
        }
        Arguments {
            presentation,
            output_file,
            iterations,
            time_per_iteration,
            numx,
            numy,
            algorithm,
            rule,
            probability,
            progressbar,
        }
    }

    /// Start a dialogue to set the arguments for the Game of Life.
    fn from_dialogue() -> Result<Self, InquireError> {
        let presentation = Select::new(
            "How do you want to present the Game of Life?",
            vec![Presentations::Gif, Presentations::Tui],
        )
        .with_vim_mode(true)
        .prompt()?;
        let output_file = match presentation {
            Presentations::Gif => {
                let file_answer = Text::new("Where should the GIF be saved?")
                    .with_validators(&[Box::new(file_validator), Box::new(required!())])
                    .with_formatter(&format_path)
                    .prompt()?;
                Some(handle_path(file_answer).expect("path inquire"))
            }
            Presentations::Tui => None,
        };

        let algorithm = Select::new(
            "Which algorithm do you want to use?",
            vec![Algorithm::Std, Algorithm::Conv],
        )
        .with_vim_mode(true)
        .with_starting_cursor(1)
        .prompt()?;

        let iterations = CustomType::<usize>::new("How many iterations do you want to see?")
            .with_default(10)
            .with_validator(|i: &usize| {
                if *i == 0 {
                    return Ok(Validation::Invalid(
                        "Iteration number has to be greater than 0".into(),
                    ));
                }
                Ok(Validation::Valid)
            })
            .prompt()?;

        let time_answer =
            CustomType::<u64>::new("How much time should every iteration take (in ms)?")
                .with_default(500)
                .prompt()?;
        let time_per_iteration = Duration::from_millis(time_answer);

        let (numx, numy) = match presentation {
            Presentations::Gif => (
                CustomType::<u32>::new("How many columns should the field have?")
                    .with_default(10)
                    .with_validator(|i: &u32| {
                        if *i == 0 {
                            return Ok(Validation::Invalid("Has to be greater than 0".into()));
                        }
                        Ok(Validation::Valid)
                    })
                    .prompt()?,
                CustomType::<u32>::new("How many rows should the field have?")
                    .with_default(10)
                    .with_validator(|i: &u32| {
                        if *i == 0 {
                            return Ok(Validation::Invalid("Has to be greater than 0".into()));
                        }
                        Ok(Validation::Valid)
                    })
                    .prompt()?,
            ),
            Presentations::Tui => {
                let (numx_def, numy_def) = get_size(None, None);
                (
                    CustomType::<u32>::new("How many columns should the field have?")
                        .with_default(numx_def)
                        .with_validator(|i: &u32| {
                            if *i == 0 {
                                return Ok(Validation::Invalid("Has to be greater than 0".into()));
                            }
                            Ok(Validation::Valid)
                        })
                        .prompt()?,
                    CustomType::<u32>::new("How many rows should the field have?")
                        .with_default(numy_def)
                        .with_validator(|i: &u32| {
                            if *i == 0 {
                                return Ok(Validation::Invalid("Has to be greater than 0".into()));
                            }
                            Ok(Validation::Valid)
                        })
                        .prompt()?,
                )
            }
        };

        let probability = CustomType::<f32>::new(
            "With which probability should each cell of the initial field be alive?",
        )
        .with_default(0.2)
        .with_validator(|p: &f32| {
            if (0.0..=1.0).contains(p) {
                return Ok(Validation::Valid);
            }
            Ok(Validation::Invalid(
                "Probability has to be between 0 and 1".into(),
            ))
        })
        .prompt()?;

        let progressbar = match presentation {
            Presentations::Gif => {
                let pb_def = ProgressBar::new(iterations as u64);
                pb_def.set_style(
                    ProgressStyle::with_template(
                        "{spinner:.green} [{elapsed}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})",
                    )
                    .expect("progressbar")
                    .progress_chars("#>-"),
                );
                Some(pb_def)
            }
            Presentations::Tui => None,
        };

        let rule = if !Confirm::new("Do you want to change the default rules?")
            .with_default(false)
            .prompt()?
        {
            Rule::default()
        } else {
            let neighbor = Select::new(
                "Which neighbor rule do you want to use?",
                vec![NeighborRule::Moore, NeighborRule::VonNeumann],
            )
            .prompt()?;

            let survival = MultiSelect::new(
                "With what amount of neighbors should a cell survive?",
                (0..=8).collect::<Vec<usize>>(),
            )
            .prompt()?;
            let survival = LifeRule::Numbers(&survival);

            let birth = MultiSelect::new(
                "With what amount of neighbors should a cell be born?",
                (0..=8).collect::<Vec<usize>>(),
            )
            .prompt()?;
            let birth = LifeRule::Numbers(&birth);

            let state = CustomType::<u8>::new("How many iterations should a cell take to die?")
                .with_validator(|i: &u8| {
                    if *i == 0 {
                        return Ok(Validation::Invalid("Has to be greater than 0".into()));
                    }
                    Ok(Validation::Valid)
                })
                .prompt()?;

            Rule::new(survival, birth, state, neighbor)
        };

        Ok(Arguments {
            presentation,
            output_file,
            iterations,
            time_per_iteration,
            numx,
            numy,
            algorithm,
            rule,
            probability,
            progressbar,
        })
    }
}

/// Handles the path to the output file.
///
/// If the file exists, the user is prompted whether to overwrite it. If not, the program terminate.
/// If the file name has a different extension than ".gif", the program terminates with an error message. If the file name has no extension, ".gif" is appended.
fn handle_path<P: AsRef<Path>>(output_path: P) -> Result<PathBuf, InquireError> {
    let mut output_path = output_path.as_ref().to_path_buf();
    match output_path.extension() {
        Some(extension) => {
            if extension != "gif" {
                eprintln!("The field must be saved as a \".gif\" file.\nAborting...");
                std::process::exit(exitcode::CONFIG);
            };
        }
        None => {
            output_path.set_extension("gif");
        }
    }
    if output_path.exists() {
        let ans = Confirm::new(
            format!(
                "The file {} already exists. Overwrite?",
                output_path.display()
            )
            .as_str(),
        )
        .with_default(false)
        .prompt()?;

        if !ans {
            eprintln!("Aborting...");
            std::process::exit(exitcode::CANTCREAT);
        }
    }

    Ok(output_path)
}

/// `inquire` validator for filename input
fn file_validator(
    text: &str,
) -> Result<Validation, Box<dyn std::error::Error + Send + Sync + 'static>> {
    let output_file = PathBuf::from_str(text);
    let output_file = match output_file {
        Ok(file) => file,
        Err(_) => return Ok(Validation::Invalid("Invalid file name".into())),
    };
    match output_file.extension() {
        Some(extension) => {
            if extension == "gif" {
                return Ok(Validation::Valid);
            }
            Ok(Validation::Invalid(
                "Field must be saved as a \".gif\" file".into(),
            ))
        }
        None => Ok(Validation::Valid),
    }
}

/// Formats the path for `inquire`
fn format_path(text: &str) -> String {
    let mut output_file = PathBuf::from_str(text).unwrap();
    output_file.set_extension("gif");
    output_file.to_str().unwrap().to_owned()
}

/// Start the Game of Life
fn start<G: GameOfLife>(
    presentation: Presentations,
    gol: G,
    iterations: usize,
    time_per_iteration: Duration,
    pb: Option<ProgressBar>,
    output_file: Option<PathBuf>,
) {
    match presentation {
        Presentations::Gif => {
            let file = File::create(output_file.as_ref().unwrap()).unwrap();
            let mut gif = GIF::new(gol);
            gif.start(&file, iterations, time_per_iteration, pb)
                .expect("running GIF presentation");
            println!("Saved Game of Life to {}.", output_file.unwrap().display());
        }
        Presentations::Tui => {
            let mut tui = TUI::new(gol);
            tui.start(iterations, time_per_iteration)
                .expect("running TUI presentation");
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let arguments = match cli.command {
        Some(_) => Arguments::parse_cli(&cli),
        None => match Arguments::from_dialogue() {
            Ok(arguments) => arguments,
            Err(InquireError::OperationInterrupted) => {
                println!("Exiting...");
                std::process::exit(130);
            }
            Err(InquireError::OperationCanceled) => {
                println!("Exiting...");
                std::process::exit(exitcode::OK);
            }
            Err(e) => panic!("{e}"),
        },
    };

    // Generate a random initial distribution
    let mut rng = rand::thread_rng();
    let field_vec: Vec<u8> = (0..arguments.numx * arguments.numy)
        .map(|_| rng.gen_bool(arguments.probability as f64) as u8 * arguments.rule.state)
        .collect();

    // Pass the field to a GameOfLife instance and start it
    match arguments.algorithm {
        Algorithm::Std => {
            let field_vec_std = field_vec.iter().map(|elem| RwLock::new(*elem)).collect();
            let field = Array1::<RwLock<u8>>::from_vec(field_vec_std)
                .into_shape((arguments.numx as usize, arguments.numy as usize))
                .expect("field reshape");
            let gol = GameOfLifeStd::new(field, arguments.rule);
            start(
                arguments.presentation,
                gol,
                arguments.iterations,
                arguments.time_per_iteration,
                arguments.progressbar,
                arguments.output_file,
            );
        }
        Algorithm::Conv => {
            let field = Array1::<u8>::from_vec(field_vec)
                .into_shape((arguments.numx as usize, arguments.numy as usize))
                .expect("field reshape");
            let gol = GameOfLifeConvolution::new(field, arguments.rule);
            start(
                arguments.presentation,
                gol,
                arguments.iterations,
                arguments.time_per_iteration,
                arguments.progressbar,
                arguments.output_file,
            );
        }
    }
}
