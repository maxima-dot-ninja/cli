use anyhow::{bail, Result};
use console::{Key, Term};
use std::io::{IsTerminal, Write};

pub fn is_interactive() -> bool {
    Term::stdout().is_term() && std::io::stdin().is_terminal()
}

/// Arrow-key menu: ↑/↓ to move, digits to jump, enter to confirm.
pub fn select(title: &str, options: &[String]) -> Result<usize> {
    let term = Term::stdout();
    if !is_interactive() {
        bail!("The menu needs a terminal. Run `ingest --help` for the commands.");
    }
    println!("{title}\n");
    let mut index = 0;
    draw(options, index, false)?;
    loop {
        match term.read_key()? {
            Key::Enter => break,
            Key::ArrowUp => index = (index + options.len() - 1) % options.len(),
            Key::ArrowDown => index = (index + 1) % options.len(),
            Key::Char(c) => index = digit(c, options.len()).unwrap_or(index),
            Key::CtrlC => std::process::exit(130),
            _ => {}
        }
        draw(options, index, true)?;
    }
    println!();
    Ok(index)
}

/// `1` picks the first option, up to `9`.
fn digit(c: char, len: usize) -> Option<usize> {
    let d = c.to_digit(10)? as usize;
    (1..=len).contains(&d).then(|| d - 1)
}

fn draw(options: &[String], index: usize, redraw: bool) -> Result<()> {
    let mut out = std::io::stdout().lock();
    if redraw {
        write!(out, "\x1B[{}A", options.len())?;
    }
    for (i, option) in options.iter().enumerate() {
        let marker = if i == index { "❯" } else { " " };
        writeln!(out, "\x1B[2K  {marker} {option}")?;
    }
    out.flush()?;
    Ok(())
}

/// Free text as a plain line, so the terminal does the typing and backspace.
pub fn prompt(title: &str) -> Result<String> {
    print!("{title} ");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(line.trim().to_string())
}
