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

// ── Undo ──────────────────────────────────────────────────────────────────────

struct UndoStack {
    stack: Vec<(Vec<String>, usize, usize)>, // (lines, row, col)
}

impl UndoStack {
    fn new() -> Self { Self { stack: Vec::new() } }

    fn push(&mut self, lines: &Vec<String>, row: usize, col: usize) {
        self.stack.push((lines.clone(), row, col));
        // Cap undo history at 100 states
        if self.stack.len() > 100 {
            self.stack.remove(0);
        }
    }

    fn pop(&mut self) -> Option<(Vec<String>, usize, usize)> {
        self.stack.pop()
    }
}

// ── Syntax highlighting ───────────────────────────────────────────────────────

fn highlight_line(line: &str, ext: &str) -> String {
    match ext {
        "rs" => highlight_rust(line),
        "py" => highlight_python(line),
        "js" | "ts" => highlight_js(line),
        "sh" | "bash" | "zsh" => highlight_shell(line),
        _ => line.to_string(),
    }
}

fn highlight_rust(line: &str) -> String {
    const KEYWORDS: &[&str] = &[
        "fn", "let", "mut", "pub", "use", "mod", "struct", "enum", "impl",
        "trait", "if", "else", "for", "while", "loop", "match", "return",
        "self", "Self", "super", "crate", "true", "false", "const", "static",
        "type", "where", "async", "await", "move", "ref", "in", "break",
        "continue", "as", "dyn", "extern", "unsafe", "Box", "Option",
        "Result", "Some", "None", "Ok", "Err", "Vec", "String",
    ];
    colorize(line, KEYWORDS, "\x1b[34m", "\x1b[32m", "\x1b[33m", "\x1b[90m", "//")
}

fn highlight_python(line: &str) -> String {
    const KEYWORDS: &[&str] = &[
        "def", "class", "if", "elif", "else", "for", "while", "return",
        "import", "from", "as", "with", "try", "except", "finally", "raise",
        "pass", "break", "continue", "lambda", "yield", "global", "nonlocal",
        "True", "False", "None", "and", "or", "not", "in", "is",
    ];
    colorize(line, KEYWORDS, "\x1b[34m", "\x1b[32m", "\x1b[33m", "\x1b[90m", "#")
}

fn highlight_js(line: &str) -> String {
    const KEYWORDS: &[&str] = &[
        "function", "const", "let", "var", "if", "else", "for", "while",
        "return", "class", "new", "this", "import", "export", "default",
        "async", "await", "try", "catch", "finally", "throw", "true",
        "false", "null", "undefined", "typeof", "instanceof", "in", "of",
    ];
    colorize(line, KEYWORDS, "\x1b[34m", "\x1b[32m", "\x1b[33m", "\x1b[90m", "//")
}

fn highlight_shell(line: &str) -> String {
    const KEYWORDS: &[&str] = &[
        "if", "then", "else", "elif", "fi", "for", "in", "do", "done",
        "while", "case", "esac", "function", "return", "local", "export",
        "echo", "source", "alias", "cd", "exit",
    ];
    colorize(line, KEYWORDS, "\x1b[34m", "\x1b[32m", "\x1b[33m", "\x1b[90m", "#")
}

