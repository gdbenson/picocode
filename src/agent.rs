use crate::output::Confirmation;
use crate::tools::{
    AgentBrowser, Bash, CopyFile, EditFile, GlobFiles, GrepText, ListDir, MakeDir, MoveFile,
    ReadFile, Remove, WriteFile,
};
use crate::Output;
use crate::Result;
use rig::agent::{Agent, AgentBuilder, CancelSignal, PromptHook};
use rig::client::{CompletionClient, ProviderClient};
use rig::completion::{CompletionModel, Prompt, ToolDefinition};
use rig::message::Message;
use rig::providers::{
    anthropic, azure, cohere, deepseek, galadriel, gemini, groq, huggingface, hyperbolic, mira,
    mistral, moonshot, ollama, openai, openrouter, perplexity, together, xai,
};
use serde_json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use async_trait::async_trait;

#[async_trait]
pub trait PicoAgent: Send + Sync {
    async fn run_interactive(self: Box<Self>) -> Result<()>;
    async fn run_once(&self, input: String) -> Result<String>;
}

#[async_trait]
impl<M: CompletionModel + 'static> PicoAgent for CodeAgent<M> {
    async fn run_interactive(self: Box<Self>) -> Result<()> {
        self.output.display_header(
            &self.provider,
            &self.model,
            self.bash,
            self.yolo,
            self.tool_call_limit,
        );
        let mut history = Vec::new();
        loop {
            self.output.display_separator();
            let input = self.output.get_user_input();
            if input.is_empty() {
                continue;
            }
            if input == "/q" || input == "exit" {
                break;
            }
            self.output.display_separator();

            let response = self.prompt(&input, Some(&mut history)).await?;
            self.output.display_text(&response);
        }
        Ok(())
    }

    async fn run_once(&self, input: String) -> Result<String> {
        self.output.display_header(
            &self.provider,
            &self.model,
            self.bash,
            self.yolo,
            self.tool_call_limit,
        );
        self.output.display_separator();
        let response = self.prompt(&input, None).await?;
        self.output.display_text(&response);
        Ok(response)
    }
}

