#![feature(return_position_impl_trait_in_trait)]
#![allow(incomplete_features)]

use std::{
    fs::File,
    io::{self, stdout, BufRead, BufReader, Seek, SeekFrom, Write},
    iter,
    path::PathBuf,
    result,
};

use clap::Parser;
use crossterm::{
    cursor::{MoveRight, MoveTo},
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    style::Print,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
    QueueableCommand,
};
use tap::{Pipe, Tap};
use thiserror::Error;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Error, Debug)]
enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),
}

type Result<T> = result::Result<T, Error>;

#[derive(Debug, Parser)]
struct Cli {
    input: PathBuf,
}

fn display_centered(
    mut writer: impl Write,
    lines: impl IntoIterator<Item = Result<String>>,
    (term_width, term_height): (u16, u16),
) -> Result<()> {
    writer
        .queue(EnterAlternateScreen)?
        .queue(Clear(ClearType::All))?;

    for (row, line) in lines.into_iter().take(term_height as usize).enumerate() {
        display_centered_line(&mut writer, &line?, row as u16, term_width as usize)?;
    }

    writer.flush()?;

    Ok(())
}

fn display_centered_line(
    mut writer: impl Write,
    line: &str,
    row: u16,
    max_width: usize,
) -> Result<()> {
    writer.queue(MoveTo(0, row))?;
    let segment_buffer = Vec::with_capacity(max_width).tap_mut(|v| v.extend(line.graphemes(true)));

    let width = segment_buffer.len();
    let diff = max_width.max(width) - max_width.min(width);
    let half_diff = diff / 2;

    if width < max_width {
        writer.queue(MoveRight(half_diff as u16))?;
        for segment in segment_buffer {
            writer.queue(Print(segment))?;
        }
    } else {
        for segment in segment_buffer.into_iter().skip(diff / 2).take(max_width) {
            writer.queue(Print(segment))?;
        }
    }

    Ok(())
}

fn line_iter(buf_read: &mut (impl BufRead + ?Sized)) -> impl '_ + Iterator<Item = Result<String>> {
    iter::from_fn(|| {
        let mut buf = String::new();
        match buf_read.read_line(&mut buf) {
            Err(e) => Some(Err(Error::from(e))),
            Ok(0) => None,
            Ok(_) => Some(Ok(buf)),
        }
    })
}

trait BufReadRefLineExt: BufRead {
    fn ref_lines(&mut self) -> impl '_ + Iterator<Item = Result<String>> {
        line_iter(self)
    }
}

impl<T: BufRead> BufReadRefLineExt for T {}

fn main() -> Result<()> {
    let Cli { input } = Cli::parse();

    terminal::enable_raw_mode()?;

    let mut file = File::open(&input)?.pipe(BufReader::new);
    let start_pos = file.stream_position()?;

    let mut display = |scroll_pos, size| -> Result<()> {
        file.seek(SeekFrom::Start(start_pos))?;
        display_centered(stdout(), file.ref_lines().skip(scroll_pos), size)?;
        Ok(())
    };

    let mut scroll_pos = 0usize;
    let mut size = terminal::size()?;

    display(scroll_pos, size)?;
    'event_l: loop {
        match event::read()? {
            Event::Key(key_event) => match key_event {
                KeyEvent {
                    code: KeyCode::Char('c'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                }
                | KeyEvent {
                    code: KeyCode::Char('q'),
                    ..
                } => break 'event_l,
                KeyEvent {
                    kind: KeyEventKind::Press,
                    code,
                    ..
                } => match code {
                    KeyCode::Down => {
                        scroll_pos = scroll_pos.saturating_add(1);
                        display(scroll_pos, size)?
                    }
                    KeyCode::Up => {
                        scroll_pos = scroll_pos.saturating_sub(1);
                        display(scroll_pos, size)?
                    }
                    _ => (),
                },
                _ => (),
            },
            Event::Resize(w, h) => {
                if (w, h) != size {
                    size = (w, h);
                    display(scroll_pos, size)?
                }
            }
            _ => (),
        }
    }

    stdout()
        .queue(Clear(ClearType::All))?
        .queue(LeaveAlternateScreen)?
        .flush()?;

    terminal::disable_raw_mode()?;

    Ok(())
}
