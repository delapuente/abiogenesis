//! Command execution module for running system and generated commands.
//!
//! This module handles the execution of:
//! - System commands (passed through to the OS)
//! - Generated Deno/TypeScript commands (sandboxed execution)
//!
//! All generated commands are executed through Deno's sandboxed runtime with
//! explicit permission grants for security.

use crate::command_cache::CommandCache;
use crate::execution_context::ExecutionContext;
use crate::llm_generator::GeneratedCommand;
use anyhow::{anyhow, Result};
use std::process::{Command, Output};
use tracing::{error, info};

/// Result of executing a generated command.
#[derive(Debug)]
pub struct ExecutionResult {
    /// Whether the command succeeded.
    pub success: bool,
    /// Standard error output (if any).
    pub stderr: Option<String>,
}

// =============================================================================
// Traits for Dependency Injection
// =============================================================================

/// Trait for running system processes.
///
/// This abstraction enables testing without spawning real processes.
pub trait ProcessRunner: Send + Sync {
    /// Executes a command and returns its output.
    fn run(&self, program: &str, args: &[&str]) -> Result<Output>;

    /// Checks if a program exists in PATH.
    fn program_exists(&self, program: &str) -> bool;
}

/// Trait for retrieving script content.
///
/// This abstraction decouples the executor from the cache implementation.
pub trait ScriptProvider {
    /// Gets the script content for a generated command.
    fn get_script(&self, command: &GeneratedCommand) -> Result<String>;
}

// =============================================================================
// Default Implementations
// =============================================================================

/// Default process runner using std::process::Command.
pub struct SystemProcessRunner;

impl ProcessRunner for SystemProcessRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<Output> {
        let mut cmd = Command::new(program);
        cmd.args(args);
        Ok(cmd.output()?)
    }

    fn program_exists(&self, program: &str) -> bool {
        which::which(program).is_ok()
    }
}

/// Script provider backed by CommandCache.
impl ScriptProvider for CommandCache {
    fn get_script(&self, command: &GeneratedCommand) -> Result<String> {
        self.get_script_content(command)
    }
}

// =============================================================================
// Executor Implementation
// =============================================================================

/// Executes system commands and generated Deno scripts.
///
/// The executor handles two types of command execution:
/// 1. **System commands** - Passed directly to the operating system
/// 2. **Generated commands** - TypeScript scripts executed in Deno's sandbox
///
/// # Security
///
/// Generated commands run in Deno's permission sandbox. Each command declares
/// its required permissions, which are passed to Deno at runtime.
///
/// # Example
///
/// ```ignore
/// let executor = Executor::new(false);
/// executor.execute_system_command(&["ls".to_string(), "-la".to_string()]).await?;
/// ```
pub struct Executor {
    verbose: bool,
}

impl Executor {
    /// Creates a new executor.
    ///
    /// # Arguments
    ///
    /// * `verbose` - If true, prints additional output during execution
    pub fn new(verbose: bool) -> Self {
        Self { verbose }
    }

    /// Executes a system command directly.
    ///
    /// The command is passed through to the operating system without sandboxing.
    ///
    /// # Arguments
    ///
    /// * `args` - Command name followed by arguments (e.g., `["ls", "-la"]`)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No command is provided (empty args)
    /// - The command fails to execute
    /// - The command exits with a non-zero status
    pub async fn execute_system_command(&self, args: &[String]) -> Result<()> {
        self.execute_system_command_with_runner(args, &SystemProcessRunner, &mut std::io::stdout(), &mut std::io::stderr())
    }

    /// Executes a system command with injected dependencies (for testing).
    pub fn execute_system_command_with_runner<W1: std::io::Write, W2: std::io::Write>(
        &self,
        args: &[String],
        runner: &impl ProcessRunner,
        stdout: &mut W1,
        stderr: &mut W2,
    ) -> Result<()> {
        if args.is_empty() {
            return Err(anyhow!("No command provided"));
        }

        let command_name = &args[0];
        let command_args: Vec<&str> = args[1..].iter().map(|s| s.as_str()).collect();

        info!("Executing system command: {} {:?}", command_name, command_args);

        let output = runner.run(command_name, &command_args)?;

        Self::handle_output(&output, stdout, stderr)?;

        Ok(())
    }

    /// Executes a cached command.
    ///
    /// This is a convenience wrapper around `execute_generated_command` that
    /// takes ownership of the command.
    pub async fn execute_cached_command(
        &self,
        command: GeneratedCommand,
        cache: &CommandCache,
        args: &[String],
    ) -> Result<()> {
        info!("Executing cached command: {} - {}", command.name, command.description);
        self.execute_generated_command(&command, cache, args).await
    }

