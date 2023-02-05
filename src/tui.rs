//! # TUI
//! Builds a terminal user interface for the Game of Life field.

use crate::game_of_life::*;
use termion::async_stdin;
use termion::cursor;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::raw::RawTerminal;
use termion::screen::{AlternateScreen, IntoAlternateScreen};

use std::io::{self, Stdout, Write};
use std::thread::sleep;
use std::time::Duration;

const HORZ_BOUNDARY: &str = "─";
const VERT_BOUNDARY: &str = "│";
const TOP_LEFT_CORNER: &str = "┌";
const TOP_RIGHT_CORNER: &str = "┐";
const BOTTOM_LEFT_CORNER: &str = "└";
const BOTTOM_RIGHT_CORNER: &str = "┘";
const CONCEALED: &str = "▒";

pub struct TUI<G: GameOfLife> {
    gol: G,
    screen: AlternateScreen<RawTerminal<Stdout>>,
}

impl<G: GameOfLife> TUI<G> {
    pub fn new(gol: G) -> Self {
        let screen = io::stdout().into_raw_mode().unwrap();
        let screen = screen.into_alternate_screen().unwrap();
        Self { gol, screen }
    }

    pub fn start(&mut self, iterations: usize, time_per_iteration: u32) {
        self.initialize_field();
        let mut stdin = async_stdin().keys();

        for _ in 0..iterations {
            for key in &mut stdin {
                let key = key.unwrap();
                if let Key::Char('q') = key {
                    return;
                }
            }

            self.gol.compute_next_generation();
            self.draw_field();
            sleep(Duration::from_millis(time_per_iteration as u64))
        }
    }

    fn initialize_field(&mut self) {
        let screen = &mut self.screen;
        let width = self.gol.numx();
        let height = self.gol.numy();

        write!(screen, "{}", cursor::Hide).unwrap();

        // Write the upper part of the frame.
        screen.write(TOP_LEFT_CORNER.as_bytes()).unwrap();
        for _ in 0..width {
            screen.write(HORZ_BOUNDARY.as_bytes()).unwrap();
        }
        screen.write(TOP_RIGHT_CORNER.as_bytes()).unwrap();
        screen.write(b"\n\r").unwrap();

        for y in 0..height {
            // The left part of the frame
            screen.write(VERT_BOUNDARY.as_bytes()).unwrap();

            for x in 0..width {
                if self.gol.cell(x, y) {
                    screen.write(CONCEALED.as_bytes()).unwrap();
                } else {
                    screen.write(b" ").unwrap();
                }
            }

            // The right part of the frame.
            screen.write(VERT_BOUNDARY.as_bytes()).unwrap();
            screen.write(b"\n\r").unwrap();
        }

        // Write the lower part of the frame.
        screen.write(BOTTOM_LEFT_CORNER.as_bytes()).unwrap();
        for _ in 0..width {
            screen.write(HORZ_BOUNDARY.as_bytes()).unwrap();
        }
        screen.write(BOTTOM_RIGHT_CORNER.as_bytes()).unwrap();

        screen.flush().unwrap();
    }

    fn draw_field(&mut self) {
        let screen = &mut self.screen;
        let width = u16::try_from(self.gol.numx()).unwrap();
        let height = u16::try_from(self.gol.numy()).unwrap();

        for y in 0..height {
            write!(screen, "{}", cursor::Goto(2, y + 2)).unwrap();
            for x in 0..width {
                if self.gol.cell(x as usize, y as usize) {
                    screen.write(CONCEALED.as_bytes()).unwrap();
                } else {
                    screen.write(b" ").unwrap();
                }
            }
        }

        screen.flush().unwrap();
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
