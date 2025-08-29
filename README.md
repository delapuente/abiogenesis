# Abiogenesis (ergo)

**AI-powered command interceptor** - *cogito, ergo sum*

Abiogenesis bridges intent (cogito) to execution (sum) by generating commands on the fly when they don't exist in your system PATH. Using AI, it creates practical Deno/TypeScript commands from natural language intents and caches them for future use.

## ✨ Features

- **Command Generation**: AI generates working Deno/TypeScript commands from natural language
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

1. Clone and build:
```bash
git clone <repository-url>
cd abiogenesis
cargo build --release
```

2. Set up your API key:
```bash
./target/release/ergo --set-api-key sk-ant-your-key-here
```

3. Start using it:
```bash
# Generate and run a command to count files
./target/release/ergo file-count

# Generate a random number
./target/release/ergo random-number

# Check current timestamp  
./target/release/ergo current-time
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

### Mock Mode (for testing)

```bash
# Use mock generator instead of real API
export ABIOGENESIS_USE_MOCK=1
ergo some-command
```

## 📁 File Structure

- **Config**: `~/.abiogenesis/config.toml` - API key and settings
- **Cache**: `~/.abiogenesis/cache/production/` - Generated commands
- **Mock Cache**: `~/.abiogenesis/cache/mock/` - Mock mode commands

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

## 🏗️ Architecture

- **CommandRouter**: Routes between system commands, cache, and generation
- **LlmGenerator**: Uses Claude API to generate commands from intents
- **CommandCache**: Persistent storage for generated commands
- **Executor**: Sandboxed execution of generated Deno/TypeScript code
- **Config**: Configuration and API key management

## 🧪 Testing

```bash
# Run unit and integration tests
cargo test

# Test with mock mode (no API calls)
ABIOGENESIS_USE_MOCK=1 cargo run -- test-command
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