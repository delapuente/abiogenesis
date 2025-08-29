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

## Cache Location

Generated commands are cached in `~/.abiogenesis/cache/commands.json`

## Security Model

Generated commands run in Deno's sandbox with explicit permissions. Each command declares the permissions it needs (file access, network access, system commands, etc.) and users can see these before execution.