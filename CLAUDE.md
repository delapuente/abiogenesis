# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Abiogenesis** is an AI-powered dev environment experiment that creates commands from "nothing" (hence the name). The core component is `ergo` - a command interceptor that bridges intent (cogito) to execution (sum).

When you run `ergo <command>`, it:
1. Checks if the command exists in the system PATH
2. If not, checks the local command cache
3. If still not found, generates the command using AI and caches it
4. Executes the command with appropriate sandboxing

## Project Structure

```
abiogenesis/
├── Cargo.toml              # Project configuration and dependencies
└── src/
    ├── main.rs             # Main CLI entry point for 'ergo' command
    ├── command_router.rs   # Routes commands between system/cache/generation
    ├── llm_generator.rs    # AI command generation (mock + future LLM integration)
    ├── command_cache.rs    # Persistent command storage and retrieval
    └── executor.rs         # Deno-based command execution with sandboxing
```

## Key Dependencies

- `clap` - CLI argument parsing
- `tokio` - Async runtime
- `serde` + `serde_json` - Command serialization/caching
- `reqwest` - HTTP client for future LLM API calls
- `which` - System command detection
- `dirs` - Cross-platform cache directory
- `anyhow` - Error handling
- `tracing` - Structured logging

## Architecture

**Command Generation Modes:**
1. 🤖 **Claude AI** (`ANTHROPIC_API_KEY` set) - Uses Claude 3 Haiku for intelligent command generation
2. 🔄 **Mock** (`ABIOGENESIS_USE_MOCK=1`) - Uses predefined responses for testing/development  
3. 💡 **Fallback** (default) - Uses simple template-based generation

The system follows this flow:
```
User Input → ergo → CommandRouter → System Command? → Execute
                                 → Cached Command? → Execute  
                                 → Generate → Cache → Execute (via Deno)
```

## Command Execution

Commands are executed using **Deno** for security:
- Sandboxed by default with no permissions
- Granular permission system (--allow-read, --allow-net, etc.)
- TypeScript/JavaScript runtime
- Users can see exactly what permissions each generated command requires

## Common Commands

### Building and Running
- `cargo build` - Build the project
- `cargo run` - Build and run the project (creates 'ergo' binary)
- `cargo build --release` - Build optimized release version

### Testing the System
- `cargo run hello world` - Test basic command generation
- `cargo run timestamp` - Generate timestamp command
- `cargo run project-info` - Show project information
- `cargo run weather` - Fetch weather (demonstrates network permissions)
- `cargo run uuid` - Generate UUID

### Development
- `cargo check` - Check for compile errors
- `cargo fmt` - Format code
- `cargo clippy` - Lint code

### Testing
- `cargo test` - Run all unit and integration tests
- `cargo test --test integration_test` - Run integration tests specifically
- Integration tests cover:
  - Command generation and execution
  - System command passthrough
  - Deno sandbox permissions
  - Caching behavior
  - Network and file system access

## Prerequisites

- **Rust** (2024 edition)
- **Deno** - Required for executing generated commands

## Storage Locations

**Configuration:**
- Config file: `~/.abiogenesis/config.toml`

**Command Cache:**
Generated commands are cached in separate directories based on mode:
- **Production**: `~/.abiogenesis/cache/production/commands.json`  
- **Mock Mode**: `~/.abiogenesis/cache/mock/commands.json`

This ensures that test/development commands don't interfere with production commands.

## Security Model

Generated commands run in Deno's sandbox with explicit permissions. Each command declares the permissions it needs (file access, network access, system commands, etc.) and users can see these before execution.

## Configuration

### Environment Variables

- **`ANTHROPIC_API_KEY`** - Anthropic API key for real LLM command generation
  - If set: Uses Claude 3 Haiku to generate commands
  - If not set: Uses simple fallback generation

- **`ABIOGENESIS_USE_MOCK`** - Enable deterministic mock mode
  - If set: Uses predefined mock responses for consistent testing/development
  - If not set: Uses production LLM/fallback generation
  - Useful for: Testing, development, CI/CD pipelines

### Generation Modes

1. **Production Mode** (default)
   ```bash
   export ANTHROPIC_API_KEY="your-key"
   cargo run hello world  # Uses Claude API
   ```

2. **Fallback Mode** (no API key)
   ```bash
   cargo run hello world  # Uses simple fallback generation
   ```

3. **Mock Mode** (deterministic)
   ```bash
   export ABIOGENESIS_USE_MOCK=1
   cargo run hello world  # Uses predefined mock responses
   ```

### Claude API Integration

To use real AI-powered command generation:

1. **Get an Anthropic API key** from https://console.anthropic.com

2. **Set the API key** using one of these methods:

   **Option A: Using ergo config (recommended)**
   ```bash
   ergo --set-api-key sk-ant-your-api-key-here
   ```
   
   **Option B: Environment variable**
   ```bash
   export ANTHROPIC_API_KEY="sk-ant-your-api-key-here"
   ```

3. **Run commands normally** - they'll be generated using Claude 3 Haiku

4. **Check your configuration** anytime:
   ```bash
   ergo --config
   ```

**Configuration Storage:**
- Config file: `~/.abiogenesis/config.toml`
- Environment variables override config file settings
- Safe storage (API keys are only stored locally)

**Why Claude 3 Haiku?**
- Fast and cost-effective for code generation
- Excellent at following structured JSON output requirements  
- Strong understanding of Deno APIs and TypeScript
- Designed for tool use and structured outputs

**Note:** API calls cost money. The system will **require** an API key - no fallback generation in production mode.