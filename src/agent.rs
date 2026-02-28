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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AgentMode {
    Code,
    Plan,
}

impl AgentMode {
    fn prompt_symbol(&self) -> &'static str {
        match self {
            AgentMode::Code => "c>",
            AgentMode::Plan => "p>",
        }
    }
}

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
            self.yolo,
            self.tool_call_limit,
            self.persona_name.as_deref(),
        );

        // Add usage hint
        self.output.display_system("💡 Tip: Press Enter for new line, Alt+Enter to submit");

        let mut history = Vec::new();
        let mut current_mode = AgentMode::Code;
        let mut responses: Vec<String> = Vec::new(); // For /write

        loop {
            self.output.display_separator();

            let prompt = format!("{} ", current_mode.prompt_symbol());
            let input = self.output.get_user_input(&prompt);

            if input.is_empty() {
                continue;
            }

            // Handle /plan command
            if input == "/plan" {
                if current_mode == AgentMode::Plan {
                    self.output.display_system("Already in plan mode");
                } else {
                    current_mode = AgentMode::Plan;
                    self.output.display_system("Switched to PLAN mode. Ask for a plan to begin exploration.");
                }
                continue;
            }

            // Handle /code command
            if input == "/code" {
                if current_mode == AgentMode::Code {
                    self.output.display_system("Already in code mode");
                } else {
                    current_mode = AgentMode::Code;
                    self.output.display_system("Switched to CODE mode. Ready to implement.");
                }
                continue;
            }

            // Handle /write command
            if input.starts_with("/write") {
                let filename = input
                    .strip_prefix("/write")
                    .unwrap()
                    .trim();
                let filename = if filename.is_empty() {
                    "plan.md"
                } else {
                    filename
                };

                if let Some(last_response) = responses.last() {
                    std::fs::write(filename, last_response)
                        .map_err(|e| crate::PicocodeError::Other(format!("Failed to save response: {}", e)))?;
                    self.output.display_system(&format!("Response saved to: {}", filename));
                } else {
                    self.output.display_system("No response to save yet");
                }
                continue;
            }

            // Handle /go command - switch to code mode and auto-implement
            if input == "/go" {
                if current_mode == AgentMode::Code {
                    self.output.display_system("Already in code mode");
                    continue;
                }

                current_mode = AgentMode::Code;
                self.output.display_system("Switched to CODE mode. Implementing the plan...");
                self.output.display_separator();

                // Automatically send "Implement the plan." to the agent
                let response = self.prompt("Implement the plan.", Some(&mut history)).await?;
                responses.push(response.clone());
                self.output.display_text(&response);
                continue;
            }

            // Handle exit commands
            if input == "/q" || input == "exit" {
                break;
            }

            self.output.display_separator();

            // Inject mode-specific context into the prompt
            let prompt_with_mode = match current_mode {
                AgentMode::Plan => format!("{}\n\nUser Request: {}", PLAN_MODE_PROMPT, input),
                AgentMode::Code => input,
            };

            let response = self.prompt(&prompt_with_mode, Some(&mut history)).await?;
            responses.push(response.clone());
            self.output.display_text(&response);
        }

        Ok(())
    }

    async fn run_once(&self, input: String) -> Result<String> {
        self.output.display_header(
            &self.provider,
            &self.model,
            self.yolo,
            self.tool_call_limit,
            self.persona_name.as_deref(),
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
    yolo: bool,
    persona_name: Option<String>,
}

pub struct AgentConfig {
    pub provider: String,
    pub model: String,
    pub output: Arc<dyn Output>,
    pub yolo: bool,
    pub tool_call_limit: usize,
    pub system_message_extension: Option<String>,
    pub persona_prompt: Option<String>,
    pub persona_name: Option<String>,
    pub bash_auto_allow: Option<Vec<String>>,
    pub agent_prompt: Option<String>,
}

pub async fn create_agent(config: AgentConfig) -> Result<Box<dyn PicoAgent>> {
    let provider = config.provider.to_lowercase();
    let model = config.model.clone();

    macro_rules! build {
        ($client:expr) => {{
            let builder = $client.agent(&model);
            let rig_agent = build_rig_agent(
                builder,
                config.yolo,
                config.output.clone(),
                config.system_message_extension,
                config.persona_prompt,
                config.bash_auto_allow.unwrap_or_default(),
                config.agent_prompt,
            );

            Box::new(CodeAgent::new(
                rig_agent,
                config.output,
                config.tool_call_limit,
                config.provider,
                model,
                config.yolo,
                config.persona_name,
            ))
        }};
    }

    macro_rules! check_env {
        ($var:expr) => {
            if std::env::var($var).is_err() {
                return Err(crate::PicocodeError::MissingApiKey(
                    provider.to_string(),
                    $var.to_string(),
                ));
            }
        };
    }

    let agent: Box<dyn PicoAgent> = match provider.as_str() {
        "anthropic" => {
            check_env!("ANTHROPIC_API_KEY");
            build!(anthropic::Client::from_env())
        }
        "openai" => {
            check_env!("OPENAI_API_KEY");
            build!(openai::Client::from_env())
        }
        "azure" => {
            check_env!("AZURE_OPENAI_API_KEY");
            check_env!("AZURE_OPENAI_ENDPOINT");
            build!(azure::Client::from_env())
        }
        "cohere" => {
            check_env!("COHERE_API_KEY");
            build!(cohere::Client::from_env())
        }
        "deepseek" => {
            check_env!("DEEPSEEK_API_KEY");
            build!(deepseek::Client::from_env())
        }
        "galadriel" => {
            check_env!("GALADRIEL_API_KEY");
            build!(galadriel::Client::from_env())
        }
        "gemini" | "google" => {
            check_env!("GOOGLE_API_KEY");
            build!(gemini::Client::from_env())
        }
        "groq" => {
            check_env!("GROQ_API_KEY");
            build!(groq::Client::from_env())
        }
        "huggingface" => {
            check_env!("HF_TOKEN");
            build!(huggingface::Client::from_env())
        }
        "hyperbolic" => {
            check_env!("HYPERBOLIC_API_KEY");
            build!(hyperbolic::Client::from_env())
        }
        "mira" => {
            check_env!("MIRA_API_KEY");
            build!(mira::Client::from_env())
        }
        "mistral" => {
            check_env!("MISTRAL_API_KEY");
            build!(mistral::Client::from_env())
        }
        "moonshot" => {
            check_env!("MOONSHOT_API_KEY");
            build!(moonshot::Client::from_env())
        }
        "ollama" => {
            if std::env::var("OLLAMA_API_BASE_URL").is_err() {
                std::env::set_var("OLLAMA_API_BASE_URL", "http://localhost:11434");
            }
            build!(ollama::Client::from_env())
        }
        "openrouter" => {
            check_env!("OPENROUTER_API_KEY");
            build!(openrouter::Client::from_env())
        }
        "perplexity" => {
            check_env!("PERPLEXITY_API_KEY");
            build!(perplexity::Client::from_env())
        }
        "together" => {
            check_env!("TOGETHER_API_KEY");
            build!(together::Client::from_env())
        }
        "xai" => {
            check_env!("XAI_API_KEY");
            build!(xai::Client::from_env())
        }
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

const DEFAULT_AGENT_PROMPT: &str = r#"You are picocode, a world-class software engineering agent. You are direct, technical, and highly efficient.

Your mission is to assist the user in their development tasks by utilizing a set of specialized tools. You operate within a specific codebase and must maintain its integrity while delivering high-quality solutions.

### WORKFLOW & STRATEGY
1. **Understand Before Acting**: Always start by exploring the codebase. Use `list_dir` to see the structure and `read_file` or `grep_text` to understand existing logic and patterns.
2. **Be Precise**: When editing files, use `edit_file` with enough context in `old_string` to ensure a unique match. Avoid replacing large blocks if a small change suffices.
3. **Verify Everything**: After modifying code, verify the results. Run tests or build commands via `bash`. Read the modified file to ensure the change was applied correctly.
4. **Tool Mastery**:
   - `read_file`: Use to read code. Note that it provides line numbers (e.g., `  10| code`). These are for your reference only; do not include them in your output or when writing files.
   - `bash`: Your window to the system. Use it for compilation, testing, and complex automation.
   - `agent_browser`: Use for external documentation, searching for solutions, or web-related debugging.
5. **Context**: You are working in the directory provided below. All paths are relative to this directory.

### GUIDING PRINCIPLES
- **Clean Code**: Follow established patterns in the codebase. Write idiomatic, readable, and well-documented code.
- **Security First**: Be vigilant about security vulnerabilities. Sanitize inputs, avoid hardcoded secrets, and follow least-privilege principles.
- **Minimalism**: Don't add unnecessary dependencies or over-engineer solutions.
- **Communication**: Keep explanations brief and focused on the "how" and "why" of your technical decisions.
"#;

const PLAN_MODE_PROMPT: &str = r#"You are picocode in PLANNING MODE. Your role is to explore, analyze, and design implementation plans before writing code.

### PLANNING MODE WORKFLOW

1. **Deep Exploration**: Start by thoroughly understanding the codebase
   - Use `list_dir` to understand project structure
   - Use `read_file` to examine relevant files
   - Use `grep_text` to find patterns, functions, and related code
   - Use `glob_files` to locate files by name patterns

2. **Analysis**: Understand the problem in context
   - What exists already that can be reused?
   - What patterns does this codebase follow?
   - What are the dependencies and constraints?
   - What are potential edge cases or challenges?

3. **Design**: Present a clear implementation plan
   - Break down the task into logical steps
   - Identify specific files to modify and why
   - Suggest code patterns that match the existing codebase
   - Consider testing and verification approaches
   - Present the plan as structured markdown in your response

4. **Iteration**: Be ready to refine the plan
   - Answer questions about the approach
   - Adjust based on user feedback
   - Consider alternative approaches if requested

### IMPORTANT GUIDELINES FOR PLANNING MODE

- **Present plans in chat**: Write your plan as markdown in your response, not to a file
- **Exploration over execution**: Focus on reading and understanding, not editing
- **Be thorough but concise**: Provide enough detail to implement, but stay focused
- **Avoid premature implementation**: Don't edit files or run commands unless necessary for understanding
- **Ask clarifying questions**: If requirements are unclear, ask before finalizing the plan
- **Think architecturally**: Consider how changes fit into the larger codebase

### PLAN STRUCTURE TEMPLATE

When presenting a plan, use this structure:

```markdown
## Implementation Plan: [Task Name]

### Context
- Why this change is needed
- Current state of the codebase
- Key files and components involved

### Approach
1. Step-by-step breakdown
2. Files to modify and why
3. Functions/components to add or change

### Verification
- How to test the changes
- Edge cases to consider
```

Remember: You're in planning mode. The user will switch to code mode when ready to implement.
"#;

fn build_rig_agent<M: CompletionModel>(
    builder: AgentBuilder<M>,
    yolo: bool,
    output: Arc<dyn Output>,
    system_message_extension: Option<String>,
    persona_prompt: Option<String>,
    bash_auto_allow: Vec<String>,
    agent_prompt: Option<String>,
) -> Agent<M> {
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let mut system_message = agent_prompt.unwrap_or_else(|| {
        format!("{}\n\nCurrent working directory: {}", DEFAULT_AGENT_PROMPT, cwd)
    });
    if let Some(persona) = persona_prompt {
        system_message = format!("{}\n\n{}", persona, system_message);
    }
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
        .tool(guard(MakeDir, yolo, output.clone(), None))
        .tool(guard(Remove, yolo, output.clone(), None))
        .tool(guard(MoveFile, yolo, output.clone(), None))
        .tool(guard(CopyFile, yolo, output.clone(), None));

    let auto_allow = bash_auto_allow.clone();
    builder = builder.tool(guard(
        Bash,
        yolo,
        output.clone(),
        Some(Arc::new(move |args| {
            auto_allow.iter().any(|pattern| {
                regex::Regex::new(pattern)
                    .map(|re| re.is_match(&args.cmd))
                    .unwrap_or(false)
            })
        })),
    ));

    if is_tool_available("agent-browser") {
        builder = builder.tool(guard(AgentBrowser, yolo, output.clone(), None));
    }
    builder.build()
}

use rig::tool::Tool;

struct Guard<T: Tool> {
    tool: T,
    yolo: bool,
    output: Arc<dyn Output>,
    always: Arc<AtomicBool>,
    auto_approve: Option<Arc<dyn Fn(&T::Args) -> bool + Send + Sync>>,
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
        let should_auto_approve = self
            .auto_approve
            .as_ref()
            .map(|f| f(&args))
            .unwrap_or(false);

        if !self.yolo && !self.always.load(Ordering::Relaxed) && !should_auto_approve {
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

fn guard<T: Tool>(
    tool: T,
    yolo: bool,
    output: Arc<dyn Output>,
    auto_approve: Option<Arc<dyn Fn(&T::Args) -> bool + Send + Sync>>,
) -> Guard<T> {
    Guard {
        tool,
        yolo,
        output,
        always: Arc::new(AtomicBool::new(false)),
        auto_approve,
    }
}

impl<M: CompletionModel + 'static> CodeAgent<M> {
    pub fn new(
        agent: Agent<M>,
        output: Arc<dyn Output>,
        tool_call_limit: usize,
        provider: String,
        model: String,
        yolo: bool,
        persona_name: Option<String>,
    ) -> Self {
        Self {
            agent,
            output,
            tool_call_limit,
            provider,
            model,
            yolo,
            persona_name,
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
