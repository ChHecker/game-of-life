//! # Frontends
//! Contains possible ways to present/plot the Game of Life.

use crate::gameoflife::*;
use indicatif::ProgressBar;
use plotters::prelude::*;
use std::fmt::Display;
use std::io::{self, Stdout, Write};
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;
use termion::async_stdin;
use termion::cursor;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::raw::RawTerminal;
use termion::screen::{AlternateScreen, IntoAlternateScreen, ToMainScreen};

pub enum Presentations {
    Gif,
    Tui,
}

impl Display for Presentations {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Presentations::Gif => write!(f, "GIF"),
            Presentations::Tui => write!(f, "TUI"),
        }
    }
}

/// Plot the Game of Life as a GIF using `plotters`
pub struct GIF<G: GameOfLife> {
    gameoflife: G,
}

impl<G: GameOfLife> GIF<G> {
    pub fn new(gameoflife: G) -> Self {
        Self { gameoflife }
    }

    /// Starts the Game of Life
    /// `timer_per_iteration`: ms
    pub fn start(
        &mut self,
        file: &Path,
        iterations: usize,
        time_per_iteration: Duration,
        pb: Option<ProgressBar>,
    ) {
        let area = BitMapBackend::gif(
            file,
            (300, 300),
            time_per_iteration.as_millis().try_into().unwrap(),
        )
        .unwrap()
        .into_drawing_area();
        let subareas = area.split_evenly((self.gameoflife.numx(), self.gameoflife.numy()));

        for _ in 0..iterations {
            for (id, subarea) in subareas.iter().enumerate() {
                let x = id % self.gameoflife.numx();
                let y = id / self.gameoflife.numy();
                let color = if self.gameoflife.cell(x, y).unwrap() > 0 {
                    &WHITE
                } else {
                    &BLACK
                };
                subarea.fill(color).unwrap();
            }
            area.present().unwrap();
            self.gameoflife.compute_next_generation();
            if let Some(ref p) = pb {
                p.inc(1);
            }
        }
        println!("Saved Game of Life to {}.", file.display());
    }
}

const HORZ_BOUNDARY: &str = "─";
const VERT_BOUNDARY: &str = "│";
const TOP_LEFT_CORNER: &str = "┌";
const TOP_RIGHT_CORNER: &str = "┐";
const BOTTOM_LEFT_CORNER: &str = "└";
const BOTTOM_RIGHT_CORNER: &str = "┘";
const CONCEALED: &str = "▒";

/// Plot the Game of Life in the terminal using `termion`
pub struct TUI<G: GameOfLife> {
    gol: G,
    screen: AlternateScreen<RawTerminal<Stdout>>,
}

impl<G: GameOfLife> TUI<G> {
    pub fn new(gol: G) -> Self {
        std::panic::set_hook(Box::new(move |info| {
            write!(
                std::io::stdout().into_raw_mode().unwrap(),
                "{}",
                ToMainScreen
            )
            .unwrap();
            eprint!("{:?}", info);
        }));

        let screen = io::stdout().into_raw_mode().unwrap();
        let screen = screen.into_alternate_screen().unwrap();

        Self { gol, screen }
    }

    /// Starts the Game of Life
    /// `timer_per_iteration`: ms
    pub fn start(&mut self, iterations: usize, time_per_iteration: Duration) {
        self.initialize_field().unwrap();
        let mut stdin = async_stdin().keys();
        let polling_time = 200;
        let sleep_how_often = time_per_iteration.as_millis() / polling_time;
        let remaining_sleep = time_per_iteration.as_millis() - sleep_how_often * polling_time;

        for _ in 0..iterations {
            self.gol.compute_next_generation();
            self.draw_field().unwrap();
            for _ in 0..sleep_how_often {
                for key in &mut stdin {
                    let key = key.unwrap();
                    if let Key::Char('q') = key {
                        return;
                    }
                }
                sleep(Duration::from_millis(polling_time as u64))
            }
            sleep(Duration::from_millis(remaining_sleep as u64))
        }
    }

    /// Initializes the TUI
    fn initialize_field(&mut self) -> std::io::Result<()> {
        let screen = &mut self.screen;
        let width = self.gol.numx();
        let height = self.gol.numy();

        write!(screen, "{}", cursor::Hide)?;

        // Write the upper part of the frame.
        screen.write_all(TOP_LEFT_CORNER.as_bytes())?;
        for _ in 0..width {
            screen.write_all(HORZ_BOUNDARY.as_bytes())?;
        }
        screen.write_all(TOP_RIGHT_CORNER.as_bytes())?;
        screen.write_all(b"\n\r")?;

        for y in 0..height {
            // The left part of the frame
            screen.write_all(VERT_BOUNDARY.as_bytes())?;

            for x in 0..width {
                if self.gol.cell(x, y).unwrap() > 0 {
                    screen.write_all(CONCEALED.as_bytes())?;
                } else {
                    screen.write_all(b" ")?;
                }
            }

            // The right part of the frame.
            screen.write_all(VERT_BOUNDARY.as_bytes())?;
            screen.write_all(b"\n\r")?;
        }

        // Write the lower part of the frame.
        screen.write_all(BOTTOM_LEFT_CORNER.as_bytes())?;
        for _ in 0..width {
            screen.write_all(HORZ_BOUNDARY.as_bytes())?;
        }
        screen.write_all(BOTTOM_RIGHT_CORNER.as_bytes())?;

        screen.flush()?;

        Ok(())
    }

    fn draw_field(&mut self) -> std::io::Result<()> {
        let screen = &mut self.screen;
        let width = u16::try_from(self.gol.numx()).unwrap();
        let height = u16::try_from(self.gol.numy()).unwrap();

        for y in 0..height {
            write!(screen, "{}", cursor::Goto(2, y + 2))?;
            for x in 0..width {
                if self.gol.cell(x as usize, y as usize).unwrap() > 0 {
                    screen.write_all(CONCEALED.as_bytes())?;
                } else {
                    screen.write_all(b" ")?;
                }
            }
        }
        screen.flush()?;

        Ok(())
    }
}

impl<G: GameOfLife> Drop for TUI<G> {
    fn drop(&mut self) {
        write!(self.screen, "{}", cursor::Show).unwrap();
    }
}

/// Returns the user preference for the field size if specified, else the terminal size.
pub fn get_size(numx: Option<u32>, numy: Option<u32>) -> (u32, u32) {
    let termsize = termion::terminal_size().ok();
    let termwidth = termsize.map(|(w, _)| w - 2);
    let termheight = termsize.map(|(_, h)| h - 2);
    (
        numx.or(termwidth.map(|elem| elem as u32)).unwrap_or(10),
        numy.or(termheight.map(|elem| elem as u32)).unwrap_or(10),
    )
}