fn is_tool_available(tool: &str) -> bool {
    std::process::Command::new("which")
        .arg(tool)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub struct CodeAgent<M: CompletionModel> {
    agent: Agent<M>,
    output: Arc<dyn Output>,
    tool_call_limit: usize,
    provider: String,
    model: String,
    bash: bool,
    yolo: bool,
}

pub struct AgentConfig {
    pub provider: String,
    pub model: String,
    pub output: Arc<dyn Output>,
    pub use_bash: bool,
    pub yolo: bool,
    pub tool_call_limit: usize,
    pub system_message_extension: Option<String>,
}

pub async fn create_agent(config: AgentConfig) -> Result<Box<dyn PicoAgent>> {
    let provider = config.provider.to_lowercase();
    let model = config.model.clone();

    macro_rules! build {
        ($client:expr) => {{
            let builder = $client.agent(&model);
            let rig_agent = build_rig_agent(
                builder,
                config.use_bash,
                config.yolo,
                config.output.clone(),
                config.system_message_extension,
            );

            Box::new(CodeAgent::new(
                rig_agent,
                config.output,
                config.tool_call_limit,
                config.provider,
                model,
                config.use_bash,
                config.yolo,
            ))
        }};
    }

    let agent: Box<dyn PicoAgent> = match provider.as_str() {
        "anthropic" => build!(anthropic::Client::from_env()),
        "openai" => build!(openai::Client::from_env()),
        "azure" => build!(azure::Client::from_env()),
        "cohere" => build!(cohere::Client::from_env()),
        "deepseek" => build!(deepseek::Client::from_env()),
        "galadriel" => build!(galadriel::Client::from_env()),
        "gemini" | "google" => build!(gemini::Client::from_env()),
        "groq" => build!(groq::Client::from_env()),
        "huggingface" => build!(huggingface::Client::from_env()),
        "hyperbolic" => build!(hyperbolic::Client::from_env()),
        "mira" => build!(mira::Client::from_env()),
        "mistral" => build!(mistral::Client::from_env()),
        "moonshot" => build!(moonshot::Client::from_env()),
        "ollama" => build!(ollama::Client::from_env()),
        "openrouter" => build!(openrouter::Client::from_env()),
        "perplexity" => build!(perplexity::Client::from_env()),
        "together" => build!(together::Client::from_env()),
        "xai" => build!(xai::Client::from_env()),
        _ => {
            return Err(crate::PicocodeError::Other(format!(
                "Unsupported provider: {}",
                provider
            )))
        }
    };

    Ok(agent)
}

pub fn load_agents_md() -> Option<String> {
    let path = std::path::Path::new("AGENTS.md");
    if path.exists() {
        return std::fs::read_to_string(path).ok();
    }
    None
}

#[derive(Clone)]
struct LoggingHook {
    output: Arc<dyn Output>,
}

impl<M: CompletionModel> PromptHook<M> for LoggingHook {
    async fn on_tool_call(
        &self,
        tool_name: &str,
        _tool_call_id: Option<String>,
        args: &str,
        _cancel_sig: CancelSignal,
    ) {
        let args_json =
            serde_json::from_str(args).unwrap_or(serde_json::Value::String(args.to_string()));
        self.output.display_tool_call(tool_name, &args_json);
    }

    async fn on_tool_result(
        &self,
        _tool_name: &str,
        _tool_call_id: Option<String>,
        _args: &str,
        result: &str,
        _cancel_sig: CancelSignal,
    ) {
        self.output.display_tool_result(result);
    }
}

fn build_rig_agent<M: CompletionModel>(
    builder: AgentBuilder<M>,
    use_bash: bool,
    yolo: bool,
    output: Arc<dyn Output>,
    system_message_extension: Option<String>,
) -> Agent<M> {
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let mut system_message = format!("Concise coding assistant. cwd: {}", cwd);
    if let Some(ext) = system_message_extension {
        system_message.push_str("\n\n");
        system_message.push_str(&ext);
    }

    let mut builder = builder
        .preamble(&system_message)
        .tool(ReadFile)
        .tool(WriteFile)
        .tool(EditFile)
        .tool(GlobFiles)
        .tool(GrepText)
        .tool(ListDir);

    builder = builder
        .tool(guard(MakeDir, yolo, output.clone()))
        .tool(guard(Remove, yolo, output.clone()))
        .tool(guard(MoveFile, yolo, output.clone()))
        .tool(guard(CopyFile, yolo, output.clone()));

    if use_bash {
        builder = builder.tool(guard(Bash, yolo, output.clone()));
    }
    if is_tool_available("agent-browser") {
        builder = builder.tool(guard(AgentBrowser, yolo, output.clone()));
    }
    builder.build()
}

use rig::tool::Tool;

struct Guard<T: Tool> {
    tool: T,
    yolo: bool,
    output: Arc<dyn Output>,
    always: Arc<AtomicBool>,
}

impl<T: Tool<Error = crate::tools::ToolError>> Tool for Guard<T> {
    type Args = T::Args;
    type Output = T::Output;
    type Error = T::Error;

    const NAME: &'static str = T::NAME;

    async fn definition(&self, prompt: String) -> ToolDefinition {
        self.tool.definition(prompt).await
    }

    async fn call(&self, args: Self::Args) -> std::result::Result<Self::Output, Self::Error> {
        if !self.yolo && !self.always.load(Ordering::Relaxed) {
            match self
                .output
                .confirm(&format!("Confirm tool {} call?", Self::NAME.to_uppercase()))
            {
                Confirmation::Always => {
                    self.always.store(true, Ordering::Relaxed);
                }
                Confirmation::Yes => {}
                Confirmation::No => {
                    return Err(crate::tools::ToolError::Generic(
                        "Action cancelled by user".into(),
                    ));
                }
            }
        }
        self.tool.call(args).await
    }
}

fn guard<T: Tool>(tool: T, yolo: bool, output: Arc<dyn Output>) -> Guard<T> {
    Guard {
        tool,
        yolo,
        output,
        always: Arc::new(AtomicBool::new(false)),
    }
}

impl<M: CompletionModel + 'static> CodeAgent<M> {
    pub fn new(
        agent: Agent<M>,
        output: Arc<dyn Output>,
        tool_call_limit: usize,
        provider: String,
        model: String,
        bash: bool,
        yolo: bool,
    ) -> Self {
        Self {
            agent,
            output,
            tool_call_limit,
            provider,
            model,
            bash,
            yolo,
        }
    }

    async fn prompt(&self, input: &str, history: Option<&mut Vec<Message>>) -> Result<String> {
        self.output.display_thinking("Thinking...");
        let mut builder = self
            .agent
            .prompt(input)
            .with_hook(LoggingHook {
                output: self.output.clone(),
            })
            .multi_turn(self.tool_call_limit);

        if let Some(h) = history {
            builder = builder.with_history(h);
        }

        let response = builder
            .await
            .map_err(|e| crate::PicocodeError::Other(e.to_string()))?;
        self.output.stop_thinking();
        Ok(response.to_string())
    }
}
