use console::{style, StyledObject, Term};
use indicatif::{ProgressBar, ProgressStyle};
use serde_json::Value;
use std::sync::Mutex;
use std::time::Duration;
use termimad;

use crate::input::InputEditor;
use rustyline::error::ReadlineError;

#[derive(Debug, PartialEq)]
pub enum Confirmation {
    Yes,
    No,
    Always,
}

pub trait Output: Send + Sync {
    fn display_text(&self, text: &str);
    fn display_tool_call(&self, name: &str, args: &Value);
    fn display_tool_result(&self, result: &str);
    fn get_user_input(&self, prompt: &str) -> String;
    fn display_error(&self, error: &str);
    fn display_system(&self, text: &str);
    fn confirm(&self, message: &str) -> Confirmation;
    fn display_separator(&self);
    fn display_thinking(&self, message: &str);
    fn stop_thinking(&self);
    fn display_header(
        &self,
        provider: &str,
        model: &str,
        yolo: bool,
        limit: usize,
        persona: Option<&str>,
    );
}

pub struct QuietOutput {
    spinner: Mutex<Option<ProgressBar>>,
}

impl QuietOutput {
    pub fn new() -> Self {
        Self {
            spinner: Mutex::new(None),
        }
    }

    fn create_spinner(message: &str) -> ProgressBar {
        let pb = ProgressBar::new_spinner();
        pb.enable_steady_tick(Duration::from_millis(120));
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.blue} {msg}")
                .unwrap(),
        );
        pb.set_message(message.to_string());
        pb
    }
}

impl Output for QuietOutput {
    fn display_text(&self, _text: &str) {}
    fn display_tool_call(&self, _name: &str, _args: &Value) {}
    fn display_tool_result(&self, _result: &str) {}
    fn get_user_input(&self, _prompt: &str) -> String {
        String::new()
    }
    fn display_error(&self, error: &str) {
        self.stop_thinking();
        eprintln!("Error: {}", error);
    }
    fn display_system(&self, _text: &str) {}
    fn confirm(&self, message: &str) -> Confirmation {
        self.stop_thinking();
        eprintln!("Confirm: {} [y/n/s]", message);
        let mut input = String::new();
        let _ = std::io::stdin().read_line(&mut input);
        match input.trim().to_lowercase().as_str() {
            "y" | "yes" => Confirmation::Yes,
            "s" | "session" => Confirmation::Always,
            _ => Confirmation::No,
        }
    }
    fn display_separator(&self) {}
    fn display_thinking(&self, message: &str) {
        let mut spinner_lock = self.spinner.lock().unwrap();
        if spinner_lock.is_none() {
            *spinner_lock = Some(Self::create_spinner(message));
        }
    }
    fn stop_thinking(&self) {
        if let Some(pb) = self.spinner.lock().unwrap().take() {
            pb.finish_and_clear();
        }
    }
    fn display_header(
        &self,
        _provider: &str,
        _model: &str,
        _yolo: bool,
        _limit: usize,
        _persona: Option<&str>,
    ) {
    }
}

pub struct NoOutput;

impl Output for NoOutput {
    fn display_text(&self, _text: &str) {}
    fn display_tool_call(&self, _name: &str, _args: &Value) {}
    fn display_tool_result(&self, _result: &str) {}
    fn get_user_input(&self, _prompt: &str) -> String {
        String::new()
    }
    fn display_error(&self, _error: &str) {}
    fn display_system(&self, _text: &str) {}
    fn confirm(&self, _message: &str) -> Confirmation {
        Confirmation::Yes
    }
    fn display_separator(&self) {}
    fn display_thinking(&self, _message: &str) {}
    fn stop_thinking(&self) {}
    fn display_header(
        &self,
        _provider: &str,
        _model: &str,
        _yolo: bool,
        _limit: usize,
        _persona: Option<&str>,
    ) {
    }
}

pub struct LogOutput;

impl Output for LogOutput {
    fn display_text(&self, text: &str) {
        tracing::info!(target: "picocode", "{}", text);
    }

    fn display_tool_call(&self, name: &str, args: &Value) {
        tracing::info!(target: "picocode", "Tool call: {} with args: {:?}", name, args);
    }

    fn display_tool_result(&self, result: &str) {
        tracing::info!(target: "picocode", "Tool result: {}", result);
    }

