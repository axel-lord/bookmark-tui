use std::{
    fs::File,
    io::{self, stdout, BufRead, BufReader, Seek, SeekFrom, Write},
    iter::{self, FusedIterator},
    path::PathBuf,
    result,
};

use clap::Parser;
use crossterm::{
    cursor::{Hide, MoveRight, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    style::Print,
    terminal::{
        self, Clear, ClearType, DisableLineWrap, EnableLineWrap, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
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
    writer.queue(Clear(ClearType::All))?;

    for (row, line) in lines
        .into_iter()
        .chain(iter::once_with(String::new).map(Ok))
        .take(term_height as usize)
        .enumerate()
    {
        queue_centered_line(&mut writer, &line?, row as u16, term_width as usize)?;
    }

    Ok(())
}

fn queue_centered_line(
    mut writer: impl Write,
    line: &str,
    row: u16,
    max_width: usize,
) -> Result<()> {
    writer
        .queue(MoveTo(0, row))?
        .queue(Clear(ClearType::CurrentLine))?;

    // Setting capacity to string length should guarantee only 1 allocation since there should not
    // be able to be more grapheme clusters than bytes.
    let segment_buffer = Vec::with_capacity(line.len())
        .tap_mut(|v| v.extend(line.grapheme_indices(true).map(|(i, _)| i)));

    let width = segment_buffer.len();
    let diff = max_width.max(width) - max_width.min(width);

    // Text gets either padded or cut depending on length.
    if width < max_width {
        writer
            .queue(MoveRight(diff as u16 / 2))?
            .queue(Print(line))?;
    } else {
        // segment_buffer
        //     .into_iter()
        //     .skip(diff / 2)
        //     .take(max_width)
        //     .try_fold(&mut writer, |writer, segment| writer.queue(Print(segment)))?;
        writer.queue(Print(
            &line[segment_buffer[diff / 2]..segment_buffer[max_width - diff / 2]],
        ))?;
    }

    writer.flush()?;

    Ok(())
}

enum RefLineIter<'a, R: ?Sized> {
    Dead,
    Alive(&'a mut R),
}

impl<'a, R> Iterator for RefLineIter<'a, R>
where
    R: BufRead + ?Sized,
{
    type Item = Result<String>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Self::Alive(reader) = self {
            let mut buf = String::new();
            match reader.read_line(&mut buf) {
                Err(e) => Some(Err(Error::from(e))),
                Ok(0) => {
                    *self = Self::Dead;
                    None
                }
                Ok(_) => Some(Ok(buf)),
            }
        } else {
            None
        }
    }
}

impl<'a, R> FusedIterator for RefLineIter<'a, R> where R: BufRead + ?Sized {}

trait BufReadRefLineExt: BufRead {
    fn ref_lines(&mut self) -> RefLineIter<'_, Self> {
        RefLineIter::Alive(self)
    }
}

impl<T: BufRead> BufReadRefLineExt for T {}

fn main() -> Result<()> {
    let Cli { input } = Cli::parse();

    terminal::enable_raw_mode()?;

    let mut writer = stdout();

    writer
        .queue(EnterAlternateScreen)?
        .queue(Clear(ClearType::All))?
        .queue(Hide)?
        .queue(DisableLineWrap)?
        .flush()?;

    let mut file = File::open(&input)?.pipe(BufReader::new);
    let start_pos = file.stream_position()?;

    let mut display = |scroll_pos, size| -> Result<()> {
        file.seek(SeekFrom::Start(start_pos))?;
        display_centered(&mut writer, file.ref_lines().skip(scroll_pos), size)?;
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
        .queue(Show)?
        .queue(EnableLineWrap)?
        .flush()?;

    terminal::disable_raw_mode()?;

    Ok(())
}
