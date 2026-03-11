// src/executor/builtin/mini.rs
use std::fs;
use std::io::{self, Write};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute, queue,
    style::Print,
    terminal::{self, ClearType},
};

pub fn builtin_mini(args: &[String]) -> i32 {
    let filename = match args.get(1) {
        Some(f) => f.clone(),
        None => {
            eprintln!("mini: usage: mini <filename>");
            return 1;
        }
    };

    // Load existing file content or start empty
    let initial = fs::read_to_string(&filename).unwrap_or_default();
    let mut lines: Vec<String> = if initial.is_empty() {
        vec![String::new()]
    } else {
        initial.lines().map(String::from).collect()
    };

    let mut row: usize = 0; // cursor row (line index)
    let mut col: usize = 0; // cursor col (char index)

    let mut stdout = io::stdout();
    terminal::enable_raw_mode().ok();
    execute!(stdout, terminal::EnterAlternateScreen).ok();
    stdout.flush().ok();

    let render = |stdout: &mut io::Stdout, lines: &Vec<String>, row: usize, col: usize, filename: &str| {
        let (term_cols, term_rows) = terminal::size().unwrap_or((80, 24));
        queue!(stdout, terminal::Clear(ClearType::All), cursor::MoveTo(0, 0)).ok();

        // Header bar
        let header = format!(" mini — {}  (Ctrl+S save  Ctrl+Q quit)", filename);
        let padded = format!("{:<width$}", header, width = term_cols as usize);
        queue!(stdout, Print(format!("\x1b[7m{}\x1b[0m\n", padded))).ok();

        // Content lines (leave 2 rows for header + status)
        let visible_rows = (term_rows as usize).saturating_sub(2);
        let start_line = row.saturating_sub(visible_rows / 2);
        for (i, line) in lines.iter().enumerate().skip(start_line).take(visible_rows) {
            let display = format!("{:<width$}", line, width = term_cols as usize);
            if i == row {
                queue!(stdout, Print(format!("\x1b[48;5;236m{}\x1b[0m\n", display))).ok();
            } else {
                queue!(stdout, Print(format!("{}\n", display))).ok();
            }
        }

        // Status bar
        let status = format!(" Ln {}, Col {} | {} lines ", row + 1, col + 1, lines.len());
        let padded_status = format!("{:<width$}", status, width = term_cols as usize);
        queue!(stdout,
            cursor::MoveTo(0, term_rows - 1),
            Print(format!("\x1b[7m{}\x1b[0m", padded_status))
        ).ok();

        // Position cursor
        let screen_row = (row.saturating_sub(start_line) + 1) as u16;
        let screen_col = col as u16;
        queue!(stdout, cursor::MoveTo(screen_col, screen_row)).ok();
        stdout.flush().ok();
    };

    render(&mut stdout, &lines, row, col, &filename);

    loop {
        if let Ok(Event::Key(key)) = event::read() {
            if key.kind != event::KeyEventKind::Press { continue; }

            match (key.modifiers, key.code) {
                // ── Save ──────────────────────────────────────
                (KeyModifiers::CONTROL, KeyCode::Char('s')) => {
                    let content = lines.join("\n");
                    match fs::write(&filename, &content) {
                        Ok(_) => {}
                        Err(e) => {
                            execute!(stdout, cursor::MoveTo(0, terminal::size().unwrap_or((80,24)).1 - 1)).ok();
                            print!("\x1b[7m Saved failed: {} \x1b[0m", e);
                            stdout.flush().ok();
                            std::thread::sleep(std::time::Duration::from_secs(1));
                        }
                    }
                }

                // ── Quit ──────────────────────────────────────
                (KeyModifiers::CONTROL, KeyCode::Char('q')) => break,

                // ── Navigation ────────────────────────────────
                (_, KeyCode::Up) => {
                    if row > 0 {
                        row -= 1;
                        col = col.min(lines[row].len());
                    }
                }
                (_, KeyCode::Down) => {
                    if row + 1 < lines.len() {
                        row += 1;
                        col = col.min(lines[row].len());
                    }
                }
                (_, KeyCode::Left) => {
                    if col > 0 { col -= 1; }
                    else if row > 0 { row -= 1; col = lines[row].len(); }
                }
                (_, KeyCode::Right) => {
                    if col < lines[row].len() { col += 1; }
                    else if row + 1 < lines.len() { row += 1; col = 0; }
                }
                (_, KeyCode::Home) => col = 0,
                (_, KeyCode::End)  => col = lines[row].len(),

                // ── Editing ───────────────────────────────────
                (_, KeyCode::Enter) => {
                    let rest = lines[row].split_off(col);
                    lines.insert(row + 1, rest);
                    row += 1;
                    col = 0;
                }
                (_, KeyCode::Backspace) => {
                    if col > 0 {
                        col -= 1;
                        lines[row].remove(col);
                    } else if row > 0 {
                        let cur = lines.remove(row);
                        row -= 1;
                        col = lines[row].len();
                        lines[row].push_str(&cur);
                    }
                }
                (_, KeyCode::Delete) => {
                    if col < lines[row].len() {
                        lines[row].remove(col);
                    } else if row + 1 < lines.len() {
                        let next = lines.remove(row + 1);
                        lines[row].push_str(&next);
                    }
                }

                // ── Typing ────────────────────────────────────
                (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                    lines[row].insert(col, c);
                    col += 1;
                }

                _ => {}
            }
            render(&mut stdout, &lines, row, col, &filename);
        }
    }

    execute!(stdout, terminal::LeaveAlternateScreen).ok();
    terminal::disable_raw_mode().ok();
    while event::poll(std::time::Duration::from_millis(0)).unwrap_or(false) {
        let _ = event::read();
    }
    0
} 