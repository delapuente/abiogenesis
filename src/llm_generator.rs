use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedCommand {
    pub name: String,
    pub description: String,
    pub script: String,
    pub permissions: Vec<String>, // Deno permissions like --allow-read, --allow-net
    pub safe: bool,              // whether the command is safe to execute
}

pub struct LlmGenerator {
    client: Client,
}

impl LlmGenerator {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    pub async fn generate_command(&self, command_name: &str, args: &[String]) -> Result<GeneratedCommand> {
        info!("Generating command for: {} with args: {:?}", command_name, args);

        // For now, let's create a mock implementation
        // In a real implementation, this would call an LLM API (OpenAI, Anthropic, etc.)
        let generated_command = self.mock_generate_command(command_name, args).await?;

        if !generated_command.safe {
            warn!("Generated command marked as potentially unsafe: {}", command_name);
        }

        Ok(generated_command)
    }

    async fn mock_generate_command(&self, command_name: &str, _args: &[String]) -> Result<GeneratedCommand> {
        // Mock implementation that generates Deno/TypeScript commands based on name patterns
        let (description, script, permissions) = match command_name {
            name if name.starts_with("git-") => {
                let git_action = &name[4..];
                (
                    format!("Custom git command for {}", git_action),
                    format!("const proc = new Deno.Command('git', {{ args: ['{}', ...Deno.args] }}); await proc.output();", git_action),
                    vec!["--allow-run=git".to_string()],
                )
            }
            "hello" => (
                "Greet the user".to_string(),
                "console.log(`Hello from ergo! Arguments: ${Deno.args.join(' ')}`);".to_string(),
                vec![], // No permissions needed for simple console output
            ),
            "timestamp" => (
                "Show current timestamp".to_string(),
                "const now = new Date(); console.log(now.toISOString().replace('T', '_').replace(/:/g, '-').split('.')[0]);".to_string(),
                vec![], // No permissions needed
            ),
            "project-info" => (
                "Show project information".to_string(),
                r#"
                try {
                    const cwd = Deno.cwd();
                    const projectName = cwd.split('/').pop() || 'unknown';
                    console.log(`Project: ${projectName}`);
                    
                    try {
                        const git = new Deno.Command('git', { args: ['branch', '--show-current'] });
                        const gitOutput = await git.output();
                        const branch = new TextDecoder().decode(gitOutput.stdout).trim();
                        console.log(`Git branch: ${branch || 'not a git repo'}`);
                    } catch {
                        console.log('Git branch: not a git repo');
                    }
                    
                    let fileCount = 0;
                    for await (const entry of Deno.readDir('.')) {
                        if (entry.isFile) fileCount++;
                    }
                    console.log(`Files: ${fileCount}`);
                } catch (error) {
                    console.error('Error:', error.message);
                }
                "#.to_string(),
                vec!["--allow-read".to_string(), "--allow-run=git".to_string()],
            ),
            "weather" => (
                "Get current weather".to_string(),
                r#"
                const response = await fetch('https://wttr.in/?format=%l:+%c+%t');
                const weather = await response.text();
                console.log(`Weather: ${weather.trim()}`);
                "#.to_string(),
                vec!["--allow-net=wttr.in".to_string()],
            ),
            "uuid" => (
                "Generate a UUID".to_string(),
                "console.log(crypto.randomUUID());".to_string(),
                vec![], // No permissions needed for crypto API
            ),
            _ => (
                format!("Generated command for {}", command_name),
                format!("console.log('This is a generated command: {}');", command_name),
                vec![],
            )
        };

        Ok(GeneratedCommand {
            name: command_name.to_string(),
            description,
            script,
            permissions,
            safe: true, // Mark as safe for demo purposes
        })
    }

    #[allow(dead_code)]
    async fn call_openai_api(&self, command_name: &str, args: &[String]) -> Result<GeneratedCommand> {
        let api_key = env::var("OPENAI_API_KEY")
            .map_err(|_| anyhow!("OPENAI_API_KEY environment variable not set"))?;

        let prompt = format!(
            "Generate a shell command for '{}' with arguments {:?}. 
            Return a JSON object with: name, description, script (executable shell script), language, safe (boolean).
            Make the script practical and useful. If unsafe, set safe to false.",
            command_name, args
        );

        let request_body = json!({
            "model": "gpt-3.5-turbo",
            "messages": [
                {
                    "role": "system",
                    "content": "You are a helpful assistant that generates shell commands based on user intent."
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "max_tokens": 500
        });

        let _response = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        // Parse response and extract generated command
        // This is a simplified implementation
        todo!("Implement OpenAI API response parsing")
    }
}