/// Apply colour to keywords, strings, numbers, and comments in a line.
/// keyword_col, string_col, number_col, comment_col are ANSI escape prefixes.
fn colorize(
    line: &str,
    keywords: &[&str],
    keyword_col: &str,
    string_col: &str,
    number_col: &str,
    comment_col: &str,
    comment_prefix: &str,
) -> String {
    // If the whole line is a comment, colour it entirely
    let trimmed = line.trim_start();
    if trimmed.starts_with(comment_prefix) {
        return format!("{}{}\x1b[0m", comment_col, line);
    }

    // Find inline comment position (crude but effective)
    let comment_pos = find_comment_pos(line, comment_prefix);

    let (code_part, comment_part) = if let Some(pos) = comment_pos {
        (&line[..pos], Some(&line[pos..]))
    } else {
        (line, None)
    };

    let mut out = String::new();
    let mut chars = code_part.chars().peekable();
    let mut word = String::new();

    let flush_word = |word: &mut String, out: &mut String| {
        if word.is_empty() { return; }
        // Check if it's a keyword
        if keywords.contains(&word.as_str()) {
            out.push_str(&format!("{}{}\x1b[0m", keyword_col, word));
        }
        // Check if it's a number
        else if word.chars().all(|c| c.is_ascii_digit() || c == '.') && !word.is_empty() {
            out.push_str(&format!("{}{}\x1b[0m", number_col, word));
        }
        else {
            out.push_str(word);
        }
        word.clear();
    };

    let mut in_string = false;
    let mut string_char = '"';

    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if c == '\\' {
                // Escaped char — consume next
                if let Some(nc) = chars.next() { out.push(nc); }
            } else if c == string_char {
                out.push_str("\x1b[0m");
                in_string = false;
            }
            continue;
        }

        if c == '"' || c == '\'' {
            flush_word(&mut word, &mut out);
            in_string = true;
            string_char = c;
            out.push_str(string_col);
            out.push(c);
            continue;
        }

        if c.is_alphanumeric() || c == '_' {
            word.push(c);
        } else {
            flush_word(&mut word, &mut out);
            out.push(c);
        }
    }
    flush_word(&mut word, &mut out);

    if let Some(comment) = comment_part {
        out.push_str(&format!("{}{}\x1b[0m", comment_col, comment));
    }

    out
}

/// Find the position of a comment marker that isn't inside a string.
fn find_comment_pos(line: &str, prefix: &str) -> Option<usize> {
    let mut in_string = false;
    let mut string_char = '"';
    let chars: Vec<char> = line.chars().collect();
    let pchars: Vec<char> = prefix.chars().collect();

    let mut i = 0;
    while i < chars.len() {
        if in_string {
            if chars[i] == '\\' { i += 2; continue; }
            if chars[i] == string_char { in_string = false; }
        } else {
            if chars[i] == '"' || chars[i] == '\'' {
                in_string = true;
                string_char = chars[i];
            } else if chars[i..].starts_with(&pchars) {
                return Some(line.char_indices().nth(i).map(|(b, _)| b).unwrap_or(i));
            }
        }
        i += 1;
    }
    None
}

// ── Render ────────────────────────────────────────────────────────────────────

fn render(stdout: &mut io::Stdout, lines: &Vec<String>, row: usize, col: usize, filename: &str, ext: &str) {
    let (term_cols, term_rows) = terminal::size().unwrap_or((80, 24));
    queue!(stdout, terminal::Clear(ClearType::All), cursor::MoveTo(0, 0)).ok();

    // ── Header ────────────────────────────────────────────
    let header = format!(" mini — {}  (Ctrl+S save  Ctrl+Z undo  Ctrl+Q quit)", filename);
    let padded = format!("{:<width$}", header, width = term_cols as usize);
    let blank  = format!("{:<width$}", "", width = term_cols as usize);
    queue!(stdout, Print(format!("\x1b[7m{}\x1b[0m\n", padded))).ok();
    queue!(stdout, Print(format!("\x1b[7m{}\x1b[0m\n", blank))).ok();

    // ── Line number gutter width ──────────────────────────
    let gutter = lines.len().to_string().len().max(3) + 1; // e.g. "  1 "
    let content_cols = (term_cols as usize).saturating_sub(gutter);

    // ── Content lines ─────────────────────────────────────
    let visible_rows = (term_rows as usize).saturating_sub(3); // 2 header + 1 status
    let start_line = row.saturating_sub(visible_rows / 2);

    for (i, line) in lines.iter().enumerate().skip(start_line).take(visible_rows) {
        let line_num = format!("{:>width$} ", i + 1, width = gutter - 1);
        let highlighted = highlight_line(line, ext);
        // Pad content area (use raw line for padding calculation, not highlighted which has escape codes)
        let padding = if line.len() < content_cols {
            " ".repeat(content_cols - line.len())
        } else {
            String::new()
        };

        if i == row {
            // Current line: dark background across full width
            queue!(stdout, Print(format!(
                "\x1b[90m\x1b[48;5;236m{}\x1b[0m\x1b[48;5;236m{}{}\x1b[0m\n",
                line_num, highlighted, padding
            ))).ok();
        } else {
            queue!(stdout, Print(format!(
                "\x1b[90m{}\x1b[0m{}\n",
                line_num, highlighted
            ))).ok();
        }
    }

    // ── Status bar (2 rows tall) ──────────────────────────
    let status = format!(" Ln {}, Col {} | {} lines ", row + 1, col + 1, lines.len());
    let padded_status = format!("{:<width$}", status, width = term_cols as usize);
    let blank_status  = format!("{:<width$}", "", width = term_cols as usize);
    queue!(stdout,
        cursor::MoveTo(0, term_rows - 2),
        Print(format!("\x1b[7m{}\x1b[0m", blank_status)),
        cursor::MoveTo(0, term_rows - 1),
        Print(format!("\x1b[7m{}\x1b[0m", padded_status))
    ).ok();

    // ── Reposition cursor (offset by gutter) ──────────────
    let screen_row = (row.saturating_sub(start_line) + 2) as u16;
    let screen_col = (col + gutter) as u16;
    queue!(stdout, cursor::MoveTo(screen_col, screen_row)).ok();
    stdout.flush().ok();
}

