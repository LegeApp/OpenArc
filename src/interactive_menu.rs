//! Arrow-key driven interactive prompts, in the style of Claude Code's settings UI.
//!
//! Two widgets are provided:
//!  - [`select_option`]: a list of named choices, navigated with Up/Down.
//!  - [`select_value`]: a numeric value adjusted with Left/Right (and
//!    PageUp/PageDown for larger steps).
//!
//! Both fall back to plain numbered/typed prompts when stdin/stdout are not
//! connected to a real terminal (e.g. piped input, CI).

use anyhow::{anyhow, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::style::{Color, Print, ResetColor, SetForegroundColor};
use crossterm::{cursor, queue, terminal, terminal::ClearType};
use std::io::{self, IsTerminal, Write};

/// A single choice presented by [`select_option`].
pub struct SelectOption {
    pub label: String,
    pub hint: String,
}

impl SelectOption {
    pub fn new(label: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            hint: hint.into(),
        }
    }
}

/// True when both stdin and stdout are connected to a real terminal, i.e.
/// arrow-key navigation is possible.
pub fn is_interactive_tty() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

const CANCELLED: &str = "Cancelled";

/// Present a list of options and let the user pick one with Up/Down + Enter.
/// Returns the index of the chosen option.
pub fn select_option(title: &str, options: &[SelectOption], default_idx: usize) -> Result<usize> {
    if options.is_empty() {
        return Ok(0);
    }
    if !is_interactive_tty() {
        return select_option_fallback(title, options, default_idx);
    }

    let mut current = default_idx.min(options.len() - 1);
    let total_lines = options.len() as u16 + 2;

    let mut stdout = io::stdout();
    terminal::enable_raw_mode()?;
    queue!(stdout, cursor::Hide)?;
    render_select(&mut stdout, title, options, current)?;

    let result = loop {
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                let mut changed = true;
                match key.code {
                    KeyCode::Up => {
                        current = if current == 0 {
                            options.len() - 1
                        } else {
                            current - 1
                        }
                    }
                    KeyCode::Down => current = (current + 1) % options.len(),
                    KeyCode::Enter => break Ok(current),
                    KeyCode::Esc => break Err(anyhow!(CANCELLED)),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        break Err(anyhow!(CANCELLED))
                    }
                    _ => changed = false,
                }
                if changed {
                    queue!(
                        stdout,
                        cursor::MoveUp(total_lines),
                        terminal::Clear(ClearType::FromCursorDown)
                    )?;
                    render_select(&mut stdout, title, options, current)?;
                }
            }
            _ => {}
        }
    };

    queue!(stdout, cursor::Show)?;
    stdout.flush()?;
    terminal::disable_raw_mode()?;
    result
}

fn render_select(
    stdout: &mut io::Stdout,
    title: &str,
    options: &[SelectOption],
    current: usize,
) -> Result<()> {
    queue!(stdout, Print(format!("{title}\r\n")))?;
    for (i, opt) in options.iter().enumerate() {
        if i == current {
            queue!(
                stdout,
                SetForegroundColor(Color::Cyan),
                Print(format!("  > {}", opt.label)),
                ResetColor
            )?;
        } else {
            queue!(stdout, Print(format!("    {}", opt.label)))?;
        }
        if !opt.hint.is_empty() {
            queue!(
                stdout,
                SetForegroundColor(Color::DarkGrey),
                Print(format!("  ({})", opt.hint)),
                ResetColor
            )?;
        }
        queue!(stdout, Print("\r\n"))?;
    }
    queue!(
        stdout,
        SetForegroundColor(Color::DarkGrey),
        Print("  \u{2191}/\u{2193} select \u{00b7} Enter confirm\r\n"),
        ResetColor
    )?;
    stdout.flush()?;
    Ok(())
}

fn select_option_fallback(
    title: &str,
    options: &[SelectOption],
    default_idx: usize,
) -> Result<usize> {
    println!("{title}");
    for (i, opt) in options.iter().enumerate() {
        if opt.hint.is_empty() {
            println!("  [{}] {}", i + 1, opt.label);
        } else {
            println!("  [{}] {} ({})", i + 1, opt.label, opt.hint);
        }
    }
    print!("  Choice [{}]: ", default_idx + 1);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(default_idx);
    }
    let choice: usize = trimmed.parse().unwrap_or(default_idx + 1);
    Ok(choice.saturating_sub(1).min(options.len() - 1))
}

/// Present a numeric value that can be adjusted with Left/Right (by `step`)
/// and PageUp/PageDown or Up/Down (by `big_step`), confirmed with Enter.
/// `describe` renders a human-readable hint for the current value.
pub fn select_value(
    title: &str,
    min: i32,
    max: i32,
    default: i32,
    step: i32,
    big_step: i32,
    describe: impl Fn(i32) -> String,
) -> Result<i32> {
    if !is_interactive_tty() {
        return select_value_fallback(title, min, max, default, describe);
    }

    let mut current = default.clamp(min, max);
    let total_lines: u16 = 2;

    let mut stdout = io::stdout();
    terminal::enable_raw_mode()?;
    queue!(stdout, cursor::Hide)?;
    render_value(&mut stdout, title, min, max, current, &describe)?;

    let result = loop {
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                let mut changed = true;
                match key.code {
                    KeyCode::Left => current = (current - step).clamp(min, max),
                    KeyCode::Right => current = (current + step).clamp(min, max),
                    KeyCode::Down | KeyCode::PageDown => {
                        current = (current - big_step).clamp(min, max)
                    }
                    KeyCode::Up | KeyCode::PageUp => current = (current + big_step).clamp(min, max),
                    KeyCode::Enter => break Ok(current),
                    KeyCode::Esc => break Err(anyhow!(CANCELLED)),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        break Err(anyhow!(CANCELLED))
                    }
                    _ => changed = false,
                }
                if changed {
                    queue!(
                        stdout,
                        cursor::MoveUp(total_lines),
                        terminal::Clear(ClearType::FromCursorDown)
                    )?;
                    render_value(&mut stdout, title, min, max, current, &describe)?;
                }
            }
            _ => {}
        }
    };

    queue!(stdout, cursor::Show)?;
    stdout.flush()?;
    terminal::disable_raw_mode()?;
    result
}

fn render_value(
    stdout: &mut io::Stdout,
    title: &str,
    min: i32,
    max: i32,
    current: i32,
    describe: &impl Fn(i32) -> String,
) -> Result<()> {
    queue!(stdout, Print(format!("{title}\r\n")))?;
    queue!(
        stdout,
        Print("  "),
        SetForegroundColor(Color::DarkGrey),
        Print("< "),
        ResetColor,
        SetForegroundColor(Color::Cyan),
        Print(format!("{current}")),
        ResetColor,
        SetForegroundColor(Color::DarkGrey),
        Print(" >  "),
        ResetColor,
        Print(describe(current)),
        SetForegroundColor(Color::DarkGrey),
        Print(format!(
            "  [{min}-{max}] \u{2190}/\u{2192} adjust \u{00b7} Enter confirm"
        )),
        ResetColor,
        Print("\r\n"),
    )?;
    stdout.flush()?;
    Ok(())
}

fn select_value_fallback(
    title: &str,
    min: i32,
    max: i32,
    default: i32,
    describe: impl Fn(i32) -> String,
) -> Result<i32> {
    println!("{title} ({min}-{max})");
    println!("  Default: {default} ({})", describe(default));
    print!("  Value [{default}]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(default);
    }
    let value: i32 = trimmed.parse().unwrap_or(default);
    Ok(value.clamp(min, max))
}
