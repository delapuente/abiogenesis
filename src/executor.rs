use crate::llm_generator::GeneratedCommand;
use anyhow::{anyhow, Result};
use std::process::Command;
use tracing::{info, warn, error};

pub struct Executor;

impl Executor {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute_system_command(&self, args: &[String]) -> Result<()> {
        if args.is_empty() {
            return Err(anyhow!("No command provided"));
        }

        let command_name = &args[0];
        let command_args = &args[1..];

        info!("Executing system command: {} {:?}", command_name, command_args);

        let mut cmd = Command::new(command_name);
        cmd.args(command_args);

        let output = cmd.output()?;

        if output.status.success() {
            if !output.stdout.is_empty() {
                print!("{}", String::from_utf8_lossy(&output.stdout));
            }
            if !output.stderr.is_empty() {
                eprint!("{}", String::from_utf8_lossy(&output.stderr));
            }
        } else {
            error!("Command failed with status: {}", output.status);
            if !output.stderr.is_empty() {
                eprint!("{}", String::from_utf8_lossy(&output.stderr));
            }
            return Err(anyhow!("Command execution failed"));
        }

        Ok(())
    }

    pub async fn execute_cached_command(&self, command: GeneratedCommand, args: &[String]) -> Result<()> {
        info!("Executing cached command: {} - {}", command.name, command.description);
        self.execute_generated_command(&command, args).await
    }

    pub async fn execute_generated_command(&self, command: &GeneratedCommand, args: &[String]) -> Result<()> {
        if !command.safe {
            warn!("Attempting to execute potentially unsafe command: {}", command.name);
            println!("⚠️  Warning: This generated command is marked as potentially unsafe.");
            println!("Command: {}", command.name);
            println!("Description: {}", command.description);
            println!("Script preview:");
            println!("{}", command.script);
            println!("Do you want to proceed? (y/N)");
            
            // In a real implementation, you'd want to get user input here
            // For now, we'll skip unsafe commands
            return Err(anyhow!("Unsafe command execution blocked"));
        }

        info!("Executing generated command: {} - {}", command.name, command.description);
        println!("🤖 Executing generated command: {}", command.description);

        // Show permissions that will be requested
        if !command.permissions.is_empty() {
            println!("🔒 Deno permissions required: {}", command.permissions.join(" "));
        }

        self.execute_deno_script(&command.script, &command.permissions, args).await
    }

    async fn execute_deno_script(&self, script: &str, permissions: &[String], args: &[String]) -> Result<()> {
        // Check if deno is available
        if which::which("deno").is_err() {
            return Err(anyhow!("Deno is not installed. Please install Deno to execute generated commands."));
        }

        // Create a temporary file for the script to maintain sandboxing
        use std::fs;
        use std::env;
        
        let temp_dir = env::temp_dir();
        let script_path = temp_dir.join(format!("ergo_script_{}.ts", std::process::id()));
        
        // Write script to temporary file
        fs::write(&script_path, script)?;

        let mut cmd = Command::new("deno");
        cmd.arg("run");
        
        // Add permissions - this maintains the sandbox
        for permission in permissions {
            cmd.arg(permission);
        }
        
        cmd.arg(&script_path);
        cmd.args(args);

        let output = cmd.output()?;
        
        // Clean up temporary file
        let _ = fs::remove_file(&script_path);

        if output.status.success() {
            if !output.stdout.is_empty() {
                print!("{}", String::from_utf8_lossy(&output.stdout));
            }
            if !output.stderr.is_empty() {
                eprint!("{}", String::from_utf8_lossy(&output.stderr));
            }
        } else {
            error!("Generated Deno script failed with status: {}", output.status);
            if !output.stderr.is_empty() {
                eprint!("{}", String::from_utf8_lossy(&output.stderr));
            }
            return Err(anyhow!("Generated Deno script execution failed"));
        }

        Ok(())
    }
}