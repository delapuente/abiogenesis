use anyhow::{anyhow, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{info, warn};

#[derive(Debug, Deserialize)]
struct ClaudeResponse {
    name: String,
    description: String,
    script: String,
    permissions: Vec<PermissionRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub permission: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedCommand {
    pub name: String,
    pub description: String,
    pub script_file: String, // Path to the script file (relative to biomas directory)
    pub permissions: Vec<PermissionRequest>, // Deno permissions with explanations
}

#[derive(Debug)]
pub struct GenerationResult {
    pub command: GeneratedCommand,
    pub script_content: String,
}

#[async_trait]
pub trait CommandGenerator {
    async fn generate_command(&self, command_name: &str, args: &[String]) -> Result<GenerationResult>;
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


    async fn generate_command_impl(&self, command_name: &str, args: &[String]) -> Result<GenerationResult> {
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

    async fn call_claude_api(&self, command_name: &str, args: &[String], api_key: &str) -> Result<GenerationResult> {
        let prompt = self.build_unified_prompt(command_name, Some(args));
        let mut result = self.call_claude_api_with_prompt(&prompt, api_key).await?;
        // Override Claude's suggested name with the user's specified name
        result.command.name = command_name.to_string();
        result.command.script_file = format!("{}.ts", command_name);
        Ok(result)
    }

    fn build_unified_prompt(&self, request: &str, args: Option<&[String]>) -> String {
        let request_description = if let Some(args) = args {
            // Command mode: describe the request as creating a command with specific name and args
            format!("Create a command named '{}' that handles arguments {:?}", request, args)
        } else {
            // Conversational mode: use the user's natural language request directly
            request.to_string()
        };

        format!(
            "CRITICAL: Your response must be EXACTLY a JSON object. No explanations, no code blocks, no other text.

Based on this request: \"{}\"

Create a Deno/TypeScript command and suggest a short, descriptive command name.

RESPOND WITH EXACTLY THIS FORMAT (with your values):
{{
  \"name\": \"suggested-command-name\",
  \"description\": \"Brief description of what this command does\",
  \"script\": \"console.log('working code here');\",
  \"permissions\": [
    {{
      \"permission\": \"--allow-read\",
      \"reason\": \"Read files from the current directory\"
    }}
  ]
}}

RULES:
- Choose a clear, short command name (2-3 words max, kebab-case)
- Create real, working functionality - no placeholder code
- Use Deno APIs when needed
- Arguments available as Deno.args if the command should accept them
- Use MINIMAL permissions (empty [] preferred)
- Valid permission values: --allow-read, --allow-write, --allow-net, --allow-env, --allow-run
- For each permission, provide a clear reason why it's needed in user-friendly language
- Include try/catch for error handling
- CRITICAL: RESPOND ONLY WITH THE JSON OBJECT ABOVE - NO OTHER TEXT",
            request_description
        )
    }

    async fn call_claude_api_with_prompt(&self, prompt: &str, api_key: &str) -> Result<GenerationResult> {
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
                if let Ok(claude_response) = serde_json::from_str::<ClaudeResponse>(content) {
                    info!("Successfully parsed Claude-generated command");
                    let generation_result = GenerationResult {
                        command: GeneratedCommand {
                            name: claude_response.name.clone(),
                            description: claude_response.description.clone(),
                            script_file: format!("{}.ts", claude_response.name),
                            permissions: claude_response.permissions.clone(),
                        },
                        script_content: claude_response.script,
                    };
                    return Ok(generation_result);
                } else {
                    warn!("Failed to parse Claude response as ClaudeResponse: {}", content);
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
    async fn generate_command(&self, command_name: &str, args: &[String]) -> Result<GenerationResult> {
        info!("Generating command for: {} with args: {:?}", command_name, args);

        // In production: use real LLM API, in tests: use mock
        let generation_result = self.generate_command_impl(command_name, args).await?;

        Ok(generation_result)
    }
}

impl LlmGenerator {
    pub async fn generate_command_from_description(&self, description: &str) -> Result<GenerationResult> {
        info!("Generating command from description: {}", description);

        let config = crate::config::Config::load()?;

        // Check for mock mode
        if config.is_mock_mode() {
            info!("Using mock generator for conversational mode (ABIOGENESIS_USE_MOCK=1)");
            return Ok(MockGenerator::new().mock_generate_from_description(description));
        }

        // Production mode: require API key
        if let Some(api_key) = config.get_api_key() {
            info!("Using Claude API for conversational command generation");
            self.call_claude_api_for_description(description, api_key).await
        } else {
            return Err(anyhow!(
                "No Anthropic API key found for conversational mode. Please set it using one of these methods:\n\
                \n\
1. Set API key in config:\n\
   ergo --set-api-key sk-ant-your-key-here\n\
   \n\
2. Set environment variable:\n\
   export ANTHROPIC_API_KEY=sk-ant-your-key-here\n\
   \n\
3. Check current config:\n\
   ergo --config\n\
   \n\
Get your API key from: https://console.anthropic.com"
            ));
        }
    }

    async fn call_claude_api_for_description(&self, description: &str, api_key: &str) -> Result<GenerationResult> {
        let prompt = self.build_unified_prompt(description, None);
        self.call_claude_api_with_prompt(&prompt, api_key).await
    }

}

#[async_trait]
impl CommandGenerator for MockGenerator {
    async fn generate_command(&self, command_name: &str, args: &[String]) -> Result<GenerationResult> {
        Ok(self.mock_generate_command(command_name, args))
    }
}

impl MockGenerator {
    pub fn new() -> Self {
        Self
    }

    pub fn mock_generate_command(&self, command_name: &str, _args: &[String]) -> GenerationResult {
        // Mock implementation that generates Deno/TypeScript commands based on name patterns
        let (description, script, permissions): (String, String, Vec<PermissionRequest>) = match command_name {
            name if name.starts_with("git-") => {
                let git_action = &name[4..];
                (
                    format!("Custom git command for {}", git_action),
                    format!("const proc = new Deno.Command('git', {{ args: ['{}', ...Deno.args] }}); await proc.output();", git_action),
                    vec![PermissionRequest {
                        permission: "--allow-run=git".to_string(),
                        reason: "Execute git commands to perform version control operations".to_string(),
                    }],
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
                vec![
                    PermissionRequest {
                        permission: "--allow-read".to_string(),
                        reason: "Read files in the current directory to count them".to_string(),
                    },
                    PermissionRequest {
                        permission: "--allow-run=git".to_string(),
                        reason: "Run git commands to determine the current branch".to_string(),
                    },
                ],
            ),
            "weather" => (
                "Get current weather".to_string(),
                r#"
                const response = await fetch('https://wttr.in/?format=%l:+%c+%t');
                const weather = await response.text();
                console.log(`Weather: ${weather.trim()}`);
                "#.to_string(),
                vec![PermissionRequest {
                    permission: "--allow-net=wttr.in".to_string(),
                    reason: "Access weather data from the wttr.in service".to_string(),
                }],
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

        GenerationResult {
            command: GeneratedCommand {
                name: command_name.to_string(),
                description,
                script_file: format!("{}.ts", command_name),
                permissions,
            },
            script_content: script,
        }
    }

    pub fn mock_generate_from_description(&self, description: &str) -> GenerationResult {
        // Mock implementation for conversational mode - analyze description and suggest command
        let (command_name, desc_text, script, permissions): (String, String, String, Vec<PermissionRequest>) = if description.contains("timestamp") || description.contains("time") {
            (
                "show-time".to_string(),
                "Display current timestamp".to_string(),
                "const now = new Date(); console.log(now.toISOString());".to_string(),
                vec![],
            )
        } else if (description.contains("json") && description.contains("format")) || (description.contains("JSON") && description.contains("format")) {
            (
                "format-json".to_string(),
                "Format JSON input with proper indentation".to_string(),
                "try { const data = JSON.parse(Deno.args[0] || '{}'); console.log(JSON.stringify(data, null, 2)); } catch (err) { console.error('Invalid JSON:', err.message); }".to_string(),
                vec![],
            )
        } else if description.contains("list") && description.contains("file") {
            (
                "list-files".to_string(),
                "List files in current directory".to_string(),
                "try { for await (const entry of Deno.readDir('.')) { console.log(entry.name); } } catch (err) { console.error(err); }".to_string(),
                vec![PermissionRequest {
                    permission: "--allow-read".to_string(),
                    reason: "Read directory contents to list files".to_string(),
                }],
            )
        } else if description.contains("random") || description.contains("uuid") || description.contains("UUID") {
            (
                "generate-id".to_string(),
                "Generate a random UUID".to_string(),
                "console.log(crypto.randomUUID());".to_string(),
                vec![],
            )
        } else if description.contains("hello") || description.contains("greet") {
            (
                "greet-user".to_string(),
                "Greet the user".to_string(),
                "console.log('Hello! This command was generated from your description.');".to_string(),
                vec![],
            )
        } else {
            // Generic fallback based on description
            let words: Vec<&str> = description.split_whitespace().take(3).collect();
            let command_name = words.join("-").to_lowercase();
            (
                command_name.clone(),
                format!("Generated command from: {}", description),
                format!("console.log('Mock command for: {}');", description),
                vec![],
            )
        };

        GenerationResult {
            command: GeneratedCommand {
                name: command_name.clone(),
                description: desc_text,
                script_file: format!("{}.ts", command_name),
                permissions,
            },
            script_content: script,
        }
    }
}