    fn get_user_input(&self, _prompt: &str) -> String {
        String::new()
    }

    fn display_error(&self, error: &str) {
        tracing::error!(target: "picocode", "{}", error);
    }

    fn display_system(&self, text: &str) {
        tracing::debug!(target: "picocode", "System: {}", text);
    }

    fn confirm(&self, _message: &str) -> Confirmation {
        Confirmation::No
    }

    fn display_separator(&self) {}

    fn display_thinking(&self, _message: &str) {}

    fn stop_thinking(&self) {}

    fn display_header(
        &self,
        provider: &str,
        model: &str,
        yolo: bool,
        limit: usize,
        persona: Option<&str>,
    ) {
        tracing::info!(target: "picocode", "picocode | {} | {} | persona:{} | yolo:{} limit:{}", provider, model, persona.unwrap_or("default"), yolo, limit);
    }
}

pub struct ConsoleOutput {
    spinner: Mutex<Option<ProgressBar>>,
    editor: Mutex<Option<InputEditor>>,
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s.to_string()
    }
}

fn get_preview(args: &Value) -> String {
    let s = if let Some(obj) = args.as_object() {
        obj.values()
            .next()
            .map(|v| match v {
                Value::String(s) => s.clone(),
                _ => v.to_string(),
            })
            .unwrap_or_default()
    } else {
        args.to_string()
    };
    truncate(&s.replace('\n', " "), 50)
}

impl ConsoleOutput {
    pub fn new() -> Self {
        Self {
            spinner: Mutex::new(None),
            editor: Mutex::new(None),
        }
    }

    fn init_editor_if_needed(&self) -> bool {
        let mut editor_lock = self.editor.lock().unwrap();
        if editor_lock.is_none() {
            match InputEditor::new() {
                Ok(ed) => {
                    *editor_lock = Some(ed);
                    true
                }
                Err(_) => false,
            }
        } else {
            true
        }
    }

    fn fallback_input() -> String {
        use std::io::{self, Write};
        let _ = io::stdout().flush();
        let mut input = String::new();
        let _ = io::stdin().read_line(&mut input);
        input.trim().to_string()
    }
}

impl Default for ConsoleOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsoleOutput {
    fn separator() {
        let width = Term::stdout().size().1 as usize;
        println!("{}", style("─".repeat(width)).dim());
    }

    fn create_spinner(message: &str) -> ProgressBar {
        let pb = ProgressBar::new_spinner();
        pb.enable_steady_tick(Duration::from_millis(120));
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.blue} {msg}")
                .unwrap(),
        );
        pb.set_message(message.to_string());
        pb
    }
}

impl Output for ConsoleOutput {
    fn display_text(&self, text: &str) {
        self.stop_thinking();
        println!();
        print!("{} ", style("⏺").cyan());
        termimad::print_inline(text);
        println!();
    }

    fn display_tool_call(&self, name: &str, args: &Value) {
        self.stop_thinking();
        let preview = get_preview(args);
        let capitalized_name = name
            .chars()
            .next()
            .map(|c| c.to_uppercase().collect::<String>() + &name[1..])
            .unwrap_or_else(|| name.to_string());
        println!(
            "\n{} {}({})",
            style("⏺").green(),
            style(capitalized_name).bold(),
            style(preview).dim()
        );
    }