// ── Save ──────────────────────────────────────────────────────────────────────

fn save(stdout: &mut io::Stdout, filename: &str, lines: &Vec<String>) {
    let content = lines.join("\n");
    match fs::write(filename, &content) {
        Ok(_) => {}
        Err(e) => {
            let row = terminal::size().unwrap_or((80, 24)).1 - 1;
            execute!(stdout, cursor::MoveTo(0, row)).ok();
            print!("\x1b[7m Save failed: {} \x1b[0m", e);
            stdout.flush().ok();
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn builtin_mini(args: &[String]) -> i32 {
    let filename = match args.get(1) {
        Some(f) => f.clone(),
        None => {
            eprintln!("mini: usage: mini <filename>");
            return 1;
        }
    };

    // Detect file extension for syntax highlighting
    let ext = std::path::Path::new(&filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_string();

    // Load existing file content or start empty
    let initial = fs::read_to_string(&filename).unwrap_or_default();
    let mut lines: Vec<String> = if initial.is_empty() {
        vec![String::new()]
    } else {
        initial.lines().map(String::from).collect()
    };

    let mut row: usize = 0;
    let mut col: usize = 0;
    let mut undo = UndoStack::new();

    let mut stdout = io::stdout();
    terminal::enable_raw_mode().ok();
    execute!(stdout, terminal::EnterAlternateScreen).ok();
    stdout.flush().ok();

    render(&mut stdout, &lines, row, col, &filename, &ext);

    loop {
        if let Ok(Event::Key(key)) = event::read() {
            if key.kind != event::KeyEventKind::Press { continue; }

            match (key.modifiers, key.code) {
                // ── Save ──────────────────────────────────────
                (KeyModifiers::CONTROL, KeyCode::Char('s')) => {
                    save(&mut stdout, &filename, &lines);
                }

                // ── Undo ──────────────────────────────────────
                (KeyModifiers::CONTROL, KeyCode::Char('z')) => {
                    if let Some((prev_lines, prev_row, prev_col)) = undo.pop() {
                        lines = prev_lines;
                        row   = prev_row;
                        col   = prev_col;
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

                // ── Editing (all push undo before mutating) ───
                (_, KeyCode::Enter) => {
                    undo.push(&lines, row, col);
                    let rest = lines[row].split_off(col);
                    lines.insert(row + 1, rest);
                    row += 1;
                    col = 0;
                }
                (_, KeyCode::Backspace) => {
                    undo.push(&lines, row, col);
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
                    undo.push(&lines, row, col);
                    if col < lines[row].len() {
                        lines[row].remove(col);
                    } else if row + 1 < lines.len() {
                        let next = lines.remove(row + 1);
                        lines[row].push_str(&next);
                    }
                }

                // ── Typing ────────────────────────────────────
                (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                    undo.push(&lines, row, col);
                    lines[row].insert(col, c);
                    col += 1;
                }

                _ => {}
            }
            render(&mut stdout, &lines, row, col, &filename, &ext);
        }
    }

    execute!(stdout, terminal::LeaveAlternateScreen).ok();
    terminal::disable_raw_mode().ok();
    while event::poll(std::time::Duration::from_millis(0)).unwrap_or(false) {
        let _ = event::read();
    }
    0
}