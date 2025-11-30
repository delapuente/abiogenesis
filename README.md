# Abiogenesis (ergo)

**AI-powered command interceptor** - *cogito, ergo sum*

Abiogenesis bridges intent (cogito) to execution (sum) by generating commands on the fly when they don't exist in your system PATH. Using AI, it creates practical Deno/TypeScript commands from natural language intents and caches them for future use.

## ✨ Features

- **Command Generation**: AI generates working Deno/TypeScript commands from natural language
- **Corrective Feedback**: Iteratively improve commands with `--nope` when they don't meet expectations
- **Intelligent Caching**: Generated commands are cached and reused across sessions
- **Sandboxed Execution**: All generated code runs in Deno's secure sandbox with minimal permissions
- **Fallback to System**: Existing system commands work normally - only generates when commands don't exist
- **Configuration Management**: Easy API key setup and configuration

## 🚀 Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (for building)
- [Deno](https://deno.land/) (for executing generated commands)
- [Anthropic API key](https://console.anthropic.com/) (for command generation)

### Installation

#### Quick Install (Recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/user/abiogenesis/main/install.sh | bash
```

This installer will:
- Check system requirements (Rust, Deno, Git) 
- Install Deno if not present
- Build and install the `ergo` binary to `~/.local/bin`
- Update your PATH if needed

#### Manual Installation

```bash
git clone <repository-url>
cd abiogenesis
cargo build --release
cp target/release/ergo ~/.local/bin/
```

### Setup

1. Set up your API key:
```bash
ergo --set-api-key sk-ant-your-key-here
```

2. Start using it:
```bash
# Generate and run a command to count files
ergo hello world

# Generate a timestamp
ergo timestamp

# Natural language description
ergo "show me the weather"
```

## 🎯 How It Works

1. **Intent Recognition**: You provide a command name and optional arguments
2. **PATH Check**: First checks if the command exists in your system PATH
3. **AI Generation**: If not found, uses Claude AI to generate a working Deno/TypeScript implementation
4. **Caching**: Stores generated commands for future reuse
5. **Sandboxed Execution**: Runs the command in Deno with minimal required permissions

## 🔧 Configuration

### API Key Management

```bash
# Set API key in config file
ergo --set-api-key sk-ant-your-key-here

# Or use environment variable
export ANTHROPIC_API_KEY=sk-ant-your-key-here

# Check current configuration
ergo --config
```


## 📁 File Structure

- **Config**: `~/.abiogenesis/config.toml` - API key and settings
- **Logs**: `~/.abiogenesis/ergo.log` - Operation logs and debugging info
- **Cache**: `~/.abiogenesis/biomas/production/` - Generated commands

## 🔍 Logging

Ergo logs all operations to `~/.abiogenesis/ergo.log` for debugging and audit purposes:

- **Default**: Info level logging (command executions, cache operations)
- **Verbose mode (`-v`)**: Debug level logging (detailed generation steps)
- **View logs**: `tail -f ~/.abiogenesis/ergo.log`
- **Log location**: `ergo --config` shows the current log file path

Logs include timestamps, operation details, and error information without cluttering stdout.

## 🛡️ Security

- **Sandboxed Execution**: All generated code runs in Deno's secure sandbox
- **Minimal Permissions**: Commands request only necessary permissions (often none)
- **No Arbitrary Code**: AI generates structured, predictable TypeScript/JavaScript
- **Local Caching**: Commands are cached locally, not sent to external services

## 🔍 Examples

### File Operations
```bash
ergo file-count          # Count files in current directory
ergo file-size README.md # Get file size information
```

### Utilities  
```bash
ergo generate-uuid       # Generate a new UUID
ergo random-number       # Generate random number
ergo current-timestamp   # Show current timestamp
```

### System Information
```bash
ergo project-info        # Show project details (git branch, file count, etc.)
```

### Corrective Feedback

When a generated command doesn't work as expected, use `--nope` to improve it:

```bash
# Generate a password command
ergo password
# Output: "x7k2m"  (too short!)

# Provide feedback to regenerate with improvements
ergo --nope "make it at least 20 characters with uppercase, numbers, and symbols"
# Output: "K9$mX2@pL5#nR8&vQ1!w"

# Or if the command failed with an error, just run --nope
# to use the stderr as context for regeneration
ergo broken-command
# Error: TypeError: Cannot read property 'foo' of undefined

ergo --nope
# Regenerates the command using the error output as context
```

The corrective feedback loop:
- Preserves the command name
- Includes stderr from the last execution (if any) as context
- Accepts optional feedback text to guide improvements
- Re-prompts for permission approval since the code changed

## 🏗️ Architecture

- **CommandRouter**: Routes between system commands, cache, and generation
- **LlmGenerator**: Uses Claude API to generate commands from intents
- **CommandCache**: Persistent storage for generated commands
- **Executor**: Sandboxed execution of generated Deno/TypeScript code
- **Config**: Configuration and API key management

## 🧪 Development

```bash
# Build and run
cargo build
cargo run -- hello world

# Check code
cargo check
cargo clippy
```

## 🤝 Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📄 License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- **Anthropic** for Claude AI API
- **Deno** for secure JavaScript/TypeScript runtime
- **Philosophy**: Named after abiogenesis (life from non-life) and Descartes' "cogito, ergo sum"

---

*"I think, therefore I am" - but now your shell thinks, therefore commands exist.*