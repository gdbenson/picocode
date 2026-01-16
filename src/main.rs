use clap::Parser;
use picocode::{create_agent, AgentConfig, ConsoleOutput};
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(author, version, about = "Minimal coding assistant")]
struct Args {
    /// LLM provider (anthropic, openai, azure, cohere, deepseek, galadriel, gemini, groq, huggingface, hyperbolic, mira, mistral, moonshot, ollama, openrouter, perplexity, together, xai)
    #[arg(short, long, default_value = "anthropic")]
    provider: String,

    /// LLM model name
    #[arg(short, long)]
    model: Option<String>,

    /// Interactive mode (default)
    #[arg(short, long, default_value_t = true)]
    interactive: bool,

    /// Single prompt input
    #[arg(long)]
    input: Option<String>,

    /// Include the bash tool
    #[arg(long)]
    bash: bool,

    /// Run destructive tools without confirmation
    #[arg(long)]
    yolo: bool,

    /// Run in quiet mode
    #[arg(short, long)]
    quiet: bool,

    /// Maximum number of tool calls per prompt
    #[arg(long, default_value = "50")]
    tool_call_limit: usize,

    /// Choose a persona for the agent
    #[arg(long, help = format!("Choose a persona for the agent. Available built-in personas:\n{}", picocode::persona::list_personas()))]
    persona: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let output: Arc<dyn picocode::Output> = if args.quiet {
        Arc::new(picocode::QuietOutput::new())
    } else {
        Arc::new(ConsoleOutput::new())
    };

    let system_message_extension = picocode::agent::load_agents_md();
    let persona_prompt = args.persona.as_ref().and_then(|p| picocode::persona::get_persona(p));
    let persona_name = args.persona.clone();

    let model = args.model.clone().unwrap_or_else(|| match args.provider.as_str() {
        "anthropic" => "claude-3-5-sonnet-20241022".to_string(),
        "openai" => "gpt-4o-mini".to_string(),
        "azure" => "gpt-4o".to_string(),
        "cohere" => "command-r-plus".to_string(),
        "deepseek" => "deepseek-chat".to_string(),
        "galadriel" => "llama3-70b".to_string(),
        "groq" => "llama3-70b-8192".to_string(),
        "huggingface" => "meta-llama/Llama-3-70b-chat-hf".to_string(),
        "hyperbolic" => "meta-llama/Llama-3-70b-instruct".to_string(),
        "mira" => "mira-v1".to_string(),
        "mistral" => "mistral-large-latest".to_string(),
        "moonshot" => "moonshot-v1-8k".to_string(),
        "ollama" => "llama3".to_string(),
        "openrouter" => "meta-llama/llama-3-70b-instruct".to_string(),
        "perplexity" => "llama-3-sonar-large-32k-online".to_string(),
        "together" => "meta-llama/Llama-3-70b-chat-hf".to_string(),
        "xai" => "grok-1".to_string(),
        "gemini" | "google" => "gemini-1.5-pro".to_string(),
        _ => "unknown".to_string(),
    });

    let agent = create_agent(AgentConfig {
        provider: args.provider.clone(),
        model,
        output,
        use_bash: args.bash,
        yolo: args.yolo,
        tool_call_limit: args.tool_call_limit,
        system_message_extension,
        persona_prompt,
        persona_name,
    }).await?;

    match args.input {
        Some(p) => {
            let response = agent.run_once(p).await?;
            if args.quiet {
                println!("{}", response);
            }
        }
        None => agent.run_interactive().await?,
    }

    Ok(())
}
