use rustyline::error::ReadlineError;
use rustyline::{DefaultEditor, Editor};
use rustyline::history::FileHistory;
use rustyline::{Cmd, ConditionalEventHandler, Event, EventContext, EventHandler, KeyEvent, RepeatCount};
use rustyline::config::Configurer;

struct SmartEnterHandler;

impl ConditionalEventHandler for SmartEnterHandler {
    fn handle(&self, _evt: &Event, _n: RepeatCount, _positive: bool, ctx: &EventContext) -> Option<Cmd> {
        if ctx.line().starts_with('/') {
            Some(Cmd::AcceptLine)
        } else {
            Some(Cmd::Newline)
        }
    }
}

pub struct InputEditor {
    editor: Editor<(), FileHistory>,
    history_path: Option<std::path::PathBuf>,
}

impl InputEditor {
    pub fn new() -> Result<Self, String> {
        let mut editor = DefaultEditor::new()
            .map_err(|e| format!("Failed to create editor: {}", e))?;

        // Configure editor
        editor.set_auto_add_history(true);
        let _ = editor.set_max_history_size(1000);

        // Setup history file path
        let history_path = dirs::home_dir()
            .map(|h| h.join(".picocode_history"));

        // Try to load existing history
        if let Some(ref path) = history_path {
            let _ = editor.load_history(path);
        }

        // Configure keybindings
        Self::setup_keybindings(&mut editor);

        Ok(Self { editor, history_path })
    }

    fn setup_keybindings(editor: &mut Editor<(), FileHistory>) {
        // Enter: submit slash commands, newline for everything else
        let _ = editor.bind_sequence(
            KeyEvent::new('\r', rustyline::Modifiers::NONE),
            EventHandler::Conditional(Box::new(SmartEnterHandler))
        );

        // Alt+Enter submits the input
        let _ = editor.bind_sequence(
            KeyEvent::alt('\r'),
            EventHandler::Simple(Cmd::AcceptLine)
        );

        // Alt+J also works as alternative to submit
        let _ = editor.bind_sequence(
            KeyEvent::alt('j'),
            EventHandler::Simple(Cmd::AcceptLine)
        );
    }

    pub fn readline(&mut self, prompt: &str) -> Result<String, ReadlineError> {
        self.editor.readline(prompt)
    }

    pub fn save_history(&mut self) {
        if let Some(ref path) = self.history_path {
            let _ = self.editor.save_history(path);
        }
    }
}