    fn display_tool_result(&self, result: &str) {
        self.stop_thinking();

        let unquoted = serde_json::from_str::<Value>(result)
            .ok()
            .and_then(|v| match v {
                Value::String(s) => Some(s),
                Value::Array(arr) => Some(
                    arr.iter()
                        .map(|v| v.as_str().unwrap_or(&v.to_string()).to_string())
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                _ => None,
            })
            .unwrap_or_else(|| result.to_string());

        let mut cleaned = unquoted.as_str();
        while cleaned.starts_with("Toolset error: ") || cleaned.starts_with("ToolCallError: ") {
            if let Some(stripped) = cleaned.strip_prefix("Toolset error: ") {
                cleaned = stripped;
            } else if let Some(stripped) = cleaned.strip_prefix("ToolCallError: ") {
                cleaned = stripped;
            }
        }

        let is_error =
            unquoted.starts_with("Toolset error") || unquoted.starts_with("ToolCallError");
        let lines: Vec<_> = cleaned.lines().collect();

        if lines.is_empty() {
            println!("  {}  {}", style("└").dim(), style("(empty)").dim());
            return;
        }

        let show_max = if is_error { usize::MAX } else { 4 };
        for (i, line) in lines.iter().take(show_max).enumerate() {
            let symbol = if i == lines.len() - 1 && lines.len() <= show_max {
                "└"
            } else {
                "│"
            };
            let styled = if is_error {
                style(line.to_string()).red()
            } else {
                style(truncate(line, 100)).dim()
            };
            println!("  {}  {}", style(symbol).dim(), styled);
        }

        if lines.len() > show_max {
            println!(
                "  {}  {}",
                style("└").dim(),
                style(format!("... +{} lines", lines.len() - show_max)).dim()
            );
        }
    }

    fn get_user_input(&self, prompt: &str) -> String {
        self.stop_thinking();

        // Try to use rustyline editor
        if !self.init_editor_if_needed() {
            // Editor initialization failed, use fallback
            return Self::fallback_input();
        }

        let mut editor_guard = self.editor.lock().unwrap();
        if let Some(ref mut editor) = *editor_guard {
            match editor.readline(prompt) {
                Ok(line) => {
                    editor.save_history();
                    line
                }
                Err(ReadlineError::Interrupted) => {
                    // Ctrl+C - exit gracefully
                    std::process::exit(0);
                }
                Err(ReadlineError::Eof) => {
                    // Ctrl+D - exit gracefully
                    std::process::exit(0);
                }
                Err(_) => {
                    // Other errors - fall back to basic input
                    drop(editor_guard);  // Release lock before fallback
                    Self::fallback_input()
                }
            }
        } else {
            drop(editor_guard);
            Self::fallback_input()
        }
    }

    fn display_error(&self, error: &str) {
        self.stop_thinking();
        println!("{} Error: {}", style("⏺").red(), error);
    }

    fn display_system(&self, text: &str) {
        self.stop_thinking();
        println!("{}", style(text).bold().dim());
    }

    fn confirm(&self, message: &str) -> Confirmation {
        self.stop_thinking();
        println!("\n{} {} [y/n/s]", style("⚠").yellow(), message);
        println!(
            "  {}es / {}o / {}ession",
            style("y").bold(),
            style("n").bold(),
            style("s").bold()
        );
        let input = self.get_user_input("").to_lowercase();
        match input.as_str() {
            "y" | "yes" => Confirmation::Yes,
            "s" | "session" => Confirmation::Always,
            _ => Confirmation::No,
        }
    }

    fn display_separator(&self) {
        self.stop_thinking();
        Self::separator();
    }

    fn display_thinking(&self, message: &str) {
        let mut spinner_lock = self.spinner.lock().unwrap();
        if spinner_lock.is_none() {
            *spinner_lock = Some(Self::create_spinner(message));
        }
    }

    fn stop_thinking(&self) {
        if let Some(pb) = self.spinner.lock().unwrap().take() {
            pb.finish_and_clear();
        }
    }

    fn display_header(
        &self,
        provider: &str,
        model: &str,
        yolo: bool,
        limit: usize,
        persona: Option<&str>,
    ) {
        let width = Term::stdout().size().1 as usize;
        let avatar = [
            "    ▄     ▄    ",
            "   ███   ███   ",
            "  ███████████  ",
            " ███ █   █ ███ ",
            " █████████████ ",
            "   ███   ███   ",
            "  ██       ██  ",
        ];

        println!();
        for line in avatar {
            let padding = width.saturating_sub(15) / 2;
            println!("{}{}", " ".repeat(padding), style(line).cyan());
        }
        println!();

        let status = |active, label, color: fn(StyledObject<String>) -> StyledObject<String>| {
            let s = style(format!("[{}] {}", if active { "x" } else { " " }, label));
            if active {
                color(s)
            } else {
                s.dim()
            }
        };

        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "unknown".into());

        print!(
            "{} | {} ({})",
            style("picocode").bold(),
            style(provider).cyan(),
            style(model).blue(),
        );

        if let Some(p) = persona {
            print!(" | {}", style(p).magenta());
        }

        println!(
            " | {} | {} | {}",
            status(yolo, "yolo", |s| s.red()),
            style(format!("limit:{}", limit)).yellow(),
            style(cwd).dim()
        );
    }

}
