
# yo: An open source meta agent

"yo" is a local agentic butler that helps you build, test, and maintain software projects using AI agents. It acts as a wrapper and policy engine for LLMs, providing a secure and controlled environment for automated coding tasks.

## Key Features

- **Local Execution**: Runs entirely on your machine, with access only to your project files.
- **Multi-backend Support**: Connects to various LLM providers including Venice, OpenAI, Anthropic, and local Ollama instances.
- **Permission System**: Granular control over tool usage with allow/deny/ask rules for security.
- **Interactive REPL**: Engage in conversations with the agent using `/help` for available commands.
- **Tool Integration**: Built-in tools for common operations:
  - `Read`: Read file contents
  - `Write`: Create or overwrite files
  - `Edit`: Modify files with find/replace operations
  - `Grep`: Search file contents with regex patterns
  - `Glob`: Find files using glob patterns
  - `Bash`: Execute shell commands (with safety restrictions)
- **Configuration Flexibility**: Customizable via config files at user, project, or local levels.
- **Context Management**: Handles conversation history and context compaction automatically.
- **Session Transcripts**: All interactions are logged for auditing and reproducibility.

## Getting Started

1. **Installation**: Build from source with `cargo build --release`
2. **Configuration**: Set up API keys and preferences in `~/.yo/config.toml` or project-level `.yo/config.toml`
3. **Usage Modes**:
   - Interactive: Run `yo` for REPL mode
   - One-shot: Use `yo -p "your prompt"` for single commands
4. **Environment Variables**: Configure your AI provider keys:
   - Venice: `VENICE_API_KEY`
   - OpenAI: `OPENAI_API_KEY`
   - Anthropic: `ANTHROPIC_API_KEY`

## Security Model

`yo` implements a strict permission system to prevent unauthorized actions:

- By default, potentially dangerous operations (`Write`, `Edit`, `Bash`) require explicit approval
- `curl` and `wget` commands are blocked by default for security
- Path access is restricted to the project root directory
- Configurable permission modes: `default`, `acceptEdits`, or `bypassPermissions`
- Rule-based pattern matching for fine-grained control over tool behavior

## Configuration

Configuration follows a hierarchy (highest to lowest priority):
1. Command-line arguments
2. `.yo/config.local.toml`
3. `.yo/config.toml`
4. `~/.yo/config.toml`
5. Built-in defaults

See `example-yo.toml` for a complete configuration reference.

## Development

The codebase is structured as a Rust application with the following components:
- **Agent loop**: Core reasoning and tool orchestration
- **CLI interface**: Interactive REPL and command-line options
- **Tool system**: Modular tools for file and system operations
- **Policy engine**: Permission checking and security controls
- **Configuration management**: Flexible settings loading and merging
- **Context handling**: Conversation state and history management

Refer to the project documentation for development guidelines.

