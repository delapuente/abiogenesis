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

impl LlmGenerator {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }


    async fn generate_command_impl(&self, command_name: &str, args: &[String]) -> Result<GenerationResult> {
        let config = crate::config::Config::load()?;

        // Require API key
        if let Some(api_key) = config.get_api_key() {
            info!("Using Claude API for command generation");
            self.call_claude_api(command_name, args, api_key).await
        } else {
            Err(anyhow!(
                "No Anthropic API key found. Please set it using one of these methods:
                
1. Set API key in config:
   ergo --set-api-key sk-ant-your-key-here
   
2. Set environment variable:
   export ANTHROPIC_API_KEY=sk-ant-your-key-here
   
3. Check current config:
   ergo --config
   
Get your API key from: https://console.anthropic.com"
            ))
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

        // Require API key
        if let Some(api_key) = config.get_api_key() {
            info!("Using Claude API for conversational command generation");
            self.call_claude_api_for_description(description, api_key).await
        } else {
            Err(anyhow!(
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
            ))
        }
    }

    async fn call_claude_api_for_description(&self, description: &str, api_key: &str) -> Result<GenerationResult> {
        let prompt = self.build_unified_prompt(description, None);
        self.call_claude_api_with_prompt(&prompt, api_key).await
    }

}