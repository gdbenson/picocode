use reedline::{
    default_emacs_keybindings, EditCommand, Emacs, FileBackedHistory, KeyCode, KeyModifiers,
    Prompt, PromptEditMode, PromptHistorySearch, PromptHistorySearchStatus, Reedline,
    ReedlineEvent, Signal,
};
use std::borrow::Cow;

#[derive(Debug)]
pub enum ReadlineError {
    Interrupted,
    Eof,
    Other(String),
}

struct SimplePrompt {
    text: String,
}

impl Prompt for SimplePrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.text)
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, _edit_mode: PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed(".. ")
    }

    fn render_prompt_history_search_indicator(
        &self,
        history_search: PromptHistorySearch,
    ) -> Cow<'_, str> {
        let prefix = match history_search.status {
            PromptHistorySearchStatus::Passing => "",
            PromptHistorySearchStatus::Failing => "failing ",
        };
        Cow::Owned(format!("({}reverse-search: {}) ", prefix, history_search.term))
    }
}

pub struct InputEditor {
    editor: Reedline,
}

impl InputEditor {
    pub fn new() -> Result<Self, String> {
        let history_path = dirs::home_dir().map(|h| h.join(".picocode_history"));

        let mut keybindings = default_emacs_keybindings();

        // Enter always submits
        keybindings.add_binding(
            KeyModifiers::NONE,
            KeyCode::Enter,
            ReedlineEvent::Submit,
        );

        // Shift+Enter inserts newline (requires Kitty keyboard protocol)
        keybindings.add_binding(
            KeyModifiers::SHIFT,
            KeyCode::Enter,
            ReedlineEvent::Edit(vec![EditCommand::InsertNewline]),
        );

        let edit_mode = Box::new(Emacs::new(keybindings));

        let mut editor = Reedline::create()
            .with_edit_mode(edit_mode)
            .use_kitty_keyboard_enhancement(true);

        if let Some(ref path) = history_path {
            match FileBackedHistory::with_file(1000, path.clone()) {
                Ok(history) => {
                    editor = editor.with_history(Box::new(history));
                }
                Err(e) => {
                    eprintln!("Warning: could not load history: {}", e);
                }
            }
        }

        Ok(Self { editor })
    }

    pub fn readline(&mut self, prompt: &str) -> Result<String, ReadlineError> {
        let p = SimplePrompt {
            text: prompt.to_string(),
        };

        match self.editor.read_line(&p) {
            Ok(Signal::Success(line)) => Ok(line),
            Ok(Signal::CtrlC) => Err(ReadlineError::Interrupted),
            Ok(Signal::CtrlD) => Err(ReadlineError::Eof),
            Err(e) => Err(ReadlineError::Other(e.to_string())),
        }
    }

    pub fn save_history(&mut self) {
        let _ = self.editor.sync_history();
    }
}
