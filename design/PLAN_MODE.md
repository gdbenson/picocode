# Plan Mode in Picocode

Picocode now supports a "plan mode" inspired by Claude Code's planning workflow. This mode allows you to explore the codebase, analyze requirements, and design implementation plans before writing code.

## Quick Start

### Entering Plan Mode

```bash
picocode
c> /plan
```

Your prompt will change from `c>` (code mode) to `p>` (plan mode).

### Using Plan Mode

In plan mode, ask for a plan:

```bash
p> Create a plan for adding a configuration validation feature
```

The agent will:
- Explore the codebase using `read_file`, `grep_text`, `list_dir`, and `glob_files`
- Analyze existing patterns and architecture
- Present a structured implementation plan as markdown in the chat

### Iterating on Plans

You can refine the plan while staying in plan mode:

```bash
p> Can you also consider backward compatibility?
p> What about edge cases for invalid config?
```

### Switching to Code Mode

There are three ways to switch to code mode:

1. **Manual switch** - Switch to code mode and wait for your next instruction:
   ```bash
   p> /code
   c> Implement the plan
   ```

2. **Auto-implement with /go** - Switch to code mode AND automatically start implementation:
   ```bash
   p> /go
   ```
   This is equivalent to typing `/code` followed by "Implement the plan."

3. **Save plan first** - Optionally save the plan before implementing:
   ```bash
   p> /write my-feature-plan.md
   p> /go
   ```

## Commands

| Command | Description |
|---------|-------------|
| `/plan` | Enter plan mode (prompt changes to `p>`) |
| `/code` | Enter code mode manually (prompt changes to `c>`) |
| `/go` | Switch to code mode AND auto-send "Implement the plan." |
| `/write [filename]` | Save the last response to a file (defaults to `plan.md`) |
| `/q` or `exit` | Exit picocode |

## Workflow Examples

### Example 1: Simple Planning + Implementation

```bash
c> /plan
p> Create a plan for adding logging to the agent
# Agent explores and presents plan...
p> /go
# Agent automatically starts implementing
```

### Example 2: Iterative Planning

```bash
c> /plan
p> How should I add support for custom system prompts?
# Agent presents initial plan...
p> Can you also consider per-session prompts?
# Agent refines plan...
p> /write custom-prompts.md
p> /code
c> Start with the first step of the plan
```

### Example 3: Research Without Implementation

```bash
c> /plan
p> Analyze the current tool architecture and suggest improvements
# Agent explores and presents analysis...
p> /write architecture-analysis.md
p> /code
# Back to code mode, ready for other tasks
```

## Plan Mode Behavior

### What the Agent Does in Plan Mode

- **Explores first**: Uses read/search tools to understand the codebase
- **Analyzes patterns**: Identifies existing conventions and architecture
- **Presents in chat**: Shows plans as formatted markdown (not auto-saved to files)
- **Stays focused**: Avoids premature implementation or file editing

### Plan Structure

Plans typically follow this structure:

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

## Key Design Principles

1. **Plans are chat-based**: Plans appear in your conversation, not automatically written to files
2. **Mode is behavioral**: The agent focuses on exploration in plan mode, but all tools remain available
3. **History preserved**: Conversation history is maintained across mode switches
4. **User control**: You decide when to switch modes and when to save plans

## Tips

- Use plan mode for non-trivial features or changes that affect multiple files
- Iterate on the plan until you're satisfied with the approach
- Use `/write` to save plans for reference or documentation
- Use `/go` for a quick transition from planning to implementation
- The agent remembers the plan when you switch to code mode (it's in the conversation history)

## Technical Details

- **Mode switching**: Changes the system prompt to emphasize exploration vs. implementation
- **Prompt injection**: Mode context is injected into each user prompt
- **Visual indicators**: `p>` for plan mode, `c>` for code mode
- **No agent recreation**: Mode is managed through prompt context, not by recreating the agent
