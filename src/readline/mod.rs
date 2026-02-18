// src/readline/mod.rs
// Line editor with Ctrl+C, Ctrl+L, and tab completion

use reedline::{
    DefaultHinter, FileBackedHistory, Reedline, ReedlineEvent, Signal,
    Prompt, PromptEditMode, PromptHistorySearch, PromptHistorySearchStatus,
    Completer, Suggestion, Span, KeyCode, KeyModifiers, EditCommand,
    ReedlineMenu, ColumnarMenu, MenuBuilder,
};
use std::borrow::Cow;
use crate::completion;

// ── Prompt ───────────────────────────────────────────────────────────────────

pub struct MyPrompt {
    pub text: String,
}

impl Prompt for MyPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.text)
    }
    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }
    fn render_prompt_indicator(&self, _mode: PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed("")
    }
    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed("... ")
    }
    fn render_prompt_history_search_indicator(
        &self,
        history_search: PromptHistorySearch,
    ) -> Cow<'_, str> {
        let indicator = match history_search.status {
            PromptHistorySearchStatus::Passing => "",
            PromptHistorySearchStatus::Failing => "failing ",
        };
        Cow::Owned(format!(
            "({}reverse-search: {}) ",
            indicator, history_search.term
        ))
    }
}

// ── Tab Completer ─────────────────────────────────────────────────────────────

pub struct ShellCompleter;

impl Completer for ShellCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        // Work out the word we're completing
        let before_cursor = &line[..pos];
        let word_start = before_cursor
            .rfind(|c: char| c == ' ' || c == '|' || c == ';' || c == '&')
            .map(|i| i + 1)
            .unwrap_or(0);

        let partial = &before_cursor[word_start..];
        let is_first_word = !before_cursor[..word_start]
            .trim()
            .contains(|c: char| !matches!(c, '|' | ';' | '&'));

        // Get completions from our engine
        let mut suggestions: Vec<Suggestion> = completion::complete(partial, is_first_word)
            .into_iter()
            .map(|s| Suggestion {
                value: s,
                description: None,
                style: None,
                extra: None,
                span: Span::new(word_start, pos),
                append_whitespace: false,
            })
            .collect();

        // Also complete builtin names if first word
        if is_first_word {
            for builtin in completion::builtin_names() {
                if builtin.starts_with(partial) {
                    suggestions.push(Suggestion {
                        value: builtin.to_string(),
                        description: Some("builtin".to_string()),
                        style: None,
                        extra: None,
                        span: Span::new(word_start, pos),
                        append_whitespace: true,
                    });
                }
            }
        }

        suggestions
    }
}

// ── Main readline struct ──────────────────────────────────────────────────────

pub struct ShellReadline {
    editor: Reedline,
}

impl ShellReadline {
    pub fn new() -> Self {
        let history_path = dirs::home_dir()
            .unwrap_or_default()
            .join(".myshell_history");

        let history = Box::new(
            FileBackedHistory::with_file(1000, history_path)
                .unwrap_or_else(|_| {
                    FileBackedHistory::new(1000).expect("history init failed")
                }),
        );

        // Tab completion menu (shows list of options when multiple matches)
        let completion_menu = Box::new(
            ColumnarMenu::default().with_name("completion_menu")
        );

        // Custom keybindings
        let mut keybindings = reedline::default_emacs_keybindings();

        // Ctrl+L → clear screen
        keybindings.add_binding(
            KeyModifiers::CONTROL,
            KeyCode::Char('l'),
            ReedlineEvent::ExecuteHostCommand("__clear__".to_string()),
        );

        // Tab → open completion menu
        keybindings.add_binding(
            KeyModifiers::NONE,
            KeyCode::Tab,
            ReedlineEvent::UntilFound(vec![
                ReedlineEvent::Menu("completion_menu".to_string()),
                ReedlineEvent::MenuNext,
            ]),
        );

        let editor = Reedline::create()
            .with_history(history)
            .with_completer(Box::new(ShellCompleter))
            .with_menu(ReedlineMenu::EngineCompleter(completion_menu))
            .with_keybindings(keybindings)
            .with_hinter(Box::new(
                DefaultHinter::default().with_style(
                    nu_ansi_term::Style::new()
                        .italic()
                        .fg(nu_ansi_term::Color::DarkGray),
                ),
            ));

        ShellReadline { editor }
    }

    pub fn readline(&mut self, prompt_text: &str) -> Result<String, ReadlineError> {
        let prompt = MyPrompt {
            text: prompt_text.to_string(),
        };
        match self.editor.read_line(&prompt) {
            Ok(Signal::Success(line)) => {
                // Internal command sent by Ctrl+L keybind
                if line.trim() == "__clear__" {
                    clear_screen();
                    return Err(ReadlineError::Interrupted); // re-show prompt
                }
                Ok(line)
            }
            Ok(Signal::CtrlC) => Err(ReadlineError::Interrupted),
            Ok(Signal::CtrlD) => Err(ReadlineError::Eof),
            Err(e) => Err(ReadlineError::Other(e.to_string())),
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Cross-platform clear screen
pub fn clear_screen() {
    print!("\x1B[2J\x1B[H");
    use std::io::Write;
    let _ = std::io::stdout().flush();
}

#[derive(Debug)]
pub enum ReadlineError {
    Interrupted,
    Eof,
    Other(String),
}