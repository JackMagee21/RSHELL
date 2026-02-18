// src/readline/mod.rs
use nu-ansi-term::Style;
// this is correct as-is, no change needed in the .rs file

use reedline::{
    DefaultHinter, FileBackedHistory,
    Reedline, Signal, Prompt, PromptEditMode, PromptHistorySearch,
    PromptHistorySearchStatus,
};
use std::borrow::Cow;

pub struct MyPrompt {
    pub text: String,
}

impl Prompt for MyPrompt {
    fn render_prompt_left(&self) -> Cow<str> {
        Cow::Borrowed(&self.text)
    }
    fn render_prompt_right(&self) -> Cow<str> {
        Cow::Borrowed("")
    }
    fn render_prompt_indicator(&self, _mode: PromptEditMode) -> Cow<str> {
        Cow::Borrowed("")
    }
    fn render_prompt_multiline_indicator(&self) -> Cow<str> {
        Cow::Borrowed("... ")
    }
    fn render_prompt_history_search_indicator(
        &self,
        history_search: PromptHistorySearch,
    ) -> Cow<str> {
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
                .unwrap_or_else(|_| FileBackedHistory::new(1000).expect("history init failed"))
        );

        let editor = Reedline::create()
            .with_history(history)
            .with_hinter(Box::new(
                DefaultHinter::default()
                    .with_style(nu_ansi_term::Style::new().italic().fg(nu_ansi_term::Color::DarkGray))
            ));

        ShellReadline { editor }
    }

    pub fn readline(&mut self, prompt_text: &str) -> Result<String, ReadlineError> {
        let prompt = MyPrompt { text: prompt_text.to_string() };
        match self.editor.read_line(&prompt) {
            Ok(Signal::Success(line)) => Ok(line),
            Ok(Signal::CtrlC) => Err(ReadlineError::Interrupted),
            Ok(Signal::CtrlD) => Err(ReadlineError::Eof),
            Err(e) => Err(ReadlineError::Other(e.to_string())),
        }
    }
}

#[derive(Debug)]
pub enum ReadlineError {
    Interrupted,
    Eof,
    Other(String),
}