use std::io::stdout;

use crossterm::{
    execute,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    ExecutableCommand, Result,
};

fn main() -> Result<()> {
    execute!(
        stdout(),
        SetForegroundColor(Color::Blue),
        SetBackgroundColor(Color::Red),
        Print("Styled text here."),
        ResetColor,
    )?;

    stdout()
        .execute(SetForegroundColor(Color::Yellow))?
        .execute(SetBackgroundColor(Color::DarkRed))?
        .execute(Print("Warning Text!"))?
        .execute(ResetColor)?;

    Ok(())
}