    /// Executes a generated Deno command.
    ///
    /// The command script is retrieved from the cache and executed in Deno's
    /// sandbox with the specified permissions.
    ///
    /// # Arguments
    ///
    /// * `command` - The generated command metadata
    /// * `cache` - Command cache to retrieve the script from
    /// * `args` - Arguments to pass to the script
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The script cannot be retrieved from cache
    /// - Deno is not installed
    /// - The script execution fails
    pub async fn execute_generated_command(
        &self,
        command: &GeneratedCommand,
        cache: &CommandCache,
        args: &[String],
    ) -> Result<()> {
        self.execute_generated_command_with_deps(
            command,
            cache,
            args,
            &SystemProcessRunner,
            &mut std::io::stdout(),
            &mut std::io::stderr(),
        )
    }

    /// Executes a generated Deno command and saves execution context.
    ///
    /// This variant saves the execution context (command name, script, stderr)
    /// to enable the `--nope` feedback loop.
    ///
    /// # Arguments
    ///
    /// * `command` - The generated command metadata
    /// * `cache` - Command cache to retrieve the script from
    /// * `args` - Arguments to pass to the script
    ///
    /// # Returns
    ///
    /// Returns `ExecutionResult` with success status and stderr output.
    pub async fn execute_generated_command_with_context(
        &self,
        command: &GeneratedCommand,
        cache: &CommandCache,
        args: &[String],
    ) -> ExecutionResult {
        let mut stdout_buf = Vec::new();
        let mut stderr_buf = Vec::new();

        let script_content = match cache.get_script_content(command) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("Error: {}", e);
                return ExecutionResult {
                    success: false,
                    stderr: Some(e.to_string()),
                };
            }
        };

        let result = self.execute_generated_command_with_deps(
            command,
            cache,
            args,
            &SystemProcessRunner,
            &mut stdout_buf,
            &mut stderr_buf,
        );

        // Print captured output
        if !stdout_buf.is_empty() {
            print!("{}", String::from_utf8_lossy(&stdout_buf));
        }
        if !stderr_buf.is_empty() {
            eprint!("{}", String::from_utf8_lossy(&stderr_buf));
        }

        let success = result.is_ok();
        let stderr_str = if stderr_buf.is_empty() {
            None
        } else {
            Some(String::from_utf8_lossy(&stderr_buf).to_string())
        };

        // Save execution context for --nope feedback
        let context = ExecutionContext::new(
            &command.name,
            &script_content,
            stderr_str.clone(),
            success,
        );
        if let Err(e) = context.save() {
            error!("Failed to save execution context: {}", e);
        }

        ExecutionResult {
            success,
            stderr: stderr_str,
        }
    }

    /// Executes a generated command with injected dependencies (for testing).
    pub fn execute_generated_command_with_deps<S, P, W1, W2>(
        &self,
        command: &GeneratedCommand,
        script_provider: &S,
        args: &[String],
        runner: &P,
        stdout: &mut W1,
        stderr: &mut W2,
    ) -> Result<()>
    where
        S: ScriptProvider,
        P: ProcessRunner,
        W1: std::io::Write,
        W2: std::io::Write,
    {
        info!("Executing generated command: {} - {}", command.name, command.description);

        if self.verbose {
            writeln!(stdout, "🤖 Executing generated command: {}", command.description)?;

            if !command.permissions.is_empty() {
                let permission_strings: Vec<String> = command.permissions
                    .iter()
                    .map(|p| p.permission.clone())
                    .collect();
                writeln!(stdout, "🔒 Deno permissions required: {}", permission_strings.join(" "))?;
            }
        }

        let script_content = script_provider.get_script(command)?;
        let permission_strings: Vec<String> = command.permissions
            .iter()
            .map(|p| p.permission.clone())
            .collect();

        self.execute_deno_script_with_deps(&script_content, &permission_strings, args, runner, stdout, stderr)
    }

    /// Executes a Deno script with injected dependencies (for testing).
    fn execute_deno_script_with_deps<P, W1, W2>(
        &self,
        script: &str,
        permissions: &[String],
        args: &[String],
        runner: &P,
        stdout: &mut W1,
        stderr: &mut W2,
    ) -> Result<()>
    where
        P: ProcessRunner,
        W1: std::io::Write,
        W2: std::io::Write,
    {
        if !runner.program_exists("deno") {
            return Err(anyhow!(
                "Deno is not installed. Please install Deno to execute generated commands."
            ));
        }

        // Create a temporary file for the script
        let temp_dir = std::env::temp_dir();
        let script_path = temp_dir.join(format!("ergo_script_{}.ts", std::process::id()));

        std::fs::write(&script_path, script)?;

        // Build deno arguments
        let script_path_str = script_path.to_string_lossy();
        let mut deno_args: Vec<&str> = vec!["run"];
        for perm in permissions {
            deno_args.push(perm.as_str());
        }
        deno_args.push(&script_path_str);
        for arg in args {
            deno_args.push(arg.as_str());
        }

        let output = runner.run("deno", &deno_args);

        // Clean up temporary file
        let _ = std::fs::remove_file(&script_path);

        let output = output?;
        Self::handle_output(&output, stdout, stderr)?;

        Ok(())
    }

    /// Handles command output, writing to stdout/stderr and checking status.
    fn handle_output<W1: std::io::Write, W2: std::io::Write>(
        output: &Output,
        stdout: &mut W1,
        stderr: &mut W2,
    ) -> Result<()> {
        if output.status.success() {
            if !output.stdout.is_empty() {
                write!(stdout, "{}", String::from_utf8_lossy(&output.stdout))?;
            }
            if !output.stderr.is_empty() {
                write!(stderr, "{}", String::from_utf8_lossy(&output.stderr))?;
            }
            Ok(())
        } else {
            error!("Command failed with status: {}", output.status);
            if !output.stderr.is_empty() {
                write!(stderr, "{}", String::from_utf8_lossy(&output.stderr))?;
            }
            Err(anyhow!("Command execution failed"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm_generator::PermissionRequest;
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    // =========================================================================
    // Mock implementations
    // =========================================================================

    /// Mock process runner for testing.
    struct MockProcessRunner {
        output: Output,
        program_exists: bool,
    }

    impl MockProcessRunner {
        fn success(stdout: &str) -> Self {
            Self {
                output: Output {
                    status: ExitStatus::from_raw(0),
                    stdout: stdout.as_bytes().to_vec(),
                    stderr: vec![],
                },
                program_exists: true,
            }
        }

        fn failure(stderr: &str) -> Self {
            Self {
                output: Output {
                    status: ExitStatus::from_raw(1 << 8), // Exit code 1
                    stdout: vec![],
                    stderr: stderr.as_bytes().to_vec(),
                },
                program_exists: true,
            }
        }

        fn missing_program() -> Self {
            Self {
                output: Output {
                    status: ExitStatus::from_raw(0),
                    stdout: vec![],
                    stderr: vec![],
                },
                program_exists: false,
            }
        }
    }

    impl ProcessRunner for MockProcessRunner {
        fn run(&self, _program: &str, _args: &[&str]) -> Result<Output> {
            Ok(self.output.clone())
        }

        fn program_exists(&self, _program: &str) -> bool {
            self.program_exists
        }
    }

    /// Mock script provider for testing.
    struct MockScriptProvider {
        script: String,
    }

    impl MockScriptProvider {
        fn new(script: &str) -> Self {
            Self {
                script: script.to_string(),
            }
        }
    }

    impl ScriptProvider for MockScriptProvider {
        fn get_script(&self, _command: &GeneratedCommand) -> Result<String> {
            Ok(self.script.clone())
        }
    }

    /// Creates a test GeneratedCommand.
    fn test_command(name: &str, permissions: Vec<(&str, &str)>) -> GeneratedCommand {
        GeneratedCommand {
            name: name.to_string(),
            description: format!("Test command: {}", name),
            script_file: format!("{}.ts", name),
            permissions: permissions
                .into_iter()
                .map(|(perm, reason)| PermissionRequest {
                    permission: perm.to_string(),
                    reason: reason.to_string(),
                })
                .collect(),
        }
    }

    // =========================================================================
    // System command tests
    // =========================================================================

    #[test]
    fn test_execute_system_command_empty_args_returns_error() {
        let executor = Executor::new(false);
        let runner = MockProcessRunner::success("");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let result = executor.execute_system_command_with_runner(
            &[],
            &runner,
            &mut stdout,
            &mut stderr,
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No command provided"));
    }

    #[test]
    fn test_execute_system_command_success_writes_stdout() {
        let executor = Executor::new(false);
        let runner = MockProcessRunner::success("Hello, World!\n");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let result = executor.execute_system_command_with_runner(
            &["echo".to_string(), "Hello, World!".to_string()],
            &runner,
            &mut stdout,
            &mut stderr,
        );

        assert!(result.is_ok());
        assert_eq!(String::from_utf8_lossy(&stdout), "Hello, World!\n");
        assert!(stderr.is_empty());
    }

    #[test]
    fn test_execute_system_command_failure_writes_stderr() {
        let executor = Executor::new(false);
        let runner = MockProcessRunner::failure("Command not found\n");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let result = executor.execute_system_command_with_runner(
            &["nonexistent".to_string()],
            &runner,
            &mut stdout,
            &mut stderr,
        );

        assert!(result.is_err());
        assert!(stdout.is_empty());
        assert_eq!(String::from_utf8_lossy(&stderr), "Command not found\n");
    }

    // =========================================================================
    // Generated command tests
    // =========================================================================

    #[test]
    fn test_execute_generated_command_deno_not_installed() {
        let executor = Executor::new(false);
        let command = test_command("hello", vec![]);
        let script_provider = MockScriptProvider::new("console.log('Hello');");
        let runner = MockProcessRunner::missing_program();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let result = executor.execute_generated_command_with_deps(
            &command,
            &script_provider,
            &[],
            &runner,
            &mut stdout,
            &mut stderr,
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Deno is not installed"));
    }

    #[test]
    fn test_execute_generated_command_success() {
        let executor = Executor::new(false);
        let command = test_command("hello", vec![]);
        let script_provider = MockScriptProvider::new("console.log('Hello');");
        let runner = MockProcessRunner::success("Hello\n");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let result = executor.execute_generated_command_with_deps(
            &command,
            &script_provider,
            &[],
            &runner,
            &mut stdout,
            &mut stderr,
        );

        assert!(result.is_ok());
        assert_eq!(String::from_utf8_lossy(&stdout), "Hello\n");
    }

    #[test]
    fn test_execute_generated_command_verbose_shows_description() {
        let executor = Executor::new(true);
        let command = test_command("hello", vec![]);
        let script_provider = MockScriptProvider::new("console.log('Hello');");
        let runner = MockProcessRunner::success("");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let result = executor.execute_generated_command_with_deps(
            &command,
            &script_provider,
            &[],
            &runner,
            &mut stdout,
            &mut stderr,
        );

        assert!(result.is_ok());
        let output = String::from_utf8_lossy(&stdout);
        assert!(output.contains("Executing generated command"));
        assert!(output.contains("Test command: hello"));
    }

    #[test]
    fn test_execute_generated_command_verbose_shows_permissions() {
        let executor = Executor::new(true);
        let command = test_command("fetch", vec![
            ("--allow-net", "Network access"),
            ("--allow-read", "Read files"),
        ]);
        let script_provider = MockScriptProvider::new("fetch('http://example.com');");
        let runner = MockProcessRunner::success("");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let result = executor.execute_generated_command_with_deps(
            &command,
            &script_provider,
            &[],
            &runner,
            &mut stdout,
            &mut stderr,
        );

        assert!(result.is_ok());
        let output = String::from_utf8_lossy(&stdout);
        assert!(output.contains("Deno permissions required"));
        assert!(output.contains("--allow-net"));
        assert!(output.contains("--allow-read"));
    }

    #[test]
    fn test_execute_generated_command_non_verbose_no_extra_output() {
        let executor = Executor::new(false);
        let command = test_command("hello", vec![("--allow-read", "Read files")]);
        let script_provider = MockScriptProvider::new("console.log('Hello');");
        let runner = MockProcessRunner::success("Hello\n");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let result = executor.execute_generated_command_with_deps(
            &command,
            &script_provider,
            &[],
            &runner,
            &mut stdout,
            &mut stderr,
        );

        assert!(result.is_ok());
        // Only the command output, no verbose messages
        assert_eq!(String::from_utf8_lossy(&stdout), "Hello\n");
    }

    #[test]
    fn test_execute_generated_command_script_failure() {
        let executor = Executor::new(false);
        let command = test_command("broken", vec![]);
        let script_provider = MockScriptProvider::new("throw new Error('Oops');");
        let runner = MockProcessRunner::failure("Error: Oops\n");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let result = executor.execute_generated_command_with_deps(
            &command,
            &script_provider,
            &[],
            &runner,
            &mut stdout,
            &mut stderr,
        );

        assert!(result.is_err());
        assert_eq!(String::from_utf8_lossy(&stderr), "Error: Oops\n");
    }

    // =========================================================================
    // handle_output tests
    // =========================================================================

    #[test]
    fn test_handle_output_success_with_stdout() {
        let output = Output {
            status: ExitStatus::from_raw(0),
            stdout: b"output".to_vec(),
            stderr: vec![],
        };
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let result = Executor::handle_output(&output, &mut stdout, &mut stderr);

        assert!(result.is_ok());
        assert_eq!(stdout, b"output");
        assert!(stderr.is_empty());
    }

    #[test]
    fn test_handle_output_success_with_stderr() {
        let output = Output {
            status: ExitStatus::from_raw(0),
            stdout: vec![],
            stderr: b"warning".to_vec(),
        };
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let result = Executor::handle_output(&output, &mut stdout, &mut stderr);

        assert!(result.is_ok());
        assert!(stdout.is_empty());
        assert_eq!(stderr, b"warning");
    }

    #[test]
    fn test_handle_output_failure_returns_error() {
        let output = Output {
            status: ExitStatus::from_raw(1 << 8),
            stdout: vec![],
            stderr: b"error".to_vec(),
        };
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let result = Executor::handle_output(&output, &mut stdout, &mut stderr);

        assert!(result.is_err());
        assert_eq!(stderr, b"error");
    }
}