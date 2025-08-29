use anyhow::{anyhow, Result};
use async_trait::async_trait;
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
}

#[async_trait]
pub trait CommandGenerator {
    async fn generate_command(&self, command_name: &str, args: &[String]) -> Result<GeneratedCommand>;
}

pub struct LlmGenerator {
    client: Client,
}

pub struct MockGenerator;

impl LlmGenerator {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }


    async fn generate_command_impl(&self, command_name: &str, args: &[String]) -> Result<GeneratedCommand> {
        let config = crate::config::Config::load()?;

        // Check for mock mode
        if config.is_mock_mode() {
            info!("Using mock generator (ABIOGENESIS_USE_MOCK=1)");
            return Ok(MockGenerator::new().mock_generate_command(command_name, args));
        }

        // Production mode: require API key
        if let Some(api_key) = config.get_api_key() {
            info!("Using Claude API for command generation");
            self.call_claude_api(command_name, args, api_key).await
        } else {
            return Err(anyhow!(
                "No Anthropic API key found. Please set it using one of these methods:
                
1. Set API key in config:
   ergo --set-api-key sk-ant-your-key-here
   
2. Set environment variable:
   export ANTHROPIC_API_KEY=sk-ant-your-key-here
   
3. Check current config:
   ergo --config
   
Get your API key from: https://console.anthropic.com"
            ));
        }
    }

    async fn call_claude_api(&self, command_name: &str, args: &[String], api_key: &str) -> Result<GeneratedCommand> {
        let prompt = format!(
            "CRITICAL: Your response must be EXACTLY a JSON object. No explanations, no code blocks, no other text.

Generate a Deno/TypeScript command for '{}' with arguments {:?}.

RESPOND WITH EXACTLY THIS FORMAT (with your values):
{{
  \"name\": \"{}\",
  \"description\": \"Brief description\",
  \"script\": \"console.log('working code here');\",
  \"permissions\": []
}}

RULES:
- Create real, working functionality - no placeholder code
- Use Deno APIs when needed
- Arguments available as Deno.args
- Use MINIMAL permissions (empty [] preferred)
- Valid permissions: --allow-read, --allow-write, --allow-net, --allow-env, --allow-run
- Include try/catch for error handling
- CRITICAL: RESPOND ONLY WITH THE JSON OBJECT ABOVE - NO OTHER TEXT",
            command_name, args, command_name
        );

        let request_body = json!({
            "model": "claude-3-haiku-20240307",
            "max_tokens": 1500,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ]
        });

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("content-type", "application/json")
            .header("anthropic-version", "2023-06-01")
            .json(&request_body)
            .send()
            .await?;

        let response_text = response.text().await?;
        info!("Claude API response: {}", response_text);
        
        // Parse Claude's response
        if let Ok(claude_response) = serde_json::from_str::<serde_json::Value>(&response_text) {
            if let Some(content) = claude_response.get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(|text| text.as_str()) {
                
                info!("Extracted content from Claude: {}", content);
                
                // Try to parse the generated JSON
                if let Ok(generated_command) = serde_json::from_str::<GeneratedCommand>(content) {
                    info!("Successfully parsed Claude-generated command");
                    return Ok(generated_command);
                } else {
                    warn!("Failed to parse Claude response as GeneratedCommand: {}", content);
                }
            } else {
                warn!("Failed to extract content from Claude response");
            }
        } else {
            warn!("Failed to parse Claude response as JSON: {}", response_text);
        }
        
        // If Claude response parsing fails, return an error instead of a useless fallback
        Err(anyhow!(
            "Failed to parse Claude API response. The generated command was not in the expected JSON format.\n\
             Raw response: {}\n\
             This usually means the prompt needs adjustment or the API returned an error.",
            response_text
        ))
    }
}

#[async_trait]
impl CommandGenerator for LlmGenerator {
    async fn generate_command(&self, command_name: &str, args: &[String]) -> Result<GeneratedCommand> {
        info!("Generating command for: {} with args: {:?}", command_name, args);

        // In production: use real LLM API, in tests: use mock
        let generated_command = self.generate_command_impl(command_name, args).await?;

        Ok(generated_command)
    }
}

#[async_trait]
impl CommandGenerator for MockGenerator {
    async fn generate_command(&self, command_name: &str, args: &[String]) -> Result<GeneratedCommand> {
        Ok(self.mock_generate_command(command_name, args))
    }
}

impl MockGenerator {
    pub fn new() -> Self {
        Self
    }

    pub fn mock_generate_command(&self, command_name: &str, _args: &[String]) -> GeneratedCommand {
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

        GeneratedCommand {
            name: command_name.to_string(),
            description,
            script,
            permissions,
        }
    }
}